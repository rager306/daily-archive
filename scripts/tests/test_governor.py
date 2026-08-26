"""Tests for governor hexagonal package.

Run with: PYTHONPATH=scripts python3 -m pytest tests/ -v
Or:       python3 tests/test_governor.py
"""

import json
import sys
import shutil
import tempfile
from pathlib import Path

import yaml

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
        root / "governor" / "fixtures" / "prompt_eval" / "fixtures.yaml",
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


def test_legacy_load_cache_stage_ids_are_caught():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-cli" / "src" / "load.rs",
        'let old = StageCacheKey { stage_id: "embed".into() };\n'
        'let also_old = StageCacheKey { stage_id: "extract".into() };\n',
    )
    report = AuditService(
        scanner=FilesystemCodeScanner(root),
        rule_repo=YAMLRuleRepository(checks_dir),
    ).run_audit()
    violations = [
        violation
        for result in report.results
        if result.rule.id == "no_legacy_load_cache_stage_ids"
        for violation in result.violations
    ]
    assert len(violations) == 2


def test_canonical_load_cache_stage_ids_are_allowed():
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "crates" / "da-cli" / "src" / "load.rs",
        'let embed = StageCacheKey { stage_id: "embedding".into() };\n'
        'let extract = StageCacheKey { stage_id: "extraction_compute".into() };\n',
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
    assert "no_legacy_load_cache_stage_ids" not in ids


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


def test_kg_crate_da_parse_dependency_caught():
    """Governor should catch kg-* crate depending on da-parse (KTD8)."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "kg-ontology" / "Cargo.toml",
        "[package]\nname = \"kg-ontology\"\n\n[dependencies]\nda-parse = { path = \"../da-parse\" }\n",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    kg_violations = [
        v
        for r in report.results
        if r.rule.id == "kg_crates_must_not_depend_on_da"
        for v in r.violations
    ]
    assert len(kg_violations) >= 1, "Expected kg_crates_must_not_depend_on_da for da-parse"
    assert kg_violations[0].severity == "high"


def test_da_application_must_not_depend_on_da_parse():
    """Prod da-parse on da-application is a hexagonal violation (KTD8 / AE4)."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-application" / "Cargo.toml",
        """[package]
name = "da-application"

[dependencies]
da-parse = { workspace = true }
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    parse_violations = [
        v
        for r in report.results
        if r.rule.id == "da_application_must_not_depend_on_da_parse"
        for v in r.violations
    ]
    assert len(parse_violations) >= 1, "Expected da_application_must_not_depend_on_da_parse"
    assert parse_violations[0].severity == "medium"


def test_da_parse_must_not_depend_on_samyama_or_kg():
    """Prod samyama/kg-* on da-parse is a hexagonal violation (R4 / KTD6)."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-parse" / "Cargo.toml",
        """[package]
name = "da-parse"

[dependencies]
da-ports = { workspace = true }
samyama = { workspace = true }
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    parse_violations = [
        v
        for r in report.results
        if r.rule.id == "da_parse_must_not_depend_on_samyama_or_kg"
        for v in r.violations
    ]
    assert len(parse_violations) >= 1, "Expected da_parse_must_not_depend_on_samyama_or_kg"
    assert parse_violations[0].severity == "medium"


def test_da_parse_da_ports_not_flagged_as_graph_dep():
    """da-ports on da-parse is allowed; only Samyama/kg-* are banned."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-parse" / "Cargo.toml",
        """[package]
name = "da-parse"

[dependencies]
da-ports = { workspace = true }
reqwest = { workspace = true }
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    parse_violations = [
        v
        for r in report.results
        if r.rule.id == "da_parse_must_not_depend_on_samyama_or_kg"
        for v in r.violations
    ]
    assert parse_violations == []


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


def _write_navigation_contract(root: Path) -> Path:
    """Write the section navigation contract + fixtures into a temp root."""
    contract = root / "scripts/governor/checks/section_navigation_contract.yaml"
    _write(contract, """schema_version: section-navigation-contract.v1
policy_id: section-navigation-contract
dimension: temporal_ast_navigation
severity: high
report_contract:
  required_fields: [ok, section_count, edge_count, diagnostic_count, counts, diagnostics]
fixtures:
  positive:
    id: complete_tree
    path: governor/fixtures/navigation_contract/complete_tree.report.json
    expect:
      ok: true
      diagnostic_count: 0
  negative:
    id: broken_tree
    path: governor/fixtures/navigation_contract/broken_tree.report.json
    expect:
      ok: false
      diagnostic_count_min: 1
      require_codes: [NAV-MISSING-SIBLING, NAV-CROSS-PARENT-SIBLING, NAV-MULTI-SUCCESSOR]
graph_task_suite:
  id: tree_navigation
  path: data/graph_tasks/tree_navigation.tasks.cypher
  required_patterns: [first_child, next_sibling, parent_section]
""")
    _write(root / "governor/fixtures/navigation_contract/complete_tree.report.json", json.dumps({
        "ok": True, "section_count": 8, "edge_count": 13,
        "diagnostic_count": 0, "counts": {}, "diagnostics": [],
    }))
    _write(root / "governor/fixtures/navigation_contract/broken_tree.report.json", json.dumps({
        "ok": False, "section_count": 8, "edge_count": 14, "diagnostic_count": 3,
        "counts": {
            "NAV-MISSING-SIBLING": 1,
            "NAV-CROSS-PARENT-SIBLING": 1,
            "NAV-MULTI-SUCCESSOR": 1,
        },
        "diagnostics": [{"code": "NAV-CROSS-PARENT-SIBLING", "path_id": "1",
                          "section_id": "sec-1", "node_id": 0, "message": "x"}],
    }))
    _write(root / "data/graph_tasks/tree_navigation.tasks.cypher", (
        "// @query First-child navigation\n"
        "MATCH (s:Section)-[:first_child]->(c:Section) RETURN c;\n"
        "// @query Next-sibling chain\n"
        "MATCH (s:Section)-[:next_sibling]->(b:Section) RETURN b;\n"
        "// @query Parent-section navigation\n"
        "MATCH (s:Section)-[:parent_section]->(p:Section) RETURN p;\n"
    ))
    return contract


def test_section_navigation_contract_pass_on_clean_fixtures():
    """Positive + negative fixtures and the suite template satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_navigation_contract(service.root)
    assert service._check_section_navigation_contract() == []


def test_section_navigation_contract_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_navigation_contract(service.root)
    (service.root / "governor/fixtures/navigation_contract/broken_tree.report.json").unlink()
    flags = service._check_section_navigation_contract()
    assert len(flags) == 1
    assert flags[0].flag == "section_navigation_contract_fixture_missing"
    assert "negative" in flags[0].evidence


def test_section_navigation_contract_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_navigation_contract(service.root)
    _write(service.root / "governor/fixtures/navigation_contract/complete_tree.report.json", json.dumps({
        "ok": False, "section_count": 8, "edge_count": 13, "diagnostic_count": 1,
        "counts": {"NAV-CYCLE": 1},
        "diagnostics": [{"code": "NAV-CYCLE", "path_id": None, "section_id": None,
                          "node_id": 0, "message": "cycle"}],
    }))
    flags = service._check_section_navigation_contract()
    broken = [f for f in flags if f.flag == "section_navigation_contract_broken"]
    assert len(broken) == 2, f"expected ok=true and zero-diagnostics flags: {flags}"
    assert any("ok=true" in f.evidence for f in broken)


def test_section_navigation_contract_negative_fixture_must_report_codes():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_navigation_contract(service.root)
    # Negative fixture that omits NAV-CROSS-PARENT-SIBLING and NAV-MULTI-SUCCESSOR.
    _write(service.root / "governor/fixtures/navigation_contract/broken_tree.report.json", json.dumps({
        "ok": False, "section_count": 8, "edge_count": 12, "diagnostic_count": 1,
        "counts": {"NAV-MISSING-SIBLING": 1},
        "diagnostics": [{"code": "NAV-MISSING-SIBLING", "path_id": "1.1",
                          "section_id": "sec-1.1", "node_id": 1, "message": "x"}],
    }))
    flags = service._check_section_navigation_contract()
    assert len(flags) == 1
    assert flags[0].flag == "section_navigation_contract_broken"
    assert "NAV-CROSS-PARENT-SIBLING" in flags[0].evidence
    assert "NAV-MULTI-SUCCESSOR" in flags[0].evidence


def test_section_navigation_contract_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write(service.root / "scripts/governor/checks/section_navigation_contract.yaml",
           "schema_version: [unclosed\n")
    flags = service._check_section_navigation_contract()
    assert len(flags) == 1
    assert flags[0].flag == "section_navigation_contract_invalid"


def test_section_navigation_contract_suite_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_navigation_contract(service.root)
    (service.root / "data/graph_tasks/tree_navigation.tasks.cypher").unlink()
    flags = service._check_section_navigation_contract()
    assert len(flags) == 1
    assert flags[0].flag == "section_navigation_contract_suite_missing"


def _write_temporal_wire_contract(root: Path) -> Path:
    """Write the canonical temporal wire contract + fixtures into a temp root."""
    contract = root / "scripts/governor/checks/canonical_temporal_wire_format.yaml"
    _write(contract, """schema_version: canonical-temporal-wire-format.v1
policy_id: canonical-temporal-wire-format
dimension: temporal_ast
severity: high
ontology_data:
  path: data/ontology/node_schemas.yaml
  required_markers:
    - "temporal_wire_contract"
    - "datetime_format: rfc3339_utc"
    - "integer_write: rejected"
    - "legacy_integer_read: compatibility_decoder_only"
    - "open_sentinel: OPEN"
wire_fixtures:
  positive:
    id: canonical_wire
    path: governor/fixtures/temporal_wire_contract/canonical_wire.positive.json
    expect: accepted
  negative:
    id: legacy_integer_wire
    path: governor/fixtures/temporal_wire_contract/legacy_integer_wire.negative.json
    expect: rejected
""")
    _write(root / "data/ontology/node_schemas.yaml", (
        "temporal_wire_contract:\n"
        "  datetime_format: rfc3339_utc\n"
        "  integer_write: rejected\n"
        "  legacy_integer_read: compatibility_decoder_only\n"
        "  open_sentinel: OPEN\n"
        "node_schemas:\n"
        "- label: Paper\n"
    ))
    _write(root / "governor/fixtures/temporal_wire_contract/canonical_wire.positive.json", json.dumps([
        "2024-01-15T10:30:00Z",
        "2023-11-14T22:13:20Z",
        "2024-01-15T10:30:00.123456Z",
        "2024-01-15T10:30:00+00:00",
        "OPEN",
        None,
    ]))
    _write(root / "governor/fixtures/temporal_wire_contract/legacy_integer_wire.negative.json", json.dumps([
        1700000000,
        0,
        -1,
        "1700000000",
        "2024-01-15T10:30:00+02:00",
        "2024-01-15 10:30:00",
        "not-a-date",
        True,
        {"valid_from": "2024-01-15T10:30:00Z"},
    ]))
    return contract


