"""S19/T01 tests for the DATA-bound scholarly parse governance contract.

These tests intentionally exercise only the tracked policy and fixtures. The
trajectory checker is wired in T02; here we prove that the binding contract is
closed, source-bound, fail-closed, and reuses promotion vocabulary rather than
reimplementing its FSM.
"""

from __future__ import annotations

import copy
import hashlib
import json
import shutil
from pathlib import Path

import pytest
import yaml

from governor.adapters.checks.scholarly_parse_plane import _check_scholarly_parse_plane
from governor.application.trajectory_service import TrajectoryService

ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = ROOT / "scripts/governor/checks/scholarly_parse_plane.yaml"
POSITIVE_PATH = ROOT / "governor/fixtures/scholarly_parse_plane/contract.positive.json"
NEGATIVE_PATH = ROOT / "governor/fixtures/scholarly_parse_plane/violations.negative.json"

REQUIRED_PLANES = {
    "dependency_boundary",
    "persistence_authority",
    "configuration_ownership",
    "cache_identity",
    "promotion_quarantine",
}
REQUIRED_VIOLATIONS = {
    "forbidden_dependency",
    "authoritative_redb",
    "memory_only_rvf",
    "hardcoded_physical_default",
    "cache_fingerprint_collision",
    "missing_rollback",
    "auto_promote",
    "deployed",
    "promotion_receipt",
    "production_route",
    "graph_write",
    "import_leak",
    "schema_vocabulary_drift",
    "missing_integration_evidence",
    "acceptance_evidence_missing",
    "acceptance_evidence_substitution",
    "acceptance_role_drift",
    "acceptance_same_n_drift",
    "acceptance_lock_erosion",
    "ledger_surface",
    "backend_identity",
    "policy_stamp",
    "silent_loss",
    "lexical_visual_mix",
    "region_coverage",
    "header_sequence",
    "invented_author",
}


def _load(path: Path):
    return json.loads(path.read_text(encoding="utf-8")) if path.suffix == ".json" else yaml.safe_load(path.read_text(encoding="utf-8"))


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _source_bindings(policy: dict) -> list[dict]:
    return list(policy["bindings"]["source_data"]) + list(policy["bindings"]["canonical_artifacts"])


def _validate_positive(policy: dict, fixture: dict, root: Path = ROOT) -> list[str]:
    errors: list[str] = []
    if not policy.get("schema_version"):
        errors.append("schema_version")
    elif fixture.get("schema_version") != policy["schema_version"]:
        errors.append("schema_version")
    if fixture.get("policy_id") != policy["policy_id"]:
        errors.append("policy_id")
    if set(fixture.get("planes", {})) != REQUIRED_PLANES:
        errors.append("plane coverage")
    for binding in _source_bindings(policy):
        path = root / binding["path"]
        matching = next((item for item in fixture.get("source_data", []) + fixture.get("canonical_artifacts", []) if item.get("path") == binding["path"]), None)
        if matching is None:
            errors.append(f"missing binding:{binding['path']}")
        elif not path.is_file() or matching.get("sha256") != binding["sha256"] or _sha256(path) != matching.get("sha256"):
            errors.append(f"substituted source:{binding['path']}")
    if fixture.get("rollback", {}).get("target") != policy["bindings"]["rollback_target"]:
        errors.append("rollback")
    if fixture.get("promotion", {}).get("state") != "candidate":
        errors.append("promotion state")
    # Only quarantine locks belong in the positive promotion envelope;
    # raw_payload/physical_path are redaction locks, not envelope fields.
    for field, expected in policy["locks"].items():
        if field not in {"import_eligible", "production_route", "graph_write", "auto_promote", "deployed", "promotion_receipt"}:
            continue
        observed_field = "receipt" if field == "promotion_receipt" else field
        if fixture.get("promotion", {}).get(observed_field) != expected:
            errors.append(f"lock:{field}")
    evidence = fixture.get("integration_evidence", [])
    required_evidence = {(item["path"], item["identity"]) for item in policy["bindings"]["integration_evidence"]}
    observed_evidence = {(item.get("path"), item.get("identity")) for item in evidence}
    if not required_evidence.issubset(observed_evidence):
        errors.append("integration evidence")
    return errors


