//! DATA contract for the canonical component licence / redistribution ledger
//! (`data/parse/license_inventory.yaml`, GAR-004, authority ADR-075).
//!
//! This gate is deliberately independent of the governor rule and of the Python
//! oracle that T03 adds (ADR-051: an oracle must not reuse the code under test).
//! It imports no `da-parse` symbol, decodes no typed model, and reads the four
//! committed YAML surfaces with the generic `serde_yaml` reader. It enforces that
//! every read-only surface identifier is claimed exactly once by the canonical
//! ledger, that every position is either `verified` with a resolvable licence
//! text or a typed `blocked` refusal with a non-empty reason and a blocked
//! redistribution status, and that a position claiming `verified` may neither be
//! contradicted by a blocked read-only surface position nor carry a licence token
//! outside the closed permissive vocabulary. An unestablished right therefore
//! cannot be reported as compatible, which is exactly the failure mode that
//! removes `UNVERIFIED`-style assumptions from the ledger.
//!
//! The validator is mutated in-process on clones of the live document, so every
//! negative case asserts the specific diagnostic, not merely a non-empty error
//! vector.

use serde_yaml::Value as Yaml;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const CANONICAL: &str = "data/parse/license_inventory.yaml";
const PREFIX: &str = "[license-provenance]";

/// The three read-only surfaces that must be bound by the canonical ledger.
const REQUIRED_SURFACES: [&str; 3] = [
    "data/parse/grobid_compatibility.yaml#license_provenance_inventory",
    "data/parse/feature_tagger_training_parity.yaml#license_provenance_inventory",
    "data/parse/grobid_model_rights.yaml#assets",
];

const SCHEMA_VERSION: &str = "parse-component-license-inventory.v1";
const LICENSE_STATUS_VOCABULARY: [&str; 2] = ["verified", "blocked"];
const BOUNDARY_VOCABULARY: [&str; 6] = [
    "subprocess",
    "http",
    "rust-source-port",
    "rust-workspace",
    "data-only",
    "cargo-dependency",
];
const REDISTRIBUTION_VOCABULARY: [&str; 4] = [
    "allowed_with_notice",
    "source_offer_required",
    "not_distributed",
    "blocked",
];

/// Closed licence vocabulary that may carry `verified`. A verified position must
/// name a permissive, well-known text; `unknown` (or any other token) cannot be
/// validated by an inventory, so it must be blocked instead.
const VERIFIED_LICENCE_TOKENS: [&str; 2] = ["Apache-2.0", "MIT"];

/// Closed vocabulary of notice / source-offer obligations. Free text ("needs
/// legal review") is indistinguishable from a promise and is rejected; a field
/// may also name an existing repository path instead of a token.
const OBLIGATION_TOKENS: [&str; 13] = [
    "not_applicable",
    "not_required_for_unmodified_service",
    "not_required_for_source_available_dependency",
    "required_in_release_notice",
    "required_before_release",
    "required_before_any_redistribution",
    "required_before_any_corpus_or_weight_release",
    "required_before_source_redistribution",
    "required_before_corpus_release",
    "required_before_weight_release",
    "required_before_engine_distribution",
    "required_before_jar_distribution",
    "source_comment_only_not_a_rights_clearance",
];

/// A redistribution token that would claim permission to distribute.
const FORBIDDEN_FOR_BLOCKED: [&str; 2] = ["allowed", "allowed_with_notice"];

/// `data/parse/grobid_model_rights.yaml` publishes no status vocabulary of its
/// own, so its `rights_status` dialect is pinned here.
const SURFACE_STATUS_FALLBACK: [&str; 3] = ["verified", "blocked", "separately_blocked"];
const BLOCKED_SURFACE_STATUSES: [&str; 2] = ["blocked", "separately_blocked"];

const STATUS_FIELDS: [&str; 2] = ["license_status", "rights_status"];
const ID_FIELDS: [&str; 2] = ["component_id", "id"];

const UNAVAILABLE_STATUS_LITERAL: &str = "UNVERIFIED";

const POLICY_LOCKS_STR: [(&str, &str); 5] = [
    ("new_distribution", "forbidden"),
    ("graph_write", "forbidden"),
    ("auto_promote", "forbidden"),
    ("gpl_linking", "forbidden"),
    ("unknown_rights", "blocked"),
];
const POLICY_LOCKS_BOOL: [(&str, bool); 3] = [
    ("commercial_use", false),
    ("import_eligible", false),
    ("blocked_components_must_not_enter_runtime", true),
];

type SurfaceIds = BTreeMap<String, BTreeSet<String>>;
type SurfaceStatuses = BTreeMap<String, BTreeMap<String, String>>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{PREFIX} cannot read `{rel}`: {e}"))
}

fn yaml(rel: &str) -> Yaml {
    let text = read(rel);
    serde_yaml::from_str(&text)
        .unwrap_or_else(|e| panic!("{PREFIX} `{rel}` is not valid YAML: {e}"))
}

fn at<'a>(v: &'a Yaml, key: &str) -> Option<&'a Yaml> {
    v.get(key)
}

fn at_mut<'a>(v: &'a mut Yaml, key: &str) -> Option<&'a mut Yaml> {
    v.get_mut(key)
}

fn s<'a>(v: &'a Yaml, key: &str) -> Option<&'a str> {
    at(v, key).and_then(|x| x.as_str())
}

fn flag(v: &Yaml, key: &str) -> Option<bool> {
    at(v, key).and_then(|x| x.as_bool())
}

fn seq<'a>(v: &'a Yaml, key: &str) -> Option<&'a Vec<Yaml>> {
    at(v, key).and_then(|x| x.as_sequence())
}

fn seq_mut<'a>(v: &'a mut Yaml, key: &str) -> &'a mut Vec<Yaml> {
    at_mut(v, key)
        .and_then(|x| x.as_sequence_mut())
        .unwrap_or_else(|| panic!("{PREFIX} expected a sequence at `{key}`"))
}

fn set(v: &mut Yaml, key: &str, value: Yaml) {
    if let Yaml::Mapping(m) = v {
        m.insert(Yaml::String(key.to_string()), value);
    }
}

fn unset(v: &mut Yaml, key: &str) {
    if let Yaml::Mapping(m) = v {
        m.remove(Yaml::String(key.to_string()));
    }
}

