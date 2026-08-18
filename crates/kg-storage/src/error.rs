//! GraphStore error types.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::snapshot_identity::SnapshotIdentity;

/// Typed, visible identity-mismatch detail (T02). Carries BOTH sides so the
/// caller sees exactly how the expected and actual graphs diverge — never an
/// empty result. Boxed inside [`GraphStoreError`] to keep the error enum
/// small (clippy `result_large_err`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityMismatchDetail {
    pub expected_semantic_sha256: String,
    pub expected_tenant: String,
    pub expected_nodes: u64,
    pub expected_edges: u64,
    pub actual_semantic_sha256: String,
    pub actual_tenant: String,
    pub actual_nodes: u64,
    pub actual_edges: u64,
}

impl std::fmt::Display for IdentityMismatchDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "remote graph identity mismatch: expected semantic {} (tenant {}, {} nodes, {} edges), got semantic {} (tenant {}, {} nodes, {} edges)",
            self.expected_semantic_sha256,
            self.expected_tenant,
            self.expected_nodes,
            self.expected_edges,
            self.actual_semantic_sha256,
            self.actual_tenant,
            self.actual_nodes,
            self.actual_edges,
        )
    }
}

#[derive(Error, Debug)]
pub enum GraphStoreError {
    #[error("Node not found: {0}")]
    NotFound(String),
    #[error("Query error: {0}")]
    Query(String),
    #[error("Vector error: {0}")]
    Vector(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Method not implemented by this adapter: {0}")]
    NotImplemented(String),
    /// Remote read plane identity mismatch (T02). Carries BOTH sides so the
    /// caller sees exactly how the expected and actual graphs diverge —
    /// never an empty result.
    #[error("{0}")]
    IdentityMismatch(Box<IdentityMismatchDetail>),
}

impl GraphStoreError {
    /// Build a typed, visible identity-mismatch error from both identities.
    pub fn identity_mismatch(expected: &SnapshotIdentity, actual: &SnapshotIdentity) -> Self {
        GraphStoreError::IdentityMismatch(Box::new(IdentityMismatchDetail {
            expected_semantic_sha256: expected.semantic_sha256.clone(),
            expected_tenant: expected.tenant.clone(),
            expected_nodes: expected.node_count,
            expected_edges: expected.edge_count,
            actual_semantic_sha256: actual.semantic_sha256.clone(),
            actual_tenant: actual.tenant.clone(),
            actual_nodes: actual.node_count,
            actual_edges: actual.edge_count,
        }))
    }
}

pub type GraphResult<T> = Result<T, GraphStoreError>;

/// Typed environment-configuration error (T02). `from_env` implementations
/// fail closed: blank, non-numeric, zero and overflow values are rejected
/// with the offending key and raw value, never silently defaulted.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum EnvParseError {
    #[error("{key} must not be blank")]
    Blank { key: &'static str },
    #[error("{key}={value:?} is not a valid non-negative integer")]
    NonNumeric { key: &'static str, value: String },
    #[error("{key} must be greater than zero, got {value:?}")]
    Zero { key: &'static str, value: String },
    #[error("{key}={value:?} is out of range for the supported type")]
    Overflow { key: &'static str, value: String },
    #[error("{key}={value:?} is invalid: {reason}")]
    Invalid {
        key: &'static str,
        value: String,
        reason: String,
    },
}

/// Read a trimmed env value: unset → `Ok(None)`, blank (empty or
/// whitespace-only) → `Err(EnvParseError::Blank)`.
pub fn env_optional_string(
    map: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Option<String>, EnvParseError> {
    match map.get(key) {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                Err(EnvParseError::Blank { key })
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
    }
}

/// Parse an env value as `u64`. Unset → `Ok(None)`; blank → `Blank`;
/// anything but ASCII digits (including signs, decimals, `0x`, units) →
/// `NonNumeric`; digits that do not fit `u64` → `Overflow`.
pub fn env_u64(
    map: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Option<u64>, EnvParseError> {
    match map.get(key) {
        None => Ok(None),
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(EnvParseError::Blank { key });
            }
            if !trimmed.chars().all(|c| c.is_ascii_digit()) {
                return Err(EnvParseError::NonNumeric {
                    key,
                    value: trimmed.to_string(),
                });
            }
            trimmed
                .parse::<u64>()
                .map(Some)
                .map_err(|_| EnvParseError::Overflow {
                    key,
                    value: trimmed.to_string(),
                })
        }
    }
}

/// Parse an env value as `usize` (u64 first, then range-checked).
pub fn env_usize(
    map: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Option<usize>, EnvParseError> {
    match env_u64(map, key)? {
        None => Ok(None),
        Some(value) => usize::try_from(value)
            .map(Some)
            .map_err(|_| EnvParseError::Overflow {
                key,
                value: value.to_string(),
            }),
    }
}

/// Parse an env value as `u16` (u64 first, then range-checked).
pub fn env_u16(
    map: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Option<u16>, EnvParseError> {
    match env_u64(map, key)? {
        None => Ok(None),
        Some(value) => u16::try_from(value)
            .map(Some)
            .map_err(|_| EnvParseError::Overflow {
                key,
                value: value.to_string(),
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn env_u64_unset_is_none_and_parses_trimmed_digits() {
        assert_eq!(env_u64(&map(&[]), "K").unwrap(), None);
        assert_eq!(env_u64(&map(&[("K", "42")]), "K").unwrap(), Some(42));
        assert_eq!(env_u64(&map(&[("K", " 42 ")]), "K").unwrap(), Some(42));
        assert_eq!(env_u64(&map(&[("K", "0")]), "K").unwrap(), Some(0));
    }

    #[test]
    fn env_u64_rejects_blank_values() {
        assert_eq!(
            env_u64(&map(&[("K", "")]), "K"),
            Err(EnvParseError::Blank { key: "K" })
        );
        assert_eq!(
            env_u64(&map(&[("K", "   ")]), "K"),
            Err(EnvParseError::Blank { key: "K" })
        );
        assert_eq!(
            env_optional_string(&map(&[("K", "  ")]), "K"),
            Err(EnvParseError::Blank { key: "K" })
        );
    }

    #[test]
    fn env_u64_rejects_non_numeric_values() {
        for raw in ["12x", "-1", "1.5", "0x10", "1e3", "one"] {
            assert_eq!(
                env_u64(&map(&[("K", raw)]), "K"),
                Err(EnvParseError::NonNumeric {
                    key: "K",
                    value: raw.into()
                }),
                "raw {raw:?}"
            );
        }
    }

    #[test]
    fn env_u64_rejects_overflowing_values() {
        let huge = "99999999999999999999999999";
        assert_eq!(
            env_u64(&map(&[("K", huge)]), "K"),
            Err(EnvParseError::Overflow {
                key: "K",
                value: huge.into()
            })
        );
        // u16/u32/u64 range narrowing is also overflow.
        assert!(matches!(
            env_u16(&map(&[("K", "65536")]), "K"),
            Err(EnvParseError::Overflow { .. })
        ));
        assert!(matches!(
            env_u16(&map(&[("K", "65535")]), "K"),
            Ok(Some(65535))
        ));
        assert!(matches!(
            env_usize(&map(&[("K", "18446744073709551616")]), "K"),
            Err(EnvParseError::Overflow { .. })
        ));
    }

    #[test]
    fn env_optional_string_parses_and_rejects() {
        assert_eq!(
            env_optional_string(&map(&[("H", " 0.0.0.0 ")]), "H").unwrap(),
            Some("0.0.0.0".to_string())
        );
        assert_eq!(env_optional_string(&map(&[]), "H").unwrap(), None);
    }
}
