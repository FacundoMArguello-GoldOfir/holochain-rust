use crate::{
    context::Context,
    network::{entry_aspect::EntryAspect, entry_with_header::EntryWithHeader},
    workflows::{
        hold_entry::hold_entry_workflow, hold_entry_remove::hold_remove_workflow,
        hold_entry_update::hold_update_workflow, hold_link::hold_link_workflow,
        remove_link::remove_link_workflow,
    },
};
use holochain_core_types::entry::{deletion_entry::DeletionEntry, Entry};
use holochain_json_api::json::JsonString;
use holochain_net::connection::json_protocol::StoreEntryAspectData;
use holochain_persistence_api::cas::content::AddressableContent;
use std::{convert::TryInto, sync::Arc, thread};

/// The network requests us to store (i.e. hold) the given entry aspect data.
pub fn handle_store(dht_data: StoreEntryAspectData, context: Arc<Context>) {
    let aspect_json =
        JsonString::from_json(&String::from_utf8(dht_data.entry_aspect.aspect).unwrap());
    if let Ok(aspect) = aspect_json.clone().try_into() {
        match aspect {
            EntryAspect::Content(entry, header) => {
                context
                    .log("debug/net/handle: handle_store: Got EntryAspect::Content. processing...");
                let entry_with_header = EntryWithHeader { entry, header };
                thread::spawn(move || {
                    match context.block_on(hold_entry_workflow(&entry_with_header, context.clone()))
                    {
                        Err(error) => context.log(format!("err/net/dht: {}", error)),
                        _ => (),
                    }
                });
            }
            EntryAspect::Header(header) => {
                panic!(format!("unimplemented store aspect Header: {:?}", header));
            }
            EntryAspect::LinkAdd(link_data, header) => {
                context
                    .log("debug/net/handle: handle_store: Got EntryAspect::LinkAdd. processing...");
                let entry = Entry::LinkAdd(link_data);
                if entry.address() != *header.entry_address() {
                    context.log("err/net/handle: handle_store: Got EntryAspect::LinkAdd with non-matching LinkData and ChainHeader! Hash of content in header does not match content! Ignoring.");
                    return;
                }
                let entry_with_header = EntryWithHeader { entry, header };
                thread::spawn(move || {
                    match context.block_on(hold_link_workflow(&entry_with_header, &context.clone()))
                    {
                        Err(error) => context.log(format!("err/net/dht: {}", error)),
                        _ => (),
                    }
                });
            }
            EntryAspect::LinkRemove((link_data, links_to_remove), header) => {
                context.log(
                    "debug/net/handle: handle_store: Got EntryAspect::LinkRemove. processing...",
                );
                let entry = Entry::LinkRemove((link_data, links_to_remove));
                let entry_with_header = EntryWithHeader { entry, header };
                thread::spawn(move || {
                    if let Err(error) =
                        context.block_on(remove_link_workflow(&entry_with_header, &context.clone()))
                    {
                        context.log(format!("err/net/dht: {}", error))
                    }
                });
            }
            EntryAspect::Update(entry, header) => {
                context
                    .log("debug/net/handle: handle_store: Got EntryAspect::Update. processing...");
                let entry_with_header = EntryWithHeader { entry, header };
                thread::spawn(move || {
                    if let Err(error) =
                        context.block_on(hold_update_workflow(entry_with_header, context.clone()))
                    {
                        context.log(format!("err/net/dht: {}", error))
                    }
                });
            }
            EntryAspect::Deletion(header) => {
                context.log(
                    "debug/net/handle: handle_store: Got EntryAspect::Deletion. processing...",
                );
                // reconstruct the deletion entry from the header.
                let deleted_entry_address = match header.link_update_delete() {
                    None => {
                        context.log("err/net/handle: handle_store: Got EntryAspect::Deletion with header that has no deletion link! Ignoring.");
                        return;
                    }
                    Some(address) => address,
                };

                let entry = Entry::Deletion(DeletionEntry::new(deleted_entry_address));
                let entry_with_header = EntryWithHeader { entry, header };
                thread::spawn(move || {
                    if let Err(error) =
                        context.block_on(hold_remove_workflow(entry_with_header, context.clone()))
                    {
                        context.log(format!("err/net/handle_store: {}", error))
                    }
                });
            }
        }
    } else {
        context.log(format!(
            "err/net/handle_store: Unable to parse entry aspect: {}",
            aspect_json
        ))
    }
}

