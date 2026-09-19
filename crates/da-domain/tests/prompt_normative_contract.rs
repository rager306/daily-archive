//! Prompt change-control contract for M510/S02/T01.
//!
//! The summarizer system prompt has a **single authoritative source**:
//! `data/prompts/summarizer/system.v1.md` — the file the executed agent
//! manifest binds (`data/agents/summarizer/manifest.yaml`). The frozen
//! prompt-eval baseline `data/prompts/article-summary/baseline.v1.md` is an
//! explicitly **derived copy**: it is exactly the documented derivation
//! header followed by the authoritative source bytes.
//!
//! These tests fail closed when the derived copy is not exactly
//! `DERIVED_HEADER + source`, so the duplicated prompt source can never
//! silently diverge again. The header is the only tolerated difference;
//! every other byte must come from the source.
//!
//! Lives under `crates/da-domain/tests/` so the governor source scanners
//! (which exclude `/tests/`) do not read the literal markers below as
//! production DATA.

use da_domain::agent_manifest::{
    AgentManifest, NormativeKind, NormativeRegister, NormativeStrength, ParamValue, Verification,
};
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Executed, authoritative prompt source (bound by the agent manifest).
const SOURCE_PATH: &str = "data/prompts/summarizer/system.v1.md";
/// Derived copy kept for the frozen prompt-eval fixture.
const DERIVED_PATH: &str = "data/prompts/article-summary/baseline.v1.md";
/// The manifest that binds the executed prompt for the summarizer agent.
const EXECUTED_MANIFEST_PATH: &str = "data/agents/summarizer/manifest.yaml";
/// Frozen eval fixture that names the derived copy as its baseline prompt.
const FROZEN_EVAL_PATH: &str = "governor/fixtures/prompt_eval/article-summary.eval.v1.json";

/// Machine-readable prompt normative register (M510/S02/T02).
const NORMATIVES_PATH: &str = "data/prompts/normatives.yaml";
/// Candidate summarizer prompt that carries the optimizer-immutable markers.
const CANDIDATE_SUMMARIZER_PROMPT_PATH: &str = "data/prompts/summarizer/system.v2.md";
/// Executed critic prompt (bound by the critic manifest).
const CRITIC_PROMPT_PATH: &str = "data/prompts/critic/system.v1.md";
/// Executed critic agent manifest.
const EXECUTED_CRITIC_MANIFEST_PATH: &str = "data/agents/critic/manifest.yaml";
/// Boundary marker an optimizer may not rewrite (present on marked prompts).
const OPTIMIZER_IMMUTABLE_MARKER: &str = "<!-- OPTIMIZER-IMMUTABLE: proof, authority, evidence -->";

/// Documented derivation header. The derived copy must be exactly this header
/// followed by the authoritative source bytes; nothing else is tolerated.
const DERIVED_HEADER: &str = concat!(
    "<!-- DERIVED-FROM: data/prompts/summarizer/system.v1.md -->\n",
    "<!-- PROMPT-CONTRACT: PR-SUM-001, PR-SUM-002 -->\n",
    "<!-- IMMUTABLE: grounding, no_invention, evidence_binding; wording may be optimized only -->\n",
    "<!-- These grounding and no-invention obligations are proof-bound and must survive prompt optimization or editing. -->\n",
);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_repo_file(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("contract input {rel} must be readable: {e}"))
}