def test_canonical_temporal_wire_format_pass_on_clean_fixtures():
    """Ontology DATA markers + positive/negative wire fixtures satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_temporal_wire_contract(service.root)
    assert service._check_canonical_temporal_wire_format() == []


def test_canonical_temporal_wire_format_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_temporal_wire_contract(service.root)
    (service.root / "governor/fixtures/temporal_wire_contract/legacy_integer_wire.negative.json").unlink()
    flags = service._check_canonical_temporal_wire_format()
    assert len(flags) == 1
    assert flags[0].flag == "canonical_temporal_wire_format_fixture_missing"
    assert "negative" in flags[0].evidence


def test_canonical_temporal_wire_format_ontology_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_temporal_wire_contract(service.root)
    _write(service.root / "data/ontology/node_schemas.yaml", (
        "node_schemas:\n- label: Paper\n"
    ))
    flags = service._check_canonical_temporal_wire_format()
    assert len(flags) == 1
    assert flags[0].flag == "canonical_temporal_wire_format_ontology_marker_missing"
    assert "integer_write: rejected" in flags[0].evidence


def test_canonical_temporal_wire_format_positive_fixture_must_be_clean():
    """A legacy integer smuggled into the positive fixture breaks the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_temporal_wire_contract(service.root)
    _write(service.root / "governor/fixtures/temporal_wire_contract/canonical_wire.positive.json",
           json.dumps(["2024-01-15T10:30:00Z", 1700000000]))
    flags = service._check_canonical_temporal_wire_format()
    assert len(flags) == 1
    assert flags[0].flag == "canonical_temporal_wire_format_broken"
    assert "positive" in flags[0].evidence
    assert "1700000000" in flags[0].evidence


def test_canonical_temporal_wire_format_negative_fixture_must_be_rejected():
    """A canonical RFC3339 UTC string smuggled into the negative fixture breaks it."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_temporal_wire_contract(service.root)
    _write(service.root / "governor/fixtures/temporal_wire_contract/legacy_integer_wire.negative.json",
           json.dumps([1700000000, "2024-01-15T10:30:00Z"]))
    flags = service._check_canonical_temporal_wire_format()
    assert len(flags) == 1
    assert flags[0].flag == "canonical_temporal_wire_format_broken"
    assert "negative" in flags[0].evidence
    assert "2024-01-15T10:30:00Z" in flags[0].evidence


def test_canonical_temporal_wire_format_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write(service.root / "scripts/governor/checks/canonical_temporal_wire_format.yaml",
           "schema_version: [unclosed\n")
    flags = service._check_canonical_temporal_wire_format()
    assert len(flags) == 1
    assert flags[0].flag == "canonical_temporal_wire_format_invalid"


def _write_promotion_contract(root: Path) -> Path:
    """Write the promotion evidence contract + fixtures into a temp root."""
    contract = root / "scripts/governor/checks/promotion_evidence_contract.yaml"
    _write(contract, """schema_version: promotion-evidence-contract.v1
policy_id: promotion-evidence-contract
dimension: candidate_promotion
severity: high
decision_contract:
  required_fields: [decision_id, transition, committed_at, candidate]
candidate_contract:
  required_fields: [candidate_id, kind, state, proposer, verifier, reviewer, promoter, evidence_refs, rejection_reason, import_eligible]
  import_eligible: false
  evidence_required_except: [rejected]
  state_shape:
    proposed: {verifier: absent, reviewer: absent, promoter: absent, rejection_reason: absent}
    verified: {verifier: present, reviewer: absent, promoter: absent, rejection_reason: absent}
    reviewed: {verifier: present, reviewer: present, promoter: absent, rejection_reason: absent}
    promoted: {verifier: present, reviewer: present, promoter: present, rejection_reason: absent}
    rejected: {rejection_reason: present}
independence:
  verifier_differs_from: [proposer]
  reviewer_differs_from: [proposer, verifier]
  promoter_differs_from: [proposer, verifier, reviewer]
fixtures:
  positive:
    id: valid_decisions
    path: governor/fixtures/promotion_contract/valid_decisions.positive.json
    expect: clean
  negative:
    id: contract_violations
    path: governor/fixtures/promotion_contract/contract_violations.negative.json
    expect:
      require_violations: [missing_evidence, import_leak, self_confirmation, reviewer_not_independent, promoter_not_independent, transition_mismatch, invalid_timestamp, double_promotion, terminal_mutation]
source_markers:
  domain:
    path: crates/da-domain/src/promotion.rs
    required_patterns: [ImportEligibleLeak, SelfConfirmation, AlreadyPromoted, TerminalMutation]
  service:
    path: crates/da-application/src/promotion.rs
    required_patterns: [PromotionCommitSink, PromotionTransitionRequest]
""")
    _write(root / "crates/da-domain/src/promotion.rs", (
        "pub enum PromotionError { ImportEligibleLeak, SelfConfirmation, "
        "AlreadyPromoted, TerminalMutation }\n"
    ))
    _write(root / "crates/da-application/src/promotion.rs", (
        "pub struct PromotionService;\n"
        "PromotionCommitSink PromotionTransitionRequest\n"
    ))
    _write(root / "governor/fixtures/promotion_contract/valid_decisions.positive.json", json.dumps([
        {
            "decision_id": "gold-propose-1",
            "transition": "proposed",
            "committed_at": "2026-08-15T10:30:00Z",
            "candidate": {
                "candidate_id": "gold:RLHF:Method", "kind": "gold", "state": "proposed",
                "proposer": "gold:multi-paper-promote",
                "verifier": None, "reviewer": None, "promoter": None,
                "evidence_refs": [
                    {"reference": "gold:2510.26094:RLHF:Method", "kind": "gold_paper_row"},
                    {"reference": "gold:constraint:min_papers=2", "kind": "gold_constraint"},
                ],
                "rejection_reason": None, "import_eligible": False,
            },
        },
        {
            "decision_id": "prompt-promote-1",
            "transition": "promoted",
            "committed_at": "2026-08-15T10:33:00Z",
            "candidate": {
                "candidate_id": "prompt-cand-1", "kind": "prompt", "state": "promoted",
                "proposer": "prompt:registry:summary",
                "verifier": "prompt:evaluation:summary",
                "reviewer": "prompt:eligibility:summary",
                "promoter": "deployment:review:summary",
                "evidence_refs": [
                    {"reference": "sha256:abc", "kind": "artifact_hash"},
                    {"reference": "decision:DR-42", "kind": "human_decision"},
                ],
                "rejection_reason": None, "import_eligible": False,
            },
        },
    ]))
    _write(root / "governor/fixtures/promotion_contract/contract_violations.negative.json", json.dumps([
        {
            "violation": "missing_evidence",
            "decision": {
                "decision_id": "neg-1", "transition": "proposed",
                "committed_at": "2026-08-15T10:30:00Z",
                "candidate": {
                    "candidate_id": "gold:LoRA:Method", "kind": "gold", "state": "proposed",
                    "proposer": "gold:multi-paper-promote",
                    "verifier": None, "reviewer": None, "promoter": None,
                    "evidence_refs": [],
                    "rejection_reason": None, "import_eligible": False,
                },
            },
        },
        {
            "violation": "import_leak",
            "decision": {
                "decision_id": "neg-2", "transition": "promoted",
                "committed_at": "2026-08-15T10:30:00Z",
                "candidate": {
                    "candidate_id": "c-leak", "kind": "prompt", "state": "promoted",
                    "proposer": "p", "verifier": "v", "reviewer": "r", "promoter": "d",
                    "evidence_refs": [{"reference": "sha256:abc", "kind": "artifact_hash"}],
                    "rejection_reason": None, "import_eligible": True,
                },
            },
        },
        {
            "violation": "self_confirmation",
            "decision": {
                "decision_id": "neg-3", "transition": "verified",
                "committed_at": "2026-08-15T10:30:00Z",
                "candidate": {
                    "candidate_id": "c-self", "kind": "gold", "state": "verified",
                    "proposer": "gold:multi-paper-promote",
                    "verifier": "gold:multi-paper-promote",
                    "reviewer": None, "promoter": None,
                    "evidence_refs": [{"reference": "gold:2510.26094:X:Y", "kind": "gold_paper_row"}],
                    "rejection_reason": None, "import_eligible": False,
                },
            },
        },
        {
            "violation": "reviewer_not_independent",
            "decision": {
                "decision_id": "neg-4", "transition": "reviewed",
                "committed_at": "2026-08-15T10:30:00Z",
                "candidate": {
                    "candidate_id": "c-rev", "kind": "entity_resolution", "state": "reviewed",
                    "proposer": "human:alice", "verifier": "er:blocking", "reviewer": "er:blocking",
                    "promoter": None,
                    "evidence_refs": [{"reference": "merge:a:b", "kind": "merge_provenance"}],
                    "rejection_reason": None, "import_eligible": False,
                },
            },
        },
        {
            "violation": "promoter_not_independent",
            "decision": {
                "decision_id": "neg-5", "transition": "promoted",
                "committed_at": "2026-08-15T10:30:00Z",
                "candidate": {
                    "candidate_id": "c-prom", "kind": "prompt", "state": "promoted",
                    "proposer": "prompt:registry:summary", "verifier": "prompt:eval:summary",
                    "reviewer": "prompt:elig:summary", "promoter": "prompt:registry:summary",
                    "evidence_refs": [{"reference": "sha256:def", "kind": "artifact_hash"}],
                    "rejection_reason": None, "import_eligible": False,
                },
            },
        },
        {
            "violation": "transition_mismatch",
            "decision": {
                "decision_id": "neg-6", "transition": "promoted",
                "committed_at": "2026-08-15T10:30:00Z",
                "candidate": {
                    "candidate_id": "c-mismatch", "kind": "entity_resolution", "state": "reviewed",
                    "proposer": "human:alice", "verifier": "er:blocking", "reviewer": "er:review",
                    "promoter": None,
                    "evidence_refs": [{"reference": "merge:m1:m2", "kind": "merge_provenance"}],
                    "rejection_reason": None, "import_eligible": False,
                },
            },
        },
        {
            "violation": "invalid_timestamp",
            "decision": {
                "decision_id": "neg-7", "transition": "proposed",
                "committed_at": "not-a-date",
                "candidate": {
                    "candidate_id": "c-ts", "kind": "gold", "state": "proposed",
                    "proposer": "gold:multi-paper-promote",
                    "verifier": None, "reviewer": None, "promoter": None,
                    "evidence_refs": [{"reference": "gold:2601.00001:X:Y", "kind": "gold_paper_row"}],
                    "rejection_reason": None, "import_eligible": False,
                },
            },
        },
        {
            "violation": "double_promotion",
            "journal": [
                {
                    "decision_id": "neg-8a", "transition": "promoted",
                    "committed_at": "2026-08-15T10:30:00Z",
                    "candidate": {
                        "candidate_id": "c-double", "kind": "prompt", "state": "promoted",
                        "proposer": "p", "verifier": "v", "reviewer": "r", "promoter": "d1",
                        "evidence_refs": [{"reference": "sha256:111", "kind": "artifact_hash"}],
                        "rejection_reason": None, "import_eligible": False,
                    },
                },
                {
                    "decision_id": "neg-8b", "transition": "promoted",
                    "committed_at": "2026-08-15T10:31:00Z",
                    "candidate": {
                        "candidate_id": "c-double", "kind": "prompt", "state": "promoted",
                        "proposer": "p", "verifier": "v", "reviewer": "r", "promoter": "d2",
                        "evidence_refs": [{"reference": "sha256:111", "kind": "artifact_hash"}],
                        "rejection_reason": None, "import_eligible": False,
                    },
                },
            ],
        },
        {
            "violation": "terminal_mutation",
            "journal": [
                {
                    "decision_id": "neg-9a", "transition": "promoted",
                    "committed_at": "2026-08-15T10:30:00Z",
                    "candidate": {
                        "candidate_id": "c-term", "kind": "gold", "state": "promoted",
                        "proposer": "p", "verifier": "v", "reviewer": "r", "promoter": "d",
                        "evidence_refs": [{"reference": "gold:2510.26094:G:Method", "kind": "gold_paper_row"}],
                        "rejection_reason": None, "import_eligible": False,
                    },
                },
                {
                    "decision_id": "neg-9b", "transition": "rejected",
                    "committed_at": "2026-08-15T10:31:00Z",
                    "candidate": {
                        "candidate_id": "c-term", "kind": "gold", "state": "rejected",
                        "proposer": "p", "verifier": "v", "reviewer": "r", "promoter": None,
                        "evidence_refs": [{"reference": "gold:2510.26094:G:Method", "kind": "gold_paper_row"}],
                        "rejection_reason": "revoked after promotion", "import_eligible": False,
                    },
                },
            ],
        },
    ]))
    return contract

def test_promotion_evidence_contract_pass_on_clean_fixtures():
    """Positive + negative fixtures and source markers satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    assert service._check_promotion_evidence_contract() == []