fn nonempty(v: &Yaml, key: &str) -> bool {
    s(v, key).is_some_and(|x| !x.trim().is_empty())
}

fn strings_of(xs: &[Yaml]) -> Vec<String> {
    xs.iter()
        .filter_map(|x| x.as_str())
        .map(|x| x.to_string())
        .collect()
}

fn str_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn entry_id(entry: &Yaml) -> Option<&str> {
    ID_FIELDS
        .iter()
        .find_map(|k| entry.get(*k).and_then(|v| v.as_str()))
}

fn claim(surface: &str, component_id: &str) -> Yaml {
    let mut m = serde_yaml::Mapping::new();
    m.insert(
        Yaml::String("surface".to_string()),
        Yaml::String(surface.to_string()),
    );
    m.insert(
        Yaml::String("component_id".to_string()),
        Yaml::String(component_id.to_string()),
    );
    Yaml::Mapping(m)
}

fn component<'a>(doc: &'a Yaml, id: &str) -> Option<&'a Yaml> {
    seq(doc, "components").and_then(|cs| cs.iter().find(|c| entry_id(c) == Some(id)))
}

fn component_mut<'a>(doc: &'a mut Yaml, id: &str) -> &'a mut Yaml {
    seq_mut(doc, "components")
        .iter_mut()
        .find(|c| entry_id(c) == Some(id))
        .unwrap_or_else(|| panic!("{PREFIX} canonical component `{id}` not found"))
}

fn claims_mut<'a>(doc: &'a mut Yaml, id: &str) -> &'a mut Vec<Yaml> {
    seq_mut(component_mut(doc, id), "surface_claims")
}

fn surface_decl(role: &str, inventory_key: &str, status_field: &str) -> Yaml {
    let mut m = serde_yaml::Mapping::new();
    m.insert(
        Yaml::String("role".to_string()),
        Yaml::String(role.to_string()),
    );
    m.insert(
        Yaml::String("inventory_key".to_string()),
        Yaml::String(inventory_key.to_string()),
    );
    m.insert(
        Yaml::String("status_field".to_string()),
        Yaml::String(status_field.to_string()),
    );
    Yaml::Mapping(m)
}

fn surface_decl_mut<'a>(doc: &'a mut Yaml, key: &str) -> &'a mut Yaml {
    doc.get_mut("surfaces")
        .and_then(|v| v.as_mapping_mut())
        .and_then(|m| m.get_mut(Yaml::String(key.to_string())))
        .unwrap_or_else(|| panic!("{PREFIX} surface declaration `{key}` not found"))
}

fn surfaces_map_mut(doc: &mut Yaml) -> &mut serde_yaml::Mapping {
    doc.get_mut("surfaces")
        .and_then(|v| v.as_mapping_mut())
        .expect("canonical surfaces map")
}

/// A licence reference is resolvable when it is an `http(s)` upstream pointer, an
/// existing absolute path (vendor licence text), or an existing path relative to
/// the workspace root. Anything else is an unusable claim of evidence.
fn resolvable(value: &str) -> bool {
    if value.starts_with("http://") || value.starts_with("https://") {
        return true;
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.exists()
    } else {
        root().join(value).exists()
    }
}

#[derive(Debug, Clone)]
struct SurfaceCfg {
    key: String,
    path: String,
    inventory_key: String,
    status_field: String,
}

/// The canonical ledger declares which read-only surfaces it binds, their
/// inventory key, and their status dialect. The declarations are validated here
/// (shape and repo-relative path) before a single surface file is read.
fn surface_cfgs(doc: &Yaml) -> (Vec<SurfaceCfg>, Vec<String>) {
    let mut errs = Vec::new();
    let mut out = Vec::new();
    let Some(map) = at(doc, "surfaces").and_then(|v| v.as_mapping()) else {
        errs.push(format!(
            "{PREFIX} surfaces: the canonical ledger must declare the read-only surface map"
        ));
        return (out, errs);
    };
    for (raw_key, cfg) in map {
        let Some(key) = raw_key.as_str() else {
            errs.push(format!("{PREFIX} surfaces: surface key must be a string"));
            continue;
        };
        let Some((path, inventory_key)) = key.split_once('#') else {
            errs.push(format!(
                "{PREFIX} surfaces[{key}]: key must be `<repo-relative file>#<inventory key>`"
            ));
            continue;
        };
        if path.trim().is_empty() || inventory_key.trim().is_empty() {
            errs.push(format!(
                "{PREFIX} surfaces[{key}]: both the file path and the inventory key must be non-empty"
            ));
            continue;
        }
        if Path::new(path).is_absolute() {
            errs.push(format!(
                "{PREFIX} surfaces[{key}]: surface file must be repo-relative"
            ));
            continue;
        }
        let Some(status_field) = s(cfg, "status_field") else {
            errs.push(format!(
                "{PREFIX} surfaces[{key}].status_field: status dialect is missing"
            ));
            continue;
        };
        if !STATUS_FIELDS.contains(&status_field) {
            errs.push(format!(
                "{PREFIX} surfaces[{key}].status_field: `{status_field}` is outside the known status dialects"
            ));
            continue;
        }
        if !nonempty(cfg, "role") {
            errs.push(format!(
                "{PREFIX} surfaces[{key}].role: the surface role must be non-empty"
            ));
        }
        out.push(SurfaceCfg {
            key: key.to_string(),
            path: path.to_string(),
            inventory_key: inventory_key.to_string(),
            status_field: status_field.to_string(),
        });
    }
    if out.is_empty() && errs.is_empty() {
        errs.push(format!(
            "{PREFIX} surfaces: no read-only licence surface is declared"
        ));
    }
    (out, errs)
}

struct SurfaceRead {
    ids: BTreeSet<String>,
    statuses: BTreeMap<String, String>,
    errs: Vec<String>,
}