fn sha256_hex(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// The contract as a pure function: derived == documented header + source.
fn expected_derived(source: &str) -> String {
    format!("{DERIVED_HEADER}{source}")
}

/// Fail-closed drift predicate shared by the real-file test and the
/// negative tamper test.
fn derived_matches_source(derived: &str, source: &str) -> bool {
    derived == expected_derived(source)
}

#[test]
fn derived_copy_equals_documented_header_plus_authoritative_source() {
    let source = read_repo_file(SOURCE_PATH);
    let derived = read_repo_file(DERIVED_PATH);

    assert!(
        !source.is_empty(),
        "authoritative source {SOURCE_PATH} must not be empty"
    );
    assert!(
        derived_matches_source(&derived, &source),
        "derived copy {DERIVED_PATH} drifted from {SOURCE_PATH}: expected exactly \
         the documented derivation header followed by the source bytes"
    );
    // The body after the documented header is byte-identical to the source.
    let body = derived
        .strip_prefix(DERIVED_HEADER)
        .expect("derived copy must start with the documented header");
    assert_eq!(
        body, source,
        "derived body must equal the authoritative source"
    );
}

#[test]
fn derivation_header_declares_source_and_immutable_markers() {
    let derived = read_repo_file(DERIVED_PATH);
    assert!(
        derived.starts_with(DERIVED_HEADER),
        "derived copy must begin with the documented derivation header"
    );
    let declared: Vec<&str> = derived
        .lines()
        .filter_map(|l| l.strip_prefix("<!-- DERIVED-FROM: "))
        .map(|l| l.trim_end_matches(" -->"))
        .collect();
    assert_eq!(
        declared,
        vec![SOURCE_PATH],
        "derived copy must declare exactly one authoritative source"
    );
    assert!(
        derived.contains("<!-- PROMPT-CONTRACT: PR-SUM-001, PR-SUM-002 -->"),
        "derived copy must carry the prompt-contract marker"
    );
    assert!(
        derived.contains("<!-- IMMUTABLE: grounding, no_invention, evidence_binding"),
        "derived copy must carry the immutable-obligation marker"
    );
    assert!(
        derived.contains("proof-bound and must survive prompt optimization or editing"),
        "derived copy must keep the proof-bound obligation statement"
    );
}

#[test]
fn authoritative_source_is_not_marked_as_derived() {
    let source = read_repo_file(SOURCE_PATH);
    assert!(
        !source.contains("DERIVED-FROM"),
        "{SOURCE_PATH} is the authoritative source and must not declare itself derived"
    );
    assert!(
        !source.contains("PROMPT-CONTRACT:"),
        "{SOURCE_PATH} must not carry the derived-copy contract header"
    );
}

#[test]
fn executed_summarizer_manifest_binds_the_authoritative_source() {
    let manifest_yaml = read_repo_file(EXECUTED_MANIFEST_PATH);
    let manifest =
        AgentManifest::from_yaml_str(&manifest_yaml).expect("summarizer agent manifest must parse");
    assert_eq!(
        manifest.prompt.system_file, SOURCE_PATH,
        "the executed summarizer prompt must be the authoritative source, not the derived copy"
    );
    assert!(
        manifest.check_locks().is_ok(),
        "optimizer lock must stay closed for the prompt-normative contract"
    );
}

#[test]
fn frozen_eval_baseline_digest_tracks_the_authoritative_source() {
    // The frozen eval fixture names the derived copy as its baseline prompt,
    // but the recorded digest is the digest of the authoritative source body
    // (the exported/executed prompt). Pin both facts so a source edit without
    // re-freezing the eval is a visible failure rather than silent drift.
    let fixture: serde_json::Value = serde_json::from_str(&read_repo_file(FROZEN_EVAL_PATH))
        .expect("frozen prompt-eval fixture must be valid JSON");
    let baseline = fixture
        .get("baseline")
        .expect("frozen fixture must declare a baseline");
    assert_eq!(
        baseline.get("prompt_file").and_then(|v| v.as_str()),
        Some(DERIVED_PATH),
        "frozen baseline must name the derived copy"
    );
    let recorded = baseline
        .get("prompt_sha256")
        .and_then(|v| v.as_str())
        .expect("frozen baseline must record prompt_sha256");
    assert_eq!(
        recorded,
        sha256_hex(&read_repo_file(SOURCE_PATH)),
        "frozen baseline digest must equal the authoritative source digest"
    );
}

#[test]
fn drift_detection_is_fail_closed() {
    let source = read_repo_file(SOURCE_PATH);
    let derived = read_repo_file(DERIVED_PATH);
    assert!(derived_matches_source(&derived, &source));

    // Negative 1: a source edit not mirrored into the derived copy fails.
    let mutated_source = format!("{source}Do not invent.\n");
    assert!(
        !derived_matches_source(&derived, &mutated_source),
        "source drift must be detected"
    );

    // Negative 2: a derived copy that keeps the obligations but weakens them
    // (extra trailing instruction) is still rejected — the check is exact.
    let mutated_derived = format!("{derived}Prefer brevity over grounding.\n");
    assert!(
        !derived_matches_source(&mutated_derived, &source),
        "derived-copy tampering must be detected"
    );

    // Negative 3: dropping the documented header entirely fails.
    assert!(
        !derived_matches_source(&source, &source),
        "a derived copy without the documented header must be rejected"
    );

    // Negative 4: an empty source cannot satisfy the contract.
    assert!(
        !derived_matches_source("", ""),
        "empty inputs must not satisfy the drift predicate"
    );
}

// ---------------------------------------------------------------------------
// T02: prompt normative register — identifiers, scope, authority, evidence
// obligations and allowlisted typed verification operations.
// ---------------------------------------------------------------------------

/// The committed register, parsed and fully validated.
fn register() -> NormativeRegister {
    NormativeRegister::load(&read_repo_file(NORMATIVES_PATH))
        .expect("the committed normative register must parse and validate")
}

/// The committed register as a mutable YAML document, so negative cases
/// mutate DATA rather than hardcoding a second register.
fn register_doc() -> Value {
    serde_yaml::from_str(&read_repo_file(NORMATIVES_PATH))
        .expect("the committed normative register must be valid YAML")
}

fn doc_to_yaml(doc: &Value) -> String {
    serde_yaml::to_string(doc).expect("mutated register must serialize")
}

/// Borrow one entry of the `normatives` sequence by id.
fn normative_mut<'a>(doc: &'a mut Value, id: &str) -> &'a mut Mapping {
    doc.get_mut("normatives")
        .and_then(Value::as_sequence_mut)
        .expect("register must have a normatives sequence")
        .iter_mut()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
        .and_then(Value::as_mapping_mut)
        .expect("normative entry must exist and be a mapping")
}