def test_promotion_evidence_contract_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    (service.root / "governor/fixtures/promotion_contract/contract_violations.negative.json").unlink()
    flags = service._check_promotion_evidence_contract()
    assert len(flags) == 1
    assert flags[0].flag == "promotion_evidence_contract_fixture_missing"
    assert "negative" in flags[0].evidence

def test_promotion_evidence_contract_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    # Inject an import leak into the positive promoted decision (D127).
    path = service.root / "governor/fixtures/promotion_contract/valid_decisions.positive.json"
    decisions = json.loads(path.read_text())
    decisions[1]["candidate"]["import_eligible"] = True
    _write(path, json.dumps(decisions))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert len(broken) == 1
    assert "import_leak" in broken[0].evidence

def test_promotion_evidence_contract_negative_fixture_must_trigger_labelled_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    # Break the self_confirmation record so its labelled violation no longer fires.
    path = service.root / "governor/fixtures/promotion_contract/contract_violations.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "self_confirmation":
            record["decision"]["candidate"]["verifier"] = "independent-verifier"
    _write(path, json.dumps(records))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert any("self_confirmation" in f.evidence for f in broken)

def test_promotion_evidence_contract_negative_fixture_must_cover_required_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    # Remove the double_promotion journal entirely — coverage must fail.
    path = service.root / "governor/fixtures/promotion_contract/contract_violations.negative.json"
    records = json.loads(path.read_text())
    records = [r for r in records if r.get("violation") != "double_promotion"]
    _write(path, json.dumps(records))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert any("double_promotion" in f.evidence for f in broken)

def test_promotion_evidence_contract_source_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    # Drop the typed guards from the domain FSM surface.
    _write(service.root / "crates/da-domain/src/promotion.rs",
           "pub enum PromotionError { }\n")
    flags = service._check_promotion_evidence_contract()
    marker = [f for f in flags if f.flag == "promotion_evidence_contract_source_marker_missing"]
    assert len(marker) == 1
    assert "domain" in marker[0].evidence

def test_promotion_evidence_contract_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    _write(service.root / "scripts/governor/checks/promotion_evidence_contract.yaml",
           "{bad yaml: [")
    flags = service._check_promotion_evidence_contract()
    assert flags[0].flag == "promotion_evidence_contract_invalid"