/*
/// The network requests us to store meta information (links/CRUD/etc) for an
/// entry that we hold.
pub fn handle_store_meta(dht_meta_data: DhtMetaData, context: Arc<Context>) {
    let attr = dht_meta_data.clone().attribute;
    // @TODO: If network crates will switch to using the `Attribute` enum,
    // we can match on the enum directly
    if attr == Attribute::Link.to_string() {
        context.log("debug/net/handle: HandleStoreMeta: got LINK. processing...");
        // TODO: do a loop on content once links properly implemented
        assert_eq!(dht_meta_data.content_list.len(), 1);
        let entry_with_header: EntryWithHeader = serde_json::from_str(
            &serde_json::to_string(&dht_meta_data.content_list[0])
                .expect("dht_meta_data should be EntryWithHeader"),
        )
        .expect("dht_meta_data should be EntryWithHeader");
        thread::spawn(move || {
            match context.block_on(hold_link_workflow(&entry_with_header, &context.clone())) {
                Err(error) => context.log(format!("err/net/dht: {}", error)),
                _ => (),
            }
        });
    } else if attr == Attribute::LinkRemove.to_string() {
        context.log("debug/net/handle: HandleStoreMeta: got LINK REMOVAL. processing...");
        // TODO: do a loop on content once links properly implemented
        assert_eq!(dht_meta_data.content_list.len(), 1);
        let entry_with_header: EntryWithHeader = serde_json::from_str(
            //should be careful doing slice access, it might panic
            &serde_json::to_string(&dht_meta_data.content_list[0])
                .expect("dht_meta_data should be EntryWithHader"),
        )
        .expect("dht_meta_data should be EntryWithHader");
        thread::spawn(move || {
            if let Err(error) =
                context.block_on(remove_link_workflow(&entry_with_header, &context.clone()))
            {
                context.log(format!("err/net/dht: {}", error))
            }
        });
    } else if CrudStatus::from_str(&attr)
        .expect("Could not convert deleted attribute to CrudStatus")
        == CrudStatus::Deleted
    {
        context.log("debug/net/handle: HandleStoreMeta: got CRUD STATUS. processing...");

        let entry_with_header: EntryWithHeader = serde_json::from_str(
            //should be careful doing slice access, it might panic
            &serde_json::to_string(&dht_meta_data.content_list[0])
                .expect("dht_meta_data should be EntryWithHader"),
        )
        .expect("dht_meta_data should be EntryWithHader");
        thread::spawn(move || {
            if let Err(error) =
                context.block_on(hold_remove_workflow(entry_with_header, context.clone()))
            {
                context.log(format!("err/net/dht: {}", error))
            }
        });
    } else if CrudStatus::from_str(&attr)
        .expect("Could not convert modified attribute to CrudStatus")
        == CrudStatus::Modified
    {
        context.log("debug/net/handle: HandleStoreMeta: got CRUD LINK. processing...");
        let entry_with_header: EntryWithHeader = serde_json::from_str(
            //should be careful doing slice access, it might panic
            &serde_json::to_string(&dht_meta_data.content_list[0])
                .expect("dht_meta_data should be EntryWithHader"),
        )
        .expect("dht_meta_data should be EntryWithHader");
        thread::spawn(move || {
            if let Err(error) =
                context.block_on(hold_update_workflow(entry_with_header, context.clone()))
            {
                context.log(format!("err/net/dht: {}", error))
            }
        });
    }
}
*/