/// Structural validation of one read-only surface under its own declared
/// vocabulary. Evidence fields are *not* resolved to files here: the training
/// parity surface points at in-file keys (`source_contract.source_documents`,
/// `corpus_binding`, `engines`), which are ledger references, not paths.
fn surface_doc_errs(cfg: &SurfaceCfg, doc: &Yaml) -> SurfaceRead {
    let mut errs = Vec::new();
    let mut ids = BTreeSet::new();
    let mut statuses = BTreeMap::new();
    let Some(node) = at(doc, &cfg.inventory_key) else {
        errs.push(format!(
            "{PREFIX} {}: inventory key `{}` is missing",
            cfg.key, cfg.inventory_key
        ));
        return SurfaceRead {
            ids,
            statuses,
            errs,
        };
    };
    // The inventory node is either a mapping that holds a component sequence
    // (`components`) or, as in `grobid_model_rights.yaml`, the sequence itself.
    let entries = match node {
        Yaml::Sequence(xs) => Some(xs),
        _ => seq(node, "components").or_else(|| seq(node, "assets")),
    };
    let Some(entries) = entries else {
        errs.push(format!(
            "{PREFIX} {}: inventory `{}` declares no `components` or `assets` sequence",
            cfg.key, cfg.inventory_key
        ));
        return SurfaceRead {
            ids,
            statuses,
            errs,
        };
    };
    if entries.is_empty() {
        errs.push(format!(
            "{PREFIX} {}: inventory `{}` declares an empty component sequence",
            cfg.key, cfg.inventory_key
        ));
    }
    let vocab_key = format!("{}_vocabulary", cfg.status_field);
    let declared_vocab = seq(node, &vocab_key)
        .or_else(|| seq(doc, &vocab_key))
        .map(|xs| strings_of(xs));
    let vocab = match declared_vocab {
        Some(v) if !v.is_empty() => v.into_iter().collect::<BTreeSet<String>>(),
        Some(_) => {
            errs.push(format!(
                "{PREFIX} {}: declared `{vocab_key}` is empty, so no status token can be validated",
                cfg.key
            ));
            BTreeSet::new()
        }
        None => str_set(&SURFACE_STATUS_FALLBACK),
    };
    let boundary_vocab = seq(node, "boundary_vocabulary").map(|xs| strings_of(xs));
    let redistribution_vocab =
        seq(node, "redistribution_status_vocabulary").map(|xs| strings_of(xs));

    for (idx, entry) in entries.iter().enumerate() {
        let id = entry_id(entry)
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|x| x.to_string());
        let label = id.clone().unwrap_or_else(|| format!("entries[{idx}]"));
        let duplicate = id.as_ref().is_some_and(|x| !ids.insert(x.clone()));
        if id.is_none() {
            errs.push(format!(
                "{PREFIX} {}: entry without a non-empty component identifier",
                cfg.key
            ));
        }
        if duplicate {
            errs.push(format!(
                "{PREFIX} {}#{label}: identifier declared more than once on the surface",
                cfg.key
            ));
        }
        let status = s(entry, &cfg.status_field);
        match status {
            Some(token) if vocab.contains(token) => {}
            Some(token) => errs.push(format!(
                "{PREFIX} {}#{label}.{}: `{token}` is outside the closed surface vocabulary",
                cfg.key, cfg.status_field
            )),
            None => errs.push(format!(
                "{PREFIX} {}#{label}.{}: typed status is missing",
                cfg.key, cfg.status_field
            )),
        }
        let blocked = status.is_some_and(|token| BLOCKED_SURFACE_STATUSES.contains(&token));
        let reason = s(entry, "blocked_reason").or_else(|| s(entry, "evidence"));
        let reason_present = reason.is_some_and(|x| !x.trim().is_empty());
        if blocked && !reason_present {
            errs.push(format!(
                "{PREFIX} {}#{label}: blocked surface position needs a non-empty refusal reason (`blocked_reason` or `evidence`)",
                cfg.key
            ));
        }
        if status == Some("verified") && reason_present {
            errs.push(format!(
                "{PREFIX} {}#{label}: verified surface position must not carry a refusal reason",
                cfg.key
            ));
        }
        if blocked && flag(entry, "redistributable") == Some(true) {
            errs.push(format!(
                "{PREFIX} {}#{label}: `redistributable: true` while the surface status is `{}`",
                cfg.key,
                status.unwrap_or("<missing>")
            ));
        }
        if let Some(boundary) = s(entry, "boundary")
            && let Some(allowed) = &boundary_vocab
            && !allowed.is_empty()
            && !allowed.iter().any(|x| x == boundary)
        {
            errs.push(format!(
                "{PREFIX} {}#{label}.boundary: `{boundary}` is outside the surface boundary vocabulary",
                cfg.key
            ));
        }
        if let Some(redistribution) = s(entry, "redistribution")
            && let Some(allowed) = &redistribution_vocab
            && !allowed.is_empty()
            && !allowed.iter().any(|x| x == redistribution)
        {
            errs.push(format!(
                "{PREFIX} {}#{label}.redistribution: `{redistribution}` is outside the surface redistribution vocabulary",
                cfg.key
            ));
        }
        if let Some(token) = status {
            statuses.insert(label, token.to_string());
        }
    }
    let any_blocked = statuses
        .values()
        .any(|token| BLOCKED_SURFACE_STATUSES.contains(&token.as_str()));
    if any_blocked && flag(doc, "redistributable") == Some(true) {
        errs.push(format!(
            "{PREFIX} {}: document declares `redistributable: true` while a position is blocked",
            cfg.key
        ));
    }
    SurfaceRead {
        ids,
        statuses,
        errs,
    }
}

struct Surfaces {
    ids: SurfaceIds,
    statuses: SurfaceStatuses,
    errs: Vec<String>,
}

