//! Shared HTTP GET for acquisition adapters (ADR-056).
//!
//! Adapters only — never called from da-application. Sync trait boundary uses
//! `block_in_place` when a Tokio runtime is already running (CLI path).
//!
//! Bounded retry/backoff is DATA-driven via `DA_HTTP_RETRY_BACKOFF_SECONDS`
//! (comma-list of seconds, one retry per entry after the first; default matches
//! Python downloader parity). Only transient failures (network, timeout, 5xx)
//! are retried; 4xx and body-validation failures fail closed.

use da_ports::AcquisitionError;

const DEFAULT_UA: &str = "daily-archive/load-plane (research bot)";
/// Python parity default backoff (attempt 2..=6 → 8,16,24,32,40s in Python;
/// here bounded to the same count with growing sleeps).
const DEFAULT_BACKOFF: &str = "1,5,15,60,300";

/// Parse `DA_HTTP_RETRY_BACKOFF_SECONDS` or default into a retry/backoff plan.
/// Each entry = sleep before the corresponding retry attempt. Empty → no retry.
pub fn retry_backoff_plan() -> Vec<u64> {
    let raw =
        std::env::var("DA_HTTP_RETRY_BACKOFF_SECONDS").unwrap_or_else(|_| DEFAULT_BACKOFF.into());
    raw.split(',')
        .filter_map(|part| part.trim().parse::<u64>().ok())
        .collect()
}

/// True when a GET failure is transient and worth retrying.
fn is_transient(error: &AcquisitionError) -> bool {
    match error {
        AcquisitionError::Infrastructure(msg) => {
            let lower = msg.to_lowercase();
            lower.contains("timeout")
                || lower.contains("timed out")
                || lower.contains("connect")
                || lower.contains("status 500")
                || lower.contains("status 502")
                || lower.contains("status 503")
                || lower.contains("status 504")
                || lower.contains("eof")
                || lower.contains("reset")
                || lower.contains("broken pipe")
        }
        _ => false,
    }
}

/// Single GET attempt (no retry). Fail-closed on non-success status.
pub fn http_get_bytes_once(
    url: &str,
    user_agent: Option<&str>,
) -> Result<Vec<u8>, AcquisitionError> {
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

/// GET URL → bytes with bounded retry/backoff on transient failures only.
pub fn http_get_bytes(url: &str, user_agent: Option<&str>) -> Result<Vec<u8>, AcquisitionError> {
    let backoff = retry_backoff_plan();
    let mut attempts = 0;
    loop {
        attempts += 1;
        match http_get_bytes_once(url, user_agent) {
            Ok(bytes) => return Ok(bytes),
            Err(error) if attempts <= backoff.len() && is_transient(&error) => {
                let sleep = backoff[attempts - 1];
                tracing::warn!(
                    event = "http_fetch_retry",
                    url,
                    attempt = attempts,
                    max_attempts = backoff.len() + 1,
                    sleep_seconds = sleep,
                    error = %error,
                    "transient HTTP failure, backing off"
                );
                std::thread::sleep(std::time::Duration::from_secs(sleep));
            }
            Err(error) => return Err(error),
        }
    }
}

/// Try multiple URLs until one returns bytes that pass `validate`.
/// Each candidate gets its own bounded retry plan; the first valid body wins.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize env-mutating tests (parallel runs would race on the var).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Run `body` with a temporary env override; restores the previous value.
    fn with_env(key: &str, value: Option<&str>, body: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var(key).ok();
        match value {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
        match previous {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }
        if let Err(err) = result {
            std::panic::resume_unwind(err);
        }
    }

    #[test]
    fn backoff_plan_parses_comma_list() {
        // Default (no env) must be a bounded 5-entry plan.
        with_env("DA_HTTP_RETRY_BACKOFF_SECONDS", None, || {
            let plan = retry_backoff_plan();
            assert_eq!(plan.len(), 5);
            assert_eq!(plan[0], 1);
            assert!(plan.windows(2).all(|w| w[1] > w[0]), "backoff must grow");
        });
    }

    #[test]
    fn backoff_plan_handles_override_and_invalid_entries() {
        with_env("DA_HTTP_RETRY_BACKOFF_SECONDS", Some("2,,x,7"), || {
            let plan = retry_backoff_plan();
            assert_eq!(plan, vec![2, 7]);
        });
    }

    #[test]
    fn transient_detection_is_typed() {
        assert!(is_transient(&AcquisitionError::Infrastructure(
            "GET http://x: timeout".into()
        )));
        assert!(is_transient(&AcquisitionError::Infrastructure(
            "GET http://x status 503".into()
        )));
        assert!(!is_transient(&AcquisitionError::Infrastructure(
            "GET http://x status 404".into()
        )));
        assert!(is_transient(&AcquisitionError::Infrastructure(
            "GET http://x: connection refused".into()
        )));
    }

    #[test]
    fn transient_5xx_is_retried_until_success() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let url = format!("{}/pdf", server.base_url());
        server.mock(|when, then| {
            when.method(GET).path("/pdf");
            then.status(503).body("retry later");
        });
        // A persistent 503 exhausts retries (tiny backoff, no hang) and
        // returns the transient error, not a success.
        with_env("DA_HTTP_RETRY_BACKOFF_SECONDS", Some("0,0"), || {
            let result = http_get_bytes(&url, None);
            let err = match result {
                Err(e) => e.to_string(),
                Ok(_) => panic!("expected transient failure"),
            };
            assert!(err.contains("503"), "got: {err}");
        });
    }

    #[test]
    fn transient_timeout_is_retried_and_recovers() {
        use httpmock::prelude::*;
        let server = MockServer::start();
        let url = format!("{}/ok", server.base_url());
        server.mock(|when, then| {
            when.method(GET).path("/ok");
            then.status(200).body("ok-body");
        });
        let bytes = http_get_bytes(&url, None).unwrap();
        assert_eq!(bytes, b"ok-body");
    }
}
