"""Tests for governor hexagonal package.

Run with: PYTHONPATH=scripts python3 -m pytest tests/ -v
Or:       python3 tests/test_governor.py
"""

import gzip
import hashlib
import json
import os
import re
import subprocess
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
from governor.adapters.checks.conservation_verdict_provenance import (
    conservation_provenance_violations,
    missing_provenance_vocabulary,
)
from governor.adapters.checks.executed_population_binding import (
    declared_report_paths,
    executed_population_violations,
    missing_binding_vocabulary,
)
from governor.application.trajectory_service import (
    TrajectoryService,
    _boundary_positional_violations,
    _candidate_contract_flags,
    _license_provenance_violations,
    _oracle_digest_violations,
    _visual_adapter_violations,
)
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
    # ADR-034 onion invariants are BLOCKING (severity "high"):
    # governor.domain.checks.CheckResult.is_blocking is `severity == "high"`, so a
    # `medium` here would make the invariant unable to fail the gate (the T03
    # regression passed a green `governor architecture` exactly that way).
    assert parse_violations[0].severity == "high"


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
    # ADR-034 onion invariants are BLOCKING (severity "high"); see adr_rules.yaml.
    assert parse_violations[0].severity == "high"


def test_da_domain_must_not_depend_on_infra():
    """Prod infra (tokio) on da-domain is a hexagonal violation; blocking severity."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-domain" / "Cargo.toml",
        """[package]
name = "da-domain"

[dependencies]
tokio = { workspace = true }
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    infra_violations = [
        v
        for r in report.results
        if r.rule.id == "da_domain_must_not_depend_on_infra"
        for v in r.violations
    ]
    assert len(infra_violations) >= 1, "Expected da_domain_must_not_depend_on_infra"
    assert infra_violations[0].severity == "high"


def test_da_application_must_not_depend_on_da_adapters_prod():
    """Prod da-adapters on da-application is a hexagonal violation; blocking severity."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-application" / "Cargo.toml",
        """[package]
name = "da-application"

[dependencies]
da-adapters = { workspace = true }
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    adapter_violations = [
        v
        for r in report.results
        if r.rule.id == "da_application_must_not_depend_on_da_adapters"
        for v in r.violations
    ]
    assert len(adapter_violations) >= 1, "Expected da_application_must_not_depend_on_da_adapters"
    assert adapter_violations[0].severity == "high"


def test_onion_inversion_blocks_trajectory_verdict():
    """A high-severity reverse-ADR violation must BLOCK the trajectory verdict.

    Regression guard for M506/S05: T03 added `da-application -> da-parse` and it
    passed a green `governor architecture` because the rule was severity=medium
    (non-blocking). Raising the ADR-034 rules to `high` only helps if the audit's
    `has_blocking` actually flips the trajectory verdict, so assert that end to end.
    """
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
    report = AuditService(scanner=scanner, rule_repo=rule_repo).run_audit()

    assert report.has_blocking, (
        "onion inversion (da-application -> da-parse) must be blocking; "
        "a non-blocking severity would let a real inversion ship"
    )

    service = TrajectoryService.__new__(TrajectoryService)
    service.root = root
    verdict = service._derive_verdict(report, [], "preflight")
    assert verdict == TrajectoryVerdict.BLOCKED


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


def test_da_parse_must_not_embed_learning_or_graph_runtimes():
    """Active SONA/LoRA work stays behind ports, not in deterministic parse deps."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-parse" / "Cargo.toml",
        """[package]
name = "da-parse"

[dependencies]
da-ports = { workspace = true }
sona = { workspace = true }
ruvllm = { workspace = true }
redb = "2"
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    violations = [
        v
        for r in report.results
        if r.rule.id == "da_parse_must_not_embed_learning_or_graph_runtimes"
        for v in r.violations
    ]
    assert len(violations) == 3
    assert all(v.severity == "high" for v in violations)


def test_da_parse_direct_cpu_substrate_dependencies_are_allowed():
    """Narrow measured CPU/SMP deps are allowed by ADR-075."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-parse" / "Cargo.toml",
        """[package]
name = "da-parse"

[dependencies]
da-ports = { workspace = true }
rayon = "1"
crossbeam = "0.8"
memmap2 = "0.9"
bytemuck = "1"
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    violations = [
        v
        for r in report.results
        if r.rule.id in {
            "da_parse_must_not_embed_learning_or_graph_runtimes",
            "da_parse_must_not_embed_jvm",
        }
        for v in r.violations
    ]
    assert violations == []


def test_da_parse_must_not_embed_jvm():
    """The native platform must not pull the grobid-rs JNI engine in-process."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"

    _write(
        root / "crates" / "da-parse" / "Cargo.toml",
        """[package]
name = "da-parse"