fn load_surfaces(doc: &Yaml) -> Surfaces {
    let (cfgs, mut errs) = surface_cfgs(doc);
    let mut ids = SurfaceIds::new();
    let mut statuses = SurfaceStatuses::new();
    for required in REQUIRED_SURFACES {
        let declared = cfgs.iter().any(|c| c.key == required);
        if !declared {
            errs.push(format!(
                "{PREFIX} surfaces: required read-only surface `{required}` is not declared"
            ));
        }
    }
    for cfg in &cfgs {
        let path = root().join(&cfg.path);
        if !path.exists() {
            errs.push(format!(
                "{PREFIX} {}: surface file `{}` does not exist",
                cfg.key, cfg.path
            ));
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{PREFIX} cannot read surface `{}`: {e}", cfg.path));
        let surface_doc: Yaml = serde_yaml::from_str(&text)
            .unwrap_or_else(|e| panic!("{PREFIX} surface `{}` is not valid YAML: {e}", cfg.path));
        let read = surface_doc_errs(cfg, &surface_doc);
        errs.extend(read.errs);
        ids.insert(cfg.key.clone(), read.ids);
        statuses.insert(cfg.key.clone(), read.statuses);
    }
    Surfaces {
        ids,
        statuses,
        errs,
    }
}

/// Canonical-ledger validation, independent of any surface file on disk: header
/// dialects and policy locks, per-position typing, claim resolution, and
/// cross-surface completeness against the identifier sets of the read-only
/// surfaces.
fn license_provenance_errs(doc: &Yaml, surfaces: &SurfaceIds) -> Vec<String> {
    let mut errs = Vec::new();

    if s(doc, "schema_version") != Some(SCHEMA_VERSION) {
        errs.push(format!(
            "{PREFIX} document.schema_version: expected `{SCHEMA_VERSION}`"
        ));
    }

    let vocabularies: [(&str, BTreeSet<String>); 3] = [
        ("boundary_vocabulary", str_set(&BOUNDARY_VOCABULARY)),
        (
            "license_status_vocabulary",
            str_set(&LICENSE_STATUS_VOCABULARY),
        ),
        (
            "redistribution_status_vocabulary",
            str_set(&REDISTRIBUTION_VOCABULARY),
        ),
    ];
    for (key, expected) in &vocabularies {
        match seq(doc, key) {
            None => errs.push(format!(
                "{PREFIX} document.{key}: the closed vocabulary is missing"
            )),
            Some(_) => {
                let raw = seq(doc, key).map(|xs| strings_of(xs)).unwrap_or_default();
                let declared = str_set(&raw.iter().map(|x| x.as_str()).collect::<Vec<&str>>());
                if declared.is_empty() {
                    errs.push(format!(
                        "{PREFIX} document.{key}: the closed vocabulary is declared empty"
                    ));
                } else if declared != *expected {
                    errs.push(format!(
                        "{PREFIX} document.{key}: closed vocabulary drift — declared {declared:?}, expected {expected:?}"
                    ));
                }
            }
        }
    }

    match at(doc, "redistribution_policy") {
        None => errs.push(format!(
            "{PREFIX} document.redistribution_policy: the lock block is missing"
        )),
        Some(policy) => {
            for (key, expected) in POLICY_LOCKS_STR {
                if s(policy, key) != Some(expected) {
                    errs.push(format!(
                        "{PREFIX} redistribution_policy.{key}: lock must stay `{expected}`"
                    ));
                }
            }
            for (key, expected) in POLICY_LOCKS_BOOL {
                if flag(policy, key) != Some(expected) {
                    errs.push(format!(
                        "{PREFIX} redistribution_policy.{key}: lock must stay `{expected}`"
                    ));
                }
            }
        }
    }

    match serde_yaml::to_string(doc) {
        Ok(text) if text.contains(UNAVAILABLE_STATUS_LITERAL) => errs.push(format!(
            "{PREFIX} document: the untrusted status literal `{UNAVAILABLE_STATUS_LITERAL}` appears in the canonical ledger"
        )),
        Ok(_) => {}
        Err(e) => errs.push(format!(
            "{PREFIX} document: cannot serialise for the status-literal scan: {e}"
        )),
    }

    let components = seq(doc, "components");
    let Some(components) = components else {
        errs.push(format!(
            "{PREFIX} document.components: position list is missing"
        ));
        return errs;
    };
    if components.is_empty() {
        errs.push(format!(
            "{PREFIX} document.components: position list is empty"
        ));
    }

    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    let mut claim_counts: BTreeMap<(String, String), usize> = BTreeMap::new();

    for (idx, position) in components.iter().enumerate() {
        let id = entry_id(position)
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|x| x.to_string());
        let label = id.clone().unwrap_or_else(|| format!("components[{idx}]"));
        if id.is_none() {
            errs.push(format!(
                "{PREFIX} {label}: `component_id` must be a non-empty string"
            ));
        }
        if id.as_ref().is_some_and(|x| !seen_ids.insert(x.clone())) {
            errs.push(format!(
                "{PREFIX} {label}: component_id is declared more than once"
            ));
        }

        let status = s(position, "license_status");
        match status {
            Some(token) if LICENSE_STATUS_VOCABULARY.contains(&token) => {}
            Some(token) => errs.push(format!(
                "{PREFIX} {label}.license_status: `{token}` is outside the closed vocabulary {{verified, blocked}}"
            )),
            None => errs.push(format!(
                "{PREFIX} {label}.license_status: typed status is missing"
            )),
        }

        let evidence = s(position, "license_evidence");
        if !nonempty(position, "license_evidence") {
            errs.push(format!(
                "{PREFIX} {label}.license_evidence: evidence reference must be non-empty"
            ));
        } else if !resolvable(evidence.unwrap_or_default()) {
            errs.push(format!(
                "{PREFIX} {label}.license_evidence: `{}` does not resolve to an existing workspace path, an existing absolute path, or an http(s) upstream pointer",
                evidence.unwrap_or_default()
            ));
        }

        for field in ["attribution_notice_artifact", "source_offer_artifact"] {
            match s(position, field) {
                None => errs.push(format!("{PREFIX} {label}.{field}: obligation is missing")),
                Some(value) if value.trim().is_empty() => errs.push(format!(
                    "{PREFIX} {label}.{field}: obligation must be non-empty"
                )),
                Some(value) => {
                    let token = OBLIGATION_TOKENS.contains(&value);
                    if !token && !resolvable(value) {
                        errs.push(format!(
                            "{PREFIX} {label}.{field}: `{value}` is neither a closed obligation token nor a resolvable path"
                        ));
                    }
                }
            }
        }

        if !nonempty(position, "gpl_violation_status") {
            errs.push(format!(
                "{PREFIX} {label}.gpl_violation_status: a typed status is required"
            ));
        }

        match s(position, "boundary") {
            Some(boundary) if BOUNDARY_VOCABULARY.contains(&boundary) => {}
            Some(boundary) => errs.push(format!(
                "{PREFIX} {label}.boundary: `{boundary}` is outside the closed boundary vocabulary"
            )),
            None => errs.push(format!("{PREFIX} {label}.boundary: boundary is missing")),
        }

        let redistribution = s(position, "redistribution");
        match redistribution {
            Some(token) if REDISTRIBUTION_VOCABULARY.contains(&token) => {}
            Some(token) => errs.push(format!(
                "{PREFIX} {label}.redistribution: `{token}` is outside the closed redistribution vocabulary"
            )),
            None => errs.push(format!(
                "{PREFIX} {label}.redistribution: redistribution status is missing"
            )),
        }

        let reason = s(position, "blocked_reason").map(|x| x.trim());
        let reason_present = reason.is_some_and(|x| !x.is_empty());
        if status == Some("blocked") && !reason_present {
            errs.push(format!(
                "{PREFIX} {label}.blocked_reason: a blocked position needs a non-empty refusal reason"
            ));
        }
        if status == Some("verified") && reason_present {
            errs.push(format!(
                "{PREFIX} {label}.blocked_reason: a verified position must not carry a refusal reason"
            ));
        }
        if status == Some("blocked")
            && redistribution.is_some_and(|token| FORBIDDEN_FOR_BLOCKED.contains(&token))
        {
            errs.push(format!(
                "{PREFIX} {label}.redistribution: a blocked position must not declare `{}` — it must not claim permission to redistribute",
                redistribution.unwrap_or_default()
            ));
        }
        if status == Some("verified") {
            match s(position, "license") {
                Some(token) if VERIFIED_LICENCE_TOKENS.contains(&token) => {}
                Some(token) => errs.push(format!(
                    "{PREFIX} {label}.license: `{token}` is outside the closed licence vocabulary allowed for a verified position"
                )),
                None => errs.push(format!(
                    "{PREFIX} {label}.license: a verified position needs a closed licence token"
                )),
            }
        }

        match at(position, "surface_claims") {
            None => errs.push(format!(
                "{PREFIX} {label}.surface_claims: claim list is missing"
            )),
            Some(Yaml::Sequence(items)) => {
                for (claim_idx, item) in items.iter().enumerate() {
                    let surface = s(item, "surface").unwrap_or_default();
                    let target = s(item, "component_id").unwrap_or_default();
                    if surface.trim().is_empty() || target.trim().is_empty() {
                        errs.push(format!(
                            "{PREFIX} {label}.surface_claims[{claim_idx}]: a claim needs a non-empty `surface` and `component_id`"
                        ));
                        continue;
                    }
                    let Some(known) = surfaces.get(surface) else {
                        errs.push(format!(
                            "{PREFIX} {label}.surface_claims[{claim_idx}]: `{surface}` is not a declared canonical surface"
                        ));
                        continue;
                    };
                    if !known.contains(target) {
                        errs.push(format!(
                            "{PREFIX} {label}.surface_claims[{claim_idx}]: `{target}` is not an identifier of surface `{surface}`"
                        ));
                        continue;
                    }
                    let count = claim_counts
                        .entry((surface.to_string(), target.to_string()))
                        .or_insert(0);
                    *count += 1;
                }
            }
            Some(_) => errs.push(format!(
                "{PREFIX} {label}.surface_claims: must be a list of claims"
            )),
        }
    }

    for (surface, ids) in surfaces {
        for id in ids {
            let claimed = claim_counts
                .get(&(surface.clone(), id.clone()))
                .copied()
                .unwrap_or(0);
            if claimed != 1 {
                errs.push(format!(
                    "{PREFIX} cross-surface: `{surface}#{id}` is not claimed exactly once by the canonical ledger (claimed {claimed} times)"
                ));
            }
        }
        let referenced = claim_counts.keys().any(|(key, _)| key == surface);
        if !referenced {
            errs.push(format!(
                "{PREFIX} cross-surface: declared surface `{surface}` is never claimed by a canonical position"
            ));
        }
    }

    errs
}

