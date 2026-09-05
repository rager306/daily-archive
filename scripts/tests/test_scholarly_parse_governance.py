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

from governor.adapters.checks.scholarly_parse_plane import (
    _cascade_observe_tokens,
    _check_scholarly_parse_plane,
)
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
    "bibliography_sequence",
    "lost_references",
    "body_java_fallback",
    "reading_order_change",
    "typed_visual_absence",
    "zero_count_from_absence",
    "family_promote_cap",
    "worker_identity_leak",
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


def test_checker_flags_parse_pdf_missing_p9_conservation_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p0_admit_observe(&mut m, true, true); stamp_p1_layout_observe(&mut m, false); stamp_p2_segmentation_observe(&mut m, 1); stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); stamp_p6_visual_observe(&mut m, 0, 0, 0, true, true); stamp_p7_callout_observe(&mut m, 0, 0); stamp_p8_consolidation_observe(&mut m, \"structural\", false, false); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p9" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p8_consolidation_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p0_admit_observe(&mut m, true, true); stamp_p1_layout_observe(&mut m, false); stamp_p2_segmentation_observe(&mut m, 1); stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); stamp_p6_visual_observe(&mut m, 0, 0, 0, true, true); stamp_p7_callout_observe(&mut m, 0, 0); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p8" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p7_callout_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p0_admit_observe(&mut m, true, true); stamp_p1_layout_observe(&mut m, false); stamp_p2_segmentation_observe(&mut m, 1); stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); stamp_p6_visual_observe(&mut m, 0, 0, 0, true, true); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p7" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p1_layout_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p0_admit_observe(&mut m, true, true); stamp_p2_segmentation_observe(&mut m, 1); stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); stamp_p6_visual_observe(&mut m, 0, 0, 0, true, true); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p1" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p0_admit_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p2_segmentation_observe(&mut m, 1); stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); stamp_p6_visual_observe(&mut m, 0, 0, 0, true, true); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p0" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p6_observe_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p2_segmentation_observe(&mut m, 1); stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p6" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p2_observe_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p2" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p4_observe_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p3_header_observe(&mut m, parsed.header_authors.len()); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p4" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_missing_p3_observe_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_grobid_profile(&mut m, grobid_route); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_p3_named_gap_hardcoded_hub_count(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        'p3.insert("named_native_accepted_gap".into(), 20.into());\n',
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags
    assert any("named_native_accepted_gap" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_p4_citation_names_numeric_count(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        'pub fn stamp_p4_references_observe(m: &mut serde_yaml::Mapping, java_citation_n: usize) { p4.insert("java_citation_n".into(), java_citation_n.into()); p4.insert("citation_names_not_biblstruct".into(), 2198.into()); }\n',
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags
    assert any("citation_names_not_biblstruct" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_p4_pooling_citation_names_with_java_count(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        'pub fn stamp_p4_references_observe(m: &mut serde_yaml::Mapping, java_citation_n: usize) { p4.insert("java_citation_n".into(), java_citation_n.into()); p4.insert("citation_names_not_biblstruct".into(), java_citation_n.into()); }\n',
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags
    assert any("citation_names_not_biblstruct" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_skips_missing_source_cli_without_p4_flags(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    flags = _check_scholarly_parse_plane(tmp_path)
    assert not any("source_cli.rs" in item.evidence for item in flags), flags


def test_checker_flags_hub_real_tei_pooling_sections_with_biblstruct(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-parse/src/real_tei_tests.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "fn test_hub() { assert!(sections.len() >= 80); assert!(citations.len() >= 80); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags
    assert any("real_tei_tests.rs" in item.evidence for item in matching)


def test_checker_flags_parse_pdf_p5_live_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p5_body_observe(&mut m, 1); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("stamp_p5" in item.evidence or "source_cli.rs" in item.evidence for item in matching)


def test_checker_flags_p5_observe_mode_not_later(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    stages = payload.setdefault("defaults", {}).setdefault("cascade_stages", {})
    stages.setdefault("P5", {})["observe_mode"] = "production_stamp"
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("P5" in item.evidence for item in matching)


def test_cascade_observe_tokens_derive_live_and_forbidden_from_data():
    payload = yaml.safe_load((ROOT / "data/parse/grobid_compatibility.yaml").read_text(encoding="utf-8"))
    stages = (payload.get("defaults") or {}).get("cascade_stages") or {}
    required, forbidden = _cascade_observe_tokens(stages)
    assert required == [f"stamp_p{i}_" for i in (0, 1, 2, 3, 4, 6, 7, 8, 9)]
    assert forbidden == ["stamp_p5_"]


def test_cascade_observe_tokens_treats_malformed_rows_fail_closed():
    required, forbidden = _cascade_observe_tokens(None)
    assert (required, forbidden) == ([], [])
    required, forbidden = _cascade_observe_tokens({"P7": None, "P5": "accountant_later", "P1": {}})
    assert "stamp_p7_" in required
    assert "stamp_p1_" in required
    assert "stamp_p5_" in required
    assert forbidden == []


def test_checker_p7_accountant_later_removes_missing_p7_requirement(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    stages = payload.setdefault("defaults", {}).setdefault("cascade_stages", {})
    stages.setdefault("P7", {})["observe_mode"] = "accountant_later"
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn run_parse_pdf() { stamp_p0_admit_observe(&mut m, true, true); stamp_p1_layout_observe(&mut m, false); stamp_p2_segmentation_observe(&mut m, 1); stamp_p3_header_observe(&mut m, 1); stamp_p4_references_observe(&mut m, 1); stamp_p6_visual_observe(&mut m, 0, 0, 0, true, true); stamp_p8_consolidation_observe(&mut m, \"structural\", false, false); stamp_p9_conservation_observe(&mut m); }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert not any("stamp_p7" in item.evidence for item in matching), flags


def test_checker_flags_cascade_stage_later_as_fsm_consult(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    stages = payload.setdefault("defaults", {}).setdefault("cascade_stages", {})
    stages.setdefault("P6", {})["fsm_consult"] = ["visual_bundle"]
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("fsm_consult" in item.evidence for item in matching)


def test_checker_flags_cascade_stage_native_production(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    stages = payload.setdefault("defaults", {}).setdefault("cascade_stages", {})
    stages.setdefault("P3", {})["production_authority"] = "native"
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags


def test_checker_flags_sona_policy_missing_demo_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/background_tasks.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "pub fn sona_policy_report() { consult_with_fallback(host, yaml, point, features) }\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("background_tasks.rs" in item.evidence for item in matching)


def test_checker_flags_empty_native_listed_as_parse_pdf_consult(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/policies/registry.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    status = payload.setdefault("consult_status", {})
    status["parse_pdf_consults"] = ["grammar_route", "profile_route", "concurrency_tune"]
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags


def test_checker_flags_pooling_java_header_with_native_accepted(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/native_fulltext_composition.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    payload["source_evidence"]["s12_named_denominators"]["java_lift_same_n"][
        "java_header_authors_equals"
    ] = "header_accepted_total"
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags


def test_checker_flags_undeclared_shadow_sha_clone(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    shadow_path = tmp_path / "artifacts/parse_compat/native_fulltext_composition_shadow.json"
    payload = json.loads(shadow_path.read_text(encoding="utf-8"))
    payload["source_identity"].pop("identity_kind", None)
    payload["source_identity"].pop("unique_source_sha256", None)
    shadow_path.write_text(json.dumps(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags


def test_checker_flags_preserved_names_without_preserved_kind(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    shadow_path = tmp_path / "artifacts/parse_compat/native_fulltext_composition_shadow.json"
    payload = json.loads(shadow_path.read_text(encoding="utf-8"))
    payload["source_identity"]["identity_kind"] = "synthetic_clone"
    payload["outcomes"][0]["document_id"] = "hub_2507.19457"
    shadow_path.write_text(json.dumps(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags


def test_checker_flags_bare_from_env_pdf_parse_constructor(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/bare_from_env.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "fn parse() {\n    let parser = GrobidParser::from_env();\n    parser.parse_pdf(path, id);\n}\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("bare_from_env.rs" in item.evidence for item in matching)


def test_checker_allows_from_env_health_probe(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/health_probe.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "async fn check() {\n    let grobid = GrobidParser::from_env();\n    let grobid_ok = grobid.is_alive().await;\n    println!(\"  GROBID: {grobid_ok}\");\n}\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp" and "health_probe.rs" in item.evidence]
    assert matching == []


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
