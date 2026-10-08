use serde::{Deserialize, Serialize};

/// Information about a device's identity and service port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_name: String,
    pub device_type: String,
    pub os: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    pub port: u16,
}

impl DeviceInfo {
    pub fn new(
        device_name: impl Into<String>,
        device_type: impl Into<String>,
        os: impl Into<String>,
        version: impl Into<String>,
        room_id: Option<String>,
        port: u16,
    ) -> Self {
        Self {
            device_name: device_name.into(),
            device_type: device_type.into(),
            os: os.into(),
            version: version.into(),
            room_id,
            port,
        }
    }
}

/// Discovered peer on local network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub device_name: String,
    pub device_type: String,
    pub os: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    pub ip: String,
    pub port: u16,
}

impl PeerInfo {
    pub fn new(
        device_name: impl Into<String>,
        device_type: impl Into<String>,
        os: impl Into<String>,
        room_id: Option<String>,
        ip: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            device_name: device_name.into(),
            device_type: device_type.into(),
            os: os.into(),
            room_id,
            ip: ip.into(),
            port,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.ip, self.port)
    }
}
