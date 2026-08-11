//! Shared HTTP GET for acquisition adapters (ADR-056).
//!
//! Adapters only — never called from da-application. Sync trait boundary uses
//! `block_in_place` when a Tokio runtime is already running (CLI path).

use da_ports::AcquisitionError;

const DEFAULT_UA: &str = "daily-archive/load-plane (research bot)";

/// GET URL → bytes. Fail-closed on non-success status.
pub fn http_get_bytes(url: &str, user_agent: Option<&str>) -> Result<Vec<u8>, AcquisitionError> {
    let url = url.to_string();
    let ua = user_agent.unwrap_or(DEFAULT_UA).to_string();
    let fetch = async move {
        let client = reqwest::Client::builder()
            .user_agent(ua)
            .timeout(std::time::Duration::from_secs(90))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| AcquisitionError::Infrastructure(format!("reqwest client: {e}")))?;
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| AcquisitionError::Infrastructure(format!("GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(AcquisitionError::Infrastructure(format!(
                "GET {url} status {}",
                resp.status()
            )));
        }
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| AcquisitionError::Infrastructure(format!("body {url}: {e}")))
    };
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fetch)),
        Err(_) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| AcquisitionError::Infrastructure(format!("tokio rt: {e}")))?;
            rt.block_on(fetch)
        }
    }
}

/// Try multiple URLs until one returns bytes that pass `validate`.
pub fn http_get_first_valid(
    urls: &[&str],
    user_agent: Option<&str>,
    validate: impl Fn(&[u8]) -> Result<(), String>,
) -> Result<(String, Vec<u8>), AcquisitionError> {
    let mut errors = Vec::new();
    for url in urls {
        match http_get_bytes(url, user_agent) {
            Ok(bytes) => match validate(&bytes) {
                Ok(()) => return Ok(((*url).to_string(), bytes)),
                Err(v) => errors.push(format!("{url}: invalid body ({v})")),
            },
            Err(e) => errors.push(format!("{url}: {e}")),
        }
    }
    Err(AcquisitionError::Infrastructure(format!(
        "all fetch candidates failed: {}",
        errors.join(" | ")
    )))
}
