use std::collections::HashMap;
use std::sync::Mutex;
use worker::*;

/// In-memory peer map: network_name -> (peer_id -> WebSocket)
/// Used as hot cache. KV is used for persistence and cross-instance discovery.
static PEER_MAP: Mutex<HashMap<String, HashMap<String, WebSocket>>> =
    Mutex::new(HashMap::new());

/// Register a peer's websocket in the in-memory map.
pub fn register_peer(network: &str, peer_id: &str, ws: WebSocket) {
    let mut map = PEER_MAP.lock().unwrap();
    map.entry(network.to_string())
        .or_default()
        .insert(peer_id.to_string(), ws);
}

/// Unregister a peer from the in-memory map.
pub fn unregister_peer(network: &str, peer_id: &str) {
    let mut map = PEER_MAP.lock().unwrap();
    if let Some(net_peers) = map.get_mut(network) {
        net_peers.remove(peer_id);
        if net_peers.is_empty() {
            map.remove(network);
        }
    }
}

/// Get a peer's websocket by network and peer_id.
pub fn get_peer(network: &str, peer_id: &str) -> Option<WebSocket> {
    let map = PEER_MAP.lock().unwrap();
    map.get(network)?.get(peer_id).clone()
}

/// Get all peers in a network.
pub fn get_network_peers(network: &str) -> Vec<(String, WebSocket)> {
    let map = PEER_MAP.lock().unwrap();
    map.get(network)
        .map(|peers| {
            peers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Check if a network has any peers.
pub fn has_network(network: &str) -> bool {
    let map = PEER_MAP.lock().unwrap();
    map.contains_key(network)
}

/// Get the count of peers in a network.
pub fn peer_count(network: &str) -> usize {
    let map = PEER_MAP.lock().unwrap();
    map.get(network).map(|m| m.len()).unwrap_or(0)
}

/// Persist peer registration to KV for cross-worker-instance discovery.
pub async fn persist_peer(kv: &kv::KvStore, network: &str, peer_id: &str, ws: &WebSocket) {
    let key = format!("{}:{}", network, peer_id);
    // Store a heartbeat timestamp. The ws reference stays in memory.
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Err(e) = kv
        .put(&key, &timestamp.to_string())
        .execute()
        .await
    {
        console_error!("failed to persist peer: {}", e);
    }
}

/// Remove persisted peer from KV.
pub async fn unpersist_peer(kv: &kv::KvStore, network: &str, peer_id: &str) {
    let key = format!("{}:{}", network, peer_id);
    let _ = kv.delete(&key).await;
}

/// Get list of registered peers in a network from KV.
pub async fn list_peers(kv: &kv::KvStore, network: &str) -> Result<Vec<String>> {
    let prefix = format!("{}:", network);
    let entries = kv.list().execute().await?;
    let mut peers = Vec::new();
    for key in entries.keys {
        if key.name.starts_with(&prefix) {
            peers.push(key.name.strip_prefix(&prefix).unwrap_or("").to_string());
        }
    }
    Ok(peers)
}
