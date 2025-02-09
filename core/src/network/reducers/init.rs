use crate::{
    action::{Action, ActionWrapper},
    network::state::NetworkState,
    state::State,
};
use holochain_net::{
    connection::{
        json_protocol::{JsonProtocol, TrackDnaData},
        net_connection::NetSend,
    },
    p2p_network::P2pNetwork,
};
use std::sync::{Arc, Mutex};

pub fn reduce_init(state: &mut NetworkState, _root_state: &State, action_wrapper: &ActionWrapper) {
    let action = action_wrapper.action();
    let network_settings = unwrap_to!(action => Action::InitNetwork);
    let mut network = P2pNetwork::new(
        network_settings.handler.clone(),
        &network_settings.p2p_config,
    )
    .unwrap();

    // Configure network logger
    // Enable this for debugging network
    //    {
    //        let mut tweetlog = TWEETLOG.write().unwrap();
    //        tweetlog.set(LogLevel::Debug, None);
    //        // set level per tag
    //        tweetlog.set(LogLevel::Debug, Some("memory_server".to_string()));
    //        tweetlog.listen_to_tag("memory_server", Tweetlog::console);
    //        tweetlog.listen(Tweetlog::console);
    //        tweetlog.i("TWEETLOG ENABLED");
    //    }

    let json = JsonProtocol::TrackDna(TrackDnaData {
        dna_address: network_settings.dna_address.clone(),
        agent_id: network_settings.agent_id.clone().into(),
    });

    let _ = network.send(json.into()).and_then(|_| {
        state.network = Some(Arc::new(Mutex::new(network)));
        state.dna_address = Some(network_settings.dna_address.clone());
        state.agent_id = Some(network_settings.agent_id.clone());
        Ok(())
    });
}

#[cfg(test)]
pub mod test {
    use self::tempfile::tempdir;
    use super::*;
    use crate::{
        context::Context,
        logger::test_logger,
        persister::SimplePersister,
        state::{test_store, State},
    };
    use holochain_core_types::agent::AgentId;
    use holochain_net::{connection::net_connection::NetHandler, p2p_config::P2pConfig};
    use holochain_persistence_api::cas::content::{Address, AddressableContent};
    use holochain_persistence_file::{cas::file::FilesystemStorage, eav::file::EavFileStorage};
    use std::sync::{Mutex, RwLock};
    use tempfile;

    fn test_context() -> Arc<Context> {
        let file_storage = Arc::new(RwLock::new(
            FilesystemStorage::new(tempdir().unwrap().path().to_str().unwrap()).unwrap(),
        ));
        let mut context = Context::new(
            AgentId::generate_fake("Terence"),
            test_logger(),
            Arc::new(Mutex::new(SimplePersister::new(file_storage.clone()))),
            file_storage.clone(),
            file_storage.clone(),
            Arc::new(RwLock::new(
                EavFileStorage::new(tempdir().unwrap().path().to_str().unwrap().to_string())
                    .unwrap(),
            )),
            P2pConfig::new_with_unique_memory_backend(),
            None,
            None,
        );

        let global_state = Arc::new(RwLock::new(State::new(Arc::new(context.clone()))));
        context.set_state(global_state.clone());
        Arc::new(context)
    }

    #[test]
    pub fn should_wait_for_protocol_p2p_ready() {
        let context: Arc<Context> = test_context();
        let dna_address: Address = context.agent_id.address();
        let agent_id = context.agent_id.content().to_string();
        let handler = NetHandler::new(Box::new(|_| Ok(())));
        let network_settings = crate::action::NetworkSettings {
            p2p_config: context.p2p_config.clone(),
            dna_address,
            agent_id,
            handler,
        };
        let action_wrapper = ActionWrapper::new(Action::InitNetwork(network_settings));

        let mut network_state = NetworkState::new();
        let root_state = test_store(context.clone());
        let result = reduce_init(&mut network_state, &root_state, &action_wrapper);

        assert_eq!(result, ());
    }

}
