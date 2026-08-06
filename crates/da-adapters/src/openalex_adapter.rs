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

const OPENALEX_BASE: &str = "https://api.openalex.org";

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
        let url = format!("{}/works", OPENALEX_BASE);
        let doi_filter = format!("doi:10.48550/arxiv.{}", arxiv_id);

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

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| {
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
        let work_json = results
            .first()
            .ok_or_else(|| {
                tracing::info!(
                    arxiv_id,
                    api = "openalex",
                    elapsed_ms,
                    result_count = 0,
                    "OpenAlex API: no results (not found)"
                );
                OpenAlexError::NotFound(arxiv_id.to_string())
            })?;

        let work: WorkResponse = serde_json::from_value(work_json.clone())
            .map_err(|e| {
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
        let url = format!("{}/works", OPENALEX_BASE);

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

        let search: SearchResponse = resp
            .json()
            .await
            .map_err(|e| {
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