def _write_acap_runtime_control_plane_contract(root: Path) -> Path:
    """Write the ACAP runtime control-plane contract + fixtures into a temp root."""
    contract = root / "scripts/governor/checks/acap_runtime_control_plane.yaml"
    _write(contract, """schema_version: acap-runtime-control-plane.v1
policy_id: acap-runtime-control-plane
dimension: acap_runtime_control_plane
severity: high
locks:
  import_eligible: false
  promotion_explicit_transitions: 3
admission:
  fsm_policy: scripts/governor/checks/article_context_assurance.yaml
  record_contract:
    required_fields: [stage_id, admitted, evidence_ref, disposition, from_state, to_state]
    dispositions: [executed, cache_hit, failed]
  fixtures:
    positive:
      id: legal_admission
      path: governor/fixtures/acap_runtime_control_plane/admission_legal.positive.json
      expect: clean
    negative:
      id: admission_violations
      path: governor/fixtures/acap_runtime_control_plane/admission_illegal.negative.json
      expect:
        require_violations: [illegal_transition, denied_without_reason, missing_evidence_ref, admitted_with_error, import_leak]
promotion:
  record_contract:
    required_fields: [decision_id, transition, verifier, reviewer, promoter, evidence_refs, sink_len, explicit_transitions, import_eligible]
    required_explicit_transitions: 3
  fixtures:
    positive:
      id: authority_legal
      path: governor/fixtures/acap_runtime_control_plane/promotion_authority.positive.json
      expect: clean
    negative:
      id: authority_violations
      path: governor/fixtures/acap_runtime_control_plane/promotion_authority.negative.json
      expect:
        require_violations: [auto_promote, import_leak, self_authority, missing_evidence]
scheduler:
  record_contract:
    required_fields: [task_vid, status, attempts, attempt_outcomes, published_by_epoch, logical_results, due_after_replay, import_eligible]
    completed_status: completed
  fixtures:
    positive:
      id: recovery_legal
      path: governor/fixtures/acap_runtime_control_plane/scheduler_recovery.positive.json
      expect: clean
    negative:
      id: recovery_violations
      path: governor/fixtures/acap_runtime_control_plane/scheduler_recovery.negative.json
      expect:
        require_violations: [double_publish, stale_epoch_publish, recovery_without_aborted_attempt, completed_without_result, due_after_completion, import_leak]
replay:
  record_contract:
    required_fields: [scenario, pipeline_executor_calls, pipeline_stage_records, pipeline_replay_dispositions, promotion_sink_len, promotion_sink_len_after_replay, scheduler_attempts, scheduler_attempts_after_replay, scheduler_due_after_replay, import_eligible]
  fixtures:
    positive:
      id: replay_idempotent
      path: governor/fixtures/acap_runtime_control_plane/replay_idempotent.positive.json
      expect: clean
    negative:
      id: replay_violations
      path: governor/fixtures/acap_runtime_control_plane/replay_idempotent.negative.json
      expect:
        require_violations: [executor_rerun, duplicate_stage_records, cache_miss_on_replay, promotion_sink_growth, scheduler_new_attempts, scheduler_due_again, import_leak]
source_markers:
  control_plane_test:
    path: crates/da-application/tests/acap_control_plane_test.rs
    required_patterns: [ControlPlaneEvidenceLedger, admission:illegal_transition, AlreadyPromoted, recovered, import_eligible]
  admission_domain:
    path: crates/da-domain/src/assurance.rs
    required_patterns: [ArticleContextState, IllegalTransition, error_class]
  admission_ledger:
    path: crates/da-domain/src/pipeline_ledger.rs
    required_patterns: [PipelineStageLedger, StageIdRegistry]
  scheduler_domain:
    path: crates/da-domain/src/scheduler.rs
    required_patterns: [logical_result_vid, AttemptOutcome, PendingTask]
""")
    _write(root / "scripts/governor/checks/article_context_assurance.yaml", """schema_version: article-context-assurance.v1
fsm:
  initial_state: source_registered
  transitions:
    - [source_registered, source_verified]
    - [source_verified, parsed]
    - [parsed, structured]
    - [structured, extracted]
    - [extracted, evidence_linked]
    - [evidence_linked, embedded]
    - [embedded, materialized]
    - [materialized, enriched]
    - [enriched, resolved]
    - [resolved, summarized]
    - [summarized, finalized]
    - [finalized, task_accepted]
    - [task_accepted, accepted_checkpoint]
""")
    _write(root / "governor/fixtures/acap_runtime_control_plane/admission_legal.positive.json", json.dumps([
        {"stage_id": "acquisition", "admitted": True,
         "evidence_ref": "source:sha256:pdf-v1", "disposition": "executed",
         "from_state": "source_registered", "to_state": "source_verified"},
        {"stage_id": "checkpoint", "admitted": True,
         "evidence_ref": "test-key-summary", "disposition": "executed",
         "from_state": "materialized", "to_state": "enriched"},
    ]))
    _write(root / "governor/fixtures/acap_runtime_control_plane/admission_illegal.negative.json", json.dumps([
        {"violation": "illegal_transition", "record": {
            "stage_id": "summary", "admitted": False, "evidence_ref": "",
            "disposition": "failed", "from_state": "source_registered",
            "to_state": "summarized", "error_class": "admission:illegal_transition"}},
        {"violation": "denied_without_reason", "record": {
            "stage_id": "checkpoint", "admitted": False, "evidence_ref": "",
            "disposition": "failed", "from_state": "task_accepted",
            "to_state": "accepted_checkpoint", "error_class": None}},
        {"violation": "missing_evidence_ref", "record": {
            "stage_id": "summary", "admitted": True, "evidence_ref": "",
            "disposition": "executed", "from_state": "resolved", "to_state": "summarized"}},
        {"violation": "admitted_with_error", "record": {
            "stage_id": "parse", "admitted": True, "evidence_ref": "test-key-catalog",
            "disposition": "executed", "from_state": "source_verified", "to_state": "parsed",
            "error": {"error_class": "network"}}},
        {"violation": "import_leak", "record": {
            "stage_id": "summary", "admitted": True, "evidence_ref": "test-key-resolution",
            "disposition": "executed", "from_state": "embedded", "to_state": "materialized",
            "import_eligible": True}},
    ]))
    _write(root / "governor/fixtures/acap_runtime_control_plane/promotion_authority.positive.json", json.dumps([
        {"decision_id": "gold-3", "transition": "promoted",
         "verifier": "contract:verifier:gold:1", "reviewer": "contract:reviewer:gold:2",
         "promoter": "contract:promoter:gold:3",
         "evidence_refs": ["gold:2510.26094:RLHF:Method"],
         "sink_len": 3, "explicit_transitions": 3, "import_eligible": False},
    ]))
    _write(root / "governor/fixtures/acap_runtime_control_plane/promotion_authority.negative.json", json.dumps([
        {"violation": "auto_promote", "record": {
            "decision_id": "opt-1", "transition": "auto_promoted",
            "verifier": "eval:summary", "reviewer": "elig:summary", "promoter": "optimizer:grpo",
            "evidence_refs": ["candidate:grpo:7"], "sink_len": 1,
            "explicit_transitions": 0, "import_eligible": False}},
        {"violation": "import_leak", "record": {
            "decision_id": "gold-3", "transition": "promoted",
            "verifier": "contract:verifier:gold:1", "reviewer": "contract:reviewer:gold:2",
            "promoter": "contract:promoter:gold:3",
            "evidence_refs": ["gold:2510.26094:RLHF:Method"], "sink_len": 3,
            "explicit_transitions": 3, "import_eligible": True}},
        {"violation": "self_authority", "record": {
            "decision_id": "gold-4", "transition": "promoted",
            "verifier": "contract:verifier:gold:1", "reviewer": "contract:reviewer:gold:2",
            "promoter": "contract:reviewer:gold:2",
            "evidence_refs": ["gold:2510.26094:X:Y"], "sink_len": 3,
            "explicit_transitions": 3, "import_eligible": False}},
        {"violation": "missing_evidence", "record": {
            "decision_id": "gold-5", "transition": "promoted",
            "verifier": "contract:verifier:gold:1", "reviewer": "contract:reviewer:gold:2",
            "promoter": "contract:promoter:gold:5", "evidence_refs": [],
            "sink_len": 3, "explicit_transitions": 3, "import_eligible": False}},
    ]))
    _write(root / "governor/fixtures/acap_runtime_control_plane/scheduler_recovery.positive.json", json.dumps([
        {"task_vid": "scheduler:openalex_enrich:2401.00001", "status": "completed",
         "attempts": 2, "attempt_outcomes": ["aborted", "succeeded"],
         "published_by_epoch": 2,
         "logical_results": [{"result_id": "lr-1", "published_by_epoch": 2,
                                "evidence_refs": ["stage:parse:cache:sha256:cache-key:sha256:abc123"]}],
         "due_after_replay": 0, "import_eligible": False},
    ]))
    _write(root / "governor/fixtures/acap_runtime_control_plane/scheduler_recovery.negative.json", json.dumps([
        {"violation": "double_publish", "record": {
            "task_vid": "scheduler:openalex_enrich:2401.00001", "status": "completed",
            "attempts": 2, "attempt_outcomes": ["aborted", "succeeded"],
            "published_by_epoch": 2,
            "logical_results": [{"result_id": "lr-1", "published_by_epoch": 2,
                                   "evidence_refs": ["stage:parse:cache:sha256:cache-key:sha256:abc123"]},
                                  {"result_id": "lr-1-dupe", "published_by_epoch": 2,
                                   "evidence_refs": ["stage:parse:cache:sha256:cache-key:sha256:abc123"]}],
            "due_after_replay": 0, "import_eligible": False}},
        {"violation": "stale_epoch_publish", "record": {
            "task_vid": "scheduler:openalex_enrich:2401.00002", "status": "completed",
            "attempts": 2, "attempt_outcomes": ["aborted", "succeeded"],
            "published_by_epoch": 1,
            "logical_results": [{"result_id": "lr-2", "published_by_epoch": 1,
                                   "evidence_refs": ["stage:parse:cache:sha256:cache-key:sha256:abc123"]}],
            "due_after_replay": 0, "import_eligible": False}},
        {"violation": "recovery_without_aborted_attempt", "record": {
            "task_vid": "scheduler:openalex_enrich:2401.00003", "status": "completed",
            "attempts": 2, "attempt_outcomes": ["succeeded", "succeeded"],
            "published_by_epoch": 2,
            "logical_results": [{"result_id": "lr-3", "published_by_epoch": 2,
                                   "evidence_refs": ["stage:parse:cache:sha256:cache-key:sha256:abc123"]}],
            "due_after_replay": 0, "import_eligible": False}},
        {"violation": "completed_without_result", "record": {
            "task_vid": "scheduler:openalex_enrich:2401.00004", "status": "completed",
            "attempts": 1, "attempt_outcomes": ["succeeded"], "published_by_epoch": 1,
            "logical_results": [], "due_after_replay": 0, "import_eligible": False}},
        {"violation": "due_after_completion", "record": {
            "task_vid": "scheduler:openalex_enrich:2401.00005", "status": "completed",
            "attempts": 1, "attempt_outcomes": ["succeeded"], "published_by_epoch": 1,
            "logical_results": [{"result_id": "lr-5", "published_by_epoch": 1,
                                   "evidence_refs": ["stage:parse:cache:sha256:cache-key:sha256:abc123"]}],
            "due_after_replay": 2, "import_eligible": False}},
        {"violation": "import_leak", "record": {
            "task_vid": "scheduler:openalex_enrich:2401.00006", "status": "completed",
            "attempts": 1, "attempt_outcomes": ["succeeded"], "published_by_epoch": 1,
            "logical_results": [{"result_id": "lr-6", "published_by_epoch": 1,
                                   "evidence_refs": ["stage:parse:cache:sha256:cache-key:sha256:abc123"]}],
            "due_after_replay": 0, "import_eligible": True}},
    ]))
    _write(root / "governor/fixtures/acap_runtime_control_plane/replay_idempotent.positive.json", json.dumps([
        {"scenario": "full-fsm-chain pdf-v1 replay",
         "pipeline_executor_calls": [["acquisition", 1], ["checkpoint", 1]],
         "pipeline_stage_records": 2,
         "pipeline_replay_dispositions": ["cache_hit", "cache_hit"],
         "promotion_sink_len": 3, "promotion_sink_len_after_replay": 3,
         "scheduler_attempts": 2, "scheduler_attempts_after_replay": 2,
         "scheduler_due_after_replay": 0, "import_eligible": False},
    ]))
    _write(root / "governor/fixtures/acap_runtime_control_plane/replay_idempotent.negative.json", json.dumps([
        {"violation": "executor_rerun", "record": {
            "scenario": "executor rerun",
            "pipeline_executor_calls": [["acquisition", 2], ["checkpoint", 1]],
            "pipeline_stage_records": 2,
            "pipeline_replay_dispositions": ["cache_hit", "cache_hit"],
            "promotion_sink_len": 3, "promotion_sink_len_after_replay": 3,
            "scheduler_attempts": 2, "scheduler_attempts_after_replay": 2,
            "scheduler_due_after_replay": 0, "import_eligible": False}},
        {"violation": "duplicate_stage_records", "record": {
            "scenario": "duplicate records",
            "pipeline_executor_calls": [["acquisition", 1], ["checkpoint", 1]],
            "pipeline_stage_records": 3,
            "pipeline_replay_dispositions": ["cache_hit", "cache_hit"],
            "promotion_sink_len": 3, "promotion_sink_len_after_replay": 3,
            "scheduler_attempts": 2, "scheduler_attempts_after_replay": 2,
            "scheduler_due_after_replay": 0, "import_eligible": False}},
        {"violation": "cache_miss_on_replay", "record": {
            "scenario": "cache miss on replay",
            "pipeline_executor_calls": [["acquisition", 1], ["checkpoint", 1]],
            "pipeline_stage_records": 2,
            "pipeline_replay_dispositions": ["cache_hit", "executed"],
            "promotion_sink_len": 3, "promotion_sink_len_after_replay": 3,
            "scheduler_attempts": 2, "scheduler_attempts_after_replay": 2,
            "scheduler_due_after_replay": 0, "import_eligible": False}},
        {"violation": "promotion_sink_growth", "record": {
            "scenario": "sink growth",
            "pipeline_executor_calls": [["acquisition", 1], ["checkpoint", 1]],
            "pipeline_stage_records": 2,
            "pipeline_replay_dispositions": ["cache_hit", "cache_hit"],
            "promotion_sink_len": 3, "promotion_sink_len_after_replay": 4,
            "scheduler_attempts": 2, "scheduler_attempts_after_replay": 2,
            "scheduler_due_after_replay": 0, "import_eligible": False}},
        {"violation": "scheduler_new_attempts", "record": {
            "scenario": "new attempts",
            "pipeline_executor_calls": [["acquisition", 1], ["checkpoint", 1]],
            "pipeline_stage_records": 2,
            "pipeline_replay_dispositions": ["cache_hit", "cache_hit"],
            "promotion_sink_len": 3, "promotion_sink_len_after_replay": 3,
            "scheduler_attempts": 2, "scheduler_attempts_after_replay": 3,
            "scheduler_due_after_replay": 0, "import_eligible": False}},
        {"violation": "scheduler_due_again", "record": {
            "scenario": "due again",
            "pipeline_executor_calls": [["acquisition", 1], ["checkpoint", 1]],
            "pipeline_stage_records": 2,
            "pipeline_replay_dispositions": ["cache_hit", "cache_hit"],
            "promotion_sink_len": 3, "promotion_sink_len_after_replay": 3,
            "scheduler_attempts": 2, "scheduler_attempts_after_replay": 2,
            "scheduler_due_after_replay": 1, "import_eligible": False}},
        {"violation": "import_leak", "record": {
            "scenario": "import leak",
            "pipeline_executor_calls": [["acquisition", 1], ["checkpoint", 1]],
            "pipeline_stage_records": 2,
            "pipeline_replay_dispositions": ["cache_hit", "cache_hit"],
            "promotion_sink_len": 3, "promotion_sink_len_after_replay": 3,
            "scheduler_attempts": 2, "scheduler_attempts_after_replay": 2,
            "scheduler_due_after_replay": 0, "import_eligible": True}},
    ]))
    _write(root / "crates/da-application/tests/acap_control_plane_test.rs", (
        "struct ControlPlaneEvidenceLedger;\n"
        "// admission:illegal_transition AlreadyPromoted recovered import_eligible\n"
    ))
    _write(root / "crates/da-domain/src/assurance.rs", (
        "pub enum ArticleContextState { }\n"
        "IllegalTransition error_class\n"
    ))
    _write(root / "crates/da-domain/src/pipeline_ledger.rs", (
        "pub struct PipelineStageLedger;\npub struct StageIdRegistry;\n"
    ))
    _write(root / "crates/da-domain/src/scheduler.rs", (
        "pub fn logical_result_vid() {} AttemptOutcome PendingTask\n"
    ))
    return contract

