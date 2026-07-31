//! OpenAlex HTTP adapter — implements OpenAlexClient port.
//!
//! D133: Fetches curated metadata from api.openalex.org.
//! Replaces noisy YAKE keyword extraction with professional, disambiguated data.

use async_trait::async_trait;
use da_ports::openalex::{
    OpenAlexAuthor, OpenAlexClient, OpenAlexConcept, OpenAlexError, OpenAlexInstitution,
    OpenAlexResult, OpenAlexTopic, OpenAlexWork,
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
    let authorships = w.authorships.unwrap_or_default();
    let authors: Vec<_> = authorships
        .iter()
        .map(|a| OpenAlexAuthor {
            id: a.author.id.clone().unwrap_or_default(),
            display_name: a.author.display_name.clone(),
            orcid: a.author.orcid.clone(),
        })
        .collect();
    let institutions: Vec<_> = authorships
        .iter()
        .flat_map(|a| {
            a.institutions.iter().map(|inst| OpenAlexInstitution {
                id: inst.id.clone().unwrap_or_default(),
                display_name: inst.display_name.clone(),
                country_code: inst.country_code.clone(),
                ror: inst.ror.clone(),
            })
        })
        .collect();
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
        referenced_works: w.referenced_works.unwrap_or_default(),
    }
}

#[async_trait]
impl OpenAlexClient for OpenAlexHttpAdapter {
    async fn fetch_by_arxiv_id(&self, arxiv_id: &str) -> OpenAlexResult<OpenAlexWork> {
        // Use filter search instead of direct DOI URL — more reliable.
        let url = format!("{}/works", OPENALEX_BASE);
        let doi_filter = format!("doi:10.48550/arxiv.{}", arxiv_id);

        let resp = self
            .client
            .get(&url)
            .query(&[("filter", doi_filter.as_str()), ("per_page", "1")])
            .send()
            .await
            .map_err(|e| OpenAlexError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(OpenAlexError::Api(format!("HTTP {}", resp.status())));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| OpenAlexError::Api(format!("JSON parse: {e}")))?;

        let results = body
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| OpenAlexError::Api("missing results array".to_string()))?;

        let work_json = results
            .first()
            .ok_or_else(|| OpenAlexError::NotFound(arxiv_id.to_string()))?;

        let work: WorkResponse = serde_json::from_value(work_json.clone())
            .map_err(|e| OpenAlexError::Api(format!("deserialize work: {e}")))?;

        Ok(parse_work(work))
    }

    async fn search(&self, query: &str, limit: usize) -> OpenAlexResult<Vec<OpenAlexWork>> {
        let url = format!("{}/works", OPENALEX_BASE);

        let resp = self
            .client
            .get(&url)
            .query(&[("search", query), ("per_page", &limit.to_string())])
            .send()
            .await
            .map_err(|e| OpenAlexError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(OpenAlexError::Api(format!("HTTP {}", resp.status())));
        }

        let search: SearchResponse = resp
            .json()
            .await
            .map_err(|e| OpenAlexError::Api(format!("JSON parse: {e}")))?;

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