[dependencies]
jni = "0.21"
""",
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()

    violations = [
        v
        for r in report.results
        if r.rule.id == "da_parse_must_not_embed_jvm"
        for v in r.violations
    ]
    assert len(violations) == 1
    assert violations[0].severity == "high"


def test_parse_persistence_authority_and_memory_only_experience_are_caught():
    """D295 keeps unique learning state in RVF and redb rebuildable."""
    root = _make_root()
    checks_dir = Path(__file__).resolve().parent.parent / "governor" / "checks"
    _write(
        root / "artifacts" / "parse" / "state.json",
        '{"redb_authoritative": true, "experience_persistence": "memory-only"}',
    )

    scanner = FilesystemCodeScanner(root)
    rule_repo = YAMLRuleRepository(checks_dir)
    audit = AuditService(scanner=scanner, rule_repo=rule_repo)
    report = audit.run_audit()
    ids = {
        r.rule.id
        for r in report.results
        if r.violations
    }
    assert "no_authoritative_redb_parse_state_in_artifacts" in ids
    assert "no_memory_only_parse_experience_in_artifacts" in ids


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
    parse_dimension = next(d for d in dims if d["id"] == "scholarly_parse_plane")
    assert "staged_platform_decided" in parse_dimension["status_logic"]
    assert parse_dimension["artifact_ref"].endswith("ADR-075-scholarly-parse-plane-staged-platform.md")

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
qualification_contract:
  policy:
    minimum_independent_producers: 3
  measured_fields:
    contamination: [detected, not_detected, not_assessed]
    producer_provenance: [method, authority, code_digest]
  disqualifiers: [shared_producer_digest, weak_producer_independence, contamination_detected, contamination_not_assessed]
  fixtures:
    positive:
      id: family_qualification_clean
      path: governor/fixtures/promotion_contract/family_qualification.positive.json
      expect: qualified
    negative:
      id: family_qualification_disqualifiers
      path: governor/fixtures/promotion_contract/family_qualification.negative.json
      expect:
        require_violations: [shared_producer_digest, weak_producer_independence, contamination_detected, contamination_not_assessed]
source_markers:
  domain:
    path: crates/da-domain/src/promotion.rs
    required_patterns: [ImportEligibleLeak, SelfConfirmation, AlreadyPromoted, TerminalMutation]
  service:
    path: crates/da-application/src/promotion.rs
    required_patterns: [PromotionCommitSink, PromotionTransitionRequest]
  qualification:
    path: crates/da-application/src/native_family_promotion_gate.rs
    required_patterns: [PromotionDisqualifier, derive_promotion_qualification, minimum_independent_producers, ContaminationAssessment]
""")
    _write(root / "crates/da-domain/src/promotion.rs", (
        "pub enum PromotionError { ImportEligibleLeak, SelfConfirmation, "
        "AlreadyPromoted, TerminalMutation }\n"
    ))
    _write(root / "crates/da-application/src/promotion.rs", (
        "pub struct PromotionService;\n"
        "PromotionCommitSink PromotionTransitionRequest\n"
    ))
    _write(root / "crates/da-application/src/native_family_promotion_gate.rs", (
        "pub enum PromotionDisqualifier { SharedProducerDigest, WeakProducerIndependence }\n"
        "pub fn derive_promotion_qualification() {}\n"
        "pub struct PromotionQualificationPolicy { minimum_independent_producers: usize }\n"
        "pub use ContaminationAssessment;\n"
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

    def _producer(code: str, method: str = "same-n-critic") -> dict:
        return {
            "method": method,
            "authority": "fixture-authority",
            "code_digest": code * 64,
            "input_digest": "c" * 64,
        }

    _write(
        root / "governor/fixtures/promotion_contract/family_qualification.positive.json",
        json.dumps([
            {
                "contamination": "not_detected",
                "producer_provenance": [_producer("1"), _producer("2"), _producer("3")],
            },
        ]),
    )
    _write(
        root / "governor/fixtures/promotion_contract/family_qualification.negative.json",
        json.dumps([
            {
                "violation": "shared_producer_digest",
                "package": {
                    "contamination": "not_detected",
                    "producer_provenance": [
                        _producer("1"), _producer("1"), _producer("2"), _producer("3"),
                    ],
                },
            },
            {
                "violation": "weak_producer_independence",
                "package": {
                    "contamination": "not_detected",
                    "producer_provenance": [_producer("4"), _producer("5")],
                },
            },
            {
                "violation": "weak_producer_independence",
                "package": {
                    "contamination": "not_detected",
                    "producer_provenance": [
                        _producer("6"), _producer("7"), _producer("8", method="   "),
                    ],
                },
            },
            {
                "violation": "contamination_not_assessed",
                "package": {
                    "producer_provenance": [_producer("9"), _producer("a"), _producer("b")],
                },
            },
            {
                "violation": "contamination_detected",
                "package": {
                    "contamination": "detected",
                    "producer_provenance": [_producer("d"), _producer("e"), _producer("f")],
                },
            },
        ]),
    )
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

def test_promotion_evidence_contract_qualification_pass_on_clean_fixture():
    """A measured-clean qualification fixture plus full disqualifier coverage passes."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    assert service._check_promotion_evidence_contract() == []

def test_promotion_evidence_contract_qualification_positive_fixture_must_be_clean():
    """Unknown contamination in the positive fixture is never read as clean."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    # Remove the measurement entirely: an absent assessment is unknown.
    path = service.root / "governor/fixtures/promotion_contract/family_qualification.positive.json"
    packages = json.loads(path.read_text())
    del packages[0]["contamination"]
    _write(path, json.dumps(packages))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert len(broken) == 1
    assert "qualification positive" in broken[0].evidence
    assert "contamination_not_assessed" in broken[0].evidence

def test_promotion_evidence_contract_qualification_shared_digest_cannot_pass_as_clean():
    """A package whose producers share one measured digest is not clean."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    path = service.root / "governor/fixtures/promotion_contract/family_qualification.positive.json"
    packages = json.loads(path.read_text())
    producers = packages[0]["producer_provenance"]
    producers[1]["code_digest"] = producers[0]["code_digest"]
    _write(path, json.dumps(packages))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert len(broken) == 1
    assert "qualification positive" in broken[0].evidence
    assert "shared_producer_digest" in broken[0].evidence

def test_promotion_evidence_contract_qualification_weak_independence_cannot_pass_as_clean():
    """A measured producer set below the declared cardinality is not clean."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    path = service.root / "governor/fixtures/promotion_contract/family_qualification.positive.json"
    packages = json.loads(path.read_text())
    packages[0]["producer_provenance"] = packages[0]["producer_provenance"][:2]
    _write(path, json.dumps(packages))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert len(broken) == 1
    assert "qualification positive" in broken[0].evidence
    assert "weak_producer_independence" in broken[0].evidence

def test_promotion_evidence_contract_qualification_negative_fixture_must_trigger_labelled_disqualifiers():
    """A negative record drifting away from its labelled disqualifier is flagged."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    path = service.root / "governor/fixtures/promotion_contract/family_qualification.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "shared_producer_digest":
            producers = record["package"]["producer_provenance"]
            producers[1]["code_digest"] = "9" * 64
    _write(path, json.dumps(records))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert any("shared_producer_digest" in f.evidence for f in broken)

def test_promotion_evidence_contract_qualification_unknown_contamination_is_flagged():
    """Laundering an unknown contamination measurement into clean fails closed."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    path = service.root / "governor/fixtures/promotion_contract/family_qualification.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "contamination_not_assessed":
            record["package"]["contamination"] = "not_detected"
    _write(path, json.dumps(records))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert any("contamination_not_assessed" in f.evidence for f in broken)

def test_promotion_evidence_contract_qualification_negative_fixture_must_cover_disqualifiers():
    """Removing a disqualifier case fails the coverage requirement."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    path = service.root / "governor/fixtures/promotion_contract/family_qualification.negative.json"
    records = json.loads(path.read_text())
    records = [r for r in records if r.get("violation") != "contamination_detected"]
    _write(path, json.dumps(records))
    flags = service._check_promotion_evidence_contract()
    broken = [f for f in flags if f.flag == "promotion_evidence_contract_broken"]
    assert any("missing required disqualifier coverage" in f.evidence for f in broken)
    assert any("contamination_detected" in f.evidence for f in broken)

def test_promotion_evidence_contract_qualification_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _write_promotion_contract(service.root)
    (service.root / "governor/fixtures/promotion_contract/family_qualification.negative.json").unlink()
    flags = service._check_promotion_evidence_contract()
    missing = [f for f in flags if f.flag == "promotion_evidence_contract_fixture_missing"]
    assert any("qualification negative" in f.evidence for f in missing)

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


def _copy_boundary_positional_contract(root: Path) -> None:
    """Copy the committed boundary positional metrics contract, fixtures and markers.

    `data/parse/document_segmentation.yaml` is required for the pinned SHA
    binding and the Rust shadow runner for the source markers; without them
    the check reports `..._binding_missing` / `..._source_marker_missing`
    instead of exercising the fixture logic (same pattern as
    `_copy_multimodal_contracts`).
    """
    repo = _scripts_dir.parent
    rels = [
        "scripts/governor/checks/boundary_positional_metrics.yaml",
        "governor/fixtures/boundary_positional_metrics/positional_clean.positive.json",
        "governor/fixtures/boundary_positional_metrics/positional_broken.negative.json",
        "data/parse/document_segmentation.yaml",
        "crates/da-cli/examples/document_segmentation_shadow.rs",
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


def test_boundary_positional_metrics_pass_on_clean_fixtures():
    """Committed fixtures, source markers and the pinned SHA satisfy the contract."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    assert service._check_boundary_positional_metrics() == []


def test_boundary_positional_metrics_missing_fixture_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    (service.root / "governor/fixtures/boundary_positional_metrics/positional_broken.negative.json").unlink()
    flags = service._check_boundary_positional_metrics()
    assert len(flags) == 1
    assert flags[0].flag == "boundary_positional_metrics_fixture_missing"
    assert "negative" in flags[0].evidence


def test_boundary_positional_metrics_positive_fixture_must_be_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    # Cross-pair the ends: the boundary count is unchanged and the declared
    # count metrics still agree, but no boundary is positionally correct.
    path = service.root / "governor/fixtures/boundary_positional_metrics/positional_clean.positive.json"
    records = json.loads(path.read_text())
    left, right = records[0]["predicted_boundaries"]
    left["end"], right["end"] = right["end"], left["end"]
    _write(path, json.dumps(records))
    flags = service._check_boundary_positional_metrics()
    broken = [f for f in flags if f.flag == "boundary_positional_metrics_broken"]
    assert len(broken) == 1
    assert "permuted_boundary_false_match" in broken[0].evidence


def test_boundary_positional_metrics_negative_fixture_must_trigger_labelled_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    # Declare a non-zero displacement on the record labelled
    # displacement_from_counts: nothing matched, so the label can no longer fire.
    path = service.root / "governor/fixtures/boundary_positional_metrics/positional_broken.negative.json"
    records = json.loads(path.read_text())
    for record in records:
        if record.get("violation") == "displacement_from_counts":
            record["record"]["reported"]["boundary_displacement"] = 0.5
            record["record"]["reported"]["boundary_displacement_max"] = 0.5
    _write(path, json.dumps(records))
    flags = service._check_boundary_positional_metrics()
    broken = [f for f in flags if f.flag == "boundary_positional_metrics_broken"]
    assert any("displacement_from_counts" in f.evidence for f in broken)


def test_boundary_positional_metrics_negative_fixture_must_cover_required_violations():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    # Remove the only record producing missing_coordinate_binding entirely —
    # required violation coverage must fail.
    path = service.root / "governor/fixtures/boundary_positional_metrics/positional_broken.negative.json"
    records = json.loads(path.read_text())
    records = [r for r in records if r.get("violation") != "missing_coordinate_binding"]
    _write(path, json.dumps(records))
    flags = service._check_boundary_positional_metrics()
    broken = [f for f in flags if f.flag == "boundary_positional_metrics_broken"]
    assert any("missing_coordinate_binding" in f.evidence for f in broken)


def test_boundary_positional_metrics_source_marker_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    # Drop the positional-metric enforcement points from the Rust runner.
    _write(service.root / "crates/da-cli/examples/document_segmentation_shadow.rs",
           "fn main() {}\n")
    flags = service._check_boundary_positional_metrics()
    marker = [f for f in flags
              if f.flag == "boundary_positional_metrics_source_marker_missing"]
    assert len(marker) == 1
    assert "runner" in marker[0].evidence


def test_boundary_positional_metrics_binding_mismatch_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    # A 64-zero scalar parses as int 0, which the binding guard must reject
    # as unverifiable rather than skip.
    path = service.root / "scripts/governor/checks/boundary_positional_metrics.yaml"
    _write(path, path.read_text().replace(
        "2bc5d8b9fe03e01eae94f7373d4777dec1318f7eacd59feb3c8237b48fd5e0ae",
        "0" * 64,
    ))
    flags = service._check_boundary_positional_metrics()
    mismatch = [f for f in flags
                if f.flag == "boundary_positional_metrics_binding_mismatch"]
    assert len(mismatch) == 1


def test_boundary_positional_metrics_binding_missing_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    (service.root / "data/parse/document_segmentation.yaml").unlink()
    flags = service._check_boundary_positional_metrics()
    missing = [f for f in flags
               if f.flag == "boundary_positional_metrics_binding_missing"]
    assert len(missing) == 1


def test_boundary_positional_metrics_invalid_yaml_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    _write(service.root / "scripts/governor/checks/boundary_positional_metrics.yaml",
           "{bad yaml: [")
    flags = service._check_boundary_positional_metrics()
    assert flags[0].flag == "boundary_positional_metrics_invalid"


def test_boundary_positional_metrics_oracle_rejects_permutation_without_governor():
    """The independent oracle is falsifiable on its own (no governor wiring)."""
    contract = {
        "coordinate_binding": {"tolerance": 0},
        "metrics": {
            "positional": [
                "boundary_precision",
                "boundary_recall",
                "boundary_f1",
                "boundary_displacement",
                "boundary_displacement_max",
            ],
        },
    }
    reviewed = [
        {"start": [2, 1, 3, 0], "end": [2, 1, 3, 7]},
        {"start": [2, 1, 20, 0], "end": [2, 1, 20, 7]},
    ]
    reported = {
        "count_agreement": 1,
        "count_difference": 0,
        "matched": 2,
        "false_positives": 0,
        "false_negatives": 0,
        "boundary_precision": 1.0,
        "boundary_recall": 1.0,
        "boundary_f1": 1.0,
        "boundary_displacement": 0.0,
        "boundary_displacement_max": 0.0,
    }
    truthful = {
        "predicted_boundaries": json.loads(json.dumps(reviewed)),
        "reviewed_boundaries": reviewed,
        "reported": reported,
    }
    assert _boundary_positional_violations(truthful, contract) == set()
    permuted = {
        "predicted_boundaries": [
            {"start": [2, 1, 3, 0], "end": [2, 1, 20, 7]},
            {"start": [2, 1, 20, 0], "end": [2, 1, 3, 7]},
        ],
        "reviewed_boundaries": reviewed,
        "reported": reported,
    }
    assert "permuted_boundary_false_match" in \
        _boundary_positional_violations(permuted, contract)


def test_boundary_positional_metrics_absent_snapshot_suite_is_not_flagged():
    """The rule has no durable snapshot task suite; the opt-out must be explicit."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_boundary_positional_contract(service.root)
    contract = yaml.safe_load(
        (service.root / "scripts/governor/checks/boundary_positional_metrics.yaml").read_text()
    )
    assert "graph_task_suite" not in contract
    assert _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        "boundary_positional_metrics", _boundary_positional_violations,
        graph_task_suite_required=False,
    ) == []
    default = _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        "boundary_positional_metrics", _boundary_positional_violations,
    )
    assert any(
        f.flag == "boundary_positional_metrics_suite_missing" for f in default
    )


# ---------------------------------------------------------------------------
# Donor visual adapter preservation contract (M508 S02 T05, GAR-003)
# ---------------------------------------------------------------------------
# Self-tests for the committed `visual_adapter_preserves_donor_content`
# contract (T04). The tests copy the REAL contract, fixtures, bound donor
# fixtures, converter source, lib gate, example delegation and gated Rust
# contract into a scratch root (never mutating tracked files), then validate
# that the check passes on the committed tree and fails on every tamper the
# contract claims to catch: a removed fixture, a pre-fix drop-path record,
# a neutralised negative label, a lost required label, a stripped source
# marker, a stale or missing or incomplete artifact, a broken or missing
# source binding, and invalid YAML. Two of them exercise the independent
# Python oracle and the explicit `graph_task_suite_required=False` opt-out
# directly, with no governor wiring, so the rule cannot go green by accident.

_VISUAL_ADAPTER_PREFIX = "visual_adapter_preserves_donor_content"
_VISUAL_ADAPTER_CONTRACT = (
    "scripts/governor/checks/visual_adapter_preserves_donor_content.yaml"
)
_VISUAL_ADAPTER_POSITIVE = (
    "governor/fixtures/visual_adapter_contract/artifact_preserves.positive.json"
)
_VISUAL_ADAPTER_NEGATIVE = (
    "governor/fixtures/visual_adapter_contract/artifact_broken.negative.json"
)


def _copy_visual_adapter_contract(root: Path) -> None:
    """Copy the donor-visual-adapter contract and every path it references.

    The path list is derived from the contract itself (fixtures, artifact,
    bound fixture sources, source markers) instead of being hard-coded, so a
    future contract edit that adds a binding or a marker is followed
    automatically. `_write` creates the missing parent directories — the
    scratch root has neither `crates/` nor `data/`.
    """
    repo = _scripts_dir.parent
    contract = yaml.safe_load(
        (repo / _VISUAL_ADAPTER_CONTRACT).read_text(encoding="utf-8")
    ) or {}
    rels = [_VISUAL_ADAPTER_CONTRACT]
    for fixture in (contract.get("fixtures") or {}).values():
        if isinstance(fixture, dict) and fixture.get("path"):
            rels.append(str(fixture["path"]))
    artifact = contract.get("artifact") or {}
    for key in ("path", "converter_source"):
        if artifact.get(key):
            rels.append(str(artifact[key]))
    for binding in (contract.get("bindings") or {}).get("fixture_sources") or []:
        if isinstance(binding, dict) and binding.get("path"):
            rels.append(str(binding["path"]))
    for entry in (contract.get("source_markers") or {}).values():
        if isinstance(entry, dict) and entry.get("path"):
            rels.append(str(entry["path"]))
    for rel in dict.fromkeys(rels):
        _write(root / rel, (repo / rel).read_text(encoding="utf-8"))


def _visual_adapter_service() -> TrajectoryService:
    """Scratch TrajectoryService with the committed contract copied in."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_visual_adapter_contract(service.root)
    return service


def test_visual_adapter_preserves_donor_content_pass_on_clean_fixtures():
    """Committed fixtures, artifact, bindings and markers satisfy the contract."""
    service = _visual_adapter_service()
    assert service._check_visual_adapter_preserves_donor_content() == []


def test_visual_adapter_preserves_donor_content_missing_fixture_is_flagged():
    service = _visual_adapter_service()
    (service.root / _VISUAL_ADAPTER_NEGATIVE).unlink()
    flags = service._check_visual_adapter_preserves_donor_content()
    assert len(flags) == 1
    assert flags[0].flag == f"{_VISUAL_ADAPTER_PREFIX}_fixture_missing"
    assert "negative" in flags[0].evidence
    assert flags[0].severity == "high"
    assert flags[0].dimension == "scholarly_parse_plane"


def test_visual_adapter_preserves_donor_content_positive_fixture_must_be_clean():
    """The pre-fix drop path (`primitives: Vec::new()`) must fail the rule."""
    service = _visual_adapter_service()
    path = service.root / _VISUAL_ADAPTER_POSITIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # Pre-fix shape: the donor produced a primitive without error while the
    # native result kept none — exactly what the committed fix removed.
    records[0]["donor"]["primitive_count"] = 1
    records[0]["donor"]["error"] = None
    records[0]["native"]["primitive_count"] = 0
    records[0]["native"]["primitives"] = []
    _write(path, json.dumps(records))
    flags = service._check_visual_adapter_preserves_donor_content()
    broken = [f for f in flags if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_broken"]
    assert len(broken) == 1
    assert "donor_primitives_discarded" in broken[0].evidence


def test_visual_adapter_preserves_donor_content_negative_fixture_must_trigger_labelled_violations():
    service = _visual_adapter_service()
    path = service.root / _VISUAL_ADAPTER_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # Neutralise the malformed label: with no lane absence the record no longer
    # reads as a mapped-to-not_observed failure.
    for record in records:
        if record.get("violation") == "malformed_mapped_to_not_observed":
            record["record"]["native"]["lane_absence"] = None
    _write(path, json.dumps(records))
    flags = service._check_visual_adapter_preserves_donor_content()
    broken = [f for f in flags if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_broken"]
    assert any("malformed_mapped_to_not_observed" in f.evidence for f in broken)


def test_visual_adapter_preserves_donor_content_negative_fixture_must_cover_required_violations():
    service = _visual_adapter_service()
    path = service.root / _VISUAL_ADAPTER_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # Remove the only record producing the label — required coverage must fail.
    records = [
        r for r in records
        if r.get("violation") != "malformed_mapped_to_not_observed"
    ]
    _write(path, json.dumps(records))
    flags = service._check_visual_adapter_preserves_donor_content()
    broken = [f for f in flags if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_broken"]
    assert any("malformed_mapped_to_not_observed" in f.evidence for f in broken)


def test_visual_adapter_preserves_donor_content_source_marker_missing_is_flagged():
    service = _visual_adapter_service()
    # Strip every enforcement marker from the converter surface.
    _write(service.root / "crates/da-cli/src/visual_adapter.rs", "fn main() {}\n")
    flags = service._check_visual_adapter_preserves_donor_content()
    marker = [
        f for f in flags
        if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_source_marker_missing"
    ]
    assert len(marker) == 1
    assert "adapter" in marker[0].evidence


def test_visual_adapter_preserves_donor_content_artifact_stale_is_flagged():
    """A converter or artifact edit without regeneration must never pass silently."""
    # (a) converter edited, artifact untouched.
    service = _visual_adapter_service()
    converter = service.root / "crates/da-cli/src/visual_adapter.rs"
    _write(converter, converter.read_text(encoding="utf-8") + "\n")
    flags = service._check_visual_adapter_preserves_donor_content()
    stale = [
        f for f in flags
        if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_artifact_stale"
    ]
    assert len(stale) == 1
    assert stale[0].severity == "high"
    assert stale[0].dimension == "scholarly_parse_plane"

    # (b) artifact declares the wrong converter digest.
    service = _visual_adapter_service()
    artifact_path = (
        service.root / "artifacts/parse_compat/visual_positive_fixture_run.json"
    )
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    artifact["converter_source_sha256"] = "0" * 64
    _write(artifact_path, json.dumps(artifact))
    flags = service._check_visual_adapter_preserves_donor_content()
    stale = [
        f for f in flags
        if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_artifact_stale"
    ]
    assert len(stale) == 1
    assert stale[0].severity == "high"
    assert stale[0].dimension == "scholarly_parse_plane"


def test_visual_adapter_preserves_donor_content_artifact_missing_is_flagged():
    service = _visual_adapter_service()
    (
        service.root / "artifacts/parse_compat/visual_positive_fixture_run.json"
    ).unlink()
    flags = service._check_visual_adapter_preserves_donor_content()
    assert any(
        f.flag == f"{_VISUAL_ADAPTER_PREFIX}_artifact_missing" for f in flags
    )


def test_visual_adapter_preserves_donor_content_artifact_incomplete_is_flagged():
    service = _visual_adapter_service()
    artifact_path = (
        service.root / "artifacts/parse_compat/visual_positive_fixture_run.json"
    )
    artifact = json.loads(artifact_path.read_text(encoding="utf-8"))
    artifact["records"] = [
        r for r in artifact["records"] if r.get("family") != "table"
    ]
    _write(artifact_path, json.dumps(artifact))
    flags = service._check_visual_adapter_preserves_donor_content()
    incomplete = [
        f for f in flags
        if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_artifact_incomplete"
    ]
    assert len(incomplete) == 1
    assert "table" in incomplete[0].evidence


def test_visual_adapter_preserves_donor_content_binding_mismatch_is_flagged():
    service = _visual_adapter_service()
    # A 64-zero digest parses as a hex string, so the guard must compare it.
    path = service.root / _VISUAL_ADAPTER_CONTRACT
    _write(path, path.read_text(encoding="utf-8").replace(
        "263e76df45068bbcd82e10e041736ef47305710b15a45aeda81e6cf8c139df11",
        "0" * 64,
    ))
    flags = service._check_visual_adapter_preserves_donor_content()
    mismatch = [
        f for f in flags
        if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_source_binding_mismatch"
    ]
    assert len(mismatch) == 1
    assert mismatch[0].severity == "high"
    assert mismatch[0].dimension == "scholarly_parse_plane"


def test_visual_adapter_preserves_donor_content_binding_missing_is_flagged():
    service = _visual_adapter_service()
    (
        service.root
        / "crates/da-cli/tests/fixtures/visual_positive_fixture_run/figure_geometry.xml"
    ).unlink()
    flags = service._check_visual_adapter_preserves_donor_content()
    missing = [
        f for f in flags
        if f.flag == f"{_VISUAL_ADAPTER_PREFIX}_source_binding_missing"
    ]
    assert len(missing) == 1


def test_visual_adapter_preserves_donor_content_invalid_yaml_is_flagged():
    service = _visual_adapter_service()
    _write(service.root / _VISUAL_ADAPTER_CONTRACT, "{bad yaml: [")
    flags = service._check_visual_adapter_preserves_donor_content()
    assert flags[0].flag == f"{_VISUAL_ADAPTER_PREFIX}_invalid"


def test_visual_adapter_preserves_donor_content_oracle_rejects_discarded_primitives_without_governor():
    """The independent oracle is falsifiable on its own (no governor wiring)."""
    service = _visual_adapter_service()
    contract = yaml.safe_load(
        (service.root / _VISUAL_ADAPTER_CONTRACT).read_text(encoding="utf-8")
    )
    records = json.loads(
        (service.root / _VISUAL_ADAPTER_POSITIVE).read_text(encoding="utf-8")
    )
    clean = records[0]
    assert _visual_adapter_violations(clean, contract) == set()

    discarded = json.loads(json.dumps(clean))
    discarded["native"]["primitive_count"] = 0
    discarded["native"]["primitives"] = []
    assert "donor_primitives_discarded" in \
        _visual_adapter_violations(discarded, contract)

    malformed = json.loads(json.dumps(clean))
    malformed["donor"]["error"] = "Malformed"
    malformed["native"]["lane_absence"] = "not_observed"
    assert "malformed_mapped_to_not_observed" in \
        _visual_adapter_violations(malformed, contract)


def test_visual_adapter_preserves_donor_content_absent_snapshot_suite_is_not_flagged():
    """The rule has no durable snapshot task suite; the opt-out must be explicit."""
    service = _visual_adapter_service()
    contract = yaml.safe_load(
        (service.root / _VISUAL_ADAPTER_CONTRACT).read_text(encoding="utf-8")
    )
    assert "graph_task_suite" not in contract
    assert _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        _VISUAL_ADAPTER_PREFIX, _visual_adapter_violations,
        graph_task_suite_required=False,
    ) == []
    default = _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        _VISUAL_ADAPTER_PREFIX, _visual_adapter_violations,
    )
    assert any(
        f.flag == f"{_VISUAL_ADAPTER_PREFIX}_suite_missing" for f in default
    )


# ---------------------------------------------------------------------------
# Component licence provenance contract (M508 S03 T04, GAR-004)
# ---------------------------------------------------------------------------
# Self-tests for the committed `component_license_provenance` contract (T03).
# The scratch root receives the REAL canonical ledger, the three read-only
# licence surfaces, the independent Rust contract, the contract DATA and the
# two fixtures — never a synthetic stand-in — so the fixture logic AND the
# strict oracle are exercised over the shipped artifact. Every mutation happens
# on that copy: no committed file is touched. Two tests bypass the Governor
# wiring on purpose (direct oracle call + the explicit
# `graph_task_suite_required=False` opt-out), so the rule cannot go green by
# accident, and one test falsifies the rule by mutating the REAL
# `data/parse/license_inventory.yaml` copy rather than a synthetic record.

_COMPONENT_LICENSE_PREFIX = "component_license_provenance"
_COMPONENT_LICENSE_CONTRACT = (
    "scripts/governor/checks/component_license_provenance.yaml"
)
_COMPONENT_LICENSE_INVENTORY = "data/parse/license_inventory.yaml"
_COMPONENT_LICENSE_POSITIVE = (
    "governor/fixtures/component_license_provenance/inventory_complete.positive.json"
)
_COMPONENT_LICENSE_NEGATIVE = (
    "governor/fixtures/component_license_provenance/inventory_broken.negative.json"
)


def _copy_component_license_contract(root: Path) -> None:
    """Copy the licence-provenance contract and every path it actually reads.

    Beyond the contract DATA and fixtures this copies the REAL canonical
    `data/parse/license_inventory.yaml`, the three read-only surfaces and the
    independent Rust contract. The oracle resolves `license_evidence` and the
    artifact obligations against the repository root, so every repo-relative
    reference of the ledger and of the positive fixture is materialised in the
    scratch root as well — otherwise a semantically identical copy would report
    `license_evidence_path_missing` / `unresolved_artifact_obligation` for
    reasons that have nothing to do with the rule. `_write` creates parents; a
    reference that is a directory (e.g. `data/gold_standard`) is created as a
    directory, which is all `Path.exists()` requires.
    """
    repo = _scripts_dir.parent
    rels = [
        _COMPONENT_LICENSE_CONTRACT,
        _COMPONENT_LICENSE_POSITIVE,
        _COMPONENT_LICENSE_NEGATIVE,
        _COMPONENT_LICENSE_INVENTORY,
        "data/parse/grobid_compatibility.yaml",
        "data/parse/feature_tagger_training_parity.yaml",
        "data/parse/grobid_model_rights.yaml",
        "crates/da-parse/tests/component_license_provenance_data_contract.rs",
        # T01 (M509-yzblqx/S06) added the `rust_capability_license_reader`
        # source marker that pins the capability-licence reader; the scratch
        # root must carry that file too, or a semantically identical copy
        # reports `source_marker_missing` for a file it was never given.
        "crates/da-parse/tests/data_contracts.rs",
    ]
    inventory = yaml.safe_load(
        (repo / _COMPONENT_LICENSE_INVENTORY).read_text(encoding="utf-8")
    )
    positive = json.loads(
        (repo / _COMPONENT_LICENSE_POSITIVE).read_text(encoding="utf-8")
    )
    references: list[str] = []
    for record in list(inventory.get("components") or []) + list(positive):
        if not isinstance(record, dict):
            continue
        for field in (
            "license_evidence",
            "attribution_notice_artifact",
            "source_offer_artifact",
        ):
            value = record.get(field)
            if (
                isinstance(value, str)
                and value
                and not value.startswith(("http://", "https://"))
                and not Path(value).is_absolute()
            ):
                references.append(value)
    for rel in dict.fromkeys(rels):
        _write(root / rel, (repo / rel).read_text(encoding="utf-8"))
    for rel in sorted(set(references)):
        source = repo / rel
        if source.is_dir():
            (root / rel).mkdir(parents=True, exist_ok=True)
        elif source.is_file():
            _write(root / rel, source.read_text(encoding="utf-8"))


def _component_license_service() -> TrajectoryService:
    """Scratch TrajectoryService with the committed licence contract copied in."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_component_license_contract(service.root)
    return service


def _license_flags_named(flags, name: str):
    return [f for f in flags if f.flag == f"{_COMPONENT_LICENSE_PREFIX}_{name}"]


def test_component_license_provenance_pass_on_clean_fixtures():
    """The committed ledger, surfaces, fixtures and markers satisfy the contract."""
    service = _component_license_service()
    assert service._check_component_license_provenance() == []


def test_component_license_provenance_missing_fixture_is_flagged():
    service = _component_license_service()
    (service.root / _COMPONENT_LICENSE_POSITIVE).unlink()
    flags = service._check_component_license_provenance()
    missing = _license_flags_named(flags, "fixture_missing")
    assert len(missing) == 1
    assert "positive" in missing[0].evidence
    assert missing[0].severity == "high"
    assert missing[0].dimension == "scholarly_parse_plane"

    service = _component_license_service()
    (service.root / _COMPONENT_LICENSE_NEGATIVE).unlink()
    flags = service._check_component_license_provenance()
    missing = _license_flags_named(flags, "fixture_missing")
    assert len(missing) == 1
    assert "negative" in missing[0].evidence


def test_component_license_provenance_positive_fixture_must_be_clean():
    service = _component_license_service()
    path = service.root / _COMPONENT_LICENSE_POSITIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    records[0]["license_status"] = "UNVERIFIED"
    _write(path, json.dumps(records))
    flags = service._check_component_license_provenance()
    broken = _license_flags_named(flags, "broken")
    assert len(broken) == 1
    assert "unverified_status_token" in broken[0].evidence


def test_component_license_provenance_negative_fixture_must_trigger_labelled_violations():
    service = _component_license_service()
    path = service.root / _COMPONENT_LICENSE_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # Turn the untyped token into a well-formed blocked position so the label
    # can no longer fire: the mismatch must be reported.
    for record in records:
        if record.get("violation") == "unverified_status_token":
            record["record"]["license_status"] = "blocked"
            record["record"]["blocked_reason"] = "fixture_reason"
    _write(path, json.dumps(records))
    flags = service._check_component_license_provenance()
    broken = _license_flags_named(flags, "broken")
    assert any("unverified_status_token" in f.evidence for f in broken)


def test_component_license_provenance_negative_fixture_must_cover_required_violations():
    service = _component_license_service()
    path = service.root / _COMPONENT_LICENSE_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    records = [
        r for r in records
        if r.get("violation") != "blocked_position_claims_redistribution"
    ]
    _write(path, json.dumps(records))
    flags = service._check_component_license_provenance()
    broken = _license_flags_named(flags, "broken")
    assert any(
        "missing required violation coverage" in f.evidence
        and "blocked_position_claims_redistribution" in f.evidence
        for f in broken
    )


def test_component_license_provenance_invalid_yaml_is_flagged():
    service = _component_license_service()
    _write(service.root / _COMPONENT_LICENSE_CONTRACT, "{bad yaml: [")
    flags = service._check_component_license_provenance()
    assert flags[0].flag == f"{_COMPONENT_LICENSE_PREFIX}_invalid"


def test_component_license_provenance_binding_mismatch_is_flagged():
    service = _component_license_service()
    path = service.root / _COMPONENT_LICENSE_CONTRACT
    text = path.read_text(encoding="utf-8")
    digest = (yaml.safe_load(text) or {})["bindings"]["source_data"][0]["sha256"]
    assert str(digest) in text
    # A 64-zero scalar parses as int 0, which the binding guard must reject as
    # unverifiable rather than skip.
    _write(path, text.replace(str(digest), "0" * 64))
    flags = service._check_component_license_provenance()
    mismatch = _license_flags_named(flags, "binding_mismatch")
    assert len(mismatch) == 1


def test_component_license_provenance_binding_missing_is_flagged():
    service = _component_license_service()
    (service.root / _COMPONENT_LICENSE_INVENTORY).unlink()
    flags = service._check_component_license_provenance()
    missing = _license_flags_named(flags, "binding_missing")
    assert len(missing) == 1


def test_component_license_provenance_inventory_missing_is_flagged():
    service = _component_license_service()
    # Drop the SHA binding first so the absence is reported as the missing
    # canonical artifact rather than as an unverifiable binding.
    path = service.root / _COMPONENT_LICENSE_CONTRACT
    contract = yaml.safe_load(path.read_text(encoding="utf-8"))
    contract.pop("bindings", None)
    _write(path, yaml.safe_dump(contract, sort_keys=False))
    (service.root / _COMPONENT_LICENSE_INVENTORY).unlink()
    flags = service._check_component_license_provenance()
    missing = _license_flags_named(flags, "inventory_missing")
    assert len(missing) == 1


def test_component_license_provenance_surface_missing_is_flagged():
    service = _component_license_service()
    (service.root / "data/parse/grobid_model_rights.yaml").unlink()
    flags = service._check_component_license_provenance()
    missing = _license_flags_named(flags, "surface_missing")
    assert len(missing) == 1
    assert "grobid_model_rights" in missing[0].evidence


def test_component_license_provenance_surface_invalid_is_flagged():
    service = _component_license_service()
    # Break the pointer target: the document stays valid YAML but the declared
    # asset list disappears.
    _write(service.root / "data/parse/grobid_model_rights.yaml", "assets: []\n")
    flags = service._check_component_license_provenance()
    invalid = _license_flags_named(flags, "surface_invalid")
    assert any("grobid_model_rights" in f.evidence for f in invalid)


def test_component_license_provenance_source_marker_missing_is_flagged():
    service = _component_license_service()
    _write(
        service.root
        / "crates/da-parse/tests/component_license_provenance_data_contract.rs",
        "fn main() {}\n",
    )
    flags = service._check_component_license_provenance()
    marker = _license_flags_named(flags, "source_marker_missing")
    assert len(marker) == 1
    assert "rust_contract" in marker[0].evidence


def test_component_license_provenance_fixture_inventory_drift_is_flagged():
    service = _component_license_service()
    path = service.root / _COMPONENT_LICENSE_POSITIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    removed = records.pop()["component_id"]
    _write(path, json.dumps(records))
    flags = service._check_component_license_provenance()
    drift = _license_flags_named(flags, "fixture_inventory_drift")
    assert len(drift) == 1
    assert removed in drift[0].evidence


def test_component_license_provenance_absent_snapshot_suite_is_not_flagged():
    """The rule has no durable snapshot task suite; the opt-out must be explicit."""
    service = _component_license_service()
    contract = yaml.safe_load(
        (service.root / _COMPONENT_LICENSE_CONTRACT).read_text(encoding="utf-8")
    )
    assert "graph_task_suite" not in contract
    # The opt-out governs only the missing-suite case. The fixture validation
    # inside the shared helper needs the rule's private `_root` / `_surface_ids`
    # oracle keys, so this test asserts the suite flag itself rather than an
    # empty list (which would also assert unrelated fixture diagnostics).
    opted_out = _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        _COMPONENT_LICENSE_PREFIX, _license_provenance_violations,
        graph_task_suite_required=False,
    )
    assert not [
        f for f in opted_out
        if f.flag == f"{_COMPONENT_LICENSE_PREFIX}_suite_missing"
    ], opted_out
    default = _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        _COMPONENT_LICENSE_PREFIX, _license_provenance_violations,
    )
    assert any(
        f.flag == f"{_COMPONENT_LICENSE_PREFIX}_suite_missing" for f in default
    )


def test_component_license_provenance_real_inventory_mutations_are_flagged():
    """Falsification on the REAL ledger, not only on the synthetic fixtures.

    The GAR-004 acceptance criterion is about `data/parse/license_inventory.yaml`
    itself, so an unmodified copy is the control (zero flags) and each mutation
    must produce its exact flag. Mutations are applied to the copy only.
    """

    def _flags_for(mutate) -> list:
        service = _component_license_service()
        path = service.root / _COMPONENT_LICENSE_INVENTORY
        document = yaml.safe_load(path.read_text(encoding="utf-8"))
        mutate(document)
        _write(path, yaml.safe_dump(document, sort_keys=False))
        return service._check_component_license_provenance()

    def _verified(document) -> dict:
        return next(
            c for c in document["components"] if c["license_status"] == "verified"
        )

    def _blocked(document) -> dict:
        return next(
            c for c in document["components"] if c["license_status"] == "blocked"
        )

    # Control: the unmodified copy of the real ledger is clean.
    assert _component_license_service()._check_component_license_provenance() == []

    def _untyped_status(document):
        _verified(document)["license_status"] = "UNVERIFIED"

    assert _license_flags_named(
        _flags_for(_untyped_status), "unverified_status_token"
    )

    def _drop_blocked_reason(document):
        _blocked(document).pop("blocked_reason", None)

    assert _license_flags_named(
        _flags_for(_drop_blocked_reason), "missing_blocked_reason"
    )

    def _blocked_claims_redistribution(document):
        _blocked(document)["redistribution"] = "allowed_with_notice"

    assert _license_flags_named(
        _flags_for(_blocked_claims_redistribution),
        "blocked_position_claims_redistribution",
    )

    def _unresolvable_evidence(document):
        _verified(document)["license_evidence"] = "/nonexistent/LICENSE"

    assert _license_flags_named(
        _flags_for(_unresolvable_evidence), "license_evidence_path_missing"
    )

    def _drop_surface_claim(document):
        holder = next(
            c for c in document["components"] if c.get("surface_claims")
        )
        holder["surface_claims"].pop()

    assert _license_flags_named(
        _flags_for(_drop_surface_claim), "cross_surface_component_unclaimed"
    )

    def _duplicate_surface_claim(document):
        holder = next(
            c for c in document["components"] if c.get("surface_claims")
        )
        claim = holder["surface_claims"][0]
        other = next(c for c in document["components"] if c is not holder)
        other.setdefault("surface_claims", []).append(dict(claim))

    assert _license_flags_named(
        _flags_for(_duplicate_surface_claim), "cross_surface_claim_duplicated"
    )

    def _relax_import_lock(document):
        _verified(document)["import_eligible"] = True

    assert _license_flags_named(
        _flags_for(_relax_import_lock), "redistribution_lock_relaxed"
    )


def test_component_license_provenance_oracle_rejects_untyped_status_without_governor():
    """The discrimination lives in the independent oracle, not in the wiring."""
    service = _component_license_service()
    contract = {
        "license_status_vocabulary": ["verified", "blocked"],
        "boundary_vocabulary": ["http"],
        "redistribution_status_vocabulary": ["blocked", "not_distributed"],
        "artifact_obligation_vocabulary": ["not_applicable"],
        "blocked_status_vocabulary": ["blocked"],
        "_root": str(service.root),
        "_surface_ids": {},
    }
    clean = {
        "component_id": "oracle-fixture",
        "license_status": "verified",
        "license_evidence": "http://example.invalid/LICENSE",
        "boundary": "http",
        "redistribution": "not_distributed",
        "attribution_notice_artifact": "not_applicable",
        "source_offer_artifact": "not_applicable",
        "surface_claims": [],
    }
    assert _license_provenance_violations(clean, contract) == set()

    untyped = json.loads(json.dumps(clean))
    untyped["license_status"] = "UNVERIFIED"
    assert "unverified_status_token" in \
        _license_provenance_violations(untyped, contract)

    no_reason = json.loads(json.dumps(clean))
    no_reason["license_status"] = "blocked"
    no_reason["redistribution"] = "blocked"
    assert "missing_blocked_reason" in \
        _license_provenance_violations(no_reason, contract)

    # Fail-closed: an empty closed vocabulary means "cannot decide", not "accept".
    no_vocabulary = dict(contract)
    no_vocabulary["license_status_vocabulary"] = []
    assert "missing_closed_vocabulary" in \
        _license_provenance_violations(clean, no_vocabulary)


# Self-tests for the committed `oracle_digest_recomputation` contract (T03/T04).
# The scratch root receives the REAL contract DATA, the three audited
# surfaces, the two fixtures and the independent Rust contract — never a
# synthetic stand-in — so the fixture logic AND the strict oracle are exercised
# over the shipped artifact. Every mutation happens on that copy: no committed
# file is touched. The scratch copy derives every path it materialises from the
# contract itself, so a future contract edit cannot silently turn the control
# into a measurement of copy completeness (MEM1504). The oracle labels are also
# exercised with a plain dict and no service at all, and one test mutes the rule
# body to prove the mutations are the discriminating signal.

_ORACLE_DIGEST_PREFIX = "oracle_digest_recomputation"
_ORACLE_DIGEST_CONTRACT = "scripts/governor/checks/oracle_digest_recomputation.yaml"
_ORACLE_DIGEST_FIXTURE = "crates/da-parse/tests/fixtures/segmentation_oracle.yaml"
_ORACLE_DIGEST_ARTIFACT = (
    "artifacts/parse_compat/document_segmentation_oracle_source.json"
)
_ORACLE_DIGEST_MANIFEST = "data/parse/document_segmentation.yaml"
_ORACLE_DIGEST_RUST = "crates/da-parse/tests/oracle_digest_recomputation.rs"
_ORACLE_DIGEST_POSITIVE = (
    "governor/fixtures/oracle_digest_recomputation/digest_fields_complete.positive.json"
)
_ORACLE_DIGEST_NEGATIVE = (
    "governor/fixtures/oracle_digest_recomputation/digest_fields_broken.negative.json"
)


def _copy_oracle_digest_contract(root: Path) -> None:
    """Copy the contract and every path it reads into the scratch root.

    The read paths are derived from the contract DATA (`audited_surfaces`,
    `bindings.source_data`, both fixtures, `source_markers`, `graph_task_suite`)
    instead of being hardcoded, so adding a surface to the contract without
    copying it fails the test rather than the copy (MEM1504).
    """
    repo = _scripts_dir.parent
    contract = yaml.safe_load(
        (repo / _ORACLE_DIGEST_CONTRACT).read_text(encoding="utf-8")
    )
    rels = [_ORACLE_DIGEST_CONTRACT]
    for surface in contract.get("audited_surfaces") or []:
        if isinstance(surface, dict) and surface.get("path"):
            rels.append(str(surface["path"]))
    for binding in (contract.get("bindings") or {}).get("source_data") or []:
        if isinstance(binding, dict) and binding.get("path"):
            rels.append(str(binding["path"]))
    for key in ("positive", "negative"):
        entry = (contract.get("fixtures") or {}).get(key) or {}
        if isinstance(entry, dict) and entry.get("path"):
            rels.append(str(entry["path"]))
    for entry in (contract.get("source_markers") or {}).values():
        if isinstance(entry, dict) and entry.get("path"):
            rels.append(str(entry["path"]))
    suite = contract.get("graph_task_suite") or {}
    if isinstance(suite, dict) and suite.get("path"):
        rels.append(str(suite["path"]))
    for rel in dict.fromkeys(rels):
        _write(root / rel, (repo / rel).read_text(encoding="utf-8"))


def _oracle_digest_service() -> TrajectoryService:
    """Scratch TrajectoryService with the committed digest contract copied in."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_oracle_digest_contract(service.root)
    return service


def _oracle_digest_flags_named(flags, name: str):
    return [f for f in flags if f.flag == f"{_ORACLE_DIGEST_PREFIX}_{name}"]


def _oracle_digest_contract_document(service: TrajectoryService) -> dict:
    path = service.root / _ORACLE_DIGEST_CONTRACT
    return yaml.safe_load(path.read_text(encoding="utf-8"))


def _write_oracle_digest_contract(service: TrajectoryService, contract: dict) -> None:
    _write(
        service.root / _ORACLE_DIGEST_CONTRACT,
        yaml.safe_dump(contract, sort_keys=False),
    )


def _oracle_digest_surface_document(service: TrajectoryService, rel: str):
    text = (service.root / rel).read_text(encoding="utf-8")
    return json.loads(text) if rel.endswith(".json") else yaml.safe_load(text)


def _write_oracle_digest_surface(service: TrajectoryService, rel: str, document) -> None:
    if rel.endswith(".json"):
        _write(service.root / rel, json.dumps(document, indent=2))
    else:
        _write(service.root / rel, yaml.safe_dump(document, sort_keys=False))


def _with_oracle_digest_control(test):
    """Run a mutation test only after the unmodified copy measured clean.

    Hazard (d) of the task plan: without an in-test control a mutation test
    proves "something turned red", not "this mutation turned it red". Every
    decorated test therefore builds a pristine scratch copy first and asserts
    zero flags before applying its mutation.
    """

    def _runner():
        assert _oracle_digest_service()._check_oracle_digest_recomputation() == []
        test()

    _runner.__name__ = test.__name__
    _runner.__doc__ = test.__doc__
    return _runner


def test_oracle_digest_recomputation_pass_on_clean_fixtures():
    """The committed contract, surfaces, fixtures and markers satisfy the rule."""
    service = _oracle_digest_service()
    assert service._check_oracle_digest_recomputation() == []
    # The shipped repository tree must be clean too: the mutation-only tests
    # below are only meaningful if the real tree starts clean.
    real = TrajectoryService.__new__(TrajectoryService)
    real.root = _scripts_dir.parent
    assert real._check_oracle_digest_recomputation() == []


def test_oracle_digest_recomputation_missing_contract_is_silent():
    """No DATA contract means no rule: absence is never a violation."""
    service = _oracle_digest_service()
    (service.root / _ORACLE_DIGEST_CONTRACT).unlink()
    assert service._check_oracle_digest_recomputation() == []


@_with_oracle_digest_control
def test_oracle_digest_recomputation_missing_fixture_is_flagged():
    service = _oracle_digest_service()
    (service.root / _ORACLE_DIGEST_POSITIVE).unlink()
    missing = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "fixture_missing"
    )
    assert len(missing) == 1
    assert "positive" in missing[0].evidence
    assert missing[0].severity == "high"
    assert missing[0].dimension == "scholarly_parse_plane"

    service = _oracle_digest_service()
    (service.root / _ORACLE_DIGEST_NEGATIVE).unlink()
    missing = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "fixture_missing"
    )
    assert len(missing) == 1
    assert "negative" in missing[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_positive_fixture_must_be_clean():
    service = _oracle_digest_service()
    path = service.root / _ORACLE_DIGEST_POSITIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    records[0]["class"] = "derived"
    _write(path, json.dumps(records))
    broken = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "broken"
    )
    assert len(broken) == 1
    assert "class_outside_vocabulary" in broken[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_negative_fixture_must_trigger_labelled_violations():
    service = _oracle_digest_service()
    path = service.root / _ORACLE_DIGEST_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # A legal alignment can no longer fire its label: the mismatch must be
    # reported instead of silently passing.
    for record in records:
        if record.get("violation") == "alignment_outside_vocabulary":
            record["record"]["alignment"] = "index"
    _write(path, json.dumps(records))
    broken = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "broken"
    )
    assert any("alignment_outside_vocabulary" in f.evidence for f in broken)


@_with_oracle_digest_control
def test_oracle_digest_recomputation_negative_fixture_must_cover_required_violations():
    service = _oracle_digest_service()
    path = service.root / _ORACLE_DIGEST_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    records = [
        r for r in records
        if r.get("violation") != "canonicalization_outside_contract"
    ]
    _write(path, json.dumps(records))
    broken = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "broken"
    )
    assert any(
        "missing required violation coverage" in f.evidence
        and "canonicalization_outside_contract" in f.evidence
        for f in broken
    )