def test_acap_runtime_control_plane_pass_on_clean_fixtures():
    """Positive + negative fixtures and source markers satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    assert service._check_acap_runtime_control_plane() == []

def test_acap_runtime_control_plane_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    (service.root / "governor/fixtures/acap_runtime_control_plane/admission_illegal.negative.json").unlink()
    flags = service._check_acap_runtime_control_plane()
    assert len(flags) == 1
    assert flags[0].flag == "acap_runtime_control_plane_fixture_missing"
    assert "admission/negative" in flags[0].evidence

def test_acap_runtime_control_plane_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    # Inject an import leak into the promotion positive record (D127).
    path = service.root / "governor/fixtures/acap_runtime_control_plane/promotion_authority.positive.json"
    records = json.loads(path.read_text())
    records[0]["import_eligible"] = True
    _write(path, json.dumps(records))
    flags = service._check_acap_runtime_control_plane()
    broken = [f for f in flags if f.flag == "acap_runtime_control_plane_broken"]
    assert len(broken) == 1
    assert "import_leak" in broken[0].evidence

def test_acap_runtime_control_plane_negative_fixture_must_trigger_labelled_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    # Break the self_authority record so its labelled violation no longer fires.
    path = service.root / "governor/fixtures/acap_runtime_control_plane/promotion_authority.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "self_authority":
            record["record"]["promoter"] = "contract:promoter:gold:4"
    _write(path, json.dumps(records))
    flags = service._check_acap_runtime_control_plane()
    broken = [f for f in flags if f.flag == "acap_runtime_control_plane_broken"]
    assert any("self_authority" in f.evidence for f in broken)

def test_acap_runtime_control_plane_negative_fixture_must_cover_required_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    # Remove the scheduler import_leak record entirely — coverage must fail.
    path = service.root / "governor/fixtures/acap_runtime_control_plane/scheduler_recovery.negative.json"
    records = json.loads(path.read_text())
    records = [r for r in records if r.get("violation") != "import_leak"]
    _write(path, json.dumps(records))
    flags = service._check_acap_runtime_control_plane()
    broken = [f for f in flags if f.flag == "acap_runtime_control_plane_broken"]
    assert any("import_leak" in f.evidence for f in broken)

def test_acap_runtime_control_plane_source_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    # Drop the typed guards from the scheduler domain surface.
    _write(service.root / "crates/da-domain/src/scheduler.rs", "pub struct Scheduler;\n")
    flags = service._check_acap_runtime_control_plane()
    marker = [f for f in flags if f.flag == "acap_runtime_control_plane_source_marker_missing"]
    assert len(marker) == 1
    assert "scheduler_domain" in marker[0].evidence

def test_acap_runtime_control_plane_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    _write(service.root / "scripts/governor/checks/acap_runtime_control_plane.yaml",
           "{bad yaml: [")
    flags = service._check_acap_runtime_control_plane()
    assert flags[0].flag == "acap_runtime_control_plane_invalid"


def test_acap_runtime_control_plane_forbidden_orchestrator_match_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    contract_path = service.root / "scripts/governor/checks/acap_runtime_control_plane.yaml"
    payload = yaml.safe_load(contract_path.read_text())
    payload["source_markers"]["admission_orchestrator"] = {
        "path": "crates/da-application/src/pipeline.rs",
        "required_patterns": ["stage_registry"],
        "forbidden_patterns": ["fn acap_state_for_stage"],
    }
    contract_path.write_text(yaml.safe_dump(payload, sort_keys=False))
    _write(
        service.root / "crates/da-application/src/pipeline.rs",
        "fn acap_state_for_stage(id: &str) -> Option<ArticleContextState> { None }\n",
    )
    flags = service._check_acap_runtime_control_plane()
    marker = [f for f in flags if f.flag == "acap_runtime_control_plane_source_marker_missing"]
    assert any("forbidden" in f.evidence for f in marker), [f.evidence for f in marker]


def test_acap_runtime_control_plane_ledger_layers_mismatch_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_acap_runtime_control_plane_contract(service.root)
    contract_path = service.root / "scripts/governor/checks/acap_runtime_control_plane.yaml"
    payload = yaml.safe_load(contract_path.read_text())
    payload["ledger_layers"] = {
        "path": "data/ontology/pipeline_stage_contracts.yaml",
        "required": {
            "parse": "parsed",
            "revision_materialization": "structured",
        },
        "forbidden": ["candidate_materialization"],
    }
    contract_path.write_text(yaml.safe_dump(payload, sort_keys=False))
    _write(
        service.root / "data/ontology/pipeline_stage_contracts.yaml",
        "schema_version: pipeline-stage-contracts.v1\n"
        "import_eligible: false\n"
        "stages:\n"
        "  - id: parse\n"
        "    acap_layer: parsed\n"
        "  - id: revision_materialization\n"
        "    acap_layer: extracted\n"
        "  - id: candidate_materialization\n"
        "    acap_layer: materialized\n",
    )
    flags = service._check_acap_runtime_control_plane()
    missing = [f for f in flags if f.flag == "acap_runtime_control_plane_ledger_layers_missing"]
    evidence = " ".join(f.evidence for f in missing)
    assert "revision_materialization" in evidence, evidence
    assert "candidate_materialization" in evidence, evidence

def _write_html_structure_contract(root: Path) -> Path:
    """Write the HTML structure honesty contract + fixtures into a temp root."""
    contract = root / "scripts/governor/checks/html_structure_honesty.yaml"
    _write(contract, """schema_version: html-structure-honesty.v1
policy_id: html-structure-honesty
dimension: html_structure_ast
severity: high
report_contract:
  required_fields: [article_id, sections]
  section_required_fields: [path_id, title, parent_section_id, previous_path_id, next_path_id, summary]
fixtures:
  positive:
    id: html_path_identity
    path: governor/fixtures/html_structure_contract/html_path_identity.projection.json
    expect:
      ok: true
      unique_paths: true
      all_sections_have_identity: true
  negative:
    id: flat_missing_identity
    path: governor/fixtures/html_structure_contract/flat_missing_identity.blocked.json
    expect:
      ok: false
      readiness: not_claimed
      reason: missing_section_path_id
      import_eligible: false
readiness_markers:
  path: crates/da-application/src/article_load.rs
  required_patterns:
    - 'graph_readiness = "not_claimed"'
    - 'source_profile.as_deref() == Some("textbook")'
""")
    _write(root / "governor/fixtures/html_structure_contract/html_path_identity.projection.json", json.dumps({
        "article_id": "gnn-ch-chapters__01-intro-to-graphs",
        "sections": [
            {"path_id": "1", "title": "Chapter 1", "parent_section_id": None,
             "previous_path_id": None, "next_path_id": "1.1", "summary": None},
            {"path_id": "1.1", "title": "Section A", "parent_section_id": "sec-1",
             "previous_path_id": "1", "next_path_id": "1.2", "summary": None},
            {"path_id": "1.2", "title": "Section B", "parent_section_id": "sec-1",
             "previous_path_id": "1.1", "next_path_id": "1.2.1", "summary": None},
            {"path_id": "1.2.1", "title": "Sub B.1", "parent_section_id": "sec-1.2",
             "previous_path_id": "1.2", "next_path_id": None, "summary": None},
        ],
    }))
    _write(root / "governor/fixtures/html_structure_contract/flat_missing_identity.blocked.json", json.dumps({
        "schema_version": "document-context-blocked.v1",
        "article_id": "gnn-ch-chapters__03-flat",
        "ok": False,
        "readiness": "not_claimed",
        "reason": "missing_section_path_id",
        "detail": "flat HTML sections carry no path identity; blocked fail-closed.",
        "import_eligible": False,
    }))
    _write(root / "crates/da-application/src/article_load.rs", (
        '        if report.source_profile.as_deref() == Some("textbook") {\n'
        '            report.graph_readiness = "not_claimed".into();\n'
        '        }\n'
    ))
    return contract

def test_html_structure_honesty_pass_on_clean_fixtures():
    """Positive projection + negative blocked record satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_html_structure_contract(service.root)
    assert service._check_html_structure_honesty() == []

