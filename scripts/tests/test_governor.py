"""Tests for governor hexagonal package.

Run with: PYTHONPATH=scripts python3 -m pytest tests/ -v
Or:       python3 tests/test_governor.py
"""

import json
import sys
import tempfile
from pathlib import Path

# Ensure scripts/ is on path.
_scripts_dir = Path(__file__).resolve().parent.parent
if str(_scripts_dir) not in sys.path:
    sys.path.insert(0, str(_scripts_dir))

from governor.adapters import (
    FilesystemArtifactReader,
    FilesystemCodeScanner,
    YAMLRuleRepository,
)
from governor.application import AuditService
from governor.application.trajectory_service import TrajectoryService
from governor.domain import CheckSeverity, CheckStatus
from governor.domain.trajectory import DriftFlag, TrajectoryVerdict


def _make_root() -> Path:
    """Create a temp project root with minimal structure."""
    root = Path(tempfile.mkdtemp(prefix="governor-test-"))
    (root / "crates" / "kg-ontology").mkdir(parents=True)
    (root / "crates" / "da-domain").mkdir(parents=True)
    (root / "artifacts").mkdir(parents=True)
    (root / "doc" / "adr").mkdir(parents=True)
    return root


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def test_clean_workspace_has_zero_violations():
    """Governor on a clean workspace should report 0 violations."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    assert report.violations_count == 0, f"Expected 0 violations, got {report.violations_count}"


def test_prompt_task_direct_provider_env_bypass_is_caught():
    """ADR-059/D239 requires models.yaml binding IDs, not direct provider env routing."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "data" / "ontology" / "prompt_tasks.yaml",
        "route_policy_id: anthropic-text\nmodel_env: ANTHROPIC_MODEL\n",
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    violations = [
        violation
        for result in report.results
        if result.rule.id == "prompt_tasks_must_use_models_registry_bindings"
        for violation in result.violations
    ]
    assert len(violations) == 2
    assert all(violation.severity == "high" for violation in violations)


def test_prompt_eval_production_or_optimizer_unlock_is_caught():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "data" / "prompt_eval" / "fixtures.yaml",
        "production_acceptance_eligible: true\noptimizer_enabled: true\n",
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    ids = {
        result.rule.id
        for result in report.results
        if result.violations
    }
    assert "prompt_eval_data_must_not_claim_production_acceptance" in ids
    assert "prompt_eval_data_must_not_enable_optimizer_or_promotion" in ids


def test_artifact_optimizer_unlock_is_caught():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "artifacts" / "runtime" / "bad.json",
        '{"optimizer_enabled": true, "auto_promote": true, "import_eligible": false}',
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    ids = {
        result.rule.id
        for result in report.results
        if result.violations
    }
    assert "no_optimizer_or_auto_promote_true_in_artifacts" in ids


def test_no_new_product_python_scripts_is_caught():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "scripts" / "download_something.py",
        "import sys\nprint('product script')\n",
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    ids = {
        result.rule.id
        for result in report.results
        if result.violations
    }
    assert "no_new_product_python_scripts" in ids


