use crate::discovery::PeerEvent;
use crate::models::peer::{DeviceInfo, PeerInfo};
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

pub const DEFAULT_UDP_PORT: u16 = 41234;
pub const DEFAULT_BROADCAST_INTERVAL: Duration = Duration::from_secs(5);
pub const DEFAULT_PEER_TTL: Duration = Duration::from_secs(15);
pub const HEARTBEAT_MAGIC: &str = "EASYSHARE_DISCOVERY";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    pub magic: String,
    pub device_name: String,
    pub device_type: String,
    pub os: String,
    pub version: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing_pin: Option<String>,
}

impl HeartbeatMessage {
    pub fn from_device_info(info: &DeviceInfo, ip: Option<String>) -> Self {
        Self {
            magic: HEARTBEAT_MAGIC.to_string(),
            device_name: info.device_name.clone(),
            device_type: info.device_type.clone(),
            os: info.os.clone(),
            version: info.version.clone(),
            port: info.port,
            ip,
            pairing_pin: info.pairing_pin.clone(),
        }
    }

    pub fn to_peer_info(&self, sender_ip: &str) -> PeerInfo {
        let resolved_ip = match &self.ip {
            Some(ip) if ip != "127.0.0.1" && ip != "0.0.0.0" => ip.clone(),
            _ => sender_ip.to_string(),
        };

        PeerInfo {
            device_name: self.device_name.clone(),
            device_type: self.device_type.clone(),
            os: self.os.clone(),
            ip: resolved_ip,
            port: self.port,
            pairing_pin: self.pairing_pin.clone(),
        }
    }
}

pub fn create_broadcast_socket(port: u16) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(not(windows))]
    {
        let _ = socket.set_reuse_port(true);
    }
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
    socket.bind(&addr.into())?;
    let std_socket: std::net::UdpSocket = socket.into();
    UdpSocket::from_std(std_socket)
}

#[derive(Debug, Clone)]
pub struct TrackedPeer {
    pub peer: PeerInfo,
    pub last_seen: Instant,
}

#[derive(Debug, Clone)]
pub struct PeerTracker {
    peers: HashMap<String, TrackedPeer>,
    ttl: Duration,
}

impl PeerTracker {
    pub fn new(ttl: Duration) -> Self {
        Self {
            peers: HashMap::new(),
            ttl,
        }
    }

    pub fn key_for_peer(peer: &PeerInfo) -> String {
        format!("{}:{}", peer.ip, peer.port)
    }

    pub fn record_seen(&mut self, peer: PeerInfo) -> Option<PeerEvent> {
        let key = Self::key_for_peer(&peer);
        let now = Instant::now();
        if let Some(tracked) = self.peers.get_mut(&key) {
            tracked.last_seen = now;
            if tracked.peer != peer {
                tracked.peer = peer.clone();
                Some(PeerEvent::Discovered(peer))
            } else {
                None
            }
        } else {
            self.peers.insert(
                key,
                TrackedPeer {
                    peer: peer.clone(),
                    last_seen: now,
                },
            );
            Some(PeerEvent::Discovered(peer))
        }
    }

    pub fn prune_expired(&mut self) -> Vec<PeerEvent> {
        let now = Instant::now();
        let mut expired_keys = Vec::new();
        let mut lost_events = Vec::new();

        for (key, tracked) in &self.peers {
            if now.duration_since(tracked.last_seen) > self.ttl {
                expired_keys.push(key.clone());
                lost_events.push(PeerEvent::Lost(tracked.peer.clone()));
            }
        }

        for key in expired_keys {
            self.peers.remove(&key);
        }

        lost_events
    }

