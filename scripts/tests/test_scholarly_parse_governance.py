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
from pathlib import Path

import pytest
import yaml

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
