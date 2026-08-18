use super::store;
use futures_util::StreamExt;
use worker::*;

/// Process a WebSocket connection as an EasyTier DERP client.
/// The client connects to /derp?network=X&peer_id=Y
/// tul only registers and discovers peers; it does NOT forward data packets.
pub async fn handle_derp_connection(
    ws: WebSocket,
    network: String,
    peer_id: String,
    env: Env,
) {
    let kv = env.kv("EASYTIER_KV").unwrap_or_else(|_| {
        console_warn!("EASYTIER_KV namespace not bound, running without persistence");
        kv::KvStore::basic().unwrap()
    });

    // Register in-memory map
    store::register_peer(&network, &peer_id, ws.clone());
    persist_peer_online(&kv, &network, &peer_id).await;

    console_debug!(
        "derp: registered peer {} on network {} (total: {})",
        peer_id,
        network,
        store::peer_count(&network)
    );

    let ws_close = ws.clone();
    let net_close = network.clone();
    let pid_close = peer_id.clone();

    worker::wasm_bindgen_futures::spawn_local(async move {
        let read_ws = ws.clone();
        let mut events = read_ws.events().expect("Failed to get event stream");

        // Keep the connection alive; consume frames without forwarding.
        loop {
            match events.next().await {
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    console_error!("derp: ws read error: {}", e);
                    break;
                }
                None => {
                    // Stream ended
                    break;
                }
            }
        }

        // Cleanup on disconnect
        store::unregister_peer(&net_close, &pid_close);
        unpersist_peer_online(&kv, &net_close, &pid_close).await;

        // Flush queued messages to this peer
        flush_queue(&kv, &net_close, &pid_close, &ws).await;

        let _ = ws_close.close();
        console_debug!(
            "derp: peer {} disconnected from network {} (remaining: {})",
            pid_close,
            net_close,
            store::peer_count(&net_close)
        );
    });
}

/// Persist online status to KV with timestamp.
async fn persist_peer_online(kv: &kv::KvStore, network: &str, peer_id: &str) {
    let key = format!("{}:{}", network, peer_id);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = kv
        .put(&key, &ts.to_string())
        .expiration_ttl(300) // 5 minute TTL
        .execute()
        .await;
}

async fn unpersist_peer_online(kv: &kv::KvStore, network: &str, peer_id: &str) {
    let key = format!("{}:{}", network, peer_id);
    let _ = kv.delete(&key).await;
}

/// Flush queued discovery messages for a newly-online peer.
async fn flush_queue(kv: &kv::KvStore, network: &str, peer_id: &str, ws: &WebSocket) {
    let queue_key = format!("queue:{}:{}", network, peer_id);
    if let Ok(Some(queued_text)) = kv.get(&queue_key).text().await {
        if let Ok(messages) = serde_json::from_str::<Vec<Vec<u8>>>(&queued_text) {
            for msg in messages {
                let _ = ws.send_with_bytes(&msg);
            }
            console_debug!(
                "derp: flushed {} queued messages for {}",
                messages.len(),
                peer_id
            );
        }
        let _ = kv.delete(&queue_key).await;
    }
}

/// Handle heartbeat/health check from a peer.
pub async fn handle_heartbeat(network: &str, peer_id: &str, env: Env) -> Result<Response> {
    let kv = env.kv("EASYTIER_KV").unwrap_or_else(|_| kv::KvStore::basic().unwrap());
    persist_peer_online(&kv, network, peer_id).await;
    Response::builder()
        .with_status(200)
        .with_body("ok")
        .with_header("content-type", "text/plain")
        .map_err(Into::into)
}

/// List online peers in a network (for debugging/node discovery).
pub async fn list_online_peers(network: &str, env: Env) -> Result<Response> {
    let kv = env.kv("EASYTIER_KV").unwrap_or_else(|_| kv::KvStore::basic().unwrap());
    let peers = store::list_peers(&kv, network).await.unwrap_or_default();
    let json = serde_json::json!({
        "network": network,
        "peers": peers,
        "count": peers.len()
    });
    Response::from_json(&json)
}