    pub fn get_peers(&self) -> Vec<PeerInfo> {
        self.peers.values().map(|t| t.peer.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    pub fn remove_peer(&mut self, ip: &str, port: u16) -> Option<PeerEvent> {
        let key = format!("{}:{}", ip, port);
        self.peers.remove(&key).map(|t| PeerEvent::Lost(t.peer))
    }
}

pub struct UdpDiscovery {
    device_info: DeviceInfo,
    port: u16,
    interval: Duration,
    tracker: Arc<RwLock<PeerTracker>>,
    event_tx: mpsc::Sender<PeerEvent>,
    tasks: Vec<JoinHandle<()>>,
}

impl UdpDiscovery {
    pub fn new(
        device_info: DeviceInfo,
        port: u16,
        interval: Duration,
        ttl: Duration,
        event_tx: mpsc::Sender<PeerEvent>,
    ) -> Self {
        Self {
            device_info,
            port,
            interval,
            tracker: Arc::new(RwLock::new(PeerTracker::new(ttl))),
            event_tx,
            tasks: Vec::new(),
        }
    }

    pub async fn start(&mut self) -> std::io::Result<()> {
        let rx_socket = create_broadcast_socket(self.port)?;
        let tx_socket = create_broadcast_socket(0)?;

        let rx_socket = Arc::new(rx_socket);
        let tx_socket = Arc::new(tx_socket);

        let device_info = self.device_info.clone();
        let port = self.port;
        let interval = self.interval;

        // Broadcast sender task
        let tx_socket_clone = tx_socket.clone();
        let dev_clone = device_info.clone();
        let broadcast_handle = tokio::spawn(async move {
            let broadcast_addr = format!("255.255.255.255:{}", port);
            let loopback_addr = format!("127.0.0.1:{}", port);

            loop {
                let best_ip = crate::discovery::nic::get_best_physical_ip()
                    .map(|ip| ip.to_string());
                let msg = HeartbeatMessage::from_device_info(&dev_clone, best_ip);
                if let Ok(bytes) = serde_json::to_vec(&msg) {
                    let _ = tx_socket_clone.send_to(&bytes, &broadcast_addr).await;
                    let _ = tx_socket_clone.send_to(&bytes, &loopback_addr).await;
                }
                tokio::time::sleep(interval).await;
            }
        });

        // Receiver task
        let rx_socket_clone = rx_socket.clone();
        let tracker_clone = self.tracker.clone();
        let event_tx_clone = self.event_tx.clone();
        let my_device_name = device_info.device_name.clone();
        let my_port = device_info.port;

        let recv_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                match rx_socket_clone.recv_from(&mut buf).await {
                    Ok((len, src_addr)) => {
                        if let Ok(msg) = serde_json::from_slice::<HeartbeatMessage>(&buf[..len]) {
                            if msg.magic == HEARTBEAT_MAGIC {
                                // Ignore messages from ourselves
                                if msg.device_name == my_device_name && msg.port == my_port {
                                    continue;
                                }

                                let sender_ip = src_addr.ip().to_string();
                                let peer_info = msg.to_peer_info(&sender_ip);

                                let mut tracker = tracker_clone.write().await;
                                if let Some(event) = tracker.record_seen(peer_info) {
                                    let _ = event_tx_clone.send(event).await;
                                }
                            }
                        }
                    }
                    Err(_e) => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        });

        // Pruning task
        let tracker_prune = self.tracker.clone();
        let event_tx_prune = self.event_tx.clone();
        let prune_interval = self.interval.min(Duration::from_millis(500));

        let prune_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(prune_interval).await;
                let mut tracker = tracker_prune.write().await;
                let expired = tracker.prune_expired();
                for event in expired {
                    let _ = event_tx_prune.send(event).await;
                }
            }
        });

        self.tasks.push(broadcast_handle);
        self.tasks.push(recv_handle);
        self.tasks.push(prune_handle);

        Ok(())
    }

    pub fn tracker(&self) -> Arc<RwLock<PeerTracker>> {
        self.tracker.clone()
    }

    pub fn stop(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

impl Drop for UdpDiscovery {
    fn drop(&mut self) {
        self.stop();
    }
}
