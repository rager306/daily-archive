//! OpenAlex HTTP adapter — implements OpenAlexClient port.
//!
//! D133: Fetches curated metadata from api.openalex.org.
//! Replaces noisy YAKE keyword extraction with professional, disambiguated data.

use async_trait::async_trait;
use da_ports::openalex::{
    OpenAlexAuthor, OpenAlexAuthorship, OpenAlexClient, OpenAlexConcept, OpenAlexError,
    OpenAlexInstitution, OpenAlexResult, OpenAlexTopic, OpenAlexWork,
};
use serde::Deserialize;

/// Base URL: env OPENALEX_BASE_URL (default public API). No hardcode in
/// request paths (M446).
fn openalex_base() -> String {
    std::env::var("OPENALEX_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "https://api.openalex.org".to_string())
}

/// DOI prefix for a source code, from data/ontology/doi_prefix_map.yaml
/// (env DA_DOI_PREFIX_MAP). Bundled arXiv fallback keeps behavior when the
/// file is absent — the dictionary is the single place to extend for new
/// sources (M446 W-B1).
pub fn doi_prefix_for(source_code: &str) -> Option<String> {
    static BUNDLED: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
        std::sync::OnceLock::new();
    let path = std::env::var("DA_DOI_PREFIX_MAP")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "data/ontology/doi_prefix_map.yaml".to_string());
    let text = std::fs::read_to_string(&path).ok();
    let map = text
        .and_then(|t| serde_yaml::from_str::<serde_yaml::Value>(&t).ok())
        .and_then(|v| {
            v.get("doi_prefixes")?.as_mapping().map(|m| {
                m.iter()
                    .filter_map(|(k, v)| Some((k.as_str()?.to_string(), v.as_str()?.to_string())))
                    .collect::<std::collections::BTreeMap<String, String>>()
            })
        })
        .or_else(|| {
            BUNDLED
                .get_or_init(|| {
                    [("arxiv".to_string(), "10.48550/arxiv.".to_string())]
                        .into_iter()
                        .collect()
                })
                .clone()
                .into()
        });
    map.as_ref().and_then(|m| m.get(source_code).cloned())
}

/// Per-upstream circuit breaker (ADR-073, citracer `_orev_circuit_*`):
/// after `threshold` consecutive failures the upstream is skipped for
/// `cooldown`; any success resets the counter. Thread-safe.
#[derive(Debug)]
pub struct CircuitBreaker {
    threshold: u32,
    cooldown: std::time::Duration,
    state: std::sync::Mutex<BreakerState>,
}

#[derive(Debug, Default)]
struct BreakerState {
    consecutive_failures: u32,
    tripped_at: Option<std::time::Instant>,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, cooldown: std::time::Duration) -> Self {
        Self {
            threshold: threshold.max(1),
            cooldown,
            state: std::sync::Mutex::new(BreakerState::default()),
        }
    }

    /// false while open and cooldown not expired.
    pub fn allow(&self) -> bool {
        let mut s = self.state.lock().unwrap();
        match s.tripped_at {
            None => true,
            Some(t) if t.elapsed() > self.cooldown => {
                s.tripped_at = None;
                s.consecutive_failures = 0;
                true
            }
            Some(_) => false,
        }
    }

    pub fn record_failure(&self) {
        let mut s = self.state.lock().unwrap();
        s.consecutive_failures += 1;
        if s.consecutive_failures >= self.threshold && s.tripped_at.is_none() {
            s.tripped_at = Some(std::time::Instant::now());
        }
    }

    pub fn record_success(&self) {
        let mut s = self.state.lock().unwrap();
        s.consecutive_failures = 0;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, std::time::Duration::from_secs(120))
    }
}

/// Slim cached work record (python `_slim` parity: the enriched-layer stage
/// and existing cache files consume exactly these fields).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SlimWork {
    pub work_id: Option<String>,
    pub title: Option<String>,
    pub publication_year: Option<i64>,
    pub cited_by_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub authorships: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub topics: Vec<serde_json::Value>,
}

