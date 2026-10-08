use serde::{Deserialize, Serialize};

/// Information about a device's identity and service port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_name: String,
    pub device_type: String,
    pub os: String,
    pub version: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing_pin: Option<String>,
}

impl DeviceInfo {
    pub fn new(
        device_name: impl Into<String>,
        device_type: impl Into<String>,
        os: impl Into<String>,
        version: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            device_name: device_name.into(),
            device_type: device_type.into(),
            os: os.into(),
            version: version.into(),
            port,
            pairing_pin: None,
        }
    }

    pub fn with_pairing_pin(mut self, pin: impl Into<String>) -> Self {
        self.pairing_pin = Some(pin.into());
        self
    }
}

/// Discovered peer on local network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub device_name: String,
    pub device_type: String,
    pub os: String,
    pub ip: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing_pin: Option<String>,
}

impl PeerInfo {
    pub fn new(
        device_name: impl Into<String>,
        device_type: impl Into<String>,
        os: impl Into<String>,
        ip: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            device_name: device_name.into(),
            device_type: device_type.into(),
            os: os.into(),
            ip: ip.into(),
            port,
            pairing_pin: None,
        }
    }

    pub fn with_pairing_pin(mut self, pin: impl Into<String>) -> Self {
        self.pairing_pin = Some(pin.into());
        self
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.ip, self.port)
    }
}
