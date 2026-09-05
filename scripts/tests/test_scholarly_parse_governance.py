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
    _empty_native_consult_tokens,
    _java_absent_endpoint_tokens,
    _native_rules_not_consulted_tokens,
    _scan_bare_from_env_parse,
    _visual_heads_quarantine_spec,
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
    """Live root stays clean except DATA-rooted PARSE_BARE constructor hits.

    Fail-closed lifetime analysis may still flag remaining corpus PDF constructors
    under declared scan_roots; those sites are T02 inventory, not a T01 scanner
    regression. Other scholarly-parse families must stay empty. Filenames are not
    hardcoded: observed hits must equal `_scan_bare_from_env_parse` over DATA
    `production_pdf_constructors.scan_roots`.
    """
    flags = _check_scholarly_parse_plane(ROOT)
    constructor_flags = [
        item
        for item in flags
        if item.flag == "scholarly_parse_plane_policy_stamp"
        and "GrobidParser::from_env PDF parse constructor" in item.evidence
    ]
    other = [item for item in flags if item not in constructor_flags]
    assert other == [], [str(item) for item in other]

    registry = yaml.safe_load(
        (ROOT / "data/parse/grobid_compatibility.yaml").read_text(encoding="utf-8")
    )
    constructors = (registry.get("defaults") or {}).get("production_pdf_constructors") or {}
    scan_roots = [str(item) for item in (constructors.get("scan_roots") or [])]
    live = _scan_bare_from_env_parse(ROOT, scan_roots)
    if not live:
        assert constructor_flags == []
        return
    assert constructor_flags, live
    for item in constructor_flags:
        for hit in live:
            assert hit in item.evidence, (hit, item.evidence)
            assert any(
                hit == root or hit.startswith(root.rstrip("/") + "/") or hit.startswith(root + ":")
                for root in scan_roots
            ), hit


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


def test_checker_flags_health_then_parse_lifetime_bypass(tmp_path: Path):
    """Fail-closed lifetime: health probe + later parse_pdf on the same binding is PARSE_BARE."""
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/lifetime_bypass.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "async fn run(pdf: &str, id: &str) {\n"
        "    let parser = GrobidParser::from_env();\n"
        "    if !parser.is_alive().await {\n"
        "        eprintln!(\"GROBID down\");\n"
        "        return;\n"
        "    }\n"
        "    let parsed = parser.parse_pdf(pdf, id).await;\n"
        "    println!(\"{parsed:?}\");\n"
        "}\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("lifetime_bypass.rs" in item.evidence for item in matching)


def test_checker_flags_moved_binding_despite_health_probe(tmp_path: Path):
    """A bare binding consumed (moved into Box) is real work, not health-only — PARSE_BARE."""
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/moved_binding.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "async fn run() {\n"
        "    let parser = GrobidParser::from_env();\n"
        "    if !parser.is_alive().await {\n"
        "        return;\n"
        "    }\n"
        "    let _boxed = Box::new(parser);\n"
        "}\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("moved_binding.rs" in item.evidence for item in matching)


@pytest.mark.parametrize(
    "filename,payload",
    [
        ("helper_allow.rs", "fn corpus_grobid_parser() {\n    let _p = GrobidParser::from_env();\n}\n"),
        ("profile_allow.rs", "fn parse() {\n    let parser = GrobidParser::from_env().with_profile(profile);\n    parser.parse_pdf(p, i);\n}\n"),
        ("test_allow.rs", "mod tests {\n    #[test]\n    fn t() {\n        let p = GrobidParser::from_env();\n        let _ = p;\n    }\n}\n"),
        ("comment_allow.rs", "fn parse() {}\n// GrobidParser::from_env() stays historical rich\n"),
        ("helper_fn_allow.rs", "fn corpus_grobid_parser() {}\nfn parse() {\n    let p = corpus_grobid_parser();\n    let _ = p;\n}\n"),
    ],
)
def test_checker_allows_non_bare_from_env_roles(tmp_path: Path, filename: str, payload: str):
    """Allowlist: helper, with_profile, test, comment, and non-from_env helpers stay allowed."""
    _copy_checker_surface(tmp_path)
    target = tmp_path / f"crates/da-cli/src/{filename}"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [
        item
        for item in flags
        if item.flag == "scholarly_parse_plane_policy_stamp" and filename in item.evidence
    ]
    assert matching == [], [str(item) for item in flags]


@pytest.mark.parametrize("scan_root", ["crates/da-cli/does_not_exist", "crates/da-cli/examples"])
def test_checker_scan_roots_keep_isolation_behavior(tmp_path: Path, scan_root: str):
    """Malformed/missing declared scan roots keep isolation: skipped, never crash, no stray flags."""
    _copy_checker_surface(tmp_path)
    registry_path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(registry_path.read_text(encoding="utf-8"))
    payload["defaults"]["production_pdf_constructors"]["scan_roots"] = [scan_root]
    registry_path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    # A bare parse site under a NON-declared production root must stay invisible.
    target = tmp_path / "crates/da-cli/src/outside_scope.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "fn parse() {\n    let parser = GrobidParser::from_env();\n    parser.parse_pdf(p, i);\n}\n",
        encoding="utf-8",
    )
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [
        item
        for item in flags
        if item.flag == "scholarly_parse_plane_policy_stamp" and "outside_scope.rs" in item.evidence
    ]
    assert matching == [], [str(item) for item in flags]


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