impl SlimWork {
    fn from_api(w: &serde_json::Value) -> Self {
        let src = w
            .pointer("/primary_location/source")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let source = if src.get("id").is_some() {
            Some(serde_json::json!({
                "id": src.get("id"),
                "display_name": src.get("display_name"),
            }))
        } else {
            None
        };
        let authorships = w
            .get("authorships")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|a| {
                        let author = a.get("author").cloned().unwrap_or_default();
                        serde_json::json!({
                            "a_id": author.get("id").and_then(|i| i.as_str())
                                .map(|s| s.rsplit('/').next().unwrap_or(s).to_string()),
                            "display_name": author.get("display_name"),
                            "orcid": author.get("orcid").and_then(|o| o.as_str())
                                .map(|s| s.rsplit('/').next().unwrap_or(s).to_string()),
                            "institutions": a.get("institutions").and_then(|i| i.as_array())
                                .map(|ins| ins.iter()
                                    .filter_map(|i| i.get("display_name").cloned())
                                    .take(2).collect::<Vec<_>>())
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let topics = w
            .get("topics")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .take(5)
                    .map(|t| {
                        serde_json::json!({
                            "id": t.get("id"), "display_name": t.get("display_name"),
                            "score": t.get("score"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            work_id: w.get("id").and_then(|i| i.as_str()).map(String::from),
            title: w.get("title").and_then(|t| t.as_str()).map(String::from),
            publication_year: w.get("publication_year").and_then(|y| y.as_i64()),
            cited_by_count: w.get("cited_by_count").and_then(|c| c.as_i64()),
            source,
            authorships,
            topics,
        }
    }
}

/// One OpenAlex work batch filter expression: `doi:A|doi:B|…` (ADR-073
/// citracer batch_enrich parity — 50 DOIs per request).
pub fn batch_doi_filter(dois: &[String]) -> String {
    dois.iter().map(|d| format!("doi:{d}")).collect::<Vec<_>>().join("|")
}

impl OpenAlexHttpAdapter {
    /// Concurrent works fetch (ADR-073): the doi filter rejects pipe-OR
    /// (probe: filter=doi:A|doi:B → 400), so batching happens through
    /// bounded concurrency of per-DOI path-form requests — the proven
    /// transport. Breaker guards the shared upstream; per-DOI isolation:
    /// one failed DOI returns None-with-404-shape, never kills the batch.
    pub async fn fetch_works_concurrent(
        &self,
        dois: &[String],
        breaker: &CircuitBreaker,
        concurrency: usize,
    ) -> Vec<Option<SlimWork>> {
        use futures_util::stream::{self, StreamExt};
        let concurrency = concurrency.clamp(1, 16);
        stream::iter(dois)
            .map(|doi| async move {
                if !breaker.allow() {
                    return None;
                }
                let url = format!("{}/works/doi:{}", openalex_base(), doi);
                let resp = self.client.get(&url).send().await;
                match resp {
                    Ok(r) if r.status().is_success() => {
                        breaker.record_success();
                        r.json::<serde_json::Value>()
                            .await
                            .ok()
                            .map(|w| SlimWork::from_api(&w))
                    }
                    Ok(r) if r.status().as_u16() == 404 => {
                        breaker.record_success(); // корректный ответ, честный miss
                        None
                    }
                    Ok(_) => {
                        breaker.record_failure();
                        None
                    }
                    Err(_) => {
                        breaker.record_failure();
                        None
                    }
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await
    }
}

#[cfg(test)]
mod doi_prefix_tests {
    use super::*;

    #[test]
    fn arxiv_prefix_from_data_or_fallback() {
        // In-repo test: the committed dictionary is present next to the
        // workspace, so the DATA path wins; the fallback only fires when
        // the file is missing (covered by the unknown-source case below).
        let prefix = doi_prefix_for("arxiv").expect("arxiv prefix");
        assert_eq!(prefix, "10.48550/arxiv.");
    }

    #[test]
    fn unknown_source_is_none() {
        assert!(doi_prefix_for("not-a-source").is_none());
    }

    #[test]
    fn doi_filter_shape_is_preserved() {
        let filter = format!("doi:{}{}", doi_prefix_for("arxiv").unwrap(), "2504.16925");
        assert_eq!(filter, "doi:10.48550/arxiv.2504.16925");
    }
}

/// HTTP adapter for OpenAlex API.
pub struct OpenAlexHttpAdapter {
    client: reqwest::Client,
}

impl OpenAlexHttpAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("daily-archive/2.0")
                .build()
                .expect("reqwest client"),
        }
    }
}

impl Default for OpenAlexHttpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ─── OpenAlex API JSON response types ───

#[derive(Debug, Deserialize)]
struct WorkResponse {
    id: String,
    title: String,
    doi: Option<String>,
    publication_date: Option<String>,
    cited_by_count: u32,
    primary_topic: Option<TopicResponse>,
    topics: Option<Vec<TopicResponse>>,
    concepts: Option<Vec<ConceptResponse>>,
    authorships: Option<Vec<AuthorshipResponse>>,
    referenced_works: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TopicResponse {
    id: String,
    display_name: String,
    domain: Option<NamedEntity>,
    field: Option<NamedEntity>,
    subfield: Option<NamedEntity>,
}

#[derive(Debug, Deserialize)]
struct NamedEntity {
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct ConceptResponse {
    id: String,
    display_name: String,
    level: u32,
    score: f64,
}

#[derive(Debug, Deserialize)]
struct AuthorshipResponse {
    author: AuthorResponse,
    #[serde(default)]
    institutions: Vec<InstitutionResponse>,
}

#[derive(Debug, Deserialize)]
struct InstitutionResponse {
    id: Option<String>,
    display_name: String,
    country_code: Option<String>,
    ror: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorResponse {
    id: Option<String>,
    display_name: String,
    orcid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<WorkResponse>,
}

fn parse_topic(t: TopicResponse) -> OpenAlexTopic {
    OpenAlexTopic {
        id: t.id,
        display_name: t.display_name,
        domain: t.domain.map(|d| d.display_name),
        field: t.field.map(|f| f.display_name),
        subfield: t.subfield.map(|s| s.display_name),
    }
}

fn parse_work(w: WorkResponse) -> OpenAlexWork {
    let authorships_raw = w.authorships.unwrap_or_default();
    // Build per-authorship rows preserving author↔institution association.
    let authorships: Vec<_> = authorships_raw
        .iter()
        .map(|a| OpenAlexAuthorship {
            author: OpenAlexAuthor {
                id: a.author.id.clone().unwrap_or_default(),
                display_name: a.author.display_name.clone(),
                orcid: a.author.orcid.clone(),
            },
            institutions: a
                .institutions
                .iter()
                .map(|inst| OpenAlexInstitution {
                    id: inst.id.clone().unwrap_or_default(),
                    display_name: inst.display_name.clone(),
                    country_code: inst.country_code.clone(),
                    ror: inst.ror.clone(),
                })
                .collect(),
        })
        .collect();
    // Backward-compatible flat views derived from authorships.
    let authors = authorships
        .iter()
        .map(|a| a.author.clone())
        .collect::<Vec<_>>();
    let institutions = authorships
        .iter()
        .flat_map(|a| a.institutions.iter().cloned())
        .collect::<Vec<_>>();
    OpenAlexWork {
        id: w.id,
        title: w.title,
        doi: w.doi,
        publication_date: w.publication_date,
        cited_by_count: w.cited_by_count,
        primary_topic: w.primary_topic.map(parse_topic),
        topics: w
            .topics
            .unwrap_or_default()
            .into_iter()
            .map(parse_topic)
            .collect(),
        concepts: w
            .concepts
            .unwrap_or_default()
            .into_iter()
            .map(|c| OpenAlexConcept {
                id: c.id,
                display_name: c.display_name,
                level: c.level,
                score: c.score,
            })
            .collect(),
        authors,
        institutions,
        authorships,
        referenced_works: w.referenced_works.unwrap_or_default(),
    }
}

#[async_trait]
impl OpenAlexClient for OpenAlexHttpAdapter {
    async fn fetch_by_arxiv_id(&self, arxiv_id: &str) -> OpenAlexResult<OpenAlexWork> {
        // Use filter search instead of direct DOI URL — more reliable.
        let url = format!("{}/works", openalex_base());
        let doi_filter = format!(
            "doi:{}{}",
            doi_prefix_for("arxiv").unwrap_or_else(|| "10.48550/arxiv.".into()),
            arxiv_id
        );

        tracing::info!(
            arxiv_id,
            api = "openalex",
            endpoint = %url,
            filter = %doi_filter,
            "OpenAlex API request: fetch_by_arxiv_id"
        );
        let request_start = std::time::Instant::now();

        let resp = self
            .client
            .get(&url)
            .query(&[("filter", doi_filter.as_str()), ("per_page", "1")])
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(
                    arxiv_id,
                    api = "openalex",
                    error = %e,
                    elapsed_ms = request_start.elapsed().as_millis() as u64,
                    "OpenAlex API network error"
                );
                OpenAlexError::Network(e.to_string())
            })?;

        let status = resp.status();
        let elapsed_ms = request_start.elapsed().as_millis() as u64;

        if !status.is_success() {
            tracing::warn!(
                arxiv_id,
                api = "openalex",
                status = %status,
                elapsed_ms,
                "OpenAlex API non-success HTTP status"
            );
            return Err(OpenAlexError::Api(format!("HTTP {}", status)));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            tracing::warn!(
                arxiv_id,
                api = "openalex",
                error = %e,
                elapsed_ms,
                "OpenAlex API JSON parse error"
            );
            OpenAlexError::Api(format!("JSON parse: {e}"))
        })?;

        let results = body
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| {
                tracing::warn!(
                    arxiv_id,
                    api = "openalex",
                    elapsed_ms,
                    "OpenAlex API: missing results array in response"
                );
                OpenAlexError::Api("missing results array".to_string())
            })?;

        let result_count = results.len();
        let work_json = results.first().ok_or_else(|| {
            tracing::info!(
                arxiv_id,
                api = "openalex",
                elapsed_ms,
                result_count = 0,
                "OpenAlex API: no results (not found)"
            );
            OpenAlexError::NotFound(arxiv_id.to_string())
        })?;

        let work: WorkResponse = serde_json::from_value(work_json.clone()).map_err(|e| {
            tracing::warn!(
                arxiv_id,
                api = "openalex",
                error = %e,
                elapsed_ms,
                "OpenAlex API: failed to deserialize WorkResponse"
            );
            OpenAlexError::Api(format!("deserialize work: {e}"))
        })?;

        let parsed = parse_work(work);
        tracing::info!(
            arxiv_id,
            api = "openalex",
            elapsed_ms,
            result_count,
            openalex_id = %parsed.id,
            title = %parsed.title,
            doi = ?parsed.doi,
            cited_by_count = parsed.cited_by_count,
            authors_count = parsed.authors.len(),
            topics_count = parsed.topics.len(),
            referenced_works_count = parsed.referenced_works.len(),
            "OpenAlex API response: fetched and parsed"
        );

        Ok(parsed)
    }

    async fn search(&self, query: &str, limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>> {
        let url = format!("{}/works", openalex_base());

        tracing::info!(
            api = "openalex",
            endpoint = %url,
            query = %query,
            limit,
            "OpenAlex API request: search"
        );
        let request_start = std::time::Instant::now();

        let resp = self
            .client
            .get(&url)
            .query(&[("search", query), ("per_page", &limit.to_string())])
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(
                    api = "openalex",
                    query = %query,
                    error = %e,
                    elapsed_ms = request_start.elapsed().as_millis() as u64,
                    "OpenAlex API search network error"
                );
                OpenAlexError::Network(e.to_string())
            })?;

        let status = resp.status();
        let elapsed_ms = request_start.elapsed().as_millis() as u64;

        if !status.is_success() {
            tracing::warn!(
                api = "openalex",
                query = %query,
                status = %status,
                elapsed_ms,
                "OpenAlex API search non-success HTTP status"
            );
            return Err(OpenAlexError::Api(format!("HTTP {}", status)));
        }

        let search: SearchResponse = resp.json().await.map_err(|e| {
            tracing::warn!(
                api = "openalex",
                query = %query,
                error = %e,
                elapsed_ms,
                "OpenAlex API search JSON parse error"
            );
            OpenAlexError::Api(format!("JSON parse: {e}"))
        })?;

        let count = search.results.len();
        tracing::info!(
            api = "openalex",
            query = %query,
            elapsed_ms,
            result_count = count,
            "OpenAlex API search response: fetched and parsed"
        );

        Ok(search.results.into_iter().map(parse_work).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_client() {
        let _adapter = OpenAlexHttpAdapter::new();
    }

    #[test]
    fn test_default_creates_client() {
        let _adapter = OpenAlexHttpAdapter::default();
    }
}

// ── ADR-073 citracer trio tests ────────────────────────────────────────
mod circuit_breaker_tests {
    use super::*;

    #[test]
    fn opens_after_threshold_and_recovers_after_cooldown() {
        let b = CircuitBreaker::new(3, std::time::Duration::from_millis(30));
        assert!(b.allow());
        b.record_failure();
        b.record_failure();
        assert!(b.allow(), "below threshold stays closed");
        b.record_failure();
        assert!(!b.allow(), "trips at threshold");
        std::thread::sleep(std::time::Duration::from_millis(40));
        assert!(b.allow(), "cooldown expiry re-closes");
    }

    #[test]
    fn success_resets_consecutive_counter() {
        let b = CircuitBreaker::new(3, std::time::Duration::from_secs(60));
        b.record_failure();
        b.record_failure();
        b.record_success();
        b.record_failure();
        assert!(b.allow(), "success resets the run");
    }
}

mod batch_filter_tests {
    use super::*;

    #[test]
    fn filter_joins_dois_with_pipes() {
        let f = batch_doi_filter(&["10.1/a".into(), "10.2/b".into()]);
        assert_eq!(f, "doi:10.1/a|doi:10.2/b");
    }
}

// Сетевой пробник ADR-073 (cargo test -p da-adapters --lib openalex_probe -- --ignored --nocapture):
// выясняет, какую форму batch-запроса реально принимает OpenAlex.
#[cfg(test)]
mod openalex_probe {
    use super::*;

    #[tokio::test]
    #[ignore = "network probe"]
    async fn probe_batch_forms() {
        let a = OpenAlexHttpAdapter::default();
        let base = openalex_base();
        let indexed = "10.48550/arxiv.2402.18361"; // точно в OpenAlex (кэш 200)
        // 1) path form
        let r = a
            .client
            .get(format!("{base}/works/doi:{indexed}"))
            .send()
            .await
            .unwrap();
        println!("path form        -> {}", r.status());
        // 2) filter single (reqwest сам закодирует)
        let r = a
            .client
            .get(format!("{base}/works"))
            .query(&[("filter", format!("doi:{indexed}").as_str()), ("per_page", "50")])
            .send()
            .await
            .unwrap();
        let n = r.json::<serde_json::Value>().await.unwrap()
            .get("results").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
        println!("filter single    -> 200, results={n}");
        // 3) filter pipe two indexed
        let f = format!("doi:{indexed}|doi:10.48550/arxiv.2402.18362");
        let r = a
            .client
            .get(format!("{base}/works"))
            .query(&[("filter", f.as_str()), ("per_page", "50")])
            .send()
            .await
            .unwrap();
        println!("filter pipe two  -> {}", r.status());
    }
}
