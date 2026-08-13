//! Candidate Influence Flower from already-known citation events (M394 S02).
//!
//! Does not touch Samyama. Scores stay import_eligible=false.

use da_domain::{CitationInfluenceEvent, InfluenceFlowerPolicy, accumulate_author_influence};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum InfluenceFlowerError {
    #[error("{0}")]
    Domain(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfluencePetal {
    pub entity_id: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfluenceFlower {
    pub ego: String,
    pub incoming: Vec<InfluencePetal>,
    pub outgoing: Vec<InfluencePetal>,
    pub policy_schema_version: String,
    pub import_eligible: bool,
}

impl InfluenceFlower {
    pub fn validate(&self) -> Result<(), InfluenceFlowerError> {
        if self.ego.trim().is_empty() {
            return Err(InfluenceFlowerError::Domain("ego must be non-empty".into()));
        }
        if self.import_eligible {
            return Err(InfluenceFlowerError::Domain(
                "flower must keep import_eligible=false".into(),
            ));
        }
        Ok(())
    }
}

/// Build a candidate flower for `ego` from citation events.
pub fn build_author_flower(
    policy: &InfluenceFlowerPolicy,
    ego: &str,
    events: &[CitationInfluenceEvent],
) -> Result<InfluenceFlower, InfluenceFlowerError> {
    if ego.trim().is_empty() {
        return Err(InfluenceFlowerError::Domain("ego must be non-empty".into()));
    }
    let scores =
        accumulate_author_influence(policy, events).map_err(InfluenceFlowerError::Domain)?;
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    for ((source, target), score) in scores {
        if source == ego && target != ego {
            outgoing.push(InfluencePetal {
                entity_id: target,
                score,
            });
        } else if target == ego && source != ego {
            incoming.push(InfluencePetal {
                entity_id: source,
                score,
            });
        }
    }
    incoming.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.entity_id.cmp(&b.entity_id))
    });
    outgoing.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.entity_id.cmp(&b.entity_id))
    });
    let flower = InfluenceFlower {
        ego: ego.to_string(),
        incoming,
        outgoing,
        policy_schema_version: policy.schema_version.clone(),
        import_eligible: false,
    };
    flower.validate()?;
    Ok(flower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ego_has_incoming_and_outgoing_petals() {
        let policy = InfluenceFlowerPolicy::bundled().unwrap();
        // Maria cites John's two-author paper → outgoing from John to Maria = 0.5
        // John cites Maria's solo paper twice conceptually as two events → incoming to John from Maria = 2.0
        let events = [
            CitationInfluenceEvent {
                citing_authors: vec!["maria".into()],
                cited_authors: vec!["john".into(), "ann".into()],
            },
            CitationInfluenceEvent {
                citing_authors: vec!["john".into()],
                cited_authors: vec!["maria".into()],
            },
            CitationInfluenceEvent {
                citing_authors: vec!["john".into()],
                cited_authors: vec!["maria".into()],
            },
        ];
        let flower = build_author_flower(&policy, "john", &events).unwrap();
        assert!(!flower.import_eligible);
        assert_eq!(flower.outgoing.len(), 1);
        assert_eq!(flower.outgoing[0].entity_id, "maria");
        assert!((flower.outgoing[0].score - 0.5).abs() < 1e-12);
        assert_eq!(flower.incoming.len(), 1);
        assert_eq!(flower.incoming[0].entity_id, "maria");
        assert!((flower.incoming[0].score - 2.0).abs() < 1e-12);
    }

    #[test]
    fn empty_ego_rejected() {
        let policy = InfluenceFlowerPolicy::bundled().unwrap();
        assert!(build_author_flower(&policy, "  ", &[]).is_err());
    }
}