def _validate_negative(policy: dict, records: list[dict]) -> dict[str, list[str]]:
    """Return fixture-shape errors keyed by labelled mutation."""
    errors: dict[str, list[str]] = {}
    expected = set(policy["fixtures"]["negative"]["expect"]["require_violations"])
    observed = [record.get("violation") for record in records]
    for label in expected - set(observed):
        errors.setdefault("coverage", []).append(label)
    if len(observed) != len(set(observed)):
        errors.setdefault("coverage", []).append("duplicate_label")
    for record in records:
        label = record.get("violation")
        if label not in expected:
            errors.setdefault(label or "malformed", []).append("unknown label")
        if record.get("severity") != policy["fixtures"]["negative"]["expect"]["severity"]:
            errors.setdefault(label, []).append("severity")
        if not record.get("plane") or not isinstance(record.get("record"), dict):
            errors.setdefault(label or "malformed", []).append("typed record")
    return errors


def test_fixtures_clean_positive_fixture_is_source_bound_and_closed():
    policy = _load(POLICY_PATH)
    fixture = _load(POSITIVE_PATH)
    assert _validate_positive(policy, fixture) == []
    assert policy["bindings"]["shared_promotion_contract"] == "scripts/governor/checks/promotion_evidence_contract.yaml"
    assert set(policy["closed_taxonomy"]["dispositions"]) == {"passed", "fallback", "rejected", "unsupported", "not_observed", "invalid_evidence", "stale"}


@pytest.mark.parametrize("label", sorted(REQUIRED_VIOLATIONS))
def test_fixtures_each_negative_mutation_has_one_isolated_typed_high_severity_result(label):
    policy = _load(POLICY_PATH)
    records = _load(NEGATIVE_PATH)
    match = [record for record in records if record.get("violation") == label]
    assert len(match) == 1
    record = match[0]
    assert record["severity"] == "high"
    assert record["plane"] in {"dependency_boundary", "persistence_authority", "configuration_ownership", "cache_identity", "promotion_quarantine", "taxonomy", "integration"}
    assert isinstance(record["record"], dict)
    assert set(record["record"]) == set(record["record"]), "mutation record must remain structurally typed"


def test_fixtures_negative_has_complete_nonduplicated_coverage_and_no_promotion():
    policy = _load(POLICY_PATH)
    records = _load(NEGATIVE_PATH)
    assert _validate_negative(policy, records) == {}
    assert policy["fixtures"]["negative"]["expect"]["no_promotion"] is True
    assert all(record.get("violation") not in {"promoted", "deployed_route"} for record in records)


@pytest.mark.parametrize("mutation", ["missing_policy", "malformed_positive", "eroded_plane", "eroded_violation"])
def test_fixtures_missing_or_malformed_contract_parts_fail_closed(mutation):
    policy = _load(POLICY_PATH)
    fixture = _load(POSITIVE_PATH)
    records = _load(NEGATIVE_PATH)
    if mutation == "missing_policy":
        broken = copy.deepcopy(policy)
        broken.pop("schema_version")
        assert "schema_version" in _validate_positive(broken, fixture)
    elif mutation == "malformed_positive":
        broken = copy.deepcopy(fixture)
        broken["promotion"] = None
        with pytest.raises(AttributeError):
            _validate_positive(policy, broken)
    elif mutation == "eroded_plane":
        broken = copy.deepcopy(fixture)
        broken["planes"].pop("cache_identity")
        assert "plane coverage" in _validate_positive(policy, broken)
    else:
        broken = [record for record in records if record["violation"] != "missing_rollback"]
        errors = _validate_negative(policy, broken)
        assert "missing_rollback" in errors["coverage"]


def test_fixtures_source_and_artifact_substitution_is_detected(tmp_path: Path):
    policy = _load(POLICY_PATH)
    fixture = _load(POSITIVE_PATH)
    substituted = tmp_path / "substituted.yaml"
    substituted.write_text("not the bound DATA\n", encoding="utf-8")
    broken = copy.deepcopy(fixture)
    binding = broken["source_data"][0]
    binding["path"] = str(substituted.relative_to(tmp_path))
    binding["sha256"] = _sha256(substituted)
    # The policy still names the real path; changing the fixture identity must
    # be rejected as a substitution, not accepted as self-attestation.
    assert any("missing binding" in error or "substituted source" in error for error in _validate_positive(policy, broken, tmp_path))


def test_fixtures_negative_schema_is_yaml_policy_compatible():
    policy = _load(POLICY_PATH)
    negative_spec = policy["fixtures"]["negative"]["expect"]
    assert negative_spec["each_label_isolated"] is True
    assert set(negative_spec["require_violations"]) == REQUIRED_VIOLATIONS
    assert policy["redaction"]["forbidden"]


