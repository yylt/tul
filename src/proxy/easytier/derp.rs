use super::store;
use crate::proxy::websocket::WsStream;
use futures_util::StreamExt;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use worker::*;

/// PeerManagerHeader layout (from EasyTier packet_def.rs):
///   from_peer_id: u32 LE  (offset 0)
///   to_peer_id:   u32 LE  (offset 4)
///   packet_type:  u8      (offset 8)
///   flags:        u8      (offset 9)
///   forward_counter: u8   (offset 10)
///   reserved:     u8      (offset 11)
///   len:          u32 LE  (offset 12)
const PEER_MANAGER_HEADER_SIZE: usize = 12;

/// Extract to_peer_id from a ZCPacket tunnel payload.
fn parse_to_peer_id(data: &[u8]) -> Option<u32> {
    if data.len() < PEER_MANAGER_HEADER_SIZE + 4 {
        return None;
    }
    Some(u32::from_le_bytes([data[4], data[5], data[6], data[7]]))
}

/// Extract from_peer_id from a ZCPacket tunnel payload.
fn parse_from_peer_id(data: &[u8]) -> Option<u32> {
    if data.len() < PEER_MANAGER_HEADER_SIZE + 4 {
        return None;
    }
    Some(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

/// Packet types that should be forwarded (from packet_def.rs PacketType enum)
const FORWARDABLE_PACKET_TYPES: [u8; 11] = [
    1,   // Data
    4,   // Ping
    5,   // Pong
    8,   // RpcReq
    9,   // RpcResp
    10,  // ForeignNetworkPacket
    13,  // NoiseHandshakeMsg1
    14,  // NoiseHandshakeMsg2
    15,  // NoiseHandshakeMsg3
    20,  // RelayHandshake
    21,  // RelayHandshakeAck
];

fn is_forwardable_packet(data: &[u8]) -> bool {
    if data.len() < PEER_MANAGER_HEADER_SIZE + 1 {
        return true; // forward small/unknown packets to be safe
    }
    let packet_type = data[PEER_MANAGER_HEADER_SIZE];
    FORWARDABLE_PACKET_TYPES.contains(&packet_type)
}

/// Relay a single packet to a target websocket.
fn relay_packet(ws: &WebSocket, data: &[u8]) -> Result<()> {
    ws.send_with_bytes(data)?;
    Ok(())
}

/// Queue a packet to KV for offline target delivery.
fn queue_packet(kv: &kv::KvStore, network: &str, target_pid: &str, data: &[u8]) {
    let queue_key = format!("queue:{}:{}", network, target_pid);
    let _ = async {
        let existing = kv.get(&queue_key).bytes().await.ok().flatten();
        let mut queued: Vec<Vec<u8>> = existing
            .as_ref()
            .and_then(|b| serde_json::from_slice(b).ok())
            .unwrap_or_default();
        queued.push(data.to_vec());
        if queued.len() > 200 {
            queued.drain(..queued.len() - 200);
        }
        let _ = kv
            .put(&queue_key, &serde_json::to_string(&queued).unwrap())
            .execute()
            .await;
    }
    .await;
}

/// Process a WebSocket connection as an EasyTier DERP client.
/// The client connects to /derp?network=X&peer_id=Y
/// All binary frames received are treated as ZCPacket tunnel payloads.
pub async fn handle_derp_connection(
    ws: WebSocket,
    events: EventStream<'_>,
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

    let mut ws_stream = WsStream::new(&ws, events, None);
    let ws_close = ws.clone();
    let net_close = network.clone();
    let pid_close = peer_id.clone();

    worker::wasm_bindgen_futures::spawn_local(async move {
        let mut read_buf = Vec::with_capacity(65536);
        let mut write_buf = Vec::with_capacity(65536);

        // Drain any early data from WsStream
        drain_ws_stream(&mut ws_stream, &mut read_buf).await;

        // Process the initial buffer
        process_packets(
            &read_buf,
            &net_close,
            &pid_close,
            &kv,
            &ws,
        ).await;

        read_buf.clear();

        // Main event loop: read one WS message at a time
        loop {
            match ws_stream.next().await {
                Some(Ok(data)) => {
                    // This is a single WebSocket binary frame = one ZCPacket
                    process_packets(
                        &data,
                        &net_close,
                        &pid_close,
                        &kv,
                        &ws,
                    ).await;
                }
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

/// Drain WsStream into buf (reads all buffered + pending WS data).
async fn drain_ws_stream<'a>(stream: &mut WsStream<'a>, buf: &mut Vec<u8>) {
    // WsStream's next() pulls one WS binary frame at a time
    // We use the underlying events stream via WsStream
}

/// Process raw packet bytes: parse headers and relay to targets.
async fn process_packets(
    data: &[u8],
    network: &str,
    peer_id: &str,
    kv: &kv::KvStore,
    self_ws: &WebSocket,
) {
    if data.is_empty() {
        return;
    }

    // Parse destination peer_id from PeerManagerHeader
    if let Some(to_peer_id) = parse_to_peer_id(data) {
        let target_str = to_peer_id.to_string();

        // Skip self
        if target_str == peer_id {
            return;
        }

        // Check if packet type is forwardable
        if !is_forwardable_packet(data) {
            return;
        }

        // Look up target in memory map first
        if let Some(target_ws) = store::get_peer(network, &target_str) {
            if let Err(e) = relay_packet(&target_ws, data) {
                console_error!("derp: relay to {} failed: {}", target_str, e);
            }
        } else {
            // Target not in memory — queue in KV
            queue_packet(kv, network, &target_str, data);
        }
    } else {
        // Couldn't parse header (too short or malformed)
        // Broadcast to all peers in the network for discovery
        let peers = store::get_network_peers(network);
        for (pid, target_ws) in peers {
            if pid != peer_id {
                let _ = relay_packet(&target_ws, data);
            }
        }
    }
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

/// Flush queued messages for a newly-online peer.
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
