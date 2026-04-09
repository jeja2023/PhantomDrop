use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Serialize, Clone, Debug)]
pub struct TunnelStatus {
    pub active: bool,
    pub url: Option<String>,
    pub port: u16,
    pub subdomain: Option<String>,
    pub provider: String,
}

pub struct TunnelManager {
    status: Arc<Mutex<TunnelStatus>>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(TunnelStatus {
                active: false,
                url: None,
                port: 4000,
                subdomain: None,
                provider: "manual".to_string(),
            })),
        }
    }

    pub fn restore(&self, port: u16, url: Option<String>) {
        let mut status = self.status.lock().expect("Failed to lock status");
        if let Some(url) = url.and_then(|value| Self::normalize_url(&value)) {
            status.active = true;
            status.url = Some(url);
            status.port = port;
            status.provider = "manual".to_string();
        }
    }

    pub fn get_status(&self) -> TunnelStatus {
        self.status.lock().expect("Failed to lock status").clone()
    }

    pub async fn start(
        &self,
        port: u16,
        subdomain: Option<String>,
        public_url: Option<String>,
    ) -> Result<String, String> {
        let public_url = public_url
            .as_deref()
            .and_then(Self::normalize_url)
            .ok_or_else(|| "内置 Node localtunnel 已移除，请提供一个可访问当前中枢的公网地址".to_string())?;

        let mut status = self.status.lock().expect("Failed to lock status");
        status.active = true;
        status.url = Some(public_url.clone());
        status.port = port;
        status.subdomain = subdomain.filter(|value| !value.trim().is_empty());
        status.provider = "manual".to_string();

        Ok(public_url)
    }

    pub async fn stop(&self) {
        let mut status = self.status.lock().expect("Failed to lock status");
        status.active = false;
        status.url = None;
        status.subdomain = None;
    }

    fn normalize_url(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            Some(trimmed.to_string())
        } else {
            None
        }
    }
}
