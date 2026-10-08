pub mod mdns;
pub mod nic;
pub mod udp;

pub use mdns::*;
pub use nic::*;
pub use udp::*;

use crate::models::peer::{DeviceInfo, PeerInfo};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerEvent {
    Discovered(PeerInfo),
    Lost(PeerInfo),
    Updated(PeerInfo),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryEvent {
    Peer(PeerEvent),
}

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub broadcast_interval: Duration,
    pub peer_ttl: Duration,
    pub udp_port: u16,
    pub enable_mdns: bool,
    pub enable_udp: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            broadcast_interval: Duration::from_secs(5),
            peer_ttl: Duration::from_secs(15),
            udp_port: DEFAULT_UDP_PORT,
            enable_mdns: true,
            enable_udp: true,
        }
    }
}

pub struct DiscoveryHandle {
    tracker: Arc<RwLock<PeerTracker>>,
    event_tx: broadcast::Sender<DiscoveryEvent>,
    tasks: Arc<RwLock<Vec<JoinHandle<()>>>>,
    udp_handle: Arc<RwLock<Option<UdpDiscovery>>>,
    mdns_handle: Arc<RwLock<Option<MdnsDiscovery>>>,
}

impl DiscoveryHandle {
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        let tracker = self.tracker.read().await;
        tracker.get_peers()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.event_tx.subscribe()
    }

    pub async fn add_manual_peer(&self, ip: &str, port: u16) -> Result<PeerInfo, String> {
        let peer = PeerInfo {
            device_name: format!("Manual-{}", ip),
            device_type: "unknown".to_string(),
            os: "unknown".to_string(),
            room_id: None,
            ip: ip.to_string(),
            port,
            pairing_pin: None,
        };

        let mut tracker = self.tracker.write().await;
        if let Some(event) = tracker.record_seen(peer.clone()) {
            let _ = self.event_tx.send(DiscoveryEvent::Peer(event));
        }

        Ok(peer)
    }

    pub async fn stop(&self) {
        let mut tasks = self.tasks.write().await;
        for task in tasks.drain(..) {
            task.abort();
        }

        if let Some(mut udp) = self.udp_handle.write().await.take() {
            udp.stop();
        }

        if let Some(mut mdns) = self.mdns_handle.write().await.take() {
            mdns.stop();
        }
    }
}

pub struct DiscoveryService {
    device_info: DeviceInfo,
    config: DiscoveryConfig,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

impl DiscoveryService {
    pub async fn new(
        device_info: DeviceInfo,
        config: DiscoveryConfig,
    ) -> std::io::Result<(Self, broadcast::Receiver<DiscoveryEvent>)> {
        let (event_tx, rx) = broadcast::channel(128);
        Ok((
            Self {
                device_info,
                config,
                event_tx,
            },
            rx,
        ))
    }

    pub async fn start(self) -> std::io::Result<DiscoveryHandle> {
        let (internal_tx, mut internal_rx) = mpsc::channel::<PeerEvent>(128);
        let tracker = Arc::new(RwLock::new(PeerTracker::new(self.config.peer_ttl)));
        let mut tasks = Vec::new();

        let udp_discovery = if self.config.enable_udp {
            let mut udp = UdpDiscovery::new(
                self.device_info.clone(),
                self.config.udp_port,
                self.config.broadcast_interval,
                self.config.peer_ttl,
                internal_tx.clone(),
            );
            udp.start().await?;
            Some(udp)
        } else {
            None
        };

        let mdns_discovery = if self.config.enable_mdns {
            let mut mdns = MdnsDiscovery::new(self.device_info.clone(), internal_tx.clone());
            if let Err(e) = mdns.start() {
                eprintln!("mDNS discovery failed to start: {}", e);
                None
            } else {
                Some(mdns)
            }
        } else {
            None
        };

        // Aggregator loop: routes internal PeerEvents to external broadcast and deduplicates
        let tracker_agg = tracker.clone();
        let broadcast_tx = self.event_tx.clone();
        let agg_handle = tokio::spawn(async move {
            while let Some(event) = internal_rx.recv().await {
                match event {
                    PeerEvent::Discovered(peer) => {
                        let mut tr = tracker_agg.write().await;
                        if let Some(dedup_event) = tr.record_seen(peer) {
                            let _ = broadcast_tx.send(DiscoveryEvent::Peer(dedup_event));
                        }
                    }
                    PeerEvent::Lost(peer) => {
                        let mut tr = tracker_agg.write().await;
                        let _ = tr.remove_peer(&peer.ip, peer.port);
                        let _ = broadcast_tx.send(DiscoveryEvent::Peer(PeerEvent::Lost(peer)));
                    }
                    PeerEvent::Updated(peer) => {
                        let mut tr = tracker_agg.write().await;
                        if let Some(dedup_event) = tr.record_seen(peer.clone()) {
                            let _ = broadcast_tx.send(DiscoveryEvent::Peer(dedup_event));
                        } else {
                            let _ = broadcast_tx.send(DiscoveryEvent::Peer(PeerEvent::Updated(peer)));
                        }
                    }
                }
            }
        });
        tasks.push(agg_handle);

        let handle = DiscoveryHandle {
            tracker,
            event_tx: self.event_tx,
            tasks: Arc::new(RwLock::new(tasks)),
            udp_handle: Arc::new(RwLock::new(udp_discovery)),
            mdns_handle: Arc::new(RwLock::new(mdns_discovery)),
        };

        Ok(handle)
    }
}