def _copy_checker_surface(destination: Path) -> None:
    """Build a tracked-only synthetic root for report-level mutations."""
    policy = _load(POLICY_PATH)
    paths = {
        "scripts/governor/checks/scholarly_parse_plane.yaml",
        "scripts/governor/checks/promotion_evidence_contract.yaml",
        str(policy["fixtures"]["positive"]["path"]),
        str(policy["fixtures"]["negative"]["path"]),
    }
    for binding in _source_bindings(policy):
        paths.add(binding["path"])
    for relative in paths:
        source = ROOT / relative
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)


def test_canonical_shadow_report_uses_closed_snake_case_taxonomy():
    report = _load(ROOT / "artifacts/parse_compat/native_fulltext_composition_shadow.json")
    dispositions = {
        item["disposition"]
        for outcome in report["outcomes"]
        for item in outcome["dispositions"]
    }
    components = {
        item["component"]
        for outcome in report["outcomes"]
        for item in outcome["dispositions"]
    }
    assert dispositions <= set(_load(ROOT / "data/parse/native_fulltext_composition.yaml")["dispositions"])
    assert components == set(_load(ROOT / "data/parse/native_fulltext_composition.yaml")["component_ids"])
    assert not dispositions & {"invalidevidence", "notobserved"}
    assert not components & {"documentsegmentation", "learningpolicy"}


def test_taxonomy_mutation_is_rejected_by_exact_wire_assertion():
    report = _load(ROOT / "artifacts/parse_compat/native_fulltext_composition_shadow.json")
    mutated = copy.deepcopy(report)
    mutated["outcomes"][0]["dispositions"][0]["component"] = "documentsegmentation"
    allowed = set(_load(ROOT / "data/parse/native_fulltext_composition.yaml")["component_ids"])
    observed = {
        item["component"]
        for outcome in mutated["outcomes"]
        for item in outcome["dispositions"]
    }
    assert not observed <= allowed


def test_checker_clean_root_has_no_s19_flags():
    assert _check_scholarly_parse_plane(ROOT) == []


@pytest.mark.parametrize("label", sorted(REQUIRED_VIOLATIONS))
def test_checker_localizes_each_synthetic_mutation_to_high_severity_flag(tmp_path: Path, label: str):
    _copy_checker_surface(tmp_path)
    negative_path = tmp_path / NEGATIVE_PATH.relative_to(ROOT)
    records = _load(negative_path)
    record = next(item for item in records if item["violation"] == label)
    clean_payloads = {
        "missing_rollback": {"rollback": {"target": "java-grobid-http@0.9.0-crf"}},
        "missing_integration_evidence": {"integration_evidence": [{"path": "x", "identity": "y"}]},
        "schema_vocabulary_drift": {"disposition": "passed"},
    }
    record["record"] = clean_payloads.get(label, {})
    # Mutate the typed record so the labelled violation no longer fires; the
    # checker must report that exact code, not a generic repository scan hit.
    negative_path.write_text(json.dumps(records), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == f"scholarly_parse_plane_{label}"]
    assert matching
    assert all(item.severity == "high" for item in matching)
    assert all(f"plane=" in item.evidence and "requirements=" in item.evidence for item in matching)


def test_checker_registration_is_present_for_active_and_closeout_reports():
    source = (ROOT / "scripts/governor/application/trajectory_service.py").read_text(encoding="utf-8")
    assert "drift_flags.extend(self._check_scholarly_parse_plane())" in source
    build_report = source[source.index("    def build_report"):source.index("    def _build_dimensions")]
    assert "def build_report(self, phase: str = \"preflight\")" in build_report
    assert "phase=phase" in build_report
    assert "_check_scholarly_parse_plane" in build_report


def test_checker_does_not_replace_independent_promotion_contract():
    source = (ROOT / "scripts/governor/application/trajectory_service.py").read_text(encoding="utf-8")
    assert "drift_flags.extend(self._check_promotion_evidence_contract())" in source
    assert "promotion_evidence_contract.yaml" in source
    # S19 consumes the shared contract path; it does not define Promotion FSM
    # transitions or copy the promotion validator into the parse-plane adapter.
    checker_source = (ROOT / "scripts/governor/adapters/checks/scholarly_parse_plane.py").read_text(encoding="utf-8")
    assert "shared_promotion_contract" in checker_source
    assert "PromotionDecision" not in checker_source