/// A canonical `verified` position must not be contradicted by the read-only
/// surface it names: if that surface position is blocked, the rights were never
/// established and the canonical ledger must be blocked too (fail-closed). The
/// opposite direction is not flagged: a canonical `blocked` position against a
/// `verified` surface token stays conservative.
fn surface_reconciliation_errs(doc: &Yaml, statuses: &SurfaceStatuses) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(components) = seq(doc, "components") else {
        return errs;
    };
    for position in components {
        let label = entry_id(position).unwrap_or("<unnamed>");
        let canonical = s(position, "license_status").unwrap_or("<missing>");
        let Some(Yaml::Sequence(items)) = at(position, "surface_claims") else {
            continue;
        };
        for item in items {
            let surface = s(item, "surface").unwrap_or_default();
            let target = s(item, "component_id").unwrap_or_default();
            let token = statuses
                .get(surface)
                .and_then(|by_id| by_id.get(target))
                .map(|x| x.as_str());
            if canonical == "verified"
                && token.is_some_and(|t| BLOCKED_SURFACE_STATUSES.contains(&t))
            {
                errs.push(format!(
                    "{PREFIX} verified_position_contradicts_surface_blocked: `{label}` is `verified` while `{surface}#{target}` is `{}`",
                    token.unwrap_or_default()
                ));
            }
        }
    }
    errs
}

/// The raw artifact is scanned as bytes: an untyped status token must not survive
/// in the ledger, not even inside a comment.
fn raw_document_errs(text: &str) -> Vec<String> {
    let mut errs = Vec::new();
    if text.contains(UNAVAILABLE_STATUS_LITERAL) {
        errs.push(format!(
            "{PREFIX} document: raw ledger still contains the untrusted status literal `{UNAVAILABLE_STATUS_LITERAL}`"
        ));
    }
    if text.contains("license_status: unknown") {
        errs.push(format!(
            "{PREFIX} document: raw ledger declares an `unknown` typed status"
        ));
    }
    errs
}

fn expect_err(errs: &[String], fragments: &[&str], case: &str) {
    assert!(
        !errs.is_empty(),
        "{case}: mutation was accepted with no diagnostic"
    );
    for fragment in fragments {
        assert!(
            errs.iter().any(|e| e.contains(fragment)),
            "{case}: no diagnostic mentions `{fragment}`; got {errs:#?}"
        );
    }
}