@_with_oracle_digest_control
def test_oracle_digest_recomputation_invalid_yaml_is_flagged():
    service = _oracle_digest_service()
    _write(service.root / _ORACLE_DIGEST_CONTRACT, "{bad yaml: [")
    flags = service._check_oracle_digest_recomputation()
    assert flags[0].flag == f"{_ORACLE_DIGEST_PREFIX}_invalid"


@_with_oracle_digest_control
def test_oracle_digest_recomputation_binding_mismatch_is_flagged():
    service = _oracle_digest_service()
    path = service.root / _ORACLE_DIGEST_CONTRACT
    text = path.read_text(encoding="utf-8")
    digest = (yaml.safe_load(text) or {})["bindings"]["source_data"][0]["sha256"]
    assert str(digest) in text
    # A 64-zero scalar parses as int 0, which the binding guard must reject as
    # unverifiable rather than skip.
    _write(path, text.replace(str(digest), "0" * 64))
    mismatch = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "binding_mismatch"
    )
    assert len(mismatch) == 1
    assert _ORACLE_DIGEST_ARTIFACT in mismatch[0].evidence

    service = _oracle_digest_service()
    path = service.root / _ORACLE_DIGEST_CONTRACT
    text = path.read_text(encoding="utf-8")
    _write(path, text.replace(str(digest), "a" * 64))
    mismatch = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "binding_mismatch"
    )
    assert len(mismatch) == 1
    assert "observed" in mismatch[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_binding_missing_is_flagged():
    service = _oracle_digest_service()
    (service.root / _ORACLE_DIGEST_ARTIFACT).unlink()
    missing = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "binding_missing"
    )
    assert len(missing) == 1
    assert _ORACLE_DIGEST_ARTIFACT in missing[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_surface_missing_is_flagged():
    service = _oracle_digest_service()
    (service.root / _ORACLE_DIGEST_FIXTURE).unlink()
    missing = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "surface_missing"
    )
    assert len(missing) == 1
    assert "fixture" in missing[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_surface_invalid_is_flagged():
    service = _oracle_digest_service()
    # Valid YAML whose declared audit pointer cannot resolve.
    _write(service.root / _ORACLE_DIGEST_MANIFEST, "unrelated: 1\n")
    invalid = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "surface_invalid"
    )
    assert any(
        "manifest" in f.evidence and "audit pointer" in f.evidence
        for f in invalid
    )

    service = _oracle_digest_service()
    _write(service.root / _ORACLE_DIGEST_FIXTURE, "{bad yaml: [")
    invalid = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "surface_invalid"
    )
    assert any("fixture" in f.evidence and "unreadable" in f.evidence for f in invalid)


@_with_oracle_digest_control
def test_oracle_digest_recomputation_unclassified_digest_field_is_flagged():
    """A digest-like field inside the audit scope that nobody classified fails."""
    service = _oracle_digest_service()
    document = _oracle_digest_surface_document(service, _ORACLE_DIGEST_ARTIFACT)
    document["receipts"][0]["unclassified_extra_digest"] = "a" * 64
    _write_oracle_digest_surface(service, _ORACLE_DIGEST_ARTIFACT, document)
    unclassified = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "unclassified_digest_field"
    )
    assert len(unclassified) == 1
    assert "source" in unclassified[0].evidence
    assert "unclassified_extra_digest" in unclassified[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_declared_field_not_found_is_flagged():
    """A declaration whose pointer resolves nowhere is a dangling claim."""
    service = _oracle_digest_service()
    contract = _oracle_digest_contract_document(service)
    contract["digest_fields"].append({
        "field_id": "source.receipt.missing_digest",
        "surface": "source",
        "pointer": "/receipts/*/missing_digest",
        "class": "recomputed",
        "ref": "self:/receipts/*/canonical_annotation/bytes",
        "alignment": "index",
        "canonicalization": "text_utf8_lf_no_trailing_newline_sha256",
    })
    _write_oracle_digest_contract(service, contract)
    not_found = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "declared_field_not_found"
    )
    assert len(not_found) == 1
    assert "source.receipt.missing_digest" in not_found[0].evidence
    assert "/receipts/*/missing_digest" in not_found[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_adjudicated_field_reintroduced_is_flagged():
    """The removed dangling pin must stay removed; control copy stays clean."""
    control = _oracle_digest_service()
    assert not _oracle_digest_flags_named(
        control._check_oracle_digest_recomputation(),
        "adjudicated_field_reintroduced",
    )

    service = _oracle_digest_service()
    document = _oracle_digest_surface_document(service, _ORACLE_DIGEST_ARTIFACT)
    document["manifest_sha256"] = "b" * 64
    _write_oracle_digest_surface(service, _ORACLE_DIGEST_ARTIFACT, document)
    flags = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(),
        "adjudicated_field_reintroduced",
    )
    assert len(flags) == 1
    assert "source.manifest_sha256" in flags[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_opaque_used_as_canonical_source_is_flagged():
    """A recomputed record cannot launder an opaque marker into provenance."""
    service = _oracle_digest_service()
    contract = _oracle_digest_contract_document(service)
    record = next(
        item for item in contract["digest_fields"]
        if item.get("field_id") == "source.receipt.canonical_page_range.sha256"
    )
    record["ref"] = "self:/receipts/*/source_sha256"
    _write_oracle_digest_contract(service, contract)
    flags = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(),
        "opaque_used_as_canonical_source",
    )
    assert len(flags) == 1
    assert "source.receipt.canonical_page_range.sha256" in flags[0].evidence
    assert "source.receipt.source_sha256" in flags[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_bytes_pair_halves_are_flagged():
    """Published canonical bytes and their digest must be both-or-neither."""
    service = _oracle_digest_service()
    document = _oracle_digest_surface_document(service, _ORACLE_DIGEST_ARTIFACT)
    del document["receipts"][0]["canonical_page_range"]["sha256"]
    _write_oracle_digest_surface(service, _ORACLE_DIGEST_ARTIFACT, document)
    flags = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "bytes_without_sha256_pair"
    )
    # One flag per declared record that resolves the unpaired bytes object
    # (the fixture digest, the artifact digest and the sibling sha256 record),
    # so the discriminating assertion is the named field, not the count.
    assert flags
    assert "source.receipt.canonical_page_range.sha256" in " ".join(
        f.evidence for f in flags
    )

    service = _oracle_digest_service()
    document = _oracle_digest_surface_document(service, _ORACLE_DIGEST_ARTIFACT)
    del document["receipts"][0]["canonical_furniture"][0]["bytes"]
    _write_oracle_digest_surface(service, _ORACLE_DIGEST_ARTIFACT, document)
    flags = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "sha256_without_bytes_pair"
    )
    assert flags
    assert "source.receipt.canonical_furniture.sha256" in " ".join(
        f.evidence for f in flags
    )


@_with_oracle_digest_control
def test_oracle_digest_recomputation_duplicate_field_binding_is_flagged():
    service = _oracle_digest_service()
    contract = _oracle_digest_contract_document(service)
    duplicate = next(
        item for item in contract["digest_fields"]
        if item.get("field_id") == "source.receipt.annotation_digest"
    )
    contract["digest_fields"].append(json.loads(json.dumps(duplicate)))
    _write_oracle_digest_contract(service, contract)
    duplicated = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "duplicate_field_binding"
    )
    assert len(duplicated) == 1
    assert "source:/receipts/*/annotation_digest" in duplicated[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_fixture_field_drift_is_flagged():
    service = _oracle_digest_service()
    path = service.root / _ORACLE_DIGEST_POSITIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    removed = records.pop()["field_id"]
    _write(path, json.dumps(records))
    drift = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "fixture_field_drift"
    )
    assert len(drift) == 1
    assert removed in drift[0].evidence


@_with_oracle_digest_control
def test_oracle_digest_recomputation_source_marker_missing_is_flagged():
    service = _oracle_digest_service()
    _write(service.root / _ORACLE_DIGEST_RUST, "fn main() {}\n")
    marker = _oracle_digest_flags_named(
        service._check_oracle_digest_recomputation(), "source_marker_missing"
    )
    assert len(marker) == 1
    assert "rust_contract" in marker[0].evidence


def test_oracle_digest_recomputation_oracle_labels_are_reachable_without_governor():
    """The independent oracle classifies form alone, from a plain dict."""
    contract = yaml.safe_load(
        (_scripts_dir.parent / _ORACLE_DIGEST_CONTRACT).read_text(encoding="utf-8")
    )
    base = {
        "field_id": "minimal.record",
        "surface": "fixture",
        "pointer": "/receipts/*/annotation_digest",
        "class": "recomputed",
        "ref": "source:/receipts/*/canonical_annotation/bytes",
        "alignment": "index",
        "canonicalization": "text_utf8_lf_no_trailing_newline_sha256",
    }
    assert _oracle_digest_violations(base, contract) == set()

    def _without(*keys):
        record = dict(base)
        for key in keys:
            record.pop(key, None)
        return record

    cases = {
        "missing_field_identity": _without("field_id"),
        "digest_without_class": _without("class"),
        "class_outside_vocabulary": {**base, "class": "derived"},
        "recomputed_without_canonical_ref": _without("ref"),
        "recomputed_without_canonicalization": _without("canonicalization"),
        "canonicalization_outside_contract": {
            **base, "canonicalization": "normalized_text_sha256",
        },
        "opaque_without_reason": {
            "field_id": base["field_id"],
            "surface": base["surface"],
            "pointer": base["pointer"],
            "class": "opaque",
        },
        "opaque_reason_outside_vocabulary": {
            **base, "class": "opaque", "opaque_reason": "because_reasons",
        },
        "opaque_marker_as_provenance": {
            **base,
            "class": "opaque",
            "opaque_reason": "external_donor_bytes_absent",
        },
        "alignment_outside_vocabulary": {**base, "alignment": "ordinal"},
    }
    for label, record in cases.items():
        assert label in _oracle_digest_violations(record, contract), label
    # The ten per-record labels are exactly the negative fixture's coverage:
    # dropping a case would silently shrink the closed vocabulary.
    require = contract["fixtures"]["negative"]["expect"]["require_violations"]
    assert set(cases) == set(require)

    # Fail-closed: an empty closed vocabulary means "cannot decide".
    no_vocabulary = dict(contract)
    no_vocabulary["classes"] = []
    assert "missing_closed_vocabulary" in _oracle_digest_violations(
        base, no_vocabulary
    )


def test_oracle_digest_recomputation_rule_body_muting_reddens_mutations():
    """Muting the rule body must redden the mutations, not the clean control."""
    mutation_tests = (
        test_oracle_digest_recomputation_missing_fixture_is_flagged,
        test_oracle_digest_recomputation_positive_fixture_must_be_clean,
        test_oracle_digest_recomputation_negative_fixture_must_trigger_labelled_violations,
        test_oracle_digest_recomputation_negative_fixture_must_cover_required_violations,
        test_oracle_digest_recomputation_invalid_yaml_is_flagged,
        test_oracle_digest_recomputation_binding_mismatch_is_flagged,
        test_oracle_digest_recomputation_binding_missing_is_flagged,
        test_oracle_digest_recomputation_surface_missing_is_flagged,
        test_oracle_digest_recomputation_surface_invalid_is_flagged,
        test_oracle_digest_recomputation_unclassified_digest_field_is_flagged,
        test_oracle_digest_recomputation_declared_field_not_found_is_flagged,
        test_oracle_digest_recomputation_adjudicated_field_reintroduced_is_flagged,
        test_oracle_digest_recomputation_opaque_used_as_canonical_source_is_flagged,
        test_oracle_digest_recomputation_bytes_pair_halves_are_flagged,
        test_oracle_digest_recomputation_duplicate_field_binding_is_flagged,
        test_oracle_digest_recomputation_fixture_field_drift_is_flagged,
        test_oracle_digest_recomputation_source_marker_missing_is_flagged,
    )

    def _failures(tests) -> list:
        failed = []
        for test in tests:
            try:
                test()
            except Exception as error:  # noqa: BLE001 - report, never raise
                failed.append(f"{test.__name__}: {error}")
        return failed

    assert _failures(mutation_tests) == []

    original = TrajectoryService._check_oracle_digest_recomputation

    def _muted(self):
        return []

    TrajectoryService._check_oracle_digest_recomputation = _muted
    try:
        reddened = _failures(mutation_tests)
        assert len(reddened) >= len(mutation_tests) - 1, reddened
        # The clean control and the absence convention cannot detect the muted
        # body: that is exactly why the mutations above carry the signal.
        assert _failures(
            (test_oracle_digest_recomputation_pass_on_clean_fixtures,)
        ) == []
        assert _failures(
            (test_oracle_digest_recomputation_missing_contract_is_silent,)
        ) == []
    finally:
        TrajectoryService._check_oracle_digest_recomputation = original


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


