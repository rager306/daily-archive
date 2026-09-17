//! Independent recomputation contract for the segmentation oracle digests.
//!
//! GAR-011 / R131 / ADR-051 / ADR-076 / D273 (MEM885). This integration test
//! shares no code with the crate under test: it re-reads the DATA contract
//! `scripts/governor/checks/oracle_digest_recomputation.yaml`, the reviewed
//! fixture `crates/da-parse/tests/fixtures/segmentation_oracle.yaml`, the
//! canonical artifact
//! `artifacts/parse_compat/document_segmentation_oracle_source.json` and the
//! declaration `data/parse/document_segmentation.yaml` *from disk at runtime*
//! (`CARGO_MANIFEST_DIR/../..`, never a compile-time string include, so the test compiles
//! against the tree it is actually run in), and it performs the SHA-256
//! mathematics itself.
//!
//! Why this exists: before GAR-011 the oracle digests were only checked for
//! *shape* (64 chars, lower-hex, not all zeros). Substituting the canonical
//! bytes while leaving the digest string unchanged passed silently. This reader
//! recomputes every `recomputed` digest from the canonical bytes, enforces a
//! discovery closure over the audited surfaces (every digest-like field claimed
//! exactly once, every claim resolvable), refuses opaque markers as provenance
//! and refuses placeholder values.
//!
//! Two properties matter for the contract to be meaningful:
//!
//!   * the digest is *never* derived from an already declared digest (that
//!     would be a tautology) — it is always recomputed from the bytes;
//!   * the mutation matrix in this file is the executable counterpart of the
//!     slice Demo: mutate the canonical source -> the digest stops matching and
//!     the check names the offending field.

use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Governor DATA contract for this check (repo-relative).
const CONTRACT_REL: &str = "scripts/governor/checks/oracle_digest_recomputation.yaml";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

// ---------------------------------------------------------------------------
// Document model
// ---------------------------------------------------------------------------

/// Format-neutral document node. Both the YAML fixture/declaration and the JSON
/// artifact are normalised into this model so the pointer engine, the discovery
/// walk and the mutation matrix have exactly one implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Map(Vec<(String, Node)>),
    Seq(Vec<Node>),
    Str(String),
    /// A scalar that is not a string (number, bool, null), rendered for the
    /// diagnostic. A digest field must be a `Str`; anything else is reported.
    NonStr(String),
}

impl Node {
    fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut Node> {
        match self {
            Node::Map(entries) => entries
                .iter_mut()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Node::Str(value) => Some(value),
            _ => None,
        }
    }

    fn as_seq(&self) -> Option<&[Node]> {
        match self {
            Node::Seq(items) => Some(items),
            _ => None,
        }
    }
}

fn yaml_to_node(value: &YamlValue) -> Node {
    match value {
        YamlValue::Null => Node::NonStr("null".to_string()),
        YamlValue::Bool(flag) => Node::NonStr(flag.to_string()),
        YamlValue::Number(number) => Node::NonStr(number.to_string()),
        YamlValue::String(text) => Node::Str(text.clone()),
        YamlValue::Sequence(items) => Node::Seq(items.iter().map(yaml_to_node).collect()),
        YamlValue::Mapping(entries) => Node::Map(
            entries
                .iter()
                .map(|(key, value)| (yaml_key(key), yaml_to_node(value)))
                .collect(),
        ),
        other => Node::NonStr(format!("{other:?}")),
    }
}

fn yaml_key(key: &YamlValue) -> String {
    match key {
        YamlValue::String(text) => text.clone(),
        other => format!("{other:?}"),
    }
}

fn json_to_node(value: &JsonValue) -> Node {
    match value {
        JsonValue::Null => Node::NonStr("null".to_string()),
        JsonValue::Bool(flag) => Node::NonStr(flag.to_string()),
        JsonValue::Number(number) => Node::NonStr(number.to_string()),
        JsonValue::String(text) => Node::Str(text.clone()),
        JsonValue::Array(items) => Node::Seq(items.iter().map(json_to_node).collect()),
        JsonValue::Object(entries) => Node::Map(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), json_to_node(value)))
                .collect(),
        ),
    }
}

/// Reads a file the contract requires. A missing or unreadable audited file
/// fails closed and names the absolute path: a silently skipped surface would
/// turn the recomputation contract into a no-op.
fn read_required(root: &Path, relative: &str) -> Vec<u8> {
    let absolute = root.join(relative);
    fs::read(&absolute)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", absolute.display()))
}

fn parse_yaml(bytes: &[u8], relative: &str) -> Node {
    let value: YamlValue = serde_yaml::from_slice(bytes)
        .unwrap_or_else(|error| panic!("cannot parse {relative} as YAML: {error}"));
    yaml_to_node(&value)
}

fn parse_json(bytes: &[u8], relative: &str) -> Node {
    let value: JsonValue = serde_json::from_slice(bytes)
        .unwrap_or_else(|error| panic!("cannot parse {relative} as JSON: {error}"));
    json_to_node(&value)
}

/// Sorts map entries recursively so two documents can be compared for semantic
/// equality regardless of key order.
fn canonicalize(node: &Node) -> Node {
    match node {
        Node::Map(entries) => {
            let mut sorted: Vec<(String, Node)> = entries
                .iter()
                .map(|(key, value)| (key.clone(), canonicalize(value)))
                .collect();
            sorted.sort_by(|left, right| left.0.cmp(&right.0));
            Node::Map(sorted)
        }
        Node::Seq(items) => Node::Seq(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// Pointer engine
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Seg {
    Key(String),
    Idx(usize),
    Wild,
}

/// Parses `""` (document root) or `/a/b/*`. `None` for a malformed pointer.
fn parse_pointer(raw: &str) -> Option<Vec<Seg>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }
    if !trimmed.starts_with('/') {
        return None;
    }
    let mut out = Vec::new();
    for part in trimmed.trim_start_matches('/').split('/') {
        if part.is_empty() {
            return None;
        }
        if part == "*" {
            out.push(Seg::Wild);
        } else if let Ok(index) = part.parse::<usize>() {
            out.push(Seg::Idx(index));
        } else {
            out.push(Seg::Key(part.to_string()));
        }
    }
    Some(out)
}

fn render_path(path: &[Seg]) -> String {
    let mut out = String::new();
    for seg in path {
        out.push('/');
        match seg {
            Seg::Key(key) => out.push_str(key),
            Seg::Idx(index) => out.push_str(&index.to_string()),
            Seg::Wild => out.push('*'),
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

fn resolve<'a>(doc: &'a Node, pattern: &[Seg]) -> Vec<(Vec<Seg>, &'a Node)> {
    let mut out = Vec::new();
    let mut path = Vec::new();
    resolve_into(doc, pattern, &mut path, &mut out);
    out
}

fn resolve_into<'a>(
    doc: &'a Node,
    pattern: &[Seg],
    path: &mut Vec<Seg>,
    out: &mut Vec<(Vec<Seg>, &'a Node)>,
) {
    match pattern.split_first() {
        None => out.push((path.clone(), doc)),
        Some((seg, rest)) => match seg {
            Seg::Key(key) => {
                if let Node::Map(entries) = doc {
                    for (candidate, value) in entries {
                        if candidate == key {
                            path.push(Seg::Key(candidate.clone()));
                            resolve_into(value, rest, path, out);
                            path.pop();
                        }
                    }
                }
            }
            Seg::Idx(index) => {
                if let Node::Seq(items) = doc
                    && let Some(value) = items.get(*index)
                {
                    path.push(Seg::Idx(*index));
                    resolve_into(value, rest, path, out);
                    path.pop();
                }
            }
            Seg::Wild => match doc {
                Node::Seq(items) => {
                    for (index, value) in items.iter().enumerate() {
                        path.push(Seg::Idx(index));
                        resolve_into(value, rest, path, out);
                        path.pop();
                    }
                }
                Node::Map(entries) => {
                    for (key, value) in entries {
                        path.push(Seg::Key(key.clone()));
                        resolve_into(value, rest, path, out);
                        path.pop();
                    }
                }
                _ => {}
            },
        },
    }
}

fn resolve_one<'a>(doc: &'a Node, path: &[Seg]) -> Option<&'a Node> {
    match path.split_first() {
        None => Some(doc),
        Some((Seg::Key(key), rest)) => match doc {
            Node::Map(entries) => {
                let (_, child) = entries.iter().find(|(candidate, _)| candidate == key)?;
                resolve_one(child, rest)
            }
            _ => None,
        },
        Some((Seg::Idx(index), rest)) => match doc {
            Node::Seq(items) => {
                let child = items.get(*index)?;
                resolve_one(child, rest)
            }
            _ => None,
        },
        Some((Seg::Wild, _)) => None,
    }
}

fn resolve_one_mut<'a>(doc: &'a mut Node, path: &[Seg]) -> Option<&'a mut Node> {
    match path.split_first() {
        None => Some(doc),
        Some((Seg::Key(key), rest)) => match doc {
            Node::Map(entries) => {
                let (_, child) = entries.iter_mut().find(|(candidate, _)| candidate == key)?;
                resolve_one_mut(child, rest)
            }
            _ => None,
        },
        Some((Seg::Idx(index), rest)) => match doc {
            Node::Seq(items) => {
                let child = items.get_mut(*index)?;
                resolve_one_mut(child, rest)
            }
            _ => None,
        },
        Some((Seg::Wild, _)) => None,
    }
}