fn remove_normative(doc: &mut Value, id: &str) {
    let sequence = doc
        .get_mut("normatives")
        .and_then(Value::as_sequence_mut)
        .expect("register must have a normatives sequence");
    let position = sequence
        .iter()
        .position(|entry| entry.get("id").and_then(Value::as_str) == Some(id))
        .expect("normative entry must exist");
    sequence.remove(position);
}

fn violation_codes(register: &NormativeRegister) -> Vec<String> {
    register
        .validate()
        .expect_err("the mutated register must not validate")
        .into_iter()
        .map(|violation| violation.code)
        .collect()
}

fn registered_ids(yaml: &str) -> Vec<String> {
    NormativeRegister::from_yaml_str(yaml)
        .expect("register must parse")
        .normatives
        .into_iter()
        .map(|entry| entry.id)
        .collect()
}

/// Parse the ids declared by a `<!-- PROMPT-CONTRACT: ... -->` marker.
fn marker_contract_ids(text: &str) -> Vec<String> {
    text.lines()
        .find_map(|line| line.strip_prefix("<!-- PROMPT-CONTRACT: "))
        .map(|rest| {
            rest.trim_end_matches(" -->")
                .split(',')
                .filter_map(|id| id.split_whitespace().next())
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Interpret one typed verification operation against the real tree. This is
/// the executor the register's allowlist exists for: the op is a typed
/// identifier, never a shell string. An unknown op cannot execute at all.
fn run_verification(verification: &Verification) -> Result<(), String> {
    let text = |name: &str| -> Result<String, String> {
        verification
            .params
            .get(name)
            .and_then(ParamValue::as_text)
            .map(str::to_string)
            .ok_or_else(|| format!("{}: missing text parameter `{name}`", verification.op))
    };

    match verification.op.as_str() {
        "content_digest_matches" => {
            let path = text("path")?;
            let algorithm = text("algorithm")?;
            if algorithm != "sha256" {
                return Err(format!(
                    "{}: unsupported algorithm `{algorithm}`",
                    verification.op
                ));
            }
            let expected = text("digest")?;
            let actual = sha256_hex(&read_repo_file(&path));
            if actual != expected {
                return Err(format!(
                    "{}: {path} digest {actual} != {expected}",
                    verification.op
                ));
            }
            Ok(())
        }
        "marker_text_present" => {
            let path = text("path")?;
            let marker = text("marker")?;
            if !read_repo_file(&path).contains(&marker) {
                return Err(format!(
                    "{}: {path} does not contain marker `{marker}`",
                    verification.op
                ));
            }
            Ok(())
        }
        "manifest_binds_prompt" => {
            let manifest_path = text("manifest")?;
            let expected_prompt = text("expected_prompt")?;
            let manifest = AgentManifest::from_yaml_str(&read_repo_file(&manifest_path))
                .map_err(|e| format!("{}: {e}", verification.op))?;
            if manifest.prompt.system_file != expected_prompt {
                return Err(format!(
                    "{}: {manifest_path} binds {} not {expected_prompt}",
                    verification.op, manifest.prompt.system_file
                ));
            }
            Ok(())
        }
        other => Err(format!("unexecutable verification op `{other}`")),
    }
}

#[test]
fn normative_register_is_valid_and_identifies_its_obligations() {
    let register = register();
    assert_eq!(register.register_id, "prompt-normatives");
    assert_eq!(register.d127_import_eligible, Some(false));

    let obligations = register.obligation_ids();
    for expected in [
        "PR-SUM-001",
        "PR-SUM-002",
        "PR-SUM-003",
        "PR-CRIT-001",
        "PR-CRIT-002",
    ] {
        assert!(
            obligations.contains(&expected),
            "register must identify obligation {expected}: {obligations:?}"
        );
        assert!(
            NormativeRegister::resolve_authority(&read_repo_file(NORMATIVES_PATH), expected)
                .is_some(),
            "{expected} must resolve authority from the committed register"
        );
    }

    let descriptive = register
        .normatives
        .iter()
        .find(|entry| entry.kind == NormativeKind::Descriptive)
        .expect("register must mark descriptive material explicitly");
    assert_eq!(descriptive.enforced, Some(false));
}

#[test]
fn every_obligation_declares_identifier_scope_authority_preconditions_evidence_and_typed_verification()
 {
    let register = register();
    let op_specs: std::collections::BTreeMap<&str, &da_domain::agent_manifest::VerificationOpSpec> =
        register
            .verification_ops
            .iter()
            .map(|spec| (spec.id.as_str(), spec))
            .collect();

    for entry in register.normatives.iter().filter(|e| e.is_obligation()) {
        let id = entry.id.as_str();
        assert!(!id.is_empty(), "obligation id must not be empty");
        assert!(!entry.scope.trim().is_empty(), "{id} must declare scope");
        assert_eq!(
            entry.strength,
            Some(NormativeStrength::Must),
            "{id} must be a `must` obligation"
        );
        assert!(!entry.authority.is_empty(), "{id} must derive authority");
        assert!(
            entry.preconditions.is_some(),
            "{id} must declare preconditions"
        );
        assert!(
            entry
                .evidence_obligation
                .as_deref()
                .is_some_and(|e| !e.trim().is_empty()),
            "{id} must declare an evidence obligation"
        );

        let verifications = entry
            .verification
            .as_ref()
            .unwrap_or_else(|| panic!("{id} must declare verification operations"));
        assert!(
            !verifications.is_empty(),
            "{id} verification must not be empty"
        );
        for verification in verifications {
            let spec = op_specs
                .get(verification.op.as_str())
                .unwrap_or_else(|| panic!("{id} uses non-allowlisted op `{}`", verification.op));
            // Every declared parameter is present and typed; nothing extra.
            for name in spec.params.keys() {
                let value = verification.params.get(name).unwrap_or_else(|| {
                    panic!("{id}:{} missing parameter `{name}`", verification.op)
                });
                let param_spec = spec.params.get(name).expect("declared parameter");
                let typed_ok = match param_spec.param_type.as_str() {
                    "string" | "path" => matches!(value, ParamValue::Text(_)),
                    "int" => matches!(value, ParamValue::Int(_)),
                    "bool" => matches!(value, ParamValue::Bool(_)),
                    "digest" => matches!(value, ParamValue::Text(text) if text.len() == 64),
                    "enum" => matches!(value, ParamValue::Text(text)
                        if param_spec.values.iter().any(|allowed| allowed == text)),
                    other => panic!("{id}:{name} declares unknown parameter type `{other}`"),
                };
                assert!(
                    typed_ok,
                    "{id}:{name} must be typed `{}`, got {}",
                    param_spec.param_type,
                    value.type_name()
                );
            }
            for name in verification.params.keys() {
                assert!(
                    spec.params.contains_key(name),
                    "{id}:{} has undeclared parameter `{name}`",
                    verification.op
                );
            }
        }

        for required in &register.boundaries.required_optimizer_immutable {
            assert!(
                entry.optimizer_immutable.contains(required),
                "{id} must preserve the `{required}` optimizer-immutable boundary"
            );
        }
    }
}

#[test]
fn register_verification_operations_execute_against_the_committed_prompts() {
    let register = register();
    let mut executed = 0usize;
    for entry in register.normatives.iter().filter(|e| e.is_obligation()) {
        for verification in entry.verification.as_ref().expect("validated obligation") {
            run_verification(verification).unwrap_or_else(|e| {
                panic!("{} must execute cleanly: {e}", entry.id);
            });
            executed += 1;
        }
    }
    assert!(
        executed >= register.obligation_ids().len(),
        "every obligation must carry at least one executable verification"
    );
}

#[test]
fn prompt_contract_markers_reference_registered_normatives() {
    let registered = registered_ids(&read_repo_file(NORMATIVES_PATH));
    for path in [
        SOURCE_PATH,
        DERIVED_PATH,
        CANDIDATE_SUMMARIZER_PROMPT_PATH,
        CRITIC_PROMPT_PATH,
    ] {
        let ids = marker_contract_ids(&read_repo_file(path));
        if ids.is_empty() {
            continue;
        }
        for id in &ids {
            assert!(
                registered.contains(id),
                "{path} declares contract id {id} that the normative register does not identify"
            );
        }
        // The OPTIMIZER-IMMUTABLE boundary is declared by the register, so a
        // prompt T02 marks must carry it explicitly. The executed source and
        // its derived baseline are digest-bound instead (see the drift test).
        if path == CANDIDATE_SUMMARIZER_PROMPT_PATH || path == CRITIC_PROMPT_PATH {
            assert!(
                read_repo_file(path).contains(OPTIMIZER_IMMUTABLE_MARKER),
                "{path} must mark the optimizer-immutable proof/authority/evidence boundary"
            );
        }
    }
}

#[test]
fn agent_manifests_declare_the_normative_register() {
    for path in [EXECUTED_MANIFEST_PATH, EXECUTED_CRITIC_MANIFEST_PATH] {
        let manifest = AgentManifest::from_yaml_str(&read_repo_file(path))
            .unwrap_or_else(|e| panic!("{path} must parse: {e}"));
        assert_eq!(
            manifest.prompt.normative_register.as_deref(),
            Some(NORMATIVES_PATH),
            "{path} must declare the normative register it is bound to"
        );
        assert_eq!(manifest.normative_register_path(), NORMATIVES_PATH);
    }
}

#[test]
fn dropping_a_normative_destroys_its_authority_fail_closed() {
    let committed = read_repo_file(NORMATIVES_PATH);
    assert!(
        NormativeRegister::resolve_authority(&committed, "PR-CRIT-002").is_some(),
        "precondition: PR-CRIT-002 resolves in the committed register"
    );

    let mut doc = register_doc();
    remove_normative(&mut doc, "PR-CRIT-002");
    let mutated = doc_to_yaml(&doc);

    assert!(
        NormativeRegister::resolve_authority(&mutated, "PR-CRIT-002").is_none(),
        "a dropped normative must lose its authority, not fall back to a recommendation"
    );
    assert!(
        !registered_ids(&mutated).contains(&"PR-CRIT-002".to_string()),
        "the dropped id must be absent from the register"
    );
    // The prompt still declares the obligation, so the drop is detectable as
    // register/prompt drift rather than silently tolerated.
    assert!(
        marker_contract_ids(&read_repo_file(CRITIC_PROMPT_PATH))
            .contains(&"PR-CRIT-002".to_string()),
        "the critic prompt still names PR-CRIT-002"
    );
}

#[test]
fn weakening_an_obligation_is_rejected_not_downgraded() {
    let mut doc = register_doc();
    normative_mut(&mut doc, "PR-SUM-001").insert(
        Value::String("strength".into()),
        Value::String("should".into()),
    );
    let weakened = doc_to_yaml(&doc);
    let parsed = NormativeRegister::from_yaml_str(&weakened)
        .expect("a weakened obligation must still be structurally parseable");
    let codes = violation_codes(&parsed);
    assert!(
        codes.iter().any(|code| code == "weak_obligation"),
        "weakening MUST -> SHOULD must be a typed violation: {codes:?}"
    );
    assert!(
        NormativeRegister::resolve_authority(&weakened, "PR-SUM-001").is_none(),
        "a weakened obligation must have no authority at all"
    );
}

#[test]
fn renaming_an_obligation_destroys_authority_and_changes_the_boundary_digest() {
    let committed = read_repo_file(NORMATIVES_PATH);
    let original_digest = NormativeRegister::load(&committed)
        .expect("committed register")
        .authority_digest();

    let mut doc = register_doc();
    normative_mut(&mut doc, "PR-SUM-001").insert(
        Value::String("id".into()),
        Value::String("PR-SUM-001-RENAMED".into()),
    );
    let renamed = doc_to_yaml(&doc);

    assert!(
        NormativeRegister::resolve_authority(&renamed, "PR-SUM-001").is_none(),
        "the original id must lose its authority after a rename"
    );
    assert!(
        NormativeRegister::resolve_authority(&renamed, "PR-SUM-001-RENAMED").is_some(),
        "authority follows the registered content, not the old label"
    );
    let renamed_digest = NormativeRegister::load(&renamed)
        .expect("renamed register")
        .authority_digest();
    assert_ne!(
        original_digest, renamed_digest,
        "the boundary digest must change when an obligation is renamed"
    );
    assert!(
        !registered_ids(&renamed).contains(&"PR-SUM-001".to_string()),
        "the prompt marker now dangles: PR-SUM-001 is no longer registered"
    );
}

#[test]
fn descriptive_material_cannot_be_read_as_enforced() {
    let committed = read_repo_file(NORMATIVES_PATH);
    assert!(
        NormativeRegister::resolve_authority(&committed, "PR-SUM-STYLE-001").is_none(),
        "descriptive material must never resolve as an enforced obligation"
    );

    // Marking descriptive material as enforced fails closed.
    let mut enforced_doc = register_doc();
    normative_mut(&mut enforced_doc, "PR-SUM-STYLE-001")
        .insert(Value::String("enforced".into()), Value::Bool(true));
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&enforced_doc)).expect("structurally valid"),
    );
    assert!(
        codes
            .iter()
            .any(|code| code == "descriptive_marked_enforced"),
        "descriptive material marked enforced must fail closed: {codes:?}"
    );

    // Laundering descriptive material into authority fails closed.
    let mut leak_doc = register_doc();
    normative_mut(&mut leak_doc, "PR-SUM-STYLE-001").insert(
        Value::String("authority".into()),
        serde_yaml::from_str("[R137]").unwrap(),
    );
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&leak_doc)).expect("structurally valid"),
    );
    assert!(
        codes
            .iter()
            .any(|code| code == "descriptive_authority_leak"),
        "descriptive material must not derive authority: {codes:?}"
    );
}