def _prepare_edge_contract_fixture(root: Path) -> None:
    _copy_data_integrity_contract(root)
    _write(root / "data/ontology/edge_contracts.yaml", "schema_version: edge-contracts.v1\n")


def test_data_integrity_canonical_edge_contract_path_is_clean():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _prepare_edge_contract_fixture(service.root)
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert not any(f.flag.startswith("data_integrity_edge_contract_") for f in flags)


def test_data_integrity_canonical_edge_contract_orphan_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _prepare_edge_contract_fixture(service.root)
    legacy_path = "data/" + "edge_contracts.yaml"
    _write(service.root / legacy_path, "legacy: true\n")
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert any(f.flag == "data_integrity_edge_contract_orphan_present" for f in flags)


def test_data_integrity_canonical_edge_contract_legacy_reader_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _prepare_edge_contract_fixture(service.root)
    legacy_path = "data/" + "edge_contracts.yaml"
    _write(service.root / "crates/da-test/src/reader.rs",
           f'const PATH: &str = "{legacy_path}";\n')
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert any(f.flag == "data_integrity_edge_contract_legacy_reader" for f in flags)


def test_data_integrity_canonical_edge_contract_historical_text_is_allowed():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _prepare_edge_contract_fixture(service.root)
    _write(service.root / "scripts/migrate_data_layout_m447.sh",
           "# ADR-067 migration may mention data/" + "edge_contracts.yaml\n")
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert not any(f.flag == "data_integrity_edge_contract_legacy_reader" for f in flags)


def test_data_integrity_arxiv_doi_literal_in_production_rust_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_data_integrity_contract(service.root)
    _write(service.root / "crates/da-test/src/openalex.rs", 'const DOI: &str = "10.48550/arxiv.";\\n')
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert any(f.flag == "data_integrity_hardcoded_arxiv_doi_prefix" for f in flags)


def test_data_integrity_arxiv_doi_literal_in_test_rust_is_ignored():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_data_integrity_contract(service.root)
    _write(service.root / "crates/da-test/tests/openalex.rs", 'const DOI: &str = "10.48550/arxiv.";\\n')
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert not any(f.flag == "data_integrity_hardcoded_arxiv_doi_prefix" for f in flags)


def test_data_integrity_canonical_edge_contract_rule_malformed_is_flagged():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_data_integrity_contract(service.root)
    contract_path = service.root / "scripts/governor/checks/data_integrity.yaml"
    contract = yaml.safe_load(contract_path.read_text())
    edge_rule = next(rule for rule in contract["rules"] if rule["id"] == "canonical_edge_contract_path")
    edge_rule["scan"] = []
    _write(contract_path, yaml.safe_dump(contract))
    from governor.adapters.data_integrity_checker import check_data_integrity
    flags = check_data_integrity(service.root)
    assert any(f.flag == "data_integrity_edge_contract_rule_invalid" for f in flags)


def _copy_graph_semantics_contract(root: Path) -> None:
    repo = _scripts_dir.parent
    rel = "scripts/governor/checks/graph_semantics.yaml"
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    import shutil
    shutil.copy2(repo / rel, root / rel)


def _write_sgsnap(path: Path, records: list[dict]) -> None:
    """Write a minimal sgsnap v2 NDJSON snapshot (header + records)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(path, "wt", encoding="utf-8") as fh:
        fh.write(json.dumps({
            "format": "sgsnap", "version": 2, "tenant": "default",
            "node_count": sum(1 for r in records if r.get("t") == "n"),
            "edge_count": sum(1 for r in records if r.get("t") == "e"),
            "labels": sorted({l for r in records if r.get("t") == "n"
                              for l in r["labels"]}),
            "edge_types": sorted({r["type"] for r in records
                                  if r.get("t") == "e"}),
            "created_at": path.stem,
        }) + "\n")
        for rec in records:
            fh.write(json.dumps(rec) + "\n")


def test_graph_semantics_counts_nodes_structurally():
    """paper_count must be distinct vid count, never string//2 heuristics."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_graph_semantics_contract(service.root)
    (service.root / "data" / "graphs").mkdir(parents=True)
    snap = service.root / "data" / "graphs" / "s.sgsnap"
    records = [
        {"t": "n", "id": 1, "labels": ["Paper"],
         "props": {"vid": "a" * 8, "raw_text": "x" * 10}},
        {"t": "n", "id": 2, "labels": ["Paper"],
         "props": {"vid": "b" * 8}},
        {"t": "n", "id": 3, "labels": ["Section"],
         "props": {"vid": "c" * 8, "raw_text": "y" * 30}},
    ]
    _write_sgsnap(snap, records)
    from governor.adapters.graph_query_reader import GraphQueryReader
    reader = GraphQueryReader(service.root)
    m = reader.compute_metrics(snap)
    assert m["paper_count"] == 2
    assert m["section_count"] == 1
    assert m["avg_raw_text_len"] == 30  # Section-only, not mixed with Paper


def test_graph_semantics_revision_binding_via_edge_type():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_graph_semantics_contract(service.root)
    (service.root / "data" / "graphs").mkdir(parents=True)
    snap = service.root / "data" / "graphs" / "s.sgsnap"
    records = [
        {"t": "n", "id": 1, "labels": ["Paper"], "props": {"vid": "p1"}},
        {"t": "n", "id": 2, "labels": ["Paper"], "props": {"vid": "p2"}},
        {"t": "n", "id": 3, "labels": ["PaperRevision"], "props": {"vid": "r1"}},
        {"t": "e", "src": 1, "tgt": 3, "type": "HAS_REVISION", "props": {}},
    ]
    _write_sgsnap(snap, records)
    from governor.adapters.graph_query_reader import GraphQueryReader
    m = GraphQueryReader(service.root).compute_metrics(snap)
    # hex vids like p1/p2: the old vid:paper:N.MvK regex matched none;
    # structural edge resolution finds the Paper endpoint.
    assert m["revision_bound_papers"] == 1
    assert m["revision_edges_declared"] is True


def test_graph_semantics_enrichment_na_when_vocab_lacks_labels():
    """Era mismatch is N/A, not a fake 0% violation."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_graph_semantics_contract(service.root)
    (service.root / "data" / "graphs").mkdir(parents=True)
    snap = service.root / "data" / "graphs" / "s.sgsnap"
    _write_sgsnap(snap, [
        {"t": "n", "id": 1, "labels": ["Paper"], "props": {"vid": "v1"}},
    ])
    from governor.adapters.graph_query_reader import check_graph_semantics
    flags = check_graph_semantics(service.root)
    assert not any(f.flag == "graph_enrichment_missing" for f in flags)


def test_graph_semantics_store_root_wins_over_aux_session_view():
    """data/graphs materialized stores beat newer artifacts/ session views."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    (service.root / "data" / "graphs").mkdir(parents=True)
    store_snap = service.root / "data" / "graphs" / "older.sgsnap"
    aux_snap = service.root / "artifacts" / "loads" / "newer.sgsnap"
    _write_sgsnap(store_snap, [
        {"t": "n", "id": 1, "labels": ["Paper"], "props": {"vid": "v1"}},
    ])
    _write_sgsnap(aux_snap, [
        {"t": "n", "id": 1, "labels": ["Paper"], "props": {"vid": "v9"}},
    ])
    os.utime(store_snap, (1, 1))  # older mtime must not matter
    from governor.adapters.graph_query_reader import check_graph_semantics
    flags = check_graph_semantics(service.root)
    # contract copy absent → no rules → no flags; selection itself is
    # asserted by the reader API below instead.
    assert not any(f.flag == "graph_semantics_no_snapshots" for f in flags)


def _copy_data_integrity_contract(root: Path) -> None:
    repo = _scripts_dir.parent
    rel = "scripts/governor/checks/data_integrity.yaml"
    (root / rel).parent.mkdir(parents=True, exist_ok=True)
    import shutil
    shutil.copy2(repo / rel, root / rel)

# GAR-005 (M509-yzblqx S01/T02): conservation verdict provenance. The P9
# observation is a pure in-memory document, so the rule is exercised over the
# committed DATA contract, its two fixtures and the two Rust surfaces that
# declare the namespaces it mirrors. Every mutation happens on a scratch copy:
# no committed file is touched. Two tests bypass the Governor wiring with direct
# oracle calls so the rule cannot go green by accident, and one test mutates the
# clean fixture into the pre-fix shape to prove the failing case the plan
# demands — a historic verdict presented as current — is actually caught.

_CONSERVATION_PROVENANCE_PREFIX = "conservation_verdict_provenance"
_CONSERVATION_PROVENANCE_CONTRACT = (
    "scripts/governor/checks/conservation_verdict_provenance.yaml"
)
_CONSERVATION_PROVENANCE_POSITIVE = (
    "governor/fixtures/conservation_verdict_provenance/report_clean.positive.json"
)
_CONSERVATION_PROVENANCE_NEGATIVE = (
    "governor/fixtures/conservation_verdict_provenance/report_broken.negative.json"
)
_CONSERVATION_PROVENANCE_RUST_SURFACE = "crates/da-cli/src/source_cli.rs"
_CONSERVATION_PROVENANCE_RUST_CONTRACT = (
    "crates/da-cli/tests/conservation_provenance.rs"
)


def _copy_conservation_provenance_contract(root: Path) -> None:
    """Copy the provenance contract and every path the rule actually reads.

    The rule reads the DATA contract, both fixtures and the two Rust surfaces
    named by `source_markers`; without the Rust files the check would report
    `..._source_marker_missing` instead of exercising the fixture logic.
    """
    repo = _scripts_dir.parent
    for rel in [
        _CONSERVATION_PROVENANCE_CONTRACT,
        _CONSERVATION_PROVENANCE_POSITIVE,
        _CONSERVATION_PROVENANCE_NEGATIVE,
        _CONSERVATION_PROVENANCE_RUST_SURFACE,
        _CONSERVATION_PROVENANCE_RUST_CONTRACT,
    ]:
        _write(root / rel, (repo / rel).read_text(encoding="utf-8"))


def _conservation_provenance_service() -> TrajectoryService:
    """Scratch TrajectoryService with the committed provenance contract copied in."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_conservation_provenance_contract(service.root)
    return service


def _conservation_provenance_flags_named(flags, name: str):
    return [f for f in flags if f.flag == f"{_CONSERVATION_PROVENANCE_PREFIX}_{name}"]


def _conservation_provenance_contract_document(service: TrajectoryService) -> dict:
    return yaml.safe_load(
        (service.root / _CONSERVATION_PROVENANCE_CONTRACT).read_text(encoding="utf-8")
    )


def _write_conservation_provenance_contract(
    service: TrajectoryService, contract: dict
) -> None:
    _write(
        service.root / _CONSERVATION_PROVENANCE_CONTRACT,
        yaml.safe_dump(contract, sort_keys=False),
    )


def test_conservation_provenance_pass_on_clean_fixtures():
    """The committed contract, fixtures and Rust markers satisfy the rule."""
    service = _conservation_provenance_service()
    assert service._check_conservation_verdict_provenance() == []


def test_conservation_provenance_missing_contract_is_silent():
    """No contract DATA means no rule to evaluate, not a failing rule."""
    service = _conservation_provenance_service()
    (service.root / _CONSERVATION_PROVENANCE_CONTRACT).unlink()
    assert service._check_conservation_verdict_provenance() == []


def test_conservation_provenance_missing_fixture_is_flagged():
    for rel, which in [
        (_CONSERVATION_PROVENANCE_POSITIVE, "positive"),
        (_CONSERVATION_PROVENANCE_NEGATIVE, "negative"),
    ]:
        service = _conservation_provenance_service()
        (service.root / rel).unlink()
        flags = service._check_conservation_verdict_provenance()
        missing = _conservation_provenance_flags_named(flags, "fixture_missing")
        assert len(missing) == 1
        assert which in missing[0].evidence
        assert missing[0].severity == "high"
        assert missing[0].dimension == "scholarly_parse_plane"


def test_conservation_provenance_positive_fixture_must_be_clean():
    """The failing case the plan names: a historic verdict presented as current."""
    service = _conservation_provenance_service()
    path = service.root / _CONSERVATION_PROVENANCE_POSITIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    row = records[0]["cascade_observe"]["P9"]
    # Promote the relocated historic claim into the current-run namespace while
    # the honesty flags keep denying the computation.
    row["current_run"]["verdict"] = "conserved"
    row["current_run"]["evaluation_state"] = "measured"
    _write(path, json.dumps(records))
    flags = service._check_conservation_verdict_provenance()
    broken = _conservation_provenance_flags_named(flags, "broken")
    assert len(broken) == 1
    assert "historic_verdict_presented_as_current" in broken[0].evidence
    assert "honesty_flags_disagree" in broken[0].evidence


def test_conservation_provenance_negative_fixture_must_trigger_labelled_violations():
    service = _conservation_provenance_service()
    path = service.root / _CONSERVATION_PROVENANCE_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # Declare the namespace the record was missing so its label can no longer
    # fire: the mismatch must be reported rather than silently accepted.
    for record in records:
        if record.get("violation") == "namespace_not_declared":
            row = record["record"]["cascade_observe"]["P9"]["current_run"]
            row["verdict_namespace"] = "current_run"
    _write(path, json.dumps(records))
    flags = service._check_conservation_verdict_provenance()
    broken = _conservation_provenance_flags_named(flags, "broken")
    assert any("namespace_not_declared" in f.evidence for f in broken)


def test_conservation_provenance_negative_fixture_must_cover_required_violations():
    service = _conservation_provenance_service()
    path = service.root / _CONSERVATION_PROVENANCE_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # `provenance_absent` is produced by exactly one record: removing it leaves
    # a detectable hole in the required coverage.
    records = [r for r in records if r.get("violation") != "provenance_absent"]
    _write(path, json.dumps(records))
    flags = service._check_conservation_verdict_provenance()
    broken = _conservation_provenance_flags_named(flags, "broken")
    assert any(
        "missing required violation coverage" in f.evidence
        and "provenance_absent" in f.evidence
        for f in broken
    )


def test_conservation_provenance_missing_vocabulary_is_flagged():
    """An empty measured vocabulary would make every record clean: fail closed."""
    service = _conservation_provenance_service()
    contract = _conservation_provenance_contract_document(service)
    contract["provenance"]["measured_verdicts"] = []
    assert missing_provenance_vocabulary(contract) == ["measured_verdicts"]
    _write_conservation_provenance_contract(service, contract)
    flags = service._check_conservation_verdict_provenance()
    missing = _conservation_provenance_flags_named(flags, "vocabulary_missing")
    assert len(missing) == 1
    assert "measured_verdicts" in missing[0].evidence
    assert missing[0].severity == "high"


def test_conservation_provenance_source_marker_missing_is_flagged():
    service = _conservation_provenance_service()
    _write(service.root / _CONSERVATION_PROVENANCE_RUST_SURFACE, "fn main() {}\n")
    flags = service._check_conservation_verdict_provenance()
    marker = _conservation_provenance_flags_named(flags, "source_marker_missing")
    assert len(marker) == 1
    assert "rust_surface" in marker[0].evidence

    service = _conservation_provenance_service()
    _write(service.root / _CONSERVATION_PROVENANCE_RUST_CONTRACT, "fn main() {}\n")
    flags = service._check_conservation_verdict_provenance()
    marker = _conservation_provenance_flags_named(flags, "source_marker_missing")
    assert len(marker) == 1
    assert "rust_contract" in marker[0].evidence


def test_conservation_provenance_invalid_yaml_is_flagged():
    service = _conservation_provenance_service()
    _write(service.root / _CONSERVATION_PROVENANCE_CONTRACT, "{bad yaml: [")
    flags = service._check_conservation_verdict_provenance()
    assert flags[0].flag == f"{_CONSERVATION_PROVENANCE_PREFIX}_invalid"


def test_conservation_provenance_absent_snapshot_suite_is_not_flagged():
    """The rule has no durable snapshot task suite; the opt-out must be explicit."""
    service = _conservation_provenance_service()
    contract = _conservation_provenance_contract_document(service)
    assert "graph_task_suite" not in contract
    opted_out = _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        _CONSERVATION_PROVENANCE_PREFIX, conservation_provenance_violations,
        graph_task_suite_required=False,
    )
    assert not [
        f for f in opted_out
        if f.flag == f"{_CONSERVATION_PROVENANCE_PREFIX}_suite_missing"
    ], opted_out
    default = _candidate_contract_flags(
        contract, service.root, "scholarly_parse_plane",
        _CONSERVATION_PROVENANCE_PREFIX, conservation_provenance_violations,
    )
    assert any(
        f.flag == f"{_CONSERVATION_PROVENANCE_PREFIX}_suite_missing" for f in default
    )


def test_conservation_provenance_oracle_rejects_historic_verdict_as_current():
    """Falsifiable without the Governor harness (ADR-051)."""
    contract = _conservation_provenance_contract_document(
        _conservation_provenance_service()
    )
    # The pre-fix GAR-005 shape: a bare top-level verdict with no namespace.
    pre_fix = {"cascade_observe": {"P9": {
        "engine_called": False,
        "computed_this_run": False,
        "tracked_verdict": "conserved",
    }}}
    assert "historic_verdict_presented_as_current" in (
        conservation_provenance_violations(pre_fix, contract)
    )
    # A run that declares its own measurement is accepted.
    measured = {"cascade_observe": {"P9": {
        "engine_called": True,
        "computed_this_run": True,
        "current_run": {
            "verdict_namespace": "current_run",
            "verdict": "conserved",
            "evaluation_state": "measured",
        },
    }}}
    assert conservation_provenance_violations(measured, contract) == set()
    # A historic claim inside the current-run namespace while the flags deny
    # the computation is the same failure, and it is named twice on purpose:
    # once for the misplaced claim, once for the disagreement with the flags.
    denied = {"cascade_observe": {"P9": {
        "engine_called": False,
        "computed_this_run": False,
        "current_run": {
            "verdict_namespace": "current_run",
            "verdict": "conserved",
            "evaluation_state": "measured",
        },
    }}}
    assert {
        "historic_verdict_presented_as_current",
        "honesty_flags_disagree",
    } <= conservation_provenance_violations(denied, contract)


def test_conservation_provenance_oracle_fails_closed_on_absent_and_unknown():
    """Absence is never compliance: every malformed shape names a label."""
    contract = _conservation_provenance_contract_document(
        _conservation_provenance_service()
    )
    cases = [
        ({"report_id": "no-observation"}, {"provenance_absent"}),
        ({"cascade_observe": {"P9": {
            "engine_called": False, "computed_this_run": False}}},
         {"missing_current_run"}),
        ({"cascade_observe": {"P9": {
            "current_run": {"verdict": "NotEvaluated"}}}},
         {"namespace_not_declared"}),
        ({"cascade_observe": {"P9": {"current_run": {
            "verdict_namespace": "historical_reference",
            "verdict": "NotEvaluated"}}}},
         {"namespace_contradicts_key"}),
        ({"cascade_observe": {"P9": {"current_run": {
            "verdict_namespace": "current_run"}}}},
         {"missing_current_verdict"}),
        ({"cascade_observe": {"P9": {"current_run": {
            "verdict_namespace": "current_run",
            "verdict": "probably_fine"}}}},
         {"unknown_verdict_token"}),
        ({"cascade_observe": {"P9": {
            "current_run": {
                "verdict_namespace": "current_run",
                "verdict": "NotEvaluated",
            },
            "reference_evidence": {"verdict": "conserved"}}}},
         {"historic_reference_mislabelled"}),
        ("not-a-record", {"missing_record_object"}),
    ]
    for record, expected in cases:
        violations = conservation_provenance_violations(record, contract)
        assert expected <= violations, (record, violations)
        assert violations, record


def test_conservation_provenance_oracle_bounds_provenance_identity():
    """Provenance is a bounded identity, not a payload (Q3 R4)."""
    contract = _conservation_provenance_contract_document(
        _conservation_provenance_service()
    )
    record = {"cascade_observe": {"P9": {
        "engine_called": False,
        "computed_this_run": False,
        "current_run": {
            "verdict_namespace": "current_run",
            "verdict": "NotEvaluated",
        },
        "reference_evidence": {
            "verdict_namespace": "historical_reference",
            "evidence_state": "historical",
            "verdict": "conserved",
            "artifact_digest": "a" * 512,
        },
    }}}
    violations = conservation_provenance_violations(record, contract)
    assert violations == {"oversized_provenance_value"}, violations


def test_conservation_provenance_is_wired_into_build_report():
    """A rule that is implemented but never invoked is a dead rule.

    `build_report` is the single governor validation path (the `preflight` and
    `doctor` entry points both go through it). Constructing it needs every
    collaborator, so this pins the call site instead: deleting the delegation
    fails here rather than silently disabling the rule. The end-to-end
    behaviour is additionally evidenced by `governor --phase preflight`
    producing no `conservation_verdict_provenance` flag on the committed tree.
    """
    import inspect

    source = inspect.getsource(TrajectoryService.build_report)
    assert "_check_conservation_verdict_provenance()" in source


# ── GAR-006 executed population binding (M509-yzblqx/S02/T02) ─────────────────
# The split contract in `crates/da-parse/tests/data_contracts.rs` (T01) proves the
# reports HAVE three populations; the rule under test proves a corpus-scale claim
# is BOUND to execution evidence for the population it names, and that
# `executed ⊆ reference` / `evaluated ⊆ executed` hold. Every mutation happens on
# a scratch copy of the committed DATA, its two fixtures, the shared population
# contract and the ten committed shadow reports: no committed file is touched.
# Two tests bypass the Governor wiring with direct oracle calls (ADR-051), and the
# plan's two named failing cases — a large referenced count beside a small
# execution, and an absent executed population — are asserted both through the
# committed fixtures and directly.

_EXECUTED_POPULATION_PREFIX = "executed_population_binding"
_EXECUTED_POPULATION_CONTRACT = (
    "scripts/governor/checks/executed_population_binding.yaml"
)
_EXECUTED_POPULATION_POSITIVE = (
    "governor/fixtures/executed_population_binding/report_clean.positive.json"
)
_EXECUTED_POPULATION_NEGATIVE = (
    "governor/fixtures/executed_population_binding/report_broken.negative.json"
)
_EXECUTED_POPULATION_MANIFEST = "data/parse/acceptance_manifest.yaml"
_EXECUTED_POPULATION_MARKERS = ("crates/da-parse/tests/data_contracts.rs",)


def _executed_population_declared_reports(repo: Path) -> list[str]:
    """The shadow reports the shared population contract enumerates."""
    manifest = yaml.safe_load(
        (repo / _EXECUTED_POPULATION_MANIFEST).read_text(encoding="utf-8")
    )
    contract = yaml.safe_load(
        (repo / _EXECUTED_POPULATION_CONTRACT).read_text(encoding="utf-8")
    )
    return declared_report_paths(manifest, contract)


def _copy_executed_population_binding(root: Path) -> None:
    """Copy every path the binding rule actually reads.

    The rule reads the DATA contract, both fixtures, the shared population
    contract and its declared reports, plus the two source markers; without the
    reports the check would report `..._report_missing` instead of exercising the
    live binding logic.
    """
    repo = _scripts_dir.parent
    for rel in [
        _EXECUTED_POPULATION_CONTRACT,
        _EXECUTED_POPULATION_POSITIVE,
        _EXECUTED_POPULATION_NEGATIVE,
        _EXECUTED_POPULATION_MANIFEST,
        *_EXECUTED_POPULATION_MARKERS,
        *_executed_population_declared_reports(repo),
    ]:
        _write(root / rel, (repo / rel).read_text(encoding="utf-8"))


def _executed_population_service() -> TrajectoryService:
    """Scratch TrajectoryService with the committed binding DATA copied in."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    _copy_executed_population_binding(service.root)
    return service