def test_html_structure_honesty_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_html_structure_contract(service.root)
    (service.root / "governor/fixtures/html_structure_contract/flat_missing_identity.blocked.json").unlink()
    flags = service._check_html_structure_honesty()
    assert len(flags) == 1
    assert flags[0].flag == "html_structure_honesty_fixture_missing"
    assert "negative" in flags[0].evidence

def test_html_structure_honesty_positive_fixture_must_have_full_identity():
    """A section without path_id (or with a phantom parent) breaks the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_html_structure_contract(service.root)
    _write(service.root / "governor/fixtures/html_structure_contract/html_path_identity.projection.json", json.dumps({
        "article_id": "gnn-ch-chapters__01-intro-to-graphs",
        "sections": [
            {"path_id": "1", "title": "Chapter 1", "parent_section_id": None,
             "previous_path_id": None, "next_path_id": "1.1", "summary": None},
            {"path_id": "1.1", "title": "Section A", "parent_section_id": "sec-9",
             "previous_path_id": "1", "next_path_id": None, "summary": None},
        ],
    }))
    flags = service._check_html_structure_honesty()
    broken = [f for f in flags if f.flag == "html_structure_honesty_broken"]
    assert len(broken) == 1, f"expected broken flags: {flags}"
    assert "parent_section_id" in broken[0].evidence
    assert "sec-9" in broken[0].evidence

def test_html_structure_honesty_positive_fixture_must_have_unique_paths():
    """Duplicate path_ids break the honesty contract (unique tree positions)."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_html_structure_contract(service.root)
    _write(service.root / "governor/fixtures/html_structure_contract/html_path_identity.projection.json", json.dumps({
        "article_id": "gnn-ch-chapters__01-intro-to-graphs",
        "sections": [
            {"path_id": "1", "title": "A", "parent_section_id": None,
             "previous_path_id": None, "next_path_id": "1", "summary": None},
            {"path_id": "1", "title": "B", "parent_section_id": None,
             "previous_path_id": "1", "next_path_id": None, "summary": None},
        ],
    }))
    flags = service._check_html_structure_honesty()
    broken = [f for f in flags if f.flag == "html_structure_honesty_broken"]
    assert len(broken) == 1
    assert "not unique" in broken[0].evidence

def test_html_structure_honesty_negative_fixture_must_stay_not_claimed():
    """A blocked record claiming ok=true or readiness is flagged."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_html_structure_contract(service.root)
    _write(service.root / "governor/fixtures/html_structure_contract/flat_missing_identity.blocked.json", json.dumps({
        "schema_version": "document-context-blocked.v1",
        "article_id": "gnn-ch-chapters__03-flat",
        "ok": True,
        "readiness": "ready",
        "reason": "",
        "import_eligible": False,
    }))
    flags = service._check_html_structure_honesty()
    broken = [f for f in flags if f.flag == "html_structure_honesty_broken"]
    assert len(broken) == 1
    assert "ok=false" in broken[0].evidence
    assert "not_claimed/blocked" in broken[0].evidence
    assert "typed reason" in broken[0].evidence

def test_html_structure_honesty_negative_fixture_must_keep_d127_locked():
    """A blocked record with import_eligible=true unlocks D127 — flagged."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_html_structure_contract(service.root)
    _write(service.root / "governor/fixtures/html_structure_contract/flat_missing_identity.blocked.json", json.dumps({
        "schema_version": "document-context-blocked.v1",
        "article_id": "gnn-ch-chapters__03-flat",
        "ok": False,
        "readiness": "not_claimed",
        "reason": "missing_section_path_id",
        "import_eligible": True,
    }))
    flags = service._check_html_structure_honesty()
    broken = [f for f in flags if f.flag == "html_structure_honesty_broken"]
    assert len(broken) == 1
    assert "import_eligible" in broken[0].evidence
    assert "D127" in broken[0].evidence

def test_html_structure_honesty_readiness_marker_missing_is_flagged():
    """The load surface must keep stamping HTML/textbook as not_claimed."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_html_structure_contract(service.root)
    _write(service.root / "crates/da-application/src/article_load.rs", (
        '        report.graph_readiness = "claimed".into();\n'
    ))
    flags = service._check_html_structure_honesty()
    assert len(flags) == 1
    assert flags[0].flag == "html_structure_honesty_readiness_marker_missing"
    assert "graph_readiness = \"not_claimed\"" in flags[0].evidence

def test_html_structure_honesty_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write(service.root / "scripts/governor/checks/html_structure_honesty.yaml",
           "schema_version: [unclosed\n")
    flags = service._check_html_structure_honesty()
    assert len(flags) == 1
    assert flags[0].flag == "html_structure_honesty_invalid"


# ---------------------------------------------------------------------------
# Multimodal Governor contracts (M400 S05 T01, PIR-018/019/020)
# ---------------------------------------------------------------------------
# Self-tests for the three committed multimodal candidate contracts. The
# tests copy the REAL committed YAML contracts, fixtures, Rust source
# markers and graph task suites into a scratch root (never mutating tracked
# files), then validate: every positive fixture must be clean, every
# negative record must trigger exactly its labelled violation, every
# required violation must stay covered, source markers and the graph task
# suite must keep their enforcement points, and invalid YAML must flag.
# No direct database access — TrajectoryService is built via __new__ with a
# scratch root only (same pattern as the other contract self-tests).


def _copy_multimodal_contracts(root: Path) -> None:
    """Copy committed multimodal Governor contracts + fixtures + markers + suites."""
    repo = _scripts_dir.parent
    rels = [
        "scripts/governor/checks/visual_candidates_require_graph_lineage.yaml",
        "scripts/governor/checks/table_payload_requires_cell_provenance.yaml",
        "scripts/governor/checks/equation_context_coverage.yaml",
        "governor/fixtures/visual_lineage_contract/lineage_complete.positive.json",
        "governor/fixtures/visual_lineage_contract/lineage_broken.negative.json",
        "governor/fixtures/table_provenance_contract/provenance_complete.positive.json",
        "governor/fixtures/table_provenance_contract/provenance_broken.negative.json",
        "governor/fixtures/equation_coverage_contract/coverage_complete.positive.json",
        "governor/fixtures/equation_coverage_contract/coverage_broken.negative.json",
        "crates/da-domain/src/figure_candidate.rs",
        "crates/da-domain/src/table_candidate.rs",
        "crates/da-domain/src/table_fidelity.rs",
        "crates/da-domain/src/equation_candidate.rs",
        "data/graph_tasks/figure_candidate.tasks.cypher",
        "data/graph_tasks/table_cell_fidelity.tasks.cypher",
        "data/graph_tasks/equation_candidate.tasks.cypher",
    ]
    for rel in rels:
        _write(root / rel, (repo / rel).read_text(encoding="utf-8"))


def test_visual_candidates_require_graph_lineage_pass_on_clean_fixtures():
    """Committed positive + negative fixtures and source markers satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    assert service._check_visual_candidates_require_graph_lineage() == []


def test_visual_candidates_require_graph_lineage_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    (service.root / "governor/fixtures/visual_lineage_contract/lineage_broken.negative.json").unlink()
    flags = service._check_visual_candidates_require_graph_lineage()
    assert len(flags) == 1
    assert flags[0].flag == "visual_candidates_require_graph_lineage_fixture_missing"
    assert "negative" in flags[0].evidence


def test_visual_candidates_require_graph_lineage_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Inject an import unlock into the positive candidate (D127).
    path = service.root / "governor/fixtures/visual_lineage_contract/lineage_complete.positive.json"
    records = json.loads(path.read_text())
    records[0]["import_eligible"] = True
    _write(path, json.dumps(records))
    flags = service._check_visual_candidates_require_graph_lineage()
    broken = [f for f in flags if f.flag == "visual_candidates_require_graph_lineage_broken"]
    assert len(broken) == 1
    assert "import_unlock" in broken[0].evidence


def test_visual_candidates_require_graph_lineage_negative_fixture_must_trigger_labelled_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Neutralise the import_unlock record so its labelled violation no longer fires.
    path = service.root / "governor/fixtures/visual_lineage_contract/lineage_broken.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "import_unlock":
            record["record"]["import_eligible"] = False
    _write(path, json.dumps(records))
    flags = service._check_visual_candidates_require_graph_lineage()
    broken = [f for f in flags if f.flag == "visual_candidates_require_graph_lineage_broken"]
    assert any("import_unlock" in f.evidence for f in broken)


def test_visual_candidates_require_graph_lineage_negative_fixture_must_cover_required_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Remove the authority_unlock record entirely — coverage must fail.
    path = service.root / "governor/fixtures/visual_lineage_contract/lineage_broken.negative.json"
    records = json.loads(path.read_text())
    records = [r for r in records if r.get("violation") != "authority_unlock"]
    _write(path, json.dumps(records))
    flags = service._check_visual_candidates_require_graph_lineage()
    broken = [f for f in flags if f.flag == "visual_candidates_require_graph_lineage_broken"]
    assert any("authority_unlock" in f.evidence for f in broken)


def test_visual_candidates_require_graph_lineage_source_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Drop the typed guards from the domain surface.
    _write(service.root / "crates/da-domain/src/figure_candidate.rs",
           "pub struct FigureCandidate;\n")
    flags = service._check_visual_candidates_require_graph_lineage()
    marker = [f for f in flags
              if f.flag == "visual_candidates_require_graph_lineage_source_marker_missing"]
    assert len(marker) == 1
    assert "domain" in marker[0].evidence


def test_visual_candidates_require_graph_lineage_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    _write(service.root / "scripts/governor/checks/visual_candidates_require_graph_lineage.yaml",
           "{bad yaml: [")
    flags = service._check_visual_candidates_require_graph_lineage()
    assert flags[0].flag == "visual_candidates_require_graph_lineage_invalid"