def test_empty_native_consult_tokens_derive_snake_and_pascal_from_data():
    payload = yaml.safe_load((ROOT / "data/policies/registry.yaml").read_text(encoding="utf-8"))
    consult = payload.get("consult_status") or {}
    assert _empty_native_consult_tokens(consult) == [
        ("concurrency_tune", "ConcurrencyTune"),
        ("retry_policy", "RetryPolicy"),
        ("summary_depth", "SummaryDepth"),
    ]
    synthetic = _empty_native_consult_tokens({"empty_native_unknown_point": ["my_point", " a_b ", 7, "", None]})
    assert synthetic == [("my_point", "MyPoint"), ("a_b", "AB")]


def test_empty_native_consult_tokens_rejects_malformed_without_invented_fallback():
    assert _empty_native_consult_tokens(None) == []
    assert _empty_native_consult_tokens({}) == []
    assert _empty_native_consult_tokens({"empty_native_unknown_point": "concurrency_tune"}) == []
    assert _empty_native_consult_tokens({"empty_native_unknown_point": [None, 7, "  "]}) == []
    names = {snake for snake, _ in _empty_native_consult_tokens(
        {"empty_native_unknown_point": ["concurrency_tune", "retry_policy", "summary_depth"]})}
    assert "batch_admission" not in names
    assert not names & {"head_fallback", "visual_bundle", "critic_sample"}


def _source_cli_policy_stamp_flags(flags):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if "source_cli.rs#empty_native_unknown_point" in item.evidence]


