//! Versioning + temporality (ADR-037 §6, ADR-040 §11.5).
//!
//! Samyama MVCC is for concurrency control, not temporal queries.
//! We implement data-level temporality via Versioned<T> + SUPERSEDES edges.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A versioned value with temporal bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versioned<T> {
    pub current: T,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub version: u32,
    pub superseded_by: Option<String>,
}

/// A temporal record holding the full version history of an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalRecord<T> {
    pub entity_id: String,
    pub history: Vec<Versioned<T>>,
}

impl<T> TemporalRecord<T> {
    pub fn new(entity_id: String, initial: T) -> Self {
        let now = Utc::now();
        Self {
            entity_id,
            history: vec![Versioned {
                current: initial,
                valid_from: now,
                valid_to: None,
                version: 1,
                superseded_by: None,
            }],
        }
    }

    /// Get the version effective at a point in time (bi-temporal query).
    pub fn as_of(&self, when: DateTime<Utc>) -> Option<&Versioned<T>> {
        self.history
            .iter()
            .rev()
            .find(|v| v.valid_from <= when && v.valid_to.map_or(true, |to| to > when))
    }

    /// Get the current (latest) version.
    pub fn current(&self) -> Option<&Versioned<T>> {
        self.history.iter().rev().find(|v| v.valid_to.is_none())
    }

    /// Supersede the current version with a new one.
    pub fn supersede(&mut self, new_value: T) {
        let now = Utc::now();
        if let Some(last) = self.history.last_mut() {
            last.valid_to = Some(now);
        }
        let next_version = self.history.len() as u32 + 1;
        self.history.push(Versioned {
            current: new_value,
            valid_from: now,
            valid_to: None,
            version: next_version,
            superseded_by: None,
        });
    }

    /// Number of versions in history.
    pub fn version_count(&self) -> usize {
        self.history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_has_one_version() {
        let rec = TemporalRecord::new("vid:123".to_string(), "first title".to_string());
        assert_eq!(rec.version_count(), 1);
        assert_eq!(rec.current().unwrap().version, 1);
    }

    #[test]
    fn test_supersede_creates_new_version() {
        let mut rec = TemporalRecord::new("vid:123".to_string(), "v1".to_string());
        rec.supersede("v2".to_string());
        assert_eq!(rec.version_count(), 2);
        assert_eq!(rec.current().unwrap().current, "v2");
        assert!(rec.history[0].valid_to.is_some()); // old version closed
    }

    #[test]
    fn test_as_of_query() {
        let mut rec = TemporalRecord::new("vid:123".to_string(), "v1".to_string());
        let past = Utc::now();
        // simulate time passing
        std::thread::sleep(std::time::Duration::from_millis(10));
        rec.supersede("v2".to_string());

        // Query the past: should get v1
        let old = rec.as_of(past).unwrap();
        assert_eq!(old.current, "v1");

        // Query now: should get v2
        let now_rec = rec.current().unwrap();
        assert_eq!(now_rec.current, "v2");
    }
}