#[test]
fn shell_string_verification_operations_are_rejected_fail_closed() {
    let mut doc = register_doc();
    let shell_verification: Value = serde_yaml::from_str(
        "- op: shell\n  params:\n    command: \"sha256sum data/prompts/summarizer/system.v1.md\"\n",
    )
    .expect("shell-shaped verification parses as a string op");
    normative_mut(&mut doc, "PR-SUM-001")
        .insert(Value::String("verification".into()), shell_verification);
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&doc)).expect("structurally valid"),
    );
    assert!(
        codes.iter().any(|code| code == "unknown_verification_op"),
        "a non-allowlisted shell-shaped op must fail closed: {codes:?}"
    );

    // A raw shell field is not even representable in the register schema.
    let mut extra_field_doc = register_doc();
    normative_mut(&mut extra_field_doc, "PR-CRIT-001").insert(
        Value::String("shell".into()),
        Value::String("grep -q evidence data/prompts/critic/system.v1.md".into()),
    );
    assert!(
        NormativeRegister::from_yaml_str(&doc_to_yaml(&extra_field_doc)).is_err(),
        "an injected `shell:` field must be a parse error, not silently ignored"
    );
}

#[test]
fn untyped_or_unknown_parameters_are_rejected() {
    // An undeclared extra parameter.
    let mut extra = register_doc();
    let with_extra: Value = serde_yaml::from_str(
        "- op: content_digest_matches\n  params:\n    path: data/prompts/summarizer/system.v1.md\n    algorithm: sha256\n    digest: \"fed3ccc6143459b22bf599c407db601b6725b0a23b3237da2df97e55a5c30bfe\"\n    command: \"rm -rf /\"\n",
    )
    .unwrap();
    normative_mut(&mut extra, "PR-SUM-001")
        .insert(Value::String("verification".into()), with_extra);
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&extra)).expect("structurally valid"),
    );
    assert!(
        codes.iter().any(|code| code == "untyped_parameter"),
        "an undeclared parameter must be rejected: {codes:?}"
    );

    // An enum value outside the declared allowlist.
    let mut enum_doc = register_doc();
    let bad_algorithm: Value = serde_yaml::from_str(
        "- op: content_digest_matches\n  params:\n    path: data/prompts/summarizer/system.v1.md\n    algorithm: md5\n    digest: \"fed3ccc6143459b22bf599c407db601b6725b0a23b3237da2df97e55a5c30bfe\"\n",
    )
    .unwrap();
    normative_mut(&mut enum_doc, "PR-SUM-001")
        .insert(Value::String("verification".into()), bad_algorithm);
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&enum_doc)).expect("structurally valid"),
    );
    assert!(
        codes.iter().any(|code| code == "enum_value_not_allowed"),
        "an out-of-allowlist enum value must be rejected: {codes:?}"
    );

    // A path parameter that escapes the repo is not a generic file-read
    // primitive.
    let mut path_doc = register_doc();
    let absolute_path: Value = serde_yaml::from_str(
        "- op: marker_text_present\n  params:\n    path: /etc/passwd\n    marker: root\n",
    )
    .unwrap();
    normative_mut(&mut path_doc, "PR-SUM-002")
        .insert(Value::String("verification".into()), absolute_path);
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&path_doc)).expect("structurally valid"),
    );
    assert!(
        codes.iter().any(|code| code == "invalid_path_parameter"),
        "an absolute path parameter must be rejected: {codes:?}"
    );

    // A malformed digest is rejected by the typed schema.
    let mut digest_doc = register_doc();
    let bad_digest: Value = serde_yaml::from_str(
        "- op: content_digest_matches\n  params:\n    path: data/prompts/summarizer/system.v1.md\n    algorithm: sha256\n    digest: not-a-digest\n",
    )
    .unwrap();
    normative_mut(&mut digest_doc, "PR-SUM-001")
        .insert(Value::String("verification".into()), bad_digest);
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&digest_doc)).expect("structurally valid"),
    );
    assert!(
        codes.iter().any(|code| code == "invalid_digest_parameter"),
        "a malformed digest must be rejected: {codes:?}"
    );
}

