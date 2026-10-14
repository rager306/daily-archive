//! Deterministic semantic identity for Samyama `.sgsnap` bytes.
//!
//! Physical SHA identifies exact compressed bytes. Semantic SHA ignores gzip,
//! export time, record ordering and physical IDs, and binds ontology-v2 node
//! VIDs, labels, all properties and edge topology/properties. Legacy v1 identity
//! is unchanged when no declaration records exist. Catalog presence (even empty)
//! or hierarchy declarations select v2, which also binds their declared values.
//! Neither version binds export metadata, timestamps or full persistence history.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const SEMANTIC_SNAPSHOT_ALGORITHM: &str = "sgsnap-vid-canonical-json.v1";
pub const SEMANTIC_SNAPSHOT_ALGORITHM_V2: &str = "sgsnap-vid-canonical-json.v2";
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

impl SnapshotIdentity {
    /// Serialize to the canonical JSON payload served by `DA.IDENTITY` (T02).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse the `DA.IDENTITY` JSON payload back into a typed identity (T02).
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
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
    #[error("snapshot declaration line {line} is invalid: {error}")]
    InvalidDeclaration { line: usize, error: String },
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexCatalogRecord {
    #[serde(rename = "t")]
    _record_type: String,
    #[serde(default)]
    definitions: Vec<IndexDefinition>,
}

// These wire types mirror the pinned Samyama snapshot declarations without
// requiring the optional Samyama feature. Unknown declaration fields fail closed;
// unlike legacy header/n/e metadata, they may carry unbound index semantics.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum IndexDefinition {
    Property {
        label: String,
        property: String,
    },
    UniqueConstraint {
        label: String,
        property: String,
    },
    FullText {
        name: String,
        label: String,
        property: String,
    },
    Vector {
        name: Option<String>,
        label: String,
        property: String,
        dimensions: usize,
        metric: DistanceMetric,
        quantization: Quantization,
    },
}