def test_table_payload_requires_cell_provenance_pass_on_clean_fixtures():
    """Committed positive + negative fixtures and source markers satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    assert service._check_table_payload_requires_cell_provenance() == []


def test_table_payload_requires_cell_provenance_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    (service.root / "governor/fixtures/table_provenance_contract/provenance_broken.negative.json").unlink()
    flags = service._check_table_payload_requires_cell_provenance()
    assert len(flags) == 1
    assert flags[0].flag == "table_payload_requires_cell_provenance_fixture_missing"
    assert "negative" in flags[0].evidence


def test_table_payload_requires_cell_provenance_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Inject an import unlock into the positive candidate (D127).
    path = service.root / "governor/fixtures/table_provenance_contract/provenance_complete.positive.json"
    records = json.loads(path.read_text())
    records[0]["import_eligible"] = True
    _write(path, json.dumps(records))
    flags = service._check_table_payload_requires_cell_provenance()
    broken = [f for f in flags if f.flag == "table_payload_requires_cell_provenance_broken"]
    assert len(broken) == 1
    assert "import_unlock" in broken[0].evidence


def test_table_payload_requires_cell_provenance_negative_fixture_must_trigger_labelled_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Neutralise the import_unlock record so its labelled violation no longer fires.
    path = service.root / "governor/fixtures/table_provenance_contract/provenance_broken.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "import_unlock":
            record["record"]["import_eligible"] = False
    _write(path, json.dumps(records))
    flags = service._check_table_payload_requires_cell_provenance()
    broken = [f for f in flags if f.flag == "table_payload_requires_cell_provenance_broken"]
    assert any("import_unlock" in f.evidence for f in broken)


def test_table_payload_requires_cell_provenance_negative_fixture_must_cover_required_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Remove the cell_without_digest record entirely — coverage must fail.
    path = service.root / "governor/fixtures/table_provenance_contract/provenance_broken.negative.json"
    records = json.loads(path.read_text())
    records = [r for r in records if r.get("violation") != "cell_without_digest"]
    _write(path, json.dumps(records))
    flags = service._check_table_payload_requires_cell_provenance()
    broken = [f for f in flags if f.flag == "table_payload_requires_cell_provenance_broken"]
    assert any("cell_without_digest" in f.evidence for f in broken)


def test_table_payload_requires_cell_provenance_source_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Drop the typed guards from the domain surface.
    _write(service.root / "crates/da-domain/src/table_candidate.rs",
           "pub struct TableCandidate;\n")
    flags = service._check_table_payload_requires_cell_provenance()
    marker = [f for f in flags
              if f.flag == "table_payload_requires_cell_provenance_source_marker_missing"]
    assert len(marker) == 1
    assert "domain" in marker[0].evidence


def test_table_payload_requires_cell_provenance_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    _write(service.root / "scripts/governor/checks/table_payload_requires_cell_provenance.yaml",
           "{bad yaml: [")
    flags = service._check_table_payload_requires_cell_provenance()
    assert flags[0].flag == "table_payload_requires_cell_provenance_invalid"


def test_equation_context_coverage_pass_on_clean_fixtures():
    """Committed positive + negative fixtures and source markers satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    assert service._check_equation_context_coverage() == []


def test_equation_context_coverage_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    (service.root / "governor/fixtures/equation_coverage_contract/coverage_broken.negative.json").unlink()
    flags = service._check_equation_context_coverage()
    assert len(flags) == 1
    assert flags[0].flag == "equation_context_coverage_fixture_missing"
    assert "negative" in flags[0].evidence


def test_equation_context_coverage_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Inject an import unlock into the positive candidate (D127).
    path = service.root / "governor/fixtures/equation_coverage_contract/coverage_complete.positive.json"
    records = json.loads(path.read_text())
    records[0]["import_eligible"] = True
    _write(path, json.dumps(records))
    flags = service._check_equation_context_coverage()
    broken = [f for f in flags if f.flag == "equation_context_coverage_broken"]
    assert len(broken) == 1
    assert "import_unlock" in broken[0].evidence


def test_equation_context_coverage_negative_fixture_must_trigger_labelled_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Neutralise the import_unlock record so its labelled violation no longer fires.
    path = service.root / "governor/fixtures/equation_coverage_contract/coverage_broken.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "import_unlock":
            record["record"]["import_eligible"] = False
    _write(path, json.dumps(records))
    flags = service._check_equation_context_coverage()
    broken = [f for f in flags if f.flag == "equation_context_coverage_broken"]
    assert any("import_unlock" in f.evidence for f in broken)


def test_equation_context_coverage_negative_fixture_must_cover_required_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Remove the invalid_span record entirely — coverage must fail.
    path = service.root / "governor/fixtures/equation_coverage_contract/coverage_broken.negative.json"
    records = json.loads(path.read_text())
    records = [r for r in records if r.get("violation") != "invalid_span"]
    _write(path, json.dumps(records))
    flags = service._check_equation_context_coverage()
    broken = [f for f in flags if f.flag == "equation_context_coverage_broken"]
    assert any("invalid_span" in f.evidence for f in broken)


def test_equation_context_coverage_source_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    # Drop the typed guards from the domain surface.
    _write(service.root / "crates/da-domain/src/equation_candidate.rs",
           "pub struct EquationCandidate;\n")
    flags = service._check_equation_context_coverage()
    marker = [f for f in flags
              if f.flag == "equation_context_coverage_source_marker_missing"]
    assert len(marker) == 1
    assert "domain" in marker[0].evidence


def test_equation_context_coverage_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_multimodal_contracts(service.root)
    _write(service.root / "scripts/governor/checks/equation_context_coverage.yaml",
           "{bad yaml: [")
    flags = service._check_equation_context_coverage()
    assert flags[0].flag == "equation_context_coverage_invalid"


def _copy_read_only_agent_contract(root: Path) -> None:
    """Copy the committed read-only agent contract + envelopes + markers."""
    repo = _scripts_dir.parent
    rels = [
        "scripts/governor/checks/read_only_agent_contract.yaml",
        "governor/fixtures/read_only_agent_contract/agent_run_verified.positive.json",
        "governor/fixtures/read_only_agent_contract/agent_run_tampered.negative.json",
        "crates/da-cli/src/agent_run.rs",
        "crates/da-application/src/agent_run.rs",
        "crates/kg-storage/src/read_only_server.rs",
        "crates/da-cli/src/serve_graph.rs",
        "crates/da-domain/src/agent.rs",
    ]
    for rel in rels:
        _write(root / rel, (repo / rel).read_text(encoding="utf-8"))


def test_read_only_agent_contract_pass_on_clean_fixtures():
    """Committed positive + negative envelopes and markers satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_read_only_agent_contract(service.root)
    assert service._check_read_only_agent_contract() == []


def test_read_only_agent_contract_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_read_only_agent_contract(service.root)
    (service.root / "governor/fixtures/read_only_agent_contract/agent_run_tampered.negative.json").unlink()
    flags = service._check_read_only_agent_contract()
    assert len(flags) == 1
    assert flags[0].flag == "read_only_agent_contract_fixture_missing"
    assert "negative" in flags[0].evidence



def _feature_matrix_gate_setup(root: Path, gate_script: str) -> None:
    """Copy the committed feature-matrix policy + write a gate script variant."""
    repo = _scripts_dir.parent
    rel = "scripts/governor/checks/feature_matrix_gate.yaml"
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(repo / rel, root / rel)
    _write(root / "scripts/verify_rust_architecture.sh", gate_script)

def test_feature_matrix_gate_pass_on_clean_gate_script():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _feature_matrix_gate_setup(
        service.root,
        "# feature matrix\ncargo check -p da-cli --features pdf-classifier --examples\n",
    )
    assert service._check_feature_matrix_gate() == []

def test_feature_matrix_gate_missing_pin_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _feature_matrix_gate_setup(service.root, "cargo clippy only\n")
    flags = service._check_feature_matrix_gate()
    marker = [f for f in flags if f.flag == "feature_matrix_gate_pin_missing"]
    assert len(marker) == 1

def test_feature_matrix_gate_source_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _feature_matrix_gate_setup(service.root, "")
    # The policy exists; remove the gate script it pins.
    import os
    os.remove(service.root / "scripts/verify_rust_architecture.sh")
    flags = service._check_feature_matrix_gate()
    marker = [f for f in flags if f.flag == "feature_matrix_gate_source_missing"]
    assert len(marker) == 1

def test_feature_matrix_gate_absent_policy_is_silent():
    # No policy file -> no check output (checks are opt-in per project).
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    assert service._check_feature_matrix_gate() == []

def test_read_only_agent_contract_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_read_only_agent_contract(service.root)
    # Inject an import_eligible key into the positive envelope (D127 unlock).
    path = service.root / "governor/fixtures/read_only_agent_contract/agent_run_verified.positive.json"
    envelope = json.loads(path.read_text(encoding="utf-8"))
    envelope["import_eligible"] = True
    _write(path, json.dumps(envelope))
    flags = service._check_read_only_agent_contract()
    broken = [f for f in flags if f.flag == "read_only_agent_contract_broken"]
    assert len(broken) == 1
    assert "import_eligible" in broken[0].evidence


def test_read_only_agent_contract_negative_fixture_must_be_structurally_clean():
    """The tampered negative is still a valid v1 envelope: Python stays clean, Rust FAILs."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_read_only_agent_contract(service.root)
    # Break the negative envelope's structure: drop the run block entirely.
    path = service.root / "governor/fixtures/read_only_agent_contract/agent_run_tampered.negative.json"
    envelope = json.loads(path.read_text(encoding="utf-8"))
    del envelope["run"]
    _write(path, json.dumps(envelope))
    flags = service._check_read_only_agent_contract()
    broken = [f for f in flags if f.flag == "read_only_agent_contract_broken"]
    assert len(broken) == 1
    assert "missing_required_field:run" in broken[0].evidence


