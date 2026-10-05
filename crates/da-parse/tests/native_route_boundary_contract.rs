//! Contract tests for the S06/T02 native-route boundary, scholarly-endpoint and
//! consolidation-scope DATA declared in the Governor rule
//! `scripts/governor/checks/grobid_absorption_progress.yaml#native_route_e2e`.
//!
//! The Governor block declares the *boundary* of the S06 PDF->native route, the
//! scholarly endpoints it must stay compatible with, and the consolidation scope
//! it runs. It deliberately restates none of those things: the slice boundary
//! resolves from `data/parse/layout_bakeoff.yaml#java_free_vertical_slice`, the
//! stage ids from `data/parse/grobid_compatibility.yaml#defaults.cascade_stages`,
//! the consolidation profile from `defaults.profiles.structural`, and every
//! endpoint from its own ledger entry. These tests prove that:
//!
//! * the declaration binding resolves by pointer and stays the SOLE source of the
//!   boundary (a second contract file is forbidden);
//! * P0..P9 are total and pairwise disjoint across admission / in-slice /
//!   out-of-slice, and every stage name resolves by pointer — no stage id is
//!   restated inside the Governor block;
//! * the consolidation scope is the structural profile with both flags off, the
//!   forbidden neighbour is `rich` (1/1), and the fingerprint field order equals
//!   the live `configuration_fingerprint` literal in `crates/da-parse/src/profile.rs`;
//! * every scholarly-endpoint declaration resolves to a real ledger entry by
//!   pointer and carries no URL/route restatement;
//! * the governor corpus binding still recomputes from the pinned layout
//!   documents (3 / 202), and the route's own `native_route_corpus` block
//!   resolves to its expanded twelve documents / 381 pages;
//! * the limit freeze is declared `frozen` with a shaped revision (the composite
//!   arithmetic is owned by `scripts/tests/test_absorption_progress_native_route_freeze.py`);
//! * five named negative mutations (restated stage list, unknown endpoint,
//!   unclassified stage, shifted consolidation flag, second contract file) are
//!   rejected by name.
//!
//! This test reads only committed DATA; it never reads gitignored `data/sources/**`.

use serde_yaml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const RULE: &str = "scripts/governor/checks/grobid_absorption_progress.yaml";
const BLOCK: &str = "native_route_e2e";
const LAYOUT_BAKEOFF: &str = "data/parse/layout_bakeoff.yaml";
const COMPATIBILITY: &str = "data/parse/grobid_compatibility.yaml";
const LEDGER: &str = "data/parse/api_compatibility_ledger.yaml";
const PROFILE_SURFACE: &str = "crates/da-parse/src/profile.rs";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_text(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn yaml(path: &str) -> Value {
    serde_yaml::from_str(&read_text(path)).unwrap_or_else(|e| panic!("{path} is valid YAML: {e}"))
}

// ─────────────────────────── pointer resolution ────────────────────────────

/// Resolve a pointer in either dialect the slice DATA actually uses: the
/// slash-separated form (optionally selecting a list item by `key[field=value]`)
/// and the dot-separated form the layout-bakeoff/compatibility authority uses.
fn resolve<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    if let Some(found) = resolve_slash(value, pointer) {
        return Some(found);
    }
    if pointer.contains('.') {
        return resolve_slash(value, &pointer.replace('.', "/"));
    }
    None
}

fn resolve_slash<'a>(value: &'a Value, pointer: &str) -> Option<&'a Value> {
    let mut node = value;
    for raw in pointer.split('/') {
        let segment = raw.trim();
        if segment.is_empty() {
            return None;
        }
        if let Some(open) = segment.find('[') {
            let inner = segment.strip_suffix(']')?;
            let key = &inner[..open];
            let (field, wanted) = inner[open + 1..].split_once('=')?;
            let candidates = node.get(key)?.as_sequence()?;
            node = candidates
                .iter()
                .find(|item| item.get(field).and_then(Value::as_str) == Some(wanted))?;
        } else {
            node = node.get(segment)?;
        }
    }
    Some(node)
}

// ─────────────────────────── live surface helpers ──────────────────────────