def _executed_population_flags_named(flags, name: str):
    return [f for f in flags if f.flag == f"{_EXECUTED_POPULATION_PREFIX}_{name}"]


def _executed_population_contract_document(service: TrajectoryService) -> dict:
    return yaml.safe_load(
        (service.root / _EXECUTED_POPULATION_CONTRACT).read_text(encoding="utf-8")
    )


def _executed_population_fixture(service: TrajectoryService, rel: str):
    return json.loads((service.root / rel).read_text(encoding="utf-8"))


def test_executed_population_binding_pass_on_clean_fixtures():
    """The committed DATA, fixtures, markers and all ten shadow reports pass."""
    service = _executed_population_service()
    assert service._check_executed_population_binding() == []


def test_executed_population_binding_missing_contract_is_silent():
    """No contract DATA means no rule to evaluate, not a failing rule."""
    service = _executed_population_service()
    (service.root / _EXECUTED_POPULATION_CONTRACT).unlink()
    assert service._check_executed_population_binding() == []


def test_executed_population_binding_missing_fixture_is_flagged():
    for rel, which in [
        (_EXECUTED_POPULATION_POSITIVE, "positive"),
        (_EXECUTED_POPULATION_NEGATIVE, "negative"),
    ]:
        service = _executed_population_service()
        (service.root / rel).unlink()
        flags = service._check_executed_population_binding()
        missing = _executed_population_flags_named(flags, "fixture_missing")
        assert len(missing) == 1
        assert which in missing[0].evidence
        assert missing[0].severity == "high"
        assert missing[0].dimension == "population_honesty"


def test_executed_population_binding_positive_fixture_must_be_clean():
    """The failing case the plan names: a corpus claim with no evidence."""
    service = _executed_population_service()
    path = service.root / _EXECUTED_POPULATION_POSITIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # The honest synthetic fixture starts claiming corpus execution while the
    # evidence that would bind that population is removed.
    executed = records[1]["executed_population"]
    executed["corpus_execution"] = True
    del executed["execution_evidence"]
    path.write_text(json.dumps(records), encoding="utf-8")
    flags = service._check_executed_population_binding()
    broken = _executed_population_flags_named(flags, "broken")
    assert len(broken) == 1
    assert "corpus_claim_without_execution_evidence" in broken[0].evidence


def test_executed_population_binding_negative_fixture_must_trigger_labelled_violations():
    service = _executed_population_service()
    path = service.root / _EXECUTED_POPULATION_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # Give the absent-executed-population record a present executed population:
    # its label can no longer fire, so the mismatch must be reported.
    for record in records:
        if record.get("violation") == "executed_population_absent":
            record["record"]["executed_population"] = {
                "corpus_execution": False,
                "documents": 1,
                "execution_evidence": "tracked_offline_fixture:x",
            }
    path.write_text(json.dumps(records), encoding="utf-8")
    flags = service._check_executed_population_binding()
    broken = _executed_population_flags_named(flags, "broken")
    assert any("executed_population_absent" in f.evidence for f in broken)


def test_executed_population_binding_negative_fixture_must_cover_required_violations():
    service = _executed_population_service()
    path = service.root / _EXECUTED_POPULATION_NEGATIVE
    records = json.loads(path.read_text(encoding="utf-8"))
    # The plan's first named failing case is the only source of this label:
    # removing it leaves a detectable hole in the required coverage.
    records = [
        r for r in records
        if r.get("violation") != "evaluated_population_exceeds_executed"
    ]
    path.write_text(json.dumps(records), encoding="utf-8")
    flags = service._check_executed_population_binding()
    broken = _executed_population_flags_named(flags, "broken")
    assert any(
        "missing required violation coverage" in f.evidence
        and "evaluated_population_exceeds_executed" in f.evidence
        for f in broken
    )


def test_executed_population_binding_missing_vocabulary_is_flagged():
    """An empty evidence vocabulary would make a corpus claim unbound-by-default."""
    service = _executed_population_service()
    contract = _executed_population_contract_document(service)
    contract["populations"]["evidence_kinds"] = []
    contract["populations"]["corpus_scale_documents"] = 0
    assert missing_binding_vocabulary(contract) == [
        "evidence_kinds", "corpus_scale_documents",
    ]
    _write(
        service.root / _EXECUTED_POPULATION_CONTRACT,
        yaml.safe_dump(contract, sort_keys=False),
    )
    flags = service._check_executed_population_binding()
    missing = _executed_population_flags_named(flags, "vocabulary_missing")
    assert len(missing) == 1
    assert "evidence_kinds" in missing[0].evidence
    assert missing[0].severity == "high"


def test_executed_population_binding_source_marker_missing_is_flagged():
    service = _executed_population_service()
    _write(service.root / _EXECUTED_POPULATION_MANIFEST, "# stripped markers\n")
    flags = service._check_executed_population_binding()
    marker = _executed_population_flags_named(flags, "source_marker_missing")
    assert len(marker) == 1
    assert "population_contract" in marker[0].evidence

    service = _executed_population_service()
    _write(service.root / _EXECUTED_POPULATION_MARKERS[0], "fn main() {}\n")
    flags = service._check_executed_population_binding()
    marker = _executed_population_flags_named(flags, "source_marker_missing")
    assert len(marker) == 1
    assert "population_split_contract_test" in marker[0].evidence


def test_executed_population_binding_invalid_yaml_is_flagged():
    service = _executed_population_service()
    _write(service.root / _EXECUTED_POPULATION_CONTRACT, "{bad yaml: [")
    flags = service._check_executed_population_binding()
    assert flags[0].flag == f"{_EXECUTED_POPULATION_PREFIX}_invalid"


def test_executed_population_binding_absent_snapshot_suite_is_not_flagged():
    """The rule has no durable snapshot task suite; the opt-out must be explicit."""
    service = _executed_population_service()
    contract = _executed_population_contract_document(service)
    assert "graph_task_suite" not in contract
    opted_out = _candidate_contract_flags(
        contract, service.root, "population_honesty",
        _EXECUTED_POPULATION_PREFIX, executed_population_violations,
        graph_task_suite_required=False,
    )
    assert not [
        f for f in opted_out
        if f.flag == f"{_EXECUTED_POPULATION_PREFIX}_suite_missing"
    ], opted_out
    default = _candidate_contract_flags(
        contract, service.root, "population_honesty",
        _EXECUTED_POPULATION_PREFIX, executed_population_violations,
    )
    assert any(
        f.flag == f"{_EXECUTED_POPULATION_PREFIX}_suite_missing" for f in default
    )


def test_executed_population_binding_oracle_rejects_absent_executed_population():
    """The plan's second named failing case, falsifiable without the harness."""
    contract = _executed_population_contract_document(
        _executed_population_service()
    )
    no_executed = {
        "report_id": "executed_population_missing",
        "reference_population": {
            "role": "identity_only", "identity": "reviewed_corpus_reference",
            "documents": 589, "lane_pages": 606,
        },
        "evaluated_population": {"status": "not_evaluated", "documents": 0},
    }
    assert executed_population_violations(no_executed, contract) == {
        "executed_population_absent"
    }


def test_executed_population_binding_oracle_rejects_large_reference_beside_small_execution():
    """The plan's first named failing case, falsifiable without the harness."""
    contract = _executed_population_contract_document(
        _executed_population_service()
    )
    reference = {
        "role": "identity_only", "identity": "reviewed_corpus_reference",
        "documents": 589, "lane_pages": 606,
    }
    small_execution = {
        "corpus_execution": False, "documents": 3, "pages": 202,
        "execution_evidence": "tracked_offline_fixture:small_constructed_input",
    }
    # The large referenced count presented as an evaluation outcome of a small
    # execution is rejected ...
    presented = {
        "reference_population": dict(reference),
        "executed_population": dict(small_execution),
        "evaluated_population": {"status": "not_evaluated", "documents": 589},
    }
    violations = executed_population_violations(presented, contract)
    assert "evaluated_population_exceeds_executed" in violations, violations
    # ... while the honest sibling (the same numbers with the evaluation
    # explicitly empty) is clean, so the rule does not reject honest reports.
    honest = {
        "reference_population": dict(reference),
        "executed_population": dict(small_execution),
        "evaluated_population": {"status": "not_evaluated", "documents": 0},
    }
    assert executed_population_violations(honest, contract) == set()


def test_executed_population_binding_oracle_rejects_replayed_and_spliced_receipts():
    contract = _executed_population_contract_document(
        _executed_population_service()
    )
    reference = {
        "role": "identity_only", "identity": "reviewed_corpus_reference",
        "documents": 600,
    }
    # A valid token copied from a real corpus report cannot bind a corpus-scale
    # executed number.
    replayed = {
        "reference_population": dict(reference),
        "executed_population": {
            "corpus_execution": True, "identity": "reviewed_corpus_reference",
            "documents": 589,
            "execution_evidence": "tracked_source_pdfs_with_sha256",
        },
        "evaluated_population": {"status": "not_evaluated", "documents": 0},
    }
    assert executed_population_violations(replayed, contract) == {
        "execution_evidence_unbound"
    }
    # A structured receipt whose identity and count belong to another report.
    spliced = {
        "reference_population": dict(reference),
        "executed_population": {
            "corpus_execution": True, "identity": "reviewed_corpus_reference",
            "documents": 589,
            "execution_evidence": {
                "kind": "tracked_source_pdfs_with_sha256",
                "population_identity": "some_other_report",
                "documents": 3,
            },
        },
        "evaluated_population": {"status": "not_evaluated", "documents": 0},
    }
    assert executed_population_violations(spliced, contract) == {
        "execution_receipt_mismatch"
    }


def test_executed_population_binding_oracle_accepts_bound_receipt_and_bounds_identity():
    contract = _executed_population_contract_document(
        _executed_population_service()
    )
    bound = {
        "reference_population": {
            "role": "identity_only", "identity": "reviewed_corpus_reference",
            "documents": 12,
        },
        "executed_population": {
            "corpus_execution": True, "identity": "reviewed_corpus_reference",
            "documents": 12,
            "execution_evidence": {
                "kind": "tracked_source_pdfs_with_sha256",
                "population_identity": "reviewed_corpus_reference",
                "documents": 12,
                "digest": "sha256:00ff",
            },
        },
        "evaluated_population": {
            "status": "evaluated", "documents": 12,
            "oracle": "reviewed_corpus_oracle",
        },
    }
    assert executed_population_violations(bound, contract) == set()
    # Identity and evidence are bounded identities, not payloads (Q3 R4).
    oversized = {
        "reference_population": {
            "role": "identity_only", "identity": "a" * 512, "documents": 3,
        },
        "executed_population": {
            "corpus_execution": False, "documents": 1,
            "execution_evidence": "tracked_offline_fixture:x",
        },
        "evaluated_population": {"status": "not_evaluated", "documents": 0},
    }
    assert executed_population_violations(oversized, contract) == {
        "oversized_population_value"
    }


def test_executed_population_binding_live_report_presented_as_executed_is_flagged():
    """A committed shadow report mutated into the GAR-006 shape is rejected."""
    service = _executed_population_service()
    rel = "artifacts/parse_compat/callout_matching_shadow.json"
    document = json.loads((service.root / rel).read_text(encoding="utf-8"))
    document["reference_population"]["documents"] = 589
    document["evaluated_population"]["documents"] = 589
    _write(service.root / rel, json.dumps(document))
    flags = service._check_executed_population_binding()
    broken = _executed_population_flags_named(flags, "report_broken")
    assert len(broken) == 1
    assert rel in broken[0].evidence
    assert "evaluated_population_exceeds_executed" in broken[0].evidence
    assert broken[0].severity == "high"


def test_executed_population_binding_live_report_missing_is_flagged():
    service = _executed_population_service()
    rel = "artifacts/parse_compat/callout_matching_shadow.json"
    (service.root / rel).unlink()
    flags = service._check_executed_population_binding()
    missing = _executed_population_flags_named(flags, "report_missing")
    assert len(missing) == 1
    assert rel in missing[0].evidence


def test_executed_population_binding_undeclared_report_is_flagged():
    """A shadow report outside the shared contract is never bound: fail closed."""
    service = _executed_population_service()
    rogue = service.root / "artifacts/parse_compat/rogue_shadow.json"
    _write(rogue, json.dumps({
        "schema_version": "shadow-populations.v1",
        "reference_population": {"documents": 589},
        "executed_population": {"documents": 3},
        "evaluated_population": {"documents": 0},
    }))
    flags = service._check_executed_population_binding()
    undeclared = _executed_population_flags_named(flags, "report_undeclared")
    assert len(undeclared) == 1
    assert "rogue_shadow.json" in undeclared[0].evidence


def test_executed_population_binding_is_wired_into_build_report():
    """A rule that is implemented but never invoked is a dead rule.

    `build_report` is the single governor validation path (`preflight` and
    `doctor` both go through it). Constructing it needs every collaborator, so
    this pins the call site instead: deleting the delegation fails here rather
    than silently disabling the rule. The end-to-end behaviour is additionally
    evidenced by `governor --phase preflight` producing no
    `executed_population_binding` flag on the committed tree.
    """
    import inspect

    source = inspect.getsource(TrajectoryService.build_report)
    assert "_check_executed_population_binding()" in source


# ---------------------------------------------------------------------------
# GAR-014: bounded cross-automata invalidation map (M509-yzblqx/S04/T01).
# ---------------------------------------------------------------------------

_FSM_INTERACTION_MAP_CONTRACT = "scripts/governor/checks/fsm_interaction_map.yaml"
_FSM_INTERACTION_MAP_POINTER = "data/policies/fsm_interaction_map.yaml"
_FSM_INTERACTION_MAP_EVIDENCE = "crates/da-domain/src/fsm_policy.rs"
_FSM_INTERACTION_MAP_AUTOMATA = [
    "parse_execution",
    "derived_maturity",
    "article_assurance",
    "learning_evaluation",
]


def _fsm_interaction_map_contract() -> dict:
    """Minimal valid GAR-014 map: four automata, each with an invalidating input."""

    def automaton(prefix: str) -> dict:
        return {
            "authority": f"{prefix}-authority",
            "evidence_source": _FSM_INTERACTION_MAP_EVIDENCE,
            "grants_production_authority": False,
            "local_transitions": [
                {
                    "id": f"{prefix}_local_step",
                    "states": ["start", "done"],
                    "stays_local": True,
                }
            ],
            "retry_preconditions": [
                {
                    "id": f"{prefix}_retry_same_key",
                    "allowed_when": ["semantic_key_unchanged"],
                    "forbidden_when": ["semantic_key_changed"],
                    "requires_rederivation": False,
                },
                {
                    "id": f"{prefix}_rederive",
                    "allowed_when": [],
                    "forbidden_when": ["semantic_key_changed"],
                    "requires_rederivation": True,
                },
            ],
            "non_authorizing_states": ["start", "done"],
        }

    return {
        "schema_version": "fsm-interaction-map-test.v1",
        "policy_id": "fsm_interaction_map",
        "dimension": "scholarly_parse_plane",
        "severity": "medium",
        "anchor": "GAR-014",
        "finding": "GAR-014",
        "finding_registry": "doc/review/review.jsonl",
        "map_role": "descriptive_only",
        "forbidden_controller_keys": ["controller", "executor", "dispatch"],
        "trigger_classes": ["source", "policy", "model", "input"],
        "actions": [
            "reparse_and_rederive",
            "full_rederivation",
            "reauthorize",
            "re_evaluate",
            "fail_visible",
            "none",
        ],
        "automata": {name: automaton(name) for name in _FSM_INTERACTION_MAP_AUTOMATA},
        "invalidation_rules": [
            {
                "trigger": "source_digest_changed",
                "trigger_class": "source",
                "invalidates": ["parse_execution", "derived_maturity"],
                "action": "full_rederivation",
                "requires_rederivation": True,
            },
            {
                "trigger": "policy_hash_changed",
                "trigger_class": "policy",
                "invalidates": ["article_assurance"],
                "action": "reauthorize",
                "requires_rederivation": True,
            },
            {
                "trigger": "model_weights_changed",
                "trigger_class": "model",
                "invalidates": ["learning_evaluation"],
                "action": "re_evaluate",
                "requires_rederivation": True,
            },
            {
                "trigger": "receipt_identity_changed",
                "trigger_class": "input",
                "invalidates": ["derived_maturity"],
                "action": "full_rederivation",
                "requires_rederivation": True,
            },
        ],
        "non_authorizing_invariants": [
            {
                "id": "no_state_authorizes_import",
                "statement": "No state in this map authorizes import or production.",
            }
        ],
        "evidence_bindings": [
            {
                "path": _FSM_INTERACTION_MAP_EVIDENCE,
                "required_patterns": ["pub enum DecisionPoint", "grammar_route"],
            }
        ],
        "pointer_sources": [_FSM_INTERACTION_MAP_POINTER],
    }


def _write_fsm_interaction_map(root: Path, contract: dict | None = None) -> None:
    """Write a self-contained GAR-014 fixture (map, evidence, pointer, registry)."""
    _write(
        root / _FSM_INTERACTION_MAP_CONTRACT,
        yaml.safe_dump(contract or _fsm_interaction_map_contract(), sort_keys=False),
    )
    _write(
        root / _FSM_INTERACTION_MAP_EVIDENCE,
        "// fixture evidence\npub enum DecisionPoint { GrammarRoute }\n// grammar_route profile_route\n",
    )
    _write(
        root / _FSM_INTERACTION_MAP_POINTER,
        "schema_version: fsm-interaction-map-ref.v1\n"
        f"canonical_map: {_FSM_INTERACTION_MAP_CONTRACT}\n",
    )
    _write(root / "doc/review/review.jsonl", json.dumps({"finding_id": "GAR-014"}) + "\n")