def test_governance_python_is_not_caught():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "scripts" / "governor" / "__init__.py",
        "# governance package\n",
    )
    _write(
        root / "scripts" / "tests" / "test_something.py",
        "def test_x(): pass\n",
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    ids = {
        result.rule.id
        for result in report.results
        if result.violations
    }
    assert "no_new_product_python_scripts" not in ids


def test_rust_python_subprocess_is_caught():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-test" / "src" / "lib.rs",
        'use std::process::Command;\nfn main() { Command::new("python3"); }\n',
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    ids = {
        result.rule.id
        for result in report.results
        if result.violations
    }
    assert "no_rust_python_subprocess" in ids


def test_cli_direct_generative_provider_bypass_is_caught():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-cli" / "src" / "main.rs",
        "let client = AnthropicLlmClient::from_env();\n",
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    violations = [
        violation
        for result in report.results
        if result.rule.id == "cli_generative_paths_must_not_bypass_prompt_execution"
        for violation in result.violations
    ]
    assert len(violations) == 1
    assert violations[0].severity == "high"


def test_accepted_article_revision_ontology_is_in_production_data():
    """D223 promotes ADR-058 labels/edges while the draft is superseded."""
    repo = Path(__file__).resolve().parents[2]
    node_text = (repo / "data" / "ontology" / "node_schemas.yaml").read_text()
    edge_text = (repo / "data" / "ontology" / "edge_contracts.yaml").read_text()
    draft_text = (
        repo / "data" / "ontology" / "drafts" / "article_revision_contracts.yaml"
    ).read_text()
    assert "label: PaperRevision" in node_text
    assert "label: SourceManifestation" in node_text
    assert "edge_constant: HAS_REVISION" in edge_text
    assert "edge_constant: HAS_MANIFESTATION" in edge_text
    assert "status: superseded" in draft_text


def test_article_load_inline_revision_graph_write_is_caught():
    """Accepted ontology still belongs in a dedicated application use case."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-application" / "src" / "article_load.rs",
        'store.create_node("PaperRevision");\n',
    )
    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    report = AuditService(scanner=scanner, rule_repo=rule_repo).run_audit()
    violations = [
        v
        for result in report.results
        if result.rule.id == "article_load_must_not_inline_revision_graph_writes"
        for v in result.violations
    ]
    assert len(violations) == 1


def test_kg_crate_da_dependency_caught():
    """Governor should catch kg-* crate depending on da-*."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    # Inject violation: kg-ontology depends on da-domain.
    _write(
        root / "crates" / "kg-ontology" / "Cargo.toml",
        "[package]\nname = \"kg-ontology\"\n\n[dependencies]\nda-domain = { path = \"../da-domain\" }\n",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    assert report.violations_count >= 1, "Expected kg_*_da violation"
    kg_violations = [
        v
        for r in report.results
        if r.rule.id == "kg_crates_must_not_depend_on_da"
        for v in r.violations
    ]
    assert len(kg_violations) >= 1, "Expected kg_crates_must_not_depend_on_da violation"
    assert kg_violations[0].severity == "high"


def test_dev_dependencies_not_flagged_as_prod_violation():
    """da-adapters in [dev-dependencies] should NOT trigger prod-dep violation."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-application" / "Cargo.toml",
        """[package]
name = "da-application"

[dependencies]
serde = "1.0"

[dev-dependencies]
da-adapters = { workspace = true }
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    # da_adapters rule has section_filter="dependencies" — should NOT match dev-deps.
    da_adapter_violations = [
        v
        for r in report.results
        if r.rule.id == "da_application_must_not_depend_on_da_adapters"
        for v in r.violations
    ]
    assert len(da_adapter_violations) == 0, "dev-dependencies should not be flagged"


def test_graph_writes_allowed_true_caught():
    """graph_writes_allowed: true in any artifact should be caught."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "artifacts" / "test-run.json",
        json.dumps({"paper_id": "test", "graph_writes_allowed": True}),
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    graph_violations = [
        v
        for r in report.results
        if r.rule.id == "no_graph_writes_allowed_true_in_artifacts"
        for v in r.violations
    ]
    assert len(graph_violations) >= 1, "Expected graph_writes_allowed violation"
    assert graph_violations[0].severity == "high"


def test_global_section_title_resolution_is_high_violation():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-application" / "src" / "extraction.rs",
        'store.find_node_by_string_property("Section", "title", section).await;\n',
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    violations = [
        violation
        for result in report.results
        if result.rule.id == "extraction_must_not_resolve_section_globally_by_title"
        for violation in result.violations
    ]
    assert len(violations) == 1
    assert violations[0].severity == "high"


def test_cacheable_stage_graph_mutation_is_high_violation_but_materializer_is_legal():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-application" / "src" / "cluster_stage.rs",
        "self.graph_store.create_node(\"ConceptCluster\").await?;\n",
    )
    _write(
        root / "crates" / "da-application" / "src" / "cluster.rs",
        "self.graph_store.create_node(\"ConceptCluster\").await?;\n",
    )

    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    violations = [
        violation
        for result in report.results
        if result.rule.id == "cacheable_cluster_stage_must_not_mutate_graph"
        for violation in result.violations
    ]
    assert len(violations) == 1
    assert violations[0].severity == "high"
    assert violations[0].file.endswith("cluster_stage.rs")


def test_current_cacheable_stage_modules_are_graph_pure():
    root = Path(__file__).resolve().parents[2]
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    stage_violations = [
        violation
        for result in report.results
        if result.rule.id.startswith("cacheable_")
        and result.rule.id.endswith("_must_not_mutate_graph")
        for violation in result.violations
    ]
    assert stage_violations == []


def test_resource_safety_flags_use_canonical_drift_flag_contract(monkeypatch):
    from governor.checks import resource_safety

    monkeypatch.setattr(
        resource_safety,
        "preflight_check",
        lambda _root: {
            "disk": {"status": "critical", "message": "disk low"},
            "memory": {"status": "warning", "message": "memory high"},
            "target_dir": {
                "status": "warning",
                "message": "target large",
                "recommendations": ["clean selected packages"],
            },
        },
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    flags = service._check_resource_safety()
    assert [(flag.flag, flag.severity) for flag in flags] == [
        ("resource_disk_pressure", "high"),
        ("resource_memory_pressure", "medium"),
        ("resource_target_bloat", "medium"),
    ]
    assert all(flag.dimension == "resource_safety" for flag in flags)
    assert all(flag.evidence for flag in flags)


def test_target_dir_reports_debug_incremental_separately(tmp_path: Path) -> None:
    from governor.checks.resource_safety import check_target_dir

    incremental = tmp_path / "target" / "debug" / "incremental"
    incremental.mkdir(parents=True)
    (incremental / "blob.bin").write_bytes(b"x" * (120 * 1024 * 1024))
    deps = tmp_path / "target" / "debug" / "deps"
    deps.mkdir(parents=True)
    (deps / "lib.rlib").write_bytes(b"y" * (110 * 1024 * 1024))

    result = check_target_dir(str(tmp_path), disk_free_gb=80)
    assert result["incremental_cache_gb"] > 0
    assert result["deps_cache_gb"] > 0
    assert result["debug_profile_gb"] > 0
    dirs = {row["dir"] for row in result["top_consumers"]}
    assert "debug/incremental" in dirs
    assert "debug/deps" in dirs


def _write_logging_contract(root: Path) -> None:
    for crate in ["da-domain", "da-ports", "kg-ontology", "kg-pipeline"]:
        _write(root / f"crates/{crate}/Cargo.toml", "[dependencies]\nserde = '1'\n")
    for example in [
        "gold_corpus_canary.rs", "gold_corpus_manifest.rs", "gold_graph_wave.rs",
        "graph_task_suite.rs", "eval_batch.rs", "pipeline_figures.rs",
        "pipeline_finalize.rs", "pipeline_semantic.rs",
    ]:
        _write(
            root / f"crates/da-cli/examples/{example}",
            "fn main() { let _log_guard = da_cli::logging::init_pipeline_logging(\"root\"); }",
        )
    _write(
        root / "crates/da-application/src/pipeline.rs",
        'tracing::info!(event="stage", article_id="a", pipeline_id="p", step="s", stage="x");',
    )
    _write(
        root / "crates/da-application/src/ingest.rs",
        'tracing::info!(event = "parsed_article_materialized", paper_id="a", sections_written=1, references_written=1, cites_edges_written=1);',
    )
    _write(
        root / "crates/da-application/src/extraction.rs",
        'tracing::info!(event = "extracted_entities_materialized", paper_id="a", entities_created=1, mentions_edges_written=1, found_in_edges_written=1);',
    )


def _write_acap_policy(root: Path, *, rollout: str = "bootstrap") -> Path:
    path = root / "scripts/governor/checks/article_context_assurance.yaml"
    _write(path, f"""schema_version: article-context-assurance.v1
rollout: {rollout}
principles:
  canonical_truth_is_lossless: true
  optimizer_outputs_are_candidates_only: true
  import_eligible_default: false
fsm:
  initial_state: source
  terminal_states: [done, failed]
  states: [source, done, failed]
  transitions: [[source, done]]
metric_definitions:
  retention: {{kind: hard}}
layers:
  - id: source
    state: source
    hard_metrics: [retention]
    quality_metrics: []
    enforcement: required
prompt_contract:
  registry_is_data: true
  output_must_validate_before_materialization: true
optimizer_admission:
  candidate_only: true
  auto_promote: false
  hard_gates:
    same_n: true
    dual_f1_non_regression: true
    no_hard_invariant_regression: true
    no_test_contamination: true
    human_decision_required: true
assurance_report_contract:
  schema_version: article-context-assurance-aggregate.v1
  path: artifacts/article_context/assurance_report.json
  required_top_level: [schema_version, policy_sha256, layer_results, aggregate, import_eligible]
  aggregate_required: [hard_failures, replay_net_writes, d127_unlock_count]
  bootstrap_coverage_hard_failure_prefixes: ["required_layer_absent:"]
""")
    return path


def test_assurance_high_drift_is_blocking_via_data_policy():
    from types import SimpleNamespace
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write(
        service.root / "scripts/governor/checks/assurance_severity.yaml",
        "schema_version: assurance-severity.v1\nblocking_dimensions: [information_preservation, article_context_assurance]\nblocking_severity: high\nphases: [preflight, active, closeout]\n",
    )
    verdict = service._derive_verdict(
        SimpleNamespace(has_blocking=False),
        [DriftFlag("broken", "high", "loss", "information_preservation")],
        "active",
    )
    assert verdict == TrajectoryVerdict.BLOCKED


def test_acap_policy_is_data_driven_and_bootstrap_allows_no_claim():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_policy(service.root)
    assert service._check_article_context_assurance() == []


def test_acap_rejects_optimizer_autopromote_and_unknown_metrics():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    path = _write_acap_policy(service.root)
    text = path.read_text().replace("auto_promote: false", "auto_promote: true").replace("hard_metrics: [retention]", "hard_metrics: [missing]")
    path.write_text(text)
    flags = service._check_article_context_assurance()
    assert len(flags) == 1
    assert flags[0].flag == "acap_policy_invalid"
    assert "auto_promote=false" in flags[0].evidence
    assert "unknown metric" in flags[0].evidence


def test_acap_report_enforces_policy_hash_hard_failures_replay_and_d127():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    path = _write_acap_policy(service.root, rollout="required")
    report_path = service.root / "artifacts/article_context/assurance_report.json"
    _write(report_path, json.dumps({
        "policy_sha256": "wrong",
        "layer_results": [{"layer_id": "source", "hard_failures": 1}],
        "aggregate": {"replay_net_writes": 2, "d127_unlock_count": 1},
    }))
    flags = service._check_article_context_assurance()
    assert len(flags) == 1
    assert flags[0].flag == "acap_report_failed"
    assert "policy_sha256 mismatch" in flags[0].evidence
    assert "replay_net_writes" in flags[0].evidence
    assert "d127_unlock_count" in flags[0].evidence



def test_acap_report_accepts_list_typed_empty_hard_failures_and_rejects_nonempty_list():
    """Product ACAP aggregate uses list[str] hard_failures (M371/S07/T03)."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    path = _write_acap_policy(service.root, rollout="required")
    policy_sha = __import__("hashlib").sha256(path.read_bytes()).hexdigest()
    report_path = service.root / "artifacts/article_context/assurance_report.json"
    # Empty list = pass for hard_failures gate (still need required keys/layers).
    good = {
        "schema_version": "article-context-assurance-aggregate.v1",
        "article_count": 1,
        "pipeline_revision": "test",
        "policy_sha256": policy_sha,
        "import_eligible": False,
        "layer_results": [{"layer_id": "source", "hard_failures": []}],
        "fsm_results": {},
        "cache_results": {},
        "prompt_results": {},
        "optimizer_results": {},
        "aggregate": {"replay_net_writes": 0, "d127_unlock_count": 0, "hard_failures": []},
    }
    _write(report_path, json.dumps(good))
    assert service._check_article_context_assurance() == []
    # Non-empty list still fails closed.
    good["layer_results"] = [{"layer_id": "source", "hard_failures": ["replay_net_writes:1"]}]
    _write(report_path, json.dumps(good))
    flags = service._check_article_context_assurance()
    assert len(flags) == 1
    assert flags[0].flag == "acap_report_failed"
    assert "hard_failures" in flags[0].evidence



def test_acap_bootstrap_coverage_gaps_are_medium_not_blocking():
    """Product required_layer_absent codes are coverage under bootstrap (T05)."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    path = _write_acap_policy(service.root, rollout="bootstrap")
    policy_sha = __import__("hashlib").sha256(path.read_bytes()).hexdigest()
    report_path = service.root / "artifacts/article_context/assurance_report.json"
    _write(report_path, json.dumps({
        "schema_version": "article-context-assurance-aggregate.v1",
        "article_count": 200,
        "pipeline_revision": "manifest:test",
        "policy_sha256": policy_sha,
        "import_eligible": False,
        "verdict": "fail",
        "layer_results": [
            {"layer_id": "source", "status": "no_claim", "hard_failures": []},
        ],
        "fsm_results": {},
        "cache_results": {},
        "prompt_results": {},
        "optimizer_results": {},
        "aggregate": {
            "hard_failures": ["required_layer_absent:source"],
            "replay_net_writes": 0,
            "d127_unlock_count": 0,
            "hard_pass_rate": 0.0,
            "weighted_layer_coverage": 0.5,
            "weighted_quality_score": 0.0,
        },
    }))
    flags = service._check_article_context_assurance()
    assert len(flags) == 1
    assert flags[0].flag == "acap_coverage_gap"
    assert flags[0].severity == "medium"
    assert "required_layer_absent:source" in flags[0].evidence


def test_acap_integrity_hard_failures_remain_high_under_bootstrap():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    path = _write_acap_policy(service.root, rollout="bootstrap")
    policy_sha = __import__("hashlib").sha256(path.read_bytes()).hexdigest()
    report_path = service.root / "artifacts/article_context/assurance_report.json"
    _write(report_path, json.dumps({
        "schema_version": "article-context-assurance-aggregate.v1",
        "article_count": 1,
        "pipeline_revision": "manifest:test",
        "policy_sha256": policy_sha,
        "import_eligible": False,
        "verdict": "fail",
        "layer_results": [{"layer_id": "source", "hard_failures": []}],
        "fsm_results": {},
        "cache_results": {},
        "prompt_results": {},
        "optimizer_results": {},
        "aggregate": {
            "hard_failures": ["replay_net_writes:1", "required_layer_absent:source"],
            "replay_net_writes": 1,
            "d127_unlock_count": 0,
        },
    }))
    flags = service._check_article_context_assurance()
    assert len(flags) == 1
    assert flags[0].flag == "acap_report_failed"
    assert flags[0].severity == "high"
    assert "replay_net_writes" in flags[0].evidence


def test_acap_schema_mismatch_and_import_eligible_true_fail_closed():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    path = _write_acap_policy(service.root, rollout="required")
    policy_sha = __import__("hashlib").sha256(path.read_bytes()).hexdigest()
    report_path = service.root / "artifacts/article_context/assurance_report.json"
    _write(report_path, json.dumps({
        "schema_version": "article-context-assurance-report.v1",
        "article_count": 1,
        "pipeline_revision": "test",
        "policy_sha256": policy_sha,
        "import_eligible": True,
        "layer_results": [{"layer_id": "source", "hard_failures": []}],
        "fsm_results": {},
        "cache_results": {},
        "prompt_results": {},
        "optimizer_results": {},
        "aggregate": {"hard_failures": [], "replay_net_writes": 0, "d127_unlock_count": 0},
    }))
    flags = service._check_article_context_assurance()
    assert len(flags) == 1
    assert flags[0].flag == "acap_report_failed"
    assert "schema_version mismatch" in flags[0].evidence
    assert "import_eligible must be false" in flags[0].evidence


def test_import_eligible_scanner_excludes_tests_but_blocks_src():
    """path_contains_exclude /tests/ must not hide production assignments (MEM718)."""
    from governor.adapters.filesystem_code_scanner import FilesystemCodeScanner

    root = _make_root()
    # Production source assignment must still fire.
    _write(
        root / "crates/da-application/src/evil.rs",
        "fn x() { let import_eligible = true; }\n",
    )
    # Fail-closed unit test fixture must be ignored.
    _write(
        root / "crates/da-application/tests/corpus_assurance_test.rs",
        "fn t() { input.import_eligible = true; }\n",
    )
    scanner = FilesystemCodeScanner(root)
    hits = list(scanner.scan_pattern(
        "crates/",
        r"import_eligible\s*[:=]\s*true",
        suffix_allowlist={".rs"},
        path_contains_exclude=["/tests/", "/test/"],
    ))
    rels = [str(p.relative_to(root)) for p, _, _ in hits]
    assert any("src/evil.rs" in r for r in rels), rels
    assert not any("/tests/" in r for r in rels), rels

    hits_all = list(scanner.scan_pattern(
        "crates/",
        r"import_eligible\s*[:=]\s*true",
        suffix_allowlist={".rs"},
    ))
    rels_all = [str(p.relative_to(root)) for p, _, _ in hits_all]
    assert any("/tests/" in r for r in rels_all), rels_all


def test_information_preservation_contracts_turn_lossy_mutations_into_regressions():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    config = service.root / "scripts/governor/checks/information_preservation.yaml"
    sink = service.root / "crates/da-application/src/ingest.rs"
    _write(config, """schema_version: information-preservation.v1
contracts:
  - id: preserve_text
    boundary: source.text -> graph.text
    mode: full_fidelity
    sink: crates/da-application/src/ingest.rs
    required_patterns: ['text\\.clone\\(\\)', 'text_sha256']
    forbidden_patterns: ['text\\.get\\(', 'chars\\(\\)\\.take\\(']
    severity: high
""")
    _write(sink, 'write(text.clone()); set("text_sha256");')
    assert service._check_information_preservation() == []
    for mutation in ["write(text.get(..10000));", "write(text.chars().take(100));"]:
        _write(sink, mutation)
        flags = service._check_information_preservation()
        assert len(flags) == 1
        assert flags[0].flag == "information_preservation_contract_broken"
        assert flags[0].severity == "high"


def test_information_preservation_contract_flags_missing_sink_and_bad_yaml():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    config = service.root / "scripts/governor/checks/information_preservation.yaml"
    _write(config, "contracts:\n  - id: missing\n    sink: absent.rs\n")
    assert service._check_information_preservation()[0].flag == "information_preservation_sink_missing"
    _write(config, "{broken")
    assert service._check_information_preservation()[0].flag == "information_preservation_contract_invalid"


def test_logging_architecture_accepts_semantic_onion_aligned_contract():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_logging_contract(service.root)
    assert service._check_logging_architecture() == []


def test_logging_architecture_flags_boundary_root_and_semantic_gaps():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_logging_contract(service.root)
    _write(service.root / "crates/da-domain/Cargo.toml", "[dependencies]\ntracing = '0.1'\n")
    _write(service.root / "crates/da-cli/examples/gold_graph_wave.rs", "fn main() {}")
    _write(service.root / "crates/da-application/src/pipeline.rs", 'tracing::info!("done");')
    flags = service._check_logging_architecture()
    assert [(flag.flag, flag.severity) for flag in flags] == [
        ("logging_transport_crosses_onion_boundary", "high"),
        ("pipeline_composition_root_without_file_logging", "high"),
        ("pipeline_stage_logs_lack_semantic_fields", "high"),
    ]


def test_gold_completeness_flags_unavailable_and_silent_denominator_loss():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write(
        service.root / "artifacts/gold_corpus/gold_corpus_manifest.json",
        json.dumps({
            "fixture_count": 200,
            "source_ready_count": 193,
            "unavailable_count": 7,
        }),
    )
    _write(
        service.root / "artifacts/gold_corpus/eval_aggregate_all_sources.json",
        json.dumps({
            "fixture_count": 200,
            "evaluated_count": 192,
            "unavailable_count": 7,
            "parse_or_extract_failed_count": 0,
            "gold_entities_all_fixtures": 1439,
            "gold_entities_evaluated": 1390,
        }),
    )
    flags = service._check_gold_completeness()
    assert [(flag.flag, flag.severity) for flag in flags] == [
        ("gold_unavailable_source_debt", "high"),
        ("gold_eval_denominator_incomplete", "high"),
        ("gold_eval_excludes_fixture_entities", "high"),
    ]


def test_gold_completeness_accepts_complete_artifacts_and_ignores_absence():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    assert service._check_gold_completeness() == []
    _write(
        service.root / "artifacts/gold_corpus/gold_corpus_manifest.json",
        json.dumps({
            "fixture_count": 200,
            "source_ready_count": 200,
            "unavailable_count": 0,
        }),
    )
    _write(
        service.root / "artifacts/gold_corpus/eval_aggregate_all_sources.json",
        json.dumps({
            "fixture_count": 200,
            "evaluated_count": 200,
            "unavailable_count": 0,
            "parse_or_extract_failed_count": 0,
            "gold_entities_all_fixtures": 1439,
            "gold_entities_evaluated": 1439,
        }),
    )
    assert service._check_gold_completeness() == []


def test_gold_completeness_flags_ready_record_without_live_path_or_checksum():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write(
        service.root / "artifacts/gold_corpus/gold_corpus_manifest.json",
        json.dumps({
            "fixture_count": 1,
            "source_ready_count": 1,
            "unavailable_count": 0,
            "records": [{
                "paper_id": "synthetic-a",
                "source": {"kind": "pdf", "path": "missing.pdf", "sha256": None,
                           "unavailable_reason": None},
                "layers": {"article_metadata": False},
            }],
        }),
    )
    flags = service._check_gold_completeness()
    assert [flag.flag for flag in flags] == ["gold_manifest_record_integrity_invalid"]


def test_gold_completeness_accepts_ready_without_article_metadata():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    source = service.root / "catalog/paper.pdf"
    _write(source, "pdf")
    _write(
        service.root / "artifacts/gold_corpus/gold_corpus_manifest.json",
        json.dumps({
            "fixture_count": 1,
            "source_ready_count": 1,
            "unavailable_count": 0,
            "records": [{
                "paper_id": "synthetic-b",
                "source": {"kind": "pdf", "path": "catalog/paper.pdf", "sha256": "a" * 64,
                           "unavailable_reason": None},
                "layers": {"article_metadata": False},
            }],
        }),
    )
    assert service._check_gold_completeness() == []


def test_gold_completeness_flags_malformed_artifact_instead_of_crashing():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write(
        service.root / "artifacts/gold_corpus/gold_corpus_manifest.json",
        "{not-json",
    )
    flags = service._check_gold_completeness()
    assert len(flags) == 1
    assert flags[0].flag == "gold_completeness_artifact_invalid"
    assert flags[0].severity == "high"


def test_yaml_rules_load_successfully():
    """YAML rule files should load without error."""
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    repo = YAMLRuleRepository(checks_dir)

    rules = repo.load_adr_rules()
    assert len(rules) >= 16, f"Expected >=16 ADR rules, got {len(rules)}"

    dims = repo.load_dimensions()
    assert len(dims) >= 9, f"Expected >=9 dimensions, got {len(dims)}"

    # Verify all rules have required fields.
    for rule in rules:
        assert "id" in rule
        assert "anchor" in rule
        assert "severity" in rule
        assert "pattern" in rule


def test_artifact_reader_finds_json_files():
    """ArtifactReader should find all JSON files under artifacts/."""
    root = _make_root()
    _write(root / "artifacts" / "a.json", "{}")
    _write(root / "artifacts" / "sub" / "b.json", "{}")
    _write(root / "artifacts" / "c.txt", "not json")

    reader = FilesystemArtifactReader(root)
    artifacts = list(reader.iter_artifacts())
    json_files = [p for p in artifacts if p.suffix == ".json"]

    assert len(json_files) == 2, f"Expected 2 JSON files, got {len(json_files)}"


def test_forbidden_graph_store_in_cargo_caught():
    """ADR-040 binding: forbidden graph store dep (nebula, falkordb, etc.) should be caught."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-domain" / "Cargo.toml",
        '[package]\nname = "da-domain"\n\n[dependencies]\nnebula = "0.1"\n',
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    forbidden = [
        v
        for r in report.results
        if r.rule.id == "no_forbidden_graph_stores_in_cargo"
        for v in r.violations
    ]
    assert len(forbidden) >= 1, "Expected no_forbidden_graph_stores_in_cargo violation"
    assert forbidden[0].severity == "high"


def test_boolean_derived_snapshot_waiver_caught():
    """D229: warning-only derived snapshot bypass must not return."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-cli" / "src" / "checkpoint.rs",
        "let allow_derived_snapshot = true;",
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    violations = [
        violation
        for result in report.results
        if result.rule.id == "no_boolean_derived_snapshot_waiver"
        for violation in result.violations
    ]
    assert len(violations) == 1
    assert violations[0].severity == "high"


def test_kg_crate_da_domain_rust_import_caught():
    """ADR-050: kg-* crate importing da_domain types in Rust source should be caught."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "kg-ontology" / "src" / "lib.rs",
        'use da_domain::schema::PaperNode;\n',
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    violations = [
        v
        for r in report.results
        if r.rule.id == "kg_crates_must_not_import_da_domain"
        for v in r.violations
    ]
    assert len(violations) >= 1, "Expected kg_crates_must_not_import_da_domain violation"


def test_stale_precommit_package_caught():
    """MEM524: stale workspace packages in pre-commit config should be caught."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    # Simulate a stale pre-commit config referencing removed package.
    _write(
        root / ".pre-commit-config.yaml",
        "repos:\n"
        "  - repo: local\n"
        "    hooks:\n"
        "      - id: cargo-fmt\n"
        "        name: cargo fmt\n"
        "        entry: cargo fmt -p da-graph -p kg-storage -- --check\n"
        "        language: system\n",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    violations = [
        v
        for r in report.results
        if r.rule.id == "no_stale_precommit_packages"
        for v in r.violations
    ]
    assert len(violations) >= 1, (
        f"Expected no_stale_precommit_packages violation for da-graph reference, "
        f"got {len(violations)} violations"
    )


def _run_all_tests() -> int:
    """Simple test runner (no pytest dependency)."""
    tests = [
        test_clean_workspace_has_zero_violations,
        test_kg_crate_da_dependency_caught,
        test_dev_dependencies_not_flagged_as_prod_violation,
        test_graph_writes_allowed_true_caught,
        test_yaml_rules_load_successfully,
        test_artifact_reader_finds_json_files,
        test_forbidden_graph_store_in_cargo_caught,
        test_boolean_derived_snapshot_waiver_caught,
        test_kg_crate_da_domain_rust_import_caught,
        test_stale_precommit_package_caught,
    ]
    passed = 0
    failed = 0
    for test in tests:
        try:
            test()
            print(f"  PASS: {test.__name__}")
            passed += 1
        except (AssertionError, Exception) as e:
            print(f"  FAIL: {test.__name__}: {e}")
            failed += 1
    print(f"\nResults: {passed} passed, {failed} failed, {len(tests)} total")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(_run_all_tests())
