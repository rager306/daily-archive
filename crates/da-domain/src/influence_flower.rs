//! Local Influence Flower scoring (M394).
//!
//! Idea from Shin et al., arXiv:1907.12748 §4.2: a citation from paper A to
//! paper B contributes `1 / n` to each cited entity, where `n` is the number
//! of entities on the *cited* paper. This is candidate-only observation, not
//! graph truth.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const BUNDLED_POLICY: &str = include_str!("../../../data/ontology/influence_flower_policy.yaml");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfluenceFlowerPolicy {
    pub schema_version: String,
    pub import_eligible: bool,
    pub skip_self_cite: bool,
    pub reject_empty_cited_authors: bool,
}

impl InfluenceFlowerPolicy {
    pub fn bundled() -> Result<Self, String> {
        Self::from_yaml(BUNDLED_POLICY)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        let policy: Self = serde_yaml::from_str(yaml).map_err(|e| e.to_string())?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != "influence-flower-policy.v1" {
            return Err(format!(
                "unsupported influence flower policy: {}",
                self.schema_version
            ));
        }
        if self.import_eligible {
            return Err("influence flower policy must keep import_eligible=false".into());
        }
        Ok(())
    }
}

/// One observed citation between two papers with already-known author sets.
/// Author ids are opaque strings supplied by the caller (OpenAlex vids, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationInfluenceEvent {
    pub citing_authors: Vec<String>,
    pub cited_authors: Vec<String>,
}

/// Author-normalized influence weight of one citation onto one cited entity.
pub fn influence_weight(cited_entity_count: u32) -> Result<f64, String> {
    if cited_entity_count == 0 {
        return Err("cited entity count must be >= 1".into());
    }
    Ok(1.0 / f64::from(cited_entity_count))
}

/// Accumulated directed influence: source author → target author.
///
/// A citation contributes `1/n` from each citing author to each cited author,
/// where `n = cited_authors.len()`. This matches the paper example:
/// Maria cites John's two-author paper → John→Maria = 0.5.
pub fn accumulate_author_influence(
    policy: &InfluenceFlowerPolicy,
    events: &[CitationInfluenceEvent],
) -> Result<BTreeMap<(String, String), f64>, String> {
    policy.validate()?;
    let mut scores: BTreeMap<(String, String), f64> = BTreeMap::new();
    for event in events {
        if event.cited_authors.is_empty() {
            if policy.reject_empty_cited_authors {
                return Err("cited author list is empty; refusing implicit n=1".into());
            }
            continue;
        }
        let n = u32::try_from(event.cited_authors.len()).map_err(|_| "too many cited authors")?;
        let weight = influence_weight(n)?;
        for citing in &event.citing_authors {
            for cited in &event.cited_authors {
                if policy.skip_self_cite && citing == cited {
                    continue;
                }
                *scores.entry((cited.clone(), citing.clone())).or_insert(0.0) += weight;
            }
        }
    }
    Ok(scores)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_policy_is_candidate_only() {
        let p = InfluenceFlowerPolicy::bundled().unwrap();
        assert!(!p.import_eligible);
        assert!(p.skip_self_cite);
        assert!(p.reject_empty_cited_authors);
    }

    #[test]
    fn weight_is_one_over_n() {
        assert_eq!(influence_weight(1).unwrap(), 1.0);
        assert_eq!(influence_weight(2).unwrap(), 0.5);
        assert_eq!(influence_weight(4).unwrap(), 0.25);
        assert!(influence_weight(0).is_err());
    }

    #[test]
    fn maria_citing_two_author_paper_gives_half() {
        // Paper example: Maria cites Algorithms (John + coauthor) once → 0.5.
        let p = InfluenceFlowerPolicy::bundled().unwrap();
        let events = [CitationInfluenceEvent {
            citing_authors: vec!["maria".into()],
            cited_authors: vec!["john".into(), "coauthor".into()],
        }];
        let scores = accumulate_author_influence(&p, &events).unwrap();
        assert!((scores[&("john".into(), "maria".into())] - 0.5).abs() < 1e-12);
        assert!((scores[&("coauthor".into(), "maria".into())] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn self_cite_is_skipped() {
        let p = InfluenceFlowerPolicy::bundled().unwrap();
        let events = [CitationInfluenceEvent {
            citing_authors: vec!["john".into()],
            cited_authors: vec!["john".into(), "other".into()],
        }];
        let scores = accumulate_author_influence(&p, &events).unwrap();
        assert!(!scores.contains_key(&("john".into(), "john".into())));
        assert!((scores[&("other".into(), "john".into())] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn empty_cited_authors_rejected() {
        let p = InfluenceFlowerPolicy::bundled().unwrap();
        let events = [CitationInfluenceEvent {
            citing_authors: vec!["maria".into()],
            cited_authors: vec![],
        }];
        assert!(accumulate_author_influence(&p, &events).is_err());
    }

    #[test]
    fn accumulate_is_order_independent() {
        let p = InfluenceFlowerPolicy::bundled().unwrap();
        let a = CitationInfluenceEvent {
            citing_authors: vec!["maria".into()],
            cited_authors: vec!["john".into()],
        };
        let b = CitationInfluenceEvent {
            citing_authors: vec!["lee".into()],
            cited_authors: vec!["john".into(), "ann".into()],
        };
        let s1 = accumulate_author_influence(&p, &[a.clone(), b.clone()]).unwrap();
        let s2 = accumulate_author_influence(&p, &[b, a]).unwrap();
        assert_eq!(s1.len(), s2.len());
        for (k, v) in &s1 {
            assert!((s2[k] - v).abs() < 1e-12);
        }
    }
}