def test_read_only_agent_contract_fixture_must_be_object_not_array():
    """A fixture is a single envelope object — `_candidate_contract_flags` (arrays) is NOT reused."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_read_only_agent_contract(service.root)
    path = service.root / "governor/fixtures/read_only_agent_contract/agent_run_verified.positive.json"
    envelope = json.loads(path.read_text(encoding="utf-8"))
    _write(path, json.dumps([envelope]))
    flags = service._check_read_only_agent_contract()
    invalid = [f for f in flags if f.flag == "read_only_agent_contract_fixture_invalid"]
    assert len(invalid) == 1
    assert "single envelope object" in invalid[0].evidence


def test_read_only_agent_contract_source_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_read_only_agent_contract(service.root)
    # Drop the serve-graph enforcement markers.
    _write(service.root / "crates/da-cli/src/serve_graph.rs",
           "// no markers here\n")
    flags = service._check_read_only_agent_contract()
    marker = [f for f in flags
              if f.flag == "read_only_agent_contract_source_marker_missing"]
    assert len(marker) == 1
    assert "serve_graph" in marker[0].evidence


def test_read_only_agent_contract_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_read_only_agent_contract(service.root)
    _write(service.root / "scripts/governor/checks/read_only_agent_contract.yaml",
           "{bad yaml: [")
    flags = service._check_read_only_agent_contract()
    assert len(flags) == 1
    assert flags[0].flag == "read_only_agent_contract_invalid"


# ═══ M449: Graph Semantics + Data Integrity ═══════════════════════════

def test_graph_semantics_no_snapshots_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_graph_semantics_contract(service.root)
    from governor.adapters.graph_query_reader import check_graph_semantics
    flags = check_graph_semantics(service.root)
    assert any(f.flag == "graph_semantics_no_snapshots" for f in flags)


def test_data_integrity_hardcoded_url_is_caught():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_data_integrity_contract(service.root)
    # Write a file with a hardcoded URL
    _write(service.root / "crates/da-test/src/api.rs",
           'const URL: &str = "https://api.some-unknown-service.io/v1";\n')
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert any(f.flag == "data_integrity_hardcoded_url" for f in flags)


def test_data_integrity_w3_namespace_url_is_allowed():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_data_integrity_contract(service.root)
    _write(service.root / "crates/da-test/src/xml.rs",
           'const NS: &str = "http://www.w3.org/1998/Math/MathML";\n')
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert not any(f.flag == "data_integrity_hardcoded_url" for f in flags)


def test_data_integrity_nested_dir_is_caught():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_data_integrity_contract(service.root)
    # Create nested dir
    (service.root / "data/article_catalog/article_catalog").mkdir(parents=True)
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert any(f.flag == "data_integrity_nested_dir" for f in flags)


def test_data_integrity_cache_binding_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_data_integrity_contract(service.root)
    # Write a stage file WITHOUT the required pattern
    _write(service.root / "crates/da-adapters/src/pipeline_stages.rs",
           "// no cache binding here\n")
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert any(f.flag == "data_integrity_cache_binding_missing" for f in flags)


def _copy_graph_semantics_contract(root: Path) -> None:
    repo = _scripts_dir.parent
    rel = "scripts/governor/checks/graph_semantics.yaml"
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    import shutil
    shutil.copy2(repo / rel, root / rel)


def _copy_data_integrity_contract(root: Path) -> None:
    repo = _scripts_dir.parent
    rel = "scripts/governor/checks/data_integrity.yaml"
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    import shutil
    shutil.copy2(repo / rel, root / rel)

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
        test_section_navigation_contract_pass_on_clean_fixtures,
        test_section_navigation_contract_missing_fixture_is_flagged,
        test_section_navigation_contract_positive_fixture_must_be_clean,
        test_section_navigation_contract_negative_fixture_must_report_codes,
        test_section_navigation_contract_invalid_yaml_is_flagged,
        test_section_navigation_contract_suite_missing_is_flagged,
        test_canonical_temporal_wire_format_pass_on_clean_fixtures,
        test_canonical_temporal_wire_format_missing_fixture_is_flagged,
        test_canonical_temporal_wire_format_ontology_marker_missing_is_flagged,
        test_canonical_temporal_wire_format_positive_fixture_must_be_clean,
        test_canonical_temporal_wire_format_negative_fixture_must_be_rejected,
        test_canonical_temporal_wire_format_invalid_yaml_is_flagged,
        test_promotion_evidence_contract_pass_on_clean_fixtures,
        test_promotion_evidence_contract_missing_fixture_is_flagged,
        test_promotion_evidence_contract_positive_fixture_must_be_clean,
        test_promotion_evidence_contract_negative_fixture_must_trigger_labelled_violations,
        test_promotion_evidence_contract_negative_fixture_must_cover_required_violations,
        test_promotion_evidence_contract_source_marker_missing_is_flagged,
        test_promotion_evidence_contract_invalid_yaml_is_flagged,
        test_acap_runtime_control_plane_pass_on_clean_fixtures,
        test_acap_runtime_control_plane_missing_fixture_is_flagged,
        test_acap_runtime_control_plane_positive_fixture_must_be_clean,
        test_acap_runtime_control_plane_negative_fixture_must_trigger_labelled_violations,
        test_acap_runtime_control_plane_negative_fixture_must_cover_required_violations,
        test_acap_runtime_control_plane_source_marker_missing_is_flagged,
        test_acap_runtime_control_plane_invalid_yaml_is_flagged,
        test_acap_runtime_control_plane_forbidden_orchestrator_match_is_flagged,
        test_acap_runtime_control_plane_ledger_layers_mismatch_is_flagged,
        test_html_structure_honesty_pass_on_clean_fixtures,
        test_html_structure_honesty_missing_fixture_is_flagged,
        test_html_structure_honesty_positive_fixture_must_have_full_identity,
        test_html_structure_honesty_positive_fixture_must_have_unique_paths,
        test_html_structure_honesty_negative_fixture_must_stay_not_claimed,
        test_html_structure_honesty_negative_fixture_must_keep_d127_locked,
        test_html_structure_honesty_readiness_marker_missing_is_flagged,
        test_html_structure_honesty_invalid_yaml_is_flagged,
        test_visual_candidates_require_graph_lineage_pass_on_clean_fixtures,
        test_visual_candidates_require_graph_lineage_missing_fixture_is_flagged,
        test_visual_candidates_require_graph_lineage_positive_fixture_must_be_clean,
        test_visual_candidates_require_graph_lineage_negative_fixture_must_trigger_labelled_violations,
        test_visual_candidates_require_graph_lineage_negative_fixture_must_cover_required_violations,
        test_visual_candidates_require_graph_lineage_source_marker_missing_is_flagged,
        test_visual_candidates_require_graph_lineage_invalid_yaml_is_flagged,
        test_table_payload_requires_cell_provenance_pass_on_clean_fixtures,
        test_table_payload_requires_cell_provenance_missing_fixture_is_flagged,
        test_table_payload_requires_cell_provenance_positive_fixture_must_be_clean,
        test_table_payload_requires_cell_provenance_negative_fixture_must_trigger_labelled_violations,
        test_table_payload_requires_cell_provenance_negative_fixture_must_cover_required_violations,
        test_table_payload_requires_cell_provenance_source_marker_missing_is_flagged,
        test_table_payload_requires_cell_provenance_invalid_yaml_is_flagged,
        test_equation_context_coverage_pass_on_clean_fixtures,
        test_equation_context_coverage_missing_fixture_is_flagged,
        test_equation_context_coverage_positive_fixture_must_be_clean,
        test_equation_context_coverage_negative_fixture_must_trigger_labelled_violations,
        test_equation_context_coverage_negative_fixture_must_cover_required_violations,
        test_equation_context_coverage_source_marker_missing_is_flagged,
        test_equation_context_coverage_invalid_yaml_is_flagged,
        test_read_only_agent_contract_pass_on_clean_fixtures,
        test_read_only_agent_contract_missing_fixture_is_flagged,
        test_read_only_agent_contract_positive_fixture_must_be_clean,
        test_read_only_agent_contract_negative_fixture_must_be_structurally_clean,
        test_read_only_agent_contract_fixture_must_be_object_not_array,
        test_read_only_agent_contract_source_marker_missing_is_flagged,
        test_read_only_agent_contract_invalid_yaml_is_flagged,
        test_feature_matrix_gate_pass_on_clean_gate_script,
        test_feature_matrix_gate_missing_pin_is_flagged,
        test_feature_matrix_gate_source_missing_is_flagged,
        test_feature_matrix_gate_absent_policy_is_silent,
        test_graph_semantics_no_snapshots_is_flagged,
        test_data_integrity_hardcoded_url_is_caught,
        test_data_integrity_w3_namespace_url_is_allowed,
        test_data_integrity_nested_dir_is_caught,
        test_data_integrity_cache_binding_missing_is_flagged,
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




# --- schema_hierarchy checker (ADR-068, M451) ---

def _schema_root(tmp_path: Path) -> Path:
    """Build a minimal tree conforming to ADR-068 for the checker."""
    root = tmp_path / "repo"
    (root / "data").mkdir(parents=True)
    (root / "data" / "SCHEMA.yaml").write_text(
        "schema_version: v1\nvalid_subdirs: [sources]\nvalid_files: ['*.yaml']\n"
    )
    (root / "data" / "sources").mkdir()
    (root / "data" / "sources" / "SCHEMA.yaml").write_text("schema_version: v1\n")
    (root / "data" / "MANIFEST.yaml").write_text("generated_at: 2026-08-23T00:00:00Z\n")
    (root / "scripts" / "governor" / "checks").mkdir(parents=True)
    (root / "scripts" / "governor" / "checks" / "schema_hierarchy.yaml").write_text(
        "schema_version: schema-hierarchy.v2\n"
        "dimension: schema_hierarchy\nseverity: medium\n"
        "structural_directories: [data, data/sources]\n"
        "aggregate_directories: [data]\n"
        "forbidden_top_level: [old_*]\n"
    )
    return root


def _checker(root: Path):
    from governor.adapters.schema_hierarchy_checker import check_schema_hierarchy
    return check_schema_hierarchy(root)


def test_schema_hierarchy_clean_tree_has_no_flags(tmp_path):
    root = _schema_root(tmp_path)
    flags = _checker(root)
    assert flags == []


def test_schema_hierarchy_flags_forbidden_top_level(tmp_path):
    root = _schema_root(tmp_path)
    (root / "old_junk").mkdir()
    flags = _checker(root)
    assert any(f.flag == "forbidden_directory" and "old_junk" in f.evidence for f in flags)


def test_schema_hierarchy_flags_missing_schema(tmp_path):
    root = _schema_root(tmp_path)
    (root / "data" / "SCHEMA.yaml").unlink()
    flags = _checker(root)
    assert any(f.flag == "schema_missing" and "data" in f.evidence for f in flags)


def test_schema_hierarchy_flags_subdir_not_in_schema(tmp_path):
    root = _schema_root(tmp_path)
    (root / "data" / "junkdir").mkdir()
    flags = _checker(root)
    assert any(f.flag == "subdir_not_in_schema" and "junkdir" in f.evidence for f in flags)


def test_schema_hierarchy_flags_file_not_in_schema(tmp_path):
    root = _schema_root(tmp_path)
    (root / "data" / "stray.exe").write_text("x")
    flags = _checker(root)
    assert any(f.flag == "file_not_in_schema" and "stray.exe" in f.evidence for f in flags)