/// The field order of the frozen D204 fingerprint, read out of the live
/// `configuration_fingerprint` literal rather than a second copy of the list.
fn fingerprint_field_order(profile_src: &str) -> Vec<String> {
    let start = profile_src
        .find("pub fn configuration_fingerprint")
        .expect("configuration_fingerprint exists in profile.rs");
    let rest = &profile_src[start..];
    let after_format = &rest[rest.find("format!(").expect("format! literal") + "format!(".len()..];
    let quote_open = after_format.find('"').expect("format literal opens");
    let literal = &after_format[quote_open + 1..];
    let literal = &literal[..literal.find('"').expect("format literal closes")];
    literal
        .split(';')
        .filter_map(|piece| {
            let name = piece.split('=').next()?.trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

struct Fixtures {
    rule: Value,
    layout: Value,
    compatibility: Value,
    ledger: Value,
    profile_src: String,
}

impl Fixtures {
    fn load() -> Self {
        Self {
            rule: yaml(RULE),
            layout: yaml(LAYOUT_BAKEOFF),
            compatibility: yaml(COMPATIBILITY),
            ledger: yaml(LEDGER),
            profile_src: read_text(PROFILE_SURFACE),
        }
    }

    fn validate(&self) -> Vec<String> {
        native_route_boundary_errs(self)
    }
}

fn block(f: &Fixtures) -> &Value {
    &f.rule[BLOCK]
}

fn mutate_block<F: FnOnce(&mut Value)>(fixtures: &mut Fixtures, mutate: F) {
    let mut block = fixtures.rule[BLOCK].clone();
    mutate(&mut block);
    fixtures.rule[BLOCK] = block;
}

fn mutate_slice<F: FnOnce(&mut Value)>(fixtures: &mut Fixtures, mutate: F) {
    let mut slice = fixtures.layout["java_free_vertical_slice"].clone();
    mutate(&mut slice);
    fixtures.layout["java_free_vertical_slice"] = slice;
}

// ───────────────────────────── the contract ────────────────────────────────

fn native_route_boundary_errs(f: &Fixtures) -> Vec<String> {
    let mut errs = Vec::new();
    let block = block(f);
    if block.is_null() {
        errs.push(format!("[native-route-boundary] {BLOCK} block is missing"));
        return errs;
    }

    // 1. Declaration binding by pointer, and the slice stays the sole source.
    check_authority(f, &mut errs);

    let slice = &f.layout["java_free_vertical_slice"];
    let cascade: BTreeSet<String> = match f.compatibility["defaults"]["cascade_stages"] {
        Value::Mapping(ref map) => map
            .keys()
            .filter_map(|key| key.as_str().map(str::to_string))
            .collect(),
        _ => BTreeSet::new(),
    };
    if cascade.is_empty() {
        errs.push("[native-route-stage] cascade stage set is unreadable".into());
        return errs;
    }

    // 2. Totality + disjointness over admission / in-slice / out-of-slice, and
    //    every stage name resolved by pointer.
    let in_ids = stage_id_set(&slice["stages_in_slice"]);
    let out_ids = stage_id_set(&slice["stages_out_of_slice"]);
    let admission_id = slice["identity_admission_stage"]["stage_id"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if !in_ids.is_disjoint(&out_ids) {
        errs.push("[native-route-stage] a stage is both in and out of the slice".into());
    }
    if admission_id.is_empty() || in_ids.contains(&admission_id) || out_ids.contains(&admission_id)
    {
        errs.push(
            "[native-route-stage] the admission stage must be classified exactly once".into(),
        );
    }
    let mut covered: BTreeSet<String> = in_ids.union(&out_ids).cloned().collect();
    covered.insert(admission_id.clone());
    if covered != cascade {
        errs.push(
            "[native-route-stage] admission + in-slice + out-of-slice must cover every cascade stage exactly once"
                .into(),
        );
    }
    for stage in stage_entries(slice) {
        let pointer = stage["name_pointer"].as_str().unwrap_or("");
        if pointer.is_empty()
            || resolve(&f.compatibility, pointer)
                .and_then(Value::as_str)
                .is_none()
        {
            errs.push(format!(
                "[native-route-stage] stage `{}` name pointer `{pointer}` does not resolve",
                stage["stage_id"].as_str().unwrap_or("<unnamed>")
            ));
        }
    }
    // The Governor block must not restate the stage list: no cascade id may
    // appear as a token anywhere inside it.
    let mut restated = BTreeSet::new();
    collect_stage_tokens(block, &cascade, &mut restated);
    if !restated.is_empty() {
        errs.push(format!(
            "[native-route-stage] stage ids are restated in the block: {restated:?}"
        ));
    }

    // 3. Consolidation scope: structural, both flags off, fingerprint order live.
    check_consolidation(f, &mut errs);

    // 4. Scholarly endpoints: closed vocabulary, pointer-bound, no URL restatement.
    check_endpoints(f, &mut errs);

    // 5. Corpus: recomputed from the pinned documents.
    check_corpus(f, &mut errs);

    // 6. Freeze shape (composite arithmetic is owned by the Python gate).
    let revision = &block["limit_policy"]["freeze_revision"]["value"];
    if block["limit_policy"]["state"].as_str() != Some("frozen")
        || revision
            .as_str()
            .map(|v| !v.starts_with("sha256:") || v.len() <= "sha256:".len())
            .unwrap_or(true)
    {
        errs.push(
            "[native-route-freeze] the limit policy must be frozen with a shaped sha256 revision"
                .into(),
        );
    }

    errs
}

fn check_authority(f: &Fixtures, errs: &mut Vec<String>) {
    let block = block(f);
    if block["declaration"]["manifest"].as_str() != Some(LAYOUT_BAKEOFF)
        || block["declaration"]["pointer"].as_str() != Some("java_free_vertical_slice")
        || resolve(&f.layout, "java_free_vertical_slice")
            .filter(|v| v.is_mapping())
            .is_none()
    {
        errs.push("[native-route-boundary] declaration must bind the slice by pointer".into());
    }
    if block["stage_id_source"]["manifest"].as_str() != Some(COMPATIBILITY)
        || block["stage_id_source"]["pointer"].as_str() != Some("defaults.cascade_stages")
    {
        errs.push(
            "[native-route-boundary] stage ids must resolve from the cascade DATA by pointer"
                .into(),
        );
    }
    let authority = &f.layout["java_free_vertical_slice"]["authority"];
    if authority["declaration_manifest"].as_str() != Some(LAYOUT_BAKEOFF)
        || authority["declaration_pointer"].as_str() != Some("java_free_vertical_slice")
        || authority["sole_source"].as_bool() != Some(true)
        || authority["second_contract_file"].as_str() != Some("forbidden")
    {
        errs.push(
            "[native-route-authority] the declaration must stay the sole source of the boundary"
                .into(),
        );
    }
}

fn check_consolidation(f: &Fixtures, errs: &mut Vec<String>) {
    let block = block(f);
    let consolidation = &block["consolidation"];
    if consolidation["manifest"].as_str() != Some(COMPATIBILITY)
        || consolidation["pointer"].as_str() != Some("defaults.profiles.structural")
        || consolidation["profile"].as_str() != Some("structural")
    {
        errs.push(
            "[native-route-consolidation] consolidation must bind the structural profile by pointer"
                .into(),
        );
    }
    let resolved = resolve(&f.compatibility, "defaults.profiles.structural");
    let resolved_header = resolved.and_then(|p| p["consolidateHeader"].as_i64());
    let resolved_citations = resolved.and_then(|p| p["consolidateCitations"].as_i64());
    if consolidation["consolidateHeader"].as_i64() != Some(0)
        || consolidation["consolidateCitations"].as_i64() != Some(0)
        || resolved_header != Some(0)
        || resolved_citations != Some(0)
    {
        errs.push(
            "[native-route-consolidation] both consolidation flags must be 0 (structural)".into(),
        );
    }
    let declared_fields: Vec<String> = consolidation["fingerprint_fields"]
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if declared_fields != fingerprint_field_order(&f.profile_src) {
        errs.push(
            "[native-route-consolidation] fingerprint field set must equal the live D204 literal"
                .into(),
        );
    }
    let rich = resolve(&f.compatibility, "defaults.profiles.rich");
    if consolidation["forbidden_profile"].as_str() != Some("rich")
        || rich.and_then(|p| p["consolidateHeader"].as_i64()) != Some(1)
        || rich.and_then(|p| p["consolidateCitations"].as_i64()) != Some(1)
    {
        errs.push(
            "[native-route-consolidation] the forbidden neighbour must be the rich 1/1 profile"
                .into(),
        );
    }
}

fn check_endpoints(f: &Fixtures, errs: &mut Vec<String>) {
    let endpoints = &block(f)["scholarly_endpoints"];
    let vocabulary: BTreeSet<&str> = endpoints["state_vocabulary"]
        .as_sequence()
        .map(|seq| seq.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if vocabulary != BTreeSet::from(["required", "not_required", "deferred"]) {
        errs.push("[native-route-endpoint] the state vocabulary must be closed".into());
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for declaration in endpoints["declarations"]
        .as_sequence()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let id = declaration["endpoint_id"].as_str().unwrap_or("<unnamed>");
        if !seen.insert(id.to_string()) {
            errs.push(format!(
                "[native-route-endpoint] endpoint `{id}` is declared twice"
            ));
        }
        match declaration["state"].as_str() {
            Some(state) if vocabulary.contains(state) => {}
            other => errs.push(format!(
                "[native-route-endpoint] endpoint `{id}` state `{other:?}` is outside the vocabulary"
            )),
        }
        if declaration["reason"]
            .as_str()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            errs.push(format!(
                "[native-route-endpoint] endpoint `{id}` carries no reason"
            ));
        }
        let manifest = declaration["manifest"].as_str().unwrap_or("");
        let pointer = declaration["pointer"].as_str().unwrap_or("");
        if !Path::new(&repo_root().join(manifest)).exists() {
            errs.push(format!(
                "[native-route-endpoint] endpoint `{id}` manifest `{manifest}` does not exist"
            ));
            continue;
        }
        let resolved_id = resolve(&f.ledger, pointer).and_then(|entry| entry["id"].as_str());
        if resolved_id != Some(id) {
            errs.push(format!(
                "[native-route-endpoint] endpoint `{id}` pointer `{pointer}` resolves to {resolved_id:?}"
            ));
        }
        if restates_url(declaration) {
            errs.push(format!(
                "[native-route-endpoint] endpoint `{id}` must not restate a URL or route"
            ));
        }
    }
}

/// Whether a declaration restates a URL/route: a `url`/`route`/`http_*` key, or
/// any string value carrying `http`.
fn restates_url(declaration: &Value) -> bool {
    if let Value::Mapping(map) = declaration {
        for (key, value) in map {
            if let Some(name) = key.as_str()
                && (name == "url" || name == "route" || name.starts_with("http"))
            {
                return true;
            }
            if value
                .as_str()
                .map(|text| text.contains("http"))
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

fn check_corpus(f: &Fixtures, errs: &mut Vec<String>) {
    let corpus = &block(f)["corpus"];
    let documents = resolve(&f.layout, "corpus.documents");
    let count = documents.and_then(Value::as_sequence).map(Vec::len);
    let pages: i64 = documents
        .and_then(Value::as_sequence)
        .map(|seq| seq.iter().filter_map(|doc| doc["pages"].as_i64()).sum())
        .unwrap_or(-1);
    let declared_count = corpus["document_count"].as_u64();
    let declared_pages = corpus["page_denominator"].as_u64();
    if corpus["manifest"].as_str() != Some(LAYOUT_BAKEOFF)
        || corpus["pointer"].as_str() != Some("corpus.documents")
        || count != declared_count.map(|n| n as usize)
        || pages < 0
        || Some(pages as u64) != declared_pages
    {
        errs.push(
            "[native-route-corpus] document_count / page_denominator must recompute from the pinned documents"
                .into(),
        );
    }
    if resolve(&f.layout, "corpus.corpus_n").and_then(Value::as_u64) != declared_count
        || resolve(&f.layout, "corpus.page_denominator").and_then(Value::as_u64) != declared_pages
    {
        errs.push(
            "[native-route-corpus] counts must agree with the manifest's own corpus_n / page_denominator"
                .into(),
        );
    }
}

fn stage_entries(slice: &Value) -> Vec<&Value> {
    let mut entries: Vec<&Value> = Vec::new();
    if let Some(seq) = slice["stages_in_slice"].as_sequence() {
        entries.extend(seq.iter());
    }
    if let Some(seq) = slice["stages_out_of_slice"].as_sequence() {
        entries.extend(seq.iter());
    }
    if slice["identity_admission_stage"].is_mapping() {
        entries.push(&slice["identity_admission_stage"]);
    }
    entries
}

fn stage_id_set(value: &Value) -> BTreeSet<String> {
    value
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|stage| stage["stage_id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Collect every string leaf token of `value` that is a declared cascade stage
/// id, so a restated stage list inside the Governor block is visible.
fn collect_stage_tokens(value: &Value, ids: &BTreeSet<String>, found: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            for token in text.split(|c: char| !c.is_ascii_alphanumeric()) {
                if ids.contains(token) {
                    found.insert(token.to_string());
                }
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                collect_stage_tokens(item, ids, found);
            }
        }
        Value::Mapping(map) => {
            for (key, child) in map {
                collect_stage_tokens(key, ids, found);
                collect_stage_tokens(child, ids, found);
            }
        }
        _ => {}
    }
}

// ──────────────────────────────── tests ────────────────────────────────────

#[test]
fn native_route_e2e_block_is_bound_to_the_live_carriers() {
    let fixtures = Fixtures::load();
    let errs = fixtures.validate();
    assert!(
        errs.is_empty(),
        "the governor block must be clean: {errs:?}"
    );

    // The governor block's layout binding still resolves to three / 202. The
    // route's OWN corpus is the expanded twelve / 381, pinned separately.
    let governor_documents = resolve(&fixtures.layout, "corpus.documents")
        .and_then(Value::as_sequence)
        .expect("corpus.documents resolves");
    assert_eq!(governor_documents.len(), 3);
    assert_eq!(
        governor_documents
            .iter()
            .filter_map(|doc| doc["pages"].as_i64())
            .sum::<i64>(),
        202
    );
    let route_documents = resolve(&fixtures.layout, "native_route_corpus.documents")
        .and_then(Value::as_sequence)
        .expect("native_route_corpus.documents resolves");
    assert_eq!(route_documents.len(), 12);
    assert_eq!(
        route_documents
            .iter()
            .filter_map(|doc| doc["pages"].as_i64())
            .sum::<i64>(),
        381
    );

    // The endpoints resolve to real ledger entries, not to copies of their URLs.
    for declaration in block(&fixtures)["scholarly_endpoints"]["declarations"]
        .as_sequence()
        .expect("endpoint declarations")
    {
        let id = declaration["endpoint_id"].as_str().expect("endpoint_id");
        let resolved = resolve(
            &fixtures.ledger,
            declaration["pointer"].as_str().expect("endpoint pointer"),
        )
        .expect("endpoint pointer resolves");
        assert_eq!(resolved["id"].as_str(), Some(id));
        assert!(
            declaration["url"].is_null() && declaration["route"].is_null(),
            "endpoint `{id}` must not restate a URL"
        );
    }

    // The live profile surface really is the fingerprint field authority.
    assert_eq!(
        fingerprint_field_order(&fixtures.profile_src),
        vec![
            "profile",
            "consolidateHeader",
            "consolidateCitations",
            "teiCoordinates"
        ]
    );
}

#[test]
fn native_route_boundary_mutations_fail_visible() {
    // 1. A restated stage list inside the Governor block.
    let mut fixtures = Fixtures::load();
    mutate_block(&mut fixtures, |block| {
        block["stages"] =
            Value::Sequence(vec![Value::String("P1".into()), Value::String("P2".into())]);
    });
    let errs = fixtures.validate();
    assert!(
        errs.iter().any(|e| e.starts_with("[native-route-stage]")),
        "a restated stage list must fail by name: {errs:?}"
    );

    // 2. An unknown endpoint id that its pointer cannot resolve to.
    let mut fixtures = Fixtures::load();
    mutate_block(&mut fixtures, |block| {
        block["scholarly_endpoints"]["declarations"][0]["endpoint_id"] =
            Value::String("parse.endpoint.process_unknown".into());
    });
    let errs = fixtures.validate();
    assert!(
        errs.iter()
            .any(|e| e.starts_with("[native-route-endpoint]")),
        "an unknown endpoint must fail by name: {errs:?}"
    );

    // 3. An unclassified stage (dropped from out-of-slice).
    let mut fixtures = Fixtures::load();
    mutate_slice(&mut fixtures, |slice| {
        slice["stages_out_of_slice"]
            .as_sequence_mut()
            .expect("out-of-slice stages")
            .pop();
    });
    let errs = fixtures.validate();
    assert!(
        errs.iter().any(|e| e.starts_with("[native-route-stage]")),
        "an unclassified stage must fail by name: {errs:?}"
    );

    // 4. A consolidation flag shifted by one.
    let mut fixtures = Fixtures::load();
    mutate_block(&mut fixtures, |block| {
        block["consolidation"]["consolidateHeader"] = Value::Number(1.into());
    });
    let errs = fixtures.validate();
    assert!(
        errs.iter()
            .any(|e| e.starts_with("[native-route-consolidation]")),
        "a shifted consolidation flag must fail by name: {errs:?}"
    );

    // 5. A second contract file for the boundary.
    let mut fixtures = Fixtures::load();
    mutate_slice(&mut fixtures, |slice| {
        slice["authority"]["second_contract_file"] = Value::String("allowed".into());
    });
    let errs = fixtures.validate();
    assert!(
        errs.iter()
            .any(|e| e.starts_with("[native-route-authority]")),
        "a second contract file must fail by name: {errs:?}"
    );
}