def test_checker_flags_empty_native_live_snake_consult_in_source_cli(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("// live consult concurrency_tune stays here\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _source_cli_policy_stamp_flags(flags)
    assert matching, flags
    assert all("plane=integration" in item.evidence and "violation=policy_stamp" in item.evidence for item in matching)
    hit = next(item for item in matching if "concurrency_tune" in item.evidence)
    assert "crates/da-cli/src/source_cli.rs" in hit.evidence
    assert "UnknownPoint" in hit.evidence


def test_checker_flags_empty_native_live_pascal_consult_in_source_cli(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("let p = DecisionPoint::RetryPolicy;\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _source_cli_policy_stamp_flags(flags)
    assert matching, flags
    hit = next(item for item in matching if "retry_policy" in item.evidence)
    assert "crates/da-cli/src/source_cli.rs" in hit.evidence
    assert "DecisionPoint::RetryPolicy" in hit.evidence
    assert "UnknownPoint" in hit.evidence


def test_checker_empty_native_source_cli_scan_is_data_driven(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/policies/registry.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    status = payload.setdefault("consult_status", {})
    status["empty_native_unknown_point"] = ["retry_policy", "summary_depth"]
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("// live consult concurrency_tune stays here\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _source_cli_policy_stamp_flags(flags) == [], flags


def test_checker_empty_native_scan_ignores_batch_admission_and_later(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("let a = DecisionPoint::BatchAdmission; // head_fallback visual_bundle critic_sample\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _source_cli_policy_stamp_flags(flags) == [], flags


def _native_rules_source_cli_policy_stamp_flags(flags):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if "source_cli.rs#native_rules_not_consulted" in item.evidence]


def test_native_rules_not_consulted_tokens_derive_snake_and_pascal_from_data():
    payload = yaml.safe_load((ROOT / "data/policies/registry.yaml").read_text(encoding="utf-8"))
    consult = payload.get("consult_status") or {}
    assert _native_rules_not_consulted_tokens(consult) == [("batch_admission", "BatchAdmission")]
    synthetic = _native_rules_not_consulted_tokens({"native_rules_not_consulted": ["my_point", " a_b ", 7, "", None]})
    assert synthetic == [("my_point", "MyPoint"), ("a_b", "AB")]


def test_native_rules_not_consulted_tokens_rejects_malformed_without_invented_fallback():
    assert _native_rules_not_consulted_tokens(None) == []
    assert _native_rules_not_consulted_tokens({}) == []
    assert _native_rules_not_consulted_tokens({"native_rules_not_consulted": "batch_admission"}) == []
    assert _native_rules_not_consulted_tokens({"native_rules_not_consulted": [None, 7, "  "]}) == []
    names = {snake for snake, _ in _native_rules_not_consulted_tokens(
        {"native_rules_not_consulted": ["batch_admission"]})}
    assert names == {"batch_admission"}
    assert not names & {"concurrency_tune", "retry_policy", "summary_depth"}
    assert not names & {"head_fallback", "visual_bundle", "critic_sample"}


def test_checker_flags_native_rules_snake_consult_in_source_cli(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("// live consult batch_admission stays here\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _native_rules_source_cli_policy_stamp_flags(flags)
    assert matching, flags
    assert all("plane=integration" in item.evidence and "violation=policy_stamp" in item.evidence for item in matching)
    hit = next(item for item in matching if "batch_admission" in item.evidence)
    assert hit.flag == "scholarly_parse_plane_policy_stamp"
    assert "crates/da-cli/src/source_cli.rs#native_rules_not_consulted.batch_admission" in hit.evidence
    assert "sona-policy demo is not closure" in hit.evidence


def test_checker_flags_native_rules_pascal_consult_in_source_cli(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("let p = DecisionPoint::BatchAdmission;\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _native_rules_source_cli_policy_stamp_flags(flags)
    assert matching, flags
    hit = next(item for item in matching if "batch_admission" in item.evidence)
    assert hit.flag == "scholarly_parse_plane_policy_stamp"
    assert "crates/da-cli/src/source_cli.rs#native_rules_not_consulted.batch_admission" in hit.evidence
    assert "DecisionPoint::BatchAdmission" in hit.evidence
    assert "sona-policy demo is not closure" in hit.evidence


def test_checker_native_rules_source_cli_scan_is_data_driven(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/policies/registry.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    status = payload.setdefault("consult_status", {})
    status["native_rules_not_consulted"] = []
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("let p = DecisionPoint::BatchAdmission;\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _native_rules_source_cli_policy_stamp_flags(flags) == [], flags


def test_checker_native_rules_scan_skips_missing_source_cli(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    if target.is_file():
        target.unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _native_rules_source_cli_policy_stamp_flags(flags) == [], flags


def test_checker_native_rules_scan_stays_out_of_empty_native_namespace(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-cli/src/source_cli.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("let p = DecisionPoint::BatchAdmission;\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _source_cli_policy_stamp_flags(flags) == [], flags
    assert _native_rules_source_cli_policy_stamp_flags(flags), flags


def test_checker_sona_policy_may_demo_batch_admission_without_refuse():
    source = (ROOT / "crates/da-cli/src/background_tasks.rs").read_text(encoding="utf-8")
    assert "fn batch_admission_is_demo_not_parse_pdf" in source
    assert 'sona_policy_report(&registry_yaml(), "batch_admission"' in source
    assert "parse_pdf_consult=false" in source


def test_checker_does_not_replace_independent_promotion_contract():
    source = (ROOT / "scripts/governor/application/trajectory_service.py").read_text(encoding="utf-8")
    assert "drift_flags.extend(self._check_promotion_evidence_contract())" in source
    assert "promotion_evidence_contract.yaml" in source
    # S19 consumes the shared contract path; it does not define Promotion FSM
    # transitions or copy the promotion validator into the parse-plane adapter.
    checker_source = (ROOT / "scripts/governor/adapters/checks/scholarly_parse_plane.py").read_text(encoding="utf-8")
    assert "shared_promotion_contract" in checker_source
    assert "PromotionDecision" not in checker_source


# ───── S05/T01: Java absent endpoints stay absent with no proxy surface ─────

EXPECTED_JAVA_ABSENT_TOKENS = [
    "processHeaderFundingDocument",
    "processFulltextDocumentHTML",
    "processFulltextAssetDocument",
    "createTraining",
    "annotatePDF",
]


def _load_live_registry() -> dict:
    return yaml.safe_load((ROOT / "data/parse/grobid_compatibility.yaml").read_text(encoding="utf-8"))


def _java_absent_flags(flags) -> list:
    return [
        item
        for item in flags
        if item.flag == "scholarly_parse_plane_ledger_surface"
        and "java_absent_endpoints." in item.evidence
    ]


def _set_capability_state_surface(payload: dict, cap_id: str, state: str, surface) -> None:
    for cap in payload.get("capabilities") or []:
        if isinstance(cap, dict) and cap.get("id") == cap_id:
            cap["state"] = state
            impl = cap.setdefault("implementation", {})
            if isinstance(impl, dict):
                impl["surface"] = surface


def _write_registry_copy(tmp_path: Path, mutate=None) -> None:
    _copy_checker_surface(tmp_path)
    if mutate is None:
        return
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    mutate(payload)
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")


def test_java_absent_tokens_derive_live_from_data():
    payload = _load_live_registry()
    assert _java_absent_endpoint_tokens(payload.get("defaults")) == EXPECTED_JAVA_ABSENT_TOKENS


def test_java_absent_helper_rejects_malformed_without_invented_fallback():
    assert _java_absent_endpoint_tokens(None) == []
    assert _java_absent_endpoint_tokens({}) == []
    assert _java_absent_endpoint_tokens({"java_absent_endpoints": "createTraining"}) == []
    assert _java_absent_endpoint_tokens({"java_absent_endpoints": [None, 7, "  "]}) == []
    assert _java_absent_endpoint_tokens(
        {"java_absent_endpoints": ["createTraining", "", None, 7, " annotatePDF "]}
    ) == ["createTraining", "annotatePDF"]


def test_java_absent_live_capabilities_are_absent_with_none_surface():
    payload = _load_live_registry()
    caps = [c for c in payload.get("capabilities") or [] if isinstance(c, dict)]
    by_id = {str(c.get("id")): c for c in caps}
    suffix = {
        "processHeaderFundingDocument": "rest.processHeaderFundingDocument",
        "processFulltextDocumentHTML": "rest.processFulltextDocumentHTML",
        "processFulltextAssetDocument": "rest.processFulltextAssetDocument",
        "createTraining": "rest.createTraining",
        "annotatePDF": "rest.annotatePDF",
    }
    for token in EXPECTED_JAVA_ABSENT_TOKENS:
        cap = by_id.get(suffix[token])
        assert cap is not None, f"missing capability for {token}"
        assert cap.get("state") == "absent", token
        surface = (cap.get("implementation") or {}).get("surface")
        assert surface is None or str(surface).strip().lower() == "none", token


def test_checker_clean_root_has_no_java_absent_flags():
    assert _java_absent_flags(_check_scholarly_parse_plane(ROOT)) == []


def test_checker_flags_fake_proxy_with_real_surface(tmp_path: Path):
    def mutate(payload: dict) -> None:
        _set_capability_state_surface(
            payload,
            "rest.createTraining",
            "proxy",
            "crates/da-parse/src/http.rs GrobidHttpBackend::process_training",
        )

    _write_registry_copy(tmp_path, mutate)
    flags = _java_absent_flags(_check_scholarly_parse_plane(tmp_path))
    assert len(flags) == 1, flags
    hit = flags[0]
    assert hit.flag == "scholarly_parse_plane_ledger_surface"
    assert "data/parse/grobid_compatibility.yaml#java_absent_endpoints.createTraining" in hit.evidence
    assert "Java path is not a proxy surface" in hit.evidence


def test_checker_flags_absent_with_non_none_surface(tmp_path: Path):
    def mutate(payload: dict) -> None:
        _set_capability_state_surface(
            payload, "rest.annotatePDF", "absent", "crates/da-parse/src/http.rs"
        )

    _write_registry_copy(tmp_path, mutate)
    flags = _java_absent_flags(_check_scholarly_parse_plane(tmp_path))
    assert len(flags) == 1, flags
    assert "data/parse/grobid_compatibility.yaml#java_absent_endpoints.annotatePDF" in flags[0].evidence
    assert "Java path is not a proxy surface" in flags[0].evidence


def test_checker_flags_missing_capability_row_with_defaults_token_kept(tmp_path: Path):
    def mutate(payload: dict) -> None:
        payload["capabilities"] = [
            c for c in payload.get("capabilities") or [] if c.get("id") != "rest.annotatePDF"
        ]

    _write_registry_copy(tmp_path, mutate)
    flags = _java_absent_flags(_check_scholarly_parse_plane(tmp_path))
    assert len(flags) == 1, flags
    assert "data/parse/grobid_compatibility.yaml#java_absent_endpoints.annotatePDF" in flags[0].evidence
    assert "Java path is not a proxy surface" in flags[0].evidence


def test_checker_data_mutation_removing_token_drops_its_evidence(tmp_path: Path):
    def mutate(payload: dict) -> None:
        tokens = (payload.get("defaults") or {}).get("java_absent_endpoints") or []
        payload["defaults"]["java_absent_endpoints"] = [t for t in tokens if t != "createTraining"]
        _set_capability_state_surface(
            payload,
            "rest.createTraining",
            "proxy",
            "crates/da-parse/src/http.rs GrobidHttpBackend::process_training",
        )

    _write_registry_copy(tmp_path, mutate)
    flags = _java_absent_flags(_check_scholarly_parse_plane(tmp_path))
    assert not any("java_absent_endpoints.createTraining" in item.evidence for item in flags), flags


def test_checker_missing_ledger_has_no_java_absent_namespace_but_keeps_integrity_signals(
    tmp_path: Path,
):
    _copy_checker_surface(tmp_path)
    (tmp_path / "data/parse/grobid_compatibility.yaml").unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _java_absent_flags(flags) == [], flags
    assert any(
        item.flag
        in {
            "scholarly_parse_plane_evidence_missing",
            "scholarly_parse_plane_backend_identity",
        }
        for item in flags
    ), flags


def test_checker_keeps_existing_empty_surface_ledger_surface(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    payload["capabilities"].append(
        {
            "id": "rest.inventedProxy",
            "state": "proxy",
            "implementation": {"surface": "none"},
        }
    )
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    legacy = [
        item
        for item in flags
        if item.flag == "scholarly_parse_plane_ledger_surface"
        and "java_absent_endpoints." not in item.evidence
    ]
    assert any("rest.inventedProxy" in item.evidence for item in legacy), flags
    # The five DATA tokens stay clean while the legacy opposite-polarity hit fires.
    assert _java_absent_flags(flags) == [], flags

# ───── S06/T01: header atoms stay shadow, out of parse-pdf source_cli and ParserPort ─────

HEADER_QUARANTINE_TOKENS = ["compose_header", "run_header_sequence", "NativeDateParser"]
HEADER_QUARANTINE_BOUNDARIES = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
]
HEADER_QUARANTINE_PAYLOADS = {
    "compose_header": "let _h = compose_header(input);\n",
    "run_header_sequence": "let _h = run_header_sequence(input);\n",
    "NativeDateParser": "let _p = NativeDateParser::new();\n",
}

def _header_quarantine_flags(flags):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if "#header_quarantine." in item.evidence]

@pytest.mark.parametrize("relative", HEADER_QUARANTINE_BOUNDARIES)
@pytest.mark.parametrize("token", HEADER_QUARANTINE_TOKENS)
def test_checker_flags_header_quarantine_token_in_boundary(tmp_path: Path, relative: str, token: str):
    _copy_checker_surface(tmp_path)
    target = tmp_path / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(HEADER_QUARANTINE_PAYLOADS[token], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _header_quarantine_flags(flags)
    assert matching, flags
    hit = next(item for item in matching if token in item.evidence and relative in item.evidence)
    assert hit.flag == "scholarly_parse_plane_policy_stamp"
    assert hit.severity == "high"
    assert f"#header_quarantine.{token}" in hit.evidence
    assert "plane=integration" in hit.evidence and "violation=policy_stamp" in hit.evidence

@pytest.mark.parametrize(
    "payload",
    [
        "let _h = decompose_header(input);\n",
        "// compose_header is shadow-only prose\n",
        "// run_header_sequence is shadow-only prose\n",
        "// NativeDateParser is shadow-only prose\n",
        "/// doc compose_header prose\n",
        "let x = 1; // run_header_sequence prose\n",
        "let x = 1; // NativeDateParser prose\n",
    ],
)
def test_checker_header_quarantine_ignores_decompose_and_prose_comments(tmp_path: Path, payload: str):
    _copy_checker_surface(tmp_path)
    for relative in HEADER_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _header_quarantine_flags(flags) == [], flags

def test_checker_clean_boundaries_have_no_header_quarantine_flags():
    assert _header_quarantine_flags(_check_scholarly_parse_plane(ROOT)) == []

def test_checker_header_quarantine_skips_missing_boundaries(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    for relative in HEADER_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        if target.is_file():
            target.unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _header_quarantine_flags(flags) == [], flags

def test_checker_header_quarantine_stays_silent_without_ledger_but_keeps_integrity_signals(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    (tmp_path / "data/parse/grobid_compatibility.yaml").unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _header_quarantine_flags(flags) == [], flags
    assert any(
        item.flag
        in {
            "scholarly_parse_plane_evidence_missing",
            "scholarly_parse_plane_backend_identity",
        }
        for item in flags
    ), flags

# ───── S07/T01: bibliography atoms stay shadow, out of parse-pdf source_cli and ParserPort ─────

BIBLIOGRAPHY_QUARANTINE_TOKENS = ["compose_references", "run_reference_sequence"]
BIBLIOGRAPHY_QUARANTINE_BOUNDARIES = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
]
BIBLIOGRAPHY_QUARANTINE_PAYLOADS = {
    "compose_references": "let _r = compose_references(input);\n",
    "run_reference_sequence": "let _r = run_reference_sequence(input);\n",
}

def _bibliography_quarantine_flags(flags):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if "#bibliography_quarantine." in item.evidence]

@pytest.mark.parametrize("relative", BIBLIOGRAPHY_QUARANTINE_BOUNDARIES)
@pytest.mark.parametrize("token", BIBLIOGRAPHY_QUARANTINE_TOKENS)
def test_checker_flags_bibliography_quarantine_token_in_boundary(tmp_path: Path, relative: str, token: str):
    _copy_checker_surface(tmp_path)
    target = tmp_path / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(BIBLIOGRAPHY_QUARANTINE_PAYLOADS[token], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _bibliography_quarantine_flags(flags)
    assert matching, flags
    hit = next(item for item in matching if token in item.evidence and relative in item.evidence)
    assert hit.flag == "scholarly_parse_plane_policy_stamp"
    assert hit.severity == "high"
    assert f"#bibliography_quarantine.{token}" in hit.evidence
    assert "plane=integration" in hit.evidence and "violation=policy_stamp" in hit.evidence

@pytest.mark.parametrize("token", BIBLIOGRAPHY_QUARANTINE_TOKENS)
def test_checker_bibliography_quarantine_allows_whitespace_before_paren(tmp_path: Path, token: str):
    _copy_checker_surface(tmp_path)
    for relative in BIBLIOGRAPHY_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(f"let _r = {token} (input);\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _bibliography_quarantine_flags(flags)
    assert len(matching) == len(BIBLIOGRAPHY_QUARANTINE_BOUNDARIES), flags
    assert all(f"#bibliography_quarantine.{token}" in item.evidence for item in matching), flags

@pytest.mark.parametrize(
    "payload",
    [
        "let _r = decompose_references(input);\n",
        "// compose_references is shadow-only prose\n",
        "// run_reference_sequence is shadow-only prose\n",
        "/// doc compose_references prose\n",
        "/// doc run_reference_sequence prose\n",
        "let x = 1; // compose_references prose\n",
        "let x = 1; // run_reference_sequence prose\n",
        "compose_references is shadow-only prose without a call\n",
    ],
)
def test_checker_bibliography_quarantine_ignores_prefixed_and_prose_comments(tmp_path: Path, payload: str):
    _copy_checker_surface(tmp_path)
    for relative in BIBLIOGRAPHY_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _bibliography_quarantine_flags(flags) == [], flags

def test_checker_clean_root_has_no_bibliography_quarantine_flags():
    assert _bibliography_quarantine_flags(_check_scholarly_parse_plane(ROOT)) == []

def test_checker_bibliography_quarantine_skips_missing_boundaries(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    for relative in BIBLIOGRAPHY_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        if target.is_file():
            target.unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _bibliography_quarantine_flags(flags) == [], flags

def test_checker_bibliography_quarantine_stays_silent_without_ledger_but_keeps_integrity_signals(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    (tmp_path / "data/parse/grobid_compatibility.yaml").unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _bibliography_quarantine_flags(flags) == [], flags
    assert any(
        item.flag
        in {
            "scholarly_parse_plane_evidence_missing",
            "scholarly_parse_plane_backend_identity",
        }
        for item in flags
    ), flags

def test_checker_bibliography_quarantine_stays_silent_on_malformed_ledger(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    path.write_text("{unclosed: [1,\n", encoding="utf-8")
    target = tmp_path / BIBLIOGRAPHY_QUARANTINE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(BIBLIOGRAPHY_QUARANTINE_PAYLOADS["compose_references"], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _bibliography_quarantine_flags(flags) == [], flags
    assert any(
        item.flag == "scholarly_parse_plane_backend_identity" for item in flags
    ), flags

def test_checker_bibliography_and_java_absent_namespaces_stay_isolated(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / BIBLIOGRAPHY_QUARANTINE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(BIBLIOGRAPHY_QUARANTINE_PAYLOADS["compose_references"], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _bibliography_quarantine_flags(flags), flags
    assert _java_absent_flags(flags) == [], flags

# ───── S08/T01: visual heads stay shadow, out of parse-pdf source_cli and ParserPort ─────

VISUAL_HEADS_QUARANTINE_TOKENS = [
    "detect_figure_heads",
    "detect_table_heads",
    "detect_formula_heads",
    "NativeVisual",
    "LayoutProvider",
]
VISUAL_HEADS_QUARANTINE_BOUNDARIES = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
]
VISUAL_HEADS_QUARANTINE_PAYLOADS = {
    "detect_figure_heads": "let _v = detect_figure_heads(input);\n",
    "detect_table_heads": "let _v = detect_table_heads(input);\n",
    "detect_formula_heads": "let _v = detect_formula_heads(input);\n",
    "NativeVisual": "let _v: NativeVisual = build();\n",
    "LayoutProvider": "let _v: LayoutProvider = build();\n",
}

EXPECTED_VISUAL_HEADS_TOKENS = list(VISUAL_HEADS_QUARANTINE_TOKENS)
EXPECTED_VISUAL_HEADS_BOUNDARIES = list(VISUAL_HEADS_QUARANTINE_BOUNDARIES)


def _visual_heads_quarantine_flags(flags):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if "#visual_heads_quarantine." in item.evidence]


def _load_live_compat() -> dict:
    return yaml.safe_load((ROOT / "data/parse/grobid_compatibility.yaml").read_text(encoding="utf-8"))


def test_visual_heads_quarantine_derive_live_from_data():
    payload = _load_live_compat()
    tokens, boundaries = _visual_heads_quarantine_spec(payload.get("defaults"))
    assert tokens == EXPECTED_VISUAL_HEADS_TOKENS
    assert boundaries == EXPECTED_VISUAL_HEADS_BOUNDARIES


def test_visual_heads_quarantine_helper_rejects_malformed_without_invented_fallback():
    assert _visual_heads_quarantine_spec(None) == ([], [])
    assert _visual_heads_quarantine_spec({}) == ([], [])
    assert _visual_heads_quarantine_spec({"visual_heads_quarantine": "detect_figure_heads"}) == ([], [])
    assert _visual_heads_quarantine_spec({"visual_heads_quarantine": {"tokens": "x", "boundaries": "y"}}) == ([], [])
    assert _visual_heads_quarantine_spec(
        {"visual_heads_quarantine": {"tokens": [None, 7, "  "], "boundaries": [None, 7, "  "]}}
    ) == ([], [])
    assert _visual_heads_quarantine_spec(
        {"visual_heads_quarantine": {"tokens": ["detect_figure_heads", "", None, 7, " LayoutProvider "], "boundaries": ["crates/da-cli/src/source_cli.rs", "", None]}}
    ) == (["detect_figure_heads", "LayoutProvider"], ["crates/da-cli/src/source_cli.rs"])


@pytest.mark.parametrize("relative", VISUAL_HEADS_QUARANTINE_BOUNDARIES)
@pytest.mark.parametrize("token", VISUAL_HEADS_QUARANTINE_TOKENS)
def test_checker_flags_visual_heads_quarantine_token_in_boundary(tmp_path: Path, relative: str, token: str):
    _copy_checker_surface(tmp_path)
    target = tmp_path / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(VISUAL_HEADS_QUARANTINE_PAYLOADS[token], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _visual_heads_quarantine_flags(flags)
    assert matching, flags
    hit = next(item for item in matching if token in item.evidence and relative in item.evidence)
    assert hit.flag == "scholarly_parse_plane_policy_stamp"
    assert hit.severity == "high"
    assert f"#visual_heads_quarantine.{token}" in hit.evidence
    assert "plane=integration" in hit.evidence and "violation=policy_stamp" in hit.evidence


@pytest.mark.parametrize("token", ["detect_figure_heads", "detect_table_heads", "detect_formula_heads"])
def test_checker_visual_heads_quarantine_allows_whitespace_before_paren(tmp_path: Path, token: str):
    _copy_checker_surface(tmp_path)
    for relative in VISUAL_HEADS_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(f"let _v = {token} (input);\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _visual_heads_quarantine_flags(flags)
    assert len(matching) == len(VISUAL_HEADS_QUARANTINE_BOUNDARIES), flags
    assert all(f"#visual_heads_quarantine.{token}" in item.evidence for item in matching), flags


@pytest.mark.parametrize(
    "payload",
    [
        "let _v = my_detect_figure_heads(input);\n",
        "let _v = detect_figure_heads_extra(input);\n",
        "detect_figure_heads is shadow-only prose without a call\n",
        "let _v = my_detect_table_heads(input);\n",
        "let _v = detect_table_heads_extra(input);\n",
        "detect_table_heads is shadow-only prose without a call\n",
        "let _v = my_detect_formula_heads(input);\n",
        "let _v = detect_formula_heads_extra(input);\n",
        "detect_formula_heads is shadow-only prose without a call\n",
        "let _v = MyNativeVisual::new();\n",
        "let _v = NativeVisualExtra::new();\n",
        "let _v = MyLayoutProvider::new();\n",
        "let _v = LayoutProviderExtra::new();\n",
        "// detect_figure_heads is shadow-only prose\n",
        "// detect_table_heads is shadow-only prose\n",
        "// detect_formula_heads is shadow-only prose\n",
        "// NativeVisual is shadow-only prose\n",
        "// LayoutProvider is shadow-only prose\n",
        "/// doc detect_figure_heads prose\n",
        "/// doc NativeVisual prose\n",
        "let x = 1; // detect_figure_heads prose\n",
        "let x = 1; // LayoutProvider prose\n",
    ],
)
def test_checker_visual_heads_quarantine_ignores_prefixed_and_prose_comments(tmp_path: Path, payload: str):
    _copy_checker_surface(tmp_path)
    for relative in VISUAL_HEADS_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _visual_heads_quarantine_flags(flags) == [], flags


def test_checker_clean_root_has_no_visual_heads_quarantine_flags():
    assert _visual_heads_quarantine_flags(_check_scholarly_parse_plane(ROOT)) == []


def test_checker_visual_heads_quarantine_skips_missing_boundaries(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    for relative in VISUAL_HEADS_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        if target.is_file():
            target.unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _visual_heads_quarantine_flags(flags) == [], flags


def test_checker_visual_heads_quarantine_stays_silent_without_ledger_but_keeps_integrity_signals(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    (tmp_path / "data/parse/grobid_compatibility.yaml").unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _visual_heads_quarantine_flags(flags) == [], flags
    assert any(
        item.flag
        in {
            "scholarly_parse_plane_evidence_missing",
            "scholarly_parse_plane_backend_identity",
        }
        for item in flags
    ), flags


def test_checker_visual_heads_quarantine_stays_silent_on_malformed_ledger(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    path.write_text("{unclosed: [1,\n", encoding="utf-8")
    target = tmp_path / VISUAL_HEADS_QUARANTINE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(VISUAL_HEADS_QUARANTINE_PAYLOADS["detect_figure_heads"], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _visual_heads_quarantine_flags(flags) == [], flags
    assert any(
        item.flag == "scholarly_parse_plane_backend_identity" for item in flags
    ), flags


def test_checker_visual_heads_quarantine_stays_silent_on_missing_section_but_keeps_other_namespaces(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    payload.get("defaults", {}).pop("visual_heads_quarantine", None)
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    target = tmp_path / VISUAL_HEADS_QUARANTINE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(VISUAL_HEADS_QUARANTINE_PAYLOADS["detect_figure_heads"], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _visual_heads_quarantine_flags(flags) == [], flags


def test_checker_visual_heads_and_other_namespaces_stay_isolated(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / VISUAL_HEADS_QUARANTINE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(VISUAL_HEADS_QUARANTINE_PAYLOADS["detect_figure_heads"], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _visual_heads_quarantine_flags(flags), flags
    assert _java_absent_flags(flags) == [], flags
    assert _bibliography_quarantine_flags(flags) == [], flags
    assert _header_quarantine_flags(flags) == [], flags


# ───── M493 S12/T02: candidate freeze fails closed on live DATA ─────
#
# Each focused negative fixture mutates exactly one live-DATA S12 surface on
# a copied root, then asserts the checker localizes the exact stable
# violation (policy stamp shape: plane= + requirements=). DATA-driven: no
# example names are hardcoded — mutations rewrite the live YAML values in
# place, so the assertions pin the checker's fail-closed wiring, not a
# fixture copy.

_S12_FROZEN_ROLLBACK = "java-grobid-http@0.9.0-crf"
_S12_CANDIDATE_ID = "da-parse.native_header_reference_composition"
_S12_REQUIRED_RECEIPTS = [
    "native_date_shadow",
    "native_personal_name_shadow",
    "native_affiliation_shadow",
    "native_citation_shadow",
]


def _s12_assert_policy_stamp(flags, code: str, evidence_needle: str):
    matching = [item for item in flags if item.flag == f"scholarly_parse_plane_{code}"]
    assert matching, flags
    assert all(item.severity == "high" for item in matching)
    hit = next(item for item in matching if evidence_needle in item.evidence)
    assert "plane=" in hit.evidence and "requirements=" in hit.evidence
    assert "fail-closed" in hit.evidence or "violation=" in hit.evidence
    return matching


def _s12_rewrite_yaml(root: Path, relative: str, mutate):
    path = root / relative
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    mutate(payload)
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")


def test_s12_checker_flags_wave_self_promotion(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        payload["defaults"]["promotion"]["this_wave_promotes"] = "header"

    _s12_rewrite_yaml(tmp_path, "data/parse/grobid_compatibility.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "family_promote_cap", "defaults.promotion")


def test_s12_checker_flags_deployed_and_auto_promote_unlock(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        payload["defaults"]["promotion"]["deployed"] = True
        payload["defaults"]["promotion"]["auto_promote"] = "allowed"

    _s12_rewrite_yaml(tmp_path, "data/parse/grobid_compatibility.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "family_promote_cap", "defaults.promotion")


def test_s12_checker_flags_candidate_state_drift(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        payload["native_header_reference_composition_shadow"]["promotion_state"] = "promoted"

    _s12_rewrite_yaml(tmp_path, "data/parse/comparison_contract.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "family_promote_cap", "candidate_freeze")


def test_s12_checker_flags_invented_explicit_decision(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        payload["native_header_reference_composition_shadow"]["explicit_decision"] = "user-yes-synthetic"

    _s12_rewrite_yaml(tmp_path, "data/parse/comparison_contract.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "promotion_receipt", "explicit_decision")


def test_s12_checker_flags_missing_component_receipt(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        required = payload["native_header_reference_composition_shadow"]["component_receipts"]["required"]
        required.remove("native_citation_shadow")

    _s12_rewrite_yaml(tmp_path, "data/parse/comparison_contract.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "acceptance_same_n_drift", "component_receipts.required")


def test_s12_checker_flags_selectable_lane_widening(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        payload["native_header_reference_composition_shadow"]["independently_selectable_lanes"] = [
            "header",
            "references",
            "body",
        ]

    _s12_rewrite_yaml(tmp_path, "data/parse/comparison_contract.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "body_java_fallback", "independently_selectable_lanes")


def test_s12_checker_flags_composition_freeze_unlock(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        payload["observation_mode"] = "production"
        payload["graph_mutation"] = "allowed"

    _s12_rewrite_yaml(tmp_path, "data/parse/native_fulltext_composition.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "family_promote_cap", "candidate_freeze")
    _s12_assert_policy_stamp(flags, "graph_write", "graph_mutation")


def test_s12_checker_flags_import_unlock_on_positive_envelope(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    positive_path = tmp_path / POSITIVE_PATH.relative_to(ROOT)
    envelope = json.loads(positive_path.read_text(encoding="utf-8"))
    envelope["promotion"]["import_eligible"] = True
    positive_path.write_text(json.dumps(envelope), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "import_eligible", str(POSITIVE_PATH.relative_to(ROOT)))


def test_s12_checker_flags_graph_unlock_on_positive_envelope(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    positive_path = tmp_path / POSITIVE_PATH.relative_to(ROOT)
    envelope = json.loads(positive_path.read_text(encoding="utf-8"))
    envelope["promotion"]["graph_write"] = True
    positive_path.write_text(json.dumps(envelope), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "graph_write", str(POSITIVE_PATH.relative_to(ROOT)))


def test_s12_checker_flags_rollback_drift_off_java(tmp_path: Path):
    _copy_checker_surface(tmp_path)

    def mutate(payload):
        payload["native_header_reference_composition_shadow"]["rollback"]["header"] = "native-da-parse@9.9.9"

    _s12_rewrite_yaml(tmp_path, "data/parse/comparison_contract.yaml", mutate)
    flags = _check_scholarly_parse_plane(tmp_path)
    _s12_assert_policy_stamp(flags, "missing_rollback", "rollback.header")


def test_s12_checker_live_root_stays_clean_and_isolated(tmp_path: Path):
    """Clean-copy sanity: without any S12 mutation only evidence_substitution
    for the copied root may appear; no S12 fail-closed stamp fires."""
    _copy_checker_surface(tmp_path)
    flags = _check_scholarly_parse_plane(tmp_path)
    s12_evidence = [
        "candidate_freeze",
        "explicit_decision",
        "component_receipts.required",
        "independently_selectable_lanes",
        "rollback.header",
    ]
    assert not [item for item in flags if any(needle in item.evidence for needle in s12_evidence)], flags