#[derive(Debug, Serialize, Deserialize)]
enum DistanceMetric {
    L2,
    Cosine,
    InnerProduct,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Quantization {
    None,
    Fp16,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HierarchyRecord {
    #[serde(rename = "t", skip_serializing)]
    _record_type: String,
    name: String,
    edge_types: Vec<String>,
    #[serde(default)]
    reverse: bool,
    #[serde(default)]
    measure_label: Option<String>,
    #[serde(default)]
    measure_property: Option<String>,
    #[serde(default)]
    ops: Vec<String>,
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
    let mut index_catalog = None;
    let mut hierarchies = Vec::new();
    let mut hierarchy_names = BTreeSet::new();
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
            "i" => {
                if index_catalog.is_some() {
                    return Err(SnapshotIdentityError::InvalidDeclaration {
                        line: line_number,
                        error: "duplicate index catalog".into(),
                    });
                }
                // Parse the original line so serde rejects duplicate declaration
                // fields instead of accepting the last value in a JSON map.
                let catalog: IndexCatalogRecord = serde_json::from_str(line).map_err(|error| {
                    SnapshotIdentityError::InvalidJson {
                        line: line_number,
                        error: error.to_string(),
                    }
                })?;
                index_catalog = Some(canonical_index_definitions(
                    catalog.definitions,
                    line_number,
                )?);
            }
            "h" => {
                let hierarchy: HierarchyRecord = serde_json::from_str(line).map_err(|error| {
                    SnapshotIdentityError::InvalidJson {
                        line: line_number,
                        error: error.to_string(),
                    }
                })?;
                if !hierarchy_names.insert(hierarchy.name.clone()) {
                    return Err(SnapshotIdentityError::InvalidDeclaration {
                        line: line_number,
                        error: format!("duplicate hierarchy name {:?}", hierarchy.name),
                    });
                }
                hierarchies.push(canonical_hierarchy(hierarchy, line_number)?);
            }
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

    let has_declarations = index_catalog.is_some() || !hierarchies.is_empty();
    let semantic_algorithm = if has_declarations {
        SEMANTIC_SNAPSHOT_ALGORITHM_V2
    } else {
        SEMANTIC_SNAPSHOT_ALGORITHM
    };
    let mut canonical = serde_json::json!({
        "algorithm": semantic_algorithm,
        "tenant": header.tenant,
        "nodes": canonical_nodes,
        "edges": canonical_edges,
    });
    if has_declarations {
        hierarchies.sort_by_key(canonical_json);
        // None is null, whereas an explicit empty catalog is []. The upstream
        // importer distinguishes them when deciding whether to rediscover indexes.
        canonical["index_catalog"] =
            serde_json::to_value(index_catalog).expect("JSON index catalog");
        canonical["hierarchies"] = Value::Array(hierarchies);
    }
    let semantic_sha256 = hex::encode(Sha256::digest(canonical_json(&canonical).as_bytes()));
    Ok(SnapshotIdentity {
        physical_sha256,
        semantic_sha256,
        semantic_algorithm: semantic_algorithm.into(),
        snapshot_format_version: header.version,
        tenant: canonical["tenant"].as_str().unwrap_or_default().into(),
        node_count: header.node_count,
        edge_count: header.edge_count,
    })
}

fn canonical_index_definitions(
    definitions: Vec<IndexDefinition>,
    line: usize,
) -> Result<Vec<Value>, SnapshotIdentityError> {
    let mut canonical = Vec::with_capacity(definitions.len());
    let mut seen = BTreeSet::new();
    let mut fulltext_names = BTreeSet::new();
    let mut vector_keys = BTreeSet::new();
    let mut vector_names = BTreeSet::new();
    for definition in definitions {
        match &definition {
            IndexDefinition::FullText { name, .. } => {
                if !fulltext_names.insert(name.clone()) {
                    return Err(SnapshotIdentityError::InvalidDeclaration {
                        line,
                        error: format!("duplicate or conflicting full-text index name {name:?}"),
                    });
                }
            }
            IndexDefinition::Vector {
                name,
                label,
                property,
                ..
            } => {
                if !vector_keys.insert((label.clone(), property.clone())) {
                    return Err(SnapshotIdentityError::InvalidDeclaration {
                        line,
                        error: format!(
                            "duplicate or conflicting vector index on {label:?}/{property:?}"
                        ),
                    });
                }
                if name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .is_some_and(|alias| !vector_names.insert(alias.to_string()))
                {
                    return Err(SnapshotIdentityError::InvalidDeclaration {
                        line,
                        error: format!("duplicate or conflicting vector index name {name:?}"),
                    });
                }
            }
            IndexDefinition::Property { .. } | IndexDefinition::UniqueConstraint { .. } => {}
        }
        let value = serde_json::to_value(definition).expect("JSON index definition");
        if !seen.insert(canonical_json(&value)) {
            return Err(SnapshotIdentityError::InvalidDeclaration {
                line,
                error: "duplicate index definition".into(),
            });
        }
        canonical.push(value);
    }
    canonical.sort_by_key(canonical_json);
    Ok(canonical)
}

fn canonical_hierarchy(
    mut hierarchy: HierarchyRecord,
    line: usize,
) -> Result<Value, SnapshotIdentityError> {
    hierarchy.edge_types.sort();
    hierarchy.edge_types.dedup();
    for op in &mut hierarchy.ops {
        op.make_ascii_lowercase();
        if !matches!(op.as_str(), "sum" | "count" | "min" | "max") {
            return Err(SnapshotIdentityError::InvalidDeclaration {
                line,
                error: format!("unsupported hierarchy operation {op:?}"),
            });
        }
    }
    hierarchy.ops.sort();
    hierarchy.ops.dedup();
    Ok(serde_json::to_value(hierarchy).expect("JSON hierarchy declaration"))
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

    const LEGACY_JSON: &str = r#"{"format":"sgsnap","version":2,"tenant":"default","node_count":2,"edge_count":1,"created_at":"ignored"}
{"t":"n","id":1,"labels":["Paper"],"props":{"vid":"paper-a","title":"A"}}
{"t":"n","id":2,"labels":["Entity"],"props":{"name":"B","vid":"entity-b"}}
{"t":"e","id":3,"src":1,"tgt":2,"type":"MENTIONS","props":{"confidence":0.9}}
"#;

    const CATALOG_JSON: &str = r#"{"t":"i","definitions":[{"Vector":{"name":"paper_embedding","label":"Paper","property":"embedding","dimensions":2,"metric":"Cosine","quantization":"fp16"}},{"UniqueConstraint":{"label":"Paper","property":"vid"}},{"Property":{"label":"Paper","property":"vid"}},{"FullText":{"name":"paper_title","label":"Paper","property":"title"}}]}
"#;

    const HIERARCHY_JSON: &str = r#"{"t":"h","name":"mentions","edge_types":["SUBSUMES","MENTIONS","SUBSUMES"],"reverse":true,"measure_label":"Paper","measure_property":"score","ops":["SUM","count","sum"]}
"#;

    fn gzip_json(text: &str) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(text.as_bytes()).unwrap();
        encoder.finish().unwrap()
    }

    fn declared_fixture(declarations: &str) -> Vec<u8> {
        let (header, records) = LEGACY_JSON.split_once('\n').unwrap();
        gzip_json(&format!("{header}\n{declarations}{records}"))
    }

    #[test]
    fn legacy_v1_raw_gzip_has_fixed_physical_and_semantic_identity() {
        // The compressed bytes and digest vectors are independent fixtures,
        // not produced by the Samyama exporter or canonicalization under test.
        let bytes = hex::decode(concat!(
            "1f8b08000000000002035d8f4d4b43311045f7fe8cbb4e45ebcab753e8a28b56417722659acc7b",
            "045e2721490be5d1ffee248afd58ce9d730f3313fa907654d0210f5928c2e0c029fb20e8e606858",
            "5a46e1df7b41f8bae2538ded8b0afb112ec86fff1d1c026a6c26ed3947e9090d8e17437a1cea27",
            "5ef1a37d296c78cee0bef1439e1db20a610359970a80c62cd67a495e2cbc89abce0746b9a5f9a16",
            "a2e4f14a25b4abcdd7fa56b3726366dbb38aff544f0639d9765c197e7f2bc758dbabc5fa73f9b6",
            "fec085d806e9bd63b10a3cdc3fabed0717c80bcc4b010000"
        ))
        .unwrap();
        let identity = fingerprint_snapshot(&bytes).unwrap();
        assert_eq!(identity.semantic_algorithm, SEMANTIC_SNAPSHOT_ALGORITHM);
        assert_eq!(
            identity.semantic_sha256,
            "9ccdc8704f3e6b58516b3d38508f211fae920548eb7a696d5f89314c96640f9d"
        );
        assert_eq!(
            identity.physical_sha256,
            "ee2860733cedf15e2272276c33e5dc3c21ab717dfb0d6e026715e1069ec1bd4d"
        );
        let mut decoded = String::new();
        GzDecoder::new(bytes.as_slice())
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(decoded, LEGACY_JSON);
        let recompressed = fingerprint_snapshot(&gzip_json(LEGACY_JSON)).unwrap();
        assert_eq!(recompressed.semantic_algorithm, identity.semantic_algorithm);
        assert_eq!(recompressed.semantic_sha256, identity.semantic_sha256);
        let v1_header = LEGACY_JSON.replace("\"version\":2", "\"version\":1");
        let old_format = fingerprint_snapshot(&gzip_json(&v1_header)).unwrap();
        assert_eq!(old_format.snapshot_format_version, 1);
        assert_eq!(old_format.semantic_sha256, identity.semantic_sha256);
    }

    #[test]
    fn explicit_empty_catalog_selects_v2_and_binds_presence() {
        let legacy = fingerprint_snapshot(&gzip_json(LEGACY_JSON)).unwrap();
        let explicit =
            fingerprint_snapshot(&declared_fixture("{\"t\":\"i\",\"definitions\":[]}\n")).unwrap();
        let defaulted = fingerprint_snapshot(&declared_fixture("{\"t\":\"i\"}\n")).unwrap();
        assert_eq!(explicit.semantic_algorithm, SEMANTIC_SNAPSHOT_ALGORITHM_V2);
        assert_eq!(explicit.snapshot_format_version, 2);
        assert_eq!(explicit.node_count, 2);
        assert_eq!(explicit.edge_count, 1);
        assert_eq!(
            explicit.semantic_sha256,
            "31f6bc461ca0516866a68ba96b61eefba82cd94f172c6c843c98992a709218e2"
        );
        assert_ne!(explicit.semantic_sha256, legacy.semantic_sha256);
        assert_eq!(explicit.semantic_sha256, defaulted.semantic_sha256);

        let hierarchy = "{\"t\":\"h\",\"name\":\"mentions\",\"edge_types\":[\"MENTIONS\"]}\n";
        let no_catalog = fingerprint_snapshot(&declared_fixture(hierarchy)).unwrap();
        let with_catalog =
            fingerprint_snapshot(&declared_fixture(&format!("{{\"t\":\"i\"}}\n{hierarchy}")))
                .unwrap();
        assert_eq!(
            no_catalog.semantic_algorithm,
            SEMANTIC_SNAPSHOT_ALGORITHM_V2
        );
        assert_ne!(no_catalog.semantic_sha256, with_catalog.semantic_sha256);
    }

    #[test]
    fn v2_typed_declarations_have_fixed_identity_and_unordered_metadata() {
        let declarations = format!("{CATALOG_JSON}{HIERARCHY_JSON}");
        let identity = fingerprint_snapshot(&declared_fixture(&declarations)).unwrap();
        assert_eq!(identity.semantic_algorithm, SEMANTIC_SNAPSHOT_ALGORITHM_V2);
        assert_eq!(
            identity.semantic_sha256,
            "efd9684afe0d5e581cdce227d04a497f6dd386524a6fb22211df1fa4d2f303c8"
        );
        let reordered = r#"{"format":"sgsnap","version":2,"tenant":"default","node_count":2,"edge_count":1,"created_at":"later"}
{"t":"h","ops":["sum","COUNT"],"measure_property":"score","measure_label":"Paper","reverse":true,"edge_types":["MENTIONS","SUBSUMES"],"name":"mentions"}
{"t":"e","id":55,"tgt":77,"src":91,"props":{"confidence":0.9},"type":"MENTIONS"}
{"t":"i","definitions":[{"FullText":{"property":"title","name":"paper_title","label":"Paper"}},{"Property":{"property":"vid","label":"Paper"}},{"UniqueConstraint":{"property":"vid","label":"Paper"}},{"Vector":{"quantization":"fp16","metric":"Cosine","dimensions":2,"property":"embedding","label":"Paper","name":"paper_embedding"}}]}
{"t":"n","id":77,"labels":["Entity"],"props":{"vid":"entity-b","name":"B"},"created_at":10}
{"t":"n","id":91,"labels":["Paper"],"props":{"title":"A","vid":"paper-a"},"updated_at":20}
"#;
        let reordered = fingerprint_snapshot(&gzip_json(reordered)).unwrap();
        assert_eq!(identity.semantic_sha256, reordered.semantic_sha256);
        assert_ne!(identity.physical_sha256, reordered.physical_sha256);
        assert_eq!(
            SnapshotIdentity::from_json(&identity.to_json().unwrap()).unwrap(),
            identity
        );
    }

    #[test]
    fn v2_binds_each_declared_index_and_hierarchy_field() {
        let declarations = format!("{CATALOG_JSON}{HIERARCHY_JSON}");
        let baseline = fingerprint_snapshot(&declared_fixture(&declarations)).unwrap();
        let changes = [
            ("\"label\":\"Paper\"", "\"label\":\"Document\""),
            ("\"property\":\"vid\"", "\"property\":\"stable_id\""),
            ("paper_title", "document_title"),
            ("paper_embedding", "document_embedding"),
            ("\"embedding\"", "\"vector\""),
            ("\"dimensions\":2", "\"dimensions\":3"),
            ("\"metric\":\"Cosine\"", "\"metric\":\"L2\""),
            ("\"metric\":\"Cosine\"", "\"metric\":\"InnerProduct\""),
            ("\"quantization\":\"fp16\"", "\"quantization\":\"none\""),
            ("\"name\":\"paper_embedding\"", "\"name\":null"),
            ("\"name\":\"mentions\"", "\"name\":\"taxonomy\""),
            ("SUBSUMES", "CONTAINS"),
            ("\"reverse\":true", "\"reverse\":false"),
            (
                "\"measure_label\":\"Paper\"",
                "\"measure_label\":\"Entity\"",
            ),
            (
                "\"measure_property\":\"score\"",
                "\"measure_property\":\"weight\"",
            ),
            ("[\"SUM\",\"count\",\"sum\"]", "[\"min\",\"max\"]"),
        ];
        for (before, after) in changes {
            let changed = declarations.replacen(before, after, 1);
            assert_ne!(changed, declarations);
            let identity = fingerprint_snapshot(&declared_fixture(&changed)).unwrap();
            assert_ne!(
                identity.semantic_sha256, baseline.semantic_sha256,
                "{before}"
            );
        }
        let without_unique = CATALOG_JSON.replace(
            "{\"UniqueConstraint\":{\"label\":\"Paper\",\"property\":\"vid\"}},",
            "",
        );
        let identity = fingerprint_snapshot(&declared_fixture(&format!(
            "{without_unique}{HIERARCHY_JSON}"
        )))
        .unwrap();
        assert_ne!(identity.semantic_sha256, baseline.semantic_sha256);
    }

    #[test]
    fn hierarchy_wire_defaults_are_declared_not_importer_fallbacks() {
        let minimal = "{\"t\":\"h\",\"name\":\"mentions\",\"edge_types\":[\"MENTIONS\"]}\n";
        let explicit = "{\"t\":\"h\",\"name\":\"mentions\",\"edge_types\":[\"MENTIONS\"],\"reverse\":false,\"measure_label\":null,\"measure_property\":null,\"ops\":[]}\n";
        let baseline = fingerprint_snapshot(&declared_fixture(minimal)).unwrap();
        let explicit = fingerprint_snapshot(&declared_fixture(explicit)).unwrap();
        assert_eq!(baseline.semantic_sha256, explicit.semantic_sha256);
        let count = minimal.replace("}\n", ",\"ops\":[\"count\"]}\n");
        let count = fingerprint_snapshot(&declared_fixture(&count)).unwrap();
        assert_ne!(baseline.semantic_sha256, count.semantic_sha256);
        let measured = minimal.replace("}\n", ",\"measure_property\":\"score\"}\n");
        let measured_sum = measured.replace("}\n", ",\"ops\":[\"sum\"]}\n");
        let measured = fingerprint_snapshot(&declared_fixture(&measured)).unwrap();
        let measured_sum = fingerprint_snapshot(&declared_fixture(&measured_sum)).unwrap();
        assert_ne!(measured.semantic_sha256, measured_sum.semantic_sha256);
        let other = "{\"t\":\"h\",\"name\":\"other\",\"edge_types\":[\"SUBSUMES\"]}\n";
        let ordered =
            fingerprint_snapshot(&declared_fixture(&format!("{minimal}{other}"))).unwrap();
        let reversed =
            fingerprint_snapshot(&declared_fixture(&format!("{other}{minimal}"))).unwrap();
        assert_eq!(ordered.semantic_sha256, reversed.semantic_sha256);
    }

    #[test]
    fn malformed_declarations_and_unknown_wire_values_fail_closed() {
        let malformed = [
            r#"{"t":"i","definitions":null}"#,
            r#"{"t":"i","definitions":[{"Spatial":{"label":"Paper"}}]}"#,
            r#"{"t":"i","definitions":[{"Property":{"label":"Paper"}}]}"#,
            r#"{"t":"i","definitions":[{"Property":{"label":"Paper","property":2}}]}"#,
            r#"{"t":"i","definitions":[{"Vector":{"label":"Paper","property":"v","dimensions":-1,"metric":"Cosine","quantization":"none"}}]}"#,
            r#"{"t":"i","definitions":[{"Vector":{"label":"Paper","property":"v","dimensions":2,"metric":"DotProduct","quantization":"none"}}]}"#,
            r#"{"t":"i","definitions":[{"Vector":{"label":"Paper","property":"v","dimensions":2,"metric":"Cosine","quantization":"f32"}}]}"#,
            r#"{"t":"i","extra":true}"#,
            r#"{"t":"i","definitions":[{"Property":{"label":"Paper","property":"vid","unique":true}}]}"#,
            r#"{"t":"i","definitions":[],"definitions":[]}"#,
            r#"{"t":"h","name":"mentions"}"#,
            r#"{"t":"h","name":"mentions","edge_types":"MENTIONS"}"#,
            r#"{"t":"h","name":"mentions","edge_types":[],"reverse":"true"}"#,
            r#"{"t":"h","name":"mentions","edge_types":[],"ops":[1]}"#,
            r#"{"t":"h","name":"mentions","edge_types":[],"ops":["avg"]}"#,
            r#"{"t":"h","name":"mentions","edge_types":[],"measure_label":false}"#,
            r#"{"t":"h","name":"mentions","edge_types":[],"extra":true}"#,
            r#"{"t":"h","name":"mentions","name":"other","edge_types":[]}"#,
        ];
        for declaration in malformed {
            assert!(
                matches!(
                    fingerprint_snapshot(&declared_fixture(&format!("{declaration}\n"))),
                    Err(SnapshotIdentityError::InvalidJson { .. }
                        | SnapshotIdentityError::InvalidDeclaration { .. })
                ),
                "{declaration}"
            );
        }
        assert!(matches!(
            fingerprint_snapshot(&declared_fixture("{\"t\":\"future\"}\n")),
            Err(SnapshotIdentityError::UnsupportedRecord { line: 2, .. })
        ));
    }

    #[test]
    fn duplicate_and_conflicting_declarations_fail_closed() {
        let duplicates = [
            "{\"t\":\"i\"}\n{\"t\":\"i\"}\n".to_string(),
            format!("{HIERARCHY_JSON}{HIERARCHY_JSON}"),
            "{\"t\":\"h\",\"name\":\"mentions\",\"edge_types\":[\"MENTIONS\"]}\n{\"t\":\"h\",\"name\":\"mentions\",\"edge_types\":[\"OTHER\"]}\n".to_string(),
            r#"{"t":"i","definitions":[{"Property":{"label":"Paper","property":"vid"}},{"Property":{"label":"Paper","property":"vid"}}]}
"#.to_string(),
            r#"{"t":"i","definitions":[{"UniqueConstraint":{"label":"Paper","property":"vid"}},{"UniqueConstraint":{"label":"Paper","property":"vid"}}]}
"#.to_string(),
            r#"{"t":"i","definitions":[{"FullText":{"name":"title","label":"Paper","property":"title"}},{"FullText":{"name":"title","label":"Entity","property":"name"}}]}
"#.to_string(),
            r#"{"t":"i","definitions":[{"Vector":{"name":"a","label":"Paper","property":"v","dimensions":2,"metric":"Cosine","quantization":"none"}},{"Vector":{"name":"b","label":"Paper","property":"v","dimensions":3,"metric":"Cosine","quantization":"fp16"}}]}
"#.to_string(),
            r#"{"t":"i","definitions":[{"Vector":{"name":"a","label":"Paper","property":"v","dimensions":2,"metric":"Cosine","quantization":"none"}},{"Vector":{"name":"a","label":"Entity","property":"v","dimensions":2,"metric":"Cosine","quantization":"none"}}]}
"#.to_string(),
        ];
        let first = serde_json::json!({"Vector": {"name": "a", "label": "Paper", "property": "v", "dimensions": 2, "metric": "Cosine", "quantization": "none"}});
        let second = serde_json::json!({"Vector": {"name": " a ", "label": "Entity", "property": "v", "dimensions": 2, "metric": "Cosine", "quantization": "none"}});
        for definitions in [vec![first.clone(), second.clone()], vec![second, first]] {
            let declaration = format!(
                "{}\n",
                serde_json::json!({"t":"i", "definitions": definitions})
            );
            assert!(matches!(
                fingerprint_snapshot(&declared_fixture(&declaration)),
                Err(SnapshotIdentityError::InvalidDeclaration { .. })
            ));
        }
        for declarations in duplicates {
            assert!(
                matches!(
                    fingerprint_snapshot(&declared_fixture(&declarations)),
                    Err(SnapshotIdentityError::InvalidDeclaration { .. })
                ),
                "{declarations}"
            );
        }
    }

    #[test]
    fn declaration_records_respect_existing_limits_and_graph_validation() {
        let bytes = declared_fixture(&format!("{CATALOG_JSON}{HIERARCHY_JSON}"));
        assert!(matches!(
            fingerprint_snapshot_with_limits(
                &bytes,
                SnapshotIdentityLimits {
                    max_decompressed_bytes: DEFAULT_MAX_DECOMPRESSED_BYTES,
                    max_records: 4,
                }
            ),
            Err(SnapshotIdentityError::RecordLimit(4))
        ));
        assert!(
            fingerprint_snapshot_with_limits(
                &bytes,
                SnapshotIdentityLimits {
                    max_decompressed_bytes: DEFAULT_MAX_DECOMPRESSED_BYTES,
                    max_records: 5,
                }
            )
            .is_ok()
        );
        assert!(matches!(
            fingerprint_snapshot_with_limits(
                &bytes,
                SnapshotIdentityLimits {
                    max_decompressed_bytes: 16,
                    max_records: 5,
                }
            ),
            Err(SnapshotIdentityError::DecompressedLimit(16))
        ));
        let bad_count = LEGACY_JSON.replace("\"node_count\":2", "\"node_count\":3");
        let (header, records) = bad_count.split_once('\n').unwrap();
        assert!(matches!(
            fingerprint_snapshot(&gzip_json(&format!("{header}\n{CATALOG_JSON}{records}"))),
            Err(SnapshotIdentityError::CountMismatch { .. })
        ));
        let dangling = LEGACY_JSON.replace("\"tgt\":2", "\"tgt\":999");
        let (header, records) = dangling.split_once('\n').unwrap();
        assert!(matches!(
            fingerprint_snapshot(&gzip_json(&format!("{header}\n{CATALOG_JSON}{records}"))),
            Err(SnapshotIdentityError::DanglingEndpoint { .. })
        ));
    }
}