def _fsm_interaction_map_check(root: Path) -> list[DriftFlag]:
    """Run the check through the trajectory service method (covers the wiring)."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = root
    return service._check_fsm_interaction_map()


def _fsm_interaction_map_flags(root: Path, contract: dict) -> set[str]:
    _write_fsm_interaction_map(root, contract)
    return {flag.flag for flag in _fsm_interaction_map_check(root)}


def test_fsm_interaction_map_contract_is_clean_on_real_repo():
    """The committed map must pass its own rule (a mapping that fails is dead DATA)."""
    assert _fsm_interaction_map_check(_scripts_dir.parent) == []


def test_fsm_interaction_map_fixture_contract_is_clean():
    root = _make_root()
    _write_fsm_interaction_map(root)
    assert _fsm_interaction_map_check(root) == []


def test_fsm_interaction_map_absent_policy_is_silent():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    assert service._check_fsm_interaction_map() == []


def test_fsm_interaction_map_invalid_yaml_is_flagged():
    root = _make_root()
    _write_fsm_interaction_map(root)
    _write(root / _FSM_INTERACTION_MAP_CONTRACT, "automata: [unclosed\n")
    flags = {flag.flag for flag in _fsm_interaction_map_check(root)}
    assert "fsm_interaction_map_invalid" in flags


def test_fsm_interaction_map_non_mapping_contract_is_flagged():
    root = _make_root()
    _write_fsm_interaction_map(root)
    _write(root / _FSM_INTERACTION_MAP_CONTRACT, "- automata\n- invalidation_rules\n")
    flags = {flag.flag for flag in _fsm_interaction_map_check(root)}
    assert "fsm_interaction_map_invalid" in flags


def test_fsm_interaction_map_unmapped_automaton_is_flagged():
    """GAR-014 acceptance: every automaton needs an explicit invalidating input."""
    contract = _fsm_interaction_map_contract()
    contract["invalidation_rules"][2]["invalidates"] = ["parse_execution"]
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_unmapped_automaton" in flags


def test_fsm_interaction_map_unknown_automaton_is_flagged():
    contract = _fsm_interaction_map_contract()
    contract["invalidation_rules"][2]["invalidates"] = ["ghost_automaton"]
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_unknown_automaton" in flags
    # The declared automaton it replaced is now unmapped as well.
    assert "fsm_interaction_map_unmapped_automaton" in flags


def test_fsm_interaction_map_empty_invalidates_is_flagged():
    contract = _fsm_interaction_map_contract()
    contract["invalidation_rules"][2]["invalidates"] = []
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_broken" in flags


def test_fsm_interaction_map_missing_trigger_class_is_flagged():
    contract = _fsm_interaction_map_contract()
    contract["invalidation_rules"][2]["trigger_class"] = "policy"
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_trigger_class_missing" in flags


def test_fsm_interaction_map_controller_key_is_flagged():
    """The map must not become an executive controller."""
    contract = _fsm_interaction_map_contract()
    contract["automata"]["parse_execution"]["dispatch"] = "run_parse"
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_controller_leak" in flags


def test_fsm_interaction_map_non_descriptive_role_is_flagged():
    contract = _fsm_interaction_map_contract()
    contract["map_role"] = "executive_controller"
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_controller_leak" in flags


def test_fsm_interaction_map_production_authority_is_flagged():
    contract = _fsm_interaction_map_contract()
    contract["automata"]["article_assurance"]["grants_production_authority"] = True
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_controller_leak" in flags


def test_fsm_interaction_map_missing_automaton_sections_is_flagged():
    contract = _fsm_interaction_map_contract()
    del contract["automata"]["derived_maturity"]["retry_preconditions"]
    del contract["automata"]["derived_maturity"]["non_authorizing_states"]
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_broken" in flags


def test_fsm_interaction_map_retry_without_rederivation_is_flagged():
    """A non-`none` action may not declare that re-derivation is unnecessary."""
    contract = _fsm_interaction_map_contract()
    contract["invalidation_rules"][0]["requires_rederivation"] = False
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_broken" in flags


def test_fsm_interaction_map_evidence_path_missing_is_flagged():
    contract = _fsm_interaction_map_contract()
    contract["evidence_bindings"][0]["path"] = "crates/da-domain/src/gone.rs"
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_evidence_missing" in flags


def test_fsm_interaction_map_evidence_drift_is_flagged():
    """A pinned symbol disappearing from the bound source reddens the map."""
    contract = _fsm_interaction_map_contract()
    contract["evidence_bindings"][0]["required_patterns"].append("pub enum InventedAutomaton")
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_evidence_drift" in flags


def test_fsm_interaction_map_duplicate_pointer_map_is_flagged():
    """A pointer that grows rival rules is a second, divergent map."""
    root = _make_root()
    _write_fsm_interaction_map(root)
    _write(
        root / _FSM_INTERACTION_MAP_POINTER,
        yaml.safe_dump(
            {
                "schema_version": "fsm-interaction-map.v1",
                "canonical_map": _FSM_INTERACTION_MAP_CONTRACT,
                "automata": {"parse_execution": {}},
                "invalidation_rules": [{"trigger": "source_digest_changed"}],
            }
        ),
    )
    flags = {flag.flag for flag in _fsm_interaction_map_check(root)}
    assert flags == {"fsm_interaction_map_duplicate_map"}


def test_fsm_interaction_map_pointer_wrong_canonical_is_flagged():
    root = _make_root()
    _write_fsm_interaction_map(root)
    _write(
        root / _FSM_INTERACTION_MAP_POINTER,
        "schema_version: fsm-interaction-map-ref.v1\n"
        "canonical_map: data/policies/other-map.yaml\n",
    )
    flags = {flag.flag for flag in _fsm_interaction_map_check(root)}
    assert flags == {"fsm_interaction_map_duplicate_map"}


def test_fsm_interaction_map_finding_missing_from_registry_is_flagged():
    contract = _fsm_interaction_map_contract()
    contract["finding"] = "GAR-999"
    flags = _fsm_interaction_map_flags(_make_root(), contract)
    assert "fsm_interaction_map_finding_missing" in flags


def test_fsm_interaction_map_every_mutation_is_not_silent():
    """Each violation class is isolated: the clean map is silent, every mutation reddens."""

    def unmapped(contract: dict) -> None:
        contract["invalidation_rules"][2]["invalidates"] = ["parse_execution"]

    def unknown(contract: dict) -> None:
        contract["invalidation_rules"][2]["invalidates"] = ["ghost_automaton"]

    def controller(contract: dict) -> None:
        contract["automata"]["parse_execution"]["dispatch"] = "run_parse"

    def role(contract: dict) -> None:
        contract["map_role"] = "executive_controller"

    def authority(contract: dict) -> None:
        contract["automata"]["article_assurance"]["grants_production_authority"] = True

    def empty_targets(contract: dict) -> None:
        contract["invalidation_rules"][2]["invalidates"] = []

    def missing_class(contract: dict) -> None:
        contract["invalidation_rules"][2]["trigger_class"] = "policy"

    def retry_bypass(contract: dict) -> None:
        contract["invalidation_rules"][2]["requires_rederivation"] = False

    def no_sections(contract: dict) -> None:
        del contract["automata"]["derived_maturity"]["retry_preconditions"]

    def evidence_missing(contract: dict) -> None:
        contract["evidence_bindings"][0]["path"] = "crates/da-domain/src/gone.rs"

    def evidence_drift(contract: dict) -> None:
        contract["evidence_bindings"][0]["required_patterns"].append("pub enum InventedAutomaton")

    def finding_gone(contract: dict) -> None:
        contract["finding"] = "GAR-999"

    cases = [
        ("unmapped", unmapped, "fsm_interaction_map_unmapped_automaton"),
        ("unknown", unknown, "fsm_interaction_map_unknown_automaton"),
        ("controller", controller, "fsm_interaction_map_controller_leak"),
        ("role", role, "fsm_interaction_map_controller_leak"),
        ("authority", authority, "fsm_interaction_map_controller_leak"),
        ("empty", empty_targets, "fsm_interaction_map_broken"),
        ("class", missing_class, "fsm_interaction_map_trigger_class_missing"),
        ("retry", retry_bypass, "fsm_interaction_map_broken"),
        ("sections", no_sections, "fsm_interaction_map_broken"),
        ("evidence", evidence_missing, "fsm_interaction_map_evidence_missing"),
        ("drift", evidence_drift, "fsm_interaction_map_evidence_drift"),
        ("finding", finding_gone, "fsm_interaction_map_finding_missing"),
    ]
    root = _make_root()
    _write_fsm_interaction_map(root)
    assert _fsm_interaction_map_check(root) == []
    for label, mutate, expected in cases:
        contract = _fsm_interaction_map_contract()
        mutate(contract)
        flags = _fsm_interaction_map_flags(root, contract)
        assert flags, f"{label} mutation was silently accepted"
        assert expected in flags, f"{label} expected {expected}, got {sorted(flags)}"


def test_fsm_interaction_map_is_wired_into_build_report():
    """A rule that is implemented but never invoked is a dead rule."""
    import inspect

    source = inspect.getsource(TrajectoryService.build_report)
    assert "_check_fsm_interaction_map()" in source


def test_fsm_interaction_map_summary_covers_every_automaton():
    from governor.adapters.checks.fsm_interaction_map import summarize_fsm_interaction_map

    summary = summarize_fsm_interaction_map(_scripts_dir.parent)
    assert summary["present"] is True
    assert summary["map_role"] == "descriptive_only"
    assert sorted(summary["automata"]) == sorted(_FSM_INTERACTION_MAP_AUTOMATA)
    assert summary["unmapped"] == []
    assert sorted(summary["trigger_classes"]) == ["input", "model", "policy", "source"]


# ---------------------------------------------------------------------------
# GAR-012: quarantine semantic boundary (M509-yzblqx/S04/T02).
# ---------------------------------------------------------------------------

_QS_CONTRACT = "scripts/governor/checks/quarantine_semantic_boundary.yaml"
_QS_SCHOLARLY = "scripts/governor/checks/scholarly_parse_plane.yaml"
_QS_GROBID = "data/parse/grobid_compatibility.yaml"
_QS_EXECUTED = "scripts/governor/checks/executed_population_binding.yaml"
_QS_LICENSE = "scripts/governor/checks/component_license_provenance.yaml"
_QS_REGISTRY = "doc/review/grobid-absorption-review-2026-09-10.jsonl"
_QS_SEMANTIC_CLAIMS = [
    "semantic_utility",
    "scientific_relevance",
    "output_quality",
    "parity",
    "legal_admissibility",
    "deployment_readiness",
]

def _qs_routing(term: str, surface: str, kind: str, path: str, patterns: list[str]) -> dict:
    return {
        "routed_to": surface,
        "evidence_kind": kind,
        "path": path,
        "required_patterns": patterns,
        "rationale": f"{term} is decided by {surface}, not by quarantine",
    }

def _quarantine_semantic_boundary_contract() -> dict:
    """Minimal valid GAR-012 boundary: isolation only, every non-claim routed."""
    return {
        "schema_version": "quarantine-semantic-boundary-test.v1",
        "policy_id": "quarantine_semantic_boundary",
        "dimension": "scholarly_parse_plane",
        "severity": "medium",
        "anchor": "GAR-012",
        "finding": "GAR-012",
        "finding_registry": _QS_REGISTRY,
        "boundary_role": "isolation_only",
        "canonical_contract": _QS_CONTRACT,
        "boundary_annotation_key": "semantic_boundary",
        "establishes": [
            "input_isolation",
            "safety_containment",
            "production_authority_lock",
        ],
        "does_not_establish": list(_QS_SEMANTIC_CLAIMS),
        "routing_evidence_kinds": [
            "executed_or_evaluated_population",
            "component_license_provenance",
        ],
        "quarantine_scan_routes": [
            "quarantine_semantic_boundary",
            "self",
            "isolation_scan",
            "quarantine_pass",
        ],
        "claim_routing": {
            "semantic_utility": _qs_routing(
                "semantic_utility", "executed_population_binding",
                "executed_or_evaluated_population", _QS_EXECUTED,
                ["policy_id: executed_population_binding", "corpus_execution"],
            ),
            "scientific_relevance": _qs_routing(
                "scientific_relevance", "executed_population_binding",
                "executed_or_evaluated_population", _QS_EXECUTED,
                ["policy_id: executed_population_binding", "corpus_execution"],
            ),
            "output_quality": _qs_routing(
                "output_quality", "executed_population_binding",
                "executed_or_evaluated_population", _QS_EXECUTED,
                ["policy_id: executed_population_binding", "corpus_execution"],
            ),
            "parity": _qs_routing(
                "parity", "executed_population_binding",
                "executed_or_evaluated_population", _QS_EXECUTED,
                ["policy_id: executed_population_binding", "corpus_execution"],
            ),
            "legal_admissibility": _qs_routing(
                "legal_admissibility", "component_license_provenance",
                "component_license_provenance", _QS_LICENSE,
                ["policy_id: component_license_provenance"],
            ),
            "deployment_readiness": _qs_routing(
                "deployment_readiness", "component_license_provenance",
                "component_license_provenance", _QS_LICENSE,
                ["policy_id: component_license_provenance"],
            ),
        },
        "semantic_claim_keys": [
            "relevance", "utility", "usefulness", "semantic_utility",
            "scientific_utility", "scientific_relevance", "admissibility",
            "admissible", "quality", "quality_score", "parity",
            "deployment_ready", "semantic_readiness",
        ],
        "rival_definition_keys": [
            "tokens", "boundaries", "forbidden_tokens", "source_bindings",
            "source_artifacts", "sanctioned_endpoints", "closed_taxonomy",
            "quarantine_definitions", "planes", "defaults",
        ],
        "reused_definitions": [
            {
                "path": _QS_SCHOLARLY,
                "scope": "planes.promotion_quarantine",
                "used_for_scope": ["input_isolation", "production_authority_lock"],
                "required_scope_annotations": {
                    "isolation_only": True,
                    "semantic_boundary": _QS_CONTRACT,
                },
            },
            {
                "path": _QS_SCHOLARLY,
                "scope": "rest_batch_asset_quarantine",
                "used_for_scope": ["input_isolation", "production_authority_lock"],
                "required_scope_annotations": {
                    "isolation_only": True,
                    "semantic_boundary": _QS_CONTRACT,
                },
            },
            {
                "path": _QS_GROBID,
                "scope": "defaults.*_quarantine",
                "used_for_scope": ["input_isolation", "safety_containment"],
                "required_patterns": [
                    "visual_heads_quarantine",
                    "feature_tagger_training_quarantine",
                    "import_eligible: false",
                    "auto_promote: forbidden",
                ],
            },
        ],
        "non_claim_invariants": [
            {
                "id": "quarantine_pass_is_not_a_quality_measurement",
                "statement": "A quarantine outcome is containment, not a quality measurement.",
            },
            {
                "id": "semantic_usefulness_requires_executed_evidence",
                "statement": "Usefulness routes to executed/evaluated population evidence.",
            },
        ],
    }

def _quarantine_semantic_boundary_fixtures() -> dict:
    """Fixture documents the boundary contract resolves against."""
    return {
        "scholarly": {
            "planes": {
                "promotion_quarantine": {
                    "promotion_state": "candidate",
                    "auto_promote": "forbidden",
                    "import_eligible": False,
                    "isolation_only": True,
                    "semantic_boundary": _QS_CONTRACT,
                }
            },
            "rest_batch_asset_quarantine": {
                "isolation_only": True,
                "semantic_boundary": _QS_CONTRACT,
                "source_bindings": [],
                "policy": {"import_eligible": False, "auto_promote": "forbidden"},
            },
        },
        "grobid": {
            "import_eligible": False,
            "auto_promote": "forbidden",
            "defaults": {
                "visual_heads_quarantine": {"tokens": ["A"], "boundaries": ["b"]},
                "body_accounting_quarantine": {"tokens": ["C"], "boundaries": ["d"]},
                "feature_tagger_training_quarantine": {"tokens": ["E"]},
                "fulltext_sequence_quarantine": {"tokens": ["F"]},
            },
        },
    }

def _write_quarantine_semantic_boundary(root: Path, contract: dict | None = None) -> None:
    """Write a self-contained GAR-012 fixture (contract, scopes, evidence, registry)."""
    _write(
        root / _QS_CONTRACT,
        yaml.safe_dump(contract or _quarantine_semantic_boundary_contract(), sort_keys=False),
    )
    fixtures = _quarantine_semantic_boundary_fixtures()
    _write(root / _QS_SCHOLARLY, yaml.safe_dump(fixtures["scholarly"], sort_keys=False))
    _write(root / _QS_GROBID, yaml.safe_dump(fixtures["grobid"], sort_keys=False))
    _write(
        root / _QS_EXECUTED,
        "policy_id: executed_population_binding\ncorpus_execution: true\n",
    )
    _write(root / _QS_LICENSE, "policy_id: component_license_provenance\n")
    _write(root / _QS_REGISTRY, json.dumps({"finding_id": "GAR-012"}) + "\n")

def _quarantine_semantic_boundary_check(root: Path) -> list[DriftFlag]:
    """Run the check through the trajectory service method (covers the wiring)."""
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = root
    return service._check_quarantine_semantic_boundary()

def _quarantine_semantic_boundary_flags(root: Path, contract: dict) -> set[str]:
    _write_quarantine_semantic_boundary(root, contract)
    return {flag.flag for flag in _quarantine_semantic_boundary_check(root)}

def _qs_rewrite_scope(root: Path, mutate) -> None:
    """Mutate the scholarly-plane fixture in place and rewrite it."""
    path = root / _QS_SCHOLARLY
    document = yaml.safe_load(path.read_text(encoding="utf-8"))
    mutate(document)
    _write(path, yaml.safe_dump(document, sort_keys=False))

def test_quarantine_semantic_boundary_contract_is_clean_on_real_repo():
    """The committed boundary must pass its own rule (a boundary that fails is dead DATA)."""
    assert _quarantine_semantic_boundary_check(_scripts_dir.parent) == []

def test_quarantine_semantic_boundary_fixture_contract_is_clean():
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    assert _quarantine_semantic_boundary_check(root) == []

def test_quarantine_semantic_boundary_absent_policy_is_silent():
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _make_root()
    assert service._check_quarantine_semantic_boundary() == []

def test_quarantine_semantic_boundary_invalid_yaml_is_flagged():
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    _write(root / _QS_CONTRACT, "claim_routing: [unclosed\n")
    flags = {flag.flag for flag in _quarantine_semantic_boundary_check(root)}
    assert "quarantine_semantic_boundary_invalid" in flags

def test_quarantine_semantic_boundary_non_mapping_contract_is_flagged():
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    _write(root / _QS_CONTRACT, "- establishes\n- does_not_establish\n")
    flags = {flag.flag for flag in _quarantine_semantic_boundary_check(root)}
    assert "quarantine_semantic_boundary_invalid" in flags

def test_quarantine_semantic_boundary_scope_missing_is_flagged():
    """A vanished reused quarantine definition is a missing containment scope."""
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    (root / _QS_SCHOLARLY).unlink()
    flags = {flag.flag for flag in _quarantine_semantic_boundary_check(root)}
    assert "quarantine_semantic_boundary_scope_missing" in flags

def test_quarantine_semantic_boundary_scope_annotation_missing_is_flagged():
    """A quarantine scope without `isolation_only` is no longer scoped to isolation."""
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    _qs_rewrite_scope(
        root,
        lambda doc: doc["planes"]["promotion_quarantine"].pop("isolation_only"),
    )
    flags = {flag.flag for flag in _quarantine_semantic_boundary_check(root)}
    assert "quarantine_semantic_boundary_scope_missing" in flags

def test_quarantine_semantic_boundary_scope_pointer_wrong_is_flagged():
    """A quarantine scope pointing at a rival boundary is not the canonical separation."""
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    _qs_rewrite_scope(
        root,
        lambda doc: doc["rest_batch_asset_quarantine"].update(
            {"semantic_boundary": "data/policies/other-boundary.yaml"}
        ),
    )
    flags = {flag.flag for flag in _quarantine_semantic_boundary_check(root)}
    assert "quarantine_semantic_boundary_scope_missing" in flags

def test_quarantine_semantic_boundary_claim_leak_is_flagged():
    """A quarantine definition that carries a utility key reads as a utility statement."""
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    path = root / _QS_GROBID
    document = yaml.safe_load(path.read_text(encoding="utf-8"))
    document["defaults"]["visual_heads_quarantine"]["utility"] = True
    _write(path, yaml.safe_dump(document, sort_keys=False))
    flags = {flag.flag for flag in _quarantine_semantic_boundary_check(root)}
    assert "quarantine_semantic_boundary_claim_leak" in flags

def test_quarantine_semantic_boundary_routing_missing_is_flagged():
    """Every semantic non-claim must route to evidence that can decide it."""
    contract = _quarantine_semantic_boundary_contract()
    del contract["claim_routing"]["output_quality"]
    flags = _quarantine_semantic_boundary_flags(_make_root(), contract)
    assert "quarantine_semantic_boundary_routing_missing" in flags

def test_quarantine_semantic_boundary_routing_forbidden_is_flagged():
    """A non-claim routed back to the quarantine scanner is the GAR-012 failure."""
    contract = _quarantine_semantic_boundary_contract()
    contract["claim_routing"]["semantic_utility"]["routed_to"] = "self"
    flags = _quarantine_semantic_boundary_flags(_make_root(), contract)
    assert "quarantine_semantic_boundary_routing_forbidden" in flags

def test_quarantine_semantic_boundary_evidence_missing_is_flagged():
    contract = _quarantine_semantic_boundary_contract()
    contract["claim_routing"]["legal_admissibility"]["path"] = "scripts/governor/checks/gone.yaml"
    flags = _quarantine_semantic_boundary_flags(_make_root(), contract)
    assert "quarantine_semantic_boundary_evidence_missing" in flags

def test_quarantine_semantic_boundary_rival_copy_is_flagged():
    """A second copy of the quarantine token/boundary lists is a rival definition home."""
    contract = _quarantine_semantic_boundary_contract()
    contract["tokens"] = ["leaked"]
    flags = _quarantine_semantic_boundary_flags(_make_root(), contract)
    assert "quarantine_semantic_boundary_rival_copy" in flags

def test_quarantine_semantic_boundary_finding_missing_is_flagged():
    contract = _quarantine_semantic_boundary_contract()
    contract["finding"] = "GAR-999"
    flags = _quarantine_semantic_boundary_flags(_make_root(), contract)
    assert "quarantine_semantic_boundary_finding_missing" in flags

def test_quarantine_semantic_boundary_every_mutation_is_not_silent():
    """Each violation class is isolated: the clean boundary is silent, every mutation reddens."""

    def routing_missing(contract: dict) -> None:
        del contract["claim_routing"]["output_quality"]

    def routing_forbidden(contract: dict) -> None:
        contract["claim_routing"]["semantic_utility"]["routed_to"] = "isolation_scan"

    def evidence_missing(contract: dict) -> None:
        contract["claim_routing"]["parity"]["path"] = "scripts/governor/checks/gone.yaml"

    def rival_copy(contract: dict) -> None:
        contract["boundaries"] = ["leaked"]

    def finding_gone(contract: dict) -> None:
        contract["finding"] = "GAR-999"

    def broken_role(contract: dict) -> None:
        contract["boundary_role"] = "quality_measurement"

    def overlap(contract: dict) -> None:
        contract["establishes"].append("semantic_utility")

    def unknown_route(contract: dict) -> None:
        contract["claim_routing"]["invented_claim"] = contract["claim_routing"]["parity"]

    cases = [
        ("routing", routing_missing, "quarantine_semantic_boundary_routing_missing"),
        ("forbidden", routing_forbidden, "quarantine_semantic_boundary_routing_forbidden"),
        ("evidence", evidence_missing, "quarantine_semantic_boundary_evidence_missing"),
        ("rival", rival_copy, "quarantine_semantic_boundary_rival_copy"),
        ("finding", finding_gone, "quarantine_semantic_boundary_finding_missing"),
        ("role", broken_role, "quarantine_semantic_boundary_broken"),
        ("overlap", overlap, "quarantine_semantic_boundary_broken"),
        ("route", unknown_route, "quarantine_semantic_boundary_broken"),
    ]
    root = _make_root()
    _write_quarantine_semantic_boundary(root)
    assert _quarantine_semantic_boundary_check(root) == []
    for label, mutate, expected in cases:
        contract = _quarantine_semantic_boundary_contract()
        mutate(contract)
        flags = _quarantine_semantic_boundary_flags(root, contract)
        assert flags, f"{label} mutation was silently accepted"
        assert expected in flags, f"{label} expected {expected}, got {sorted(flags)}"

def test_quarantine_semantic_boundary_is_wired_into_build_report():
    """A rule that is implemented but never invoked is a dead rule."""
    import inspect

    source = inspect.getsource(TrajectoryService.build_report)
    assert "_check_quarantine_semantic_boundary()" in source

def test_quarantine_semantic_boundary_summary_separates_relevance_from_isolation():
    """The summary keeps isolation claims and routed semantic claims apart."""
    from governor.adapters.checks.quarantine_semantic_boundary import (
        summarize_quarantine_semantic_boundary,
    )

    summary = summarize_quarantine_semantic_boundary(_scripts_dir.parent)
    assert summary["present"] is True
    assert summary["boundary_role"] == "isolation_only"
    assert summary["unrouted"] == []
    assert "relevance" not in summary["establishes"]
    assert summary["routing"]["semantic_utility"] == "executed_population_binding"
    assert summary["routing"]["legal_admissibility"] == "component_license_provenance"
    assert summary["reused_scopes"]

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
        test_promotion_evidence_contract_qualification_pass_on_clean_fixture,
        test_promotion_evidence_contract_qualification_positive_fixture_must_be_clean,
        test_promotion_evidence_contract_qualification_shared_digest_cannot_pass_as_clean,
        test_promotion_evidence_contract_qualification_weak_independence_cannot_pass_as_clean,
        test_promotion_evidence_contract_qualification_negative_fixture_must_trigger_labelled_disqualifiers,
        test_promotion_evidence_contract_qualification_unknown_contamination_is_flagged,
        test_promotion_evidence_contract_qualification_negative_fixture_must_cover_disqualifiers,
        test_promotion_evidence_contract_qualification_missing_fixture_is_flagged,
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
        test_boundary_positional_metrics_pass_on_clean_fixtures,
        test_boundary_positional_metrics_missing_fixture_is_flagged,
        test_boundary_positional_metrics_positive_fixture_must_be_clean,
        test_boundary_positional_metrics_negative_fixture_must_trigger_labelled_violations,
        test_boundary_positional_metrics_negative_fixture_must_cover_required_violations,
        test_boundary_positional_metrics_source_marker_missing_is_flagged,
        test_boundary_positional_metrics_binding_mismatch_is_flagged,
        test_boundary_positional_metrics_binding_missing_is_flagged,
        test_boundary_positional_metrics_invalid_yaml_is_flagged,
        test_boundary_positional_metrics_oracle_rejects_permutation_without_governor,
        test_boundary_positional_metrics_absent_snapshot_suite_is_not_flagged,
        test_visual_adapter_preserves_donor_content_pass_on_clean_fixtures,
        test_visual_adapter_preserves_donor_content_missing_fixture_is_flagged,
        test_visual_adapter_preserves_donor_content_positive_fixture_must_be_clean,
        test_visual_adapter_preserves_donor_content_negative_fixture_must_trigger_labelled_violations,
        test_visual_adapter_preserves_donor_content_negative_fixture_must_cover_required_violations,
        test_visual_adapter_preserves_donor_content_source_marker_missing_is_flagged,
        test_visual_adapter_preserves_donor_content_artifact_stale_is_flagged,
        test_visual_adapter_preserves_donor_content_artifact_missing_is_flagged,
        test_visual_adapter_preserves_donor_content_artifact_incomplete_is_flagged,
        test_visual_adapter_preserves_donor_content_binding_mismatch_is_flagged,
        test_visual_adapter_preserves_donor_content_binding_missing_is_flagged,
        test_visual_adapter_preserves_donor_content_invalid_yaml_is_flagged,
        test_visual_adapter_preserves_donor_content_oracle_rejects_discarded_primitives_without_governor,
        test_visual_adapter_preserves_donor_content_absent_snapshot_suite_is_not_flagged,
        test_component_license_provenance_pass_on_clean_fixtures,
        test_component_license_provenance_missing_fixture_is_flagged,
        test_component_license_provenance_positive_fixture_must_be_clean,
        test_component_license_provenance_negative_fixture_must_trigger_labelled_violations,
        test_component_license_provenance_negative_fixture_must_cover_required_violations,
        test_component_license_provenance_invalid_yaml_is_flagged,
        test_component_license_provenance_binding_mismatch_is_flagged,
        test_component_license_provenance_binding_missing_is_flagged,
        test_component_license_provenance_inventory_missing_is_flagged,
        test_component_license_provenance_surface_missing_is_flagged,
        test_component_license_provenance_surface_invalid_is_flagged,
        test_component_license_provenance_source_marker_missing_is_flagged,
        test_component_license_provenance_fixture_inventory_drift_is_flagged,
        test_component_license_provenance_absent_snapshot_suite_is_not_flagged,
        test_component_license_provenance_real_inventory_mutations_are_flagged,
        test_component_license_provenance_oracle_rejects_untyped_status_without_governor,
        test_conservation_provenance_pass_on_clean_fixtures,
        test_conservation_provenance_missing_contract_is_silent,
        test_conservation_provenance_missing_fixture_is_flagged,
        test_conservation_provenance_positive_fixture_must_be_clean,
        test_conservation_provenance_negative_fixture_must_trigger_labelled_violations,
        test_conservation_provenance_negative_fixture_must_cover_required_violations,
        test_conservation_provenance_missing_vocabulary_is_flagged,
        test_conservation_provenance_source_marker_missing_is_flagged,
        test_conservation_provenance_invalid_yaml_is_flagged,
        test_conservation_provenance_absent_snapshot_suite_is_not_flagged,
        test_conservation_provenance_oracle_rejects_historic_verdict_as_current,
        test_conservation_provenance_oracle_fails_closed_on_absent_and_unknown,
        test_conservation_provenance_oracle_bounds_provenance_identity,
        test_conservation_provenance_is_wired_into_build_report,
        test_executed_population_binding_pass_on_clean_fixtures,
        test_executed_population_binding_missing_contract_is_silent,
        test_executed_population_binding_missing_fixture_is_flagged,
        test_executed_population_binding_positive_fixture_must_be_clean,
        test_executed_population_binding_negative_fixture_must_trigger_labelled_violations,
        test_executed_population_binding_negative_fixture_must_cover_required_violations,
        test_executed_population_binding_missing_vocabulary_is_flagged,
        test_executed_population_binding_source_marker_missing_is_flagged,
        test_executed_population_binding_invalid_yaml_is_flagged,
        test_executed_population_binding_absent_snapshot_suite_is_not_flagged,
        test_executed_population_binding_oracle_rejects_absent_executed_population,
        test_executed_population_binding_oracle_rejects_large_reference_beside_small_execution,
        test_executed_population_binding_oracle_rejects_replayed_and_spliced_receipts,
        test_executed_population_binding_oracle_accepts_bound_receipt_and_bounds_identity,
        test_executed_population_binding_live_report_presented_as_executed_is_flagged,
        test_executed_population_binding_live_report_missing_is_flagged,
        test_executed_population_binding_undeclared_report_is_flagged,
        test_executed_population_binding_is_wired_into_build_report,
        test_oracle_digest_recomputation_pass_on_clean_fixtures,
        test_oracle_digest_recomputation_missing_contract_is_silent,
        test_oracle_digest_recomputation_missing_fixture_is_flagged,
        test_oracle_digest_recomputation_positive_fixture_must_be_clean,
        test_oracle_digest_recomputation_negative_fixture_must_trigger_labelled_violations,
        test_oracle_digest_recomputation_negative_fixture_must_cover_required_violations,
        test_oracle_digest_recomputation_invalid_yaml_is_flagged,
        test_oracle_digest_recomputation_binding_mismatch_is_flagged,
        test_oracle_digest_recomputation_binding_missing_is_flagged,
        test_oracle_digest_recomputation_surface_missing_is_flagged,
        test_oracle_digest_recomputation_surface_invalid_is_flagged,
        test_oracle_digest_recomputation_unclassified_digest_field_is_flagged,
        test_oracle_digest_recomputation_declared_field_not_found_is_flagged,
        test_oracle_digest_recomputation_adjudicated_field_reintroduced_is_flagged,
        test_oracle_digest_recomputation_opaque_used_as_canonical_source_is_flagged,
        test_oracle_digest_recomputation_bytes_pair_halves_are_flagged,
        test_oracle_digest_recomputation_duplicate_field_binding_is_flagged,
        test_oracle_digest_recomputation_fixture_field_drift_is_flagged,
        test_oracle_digest_recomputation_source_marker_missing_is_flagged,
        test_oracle_digest_recomputation_oracle_labels_are_reachable_without_governor,
        test_oracle_digest_recomputation_rule_body_muting_reddens_mutations,
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
        test_data_integrity_canonical_edge_contract_path_is_clean,
        test_data_integrity_canonical_edge_contract_orphan_is_flagged,
        test_data_integrity_canonical_edge_contract_legacy_reader_is_flagged,
        test_data_integrity_canonical_edge_contract_historical_text_is_allowed,
        test_data_integrity_canonical_edge_contract_rule_malformed_is_flagged,
        test_data_integrity_arxiv_doi_literal_in_production_rust_is_flagged,
        test_data_integrity_arxiv_doi_literal_in_test_rust_is_ignored,
        test_grobid_review_registry_pass_on_valid_registry,
        test_grobid_review_registry_missing_finding_is_flagged,
        test_grobid_review_registry_unresolved_todo_evidence_is_flagged,
        test_grobid_review_registry_missing_evidence_is_flagged,
        test_grobid_review_registry_resolved_without_closure_is_flagged,
        test_grobid_review_registry_invalid_json_is_flagged,
        test_grobid_review_registry_missing_contract_is_flagged,
        test_grobid_review_registry_malformed_contract_is_flagged,
        test_grobid_review_registry_non_mapping_contract_is_flagged,
        test_grobid_review_registry_cli_rejects_non_mapping_contract_cleanly,
        test_grobid_review_registry_wrong_item_type_is_flagged,
        test_grobid_review_registry_wrong_object_type_is_flagged,
        test_grobid_review_registry_enclosed_without_completion_evidence_is_flagged,
        test_grobid_review_registry_unknown_owner_is_flagged,
        test_grobid_review_registry_unknown_owner_is_flagged_even_when_enclosed,
        test_grobid_horizon_review_pass_on_pending_registry,
        test_grobid_horizon_review_dispatch_selects_horizon,
        test_grobid_horizon_review_missing_review_is_flagged,
        test_grobid_horizon_review_invalid_json_is_flagged,
        test_grobid_horizon_review_planning_pass_claim_is_flagged,
        test_grobid_horizon_review_claim_without_closure_is_flagged,
        test_grobid_horizon_review_missing_evidence_path_is_flagged,
        test_grobid_horizon_review_evidence_overlap_is_flagged,
        test_grobid_horizon_review_missing_adr_refs_is_flagged,
        test_grobid_horizon_review_missing_negative_tests_is_flagged,
        test_grobid_horizon_review_matrix_missing_reason_is_flagged,
        test_grobid_horizon_review_index_limitation_missing_is_flagged,
        test_grobid_horizon_review_stage_range_gap_is_flagged,
        test_grobid_horizon_review_unknown_status_is_flagged,
        test_grobid_horizon_review_unresolved_without_open_item_is_flagged,
        test_grobid_horizon_review_accepted_open_item_passes,
        test_grobid_horizon_review_unassigned_without_acceptance_is_flagged,
        test_grobid_horizon_review_complete_with_pending_verification_is_flagged,
        test_grobid_horizon_review_cli_accepts_contract_cleanly,
        test_grobid_horizon_review_cli_rejects_missing_review_cleanly,
        test_grobid_horizon_review_real_registry_is_valid,
        test_grobid_absorption_review_real_registry_closes_four_p0_findings,
        test_grobid_absorption_review_real_registry_closure_is_falsifiable,
        test_fsm_interaction_map_contract_is_clean_on_real_repo,
        test_fsm_interaction_map_fixture_contract_is_clean,
        test_fsm_interaction_map_absent_policy_is_silent,
        test_fsm_interaction_map_invalid_yaml_is_flagged,
        test_fsm_interaction_map_non_mapping_contract_is_flagged,
        test_fsm_interaction_map_unmapped_automaton_is_flagged,
        test_fsm_interaction_map_unknown_automaton_is_flagged,
        test_fsm_interaction_map_empty_invalidates_is_flagged,
        test_fsm_interaction_map_missing_trigger_class_is_flagged,
        test_fsm_interaction_map_controller_key_is_flagged,
        test_fsm_interaction_map_non_descriptive_role_is_flagged,
        test_fsm_interaction_map_production_authority_is_flagged,
        test_fsm_interaction_map_missing_automaton_sections_is_flagged,
        test_fsm_interaction_map_retry_without_rederivation_is_flagged,
        test_fsm_interaction_map_evidence_path_missing_is_flagged,
        test_fsm_interaction_map_evidence_drift_is_flagged,
        test_fsm_interaction_map_duplicate_pointer_map_is_flagged,
        test_fsm_interaction_map_pointer_wrong_canonical_is_flagged,
        test_fsm_interaction_map_finding_missing_from_registry_is_flagged,
        test_fsm_interaction_map_every_mutation_is_not_silent,
        test_fsm_interaction_map_is_wired_into_build_report,
        test_fsm_interaction_map_summary_covers_every_automaton,
        test_quarantine_semantic_boundary_contract_is_clean_on_real_repo,
        test_quarantine_semantic_boundary_fixture_contract_is_clean,
        test_quarantine_semantic_boundary_absent_policy_is_silent,
        test_quarantine_semantic_boundary_invalid_yaml_is_flagged,
        test_quarantine_semantic_boundary_non_mapping_contract_is_flagged,
        test_quarantine_semantic_boundary_scope_missing_is_flagged,
        test_quarantine_semantic_boundary_scope_annotation_missing_is_flagged,
        test_quarantine_semantic_boundary_scope_pointer_wrong_is_flagged,
        test_quarantine_semantic_boundary_claim_leak_is_flagged,
        test_quarantine_semantic_boundary_routing_missing_is_flagged,
        test_quarantine_semantic_boundary_routing_forbidden_is_flagged,
        test_quarantine_semantic_boundary_evidence_missing_is_flagged,
        test_quarantine_semantic_boundary_rival_copy_is_flagged,
        test_quarantine_semantic_boundary_finding_missing_is_flagged,
        test_quarantine_semantic_boundary_every_mutation_is_not_silent,
        test_quarantine_semantic_boundary_is_wired_into_build_report,
        test_quarantine_semantic_boundary_summary_separates_relevance_from_isolation,
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




def _write_grobid_review_registry(root: Path) -> Path:
    """Write a minimal valid GAR review registry into a temp root."""
    contract = root / "scripts/governor/checks/grobid_absorption_review.yaml"
    contract.parent.mkdir(parents=True, exist_ok=True)
    contract.write_text(
        "schema_version: grobid-review-registry.v1\n"
        "policy_id: grobid_review_registry\n"
        "dimension: scholarly_parse_plane\n"
        "severity: medium\n"
        "registry:\n"
        "  findings: doc/review/review.jsonl\n"
        "  finding_schema: doc/review/review.schema.json\n"
        '  id_pattern: "^GAR-[0-9]{3}$"\n'
        "  id_range: [GAR-001, GAR-002]\n"
        "  allow_status: [open, resolved, superseded]\n"
        "  unresolved_status: [open]\n"
        "  owner_field: assessment.owning_wave\n"
        "  owner_unassigned_value: unassigned\n",
        encoding="utf-8",
    )
    schema = root / "doc/review/review.schema.json"
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text(
        json.dumps(
            {
                "type": "object",
                "additionalProperties": False,
                "required": ["finding_id", "status", "dependencies", "evidence", "assessment"],
                "properties": {
                    "finding_id": {"pattern": "^GAR-[0-9]{3}$"},
                    "status": {"enum": ["open", "resolved", "superseded"]},
                    "dependencies": {"type": "array", "items": {"pattern": "^GAR-[0-9]{3}$"}, "uniqueItems": True},
                    "evidence": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "additionalProperties": False,
                            "required": ["path", "note"],
                            "properties": {"path": {}, "note": {"minLength": 3}},
                        },
                    },
                    "assessment": {
                        "type": "object",
                        "additionalProperties": False,
                        "required": ["owning_wave"],
                        "properties": {"owning_wave": {}},
                    },
                    "completion_evidence": {
                        "type": "object",
                        "required": ["mode", "ref", "result", "artifacts", "observed_at", "source_revision"],
                        "properties": {
                            "mode": {},
                            "ref": {},
                            "result": {},
                            "artifacts": {},
                            "observed_at": {},
                            "source_revision": {},
                        },
                    },
                },
            }
        ),
        encoding="utf-8",
    )
    evidence = root / "crates/da-cli/examples/document_segmentation_shadow.rs"
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.write_text("// fixture\n", encoding="utf-8")
    findings = root / "doc/review/review.jsonl"
    findings.write_text(
        json.dumps(
            {
                "finding_id": "GAR-001",
                "status": "open",
                "dependencies": [],
                "evidence": [{"path": "crates/da-cli/examples/document_segmentation_shadow.rs", "note": "real file"}],
                "assessment": {"owning_wave": "unassigned"},
            }
        )
        + "\n"
        + json.dumps(
            {
                "finding_id": "GAR-002",
                "status": "superseded",
                "dependencies": [],
                "evidence": [{"path": "doc/review/review.jsonl", "note": "self reference"}],
                "assessment": {"owning_wave": "unassigned"},
                "completion_evidence": {
                    "mode": "governor_rule",
                    "ref": "grobid_review_registry_broken",
                    "result": "pass",
                    "artifacts": ["doc/review/review.jsonl"],
                    "observed_at": "2026-09-10",
                    "source_revision": "test-fixture",
                },
            }
        )
        + "\n",
        encoding="utf-8",
    )
    return contract


def _review_flags(root: Path):
    from governor.adapters.checks.grobid_review_registry import check_grobid_review_registry

    return check_grobid_review_registry(root)


def test_grobid_review_registry_pass_on_valid_registry():
    root = _make_root()
    _write_grobid_review_registry(root)
    flags = _review_flags(root)
    allowed = {"grobid_review_registry_status", "grobid_review_registry_scope_warning"}
    assert not [f for f in flags if f.flag not in allowed], flags
    status = next(f for f in flags if f.flag == "grobid_review_registry_status")
    assert "records=2" in status.evidence
    assert "unresolved=1" in status.evidence
    assert "enclosed=1" in status.evidence


def test_grobid_review_registry_missing_finding_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    (root / "doc/review/review.jsonl").unlink()
    flags = _review_flags(root)
    assert any(f.flag == "grobid_review_registry_fixture_missing" for f in flags)


def test_grobid_review_registry_unresolved_todo_evidence_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    path = root / "doc/review/review.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    rows[0]["evidence"] = [{"path": "TODO: missing", "note": "todo"}]
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
    flags = _review_flags(root)
    assert any(
        f.flag == "grobid_review_registry_broken" and "TODO: missing" in f.evidence for f in flags
    )


def test_grobid_review_registry_missing_evidence_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    path = root / "doc/review/review.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    rows[0]["evidence"] = [{"path": "missing/file.rs", "note": "absent"}]
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
    flags = _review_flags(root)
    assert any(
        f.flag == "grobid_review_registry_broken" and "missing/file.rs" in f.evidence for f in flags
    )


def test_grobid_review_registry_resolved_without_closure_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    path = root / "doc/review/review.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    rows[1]["status"] = "resolved"
    rows[1]["evidence"] = [{"path": "missing/file.rs", "note": "absent"}]
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
    flags = _review_flags(root)
    # Missing evidence must fail even on enclosed history: closure needs proof, not prose.
    assert any(
        f.flag == "grobid_review_registry_broken" and "missing/file.rs" in f.evidence for f in flags
    )


def test_grobid_review_registry_invalid_json_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    (root / "doc/review/review.jsonl").write_text("{not json\n", encoding="utf-8")
    flags = _review_flags(root)
    assert any(f.flag == "grobid_review_registry_fixture_invalid" for f in flags)

def test_grobid_review_registry_missing_contract_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    (root / "scripts/governor/checks/grobid_absorption_review.yaml").unlink()
    flags = _review_flags(root)
    assert any(
        f.flag == "grobid_review_registry_fixture_missing" and "contract missing" in f.evidence
        for f in flags
    )


def test_grobid_review_registry_malformed_contract_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    (root / "scripts/governor/checks/grobid_absorption_review.yaml").write_text(
        "registry: [unbalanced\n", encoding="utf-8"
    )
    flags = _review_flags(root)
    assert any(f.flag == "grobid_review_registry_invalid" for f in flags)


def test_grobid_review_registry_wrong_item_type_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    path = root / "doc/review/review.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    rows[0]["dependencies"] = ["not-a-GAR-ID"]
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
    flags = _review_flags(root)
    assert any(f.flag == "grobid_review_registry_fixture_invalid" for f in flags)


def test_grobid_review_registry_non_mapping_contract_is_flagged():
    for payload in ("plain scalar\n", "- list item\n", "42\n"):
        root = _make_root()
        contract = _write_grobid_review_registry(root)
        contract.write_text(payload, encoding="utf-8")
        flags = _review_flags(root)
        assert any(f.flag == "grobid_review_registry_invalid" for f in flags), payload
        assert not any("AttributeError" in f.evidence for f in flags), flags


def test_grobid_review_registry_cli_rejects_non_mapping_contract_cleanly():
    root = _make_root()
    contract = _write_grobid_review_registry(root)
    contract.write_text("- list item\n", encoding="utf-8")
    scripts_dir = Path(__file__).resolve().parent.parent
    result = subprocess.run(
        [sys.executable, "-m", "governor", "review", "--root", str(root), "--registry", str(contract)],
        cwd=root,
        env={**os.environ, "PYTHONPATH": str(scripts_dir)},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "review registry: INVALID" in result.stderr
    assert "Traceback" not in result.stderr
    assert "AttributeError" not in result.stderr


def test_grobid_review_registry_wrong_object_type_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    path = root / "doc/review/review.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    rows[0]["assessment"] = ["not-an-object"]
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
    flags = _review_flags(root)
    assert any(f.flag == "grobid_review_registry_fixture_invalid" for f in flags)


def test_grobid_review_registry_unknown_owner_is_flagged_even_when_enclosed():
    root = _make_root()
    contract = _write_grobid_review_registry(root)
    text = contract.read_text(encoding="utf-8")
    text += "  owner_allow: [M502-2w7y8k/S01]\n"
    contract.write_text(text, encoding="utf-8")
    path = root / "doc/review/review.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    rows[0]["assessment"]["owning_wave"] = "M502-2w7y8k/S09"
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
    flags = _review_flags(root)
    assert any(
        f.flag == "grobid_review_registry_broken" and "unknown owner" in f.evidence for f in flags
    )


def test_grobid_review_registry_enclosed_without_completion_evidence_is_flagged():
    root = _make_root()
    _write_grobid_review_registry(root)
    path = root / "doc/review/review.jsonl"
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    rows[1]["status"] = "resolved"
    rows[1].pop("completion_evidence", None)
    # Evidence path itself exists, so only the missing closure proof must fail.
    rows[1]["evidence"] = [{"path": "doc/review/review.jsonl", "note": "existing file"}]
    path.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
    flags = _review_flags(root)
    assert any(
        f.flag == "grobid_review_registry_broken" and "closure proof" in f.evidence for f in flags
    )


def test_grobid_review_registry_unknown_owner_is_flagged():
    root = _make_root()
    contract = _write_grobid_review_registry(root)
    text = contract.read_text(encoding="utf-8")
    text += "  owner_allow: [M502-2w7y8k/S01]\n"
    contract.write_text(text, encoding="utf-8")
    flags = _review_flags(root)
    # Temp fixture owners are "unassigned", which is outside this allow-list.
    assert any(
        f.flag == "grobid_review_registry_broken" and "unknown owner" in f.evidence for f in flags
    )

def _horizon_contract(root: Path) -> Path:
    """Write the long-horizon review contract for a temp root and return its path."""
    contract = root / "scripts/governor/checks/grobid_rust_horizon_review.yaml"
    contract.parent.mkdir(parents=True, exist_ok=True)
    contract.write_text(
        "schema_version: grobid-horizon-review-registry.v1\n"
        "policy_id: grobid_horizon_review\n"
        "format: horizon-json-v1\n"
        "dimension: scholarly_parse_plane\n"
        "severity: medium\n"
        "registry:\n"
        "  review: doc/review/horizon.json\n"
        "  review_schema: doc/review/horizon.schema.json\n"
        '  stage_id_pattern: "^H[0-9]{2}$"\n'
        "  stage_id_range: [H01, H02]\n"
        "  allow_status: [pending, in_progress, blocked, complete, superseded]\n"
        "  unresolved_status: [pending, in_progress, blocked]\n"
        "  required_stage_fields: [stage_id, title, scope, status, depends_on, owner, adr_refs, "
        "implementation_evidence, verification_evidence, tested_source, index_freshness, negative_tests, "
        "test_matrix, implementation_pass, verification_pass]\n"
        '  contours: ["7 Architecture Invariant", "5 Adapter Integration"]\n'
        "  test_types: [Integration, E2E journey]\n"
        "  owner_field: owner\n"
        "  owner_unassigned_value: unassigned\n"
        "  owner_allow: [unassigned]\n"
        "  open_item_required_for_unresolved: true\n",
        encoding="utf-8",
    )
    return contract


def _horizon_schema(root: Path) -> Path:
    """Write a permissive top-level schema so code-level policy checks are exercised."""
    schema = root / "doc/review/horizon.schema.json"
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text(
        json.dumps(
            {
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": False,
                "required": [
                    "schema_version",
                    "review_id",
                    "policy_id",
                    "created_at",
                    "claim_scope",
                    "status_semantics",
                    "review_policy",
                    "stages",
                ],
                "properties": {
                    "schema_version": {"const": "grobid-rust-horizon-review.v1"},
                    "review_id": {"type": "string"},
                    "policy_id": {"const": "grobid_horizon_review"},
                    "created_at": {"type": "string"},
                    "claim_scope": {"enum": ["planning", "evidence"]},
                    "status_semantics": {"type": "string"},
                    "review_policy": {"type": "object"},
                    "stages": {"type": "array", "items": {"type": "object"}},
                },
            }
        ),
        encoding="utf-8",
    )
    return schema


def _horizon_stage(stage_id: str, **overrides) -> dict:
    stage = {
        "stage_id": stage_id,
        "title": f"{stage_id} fixture stage",
        "scope": f"{stage_id} fixture scope long enough for the contract",
        "status": "pending",
        "depends_on": [],
        "owner": "unassigned",
        "adr_refs": ["ADR-051"],
        "implementation_evidence": [
            {"path": "crates/da-parse/src/lib.rs", "note": "fixture anchor", "state": "present"}
        ],
        "verification_evidence": [
            {
                "mode": "command",
                "ref": "cargo test -p da-parse --lib",
                "expected": "fixture verification",
                "status": "pending",
            }
        ],
        "tested_source": {"revision": "git:fixture-revision", "verified": False, "note": "fixture"},
        "index_freshness": {
            "state": "stale",
            "limitation": "fixture index is not fresh for this isolated test root",
        },
        "negative_tests": [
            {"id": f"{stage_id}-N1", "scenario": "fixture negative scenario", "expected": "fixture negative expectation"}
        ],
        "test_matrix": [
            {
                "contour": "7 Architecture Invariant",
                "type": "E2E journey",
                "applicability": "applicable",
                "reason": "governor check",
            },
            {
                "contour": "5 Adapter Integration",
                "type": "Integration",
                "applicability": "not_applicable",
                "reason": "no adapter in the fixture",
            },
        ],
        "implementation_pass": False,
        "verification_pass": False,
        "open_item": {
            "reason": f"{stage_id} fixture stage stays explicitly open in this isolated root",
            "since": "2026-09-12",
            "accepted": True,
            "closure_condition": f"{stage_id} closes when the fixture condition is measured",
        },
    }
    stage.update(overrides)
    return stage


def _write_grobid_horizon_review(root: Path) -> Path:
    """Write a minimal valid long-horizon review registry into a temp root."""
    _horizon_contract(root)
    _horizon_schema(root)
    anchor = root / "crates/da-parse/src/lib.rs"
    anchor.parent.mkdir(parents=True, exist_ok=True)
    anchor.write_text("// fixture anchor\n", encoding="utf-8")
    review_path = root / "doc/review/horizon.json"
    document = {
        "schema_version": "grobid-rust-horizon-review.v1",
        "review_id": "REV-2026-09-12-fixture-horizon",
        "policy_id": "grobid_horizon_review",
        "created_at": "2026-09-12",
        "claim_scope": "planning",
        "status_semantics": "Fixture stages are unresolved planning records without acceptance authority.",
        "review_policy": {
            "no_claim_from_adr_only": True,
            "no_pass_without_verified_source": True,
            "implementation_and_verification_evidence_separate": True,
            "index_freshness_required_or_limited": True,
            "negative_tests_required": True,
            "matrix_applicability_requires_reason": True,
        },
        "stages": [_horizon_stage("H01"), _horizon_stage("H02", depends_on=["H01"])],
    }
    review_path.write_text(json.dumps(document, indent=2), encoding="utf-8")
    return review_path


def _horizon_flags(root: Path):
    from governor.adapters.checks.grobid_horizon_review import check_grobid_horizon_review

    return check_grobid_horizon_review(root)


def _load_horizon(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _save_horizon(path: Path, document: dict) -> None:
    path.write_text(json.dumps(document, indent=2), encoding="utf-8")


def _horizon_broken(flags) -> list:
    return [f for f in flags if f.flag == "grobid_horizon_review_broken"]


def test_grobid_horizon_review_pass_on_pending_registry():
    root = _make_root()
    _write_grobid_horizon_review(root)
    flags = _horizon_flags(root)
    allowed = {"grobid_horizon_review_status", "grobid_horizon_review_scope_warning"}
    assert not [f for f in flags if f.flag not in allowed], flags
    status = next(f for f in flags if f.flag == "grobid_horizon_review_status")
    assert "stages=2" in status.evidence
    assert "unresolved=2" in status.evidence
    assert "claims=0" in status.evidence


def test_grobid_horizon_review_dispatch_selects_horizon():
    from governor.adapters.checks.review_registry_dispatch import check_review_registry

    root = _make_root()
    _write_grobid_horizon_review(root)
    flags = check_review_registry(root, "scripts/governor/checks/grobid_rust_horizon_review.yaml")
    assert any(f.flag == "grobid_horizon_review_status" for f in flags), flags
    assert not _horizon_broken(flags), flags


def test_grobid_horizon_review_missing_review_is_flagged():
    root = _make_root()
    _write_grobid_horizon_review(root)
    (root / "doc/review/horizon.json").unlink()
    flags = _horizon_flags(root)
    assert any(f.flag == "grobid_horizon_review_fixture_missing" for f in flags)


def test_grobid_horizon_review_invalid_json_is_flagged():
    root = _make_root()
    _write_grobid_horizon_review(root)
    (root / "doc/review/horizon.json").write_text("{not json\n", encoding="utf-8")
    flags = _horizon_flags(root)
    assert any(f.flag == "grobid_horizon_review_fixture_invalid" for f in flags)


def test_grobid_horizon_review_planning_pass_claim_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["implementation_pass"] = True
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "claim_scope=planning" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_claim_without_closure_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["claim_scope"] = "evidence"
    stage = document["stages"][0]
    stage["status"] = "complete"
    stage["implementation_pass"] = True
    stage["verification_pass"] = True
    stage["tested_source"]["verified"] = True
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "closure without completion_evidence" in f.evidence
        for f in flags
    ), flags


def test_grobid_horizon_review_missing_evidence_path_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["implementation_evidence"] = [
        {"path": "missing/file.rs", "note": "absent", "state": "present"}
    ]
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "missing/file.rs" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_evidence_overlap_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["verification_evidence"] = [
        {
            "mode": "artifact",
            "ref": "crates/da-parse/src/lib.rs",
            "expected": "reuses the implementation anchor",
            "status": "pending",
        }
    ]
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "overlap" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_missing_adr_refs_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["adr_refs"] = []
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "at least one ADR ref" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_missing_negative_tests_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["negative_tests"] = []
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "negative tests" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_matrix_missing_reason_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["test_matrix"][0]["reason"] = ""
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "applicability reason" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_index_limitation_missing_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["index_freshness"] = {"state": "stale", "limitation": ""}
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "without a documented limitation" in f.evidence
        for f in flags
    ), flags


def test_grobid_horizon_review_stage_range_gap_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"] = [document["stages"][0]]
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "coverage mismatch" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_unknown_status_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][1]["status"] = "in_progress"
    _save_horizon(path, document)
    contract = root / "scripts/governor/checks/grobid_rust_horizon_review.yaml"
    text = contract.read_text(encoding="utf-8").replace(
        "  allow_status: [pending, in_progress, blocked, complete, superseded]\n",
        "  allow_status: [pending]\n",
    )
    contract.write_text(text, encoding="utf-8")
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "unknown status" in f.evidence for f in flags
    ), flags


def test_grobid_horizon_review_unresolved_without_open_item_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0].pop("open_item", None)
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "explicit open_item" in f.evidence for f in flags
    ), flags

def test_grobid_horizon_review_accepted_open_item_passes():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["owner"] = "unassigned"
    document["stages"][0]["open_item"] = {
        "reason": "fixture stage stays open pending a measured closure condition sign-off",
        "since": "2026-09-12",
        "accepted": True,
        "closure_condition": "fixture stage closes once the declared measurement is registered",
    }
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert not [
        f for f in _horizon_broken(flags) if "open_item" in f.evidence or "not accepted" in f.evidence
    ], flags

def test_grobid_horizon_review_unassigned_without_acceptance_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    document["stages"][0]["owner"] = "unassigned"
    document["stages"][0]["open_item"]["accepted"] = False
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "not accepted" in f.evidence for f in flags
    ), flags

def test_grobid_horizon_review_complete_with_pending_verification_is_flagged():
    root = _make_root()
    path = _write_grobid_horizon_review(root)
    document = _load_horizon(path)
    stage = document["stages"][0]
    stage["status"] = "complete"
    stage["implementation_pass"] = True
    stage["verification_pass"] = True
    stage["tested_source"]["verified"] = True
    stage["completion_evidence"] = {
        "mode": "command",
        "ref": "cargo test -p da-parse --lib",
        "result": "pass",
        "artifacts": ["crates/da-parse/src/lib.rs"],
        "observed_at": "2026-09-18",
        "source_revision": "fixture-revision-1",
    }
    stage["verification_evidence"][0]["status"] = "pending"
    _save_horizon(path, document)
    flags = _horizon_flags(root)
    assert any(
        f.flag == "grobid_horizon_review_broken" and "not pass" in f.evidence for f in flags
    ), flags

def test_grobid_horizon_review_cli_accepts_contract_cleanly():
    root = _make_root()
    _write_grobid_horizon_review(root)
    scripts_dir = Path(__file__).resolve().parent.parent
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "governor",
            "review",
            "--root",
            str(root),
            "--registry",
            "scripts/governor/checks/grobid_rust_horizon_review.yaml",
        ],
        cwd=root,
        env={**os.environ, "PYTHONPATH": str(scripts_dir)},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    assert "review registry: stages=2" in result.stdout
    assert "claim_scope=planning" not in result.stdout


def test_grobid_horizon_review_cli_rejects_missing_review_cleanly():
    root = _make_root()
    _write_grobid_horizon_review(root)
    (root / "doc/review/horizon.json").unlink()
    scripts_dir = Path(__file__).resolve().parent.parent
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "governor",
            "review",
            "--root",
            str(root),
            "--registry",
            "scripts/governor/checks/grobid_rust_horizon_review.yaml",
        ],
        cwd=root,
        env={**os.environ, "PYTHONPATH": str(scripts_dir)},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 2
    assert "review registry: INVALID" in result.stderr
    assert "Traceback" not in result.stderr


def test_grobid_horizon_review_real_registry_is_valid():
    from governor.adapters.checks.grobid_horizon_review import check_grobid_horizon_review

    repo_root = Path(__file__).resolve().parent.parent.parent
    flags = check_grobid_horizon_review(repo_root)
    blockers = [
        f
        for f in flags
        if f.flag.endswith(("_broken", "_invalid", "_missing"))
    ]
    assert blockers == [], blockers
    status = next(f for f in flags if f.flag == "grobid_horizon_review_status")
    assert "stages=13" in status.evidence


def test_grobid_absorption_review_real_registry_closes_four_p0_findings():
    """The four GAR P0 findings closed by this wave stay closed, with proof.

    Registry integrity alone never enforces closure, so this test discriminates
    on committed facts: the GAR counts (11 unresolved, 5 enclosed), the exact
    six completion_evidence keys (additionalProperties: false), artifact
    existence under the repository root, and the eleven findings that must stay
    open (GAR-002 is superseded history, not a contribution of this wave).
    """
    from governor.adapters.checks.grobid_review_registry import check_grobid_review_registry

    repo_root = Path(__file__).resolve().parent.parent.parent
    flags = check_grobid_review_registry(repo_root)
    blockers = [
        f
        for f in flags
        if f.flag.endswith(("_broken", "_invalid", "_missing"))
    ]
    assert blockers == [], blockers
    status = next(f for f in flags if f.flag == "grobid_review_registry_status")
    assert "records=16" in status.evidence, status.evidence
    assert "unresolved=11" in status.evidence, status.evidence
    assert "enclosed=5" in status.evidence, status.evidence
    assert "unassigned_unresolved=0" in status.evidence, status.evidence

    contract = yaml.safe_load(
        (repo_root / "scripts/governor/checks/grobid_absorption_review.yaml").read_text(
            encoding="utf-8"
        )
    )
    findings_rel = str(contract["registry"]["findings"])
    records = [
        json.loads(line)
        for line in (repo_root / findings_rel).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    by_id = {str(record["finding_id"]): record for record in records}

    closed_ids = ("GAR-001", "GAR-003", "GAR-004", "GAR-011")
    for finding_id in closed_ids:
        record = by_id[finding_id]
        assert record["status"] == "resolved", (finding_id, record["status"])
        completion = record["completion_evidence"]
        assert sorted(completion) == [
            "artifacts",
            "mode",
            "observed_at",
            "ref",
            "result",
            "source_revision",
        ], (finding_id, sorted(completion))
        assert completion["mode"] == record["verification"][0]["mode"], finding_id
        assert completion["result"] == "pass", finding_id
        assert str(completion["ref"]).strip(), finding_id
        assert str(completion["source_revision"]).strip(), finding_id
        assert re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", str(completion["observed_at"])), (
            finding_id,
            completion["observed_at"],
        )
        artifacts = completion["artifacts"]
        assert isinstance(artifacts, list) and artifacts, finding_id
        for artifact in artifacts:
            artifact = str(artifact)
            assert not artifact.startswith(("TODO", ".gsd/")), (finding_id, artifact)
            assert (repo_root / artifact).exists(), (finding_id, artifact)
        governor = record["governor"]
        assert governor["coverage"] in {"existing", "partial"}, (
            finding_id,
            governor["coverage"],
        )
        assert governor["existing_rules"], finding_id

    for number in range(1, 17):
        finding_id = f"GAR-{number:03d}"
        if finding_id in closed_ids:
            continue
        expected = "superseded" if finding_id == "GAR-002" else "open"
        assert by_id[finding_id]["status"] == expected, (
            finding_id,
            by_id[finding_id]["status"],
        )


def test_grobid_absorption_review_real_registry_closure_is_falsifiable():
    """Closure must be refuted by the adapter, not merely asserted by prose.

    T01 proves the four GAR P0 findings carry machine-checkable closure proof in
    the tracked registry, but a signature over an already-green state would look
    identical. This control is built to fail: it mutates a *copy* of the
    registry inside a symlink farm and then asserts the adapter - not the test -
    rejects the change. The farm is not decoration (MEM1504): every evidence and
    artifact path the oracle resolves must exist in the copy, so the real tree
    is linked rather than an empty skeleton, otherwise the control would measure
    the copier instead of the rule. The tracked JSONL is never written: its
    sha256 is compared byte-for-byte before and after.

    Three mutations, each on its own copy:
      A1 proof stripped (status stays resolved, completion_evidence removed)
         -> grobid_review_registry_broken "closure proof", severity high;
      A2 record reopened (status back to open, proof removed)
         -> counts move to unresolved=12 enclosed=4, so the copy is really the
            thing being read and closure is not frozen text;
      B  completion artifact points at a path that does not exist
         -> grobid_review_registry_broken "completion artifact missing".
    The unmodified copy is the positive control: zero *_broken/_invalid/_missing
    flags with unresolved=11 enclosed=5, so the farm itself is not the flag
    source.
    """
    from governor.adapters.checks.grobid_review_registry import check_grobid_review_registry

    repo_root = Path(__file__).resolve().parent.parent.parent
    findings_rel = "doc/review/grobid-absorption-review-2026-09-10.jsonl"
    schema_rel = "doc/review/grobid-absorption-review-2026-09-10.schema.json"
    tracked_path = repo_root / findings_rel
    tracked_before = hashlib.sha256(tracked_path.read_bytes()).hexdigest()

    def _gar_copy_root(mutate) -> tuple[tempfile.TemporaryDirectory, Path]:
        """Temp root: doc/review is a mutable copy, every other resolved path links the real tree."""
        tmp = tempfile.TemporaryDirectory(prefix="gar-closure-falsify-")
        root = Path(tmp.name)
        (root / "doc").mkdir(parents=True)
        for child in sorted((repo_root / "doc").iterdir()):
            if child.name == "review":
                continue
            (root / "doc" / child.name).symlink_to(
                child, target_is_directory=child.is_dir()
            )
        (root / "doc" / "review").mkdir()
        shutil.copy2(tracked_path, root / findings_rel)
        shutil.copy2(repo_root / schema_rel, root / schema_rel)
        for name in ("crates", "scripts", "data", "governor", "artifacts"):
            (root / name).symlink_to(repo_root / name, target_is_directory=True)
        records = [
            json.loads(line)
            for line in (root / findings_rel).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        by_id = {str(record["finding_id"]): record for record in records}
        mutate(by_id)
        (root / findings_rel).write_text(
            "".join(json.dumps(record, ensure_ascii=False) + "\n" for record in records),
            encoding="utf-8",
        )
        return tmp, root

    def _registry_flags(root: Path) -> tuple[list, object]:
        flags = check_grobid_review_registry(root)
        blockers = [f for f in flags if f.flag.endswith(("_broken", "_invalid", "_missing"))]
        status = next(f for f in flags if f.flag == "grobid_review_registry_status")
        return blockers, status

    def _strip_completion(by_id):
        by_id["GAR-001"].pop("completion_evidence", None)

    def _reopen_closed(by_id):
        by_id["GAR-001"].pop("completion_evidence", None)
        by_id["GAR-001"]["status"] = "open"

    def _break_artifact(by_id):
        by_id["GAR-011"]["completion_evidence"]["artifacts"] = [
            "artifacts/parse_compat/__s05_missing_artifact__.json"
        ]

    # Positive control: an unmodified copy is clean, so the farm is not the flag source.
    tmp, root = _gar_copy_root(lambda by_id: None)
    try:
        blockers, status = _registry_flags(root)
        assert blockers == [], blockers
        assert "records=16" in status.evidence and "unresolved=11" in status.evidence
        assert "enclosed=5" in status.evidence, status.evidence
        assert "unassigned_unresolved=0" in status.evidence, status.evidence
    finally:
        tmp.cleanup()

    # A1: a closed record that loses its proof is rejected - closure cannot be claimed by text.
    tmp, root = _gar_copy_root(_strip_completion)
    try:
        blockers, status = _registry_flags(root)
        closure = [f for f in blockers if "closure proof" in f.evidence]
        assert len(closure) == 1, blockers
        assert "GAR-001" in closure[0].evidence, closure[0].evidence
        assert closure[0].severity == "high", closure[0].severity
        assert "unresolved=11" in status.evidence, status.evidence
        assert "enclosed=5" in status.evidence, status.evidence
    finally:
        tmp.cleanup()

    # A2: reopening moves the observable counts, proving the copy is what the oracle reads.
    tmp, root = _gar_copy_root(_reopen_closed)
    try:
        blockers, status = _registry_flags(root)
        assert blockers == [], blockers
        assert "unresolved=12" in status.evidence, status.evidence
        assert "enclosed=4" in status.evidence, status.evidence
    finally:
        tmp.cleanup()

    # B: a completion artifact that does not exist fails closed.
    tmp, root = _gar_copy_root(_break_artifact)
    try:
        blockers, status = _registry_flags(root)
        missing = [f for f in blockers if "completion artifact missing" in f.evidence]
        assert len(missing) == 1, blockers
        assert "GAR-011" in missing[0].evidence, missing[0].evidence
        assert missing[0].severity == "high", missing[0].severity
        assert "unresolved=11" in status.evidence, status.evidence
        assert "enclosed=5" in status.evidence, status.evidence
    finally:
        tmp.cleanup()

    # The tracked registry is evidence, not a scratch pad: it must be byte-identical.
    assert hashlib.sha256(tracked_path.read_bytes()).hexdigest() == tracked_before


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


def test_exclude_cfg_test_regions_keeps_production_matches():
    """D127 guard: #[cfg(test)] fail-closed tests are excluded, production is not.

    The rule's rationale promises that fail-closed unit tests assigning a locked
    value to prove rejection are not treated as production violations. This test
    pins BOTH directions: the exclusion applies only inside `#[cfg(test)]` items,
    so a production assignment is still reported.
    """
    root = _make_root()
    src = root / "crates" / "da-application" / "src" / "policy.rs"
    _write(
        src,
        "\n".join(
            [
                "pub fn shipped() -> bool {",
                "    let leaked = manifest();",
                "    leaked.import_eligible = true;",
                "    leaked.import_eligible",
                "}",
                "",
                "#[cfg(test)]",
                "mod tests {",
                "    #[test]",
                "    fn rejects_unlocked_variant() {",
                "        let mut leaked = variant();",
                "        leaked.import_eligible = true;",
                "        assert!(evaluate(&leaked).is_err());",
                "    }",
                "}",
            ]
        ),
    )
    scanner = FilesystemCodeScanner(root)
    pattern = r"import_eligible\s*[:=]\s*true"

    plain = list(scanner.scan_pattern("crates", pattern, suffix_allowlist={".rs"}))
    guarded = list(
        scanner.scan_pattern(
            "crates",
            pattern,
            suffix_allowlist={".rs"},
            exclude_cfg_test_regions=True,
        )
    )

    assert {line for _, line, _ in plain} == {3, 12}, "baseline must see both matches"
    assert {line for _, line, _ in guarded} == {3}, "only the cfg(test) match is excluded"
