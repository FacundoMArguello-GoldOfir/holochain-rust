use holochain_core_types::{
    agent::AgentId,
    };

use holochain_persistence_api::cas::content::{Address, AddressableContent};

use holochain_dpki::{
    key_bundle::KeyBundle,
    SEED_SIZE,
};
use lib3h_sodium::secbuf::SecBuf;
use jsonrpc_ws_server::jsonrpc_core::{self, types::params::Params, IoHandler};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

lazy_static! {
    pub static ref TEST_AGENT_KEYBUNDLES: Mutex<HashMap<Address, Arc<Mutex<KeyBundle>>>>
        = Mutex::new(HashMap::new());
}

pub fn registered_test_agent<S: Into<String>>(nick: S) -> AgentId {
    let nick = nick.into();
    // Create deterministic seed from nick:
    let mut seed = SecBuf::with_insecure(SEED_SIZE);
    let nick_bytes = nick.as_bytes();
    let seed_bytes: Vec<u8> = (1..SEED_SIZE).map(|num| {
        if num <= nick_bytes.len(){
            nick_bytes[num-1]
        } else {
            num as u8
        }
    }).collect();

    seed.write(0, seed_bytes.as_slice())
        .expect("SecBuf must be writeable");

    // Create KeyBundle from seed
    let keybundle = KeyBundle::new_from_seed_buf(&mut seed).unwrap();
    let agent_id = AgentId::new(&nick, keybundle.get_id());

    // Register key in static TEST_AGENT_KEYS
    TEST_AGENT_KEYBUNDLES.lock().unwrap().insert(agent_id.address(), Arc::new(Mutex::new(keybundle)));
    agent_id
}

/// This is a local mock for the `agent/sign` conductor API function.
/// It creates a syntactically equivalent signature using dpki::Keypair
/// but with key generated from a static/deterministic mock seed.
/// This enables unit testing of core code that creates signatures without
/// depending on the conductor or actual key files.
pub fn mock_signer(payload: String, agent_id: &AgentId) -> String { TEST_AGENT_KEYBUNDLES
        .lock()
        .unwrap()
        .get(&agent_id.address())
        .expect(format!(
                "Agent {:?} not found in mock registry. \
                 Test agent keys need to be registered first.", agent_id).as_str())
        .lock()
        .map(|mut keybundle| {
            // Convert payload string into a SecBuf
            let mut message = SecBuf::with_insecure_from_string(payload);

            // Create signature
            let mut message_signed = keybundle.sign(&mut message).expect("Mock signing failed.");
            let message_signed = message_signed.read_lock();

            // Return as base64 encoded string
            base64::encode(&**message_signed)
        })
        .unwrap()
}

/// Wraps `fn mock_signer(String) -> String` in an `IoHandler` to mock the conductor API
/// in a way that core can safely assume the conductor API to be present with at least
/// the `agent/sign` method.
pub fn mock_conductor_api(agent_id: AgentId) -> IoHandler {
    let mut handler = IoHandler::new();
    handler.add_method("agent/sign", move |params| {
        let params_map = match params {
            Params::Map(map) => Ok(map),
            _ => Err(jsonrpc_core::Error::invalid_params("expected params map")),
        }?;


        let key = "payload";
        let payload = Ok(params_map
            .get(key)
            .ok_or(jsonrpc_core::Error::invalid_params(format!(
                "`{}` param not provided",
                key
            )))?
            .as_str()
            .ok_or(jsonrpc_core::Error::invalid_params(format!(
                "`{}` is not a valid json string",
                key
            )))?
            .to_string())?;

        Ok(json!({"payload": payload, "signature": mock_signer(payload, &agent_id)}))
    });
    handler
}