fn matches_pattern(pattern: &[Seg], concrete: &[Seg]) -> bool {
    pattern.len() == concrete.len()
        && pattern
            .iter()
            .zip(concrete)
            .all(|(expected, actual)| match expected {
                Seg::Wild => true,
                Seg::Key(key) => matches!(actual, Seg::Key(candidate) if candidate == key),
                Seg::Idx(index) => matches!(actual, Seg::Idx(candidate) if candidate == index),
            })
}

/// Key dialect of `digest_field_patterns`. Supported (documented in the DATA
/// contract): `suffix$`, `^prefix`, `^exact$`, or a bare literal. No regex
/// engine is pulled in, so the DATA stays literal and auditable.
fn key_matches(pattern: &str, key: &str) -> bool {
    if let Some(rest) = pattern.strip_prefix('^') {
        if let Some(exact) = rest.strip_suffix('$') {
            return key == exact;
        }
        return key.starts_with(rest);
    }
    if let Some(suffix) = pattern.strip_suffix('$') {
        return key.ends_with(suffix);
    }
    key.contains(pattern)
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct SurfaceSpec {
    key: String,
    path: String,
    format: String,
    audit_pointers: Vec<String>,
}

#[derive(Clone, Debug)]
struct World {
    contract: Node,
    docs: BTreeMap<String, Node>,
    paths: BTreeMap<String, String>,
    /// Raw file bytes per surface key. Needed by whole-file refs
    /// (`exact_file_bytes_sha256`), which hash the file itself rather than a
    /// canonical `bytes` string.
    raw_bytes: BTreeMap<String, Vec<u8>>,
}

fn surface_specs(contract: &Node) -> Vec<SurfaceSpec> {
    contract
        .get("audited_surfaces")
        .and_then(|node| node.as_seq())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(SurfaceSpec {
                        key: item.get("key")?.as_str()?.to_string(),
                        path: item.get("path")?.as_str()?.to_string(),
                        format: item
                            .get("format")
                            .and_then(|node| node.as_str())
                            .unwrap_or("yaml")
                            .to_string(),
                        audit_pointers: item
                            .get("audit_pointers")
                            .and_then(|node| node.as_seq())
                            .map(|pointers| {
                                pointers
                                    .iter()
                                    .filter_map(|pointer| pointer.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn load_world() -> World {
    let root = repo_root();
    let contract_bytes = read_required(&root, CONTRACT_REL);
    let contract = parse_yaml(&contract_bytes, CONTRACT_REL);

    let mut docs = BTreeMap::new();
    let mut paths = BTreeMap::new();
    let mut raw_bytes = BTreeMap::new();
    for surface in surface_specs(&contract) {
        let bytes = read_required(&root, &surface.path);
        let node = if surface.format == "json" {
            parse_json(&bytes, &surface.path)
        } else {
            parse_yaml(&bytes, &surface.path)
        };
        raw_bytes.insert(surface.key.clone(), bytes);
        docs.insert(surface.key.clone(), node);
        paths.insert(surface.key.clone(), surface.path.clone());
    }

    World {
        contract,
        docs,
        paths,
        raw_bytes,
    }
}

impl World {
    fn audit_doc(&self, key: &str) -> &Node {
        self.docs
            .get(key)
            .unwrap_or_else(|| panic!("no audited surface `{key}`"))
    }

    fn digest_fields(&self) -> &[Node] {
        self.contract
            .get("digest_fields")
            .and_then(|node| node.as_seq())
            .expect("DATA contract must declare digest_fields")
    }

    /// Mutable handle on the mutable declaration list, for the negative matrix.
    fn digest_fields_mut(&mut self) -> &mut Vec<Node> {
        match self.contract.get_mut("digest_fields") {
            Some(Node::Seq(items)) => items,
            _ => panic!("digest_fields must be a sequence"),
        }
    }
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Finding {
    label: String,
    surface: String,
    pointer: String,
    field_id: String,
    detail: String,
}

impl Finding {
    fn new(
        label: &str,
        surface: &str,
        pointer: &str,
        field_id: &str,
        detail: impl Into<String>,
    ) -> Finding {
        Finding {
            label: label.to_string(),
            surface: surface.to_string(),
            pointer: pointer.to_string(),
            field_id: field_id.to_string(),
            detail: detail.into(),
        }
    }

    fn render(&self) -> String {
        format!(
            "[{}] surface={} pointer={} field_id={} :: {}",
            self.label, self.surface, self.pointer, self.field_id, self.detail
        )
    }
}

fn render_findings(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "<none>".to_string();
    }
    findings
        .iter()
        .map(|finding| format!("  - {}", finding.render()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn string_list(node: &Node, key: &str) -> Vec<String> {
    node.get(key)
        .and_then(|item| item.as_seq())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Plan: vocabulary, surfaces, declared records, discovered fields
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Vocabulary {
    classes: Vec<String>,
    canonicalizations: Vec<String>,
    opaque_reasons: Vec<String>,
    alignments: Vec<String>,
    patterns: Vec<String>,
    normalization: String,
    oracle_violations: Vec<String>,
    reader_violations: Vec<String>,
}

#[derive(Clone, Debug)]
struct Declared {
    field_id: String,
    surface: String,
    pointer_raw: String,
    path: Option<Vec<Seg>>,
    class: String,
    canonical_ref: Option<String>,
    alignment: Option<String>,
    canonicalization: Option<String>,
}

#[derive(Clone, Debug)]
struct Discovered {
    surface: String,
    path: Vec<Seg>,
    rendered: String,
    value: Option<String>,
}

struct Plan {
    vocab: Vocabulary,
    surfaces: Vec<SurfaceSpec>,
    declared: Vec<Declared>,
    discovered: Vec<Discovered>,
    /// For each declared record, the indices of the discovered fields it claims.
    hits: Vec<Vec<usize>>,
    /// For each discovered field, the indices of the declared records claiming it.
    claims: Vec<Vec<usize>>,
}

fn build_plan(world: &World) -> (Plan, Vec<Finding>) {
    let contract = &world.contract;
    let vocab = Vocabulary {
        classes: string_list(contract, "classes"),
        canonicalizations: string_list(contract, "canonicalizations"),
        opaque_reasons: string_list(contract, "opaque_reasons"),
        alignments: string_list(contract, "alignments"),
        patterns: string_list(contract, "digest_field_patterns"),
        normalization: contract
            .get("digest_value_normalization")
            .and_then(|node| node.as_str())
            .unwrap_or_default()
            .to_string(),
        oracle_violations: string_list(contract, "violations"),
        reader_violations: string_list(contract, "reader_violations"),
    };

    let mut findings = Vec::new();
    for (key, values) in [
        ("classes", &vocab.classes),
        ("canonicalizations", &vocab.canonicalizations),
        ("opaque_reasons", &vocab.opaque_reasons),
        ("alignments", &vocab.alignments),
        ("digest_field_patterns", &vocab.patterns),
    ] {
        if values.is_empty() {
            findings.push(Finding::new(
                "missing_closed_vocabulary",
                "contract",
                &format!("/{key}"),
                key,
                format!("closed list `{key}` is missing or empty (fail-closed)"),
            ));
        }
    }
    if vocab.normalization.as_str() != "strip_optional_sha256_prefix" {
        findings.push(Finding::new(
            "missing_closed_vocabulary",
            "contract",
            "/digest_value_normalization",
            "digest_value_normalization",
            format!(
                "unsupported digest value normalization `{}`",
                vocab.normalization
            ),
        ));
    }

    let surfaces = surface_specs(contract);

    // --- discovery, bounded by the declared audit pointers -----------------
    let mut discovered: Vec<Discovered> = Vec::new();
    for surface in &surfaces {
        let doc = world.audit_doc(&surface.key);
        for pointer in &surface.audit_pointers {
            let Some(pattern) = parse_pointer(pointer) else {
                findings.push(Finding::new(
                    "declared_field_not_found",
                    &surface.key,
                    pointer,
                    &surface.key,
                    "audit pointer is not a valid pointer",
                ));
                continue;
            };
            for (path, node) in resolve(doc, &pattern) {
                let mut walking = path.clone();
                collect_digest_fields(
                    node,
                    &mut walking,
                    &vocab.patterns,
                    &surface.key,
                    &mut discovered,
                );
            }
        }
    }

    // --- declared records, with vocabulary validation ----------------------
    let mut declared: Vec<Declared> = Vec::new();
    let mut declared_index: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (index, record) in world.digest_fields().iter().enumerate() {
        let field_id = record
            .get("field_id")
            .and_then(|node| node.as_str())
            .unwrap_or_default()
            .to_string();
        let surface = record
            .get("surface")
            .and_then(|node| node.as_str())
            .unwrap_or_default()
            .to_string();
        let pointer_raw = record
            .get("pointer")
            .and_then(|node| node.as_str())
            .unwrap_or_default()
            .to_string();
        let class = record
            .get("class")
            .and_then(|node| node.as_str())
            .unwrap_or_default()
            .to_string();
        let placeholder_id = format!("digest_fields[{index}]");

        if field_id.is_empty() || surface.is_empty() {
            findings.push(Finding::new(
                "missing_field_identity",
                &surface,
                &pointer_raw,
                &placeholder_id,
                "record has no field_id/surface identity",
            ));
            continue;
        }
        if class.is_empty() {
            findings.push(Finding::new(
                "digest_without_class",
                &surface,
                &pointer_raw,
                &field_id,
                "record declares no class",
            ));
        } else if !vocab.classes.contains(&class) {
            findings.push(Finding::new(
                "class_outside_vocabulary",
                &surface,
                &pointer_raw,
                &field_id,
                format!("class `{class}` is outside {:?}", vocab.classes),
            ));
        }

        let path = parse_pointer(&pointer_raw);
        if path.is_none() {
            findings.push(Finding::new(
                "declared_field_not_found",
                &surface,
                &pointer_raw,
                &field_id,
                "pointer is not a valid pointer",
            ));
        }

        let canonical_ref = record
            .get("ref")
            .and_then(|node| node.as_str())
            .map(str::to_string);
        let alignment = record
            .get("alignment")
            .and_then(|node| node.as_str())
            .map(str::to_string);
        let canonicalization = record
            .get("canonicalization")
            .and_then(|node| node.as_str())
            .map(str::to_string);
        let opaque_reason = record
            .get("opaque_reason")
            .and_then(|node| node.as_str())
            .map(str::to_string);

        if class == "recomputed" {
            if canonical_ref.is_none() {
                findings.push(Finding::new(
                    "recomputed_without_canonical_ref",
                    &surface,
                    &pointer_raw,
                    &field_id,
                    "recomputed digest must name the canonical bytes it is computed from",
                ));
            }
            match canonicalization.as_deref() {
                None => findings.push(Finding::new(
                    "recomputed_without_canonicalization",
                    &surface,
                    &pointer_raw,
                    &field_id,
                    "recomputed digest must name its canonicalization",
                )),
                Some(value) if !vocab.canonicalizations.iter().any(|c| c == value) => findings
                    .push(Finding::new(
                        "canonicalization_outside_contract",
                        &surface,
                        &pointer_raw,
                        &field_id,
                        format!(
                            "canonicalization `{value}` is outside {:?}",
                            vocab.canonicalizations
                        ),
                    )),
                Some(_) => {}
            }
            match alignment.as_deref() {
                Some(value) if vocab.alignments.iter().any(|a| a == value) => {}
                Some(value) => findings.push(Finding::new(
                    "alignment_outside_vocabulary",
                    &surface,
                    &pointer_raw,
                    &field_id,
                    format!("alignment `{value}` is outside {:?}", vocab.alignments),
                )),
                None => findings.push(Finding::new(
                    "alignment_outside_vocabulary",
                    &surface,
                    &pointer_raw,
                    &field_id,
                    "recomputed digest must name how its ref wildcards are bound",
                )),
            }
        } else if class == "opaque" {
            match opaque_reason.as_deref() {
                None => findings.push(Finding::new(
                    "opaque_without_reason",
                    &surface,
                    &pointer_raw,
                    &field_id,
                    "opaque marker must name its reason",
                )),
                Some(value) if !vocab.opaque_reasons.iter().any(|r| r == value) => {
                    findings.push(Finding::new(
                        "opaque_reason_outside_vocabulary",
                        &surface,
                        &pointer_raw,
                        &field_id,
                        format!(
                            "opaque reason `{value}` is outside {:?}",
                            vocab.opaque_reasons
                        ),
                    ))
                }
                Some(_) => {}
            }
        }

        if let Some(previous) = declared_index.insert((surface.clone(), pointer_raw.clone()), index)
        {
            findings.push(Finding::new(
                "duplicate_field_binding",
                &surface,
                &pointer_raw,
                &field_id,
                format!("digest_fields[{previous}] already binds this surface pointer"),
            ));
        }

        declared.push(Declared {
            field_id,
            surface,
            pointer_raw,
            path,
            class,
            canonical_ref,
            alignment,
            canonicalization,
        });
    }

    // --- closure: every claim resolves, every discovery is claimed exactly once
    let hits: Vec<Vec<usize>> = declared
        .iter()
        .map(|record| {
            discovered
                .iter()
                .enumerate()
                .filter(|(_, field)| {
                    field.surface == record.surface
                        && record
                            .path
                            .as_ref()
                            .map(|pattern| matches_pattern(pattern, &field.path))
                            .unwrap_or(false)
                })
                .map(|(index, _)| index)
                .collect()
        })
        .collect();

    for (index, record) in declared.iter().enumerate() {
        if record.path.is_some() && hits[index].is_empty() {
            findings.push(Finding::new(
                "declared_field_not_found",
                &record.surface,
                &record.pointer_raw,
                &record.field_id,
                "declared digest field does not resolve to any audited value",
            ));
        }
    }

    let claims: Vec<Vec<usize>> = discovered
        .iter()
        .map(|field| {
            declared
                .iter()
                .enumerate()
                .filter(|(_, record)| {
                    field.surface == record.surface
                        && record
                            .path
                            .as_ref()
                            .map(|pattern| matches_pattern(pattern, &field.path))
                            .unwrap_or(false)
                })
                .map(|(index, _)| index)
                .collect()
        })
        .collect();

    for (index, field) in discovered.iter().enumerate() {
        match claims[index].len() {
            0 => findings.push(Finding::new(
                "unclassified_digest_field",
                &field.surface,
                &field.rendered,
                &format!("{}{}", field.surface, field.rendered),
                "digest-like field is not claimed by any digest_fields record",
            )),
            1 => {}
            _ => findings.push(Finding::new(
                "duplicate_field_binding",
                &field.surface,
                &field.rendered,
                &format!("{}{}", field.surface, field.rendered),
                format!("claimed by {} digest_fields records", claims[index].len()),
            )),
        }
    }

    (
        Plan {
            vocab,
            surfaces,
            declared,
            discovered,
            hits,
            claims,
        },
        findings,
    )
}

fn collect_digest_fields(
    node: &Node,
    path: &mut Vec<Seg>,
    patterns: &[String],
    surface: &str,
    out: &mut Vec<Discovered>,
) {
    match node {
        Node::Map(entries) => {
            for (key, value) in entries {
                if patterns.iter().any(|pattern| key_matches(pattern, key)) {
                    let mut field_path = path.clone();
                    field_path.push(Seg::Key(key.clone()));
                    out.push(Discovered {
                        surface: surface.to_string(),
                        rendered: render_path(&field_path),
                        path: field_path,
                        value: value.as_str().map(str::to_string),
                    });
                }
                path.push(Seg::Key(key.clone()));
                collect_digest_fields(value, path, patterns, surface, out);
                path.pop();
            }
        }
        Node::Seq(items) => {
            for (index, value) in items.iter().enumerate() {
                path.push(Seg::Idx(index));
                collect_digest_fields(value, path, patterns, surface, out);
                path.pop();
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------

fn digest_normalize(value: &str, normalization: &str) -> String {
    match normalization {
        "strip_optional_sha256_prefix" => value
            .strip_prefix("sha256:")
            .unwrap_or(value)
            .to_ascii_lowercase(),
        _ => value.to_string(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Repeated-nibble (`5555…`) or repeated-pair (`5656…`) values are the exact
/// shape of the 45 placeholders GAR-011 removed; they read as digests while
/// carrying no provenance, so they must never be able to return.
fn is_placeholder_digest(value: &str) -> bool {
    let hexish = value.strip_prefix("sha256:").unwrap_or(value);
    let bytes = hexish.as_bytes();
    if bytes.len() < 2 {
        return false;
    }
    if bytes.iter().all(|byte| *byte == bytes[0]) {
        return true;
    }
    if bytes.len().is_multiple_of(2) {
        let unit = &bytes[..2];
        if bytes.chunks(2).all(|chunk| chunk == unit) {
            return true;
        }
    }
    false
}

fn split_ref(raw: &str) -> Option<(String, String)> {
    raw.split_once(':')
        .map(|(surface, pointer)| (surface.to_string(), pointer.to_string()))
}

fn wildcard_bindings(pattern: &[Seg], concrete: &[Seg]) -> Vec<Seg> {
    pattern
        .iter()
        .zip(concrete)
        .filter(|(expected, _)| matches!(expected, Seg::Wild))
        .map(|(_, actual)| actual.clone())
        .collect()
}

/// Deepest map on the concrete field path that carries a `region` key. The
/// fixture's region runs are the only such nodes, and this is what makes
/// `region_key` alignment possible: `canonical_boundary` holds five present
/// regions while `region_runs` holds nine (four typed absence), so positional
/// matching would silently mis-bind.
fn region_value(field_doc: &Node, pattern: &[Seg], concrete: &[Seg]) -> Option<String> {
    let mut walked: Vec<Seg> = Vec::new();
    for (expected, actual) in pattern.iter().zip(concrete) {
        walked.push(actual.clone());
        if matches!(expected, Seg::Wild)
            && let Some(node) = resolve_one(field_doc, &walked)
            && let Some(region) = node.get("region").and_then(|node| node.as_str())
        {
            return Some(region.to_string());
        }
    }
    None
}

fn align_ref_path(
    field_doc: &Node,
    ref_doc: &Node,
    field_pattern: &[Seg],
    field_concrete: &[Seg],
    ref_pattern: &[Seg],
    alignment: &str,
) -> Option<Vec<Seg>> {
    let bindings = wildcard_bindings(field_pattern, field_concrete);
    let ref_wildcards: Vec<usize> = ref_pattern
        .iter()
        .enumerate()
        .filter(|(_, seg)| matches!(seg, Seg::Wild))
        .map(|(index, _)| index)
        .collect();
    let region = if alignment == "region_key" {
        region_value(field_doc, field_pattern, field_concrete)
    } else {
        None
    };

    let mut out: Vec<Seg> = Vec::new();
    for (position, seg) in ref_pattern.iter().enumerate() {
        match seg {
            Seg::Wild => {
                let is_last = ref_wildcards.last() == Some(&position);
                if alignment == "region_key" && is_last {
                    let expected_region = region.clone()?;
                    let container = resolve_one(ref_doc, &out)?;
                    let items = container.as_seq()?;
                    let index = items.iter().position(|item| {
                        item.get("region").and_then(|node| node.as_str())
                            == Some(expected_region.as_str())
                    })?;
                    out.push(Seg::Idx(index));
                } else {
                    let which = ref_wildcards.iter().position(|item| *item == position)?;
                    out.push(bindings.get(which)?.clone());
                }
            }
            other => out.push(other.clone()),
        }
    }
    Some(out)
}

fn opaque_paths(plan: &Plan) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (index, record) in plan.declared.iter().enumerate() {
        if record.class != "opaque" {
            continue;
        }
        for hit in &plan.hits[index] {
            out.insert(format!(
                "{}|{}",
                plan.discovered[*hit].surface, plan.discovered[*hit].rendered
            ));
        }
    }
    out
}

/// Recomputes every `recomputed` digest from the canonical bytes. This is the
/// core of the contract: the digest is recomputed, never copied from a declared
/// digest.
fn check_recomputation(world: &World, plan: &Plan) -> Vec<Finding> {
    let mut findings = Vec::new();
    let opaque = opaque_paths(plan);

    for (index, record) in plan.declared.iter().enumerate() {
        if record.class != "recomputed" {
            continue;
        }
        let Some(ref_raw) = record.canonical_ref.as_deref() else {
            continue;
        };
        let Some((ref_surface_raw, ref_pointer_raw)) = split_ref(ref_raw) else {
            continue;
        };
        let Some(ref_pattern) = parse_pointer(&ref_pointer_raw) else {
            continue;
        };
        let ref_surface = if ref_surface_raw == "self" {
            record.surface.clone()
        } else {
            ref_surface_raw
        };
        let (Some(ref_doc), Some(field_doc)) = (
            world.docs.get(&ref_surface),
            world.docs.get(&record.surface),
        ) else {
            continue;
        };
        let Some(field_pattern) = record.path.as_ref() else {
            continue;
        };
        let canonicalization = record.canonicalization.clone().unwrap_or_default();
        let alignment = record.alignment.clone().unwrap_or_default();

        for hit_index in &plan.hits[index] {
            let hit = &plan.discovered[*hit_index];
            let Some(concrete_ref) = align_ref_path(
                field_doc,
                ref_doc,
                field_pattern,
                &hit.path,
                &ref_pattern,
                &alignment,
            ) else {
                findings.push(Finding::new(
                    "declared_field_not_found",
                    &record.surface,
                    &record.pointer_raw,
                    &record.field_id,
                    format!("cannot align ref `{ref_raw}` for {}", hit.rendered),
                ));
                continue;
            };

            // An opaque marker may never back a digest claim.
            let target_field_path = concrete_ref
                .split_last()
                .filter(|(last, _)| matches!(last, Seg::Key(key) if key == "bytes"))
                .map(|(_, head)| head.to_vec())
                .unwrap_or_else(|| concrete_ref.clone());
            if opaque.contains(&format!(
                "{}|{}",
                ref_surface,
                render_path(&target_field_path)
            )) {
                findings.push(Finding::new(
                    "opaque_marker_as_provenance",
                    &record.surface,
                    &record.pointer_raw,
                    &record.field_id,
                    format!(
                        "ref `{ref_raw}` resolves to an opaque marker at {}",
                        render_path(&target_field_path)
                    ),
                ));
                continue;
            }

            let bytes: Vec<u8> = if ref_pattern.is_empty() {
                if canonicalization != "exact_file_bytes_sha256" {
                    findings.push(Finding::new(
                        "non_canonical_bytes",
                        &record.surface,
                        &record.pointer_raw,
                        &record.field_id,
                        format!(
                            "whole-file ref requires exact_file_bytes_sha256, found `{canonicalization}`"
                        ),
                    ));
                    continue;
                }
                world
                    .raw_bytes
                    .get(&ref_surface)
                    .cloned()
                    .unwrap_or_default()
            } else {
                if canonicalization == "exact_file_bytes_sha256" {
                    findings.push(Finding::new(
                        "non_canonical_bytes",
                        &record.surface,
                        &record.pointer_raw,
                        &record.field_id,
                        format!("ref `{ref_raw}` is not a whole-file ref"),
                    ));
                    continue;
                }
                let Some(target) = resolve_one(ref_doc, &concrete_ref) else {
                    findings.push(Finding::new(
                        "declared_field_not_found",
                        &record.surface,
                        &record.pointer_raw,
                        &record.field_id,
                        format!(
                            "canonical bytes do not resolve at {}",
                            render_path(&concrete_ref)
                        ),
                    ));
                    continue;
                };
                let Some(text) = target.as_str() else {
                    findings.push(Finding::new(
                        "non_canonical_bytes",
                        &record.surface,
                        &record.pointer_raw,
                        &record.field_id,
                        format!("{} is not a string", render_path(&concrete_ref)),
                    ));
                    continue;
                };
                if text.contains('\r') || text.ends_with('\n') {
                    findings.push(Finding::new(
                        "non_canonical_bytes",
                        &record.surface,
                        &record.pointer_raw,
                        &record.field_id,
                        format!(
                            "{} violates UTF-8/LF/no-trailing-newline canonicalization",
                            render_path(&concrete_ref)
                        ),
                    ));
                    continue;
                }
                text.as_bytes().to_vec()
            };

            let computed = sha256_hex(&bytes);
            let Some(declared_value) = hit.value.as_deref() else {
                findings.push(Finding::new(
                    "digest_mismatch",
                    &record.surface,
                    &hit.rendered,
                    &record.field_id,
                    "declared value is not a string",
                ));
                continue;
            };
            let normalized = digest_normalize(declared_value, &plan.vocab.normalization);
            if normalized != computed {
                findings.push(Finding::new(
                    "digest_mismatch",
                    &record.surface,
                    &hit.rendered,
                    &record.field_id,
                    format!(
                        "declared {normalized} but recomputed {computed} from {}",
                        render_path(&concrete_ref)
                    ),
                ));
            }
        }
    }

    findings
}

/// Both-or-neither: every object carrying canonical bytes must carry `bytes`
/// *and* `sha256`. A half-pair is not provable.
fn check_both_or_neither(world: &World, plan: &Plan) -> Vec<Finding> {
    let mut findings = Vec::new();
    for surface in &plan.surfaces {
        let Some(doc) = world.docs.get(&surface.key) else {
            continue;
        };
        for pointer in &surface.audit_pointers {
            let Some(pattern) = parse_pointer(pointer) else {
                continue;
            };
            for (path, node) in resolve(doc, &pattern) {
                let mut walking = path.clone();
                collect_byte_maps(node, &mut walking, &surface.key, &mut findings);
            }
        }
    }
    findings
}

fn collect_byte_maps(node: &Node, path: &mut Vec<Seg>, surface: &str, findings: &mut Vec<Finding>) {
    match node {
        Node::Map(entries) => {
            let has_bytes = entries.iter().any(|(key, _)| key == "bytes");
            let has_sha256 = entries
                .iter()
                .any(|(key, value)| key == "sha256" && value.as_str().is_some());
            let rendered = render_path(path);
            if has_bytes && !has_sha256 {
                findings.push(Finding::new(
                    "bytes_without_sha256_pair",
                    surface,
                    &rendered,
                    &format!("{surface}{rendered}"),
                    "canonical bytes object carries `bytes` without `sha256`",
                ));
            }
            if has_sha256 && !has_bytes {
                findings.push(Finding::new(
                    "sha256_without_bytes_pair",
                    surface,
                    &rendered,
                    &format!("{surface}{rendered}"),
                    "canonical bytes object carries `sha256` without `bytes`",
                ));
            }
            for (key, value) in entries {
                path.push(Seg::Key(key.clone()));
                collect_byte_maps(value, path, surface, findings);
                path.pop();
            }
        }
        Node::Seq(items) => {
            for (index, value) in items.iter().enumerate() {
                path.push(Seg::Idx(index));
                collect_byte_maps(value, path, surface, findings);
                path.pop();
            }
        }
        _ => {}
    }
}

/// Placeholder guard: no recomputed value may be a repeated nibble or a
/// repeated pair.
fn check_placeholders(plan: &Plan) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (index, record) in plan.declared.iter().enumerate() {
        if record.class != "recomputed" {
            continue;
        }
        for hit_index in &plan.hits[index] {
            let hit = &plan.discovered[*hit_index];
            if let Some(value) = hit.value.as_deref()
                && is_placeholder_digest(value)
            {
                findings.push(Finding::new(
                    "placeholder_digest_value",
                    &record.surface,
                    &hit.rendered,
                    &record.field_id,
                    format!("value `{value}` is a repeated-nibble/pair placeholder"),
                ));
            }
        }
    }
    findings
}

/// A furniture row claiming `reviewed_digest_bound` must actually carry the
/// digest, and that digest must be classified `recomputed`.
fn check_furniture_claims(world: &World, plan: &Plan) -> Vec<Finding> {
    let mut findings = Vec::new();
    for surface in &plan.surfaces {
        let Some(doc) = world.docs.get(&surface.key) else {
            continue;
        };
        for pointer in &surface.audit_pointers {
            let Some(pattern) = parse_pointer(pointer) else {
                continue;
            };
            for (path, node) in resolve(doc, &pattern) {
                let mut walking = path.clone();
                collect_furniture_claims(node, &mut walking, &surface.key, plan, &mut findings);
            }
        }
    }
    findings
}

fn collect_furniture_claims(
    node: &Node,
    path: &mut Vec<Seg>,
    surface: &str,
    plan: &Plan,
    findings: &mut Vec<Finding>,
) {
    match node {
        Node::Map(entries) => {
            let claims_digest = entries.iter().any(|(key, value)| {
                key == "evidence"
                    && value
                        .as_str()
                        .map(|text| text.contains("digest"))
                        .unwrap_or(false)
            });
            if claims_digest {
                let mut digest_path = path.clone();
                digest_path.push(Seg::Key("digest".to_string()));
                let rendered = render_path(&digest_path);
                let discovered_index = plan
                    .discovered
                    .iter()
                    .position(|field| field.surface == surface && field.rendered == rendered);
                let classified_recomputed = discovered_index
                    .map(|index| {
                        plan.claims[index].iter().any(|declared_index| {
                            plan.declared[*declared_index].class == "recomputed"
                        })
                    })
                    .unwrap_or(false);
                if !classified_recomputed {
                    findings.push(Finding::new(
                        "furniture_claim_without_bound_digest",
                        surface,
                        &rendered,
                        &format!("{surface}{rendered}"),
                        "element claims a digest in `evidence` but carries no recomputed digest",
                    ));
                }
            }
            for (key, value) in entries {
                path.push(Seg::Key(key.clone()));
                collect_furniture_claims(value, path, surface, plan, findings);
                path.pop();
            }
        }
        Node::Seq(items) => {
            for (index, value) in items.iter().enumerate() {
                path.push(Seg::Idx(index));
                collect_furniture_claims(value, path, surface, plan, findings);
                path.pop();
            }
        }
        _ => {}
    }
}

/// A field adjudicated as an unreferenceable dangling pin must stay removed;
/// its reinstatement must fail until it is explicitly classified.
fn check_adjudications(world: &World) -> Vec<Finding> {
    let mut findings = Vec::new();
    let Some(adjudications) = world
        .contract
        .get("adjudications")
        .and_then(|node| node.as_seq())
    else {
        return findings;
    };
    for entry in adjudications {
        let action = entry
            .get("action")
            .and_then(|node| node.as_str())
            .unwrap_or_default();
        if action != "removed_unreferenced_dangling_pin" {
            continue;
        }
        let surface = entry
            .get("surface")
            .and_then(|node| node.as_str())
            .unwrap_or_default();
        let pointer = entry
            .get("pointer")
            .and_then(|node| node.as_str())
            .unwrap_or_default();
        let field_id = entry
            .get("field_id")
            .and_then(|node| node.as_str())
            .unwrap_or_default();
        let (Some(doc), Some(path)) = (world.docs.get(surface), parse_pointer(pointer)) else {
            continue;
        };
        if resolve_one(doc, &path).is_some() {
            findings.push(Finding::new(
                "adjudicated_field_reintroduced",
                surface,
                pointer,
                field_id,
                "field was adjudicated as removed but is present again",
            ));
        }
    }
    findings
}

/// Cross-file binding: the recomputed sha256 of the artifact bytes must equal
/// the declaration's pin and the contract's `bindings.source_data` pin.
fn check_cross_file_binding(world: &World, plan: &Plan) -> Vec<Finding> {
    let mut findings = Vec::new();
    let Some(source_bytes) = world.raw_bytes.get("source") else {
        return findings;
    };
    let recomputed = sha256_hex(source_bytes);

    if let Some(manifest) = world.docs.get("manifest")
        && let Some(pin) = manifest
            .get("oracle")
            .and_then(|node| node.get("source_artifact_sha256"))
            .and_then(|node| node.as_str())
    {
        let normalized = digest_normalize(pin, &plan.vocab.normalization);
        if normalized != recomputed {
            findings.push(Finding::new(
                "cross_file_binding_mismatch",
                "manifest",
                "/oracle/source_artifact_sha256",
                "manifest.oracle.source_artifact_sha256",
                format!("declaration pins {normalized} but the artifact recomputes {recomputed}"),
            ));
        }
    }

    if let Some(pins) = world
        .contract
        .get("bindings")
        .and_then(|node| node.get("source_data"))
        .and_then(|node| node.as_seq())
    {
        for pin in pins {
            let path = pin
                .get("path")
                .and_then(|node| node.as_str())
                .unwrap_or_default();
            let Some(declared) = pin.get("sha256").and_then(|node| node.as_str()) else {
                findings.push(Finding::new(
                    "cross_file_binding_mismatch",
                    "contract",
                    "/bindings/source_data",
                    "bindings.source_data",
                    format!("pin for {path} has no sha256"),
                ));
                continue;
            };
            let is_artifact = world
                .paths
                .get("source")
                .map(|known| known == path)
                .unwrap_or(false);
            if is_artifact {
                let normalized = digest_normalize(declared, &plan.vocab.normalization);
                if normalized != recomputed {
                    findings.push(Finding::new(
                        "cross_file_binding_mismatch",
                        "contract",
                        "/bindings/source_data",
                        "bindings.source_data",
                        format!("pin {normalized} does not match recomputed {recomputed}"),
                    ));
                }
            }
        }
    }

    findings
}

fn audit(world: &World) -> Vec<Finding> {
    let (plan, mut findings) = build_plan(world);
    findings.extend(check_both_or_neither(world, &plan));
    findings.extend(check_recomputation(world, &plan));
    findings.extend(check_placeholders(&plan));
    findings.extend(check_furniture_claims(world, &plan));
    findings.extend(check_adjudications(world));
    findings.extend(check_cross_file_binding(world, &plan));
    findings
}

fn allowed_labels(plan: &Plan) -> BTreeSet<String> {
    plan.vocab
        .oracle_violations
        .iter()
        .chain(plan.vocab.reader_violations.iter())
        .cloned()
        .collect()
}

fn assert_labels_in_vocabulary(plan: &Plan, findings: &[Finding]) {
    let allowed = allowed_labels(plan);
    for finding in findings {
        assert!(
            allowed.contains(&finding.label),
            "reader emitted `{}`, which is outside violations ∪ reader_violations: {}",
            finding.label,
            finding.render()
        );
    }
}

fn assert_has_finding(findings: &[Finding], label: &str, field_id: &str) {
    assert!(
        findings
            .iter()
            .any(|finding| finding.label == label && finding.field_id == field_id),
        "expected `{label}` for field_id `{field_id}`, got:\n{}",
        render_findings(findings)
    );
    assert!(
        findings
            .iter()
            .filter(|finding| finding.label == label && finding.field_id == field_id)
            .all(|finding| finding.render().contains(field_id)),
        "diagnostic for `{label}` must name the field id `{field_id}`:\n{}",
        render_findings(findings)
    );
}

// ---------------------------------------------------------------------------
// Mutation helpers
// ---------------------------------------------------------------------------

fn set_at(doc: &mut Node, pointer: &str, value: Node) {
    let path = parse_pointer(pointer).unwrap_or_else(|| panic!("bad pointer {pointer}"));
    let target =
        resolve_one_mut(doc, &path).unwrap_or_else(|| panic!("pointer {pointer} does not resolve"));
    *target = value;
}

fn remove_at(doc: &mut Node, pointer: &str) {
    let path = parse_pointer(pointer).unwrap_or_else(|| panic!("bad pointer {pointer}"));
    let (last, parent_path) = path
        .split_last()
        .unwrap_or_else(|| panic!("cannot remove the document root"));
    let parent = resolve_one_mut(doc, parent_path)
        .unwrap_or_else(|| panic!("parent of {pointer} does not resolve"));
    match (parent, last) {
        (Node::Map(entries), Seg::Key(key)) => entries.retain(|(candidate, _)| candidate != key),
        (Node::Seq(items), Seg::Idx(index)) => {
            items.remove(*index);
        }
        _ => panic!("cannot remove {pointer}"),
    }
}

fn insert_str_at(doc: &mut Node, parent_pointer: &str, key: &str, value: &str) {
    let path = parse_pointer(parent_pointer).expect("valid parent pointer");
    let parent = resolve_one_mut(doc, &path).expect("parent resolves");
    match parent {
        Node::Map(entries) => entries.push((key.to_string(), Node::Str(value.to_string()))),
        _ => panic!("insert target {parent_pointer} is not a map"),
    }
}

/// Flips the first hex digit so the value stays a well-formed digest but stops
/// being the recomputed one.
fn perturb_digest(value: &str) -> String {
    let (prefix, hexish) = match value.strip_prefix("sha256:") {
        Some(rest) => ("sha256:", rest),
        None => ("", value),
    };
    let mut flipped = false;
    let mut out = String::with_capacity(hexish.len());
    for character in hexish.chars() {
        if !flipped && character.is_ascii_hexdigit() {
            out.push(if character == '0' { '1' } else { '0' });
            flipped = true;
        } else {
            out.push(character);
        }
    }
    format!("{prefix}{out}")
}

/// One recomputed record together with the concrete canonical-bytes location
/// the negative matrix will mutate.
struct RecomputedTarget {
    field_id: String,
    ref_surface: String,
    ref_path: Vec<Seg>,
}

/// The concrete canonical-bytes location behind the first instance of every
/// recomputed record.
fn recomputed_targets(world: &World, plan: &Plan) -> Vec<RecomputedTarget> {
    let mut out = Vec::new();
    for (index, record) in plan.declared.iter().enumerate() {
        if record.class != "recomputed" {
            continue;
        }
        let (Some(raw), Some(field_pattern), Some(hit_index)) = (
            record.canonical_ref.as_deref(),
            record.path.as_ref(),
            plan.hits[index].first(),
        ) else {
            continue;
        };
        let Some((ref_surface_raw, ref_pointer_raw)) = split_ref(raw) else {
            continue;
        };
        let ref_surface = if ref_surface_raw == "self" {
            record.surface.clone()
        } else {
            ref_surface_raw
        };
        let (Some(ref_doc), Some(field_doc)) = (
            world.docs.get(&ref_surface),
            world.docs.get(&record.surface),
        ) else {
            continue;
        };
        let Some(ref_pattern) = parse_pointer(&ref_pointer_raw) else {
            continue;
        };
        let hit = &plan.discovered[*hit_index];
        let Some(concrete) = align_ref_path(
            field_doc,
            ref_doc,
            field_pattern,
            &hit.path,
            &ref_pattern,
            record.alignment.as_deref().unwrap_or("index"),
        ) else {
            continue;
        };
        out.push(RecomputedTarget {
            field_id: record.field_id.clone(),
            ref_surface,
            ref_path: concrete,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn oracle_digest_recomputation_control_on_unchanged_tree_is_clean() {
    let world = load_world();
    let (plan, plan_findings) = build_plan(&world);
    assert!(
        plan_findings.is_empty(),
        "plan-level violations on the committed tree:\n{}",
        render_findings(&plan_findings)
    );
    let findings = audit(&world);
    assert!(
        findings.is_empty(),
        "the committed tree must recompute cleanly:\n{}",
        render_findings(&findings)
    );
    assert_labels_in_vocabulary(&plan, &findings);
    // The closure must be non-trivial: 99 fixture fields + 99 artifact fields +
    // 4 declaration fields. A silently empty discovery would pass vacuously.
    assert_eq!(plan.discovered.len(), 202);
    assert_eq!(plan.declared.len(), 16);
}

#[test]
fn oracle_digest_recomputation_fixtures_mirror_the_contract_vocabulary() {
    let world = load_world();
    let root = repo_root();

    let positive_path = world
        .contract
        .get("fixtures")
        .and_then(|node| node.get("positive"))
        .and_then(|node| node.get("path"))
        .and_then(|node| node.as_str())
        .expect("positive fixture path is declared");
    let positive = parse_json(&read_required(&root, positive_path), positive_path);
    let declared = Node::Seq(world.digest_fields().to_vec());
    assert_eq!(
        canonicalize(&positive),
        canonicalize(&declared),
        "positive fixture must be a verbatim copy of digest_fields"
    );

    let negative_path = world
        .contract
        .get("fixtures")
        .and_then(|node| node.get("negative"))
        .and_then(|node| node.get("path"))
        .and_then(|node| node.as_str())
        .expect("negative fixture path is declared");
    let negative = parse_json(&read_required(&root, negative_path), negative_path);
    let labels: Vec<String> = negative
        .as_seq()
        .expect("negative fixture is a list")
        .iter()
        .map(|item| {
            assert!(
                item.get("record").is_some(),
                "every negative record must carry a `record` payload"
            );
            item.get("violation")
                .and_then(|node| node.as_str())
                .expect("negative record has a violation label")
                .to_string()
        })
        .collect();
    let required: Vec<String> = world
        .contract
        .get("fixtures")
        .and_then(|node| node.get("negative"))
        .and_then(|node| node.get("expect"))
        .and_then(|node| node.get("require_violations"))
        .and_then(|node| node.as_seq())
        .expect("require_violations is declared")
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect();
    assert_eq!(
        labels, required,
        "negative fixture labels must match the contract"
    );
    assert_eq!(required.len(), 10, "ten per-record labels are required");

    let oracle: BTreeSet<String> = string_list(&world.contract, "violations")
        .into_iter()
        .collect();
    for label in &required {
        assert!(
            oracle.contains(label),
            "required negative label `{label}` is outside the closed oracle vocabulary"
        );
    }
    let reader: BTreeSet<String> = string_list(&world.contract, "reader_violations")
        .into_iter()
        .collect();
    for guard in [
        "missing_closed_vocabulary",
        "bytes_without_sha256_pair",
        "sha256_without_bytes_pair",
        "duplicate_field_binding",
    ] {
        assert!(
            oracle.contains(guard),
            "contract-level guard `{guard}` must stay in the oracle vocabulary"
        );
        assert!(
            !required.iter().any(|label| label == guard),
            "contract-level guard `{guard}` must not be a per-record requirement"
        );
    }
    assert!(!reader.is_empty());
}

#[test]
fn oracle_digest_recomputation_detects_canonical_byte_drift_for_every_recomputed_record() {
    let world = load_world();
    let (plan, _) = build_plan(&world);
    let targets = recomputed_targets(&world, &plan);
    assert!(
        targets.len() == 11,
        "expected every recomputed record to expose a target (11), got {}",
        targets.len()
    );

    for target in targets {
        let RecomputedTarget {
            field_id,
            ref_surface,
            ref_path,
        } = target;
        let mut mutated = world.clone();
        if ref_path.is_empty() {
            // Whole-file ref: appending a byte changes the artifact identity
            // without touching the parsed documents.
            mutated
                .raw_bytes
                .get_mut(&ref_surface)
                .expect("surface raw bytes")
                .push(b' ');
        } else {
            let doc = mutated
                .docs
                .get_mut(&ref_surface)
                .unwrap_or_else(|| panic!("no surface {ref_surface}"));
            let current = resolve_one(doc, &ref_path)
                .and_then(|node| node.as_str())
                .unwrap_or_default()
                .to_string();
            set_at(
                doc,
                &render_path(&ref_path),
                Node::Str(format!("{current}x")),
            );
        }

        let (mutated_plan, _) = build_plan(&mutated);
        let findings = audit(&mutated);
        assert_labels_in_vocabulary(&mutated_plan, &findings);
        assert!(
            findings.iter().any(|finding| {
                finding.field_id == field_id
                    && matches!(
                        finding.label.as_str(),
                        "digest_mismatch" | "non_canonical_bytes"
                    )
            }),
            "mutating canonical bytes for `{field_id}` must be detected:\n{}",
            render_findings(&findings)
        );
    }
}

#[test]
fn oracle_digest_recomputation_detects_declared_digest_drift_for_every_recomputed_record() {
    let world = load_world();
    let (plan, _) = build_plan(&world);
    let mut checked = 0usize;

    for (index, record) in plan.declared.iter().enumerate() {
        if record.class != "recomputed" {
            continue;
        }
        for hit_index in &plan.hits[index] {
            let hit = &plan.discovered[*hit_index];
            let Some(value) = hit.value.as_deref() else {
                continue;
            };
            let mut mutated = world.clone();
            let doc = mutated
                .docs
                .get_mut(&record.surface)
                .expect("record surface");
            set_at(doc, &hit.rendered, Node::Str(perturb_digest(value)));
            let (mutated_plan, _) = build_plan(&mutated);
            let findings = audit(&mutated);
            assert_labels_in_vocabulary(&mutated_plan, &findings);
            assert_has_finding(&findings, "digest_mismatch", &record.field_id);
            checked += 1;
            break; // one instance per record is enough; they share the same claim
        }
    }
    assert!(
        checked == 11,
        "expected to exercise every recomputed record (11), got {checked}"
    );
}

#[test]
fn oracle_digest_recomputation_detects_index_alignment_break_on_swapped_bytes() {
    let world = load_world();
    let mut mutated = world.clone();
    {
        let source = mutated.docs.get_mut("source").expect("source surface");
        let first = resolve_one(
            source,
            &parse_pointer("/receipts/0/canonical_page_range/bytes").unwrap(),
        )
        .and_then(|node| node.as_str())
        .expect("first page range bytes")
        .to_string();
        let second = resolve_one(
            source,
            &parse_pointer("/receipts/1/canonical_page_range/bytes").unwrap(),
        )
        .and_then(|node| node.as_str())
        .expect("second page range bytes")
        .to_string();
        set_at(
            source,
            "/receipts/0/canonical_page_range/bytes",
            Node::Str(second),
        );
        set_at(
            source,
            "/receipts/1/canonical_page_range/bytes",
            Node::Str(first),
        );
    }
    let (plan, _) = build_plan(&mutated);
    let findings = audit(&mutated);
    assert_labels_in_vocabulary(&plan, &findings);
    assert_has_finding(
        &findings,
        "digest_mismatch",
        "fixture.receipt.page_receipt_range.receipt_digest",
    );
    assert_has_finding(
        &findings,
        "digest_mismatch",
        "source.receipt.page_range_digest",
    );
}

#[test]
fn oracle_digest_recomputation_detects_region_key_mis_binding() {
    let world = load_world();
    let mut mutated = world.clone();
    {
        let source = mutated.docs.get_mut("source").expect("source surface");
        let header = resolve_one(
            source,
            &parse_pointer("/receipts/0/canonical_boundary/0/bytes").unwrap(),
        )
        .and_then(|node| node.as_str())
        .expect("header boundary bytes")
        .to_string();
        let body = resolve_one(
            source,
            &parse_pointer("/receipts/0/canonical_boundary/1/bytes").unwrap(),
        )
        .and_then(|node| node.as_str())
        .expect("body boundary bytes")
        .to_string();
        assert!(header.contains("region=header"));
        assert!(body.contains("region=body"));
        set_at(
            source,
            "/receipts/0/canonical_boundary/0/bytes",
            Node::Str(body),
        );
        set_at(
            source,
            "/receipts/0/canonical_boundary/1/bytes",
            Node::Str(header),
        );
    }
    let (plan, _) = build_plan(&mutated);
    let findings = audit(&mutated);
    assert_labels_in_vocabulary(&plan, &findings);
    assert_has_finding(
        &findings,
        "digest_mismatch",
        "fixture.receipt.region_run.boundary_digest",
    );
    assert_has_finding(
        &findings,
        "digest_mismatch",
        "source.receipt.canonical_boundary.sha256",
    );
}

#[test]
fn oracle_digest_recomputation_detects_half_canonical_byte_pairs() {
    let world = load_world();

    let mut without_sha256 = world.clone();
    remove_at(
        without_sha256
            .docs
            .get_mut("source")
            .expect("source surface"),
        "/receipts/0/canonical_page_range/sha256",
    );
    let (plan, _) = build_plan(&without_sha256);
    let findings = audit(&without_sha256);
    assert_labels_in_vocabulary(&plan, &findings);
    assert!(
        findings
            .iter()
            .any(|finding| finding.label == "bytes_without_sha256_pair"),
        "bytes without sha256 must be reported:\n{}",
        render_findings(&findings)
    );

    let mut without_bytes = world.clone();
    remove_at(
        without_bytes
            .docs
            .get_mut("source")
            .expect("source surface"),
        "/receipts/0/canonical_annotation/bytes",
    );
    let (plan, _) = build_plan(&without_bytes);
    let findings = audit(&without_bytes);
    assert_labels_in_vocabulary(&plan, &findings);
    assert!(
        findings
            .iter()
            .any(|finding| finding.label == "sha256_without_bytes_pair"),
        "sha256 without bytes must be reported:\n{}",
        render_findings(&findings)
    );
}

#[test]
fn oracle_digest_recomputation_detects_unclassified_and_unresolvable_fields() {
    let world = load_world();

    let mut with_rogue = world.clone();
    insert_str_at(
        with_rogue.docs.get_mut("source").expect("source surface"),
        "/receipts/0",
        "rogue_digest",
        &"a".repeat(64),
    );
    let (plan, _) = build_plan(&with_rogue);
    let findings = audit(&with_rogue);
    assert_labels_in_vocabulary(&plan, &findings);
    assert!(
        findings.iter().any(|finding| {
            finding.label == "unclassified_digest_field" && finding.pointer.contains("rogue_digest")
        }),
        "an unclaimed digest field must be reported:\n{}",
        render_findings(&findings)
    );

    let mut with_phantom = world.clone();
    with_phantom.digest_fields_mut().push(Node::Map(vec![
        (
            "field_id".to_string(),
            Node::Str("source.receipt.phantom_digest".to_string()),
        ),
        ("surface".to_string(), Node::Str("source".to_string())),
        (
            "pointer".to_string(),
            Node::Str("/receipts/*/phantom_digest".to_string()),
        ),
        ("class".to_string(), Node::Str("recomputed".to_string())),
        (
            "ref".to_string(),
            Node::Str("self:/receipts/*/canonical_page_range/bytes".to_string()),
        ),
        ("alignment".to_string(), Node::Str("index".to_string())),
        (
            "canonicalization".to_string(),
            Node::Str("text_utf8_lf_no_trailing_newline_sha256".to_string()),
        ),
    ]));
    let (plan, _) = build_plan(&with_phantom);
    let findings = audit(&with_phantom);
    assert_labels_in_vocabulary(&plan, &findings);
    assert_has_finding(
        &findings,
        "declared_field_not_found",
        "source.receipt.phantom_digest",
    );
}

#[test]
fn oracle_digest_recomputation_rejects_opaque_markers_as_provenance() {
    let world = load_world();
    let mut mutated = world.clone();
    mutated.digest_fields_mut().push(Node::Map(vec![
        (
            "field_id".to_string(),
            Node::Str("fixture.receipt.opaque_provenance_probe".to_string()),
        ),
        ("surface".to_string(), Node::Str("fixture".to_string())),
        (
            "pointer".to_string(),
            Node::Str("/receipts/*/source_sha256".to_string()),
        ),
        ("class".to_string(), Node::Str("recomputed".to_string())),
        (
            "ref".to_string(),
            Node::Str("fixture:/receipts/*/source_sha256".to_string()),
        ),
        ("alignment".to_string(), Node::Str("index".to_string())),
        (
            "canonicalization".to_string(),
            Node::Str("text_utf8_lf_no_trailing_newline_sha256".to_string()),
        ),
    ]));
    let (plan, _) = build_plan(&mutated);
    let findings = audit(&mutated);
    assert_labels_in_vocabulary(&plan, &findings);
    assert_has_finding(
        &findings,
        "opaque_marker_as_provenance",
        "fixture.receipt.opaque_provenance_probe",
    );
}

#[test]
fn oracle_digest_recomputation_rejects_reintroduced_adjudicated_field() {
    let world = load_world();
    let mut mutated = world.clone();
    insert_str_at(
        mutated.docs.get_mut("source").expect("source surface"),
        "",
        "manifest_sha256",
        &"f".repeat(64),
    );
    let (plan, _) = build_plan(&mutated);
    let findings = audit(&mutated);
    assert_labels_in_vocabulary(&plan, &findings);
    assert_has_finding(
        &findings,
        "adjudicated_field_reintroduced",
        "source.manifest_sha256",
    );
}

#[test]
fn oracle_digest_recomputation_detects_manifest_pin_drift() {
    let world = load_world();
    let mut mutated = world.clone();
    {
        let manifest = mutated.docs.get_mut("manifest").expect("manifest surface");
        let current = resolve_one(
            manifest,
            &parse_pointer("/oracle/source_artifact_sha256").unwrap(),
        )
        .and_then(|node| node.as_str())
        .expect("declaration pin")
        .to_string();
        set_at(
            manifest,
            "/oracle/source_artifact_sha256",
            Node::Str(perturb_digest(&current)),
        );
    }
    let (plan, _) = build_plan(&mutated);
    let findings = audit(&mutated);
    assert_labels_in_vocabulary(&plan, &findings);
    assert_has_finding(
        &findings,
        "cross_file_binding_mismatch",
        "manifest.oracle.source_artifact_sha256",
    );
    assert_has_finding(
        &findings,
        "digest_mismatch",
        "manifest.oracle.source_artifact_sha256",
    );

    // The contract's own pin must close too.
    let mut repinned = world.clone();
    {
        let contract = &mut repinned.contract;
        let path = parse_pointer("/bindings/source_data/0/sha256").unwrap();
        let target = resolve_one_mut(contract, &path).expect("contract pin");
        *target = Node::Str("0".repeat(63) + "1");
    }
    let (plan, _) = build_plan(&repinned);
    let findings = audit(&repinned);
    assert_labels_in_vocabulary(&plan, &findings);
    assert!(
        findings.iter().any(|finding| {
            finding.label == "cross_file_binding_mismatch"
                && finding.field_id == "bindings.source_data"
        }),
        "a drifted contract pin must be reported:\n{}",
        render_findings(&findings)
    );
}

#[test]
fn oracle_digest_recomputation_detects_placeholder_digest_values() {
    let world = load_world();
    for placeholder in ["5".repeat(64), "56".repeat(32)] {
        let mut mutated = world.clone();
        set_at(
            mutated.docs.get_mut("fixture").expect("fixture surface"),
            "/receipts/0/annotation_digest",
            Node::Str(placeholder.clone()),
        );
        let (plan, _) = build_plan(&mutated);
        let findings = audit(&mutated);
        assert_labels_in_vocabulary(&plan, &findings);
        assert_has_finding(
            &findings,
            "placeholder_digest_value",
            "fixture.receipt.annotation_digest",
        );
    }
}

#[test]
fn oracle_digest_recomputation_detects_furniture_claim_without_bound_digest() {
    let world = load_world();
    let mut mutated = world.clone();
    remove_at(
        mutated.docs.get_mut("fixture").expect("fixture surface"),
        "/receipts/0/furniture_sequence/0/digest",
    );
    let (plan, _) = build_plan(&mutated);
    let findings = audit(&mutated);
    assert_labels_in_vocabulary(&plan, &findings);
    assert!(
        findings
            .iter()
            .any(|finding| finding.label == "furniture_claim_without_bound_digest"),
        "a `reviewed_digest_bound` claim without the digest must be reported:\n{}",
        render_findings(&findings)
    );
}

#[test]
fn oracle_digest_recomputation_reader_is_independent_of_the_crate_under_test() {
    let own = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle_digest_recomputation.rs");
    let source = fs::read_to_string(&own)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", own.display()));

    // Built by concatenation so this assertion does not match itself.
    let needles = [
        ["da", "_parse::"].concat(),
        ["use da", "_parse"].concat(),
        ["include", "_str!"].concat(),
    ];
    for needle in needles {
        assert!(
            !source.contains(&needle),
            "the contract must not import or pre-bake the code under test (`{needle}`)"
        );
    }
    for hidden in [".gsd", ".planning", ".audits"].map(|prefix| format!("{prefix}/")) {
        assert!(
            !source.contains(&hidden),
            "the contract must not read `{hidden}` (gitignored, not durable evidence)"
        );
    }
}

#[test]
fn oracle_digest_recomputation_fails_closed_when_a_closed_vocabulary_is_missing() {
    let world = load_world();

    // A missing closed vocabulary is a contract-level guard, never a silent skip.
    let mut without_reasons = world.clone();
    remove_at(&mut without_reasons.contract, "/opaque_reasons");
    let (plan, plan_findings) = build_plan(&without_reasons);
    assert!(
        plan_findings.iter().any(|finding| {
            finding.label == "missing_closed_vocabulary" && finding.field_id == "opaque_reasons"
        }),
        "a missing opaque_reasons vocabulary must fail closed:\n{}",
        render_findings(&plan_findings)
    );
    assert_labels_in_vocabulary(&plan, &plan_findings);
    assert!(
        plan_findings
            .iter()
            .any(|finding| finding.label == "opaque_reason_outside_vocabulary"),
        "with no vocabulary to validate against, opaque declarations must be unresolvable"
    );

    // With no key dialect the discovery walk is empty, and that emptiness must be
    // visible rather than passing vacuously.
    let mut without_patterns = world.clone();
    remove_at(&mut without_patterns.contract, "/digest_field_patterns");
    let (plan, plan_findings) = build_plan(&without_patterns);
    assert!(
        plan_findings.iter().any(|finding| {
            finding.label == "missing_closed_vocabulary"
                && finding.field_id == "digest_field_patterns"
        }),
        "a missing key dialect must fail closed:\n{}",
        render_findings(&plan_findings)
    );
    assert!(plan.discovered.is_empty());
    assert_eq!(plan.declared.len(), 16);
}

#[test]
fn oracle_digest_recomputation_fails_closed_on_unreadable_or_malformed_surfaces() {
    let missing = std::panic::catch_unwind(|| {
        read_required(&repo_root(), "artifacts/parse_compat/does_not_exist.json")
    });
    let message = panic_message(missing.expect_err("a missing audited file must not be skipped"));
    assert!(
        message.contains("cannot read") && message.contains("does_not_exist.json"),
        "missing-file failure must name the path: {message}"
    );

    let malformed =
        std::panic::catch_unwind(|| parse_yaml(b"receipts: [unclosed", "docs/example.yaml"));
    let message = panic_message(malformed.expect_err("malformed YAML must not be skipped"));
    assert!(
        message.contains("cannot parse") && message.contains("docs/example.yaml"),
        "malformed-YAML failure must name the path: {message}"
    );

    let malformed_json =
        std::panic::catch_unwind(|| parse_json(b"{\"receipts\": [", "artifacts/example.json"));
    let message = panic_message(malformed_json.expect_err("malformed JSON must not be skipped"));
    assert!(
        message.contains("cannot parse") && message.contains("artifacts/example.json"),
        "malformed-JSON failure must name the path: {message}"
    );
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else {
        "<non-string panic payload>".to_string()
    }
}
