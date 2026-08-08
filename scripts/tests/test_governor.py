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