#[test]
fn canonical_ledger_is_typed_closed_and_cross_bound() {
    let text = read(CANONICAL);
    let doc = yaml(CANONICAL);
    let raw = raw_document_errs(&text);
    assert!(raw.is_empty(), "raw ledger errors: {raw:#?}");

    let surfaces = load_surfaces(&doc);
    assert!(
        surfaces.errs.is_empty(),
        "surface errors: {:#?}",
        surfaces.errs
    );
    for (surface, ids) in &surfaces.ids {
        assert!(
            !ids.is_empty(),
            "{surface}: the read-only surface resolved no identifiers"
        );
    }

    let errs = license_provenance_errs(&doc, &surfaces.ids);
    assert!(errs.is_empty(), "canonical ledger errors: {errs:#?}");
    let reconciled = surface_reconciliation_errs(&doc, &surfaces.statuses);
    assert!(
        reconciled.is_empty(),
        "surface reconciliation errors: {reconciled:#?}"
    );

    // An untouched ledger must produce no diagnostic at all (the fail-closed
    // contract is not allowed to be noisy on the committed artifact).
    let repeat = license_provenance_errs(&doc, &surfaces.ids);
    assert!(repeat.is_empty(), "repeat run errors: {repeat:#?}");
}

#[test]
fn mutation_untyped_status_token_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    set(
        component_mut(&mut mutated, "pdffigures2"),
        "license_status",
        Yaml::String(UNAVAILABLE_STATUS_LITERAL.to_string()),
    );
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &[
            "pdffigures2",
            "license_status",
            "outside the closed vocabulary",
            "untrusted status literal",
        ],
        "untyped status token",
    );
    let serialized = serde_yaml::to_string(&mutated).expect("serializable");
    assert!(
        raw_document_errs(&serialized)
            .iter()
            .any(|e| e.contains(UNAVAILABLE_STATUS_LITERAL)),
        "serialized mutation must trip the raw scan"
    );
}

#[test]
fn mutation_missing_blocked_reason_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    unset(component_mut(&mut mutated, "pdffigures2"), "blocked_reason");
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &["pdffigures2", "blocked_reason", "refusal reason"],
        "missing blocked_reason",
    );
}

#[test]
fn mutation_blocked_position_claiming_redistribution_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    set(
        component_mut(&mut mutated, "pdf-craft-cv-structure"),
        "redistribution",
        Yaml::String("allowed_with_notice".to_string()),
    );
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &[
            "pdf-craft-cv-structure",
            "redistribution",
            "must not claim permission to redistribute",
        ],
        "blocked position claiming redistribution",
    );
}

#[test]
fn mutation_unresolvable_license_evidence_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    set(
        component_mut(&mut mutated, "java-grobid-0.9.0-crf"),
        "license_evidence",
        Yaml::String("vendor/does_not_exist/LICENSE".to_string()),
    );
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &[
            "java-grobid-0.9.0-crf",
            "license_evidence",
            "does not resolve",
        ],
        "unresolvable licence evidence",
    );
}

#[test]
fn mutation_free_text_obligation_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    set(
        component_mut(&mut mutated, "pdffigures2"),
        "attribution_notice_artifact",
        Yaml::String("needs legal review".to_string()),
    );
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &[
            "pdffigures2",
            "attribution_notice_artifact",
            "neither a closed obligation token nor a resolvable path",
        ],
        "free-text obligation",
    );
}

#[test]
fn mutation_removed_surface_claim_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    claims_mut(&mut mutated, "pdfalto_runtime").clear();
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &[
            "data/parse/grobid_model_rights.yaml#assets#pdfalto_runtime",
            "not claimed exactly once",
            "claimed 0 times",
        ],
        "removed surface claim",
    );
}

#[test]
fn mutation_foreign_surface_claim_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    let claims = claims_mut(&mut mutated, "pdfalto_runtime");
    set(
        &mut claims[0],
        "component_id",
        Yaml::String("not_a_declared_component".to_string()),
    );
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &[
            "not_a_declared_component",
            "is not an identifier of surface",
        ],
        "foreign surface claim",
    );
}

#[test]
fn mutation_undeclared_and_duplicate_claims_fail_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);

    let mut undeclared = doc.clone();
    claims_mut(&mut undeclared, "pdf_inspector").push(claim(
        "data/parse/does_not_exist.yaml#license_provenance_inventory",
        "phantom",
    ));
    let errs = license_provenance_errs(&undeclared, &surfaces.ids);
    expect_err(
        &errs,
        &["pdf_inspector", "is not a declared canonical surface"],
        "undeclared surface claim",
    );

    let mut duplicated = doc.clone();
    claims_mut(&mut duplicated, "pdf_inspector").push(claim(
        "data/parse/grobid_compatibility.yaml#license_provenance_inventory",
        "pdffigures2",
    ));
    let errs = license_provenance_errs(&duplicated, &surfaces.ids);
    expect_err(
        &errs,
        &[
            "data/parse/grobid_compatibility.yaml#license_provenance_inventory#pdffigures2",
            "not claimed exactly once",
            "claimed 2 times",
        ],
        "duplicate surface claim",
    );
}

#[test]
fn mutation_redistribution_lock_relaxation_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let cases: [(&str, Yaml); 7] = [
        ("new_distribution", Yaml::String("deferred".to_string())),
        ("graph_write", Yaml::String("allowed".to_string())),
        ("auto_promote", Yaml::String("allowed".to_string())),
        ("gpl_linking", Yaml::String("permitted".to_string())),
        ("unknown_rights", Yaml::String("untyped".to_string())),
        ("import_eligible", Yaml::Bool(true)),
        (
            "blocked_components_must_not_enter_runtime",
            Yaml::Bool(false),
        ),
    ];
    for (key, value) in cases {
        let mut mutated = doc.clone();
        let policy = mutated
            .get_mut("redistribution_policy")
            .expect("policy block");
        set(policy, key, value);
        let errs = license_provenance_errs(&mutated, &surfaces.ids);
        expect_err(
            &errs,
            &["redistribution_policy", key, "lock must stay"],
            &format!("redistribution lock `{key}` relaxed"),
        );
    }
}