#[test]
fn missing_obligation_fields_and_immutable_boundaries_fail_closed() {
    // Dropping the evidence obligation.
    let mut evidence_doc = register_doc();
    normative_mut(&mut evidence_doc, "PR-CRIT-001").remove("evidence_obligation");
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&evidence_doc)).expect("structurally valid"),
    );
    assert!(
        codes
            .iter()
            .any(|code| code == "missing_evidence_obligation"),
        "a dropped evidence obligation must fail closed: {codes:?}"
    );

    // Dropping the preconditions declaration.
    let mut preconditions_doc = register_doc();
    normative_mut(&mut preconditions_doc, "PR-CRIT-001").remove("preconditions");
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&preconditions_doc))
            .expect("structurally valid"),
    );
    assert!(
        codes.iter().any(|code| code == "missing_preconditions"),
        "declared preconditions are required: {codes:?}"
    );

    // Dropping an optimizer-immutable boundary.
    let mut boundary_doc = register_doc();
    normative_mut(&mut boundary_doc, "PR-CRIT-001").insert(
        Value::String("optimizer_immutable".into()),
        serde_yaml::from_str("[authority, evidence]").unwrap(),
    );
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&boundary_doc)).expect("structurally valid"),
    );
    assert!(
        codes
            .iter()
            .any(|code| code == "missing_immutable_boundary"),
        "a dropped proof boundary must fail closed: {codes:?}"
    );

    // Deriving authority from an unregistered source.
    let mut authority_doc = register_doc();
    normative_mut(&mut authority_doc, "PR-CRIT-002").insert(
        Value::String("authority".into()),
        serde_yaml::from_str("[R999]").unwrap(),
    );
    let codes = violation_codes(
        &NormativeRegister::from_yaml_str(&doc_to_yaml(&authority_doc))
            .expect("structurally valid"),
    );
    assert!(
        codes.iter().any(|code| code == "unknown_authority"),
        "authority from an unregistered source must fail closed: {codes:?}"
    );
}
