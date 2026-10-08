use crate::discovery::PeerEvent;
use crate::models::peer::{DeviceInfo, PeerInfo};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub const MDNS_SERVICE_TYPE: &str = "_easyshare._tcp.local.";

pub struct MdnsDiscovery {
    device_info: DeviceInfo,
    daemon: Option<ServiceDaemon>,
    fullname: Option<String>,
    event_tx: mpsc::Sender<PeerEvent>,
    browse_task: Option<JoinHandle<()>>,
}

impl MdnsDiscovery {
    pub fn new(device_info: DeviceInfo, event_tx: mpsc::Sender<PeerEvent>) -> Self {
        Self {
            device_info,
            daemon: None,
            fullname: None,
            event_tx,
            browse_task: None,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        let daemon = ServiceDaemon::new().map_err(|e| format!("Failed to create mDNS daemon: {}", e))?;

        let instance_name = self.device_info.device_name.replace(' ', "-");
        let host_name = format!("{}.local.", instance_name);
        let port = self.device_info.port;

        let mut properties = HashMap::new();
        properties.insert("name".to_string(), self.device_info.device_name.clone());
        properties.insert("os".to_string(), self.device_info.os.clone());
        properties.insert("type".to_string(), self.device_info.device_type.clone());
        properties.insert("v".to_string(), self.device_info.version.clone());
        if let Some(room) = &self.device_info.room_id {
            properties.insert("room".to_string(), room.clone());
        }

        let best_ip = crate::discovery::nic::get_best_physical_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "127.0.0.1".to_string());

        let service_info = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &instance_name,
            &host_name,
            &best_ip,
            port,
            properties,
        )
        .map_err(|e| format!("Failed to build ServiceInfo: {}", e))?;

        let fullname = service_info.get_fullname().to_string();
        daemon
            .register(service_info)
            .map_err(|e| format!("Failed to register mDNS service: {}", e))?;

        let receiver = daemon
            .browse(MDNS_SERVICE_TYPE)
            .map_err(|e| format!("Failed to browse mDNS: {}", e))?;

        let event_tx_clone = self.event_tx.clone();
        let my_device_name = self.device_info.device_name.clone();
        let my_port = self.device_info.port;

        let browse_task = tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let properties = info.get_properties();
                        let dev_name = properties
                            .get_property_val_str("name")
                            .unwrap_or_else(|| info.get_fullname())
                            .to_string();
                        let os = properties
                            .get_property_val_str("os")
                            .unwrap_or("unknown")
                            .to_string();
                        let dev_type = properties
                            .get_property_val_str("type")
                            .unwrap_or("desktop")
                            .to_string();
                        let room_id = properties
                            .get_property_val_str("room")
                            .map(|s| s.to_string());
                        let port = info.get_port();

                        // Avoid discovering ourselves
                        if dev_name == my_device_name && port == my_port {
                            continue;
                        }

                        let addresses = info.get_addresses();
                        let ip_str = if let Some(ip) = addresses.iter().next() {
                            ip.to_string()
                        } else {
                            continue;
                        };

                        let peer = PeerInfo {
                            device_name: dev_name,
                            device_type: dev_type,
                            os,
                            room_id,
                            ip: ip_str,
                            port,
                        };

                        let _ = event_tx_clone.send(PeerEvent::Discovered(peer)).await;
                    }
                    ServiceEvent::ServiceRemoved(_service_type, _fullname) => {
                        // Removal will be picked up or handled by expiry
                    }
                    _ => {}
                }
            }
        });

        self.daemon = Some(daemon);
        self.fullname = Some(fullname);
        self.browse_task = Some(browse_task);

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(task) = self.browse_task.take() {
            task.abort();
        }
        if let (Some(daemon), Some(fullname)) = (self.daemon.take(), self.fullname.take()) {
            let _ = daemon.unregister(&fullname);
            let _ = daemon.shutdown();
        }
    }
}

impl Drop for MdnsDiscovery {
    fn drop(&mut self) {
        self.stop();
    }
}