#[test]
fn mutation_verified_position_on_blocked_surface_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    let position = component_mut(&mut mutated, "pdf-craft-cv-structure");
    set(
        position,
        "license_status",
        Yaml::String("verified".to_string()),
    );
    set(position, "license", Yaml::String("Apache-2.0".to_string()));
    unset(position, "blocked_reason");
    set(
        position,
        "redistribution",
        Yaml::String("allowed_with_notice".to_string()),
    );

    // The form alone is now satisfied: only the surface reconciliation can see
    // that the named read-only surface still refuses the same component.
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    assert!(
        errs.is_empty(),
        "form-only mutation kept clean, got {errs:#?}"
    );
    let reconciled = surface_reconciliation_errs(&mutated, &surfaces.statuses);
    expect_err(
        &reconciled,
        &[
            "verified_position_contradicts_surface_blocked",
            "pdf-craft-cv-structure",
            "data/parse/grobid_compatibility.yaml#license_provenance_inventory#pdf-craft-cv-structure",
            "is `blocked`",
        ],
        "verified position contradicting a blocked surface",
    );
}

#[test]
fn mutation_verified_with_unknown_licence_token_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut mutated = doc.clone();
    set(
        component_mut(&mut mutated, "java-grobid-0.9.0-crf"),
        "license",
        Yaml::String("unknown".to_string()),
    );
    let errs = license_provenance_errs(&mutated, &surfaces.ids);
    expect_err(
        &errs,
        &["java-grobid-0.9.0-crf", "closed licence vocabulary"],
        "verified position with an unknown licence token",
    );
}

#[test]
fn surface_structural_mutations_fail_closed() {
    let doc = yaml(CANONICAL);
    let (cfgs, cfg_errs) = surface_cfgs(&doc);
    assert!(cfg_errs.is_empty(), "surface config errors: {cfg_errs:#?}");

    let model_rights = cfgs
        .iter()
        .find(|c| c.key == "data/parse/grobid_model_rights.yaml#assets")
        .expect("model rights surface declared");
    let model_doc = yaml(&model_rights.path);

    let mut outside_vocabulary = model_doc.clone();
    set(
        &mut seq_mut(&mut outside_vocabulary, "assets")[0],
        "rights_status",
        Yaml::String("trusted".to_string()),
    );
    let read = surface_doc_errs(model_rights, &outside_vocabulary);
    expect_err(
        &read.errs,
        &[
            "lexical_resources",
            "rights_status",
            "outside the closed surface vocabulary",
        ],
        "surface status token outside the surface vocabulary",
    );

    let mut missing_reason = model_doc.clone();
    unset(&mut seq_mut(&mut missing_reason, "assets")[0], "evidence");
    let read = surface_doc_errs(model_rights, &missing_reason);
    expect_err(
        &read.errs,
        &["lexical_resources", "refusal reason"],
        "blocked surface position without evidence",
    );

    let mut redistributable = model_doc.clone();
    set(
        &mut seq_mut(&mut redistributable, "assets")[0],
        "redistributable",
        Yaml::Bool(true),
    );
    let read = surface_doc_errs(model_rights, &redistributable);
    expect_err(
        &read.errs,
        &["lexical_resources", "redistributable: true"],
        "redistributable blocked surface position",
    );

    let mut missing_inventory = model_doc.clone();
    unset(&mut missing_inventory, &model_rights.inventory_key);
    let read = surface_doc_errs(model_rights, &missing_inventory);
    expect_err(
        &read.errs,
        &["assets", "is missing"],
        "missing surface inventory key",
    );

    let compatibility = cfgs
        .iter()
        .find(|c| c.key == "data/parse/grobid_compatibility.yaml#license_provenance_inventory")
        .expect("compatibility surface declared");
    let compatibility_doc = yaml(&compatibility.path);

    let mut foreign_status = compatibility_doc.clone();
    let inventory = foreign_status
        .get_mut(&compatibility.inventory_key)
        .expect("inventory");
    let components = seq_mut(inventory, "components");
    set(
        &mut components[0],
        "license_status",
        Yaml::String("assumed".to_string()),
    );
    let read = surface_doc_errs(compatibility, &foreign_status);
    expect_err(
        &read.errs,
        &[
            "java-grobid-0.9.0-crf",
            "license_status",
            "outside the closed surface vocabulary",
        ],
        "declared surface vocabulary rejected a foreign token",
    );

    let mut file_level = compatibility_doc.clone();
    set(&mut file_level, "redistributable", Yaml::Bool(true));
    let read = surface_doc_errs(compatibility, &file_level);
    expect_err(
        &read.errs,
        &["redistributable: true", "a position is blocked"],
        "file-level redistributable with blocked positions",
    );

    let mut repaired = model_doc.clone();
    let assets = seq_mut(&mut repaired, "assets");
    set(
        &mut assets[0],
        "rights_status",
        Yaml::String("verified".to_string()),
    );
    set(
        &mut assets[0],
        "blocked_reason",
        Yaml::String("stale".to_string()),
    );
    let read = surface_doc_errs(model_rights, &repaired);
    expect_err(
        &read.errs,
        &["lexical_resources", "must not carry a refusal reason"],
        "verified surface position with a refusal reason",
    );
}

#[test]
fn raw_scan_rejects_untyped_status_even_in_a_comment() {
    let clean = read(CANONICAL);
    assert!(
        raw_document_errs(&clean).is_empty(),
        "committed ledger must stay free of untyped status tokens"
    );
    let comment = format!("# no {UNAVAILABLE_STATUS_LITERAL} tokens remain\n{}", clean);
    expect_err(
        &raw_document_errs(&comment),
        &["untrusted status literal"],
        "untyped token hidden in a comment",
    );
    expect_err(
        &raw_document_errs("license_status: unknown\n"),
        &["`unknown` typed status"],
        "explicit unknown typed status",
    );
}

