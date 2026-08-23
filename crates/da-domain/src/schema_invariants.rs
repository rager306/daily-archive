//! Generic schema-invariant engine (ADR-068, M451).
//!
//! This is NOT an article validator — it is the COMMON validation
//! engine for any schema-manifest object. Articles are the first
//! object type (`article_schema.yaml`); textbook chapters, ontology
//! dictionaries and graph snapshots follow the same contract.
//!
//! # Meta/prompt principle
//!
//! Invariant PATTERNS live here as reusable code; WHICH fields,
//! vocabularies and collections a pattern applies to is declared by
//! the object's schema YAML (DATA). Adding a new invariant to an
//! object type requires a schema edit, not new Rust code.
//!
//! # Patterns
//!
//! - `derived_state_from_events` — a state field must equal the last
//!   event's stage (states are derived, never hand-set)
//! - `monotonic_timestamp` — a timestamp field never decreases in an
//!   event log
//! - `enum_membership` — field values must come from a schema
//!   vocabulary
//! - `graph_edges_known` — `derived_from` edges point at nodes the
//!   schema declares (or an allowed root like `source`)
//! - `cross_layer_stat_eq` — two layer stats must be equal
//!   (e.g. `ast_nodes == sections`)
//! - `collection_records` — every record of an on-disk collection
//!   satisfies a field requirement (present + >= min)
//!
//! # Design
//!
//! - Pure logic, no IO (da-domain). Objects arrive as
//!   [`serde_yaml::Value`]; stores/ports do the reading.
//! - Returns ALL violations in one pass.
//! - Unknown patterns in a schema are a configuration error, reported
//!   as violations of kind [`ViolationKind::UnknownPattern`] — fail
//!   loud, never silently skip.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which generic pattern failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationKind {
    DerivedStateMismatch,
    TimestampNotMonotonic,
    ValueNotInVocabulary,
    EdgeTargetUnknown,
    CrossLayerStatMismatch,
    RecordFieldInvalid,
    ObjectFileMissing,
    UnknownPattern,
}

/// A single violation for one object instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaViolation {
    pub object_id: String,
    pub pattern: String,
    pub kind: ViolationKind,
    pub evidence: String,
}

/// One invariant declaration from the object schema.
/// `pattern` selects the engine rule; remaining fields are pattern
/// parameters, kept as data so the engine never hardcodes object shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvariantDeclaration {
    pub pattern: String,
    #[serde(flatten)]
    pub params: BTreeMap<String, serde_yaml::Value>,
}

/// The norm parsed from an object schema: vocabularies + invariant
/// declarations. Vocabularies are keyed lists the schema declares
/// (e.g. `lifecycle_order`, `layers`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectNorm {
    /// named vocabularies: name -> allowed values
    pub vocabularies: BTreeMap<String, Vec<String>>,
    pub invariants: Vec<InvariantDeclaration>,
}

#[derive(thiserror::Error, Debug)]
pub enum ObjectNormError {
    #[error("object norm: YAML parse failed: {0}")]
    Parse(String),
}

