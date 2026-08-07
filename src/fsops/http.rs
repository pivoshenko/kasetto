use reqwest::blocking::Client;
use std::sync::OnceLock;
use std::time::Duration;

use crate::error::{err, Result};

static HTTP_CLIENT: OnceLock<std::result::Result<Client, String>> = OnceLock::new();

/// Shared client: avoids TLS/session setup on every asset or config fetch.
pub(crate) fn http_client() -> Result<Client> {
    let built = HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(format!("kasetto/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| e.to_string())
    });
    match built {
        Ok(c) => Ok(c.clone()),
        Err(e) => Err(err(format!("failed to build HTTP client: {e}"))),
    }
}