#[test]
fn missing_surface_file_and_declaration_drift_fail_closed() {
    let doc = yaml(CANONICAL);

    let mut removed = doc.clone();
    {
        let map = surfaces_map_mut(&mut removed);
        map.remove(Yaml::String(REQUIRED_SURFACES[0].to_string()));
        map.insert(
            Yaml::String("data/parse/does_not_exist.yaml#inventory".to_string()),
            surface_decl("phantom_surface", "inventory", "license_status"),
        );
    }
    let loaded = load_surfaces(&removed);
    expect_err(
        &loaded.errs,
        &["required read-only surface", "is not declared"],
        "required surface declaration removed",
    );
    expect_err(
        &loaded.errs,
        &["data/parse/does_not_exist.yaml", "does not exist"],
        "surface file missing on disk",
    );

    let mut wrong_dialect = doc.clone();
    set(
        surface_decl_mut(&mut wrong_dialect, REQUIRED_SURFACES[2]),
        "status_field",
        Yaml::String("license_status".to_string()),
    );
    let loaded = load_surfaces(&wrong_dialect);
    expect_err(
        &loaded.errs,
        &[
            "lexical_resources",
            "license_status",
            "typed status is missing",
        ],
        "surface read under the wrong status dialect",
    );

    let mut unknown_dialect = doc.clone();
    set(
        surface_decl_mut(&mut unknown_dialect, REQUIRED_SURFACES[1]),
        "status_field",
        Yaml::String("ownership_status".to_string()),
    );
    let (_, cfg_errs) = surface_cfgs(&unknown_dialect);
    expect_err(
        &cfg_errs,
        &["ownership_status", "outside the known status dialects"],
        "unknown surface status dialect",
    );

    let mut malformed_key = doc.clone();
    surfaces_map_mut(&mut malformed_key).insert(
        Yaml::String("data/parse/grobid_model_rights.yaml".to_string()),
        surface_decl("phantom_surface", "assets", "rights_status"),
    );
    let (_, cfg_errs) = surface_cfgs(&malformed_key);
    expect_err(
        &cfg_errs,
        &["<repo-relative file>#<inventory key>"],
        "surface key without an inventory key",
    );

    let mut absolute_path = doc.clone();
    surfaces_map_mut(&mut absolute_path).insert(
        Yaml::String("/etc/hostname#assets".to_string()),
        surface_decl("phantom_surface", "assets", "rights_status"),
    );
    let (_, cfg_errs) = surface_cfgs(&absolute_path);
    expect_err(
        &cfg_errs,
        &["/etc/hostname#assets", "must be repo-relative"],
        "absolute surface path",
    );
}

#[test]
fn mutation_field_typing_fails_closed() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);

    let mut duplicate_identity = doc.clone();
    set(
        component_mut(&mut duplicate_identity, "ruvector"),
        "component_id",
        Yaml::String("pdf_inspector".to_string()),
    );
    expect_err(
        &license_provenance_errs(&duplicate_identity, &surfaces.ids),
        &["component_id is declared more than once"],
        "duplicate component identity",
    );

    let mut no_gpl = doc.clone();
    unset(
        component_mut(&mut no_gpl, "glove_embeddings"),
        "gpl_violation_status",
    );
    expect_err(
        &license_provenance_errs(&no_gpl, &surfaces.ids),
        &[
            "glove_embeddings",
            "gpl_violation_status",
            "a typed status is required",
        ],
        "missing gpl_violation_status",
    );

    let mut foreign_boundary = doc.clone();
    set(
        component_mut(&mut foreign_boundary, "delft_models"),
        "boundary",
        Yaml::String("kernel-module".to_string()),
    );
    expect_err(
        &license_provenance_errs(&foreign_boundary, &surfaces.ids),
        &[
            "delft_models",
            "boundary",
            "outside the closed boundary vocabulary",
        ],
        "boundary outside the closed vocabulary",
    );

    let mut foreign_redistribution = doc.clone();
    set(
        component_mut(&mut foreign_redistribution, "corpus.internal_gold"),
        "redistribution",
        Yaml::String("permitted".to_string()),
    );
    expect_err(
        &license_provenance_errs(&foreign_redistribution, &surfaces.ids),
        &[
            "corpus.internal_gold",
            "redistribution",
            "outside the closed redistribution vocabulary",
        ],
        "redistribution outside the closed vocabulary",
    );

    let mut drifted_schema = doc.clone();
    set(
        &mut drifted_schema,
        "schema_version",
        Yaml::String("parse-component-license-inventory.v9".to_string()),
    );
    expect_err(
        &license_provenance_errs(&drifted_schema, &surfaces.ids),
        &["schema_version", "expected"],
        "schema version drift",
    );

    let mut softened_vocabulary = doc.clone();
    set(
        &mut softened_vocabulary,
        "license_status_vocabulary",
        Yaml::Sequence(vec![
            Yaml::String("verified".to_string()),
            Yaml::String("blocked".to_string()),
            Yaml::String("assumed".to_string()),
        ]),
    );
    expect_err(
        &license_provenance_errs(&softened_vocabulary, &surfaces.ids),
        &["license_status_vocabulary", "closed vocabulary drift"],
        "softened status vocabulary",
    );

    let mut no_policy = doc.clone();
    unset(&mut no_policy, "redistribution_policy");
    expect_err(
        &license_provenance_errs(&no_policy, &surfaces.ids),
        &["redistribution_policy", "lock block is missing"],
        "removed policy lock block",
    );
}

#[test]
fn ledger_keeps_the_expected_read_only_bindings() {
    let doc = yaml(CANONICAL);
    let surfaces = load_surfaces(&doc);
    let mut claimed: BTreeSet<(String, String)> = BTreeSet::new();
    for position in seq(&doc, "components").expect("components") {
        let Some(Yaml::Sequence(items)) = at(position, "surface_claims") else {
            continue;
        };
        for item in items {
            claimed.insert((
                s(item, "surface").unwrap_or_default().to_string(),
                s(item, "component_id").unwrap_or_default().to_string(),
            ));
        }
    }
    let mut expected: BTreeSet<(String, String)> = BTreeSet::new();
    for (surface, ids) in &surfaces.ids {
        for id in ids {
            expected.insert((surface.clone(), id.clone()));
        }
    }
    assert_eq!(
        claimed, expected,
        "the canonical ledger must claim every read-only surface identifier exactly once"
    );
    for required in REQUIRED_SURFACES {
        assert!(
            surfaces.ids.contains_key(required),
            "{required}: required read-only surface disappeared"
        );
    }
    let _ = component(&doc, "pdfalto_runtime").expect("pdfalto_runtime position");
}