fn seq_strings(v: &serde_yaml::Value) -> Vec<String> {
    v.as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

impl ObjectNorm {
    /// Parse a norm from schema YAML text. Recognizes vocabularies it
    /// knows how to extract (`lifecycle_order`, `layers[].id`); other
    /// object types extend [`ObjectNorm::register_vocabulary`].
    pub fn from_yaml_str(yaml: &str) -> Result<Self, ObjectNormError> {
        let v: serde_yaml::Value = serde_yaml::from_str(yaml)
            .map_err(|e| ObjectNormError::Parse(e.to_string()))?;
        let mut norm = Self::default();

        if let Some(order) = v.get("lifecycle_order") {
            norm.vocabularies
                .insert("lifecycle_order".into(), seq_strings(order));
        }
        if let Some(layers) = v.get("layers").and_then(|l| l.as_sequence()) {
            let ids = layers
                .iter()
                .filter_map(|l| l.get("id"))
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect();
            norm.vocabularies.insert("layers".into(), ids);
        }
        if let Some(invs) = v.get("invariants").and_then(|i| i.as_sequence()) {
            for inv in invs {
                if let Some(pattern) = inv.get("pattern").and_then(|p| p.as_str()) {
                    let params: BTreeMap<String, serde_yaml::Value> = inv
                        .as_mapping()
                        .map(|m| {
                            m.iter()
                                .filter(|(k, _)| k.as_str() != Some("pattern"))
                                .filter_map(|(k, val)| {
                                    k.as_str().map(|ks| (ks.to_string(), val.clone()))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    norm.invariants.push(InvariantDeclaration {
                        pattern: pattern.to_string(),
                        params,
                    });
                }
            }
        }
        Ok(norm)
    }

    pub fn vocabulary(&self, name: &str) -> Option<&[String]> {
        self.vocabularies.get(name).map(|v| v.as_slice())
    }
}

fn param_str<'a>(d: &'a InvariantDeclaration, key: &str) -> Option<&'a str> {
    d.params.get(key).and_then(|v| v.as_str())
}

fn param_list(d: &InvariantDeclaration, key: &str) -> Vec<String> {
    d.params.get(key).map(seq_strings).unwrap_or_default()
}

fn param_i64(d: &InvariantDeclaration, key: &str) -> Option<i64> {
    d.params.get(key).and_then(|v| v.as_i64())
}

fn param_bool(d: &InvariantDeclaration, key: &str) -> bool {
    d.params.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Validate one object instance (a manifest parsed as YAML) against the
/// norm. Pure: filesystem-dependent patterns (`collection_records`,
/// file existence) delegate to [`FileBackedChecks`].
pub fn validate_object(
    object_id: &str,
    object: &serde_yaml::Value,
    norm: &ObjectNorm,
) -> Vec<SchemaViolation> {
    let mut out = Vec::new();
    for decl in &norm.invariants {
        match decl.pattern.as_str() {
            "derived_state_from_events" => {
                derived_state_from_events(object_id, object, decl, &mut out)
            }
            "monotonic_timestamp" => monotonic_timestamp(object_id, object, decl, &mut out),
            "enum_membership" => enum_membership(object_id, object, decl, norm, &mut out),
            "graph_edges_known" => graph_edges_known(object_id, object, decl, norm, &mut out),
            "cross_layer_stat_eq" => cross_layer_stat_eq(object_id, object, decl, &mut out),
            "node_ref_integrity" => node_ref_integrity(object_id, object, decl, &mut out),
            "list_symmetric_next_prev" => list_symmetric_next_prev(object_id, object, decl, &mut out),
            // file-backed: skipped here, run via FileBackedChecks
            "collection_records" => {}
            other => out.push(SchemaViolation {
                object_id: object_id.to_string(),
                pattern: other.to_string(),
                kind: ViolationKind::UnknownPattern,
                evidence: format!("pattern {other:?} not implemented by engine"),
            }),
        }
    }
    out
}

/// File-backed pattern context: how to read a collection file for an
/// object and check artifact existence. Implemented by ports/adapters;
/// keeps the engine pure.
pub trait FileBackedChecks {
    /// Read the collection declared by the invariant (returns records).
    fn collection_records(
        &self,
        object_id: &str,
        decl: &InvariantDeclaration,
    ) -> Result<Option<Vec<serde_yaml::Value>>, String>;
    /// Does the artifact file (relative path) exist for the object?
    fn artifact_exists(&self, object_id: &str, rel: &str) -> Result<bool, String>;
}

/// Run the file-backed patterns (`collection_records`; artifact file
/// existence for `graph_edges_known`-declared graphs).
pub fn validate_file_backed(
    object_id: &str,
    object: &serde_yaml::Value,
    norm: &ObjectNorm,
    io: &dyn FileBackedChecks,
) -> Vec<SchemaViolation> {
    let mut out = Vec::new();
    for decl in &norm.invariants {
        match decl.pattern.as_str() {
            "collection_records" => {
                // DATA-driven gate: when the layer is honestly missing
                // (typed missingness, e.g. extraction_not_run for textbook
                // chapters), the collection check does not apply.
                let gated = decl
                    .params
                    .get("require_layer")
                    .and_then(|v| v.as_str())
                    .and_then(|layer| {
                        object
                            .get("artifacts")
                            .and_then(|a| a.get(layer))
                            .and_then(|l| l.get("present"))
                            .and_then(|v| v.as_bool())
                            .map(|present| !present)
                    })
                    .unwrap_or(false);
                if !gated {
                    collection_records(object_id, decl, io, &mut out);
                }
            }
            "graph_edges_known" => {
                artifact_files_exist(object_id, object, decl, io, &mut out);
            }
            // list-based patterns whose list lives in a separate DATA file:
            // read the collection, wrap it as {list_field: [...]}, and run
            // the pure implementation — never silently skip.
            "node_ref_integrity" | "list_symmetric_next_prev" => {
                let Some(file) = decl.params.get("file").and_then(|v| v.as_str()) else {
                    out.push(bad_params(object_id, &decl.pattern));
                    continue;
                };
                match io.collection_records(object_id, &{
                    let mut d = decl.clone();
                    d.params.insert(
                        "file".to_string(),
                        serde_yaml::Value::from(file),
                    );
                    d
                }) {
                    Ok(Some(nodes)) if !nodes.is_empty() => {
                        let list_field = decl
                            .params
                            .get("list_field")
                            .and_then(|v| v.as_str())
                            .unwrap_or("navigation");
                        let mut wrapped = serde_yaml::Mapping::new();
                        wrapped.insert(
                            serde_yaml::Value::from(list_field),
                            serde_yaml::Value::Sequence(nodes),
                        );
                        let wrapped = serde_yaml::Value::Mapping(wrapped);
                        match decl.pattern.as_str() {
                            "node_ref_integrity" => node_ref_integrity(
                                object_id,
                                &wrapped,
                                decl,
                                &mut out,
                            ),
                            _ => list_symmetric_next_prev(
                                object_id,
                                &wrapped,
                                decl,
                                &mut out,
                            ),
                        }
                    }
                    Ok(_) => out.push(SchemaViolation {
                        object_id: object_id.to_string(),
                        pattern: decl.pattern.clone(),
                        kind: ViolationKind::ObjectFileMissing,
                        evidence: format!("navigation list {file:?} empty/absent"),
                    }),
                    Err(e) => out.push(SchemaViolation {
                        object_id: object_id.to_string(),
                        pattern: decl.pattern.clone(),
                        kind: ViolationKind::ObjectFileMissing,
                        evidence: format!("navigation list unreadable: {e}"),
                    }),
                }
            }
            _ => {}
        }
    }
    out
}

/// `node_ref_integrity`: every declared reference field of every node in
/// a list points at an existing node id (dangling parents/next/prev).
/// Pure: the list arrives as part of the object (manifest-level) or is
/// injected as `nodes` by the file-backed pass.
fn node_ref_integrity(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    out: &mut Vec<SchemaViolation>,
) {
    let (Some(list_field), Some(id_field)) = (
        param_str(decl, "list_field"),
        param_str(decl, "id_field"),
    ) else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let ref_fields = param_list(decl, "ref_fields");
    let Some(nodes) = obj.get(list_field).and_then(|v| v.as_sequence()) else {
        return;
    };
    let ids: Vec<&str> = nodes
        .iter()
        .filter_map(|n| n.get(id_field).and_then(|v| v.as_str()))
        .collect();
    for n in nodes {
        let node_id = n.get(id_field).and_then(|v| v.as_str()).unwrap_or("?");
        for rf in &ref_fields {
            if let Some(target) = n.get(rf).and_then(|v| v.as_str())
                && !ids.contains(&target)
            {
                out.push(SchemaViolation {
                    object_id: id.to_string(),
                    pattern: decl.pattern.clone(),
                    kind: ViolationKind::EdgeTargetUnknown,
                    evidence: format!("{node_id}.{rf} -> dangling {target:?}"),
                });
            }
        }
    }
}

/// `list_symmetric_next_prev`: for every node, next(A)=B implies
/// prev(B)=A (document-order walk consistency).
fn list_symmetric_next_prev(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    out: &mut Vec<SchemaViolation>,
) {
    let (Some(list_field), Some(id_field)) = (
        param_str(decl, "list_field"),
        param_str(decl, "id_field"),
    ) else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let Some(nodes) = obj.get(list_field).and_then(|v| v.as_sequence()) else {
        return;
    };
    let by_id: Vec<(&str, &serde_yaml::Value)> = nodes
        .iter()
        .filter_map(|n| n.get(id_field).and_then(|v| v.as_str()).map(|s| (s, n)))
        .collect();
    for (node_id, n) in &by_id {
        let Some(next) = n.get("next_section_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some((_, target)) = by_id.iter().find(|(k, _)| *k == next) else {
            continue; // dangling handled by node_ref_integrity
        };
        let back = target.get("prev_section_id").and_then(|v| v.as_str());
        if back != Some(*node_id) {
            out.push(SchemaViolation {
                object_id: id.to_string(),
                pattern: decl.pattern.clone(),
                kind: ViolationKind::TimestampNotMonotonic, // reuse ordering-kind
                evidence: format!("next({node_id})={next:?} but prev({next})={back:?}"),
            });
        }
    }
}

// ── pattern implementations ──────────────────────────────────────────

fn derived_state_from_events(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    out: &mut Vec<SchemaViolation>,
) {
    let (Some(state_field), Some(events_field), Some(stage_field)) = (
        param_str(decl, "state_field"),
        param_str(decl, "events_field"),
        param_str(decl, "stage_field"),
    ) else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let default_state = param_str(decl, "default_state").unwrap_or("raw");
    let events = obj.get(events_field).and_then(|e| e.as_sequence());
    let expected = events
        .and_then(|evs| evs.last())
        .and_then(|e| e.get(stage_field))
        .and_then(|s| s.as_str())
        .unwrap_or(default_state);
    let actual = obj.get(state_field).and_then(|s| s.as_str()).unwrap_or("");
    if actual != expected {
        out.push(SchemaViolation {
            object_id: id.to_string(),
            pattern: decl.pattern.clone(),
            kind: ViolationKind::DerivedStateMismatch,
            evidence: format!("{state_field}={actual:?} but last {stage_field}={expected:?}"),
        });
    }
}

fn monotonic_timestamp(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    out: &mut Vec<SchemaViolation>,
) {
    let Some(field) = param_str(decl, "field") else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    // dotted path with [] list-all semantics: "events[].recorded_at"
    let Some((list_field, leaf)) = split_list_path(field) else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let Some(events) = obj.get(list_field).and_then(|e| e.as_sequence()) else {
        return;
    };
    let times: Vec<&str> = events
        .iter()
        .filter_map(|e| e.get(leaf))
        .filter_map(|t| t.as_str())
        .collect();
    if times.len() > 1 {
        let mut sorted = times.clone();
        sorted.sort_unstable();
        if times != sorted {
            out.push(SchemaViolation {
                object_id: id.to_string(),
                pattern: decl.pattern.clone(),
                kind: ViolationKind::TimestampNotMonotonic,
                evidence: format!("{list_field}[].{leaf} decreases"),
            });
        }
    }
}

fn enum_membership(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    norm: &ObjectNorm,
    out: &mut Vec<SchemaViolation>,
) {
    let (Some(field), Some(vocab_name)) = (param_str(decl, "field"), param_str(decl, "vocabulary"))
    else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let Some(values) = norm.vocabulary(vocab_name) else {
        out.push(SchemaViolation {
            object_id: id.to_string(),
            pattern: decl.pattern.clone(),
            kind: ViolationKind::UnknownPattern,
            evidence: format!("vocabulary {vocab_name:?} not found in norm"),
        });
        return;
    };
    let targets: Vec<String> = match split_list_path(field) {
        Some((list_field, leaf)) => obj
            .get(list_field)
            .and_then(|e| e.as_sequence())
            .map(|evs| {
                evs.iter()
                    .filter_map(|e| e.get(leaf))
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        None => obj
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| vec![s.to_string()])
            .unwrap_or_default(),
    };
    for v in targets {
        if !values.iter().any(|allowed| allowed == &v) {
            out.push(SchemaViolation {
                object_id: id.to_string(),
                pattern: decl.pattern.clone(),
                kind: ViolationKind::ValueNotInVocabulary,
                evidence: format!("{field}={v:?} not in {vocab_name}"),
            });
        }
    }
}

fn graph_edges_known(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    norm: &ObjectNorm,
    out: &mut Vec<SchemaViolation>,
) {
    let (Some(graph_field), Some(edge_field), Some(node_vocab)) = (
        param_str(decl, "graph_field"),
        param_str(decl, "edge_field"),
        param_str(decl, "node_vocabulary"),
    ) else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let roots = param_list(decl, "allowed_roots");
    let Some(nodes) = norm.vocabulary(node_vocab) else {
        out.push(SchemaViolation {
            object_id: id.to_string(),
            pattern: decl.pattern.clone(),
            kind: ViolationKind::UnknownPattern,
            evidence: format!("vocabulary {node_vocab:?} not found in norm"),
        });
        return;
    };
    let Some(graph) = obj.get(graph_field).and_then(|g| g.as_mapping()) else {
        return;
    };
    for (node_id, entry) in graph {
        let node_name = node_id.as_str().unwrap_or("");
        if !node_name.is_empty() && !nodes.iter().any(|n| n == node_name) {
            out.push(SchemaViolation {
                object_id: id.to_string(),
                pattern: decl.pattern.clone(),
                kind: ViolationKind::EdgeTargetUnknown,
                evidence: format!("node {node_name:?} not in {node_vocab}"),
            });
        }
        if let Some(edges) = entry.get(edge_field).and_then(|e| e.as_sequence()) {
            for e in edges.iter().filter_map(|e| e.as_str()) {
                if !roots.iter().any(|r| r == e) && !nodes.iter().any(|n| n == e) {
                    out.push(SchemaViolation {
                        object_id: id.to_string(),
                        pattern: decl.pattern.clone(),
                        kind: ViolationKind::EdgeTargetUnknown,
                        evidence: format!("{node_name}.{edge_field} -> unknown {e:?}"),
                    });
                }
            }
        }
    }
}

fn cross_layer_stat_eq(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    out: &mut Vec<SchemaViolation>,
) {
    let (Some(layer_a), Some(stat_a), Some(layer_b), Some(stat_b)) = (
        param_str(decl, "layer_a"),
        param_str(decl, "stat_a"),
        param_str(decl, "layer_b"),
        param_str(decl, "stat_b"),
    ) else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let read = |layer: &str, stat: &str| -> Option<serde_yaml::Value> {
        obj.get("artifacts")
            .and_then(|a| a.get(layer))
            .and_then(|l| l.get("stats"))
            .and_then(|s| s.get(stat))
            .cloned()
    };
    if let (Some(va), Some(vb)) = (read(layer_a, stat_a), read(layer_b, stat_b))
        && va != vb
    {
        out.push(SchemaViolation {
            object_id: id.to_string(),
            pattern: decl.pattern.clone(),
            kind: ViolationKind::CrossLayerStatMismatch,
            evidence: format!("{layer_b}.{stat_b}={vb:?} != {layer_a}.{stat_a}={va:?}"),
        });
    }
}

fn collection_records(
    id: &str,
    decl: &InvariantDeclaration,
    io: &dyn FileBackedChecks,
    out: &mut Vec<SchemaViolation>,
) {
    let Some(field) = param_str(decl, "field") else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let required = param_bool(decl, "required");
    let min = param_i64(decl, "min").unwrap_or(i64::MIN);
    let Ok(Some(records)) = io.collection_records(id, decl) else {
        if required {
            out.push(SchemaViolation {
                object_id: id.to_string(),
                pattern: decl.pattern.clone(),
                kind: ViolationKind::ObjectFileMissing,
                evidence: format!("collection {:?} unreadable", param_str(decl, "file")),
            });
        }
        return;
    };
    let bad: Vec<String> = records
        .iter()
        .filter(|r| {
            let v = r.get(field);
            match v.and_then(|v| v.as_i64()) {
                Some(n) => n < min,
                None => required,
            }
        })
        .filter_map(|r| r.get("label").and_then(|l| l.as_str().map(str::to_string)))
        .collect();
    if !bad.is_empty() {
        out.push(SchemaViolation {
            object_id: id.to_string(),
            pattern: decl.pattern.clone(),
            kind: ViolationKind::RecordFieldInvalid,
            evidence: format!(
                "{} records with invalid {field}: {}",
                bad.len(),
                bad.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
            ),
        });
    }
}

fn artifact_files_exist(
    id: &str,
    obj: &serde_yaml::Value,
    decl: &InvariantDeclaration,
    io: &dyn FileBackedChecks,
    out: &mut Vec<SchemaViolation>,
) {
    let Some(graph_field) = param_str(decl, "graph_field") else {
        out.push(bad_params(id, &decl.pattern));
        return;
    };
    let Some(graph) = obj.get(graph_field).and_then(|g| g.as_mapping()) else {
        return;
    };
    for (node, entry) in graph {
        let Some(rel) = entry.get("file").and_then(|f| f.as_str()) else {
            continue;
        };
        match io.artifact_exists(id, rel) {
            Ok(true) => {}
            _ => out.push(SchemaViolation {
                object_id: id.to_string(),
                pattern: decl.pattern.clone(),
                kind: ViolationKind::ObjectFileMissing,
                evidence: format!(
                    "{}.file {rel:?} missing on disk",
                    node.as_str().unwrap_or("?")
                ),
            }),
        }
    }
}

fn bad_params(id: &str, pattern: &str) -> SchemaViolation {
    SchemaViolation {
        object_id: id.to_string(),
        pattern: pattern.to_string(),
        kind: ViolationKind::UnknownPattern,
        evidence: "declaration has bad/missing parameters".to_string(),
    }
}

/// "events[].field" -> ("events", "field"); plain fields -> None.
fn split_list_path(field: &str) -> Option<(&str, &str)> {
    let (list, leaf) = field.split_once("[]")?;
    let leaf = leaf.strip_prefix('.')?;
    Some((list, leaf))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &str = "lifecycle_order: [raw, parsed, structured, extracted]\nlayers:\n  - {id: source}\n  - {id: parsed}\n  - {id: context}\n  - {id: extracted}\ninvariants:\n  - pattern: derived_state_from_events\n    state_field: lifecycle\n    events_field: lifecycle_events\n    stage_field: stage\n    default_state: raw\n  - pattern: monotonic_timestamp\n    field: lifecycle_events[].recorded_at\n  - pattern: enum_membership\n    field: lifecycle_events[].stage\n    vocabulary: lifecycle_order\n  - pattern: graph_edges_known\n    graph_field: artifacts\n    edge_field: derived_from\n    node_vocabulary: layers\n    allowed_roots: [source]\n  - pattern: cross_layer_stat_eq\n    layer_a: parsed\n    stat_a: sections\n    layer_b: context\n    stat_b: ast_nodes\n";

    fn norm() -> ObjectNorm {
        ObjectNorm::from_yaml_str(SCHEMA).unwrap()
    }

    fn obj(yaml: &str) -> serde_yaml::Value {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn norm_parses_vocabularies_and_declarations() {
        let n = norm();
        assert_eq!(
            n.vocabulary("lifecycle_order").unwrap(),
            &["raw", "parsed", "structured", "extracted"]
        );
        assert!(n.vocabulary("layers").unwrap().contains(&"context".to_string()));
        assert_eq!(n.invariants.len(), 5);
    }

    #[test]
    fn clean_object_produces_no_violations() {
        let o = obj(
            "lifecycle: extracted\nlifecycle_events:\n  - {stage: parsed, recorded_at: '01'}\n  - {stage: structured, recorded_at: '02'}\n  - {stage: extracted, recorded_at: '03'}\nartifacts:\n  parsed: {file: parsed/article.json, derived_from: [source], stats: {sections: 3}}\n  context: {file: context/projection.json, derived_from: [parsed], stats: {ast_nodes: 3}}\n",
        );
        assert!(validate_object("p1", &o, &norm()).is_empty());
    }

    #[test]
    fn derived_state_mismatch_detected() {
        let o = obj("lifecycle: validated\nlifecycle_events:\n  - {stage: parsed, recorded_at: '01'}\n");
        let vs = validate_object("p2", &o, &norm());
        assert_eq!(vs[0].kind, ViolationKind::DerivedStateMismatch);
    }

    #[test]
    fn empty_events_default_state() {
        let o = obj("lifecycle: raw\nlifecycle_events: []\n");
        assert!(validate_object("p", &o, &norm()).is_empty());
    }

    #[test]
    fn non_monotonic_detected() {
        let o = obj(
            "lifecycle: structured\nlifecycle_events:\n  - {stage: parsed, recorded_at: '03'}\n  - {stage: structured, recorded_at: '01'}\n",
        );
        let vs = validate_object("p3", &o, &norm());
        assert!(vs.iter().any(|v| v.kind == ViolationKind::TimestampNotMonotonic));
    }

    #[test]
    fn enum_membership_detected() {
        let o = obj(
            "lifecycle: teleported\nlifecycle_events:\n  - {stage: teleported, recorded_at: '01'}\n",
        );
        let vs = validate_object("p4", &o, &norm());
        // stage not in lifecycle_order; derived_state PASSES because
        // lifecycle matches the (invalid) last event — enum catches it.
        assert!(vs.iter().any(|v| v.kind == ViolationKind::ValueNotInVocabulary));
        assert!(!vs.iter().any(|v| v.kind == ViolationKind::DerivedStateMismatch));
    }

    #[test]
    fn unknown_edge_and_node_detected() {
        let o = obj(
            "lifecycle: parsed\nlifecycle_events:\n  - {stage: parsed, recorded_at: '01'}\nartifacts:\n  warp: {file: warp/x.json, derived_from: [antimatter]}\n",
        );
        let vs = validate_object("p5", &o, &norm());
        let evidences: Vec<_> = vs.iter().map(|v| v.evidence.clone()).collect();
        assert!(evidences.iter().any(|e| e.contains("warp")));
        assert!(evidences.iter().any(|e| e.contains("antimatter")));
    }

    #[test]
    fn cross_layer_mismatch_detected() {
        let o = obj(
            "lifecycle: structured\nlifecycle_events:\n  - {stage: structured, recorded_at: '01'}\nartifacts:\n  parsed: {derived_from: [source], stats: {sections: 3}}\n  context: {derived_from: [parsed], stats: {ast_nodes: 9}}\n",
        );
        let vs = validate_object("p6", &o, &norm());
        assert!(vs.iter().any(|v| v.kind == ViolationKind::CrossLayerStatMismatch));
    }

    struct FakeIo {
        records: Option<Vec<serde_yaml::Value>>,
        exists: bool,
    }
    impl FileBackedChecks for FakeIo {
        fn collection_records(
            &self,
            _id: &str,
            _decl: &InvariantDeclaration,
        ) -> Result<Option<Vec<serde_yaml::Value>>, String> {
            Ok(self.records.clone())
        }
        fn artifact_exists(&self, _id: &str, _rel: &str) -> Result<bool, String> {
            Ok(self.exists)
        }
    }

    #[test]
    fn collection_records_bad_field_detected() {
        let norm = ObjectNorm::from_yaml_str(
            "invariants:\n  - pattern: collection_records\n    layer: extracted\n    file: extracted/entities.json\n    field: section_ordinal\n    required: true\n    min: 0\n",
        )
        .unwrap();
        let io = FakeIo {
            records: Some(vec![
                serde_yaml::from_str("{label: good, section_ordinal: 0}").unwrap(),
                serde_yaml::from_str("{label: orphan}").unwrap(),
                serde_yaml::from_str("{label: negative, section_ordinal: -1}").unwrap(),
            ]),
            exists: true,
        };
        let vs = validate_file_backed("p7", &serde_yaml::Value::Null, &norm, &io);
        assert_eq!(vs.len(), 1);
        assert_eq!(vs[0].kind, ViolationKind::RecordFieldInvalid);
        assert!(vs[0].evidence.contains("orphan"));
        assert!(vs[0].evidence.contains("negative"));
    }

    const NAV_YAML: &str = "lifecycle_order: [raw, parsed, structured]\nlayers:\n  - {id: parsed}\ninvariants:\n  - pattern: node_ref_integrity\n    list_field: navigation\n    id_field: section_id\n    ref_fields: [parent_section_id, next_section_id, prev_section_id]\n  - pattern: list_symmetric_next_prev\n    list_field: navigation\n    id_field: section_id\n";

    #[test]
    fn node_ref_integrity_catches_dangling_parent() {
        let n = ObjectNorm::from_yaml_str(NAV_YAML).unwrap();
        let o = obj(
            "navigation:\n  - {section_id: a, parent_section_id: ghost, next_section_id: b, prev_section_id: null}\n  - {section_id: b, parent_section_id: a, next_section_id: null, prev_section_id: a}\n",
        );
        let vs = validate_object("p", &o, &n);
        assert!(vs.iter().any(|v| v.kind == ViolationKind::EdgeTargetUnknown
            && v.evidence.contains("ghost")));
    }

    #[test]
    fn next_prev_symmetry_checked() {
        let n = ObjectNorm::from_yaml_str(NAV_YAML).unwrap();
        // broken: next(a)=b but prev(b)=null
        let o = obj(
            "navigation:\n  - {section_id: a, parent_section_id: null, next_section_id: b, prev_section_id: null}\n  - {section_id: b, parent_section_id: a, next_section_id: null, prev_section_id: null}\n",
        );
        let vs = validate_object("p", &o, &n);
        assert!(vs.iter().any(|v| v.evidence.contains("prev")));
        // consistent pair passes
        let ok = obj(
            "navigation:\n  - {section_id: a, parent_section_id: null, next_section_id: b, prev_section_id: null}\n  - {section_id: b, parent_section_id: a, next_section_id: null, prev_section_id: a}\n",
        );
        assert!(validate_object("p", &ok, &n).is_empty());
    }

    #[test]
    fn unknown_pattern_reported_loudly() {
        let norm = ObjectNorm::from_yaml_str("invariants:\n  - pattern: quantum_entangle\n").unwrap();
        let vs = validate_object("p8", &serde_yaml::Value::Null, &norm);
        assert_eq!(vs[0].kind, ViolationKind::UnknownPattern);
    }

    #[test]
    fn split_list_path_parses_and_rejects() {
        assert_eq!(split_list_path("events[].ts"), Some(("events", "ts")));
        assert_eq!(split_list_path("events.ts"), None);
        assert_eq!(split_list_path("events[].a.b"), Some(("events", "a.b")));
    }
}
