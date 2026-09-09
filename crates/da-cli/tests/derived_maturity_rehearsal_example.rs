use serde::Deserialize;
use serde_json::Value;
use std::process::Command;

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Digest {
    schema_version: String,
    deployed: bool,
    import_eligible: bool,
    production_parse_invoked: bool,
    decision_points_consulted: Vec<String>,
    rollback_target: String,
    production_authority: String,
    families: Vec<FamilyDigest>,
    cross_family: CrossFamilyDigest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FamilyDigest {
    family: String,
    candidate_id: String,
    bootstrap: TransitionDigest,
    plus_one: TransitionDigest,
    replay: TransitionDigest,
    human_gate: TransitionDigest,
    blocked: Vec<BlockedDigest>,
    visual_typed_absence_is_not_failure: Option<bool>,
    body_empty_count_is_not_success: Option<bool>,
    header_references_scopes_are_independent: Option<bool>,
    prompt_claim_fields_match: bool,
    deliberate_mismatch: BlockedDigest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionDigest {
    status: String,
    from: Option<String>,
    to: Option<String>,
    goal_kind: Option<String>,
    next_task: Option<TaskDigest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskDigest {
    goal_kind: String,
    current_state: Option<String>,
    next_allowed_state: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockedDigest {
    reason: String,
    missing_or_offending: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CrossFamilyDigest {
    prefixed_denominators_do_not_pool: bool,
    bare_name_documents_pools: bool,
}

fn run_example() -> std::process::Output {
    Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "-p",
            "da-cli",
            "--example",
            "derived_maturity_rehearsal",
        ])
        .current_dir(workspace_root())
        .output()
        .expect("run offline derived maturity rehearsal")
}

fn assert_transition(
    transition: &TransitionDigest,
    status: &str,
    from: Option<&str>,
    to: Option<&str>,
    goal_kind: Option<&str>,
    has_task: bool,
) {
    assert_eq!(transition.status, status);
    assert_eq!(transition.from.as_deref(), from);
    assert_eq!(transition.to.as_deref(), to);
    assert_eq!(transition.goal_kind.as_deref(), goal_kind);
    assert_eq!(transition.next_task.is_some(), has_task);
}

fn assert_task(task: &TaskDigest, goal: &str, current: Option<&str>, next: &str) {
    assert_eq!(task.goal_kind, goal);
    assert_eq!(task.current_state.as_deref(), current);
    assert_eq!(task.next_allowed_state, next);
}

fn walk_forbidden(value: &Value, path: &str) {
    const FORBIDDEN_KEYS: [&str; 8] = [
        "source_path",
        "pdf_path",
        "tei_xml",
        "body_text",
        "graph_id",
        "graph_ids",
        "document_text",
        "source_file",
    ];
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                assert!(
                    !FORBIDDEN_KEYS.contains(&key.as_str()),
                    "forbidden payload key at {path}.{key}"
                );
                walk_forbidden(child, &format!("{path}.{key}"));
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                walk_forbidden(child, &format!("{path}[{index}]"));
            }
        }
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            for forbidden in [".pdf", ".tei", "tei xml", "body text", "graph id"] {
                assert!(
                    !lower.contains(forbidden),
                    "forbidden payload value at {path}: {forbidden}"
                );
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[test]
fn validates_closed_digest_semantics_and_replay() {
    let first = run_example();
    assert!(
        first.status.success(),
        "example failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = run_example();
    assert!(
        second.status.success(),
        "replay failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        first.stdout, second.stdout,
        "offline digest must be byte-stable"
    );

    let stdout = String::from_utf8(first.stdout).expect("UTF-8 digest");
    let raw: Value = serde_json::from_str(&stdout).expect("valid JSON digest");
    walk_forbidden(&raw, "$");
    let digest: Digest = serde_json::from_value(raw).expect("closed digest schema");

    assert_eq!(
        digest.schema_version,
        "da-application.derived-maturity-rehearsal/v1"
    );
    assert!(!digest.deployed);
    assert!(!digest.import_eligible);
    assert!(!digest.production_parse_invoked);
    assert!(digest.decision_points_consulted.is_empty());
    assert_eq!(digest.production_authority, "java_http");
    assert_eq!(digest.rollback_target, "java-grobid-http@0.9.0-crf");
    assert_eq!(digest.families.len(), 5);

    let expected = [
        ("segmentation", "da-parse.native_segmentation"),
        (
            "header",
            "da-parse.native_header_reference_composition@1.0.0:header",
        ),
        (
            "bibliography",
            "da-parse.native_header_reference_composition@1.0.0:references",
        ),
        ("body", "da-parse.native_body_accounting"),
        ("visual", "da-parse.native_visual_heads@1.0.0"),
    ];
    let required_reasons = [
        "pooling",
        "missing_evidence",
        "contamination",
        "failed_evidence",
        "unsafe_production_call",
        "attempted_automatic_promotion",
        "stale_identity",
        "denominator_drift",
    ];

    for (family, candidate) in expected {
        let row = digest
            .families
            .iter()
            .find(|row| row.family == family)
            .unwrap_or_else(|| panic!("missing family {family}"));
        assert_eq!(row.candidate_id, candidate);
        assert_transition(
            &row.bootstrap,
            "advanced",
            None,
            Some("inventory_bound"),
            Some("inventory"),
            true,
        );
        assert_task(
            row.bootstrap.next_task.as_ref().unwrap(),
            "inventory",
            None,
            "inventory_bound",
        );
        assert_transition(
            &row.plus_one,
            "advanced",
            Some("inventory_bound"),
            Some("oracle_bound"),
            Some("oracle"),
            true,
        );
        assert_task(
            row.plus_one.next_task.as_ref().unwrap(),
            "oracle",
            Some("inventory_bound"),
            "oracle_bound",
        );
        assert_transition(
            &row.replay,
            "idempotent",
            Some("inventory_bound"),
            Some("inventory_bound"),
            None,
            false,
        );
        assert_transition(
            &row.human_gate,
            "human_gate",
            None,
            Some("human_gate_required"),
            None,
            false,
        );
        assert!(row.prompt_claim_fields_match);
        assert_eq!(row.deliberate_mismatch.reason, "malformed_prompt");
        assert_eq!(
            row.deliberate_mismatch.missing_or_offending.as_deref(),
            Some("goal_kind")
        );

        for reason in required_reasons {
            let blocked = row.blocked.iter().find(|item| item.reason == reason);
            assert!(
                blocked.is_some(),
                "{family} missing blocked reason {reason}"
            );
            assert!(
                blocked.unwrap().missing_or_offending.is_some(),
                "{family}/{reason} must identify offending or missing input"
            );
        }
    }

    let order: Vec<&str> = digest
        .families
        .iter()
        .map(|row| row.family.as_str())
        .collect();
    assert_eq!(
        order,
        vec!["segmentation", "header", "bibliography", "body", "visual"]
    );
    let body = digest
        .families
        .iter()
        .find(|row| row.family == "body")
        .unwrap();
    assert_eq!(body.body_empty_count_is_not_success, Some(true));
    let visual = digest
        .families
        .iter()
        .find(|row| row.family == "visual")
        .unwrap();
    assert_eq!(visual.visual_typed_absence_is_not_failure, Some(true));
    assert!(
        digest
            .families
            .iter()
            .find(|row| row.family == "header")
            .unwrap()
            .header_references_scopes_are_independent
            == Some(true)
    );
    assert!(
        digest
            .families
            .iter()
            .find(|row| row.family == "bibliography")
            .unwrap()
            .header_references_scopes_are_independent
            == Some(true)
    );
    assert!(digest.cross_family.prefixed_denominators_do_not_pool);
    assert!(digest.cross_family.bare_name_documents_pools);
}
