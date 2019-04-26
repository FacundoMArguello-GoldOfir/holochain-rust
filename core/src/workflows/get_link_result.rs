use crate::{
    context::Context, network::actions::get_links::get_links,
    workflows::get_entry_result::get_entry_with_meta_workflow,
};

use holochain_core_types::{
    cas::content::Address, crud_status::CrudStatus, entry::EntryWithMetaAndHeader,
    error::HolochainError, time::Timeout,
};
use holochain_wasm_utils::api_serialization::get_links::{
    GetLinksArgs, GetLinksResult, LinksResult, LinksStatusRequestKind,
};
use std::sync::Arc;

pub async fn get_link_result_workflow<'a>(
    context: &'a Arc<Context>,
    link_args: &'a GetLinksArgs,
) -> Result<GetLinksResult, HolochainError> {
    let links = await!(get_links(
        context.clone(),
        link_args.entry_address.clone(),
        link_args.tag.clone(),
        link_args.options.timeout.clone()
    ))?;

    let (link_results, errors): (Vec<_>, Vec<_>) = links
        .iter()
        .map(|link| {
            get_latest_entry(
                context.clone(),
                link.clone(),
                link_args.options.timeout.clone(),
            )
            .map(|link_entry_result_option| {
                link_entry_result_option.map(|link_entry| {
                    let headers = if link_args.options.headers {
                        link_entry.headers
                    } else {
                        Vec::new()
                    };
                    Ok(LinksResult {
                        address: link.clone(),
                        headers,
                        crud_status: link_entry.entry_with_meta.crud_status,
                        crud_link: link_entry.entry_with_meta.maybe_link_update_delete,
                    })
                })
            })
            .unwrap_or(None)
            .unwrap_or(Err(HolochainError::ErrorGeneric(
                "Could not crud information for link".to_string(),
            )))
        })
        .partition(Result::is_ok);

    if errors.is_empty() {
        Ok(GetLinksResult::new(
            link_results
                .into_iter()
                .map(|s| s.unwrap())
                .filter(|link_result| match link_args.options.status_request {
                    LinksStatusRequestKind::All => true,
                    LinksStatusRequestKind::Deleted => {
                        link_result.crud_status == CrudStatus::Deleted
                    }
                    LinksStatusRequestKind::Live => link_result.crud_status == CrudStatus::Live,
                })
                .collect(),
        ))
    } else {
        Err(HolochainError::ErrorGeneric(
            "Could not get links".to_string(),
        ))
    }
}

pub fn get_latest_entry(
    context: Arc<Context>,
    address: Address,
    timeout: Timeout,
) -> Result<Option<EntryWithMetaAndHeader>, HolochainError> {
    let entry_with_meta_and_header =
        context.block_on(get_entry_with_meta_workflow(&context, &address, &timeout))?;
    entry_with_meta_and_header
        .map(|entry_meta_header| {
            if let Some(maybe_link_update) =
                entry_meta_header.entry_with_meta.maybe_link_update_delete
            {
                get_latest_entry(context.clone(), maybe_link_update, timeout)
            } else {
                entry_meta_header
                    .headers
                    .first()
                    .map(|first_chain_header| {
                        first_chain_header
                            .link_update_delete()
                            .map(|link| {
                                context.block_on(get_entry_with_meta_workflow(
                                    &context, &link, &timeout,
                                ))
                            })
                            .unwrap_or(Ok(Some(entry_meta_header.clone())))
                    })
                    .unwrap_or(Err(HolochainError::ErrorGeneric(
                        "disjointed link update".to_string(),
                    )))
            }
        })
        .unwrap_or(Ok(None))
}
