//! Deterministic semantic identity for Samyama `.sgsnap` bytes.
//!
//! Physical SHA identifies exact compressed bytes. Semantic SHA ignores gzip,
//! export time, record ordering and physical IDs, and binds ontology-v2 node
//! VIDs, labels, all properties and edge topology/properties.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const SEMANTIC_SNAPSHOT_ALGORITHM: &str = "sgsnap-vid-canonical-json.v1";
pub const DEFAULT_MAX_DECOMPRESSED_BYTES: usize = 256 * 1024 * 1024;
pub const DEFAULT_MAX_RECORDS: usize = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotIdentity {
    pub physical_sha256: String,
    pub semantic_sha256: String,
    pub semantic_algorithm: String,
    pub snapshot_format_version: u32,
    pub tenant: String,
    pub node_count: u64,
    pub edge_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotIdentityLimits {
    pub max_decompressed_bytes: usize,
    pub max_records: usize,
}

impl Default for SnapshotIdentityLimits {
    fn default() -> Self {
        Self {
            max_decompressed_bytes: DEFAULT_MAX_DECOMPRESSED_BYTES,
            max_records: DEFAULT_MAX_RECORDS,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SnapshotIdentityError {
    #[error("snapshot gzip decode failed: {0}")]
    Gzip(String),
    #[error("snapshot decompressed bytes exceed limit {0}")]
    DecompressedLimit(usize),
    #[error("snapshot record count exceeds limit {0}")]
    RecordLimit(usize),
    #[error("snapshot is empty")]
    Empty,
    #[error("snapshot JSON line {line} is invalid: {error}")]
    InvalidJson { line: usize, error: String },
    #[error("snapshot header is invalid: {0}")]
    InvalidHeader(String),
    #[error("unsupported snapshot format/version {format:?}/v{version}")]
    UnsupportedFormat { format: String, version: u32 },
    #[error("snapshot record line {line} has unsupported type {record_type:?}")]
    UnsupportedRecord { line: usize, record_type: String },
    #[error("snapshot node physical id {physical_id} is missing unique string vid")]
    MissingVid { physical_id: u64 },
    #[error("snapshot contains duplicate node vid {0:?}")]
    DuplicateVid(String),
    #[error("snapshot contains duplicate physical node id {0}")]
    DuplicatePhysicalId(u64),
    #[error("snapshot edge {edge_id} has dangling endpoint {endpoint}")]
    DanglingEndpoint { edge_id: u64, endpoint: u64 },
    #[error(
        "snapshot header count mismatch: header nodes={header_nodes} edges={header_edges}, records nodes={record_nodes} edges={record_edges}"
    )]
    CountMismatch {
        header_nodes: u64,
        header_edges: u64,
        record_nodes: u64,
        record_edges: u64,
    },
}

#[derive(Debug, Deserialize)]
struct Header {
    format: String,
    version: u32,
    tenant: String,
    node_count: u64,
    edge_count: u64,
}

#[derive(Debug, Deserialize)]
struct NodeRecord {
    id: u64,
    labels: Vec<String>,
    props: serde_json::Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct EdgeRecord {
    id: u64,
    src: u64,
    tgt: u64,
    #[serde(rename = "type")]
    edge_type: String,
    props: serde_json::Map<String, Value>,
}

pub fn fingerprint_snapshot(bytes: &[u8]) -> Result<SnapshotIdentity, SnapshotIdentityError> {
    fingerprint_snapshot_with_limits(bytes, SnapshotIdentityLimits::default())
}

pub fn fingerprint_snapshot_with_limits(
    bytes: &[u8],
    limits: SnapshotIdentityLimits,
) -> Result<SnapshotIdentity, SnapshotIdentityError> {
    let physical_sha256 = hex::encode(Sha256::digest(bytes));
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder
        .by_ref()
        .take(limits.max_decompressed_bytes.saturating_add(1) as u64)
        .read_to_end(&mut decompressed)
        .map_err(|error| SnapshotIdentityError::Gzip(error.to_string()))?;
    if decompressed.len() > limits.max_decompressed_bytes {
        return Err(SnapshotIdentityError::DecompressedLimit(
            limits.max_decompressed_bytes,
        ));
    }
    let text = std::str::from_utf8(&decompressed)
        .map_err(|error| SnapshotIdentityError::InvalidHeader(error.to_string()))?;
    let mut lines = text.lines();
    let header_line = lines.next().ok_or(SnapshotIdentityError::Empty)?;
    let header: Header =
        serde_json::from_str(header_line).map_err(|error| SnapshotIdentityError::InvalidJson {
            line: 1,
            error: error.to_string(),
        })?;
    if header.format != "sgsnap" || !matches!(header.version, 1 | 2) {
        return Err(SnapshotIdentityError::UnsupportedFormat {
            format: header.format,
            version: header.version,
        });
    }

    let records = lines.collect::<Vec<_>>();
    if records.len() > limits.max_records {
        return Err(SnapshotIdentityError::RecordLimit(limits.max_records));
    }
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for (offset, line) in records.iter().enumerate() {
        let line_number = offset + 2;
        let value: Value =
            serde_json::from_str(line).map_err(|error| SnapshotIdentityError::InvalidJson {
                line: line_number,
                error: error.to_string(),
            })?;
        let record_type = value.get("t").and_then(Value::as_str).unwrap_or("");
        match record_type {
            "n" => nodes.push(
                serde_json::from_value::<NodeRecord>(value).map_err(|error| {
                    SnapshotIdentityError::InvalidJson {
                        line: line_number,
                        error: error.to_string(),
                    }
                })?,
            ),
            "e" => edges.push(
                serde_json::from_value::<EdgeRecord>(value).map_err(|error| {
                    SnapshotIdentityError::InvalidJson {
                        line: line_number,
                        error: error.to_string(),
                    }
                })?,
            ),
            other => {
                return Err(SnapshotIdentityError::UnsupportedRecord {
                    line: line_number,
                    record_type: other.into(),
                });
            }
        }
    }
    if nodes.len() as u64 != header.node_count || edges.len() as u64 != header.edge_count {
        return Err(SnapshotIdentityError::CountMismatch {
            header_nodes: header.node_count,
            header_edges: header.edge_count,
            record_nodes: nodes.len() as u64,
            record_edges: edges.len() as u64,
        });
    }

    let mut physical_to_vid = BTreeMap::new();
    let mut vids = BTreeSet::new();
    let mut canonical_nodes = Vec::with_capacity(nodes.len());
    for node in nodes {
        if physical_to_vid.contains_key(&node.id) {
            return Err(SnapshotIdentityError::DuplicatePhysicalId(node.id));
        }
        let vid = node.props.get("vid").and_then(Value::as_str).ok_or(
            SnapshotIdentityError::MissingVid {
                physical_id: node.id,
            },
        )?;
        if !vids.insert(vid.to_string()) {
            return Err(SnapshotIdentityError::DuplicateVid(vid.into()));
        }
        let vid = vid.to_string();
        physical_to_vid.insert(node.id, vid.clone());
        let mut labels = node.labels;
        labels.sort();
        labels.dedup();
        canonical_nodes.push(serde_json::json!({
            "labels": labels,
            "props": canonical_object(node.props),
            "vid": vid,
        }));
    }
    canonical_nodes.sort_by_key(canonical_json);

    let mut canonical_edges = Vec::with_capacity(edges.len());
    for edge in edges {
        let source =
            physical_to_vid
                .get(&edge.src)
                .ok_or(SnapshotIdentityError::DanglingEndpoint {
                    edge_id: edge.id,
                    endpoint: edge.src,
                })?;
        let target =
            physical_to_vid
                .get(&edge.tgt)
                .ok_or(SnapshotIdentityError::DanglingEndpoint {
                    edge_id: edge.id,
                    endpoint: edge.tgt,
                })?;
        canonical_edges.push(serde_json::json!({
            "source_vid": source,
            "target_vid": target,
            "type": edge.edge_type,
            "props": canonical_object(edge.props),
        }));
    }
    canonical_edges.sort_by_key(canonical_json);

    let canonical = serde_json::json!({
        "algorithm": SEMANTIC_SNAPSHOT_ALGORITHM,
        "tenant": header.tenant,
        "nodes": canonical_nodes,
        "edges": canonical_edges,
    });
    let semantic_sha256 = hex::encode(Sha256::digest(canonical_json(&canonical).as_bytes()));
    Ok(SnapshotIdentity {
        physical_sha256,
        semantic_sha256,
        semantic_algorithm: SEMANTIC_SNAPSHOT_ALGORITHM.into(),
        snapshot_format_version: header.version,
        tenant: canonical["tenant"].as_str().unwrap_or_default().into(),
        node_count: header.node_count,
        edge_count: header.edge_count,
    })
}

fn canonical_object(object: serde_json::Map<String, Value>) -> Value {
    Value::Object(
        object
            .into_iter()
            .map(|(key, value)| (key, canonical_value(value)))
            .collect(),
    )
}

fn canonical_value(value: Value) -> Value {
    match value {
        Value::Object(object) => canonical_object(object),
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_value).collect()),
        other => other,
    }
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value.clone())))
                .collect::<BTreeMap<_, _>>();
            let mut output = String::from("{");
            for (index, (key, value)) in sorted.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).expect("JSON key"));
                output.push(':');
                output.push_str(&canonical_json(value));
            }
            output.push('}');
            output
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => serde_json::to_string(other).expect("JSON scalar"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    fn snapshot(header_time: &str, ids: (u64, u64, u64), reversed: bool, changed: bool) -> Vec<u8> {
        let (a, b, edge) = ids;
        let mut records = vec![
            serde_json::json!({"t":"n","id":a,"labels":["Paper"],"props":{"vid":"paper-a","title":if changed {"Changed"} else {"A"}}}),
            serde_json::json!({"t":"n","id":b,"labels":["Entity"],"props":{"name":"B","vid":"entity-b"}}),
            serde_json::json!({"t":"e","id":edge,"src":a,"tgt":b,"type":"MENTIONS","props":{"confidence":0.9}}),
        ];
        if reversed {
            records.reverse();
        }
        let header = serde_json::json!({"format":"sgsnap","version":2,"tenant":"default","node_count":2,"edge_count":1,"labels":[],"edge_types":[],"created_at":header_time,"samyama_version":"x"});
        let text = std::iter::once(header)
            .chain(records)
            .map(|value| serde_json::to_string(&value).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn semantic_digest_ignores_header_order_gzip_and_physical_ids() {
        let left = snapshot("2026-01-01T00:00:00Z", (1, 2, 3), false, false);
        let right = snapshot("2026-02-02T00:00:00Z", (91, 77, 55), true, false);
        let a = fingerprint_snapshot(&left).unwrap();
        let b = fingerprint_snapshot(&right).unwrap();
        assert_ne!(a.physical_sha256, b.physical_sha256);
        assert_eq!(a.semantic_sha256, b.semantic_sha256);
        assert_eq!(a.node_count, 2);
        assert_eq!(a.edge_count, 1);
    }

    #[test]
    fn semantic_mutation_changes_digest() {
        let before = fingerprint_snapshot(&snapshot("a", (1, 2, 3), false, false)).unwrap();
        let after = fingerprint_snapshot(&snapshot("a", (1, 2, 3), false, true)).unwrap();
        assert_ne!(before.semantic_sha256, after.semantic_sha256);
    }

    #[test]
    fn missing_duplicate_vid_dangling_and_limits_fail_closed() {
        let missing = snapshot("a", (1, 2, 3), false, false);
        let mut decoder = GzDecoder::new(missing.as_slice());
        let mut text = String::new();
        decoder.read_to_string(&mut text).unwrap();
        let missing = text.replace("\"vid\":\"paper-a\"", "\"not_vid\":\"paper-a\"");
        let encode = |text: &str| {
            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            e.write_all(text.as_bytes()).unwrap();
            e.finish().unwrap()
        };
        assert!(matches!(
            fingerprint_snapshot(&encode(&missing)),
            Err(SnapshotIdentityError::MissingVid { .. })
        ));
        let duplicate = text.replace("entity-b", "paper-a");
        assert!(matches!(
            fingerprint_snapshot(&encode(&duplicate)),
            Err(SnapshotIdentityError::DuplicateVid(_))
        ));
        let dangling = text.replace("\"tgt\":2", "\"tgt\":999");
        assert!(matches!(
            fingerprint_snapshot(&encode(&dangling)),
            Err(SnapshotIdentityError::DanglingEndpoint { .. })
        ));
        assert!(matches!(
            fingerprint_snapshot_with_limits(
                &encode(&text),
                SnapshotIdentityLimits {
                    max_decompressed_bytes: 8,
                    max_records: 10
                }
            ),
            Err(SnapshotIdentityError::DecompressedLimit(8))
        ));
    }
}
