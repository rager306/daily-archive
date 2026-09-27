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
import re
import shutil
from pathlib import Path

import pytest
import yaml

from governor.adapters.checks.scholarly_parse_plane import (
    _cascade_observe_tokens,
    _check_scholarly_parse_plane,
    _empty_native_consult_tokens,
    _release_probe_digest,
    _java_absent_endpoint_tokens,
    _native_rules_not_consulted_tokens,
    _body_accounting_quarantine_spec,
    _body_accounting_quarantine_tokens_present,
    _feature_tagger_training_quarantine_spec,
    _feature_tagger_training_quarantine_tokens_present,
    _fulltext_sequence_quarantine_spec,
    _fulltext_sequence_quarantine_tokens_present,
    _visual_family_sequence_quarantine_spec,
    _visual_family_sequence_quarantine_tokens_present,
    _funding_sequence_quarantine_spec,
    _funding_sequence_quarantine_tokens_present,
    _bibliography_sequence_quarantine_spec,
    _bibliography_sequence_quarantine_tokens_present,
    _header_sequence_quarantine_spec,
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


def _ref(mapping, ref: str):
    """Read a dotted reference out of a nested mapping, None when absent."""
    node = mapping
    for part in str(ref).split("."):
        if not isinstance(node, dict):
            return None
        node = node.get(part)
    return node


def _copy_checker_surface(destination: Path) -> None:
    """Build a tracked-only synthetic root for report-level mutations.

    Two classes of file end up in the synthetic root:

    * small, mutated or pinned-by-tests-carrier files (policy DATA, contract
      DATA, fixtures, the release-probe artifact). These stay real copies,
      because the tests rewrite them to exercise negative cases.
    * the frozen holdout reference corpus carried by the release-probe lane
      (``reference_tei`` / ``reference_exceptions``): one ``.tei.xml`` per
      censused document. Every test only *reads* those bytes -- their sha256 is
      the pin under test -- so copying them per test cost ~99 MiB of scratch per
      call (119 call sites, ~14-38 GiB per suite run, which twice filled the
      filesystem and aborted auto-mode with ENOSPC on 2026-09-26). They are
      linked read-only instead, which is byte-identical for every reader and
      keeps the mutation surface unchanged.
    """
    policy = _load(POLICY_PATH)
    paths = {
        "scripts/governor/checks/scholarly_parse_plane.yaml",
        "scripts/governor/checks/promotion_evidence_contract.yaml",
        "data/parse/native_body_accounting.yaml",
        str(policy["fixtures"]["positive"]["path"]),
        str(policy["fixtures"]["negative"]["path"]),
    }
    for binding in _source_bindings(policy):
        paths.add(binding["path"])
    # M511/S08/T05: the release-probe lane reads the frozen holdout manifest and
    # the pinned reference bytes, so the synthetic root carries them too.
    link_only: set[str] = set()
    lane_spec = policy.get("release_probe_lane") or {}
    if lane_spec:
        holdout_rel = str(lane_spec.get("holdout_manifest") or "")
        if holdout_rel:
            paths.add(holdout_rel)
        lane = _ref(_load(ROOT / str(lane_spec.get("manifest") or "")), str(lane_spec.get("lane_ref") or "")) or {}
        for key in ("reference_tei", "reference_exceptions"):
            for entry in lane.get(key) or []:
                if isinstance(entry, dict) and entry.get("path"):
                    paths.add(str(entry["path"]))
                    link_only.add(str(entry["path"]))
    for relative in paths:
        source = ROOT / relative
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if relative in link_only and source.exists():
            target.symlink_to(source.resolve())
            continue
        shutil.copy2(source, target)


def _detach_linked_fixture(path: Path) -> Path:
    """Give a linked fixture a private copy so a mutation cannot reach the repo.

    ``_copy_checker_surface`` links the frozen holdout corpus instead of copying
    it. A test that wants to *mutate* one of those files must detach it first,
    otherwise the write follows the link into the tracked fixture: on
    2026-09-26 two holdout ``.tei.xml`` files were modified in place that way.
    """
    if path.is_symlink():
        source = path.resolve()
        path.unlink()
        shutil.copy2(source, path)
    return path


def _release_probe_flags(flags, suffix: str = ""):
    return [
        item
        for item in flags
        if item.flag.startswith("scholarly_parse_plane_release_probe")
        and item.flag.endswith(suffix)
    ]


def _probe_lane_path(root: Path) -> Path:
    return root / "data/parse/acceptance_manifest.yaml"


def _probe_artifact_path(root: Path) -> Path:
    return root / "artifacts/parse_compat/external_holdout_release_probe.json"


def _probe_holdout_path(root: Path) -> Path:
    """The DATA carrier of the probe population declared by the policy."""
    spec = _load(POLICY_PATH)["release_probe_lane"]
    return root / str(spec["holdout_manifest"])


def _probe_population(root: Path) -> int:
    """The expected document count, read from the holdout manifest DATA."""
    holdout = yaml.safe_load(_probe_holdout_path(root).read_text(encoding="utf-8")) or {}
    return len([article for article in (holdout.get("articles") or []) if isinstance(article, dict)])

def _probe_partition_refs(root: Path) -> tuple[str, str]:
    """The measurable and exception lane refs declared by the policy partition."""
    partition = _load(POLICY_PATH)["release_probe_lane"].get("population_partition") or {}
    return (
        str(partition.get("measurable_ref") or "reference_tei"),
        str(partition.get("exceptions_ref") or "reference_exceptions"),
    )

def _probe_measurable_population(root: Path) -> int:
    """The measured subset, read from the lane partition DATA."""
    spec = _load(POLICY_PATH)["release_probe_lane"]
    lane = _ref(_load(root / str(spec["manifest"])), str(spec["lane_ref"])) or {}
    measurable_ref, _ = _probe_partition_refs(root)
    return len([entry for entry in (lane.get(measurable_ref) or []) if isinstance(entry, dict)])

def _probe_exception_population(root: Path) -> int:
    """The typed-exception count, read from the lane partition DATA."""
    spec = _load(POLICY_PATH)["release_probe_lane"]
    lane = _ref(_load(root / str(spec["manifest"])), str(spec["lane_ref"])) or {}
    _, exceptions_ref = _probe_partition_refs(root)
    return len([entry for entry in (lane.get(exceptions_ref) or []) if isinstance(entry, dict)])


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


def test_body_accounting_quarantine_is_data_driven_and_comment_safe():
    defaults = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"]
    tokens, boundaries = _body_accounting_quarantine_spec(defaults)
    assert tokens == [
        "NativeBodyAccountant",
        "run_native_body_sequence",
        "join_p5_shadow",
        "check_cross_stage_semantic_invariants",
    ]
    assert boundaries == [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
    ]
    assert _body_accounting_quarantine_tokens_present(
        "let _x: NativeBodyAccountant; run_native_body_sequence (input); join_p5_shadow(input); check_cross_stage_semantic_invariants(input);",
        tokens,
    ) == tokens
    assert _body_accounting_quarantine_tokens_present(
        "// NativeBodyAccountant(input)\nlet NativeBodyAccountantFactory = 1;",
        tokens,
    ) == []


def test_checker_flags_native_body_accounting_in_production_boundary(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-ports/src/parser.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("fn parse() { run_native_body_sequence(input); }\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [
        item for item in flags
        if item.flag == "scholarly_parse_plane_policy_stamp"
        and "body_accounting_quarantine" in item.evidence
    ]
    assert matching, flags
    assert "run_native_body_sequence" in matching[0].evidence


def test_checker_rejects_native_body_ledger_mutations(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    ledger = tmp_path / "data/parse/native_body_accounting.yaml"
    original = ledger.read_text(encoding="utf-8")
    for mutation in ("unknown_label_behavior: accept", "pooling: allowed", "production_claims: allowed"):
        ledger.write_text(original.replace("unknown_label_behavior: fail_closed", mutation) if mutation.startswith("unknown") else original.replace("pooling: forbidden", mutation) if mutation.startswith("pooling") else original.replace("production_claims: forbidden", mutation), encoding="utf-8")
        flags = _check_scholarly_parse_plane(tmp_path)
        assert flags, mutation
        assert flags, mutation
        ledger.write_text(original, encoding="utf-8")


def test_feature_tagger_training_quarantine_is_data_bound_and_closed():
    registry = yaml.safe_load((ROOT / "data/parse/grobid_compatibility.yaml").read_text(encoding="utf-8"))
    defaults = registry["defaults"]
    tokens, boundaries = _feature_tagger_training_quarantine_spec(defaults)
    assert tokens == ["NativeFeatureTagger", "NativeFeatureInput", "FeatureUpstreamStatus"]
    assert boundaries == [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
        "crates/da-application/src",
        "crates/da-adapters/src",
    ]
    spec = defaults["feature_tagger_training_quarantine"]
    assert spec["production_authority"] == "java_http"
    assert spec["observe_mode"] == "shadow_receipt"
    assert spec["production_route"] == "forbidden"
    assert spec["graph_write"] == "forbidden"
    assert spec["import_eligible"] is False
    assert spec["auto_promote"] == "forbidden"
    assert spec["training_and_model_runtime"] == "forbidden"
    assert spec["same_n_required"] is True
    assert set(spec["dispositions"]) == {"port_structural_subset", "replace_runtime_abstraction", "defer_insufficient_evidence"}
    assert _feature_tagger_training_quarantine_tokens_present(
        "use da_parse::NativeFeatureTagger; fn f(x: NativeFeatureInput) { let _ = FeatureUpstreamStatus::Pass; }", tokens
    ) == tokens
    assert _feature_tagger_training_quarantine_tokens_present(
        "// NativeFeatureTagger NativeFeatureInput FeatureUpstreamStatus\n", tokens
    ) == []


def test_checker_flags_feature_tagger_training_leak(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    target = tmp_path / "crates/da-ports/src/parser.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("fn parse() { let _ = NativeFeatureTagger; }\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp" and "feature_tagger_training_quarantine" in item.evidence]
    assert matching, flags
    assert "NativeFeatureTagger" in matching[0].evidence


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


def test_checker_flags_p5_observe_mode_not_production_stamp(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    stages = payload.setdefault("defaults", {}).setdefault("cascade_stages", {})
    stages.setdefault("P5", {})["observe_mode"] = "accountant_later"
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    assert matching, flags
    assert any("P5" in item.evidence for item in matching)


def test_cascade_observe_tokens_derive_live_and_forbidden_from_data():
    payload = yaml.safe_load((ROOT / "data/parse/grobid_compatibility.yaml").read_text(encoding="utf-8"))
    stages = (payload.get("defaults") or {}).get("cascade_stages") or {}
    required, forbidden = _cascade_observe_tokens(stages)
    assert required == [f"stamp_p{i}_" for i in (0, 1, 2, 3, 4, 5, 6, 7, 8, 9)]
    assert forbidden == []


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


def test_checker_flags_citation_names_pooled_as_biblstruct_count(tmp_path: Path):
    """M495/S01: citation_names_total=2198 — отдельный именованный знаменатель.

    Подмена 2198 на 207 (число biblStruct/references) даёт существующий флаг
    scholarly_parse_plane_acceptance_same_n_drift с evidence по конкретной
    DATA-поверхности; нового violation label нет. Мутация в tmp_path, live
    YAML не трогается."""
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/native_fulltext_composition.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    payload["source_evidence"]["s12_named_denominators"]["citation_names_total"] = 207
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags
    assert any("native_fulltext_composition.yaml" in item.evidence and "citation_names_total" in item.evidence for item in matching)


def test_checker_flags_citation_names_pooling_unlocked(tmp_path: Path):
    """M495/S01: pooling_citation_names_with_references stays forbidden.

    Снятие запрета (allowed) даёт существующий флаг
    scholarly_parse_plane_acceptance_same_n_drift; нового label нет.
    Мутация в tmp_path, live YAML не трогается."""
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/native_fulltext_composition.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    payload["source_evidence"]["s12_named_denominators"]["java_lift_same_n"][
        "pooling_citation_names_with_references"
    ] = "allowed"
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags
    assert any("native_fulltext_composition.yaml" in item.evidence for item in matching)


def test_checker_flags_citation_names_pooled_on_comparison_shadow(tmp_path: Path):
    """M495/S01: comparison shadow держит те же знаменатели и lift-флаги.

    Подмена citation_names_total=2198 на 207 в
    native_fulltext_composition_shadow из comparison_contract.yaml даёт тот же
    scholarly_parse_plane_acceptance_same_n_drift с evidence по shadow-
    поверхности. Ключ selectable здесь — independently_selectable_lanes.
    Мутация в tmp_path, live YAML не трогается."""
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/comparison_contract.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    payload["native_fulltext_composition_shadow"]["source_evidence"]["s12_named_denominators"][
        "citation_names_total"
    ] = 207
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_acceptance_same_n_drift"]
    assert matching, flags
    assert any("native_fulltext_composition_shadow" in item.evidence and "citation_names_total" in item.evidence for item in matching)


def test_checker_flags_undeclared_shadow_sha_clone(tmp_path: Path):
    """M474/S01 + M509/S04: снятие объявленной source-identity валит fail-closed.

    Мутация та же (identity_kind и unique_source_sha256 уходят), но честный
    флаг на этом артефакте другой. Файл
    artifacts/parse_compat/native_fulltext_composition_shadow.json пиннится
    byte-exactly в bindings.canonical_artifacts, поэтому любая перезапись
    (включая json.dumps-перезапись без снятия ключей) даёт дайджест, отличный
    от пина, и плоскость обязана ответить
    scholarly_parse_plane_evidence_substitution
    (expected='tracked source binding'; observed='missing'). Артефакт, чьи байты
    не совпадают с объявленной tracked-привязкой, к семантическим выводам не
    допускается вообще — это и есть проверяемая honesty-свойство.

    Путь acceptance_same_n_drift здесь больше не утверждается: после
    population-сплита (reference/executed/evaluated вместо denominators) снятие
    identity-ключей само по себе его не поднимает, а вернуть снятое поле
    denominators в мутацию значило бы проверять схему, которой у живого
    артефакта уже нет. Живой дрейф acceptance_same_n_drift покрывает
    sibling-мутация test_checker_flags_preserved_names_without_preserved_kind
    (document_id → hub_2507.19457 при synthetic_clone).

    Guard не ослаблен: байтовый sha256-пин в
    scripts/governor/checks/scholarly_parse_plane.yaml остаётся, allowlist для
    shadow-артефакта не добавлен, предикат evidence_substitution не ослаблен,
    покрытие undeclared-shadow-clone сохранено."""
    _copy_checker_surface(tmp_path)
    shadow_path = tmp_path / "artifacts/parse_compat/native_fulltext_composition_shadow.json"
    payload = json.loads(shadow_path.read_text(encoding="utf-8"))
    payload["source_identity"].pop("identity_kind", None)
    payload["source_identity"].pop("unique_source_sha256", None)
    shadow_path.write_text(json.dumps(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_evidence_substitution"]
    assert matching, flags
    assert any("native_fulltext_composition_shadow.json" in item.evidence for item in matching), flags


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


CORPUS_SHADOW_RELATIVE = "artifacts/parse_compat/native_fulltext_corpus_shadow.json"


def _scholarly_flags(flags, name: str):
    return [item for item in flags if item.flag == f"scholarly_parse_plane_{name}"]


def test_checker_flags_missing_corpus_shadow_artifact(tmp_path: Path):
    """S05/T05: the corpus shadow is byte-pinned, so its absence fails closed.

    `native_fulltext_corpus_shadow.json` is declared in both
    `bindings.canonical_artifacts` and `bindings.integration_evidence`; a tree
    that does not carry the artifact must answer `evidence_missing` naming that
    exact path instead of silently dropping the corpus identity surface."""
    _copy_checker_surface(tmp_path)
    (tmp_path / CORPUS_SHADOW_RELATIVE).unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _scholarly_flags(flags, "evidence_missing")
    assert matching, flags
    assert any(CORPUS_SHADOW_RELATIVE in item.evidence for item in matching), flags


def test_checker_flags_truncated_corpus_shadow_artifact(tmp_path: Path):
    """S05/T05: a truncated corpus artifact is a byte substitution, not a pass.

    The digest pin is the byte-level control; truncation must raise
    `evidence_substitution` naming the corpus path before any identity
    inference is attempted."""
    _copy_checker_surface(tmp_path)
    path = tmp_path / CORPUS_SHADOW_RELATIVE
    text = path.read_text(encoding="utf-8")
    path.write_text(text[: len(text) // 2], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _scholarly_flags(flags, "evidence_substitution")
    assert matching, flags
    assert any(CORPUS_SHADOW_RELATIVE in item.evidence for item in matching), flags


def test_checker_flags_corpus_identity_collapse_to_one_sha(tmp_path: Path):
    """S05/T05: preserved_three_sha with unique_source_sha256=1 is drift.

    The predicate that guards the synthetic artifact now also runs over the
    corpus artifact: a three-sha corpus that declares one unique source is
    `acceptance_same_n_drift` with the corpus path in the evidence."""
    _copy_checker_surface(tmp_path)
    path = tmp_path / CORPUS_SHADOW_RELATIVE
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert payload["source_identity"]["identity_kind"] == "preserved_three_sha"
    payload["source_identity"]["unique_source_sha256"] = 1
    path.write_text(json.dumps(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _scholarly_flags(flags, "acceptance_same_n_drift")
    assert matching, flags
    assert any(CORPUS_SHADOW_RELATIVE in item.evidence for item in matching), flags


def test_checker_flags_preserved_corpus_relabelled_synthetic_clone(tmp_path: Path):
    """S05/T05: preserved sha256 in outcomes forbid identity_kind=synthetic_clone.

    `looks_preserved` is data-driven (pinned sha256 set or known corpus document
    ids), so re-labelling the corpus artifact as a synthetic clone is
    `acceptance_same_n_drift` naming the corpus path."""
    _copy_checker_surface(tmp_path)
    path = tmp_path / CORPUS_SHADOW_RELATIVE
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["source_identity"]["identity_kind"] = "synthetic_clone"
    path.write_text(json.dumps(payload), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _scholarly_flags(flags, "acceptance_same_n_drift")
    assert matching, flags
    assert any(CORPUS_SHADOW_RELATIVE in item.evidence for item in matching), flags


def test_checker_flags_corpus_data_digest_shift(tmp_path: Path):
    """S05/T05: the comparison-contract DATA digest is pinned, not assumed.

    Shifting `bindings.source_data` sha256 for
    `data/parse/comparison_contract.yaml` in the policy must raise
    `evidence_substitution` naming that DATA path, so the corpus binding cannot
    survive a DATA edit without a re-pin."""
    _copy_checker_surface(tmp_path)
    policy_path = tmp_path / "scripts/governor/checks/scholarly_parse_plane.yaml"
    payload = yaml.safe_load(policy_path.read_text(encoding="utf-8"))
    entry = next(
        item
        for item in payload["bindings"]["source_data"]
        if item["path"] == "data/parse/comparison_contract.yaml"
    )
    entry["sha256"] = "0" * 64
    policy_path.write_text(yaml.safe_dump(payload, sort_keys=False), encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _scholarly_flags(flags, "evidence_substitution")
    assert matching, flags
    assert any("data/parse/comparison_contract.yaml" in item.evidence for item in matching), flags


def test_corpus_shadow_binding_is_mirrored_and_identity_clean():
    """S05/T05: both governor surfaces bind the corpus artifact and it is clean.

    The policy `canonical_artifacts`/`integration_evidence` entries, the fixture
    mirror and the on-disk digest must agree; and the live root must produce no
    scholarly-parse flag naming the corpus artifact. That the new loop does not
    weaken the synthetic artifact is covered by
    test_checker_flags_preserved_names_without_preserved_kind and
    test_checker_flags_undeclared_shadow_sha_clone."""
    policy = _load(POLICY_PATH)
    fixture = _load(POSITIVE_PATH)
    canonical = next(
        item
        for item in policy["bindings"]["canonical_artifacts"]
        if item["path"] == CORPUS_SHADOW_RELATIVE
    )
    assert canonical["sha256"] == _sha256(ROOT / CORPUS_SHADOW_RELATIVE)
    mirrored = next(
        item for item in fixture["canonical_artifacts"] if item["path"] == CORPUS_SHADOW_RELATIVE
    )
    assert mirrored["sha256"] == canonical["sha256"]
    assert mirrored["identity"] == "native-fulltext-corpus-shadow.v1"
    evidence = next(
        item
        for item in policy["bindings"]["integration_evidence"]
        if item["path"] == CORPUS_SHADOW_RELATIVE
    )
    assert evidence["identity"] == "native-fulltext-corpus-shadow.v1"
    assert any(
        item["path"] == CORPUS_SHADOW_RELATIVE
        and item["identity"] == "native-fulltext-corpus-shadow.v1"
        for item in fixture["integration_evidence"]
    )
    flags = _check_scholarly_parse_plane(ROOT)
    corpus_flags = [item for item in flags if CORPUS_SHADOW_RELATIVE in item.evidence]
    assert corpus_flags == [], [str(item) for item in corpus_flags]


BATCH_BUDGET_SHADOW_RELATIVE = "artifacts/parse_compat/parse_batch_budget_shadow.json"


def test_checker_flags_missing_batch_budget_shadow_artifact(tmp_path: Path):
    """S06/T06: the batch-budget shadow is byte-pinned, so its absence fails closed.

    `parse_batch_budget_shadow.json` is declared in both
    `bindings.canonical_artifacts` and `bindings.integration_evidence`; a tree
    that does not carry the artifact must answer `evidence_missing` naming that
    exact path instead of silently dropping the H05/H19 measurement surface."""
    _copy_checker_surface(tmp_path)
    (tmp_path / BATCH_BUDGET_SHADOW_RELATIVE).unlink()
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _scholarly_flags(flags, "evidence_missing")
    assert matching, flags
    assert any(BATCH_BUDGET_SHADOW_RELATIVE in item.evidence for item in matching), flags


def test_checker_flags_truncated_batch_budget_shadow_artifact(tmp_path: Path):
    """S06/T06: a truncated budget artifact is a byte substitution, not a pass.

    The digest pin is the byte-level control; truncation must raise
    `evidence_substitution` naming the budget path before any measurement
    inference is attempted."""
    _copy_checker_surface(tmp_path)
    path = tmp_path / BATCH_BUDGET_SHADOW_RELATIVE
    text = path.read_text(encoding="utf-8")
    path.write_text(text[: len(text) // 2], encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _scholarly_flags(flags, "evidence_substitution")
    assert matching, flags
    assert any(BATCH_BUDGET_SHADOW_RELATIVE in item.evidence for item in matching), flags


def test_batch_budget_shadow_binding_is_mirrored_and_identity_clean():
    """S06/T06: both governor surfaces bind the budget artifact and it is clean.

    The policy `canonical_artifacts`/`integration_evidence` entries, the fixture
    mirror and the on-disk digest must agree; and the live root must produce no
    scholarly-parse flag naming the budget artifact. The selectable-set is not
    widened: the artifact carries a measurement identity, not a new decision
    point (R125 `independently_selectable` stays [header, references])."""
    policy = _load(POLICY_PATH)
    fixture = _load(POSITIVE_PATH)
    canonical = next(
        item
        for item in policy["bindings"]["canonical_artifacts"]
        if item["path"] == BATCH_BUDGET_SHADOW_RELATIVE
    )
    assert canonical["sha256"] == _sha256(ROOT / BATCH_BUDGET_SHADOW_RELATIVE)
    mirrored = next(
        item
        for item in fixture["canonical_artifacts"]
        if item["path"] == BATCH_BUDGET_SHADOW_RELATIVE
    )
    assert mirrored["sha256"] == canonical["sha256"]
    assert mirrored["identity"] == "parse-batch-budget-shadow.v1"
    evidence = next(
        item
        for item in policy["bindings"]["integration_evidence"]
        if item["path"] == BATCH_BUDGET_SHADOW_RELATIVE
    )
    assert evidence["identity"] == "parse-batch-budget-shadow.v1"
    assert any(
        item["path"] == BATCH_BUDGET_SHADOW_RELATIVE
        and item["identity"] == "parse-batch-budget-shadow.v1"
        for item in fixture["integration_evidence"]
    )
    selectable = policy["rest_batch_asset_quarantine"]["policy"]["independently_selectable"]
    assert selectable == ["header", "references"]
    flags = _check_scholarly_parse_plane(ROOT)
    budget_flags = [item for item in flags if BATCH_BUDGET_SHADOW_RELATIVE in item.evidence]
    assert budget_flags == [], [str(item) for item in budget_flags]


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

HEADER_QUARANTINE_TOKENS = [
    "compose_header",
    "run_header_sequence",
    "NativeDateParser",
    "decode_header_sequence",
    "render_header_features",
    "render_header_feature_text",
    "project_native_header_fields",
]
HEADER_QUARANTINE_BOUNDARIES = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
]
HEADER_QUARANTINE_PAYLOADS = {
    "compose_header": "let _h = compose_header(input);\n",
    "run_header_sequence": "let _h = run_header_sequence(input);\n",
    "NativeDateParser": "let _p = NativeDateParser::new();\n",
    "decode_header_sequence": "let _d = decode_header_sequence(rows);\n",
    "render_header_features": "let _r = render_header_features(tokens);\n",
    "render_header_feature_text": "let _t = render_header_feature_text(tokens);\n",
    "project_native_header_fields": "let _f = project_native_header_fields(rows, spans);\n",
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


def test_header_sequence_quarantine_is_data_driven_and_comment_safe():
    defaults = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"]
    tokens, boundaries = _header_sequence_quarantine_spec(defaults)
    assert tokens == [
        "compose_header",
        "run_header_sequence",
        "NativeDateParser",
        "decode_header_sequence",
        "render_header_features",
        "render_header_feature_text",
        "project_native_header_fields",
    ]
    assert boundaries == [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
    ]
    # Missing or malformed DATA never invents a forbidden set or boundaries:
    # the existing SHA/evidence_missing signals own the missing-ledger surface.
    assert _header_sequence_quarantine_spec(None) == ([], [])
    assert _header_sequence_quarantine_spec({}) == ([], [])
    assert _header_sequence_quarantine_spec({"header_sequence_quarantine": {}}) == ([], [])
    assert _header_sequence_quarantine_spec({"header_sequence_quarantine": {"tokens": [1]}}) == ([] , [])

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

# ───── S04/T08: native bibliography/callout lane stays shadow, out of four boundaries ─────
#
# Additive to the S07/T01 bibliography quarantine (MEM1115 / D362): the older
# tuple keeps ONLY snake_case call-site atoms while the new DATA key carries the
# S04 lane's own atoms, including its PascalCase row/span/role types. The
# ParserPort type/method prohibition still lives on the Rust side in
# crates/da-parse/tests/architecture_contract.rs.

EXPECTED_BIBLIOGRAPHY_SEQUENCE_TOKENS = [
    "citation_row",
    "reference_segmenter_row",
    "name_citation_row",
    "date_row",
    "render_citation_feature_text",
    "render_reference_segmenter_feature_text",
    "render_name_citation_feature_text",
    "render_date_feature_text",
    "decode_bibliography_labels",
    "decode_reference_sequence",
    "decode_citation_sequence",
    "decode_name_citation_cluster",
    "decode_date_cluster",
    "bibliography_label_runs",
    "gazetteer_spans",
    "classify_callout_markers",
    "link_callout_markers",
    "CitationFeatureRow",
    "ReferenceSegmenterFeatureRow",
    "BibliographySequenceFamily",
    "BibliographyLabelRun",
    "ReferenceSegmentSpan",
    "CitationRoleSpan",
    "NameCitationFeatureRow",
    "DateFeatureRow",
    "NativeNameCitation",
    "NativeDateValue",
    "NativePersonTitleCoverage",
    "CalloutMarkerClass",
    "CalloutBodyInventory",
    "CalloutLinkReport",
    "BibliographyGazetteerIndex",
]
EXPECTED_BIBLIOGRAPHY_SEQUENCE_BOUNDARIES = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/projector.rs",
]

def _bibliography_sequence_quarantine_flags(flags):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if "#bibliography_sequence_quarantine." in item.evidence]

def test_bibliography_sequence_quarantine_is_data_bound_and_closed():
    registry = _load(ROOT / "data/parse/grobid_compatibility.yaml")
    defaults = registry["defaults"]
    tokens, boundaries = _bibliography_sequence_quarantine_spec(defaults)
    assert tokens == EXPECTED_BIBLIOGRAPHY_SEQUENCE_TOKENS
    assert boundaries == EXPECTED_BIBLIOGRAPHY_SEQUENCE_BOUNDARIES
    spec = defaults["bibliography_sequence_quarantine"]
    assert spec["production_authority"] == "java_http"
    assert spec["observe_mode"] == "shadow_receipt"
    assert spec["production_route"] == "forbidden"
    assert spec["graph_write"] == "forbidden"
    assert spec["import_eligible"] is False
    assert spec["auto_promote"] == "forbidden"
    assert spec["deployed"] is False
    assert spec["decision_point"] == 6
    assert spec["independently_selectable"] == ["header", "references"]
    assert spec["proof_level"] == "contract"
    assert spec["seam_reexport"] == "crates/da-ports/src/scalar_crf.rs"
    assert spec["independent_oracle"] == "crates/da-parse/tests/native_bibliography_same_n.rs"
    assert spec["real_model_harness"] == "crates/da-adapters/tests/native_bibliography_real_model.rs"
    assert spec["offline_limits"]
    # Closed lanes: no promoted lane, no pool, no invented graph surface.
    assert {lane["path"] for lane in spec["native_surfaces"]} == {
        "crates/da-parse/src/native_feature_facts.rs",
        "crates/da-parse/src/native_bibliography_gazetteer.rs",
        "crates/da-parse/src/native_bibliography_features.rs",
        "crates/da-parse/src/native_bibliography_sequence.rs",
        "crates/da-parse/src/native_bibliography_nested.rs",
        "crates/da-parse/src/callout_markers.rs",
    }
    # The S07 tuple is untouched by S04/T08: snake_case call-site atoms only.
    assert BIBLIOGRAPHY_QUARANTINE_TOKENS == ["compose_references", "run_reference_sequence"]
    assert all("_" in token for token in BIBLIOGRAPHY_QUARANTINE_TOKENS)
    # Both polarities of the per-token predicate.
    assert _bibliography_sequence_quarantine_tokens_present(
        "let _r = citation_row(tokens); let _x: CitationFeatureRow = row;", tokens
    ) == ["citation_row", "CitationFeatureRow"]
    assert _bibliography_sequence_quarantine_tokens_present(
        "let _r = decompose_citation_row(tokens); let _x = CitationFeatureRowExtra;", tokens
    ) == []
    assert _bibliography_sequence_quarantine_tokens_present(
        "fn f() { citation_row (tokens); }", tokens
    ) == ["citation_row"]

def test_bibliography_sequence_quarantine_spec_is_silent_without_data():
    # Missing or malformed DATA never invents a forbidden set or boundaries.
    assert _bibliography_sequence_quarantine_spec(None) == ([], [])
    assert _bibliography_sequence_quarantine_spec({}) == ([], [])
    assert _bibliography_sequence_quarantine_spec({"bibliography_sequence_quarantine": {}}) == ([], [])
    assert _bibliography_sequence_quarantine_spec(
        {"bibliography_sequence_quarantine": {"tokens": [1]}}
    ) == ([], [])
    assert _bibliography_sequence_quarantine_spec(
        {"bibliography_sequence_quarantine": {"tokens": ["citation_row", "  "]}}
    ) == (["citation_row"], [])

@pytest.mark.parametrize("token", EXPECTED_BIBLIOGRAPHY_SEQUENCE_TOKENS)
def test_checker_flags_bibliography_sequence_atom_in_every_boundary(tmp_path: Path, token: str):
    _copy_checker_surface(tmp_path)
    payload = (
        f"let _x = {token}(input);\n"
        if "_" in token
        else f"let _x: {token} = build();\n"
    )
    for relative in EXPECTED_BIBLIOGRAPHY_SEQUENCE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _bibliography_sequence_quarantine_flags(flags)
    assert len(matching) == len(EXPECTED_BIBLIOGRAPHY_SEQUENCE_BOUNDARIES), flags
    assert all(f"#bibliography_sequence_quarantine.{token}" in item.evidence for item in matching), flags
    # The S07 namespace stays independent: this lane never flags it.
    assert _bibliography_quarantine_flags(flags) == [], flags

@pytest.mark.parametrize(
    "payload",
    [
        "// citation_row is shadow-only prose\n",
        "// decode_reference_sequence is shadow-only prose\n",
        "/// doc CitationFeatureRow prose\n",
        "let x = 1; // bibliography_label_runs prose\n",
        "let _r = decompose_citation_row(input);\n",
        "let _x = CitationFeatureRowExtra;\n",
        "citation_row is prose without a call\n",
    ],
)
def test_checker_bibliography_sequence_quarantine_ignores_prose_and_prefixed(tmp_path: Path, payload: str):
    _copy_checker_surface(tmp_path)
    for relative in EXPECTED_BIBLIOGRAPHY_SEQUENCE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _bibliography_sequence_quarantine_flags(flags) == [], flags

def test_checker_clean_root_has_no_bibliography_sequence_quarantine_flags():
    assert _bibliography_sequence_quarantine_flags(_check_scholarly_parse_plane(ROOT)) == []

def test_checker_bibliography_sequence_quarantine_stays_silent_without_ledger(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    (tmp_path / "data/parse/grobid_compatibility.yaml").unlink()
    target = tmp_path / EXPECTED_BIBLIOGRAPHY_SEQUENCE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("let _x = citation_row(input);\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _bibliography_sequence_quarantine_flags(flags) == [], flags
    assert any(
        item.flag
        in {
            "scholarly_parse_plane_evidence_missing",
            "scholarly_parse_plane_backend_identity",
        }
        for item in flags
    ), flags

# ───── S05/T06: the native body/fulltext lane stays shadow, out of four production boundaries ─────

FULLTEXT_SEQUENCE_QUARANTINE_TOKENS = [
    "render_fulltext_features",
    "render_fulltext_feature_text",
    "check_fulltext_feature_columns",
    "relative_position_fulltext_document",
    "relative_position_fulltext_page",
    "decode_fulltext_sequence",
    "check_fulltext_label_block",
    "fulltext_label_is_admissible",
    "glue_fulltext_reading_order",
    "cross_page_runs",
    "native_body_version_surface",
    "FulltextAlignment",
    "FulltextCalloutType",
    "FulltextFeatureRow",
    "FulltextFeatureToken",
    "FulltextFontSize",
    "FulltextFontStatus",
    "NativeFulltextFeatureError",
    "FulltextLabelRun",
    "NativeFulltextDecodeCandidate",
    "NativeFulltextDecodeOutcome",
    "NativeFulltextSequenceError",
    "ColumnShift",
    "CrossPageRun",
    "NativeFulltextPageToken",
    "NativeFulltextReadingOrder",
    "NativeFulltextReadingOrderError",
    "NativeFulltextTokenText",
    "PageReadingSpan",
    "NativeBodyStep",
    "NativeBodyStepOutcome",
    "NativeBodyStepReceipt",
    "NativeBodySurfaceKind",
    "NativeBodyVersionSurface",
    "NativeLanguageStep",
    "NativeSentenceStep",
]
EXPECTED_FULLTEXT_SEQUENCE_BOUNDARIES = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/projector.rs",
]

def _fulltext_sequence_quarantine_flags(flags):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if "#fulltext_sequence_quarantine." in item.evidence]

def test_fulltext_sequence_quarantine_is_data_bound_and_closed():
    registry = _load(ROOT / "data/parse/grobid_compatibility.yaml")
    defaults = registry["defaults"]
    tokens, boundaries = _fulltext_sequence_quarantine_spec(defaults)
    assert tokens == FULLTEXT_SEQUENCE_QUARANTINE_TOKENS
    assert boundaries == EXPECTED_FULLTEXT_SEQUENCE_BOUNDARIES
    spec = defaults["fulltext_sequence_quarantine"]
    assert spec["production_authority"] == "java_http"
    assert spec["observe_mode"] == "shadow_receipt"
    assert spec["production_route"] == "forbidden"
    assert spec["graph_write"] == "forbidden"
    assert spec["import_eligible"] is False
    assert spec["auto_promote"] == "forbidden"
    assert spec["deployed"] is False
    assert spec["decision_point"] == 6
    assert spec["independently_selectable"] == ["header", "references"]
    assert spec["proof_level"] == "contract"
    assert spec["seam_reexport"] == "crates/da-ports/src/scalar_crf.rs"
    assert spec["independent_oracle"] == "crates/da-parse/tests/native_fulltext_same_n.rs"
    assert spec["real_model_harness"] == "crates/da-adapters/tests/native_fulltext_real_model.rs"
    assert spec["offline_limits"]
    # The 27-column ABI is this lane's own width, not a copy of 29 or 30.
    surfaces = {surface["path"]: surface for surface in spec["native_surfaces"]}
    assert set(surfaces) == {
        "crates/da-parse/src/native_fulltext_features.rs",
        "crates/da-parse/src/native_fulltext_sequence.rs",
        "crates/da-parse/src/native_fulltext_reading_order.rs",
        "crates/da-parse/src/native_fulltext_sentence.rs",
    }
    assert surfaces["crates/da-parse/src/native_fulltext_features.rs"]["abi_columns"] == 27
    assert surfaces["crates/da-parse/src/native_fulltext_sequence.rs"]["base_labels"] == 12
    assert surfaces["crates/da-parse/src/native_fulltext_sequence.rs"]["label_spellings"] == 24
    assert surfaces["crates/da-parse/src/native_fulltext_sentence.rs"]["crf_family"] is False
    # Both polarities of the per-token predicate.
    assert _fulltext_sequence_quarantine_tokens_present(
        "let _r = decode_fulltext_sequence(rows); let _x: FulltextFeatureRow = row;", tokens
    ) == ["decode_fulltext_sequence", "FulltextFeatureRow"]
    assert _fulltext_sequence_quarantine_tokens_present(
        "let _r = decode_fulltext_sequence_legacy(rows); let _x = FulltextFeatureRowBuilder;", tokens
    ) == []
    assert _fulltext_sequence_quarantine_tokens_present(
        "fn f() { glue_fulltext_reading_order (pages); }", tokens
    ) == ["glue_fulltext_reading_order"]

def test_fulltext_sequence_quarantine_is_distinct_from_the_other_fulltext_keys():
    # D362: `fulltext_blocks_quarantine` (P2 region accounting),
    # `body_accounting_quarantine` (M496 P5 accountant) and this S05 body
    # reading-order lane are three lanes; none may be merged into another.
    defaults = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"]
    sequence_tokens = set(_fulltext_sequence_quarantine_spec(defaults)[0])
    blocks_tokens = set(defaults["fulltext_blocks_quarantine"]["tokens"])
    accountant_tokens = set(defaults["body_accounting_quarantine"]["tokens"])
    assert sequence_tokens.isdisjoint(blocks_tokens)
    assert sequence_tokens.isdisjoint(accountant_tokens)
    assert blocks_tokens.isdisjoint(accountant_tokens)
    assert len(sequence_tokens) == 36
    assert len(blocks_tokens) == 3
    assert len(accountant_tokens) == 4
    # The older two keys keep their own (narrower) boundary sets untouched.
    assert defaults["body_accounting_quarantine"]["boundaries"] == [
        "crates/da-cli/src/source_cli.rs",
        "crates/da-ports/src/parser.rs",
    ]

def test_fulltext_sequence_quarantine_spec_is_silent_without_data():
    # Missing or malformed DATA never invents a forbidden set or boundaries.
    assert _fulltext_sequence_quarantine_spec(None) == ([], [])
    assert _fulltext_sequence_quarantine_spec({}) == ([], [])
    assert _fulltext_sequence_quarantine_spec({"fulltext_sequence_quarantine": {}}) == ([], [])
    assert _fulltext_sequence_quarantine_spec(
        {"fulltext_sequence_quarantine": {"tokens": [1]}}
    ) == ([], [])
    assert _fulltext_sequence_quarantine_spec(
        {"fulltext_sequence_quarantine": {"tokens": ["decode_fulltext_sequence", "  "]}}
    ) == (["decode_fulltext_sequence"], [])

@pytest.mark.parametrize("token", FULLTEXT_SEQUENCE_QUARANTINE_TOKENS)
def test_checker_flags_fulltext_sequence_atom_in_every_boundary(tmp_path: Path, token: str):
    _copy_checker_surface(tmp_path)
    payload = (
        f"let _x = {token}(input);\n"
        if "_" in token
        else f"let _x: {token} = build();\n"
    )
    for relative in EXPECTED_FULLTEXT_SEQUENCE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _fulltext_sequence_quarantine_flags(flags)
    assert len(matching) == len(EXPECTED_FULLTEXT_SEQUENCE_BOUNDARIES), flags
    assert all(f"#fulltext_sequence_quarantine.{token}" in item.evidence for item in matching), flags
    # Adjacent lanes stay independent: this lane never flags the other two keys.
    assert _bibliography_sequence_quarantine_flags(flags) == [], flags

@pytest.mark.parametrize(
    "payload",
    [
        "// decode_fulltext_sequence is shadow-only prose\n",
        "/// doc FulltextFeatureRow prose\n",
        "let x = 1; // render_fulltext_features prose\n",
        "let _r = decode_fulltext_sequence_legacy(input);\n",
        "let _x = FulltextFeatureRowBuilder;\n",
        "render_fulltext_features is prose without a call\n",
        "// NativeFulltextReadingOrderError prose\n",
    ],
)
def test_checker_fulltext_sequence_quarantine_ignores_prose_and_prefixed(tmp_path: Path, payload: str):
    _copy_checker_surface(tmp_path)
    for relative in EXPECTED_FULLTEXT_SEQUENCE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _fulltext_sequence_quarantine_flags(flags) == [], flags

def test_checker_clean_root_has_no_fulltext_sequence_quarantine_flags():
    assert _fulltext_sequence_quarantine_flags(_check_scholarly_parse_plane(ROOT)) == []

def test_checker_fulltext_sequence_quarantine_stays_silent_without_ledger(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    (tmp_path / "data/parse/grobid_compatibility.yaml").unlink()
    target = tmp_path / EXPECTED_FULLTEXT_SEQUENCE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("let _x = decode_fulltext_sequence(input);\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _fulltext_sequence_quarantine_flags(flags) == [], flags
    assert any(
        item.flag
        in {
            "scholarly_parse_plane_evidence_missing",
            "scholarly_parse_plane_backend_identity",
        }
        for item in flags
    ), flags

# ───── S06/T08: visual-family and funding lanes stay shadow, out of four production boundaries ─────

# The Rust mirror lives in `crates/da-parse/tests/architecture_contract.rs`
# (`VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS` / `FUNDING_SEQUENCE_QUARANTINE_TOKENS`),
# so a dropped DATA key breaks BOTH sides and can never be quietly accepted.
VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS = [
    "cluster_family_label_runs",
    "caption_links_from_markers",
    "decode_figure_sequence",
    "check_figure_label_block",
    "check_figure_column_window",
    "figure_spans_from_decoded",
    "figure_label_is_admissible",
    "decode_table_sequence",
    "check_table_label_block",
    "check_table_column_window",
    "table_spans_from_decoded",
    "table_cells_from_inventory",
    "table_revert_reason",
    "cluster_opens_new_table",
    "table_label_is_admissible",
    "join_native_equations",
    "join_native_equation_labels",
    "is_formula_join_label",
    "FamilyLabelCluster",
    "NativeVisualFamilyClusterError",
    "NativeCaptionMarker",
    "NativeCaptionLinks",
    "NativeCaptionLinkAbsence",
    "FigureSpan",
    "NativeFigureAbsence",
    "NativeFigureDecodeOutcome",
    "NativeFigureSequenceError",
    "NativeTableSpan",
    "NativeTableCell",
    "NativeTableCells",
    "NativeTableAbsence",
    "NativeTableRevertReason",
    "NativeTableDecodeOutcome",
    "NativeTableSequenceError",
    "NativeEquationSpan",
    "NativeFormulaAbsence",
    "NativeFormulaJoinError",
    "NativeFormulaJoinOutcome",
]
FUNDING_SEQUENCE_QUARANTINE_TOKENS = [
    "render_funding_features",
    "render_funding_feature_text",
    "check_funding_feature_columns",
    "validate_funding_gazetteer_coverage",
    "decode_funding_sequence",
    "check_funding_label_block",
    "check_funding_column_window",
    "funding_entries_from_decoded",
    "funding_label_is_admissible",
    "funding_role_of_label",
    "FundingFeatureRow",
    "FundingFeatureToken",
    "FundingLineStatus",
    "FundingCapitalisation",
    "FundingDigit",
    "FundingPunctType",
    "NativeFundingFeatureError",
    "NativeFundingGazetteerCoverage",
    "NativeFundingGazetteerLane",
    "NativeFundingGazetteerSpan",
    "NativeFundingEntry",
    "NativeFundingRole",
    "NativeFundingAbsence",
    "NativeFundingDecodeOutcome",
    "NativeFundingSequenceError",
]
FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES = [
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
    "crates/da-parse/src/parser.rs",
    "crates/da-parse/src/projector.rs",
]
FAMILY_SEQUENCE_QUARANTINE_KEYS = {
    "visual_family_sequence_quarantine": (
        _visual_family_sequence_quarantine_spec,
        _visual_family_sequence_quarantine_tokens_present,
        VISUAL_FAMILY_SEQUENCE_QUARANTINE_TOKENS,
    ),
    "funding_sequence_quarantine": (
        _funding_sequence_quarantine_spec,
        _funding_sequence_quarantine_tokens_present,
        FUNDING_SEQUENCE_QUARANTINE_TOKENS,
    ),
}
FAMILY_SEQUENCE_QUARANTINE_TOKEN_CASES = [
    (key, token)
    for key, (_spec, _present, tokens) in sorted(FAMILY_SEQUENCE_QUARANTINE_KEYS.items())
    for token in tokens
]


def _family_sequence_quarantine_flags(flags, key: str):
    matching = [item for item in flags if item.flag == "scholarly_parse_plane_policy_stamp"]
    return [item for item in matching if f"#{key}." in item.evidence]


def _other_family_key(key: str) -> str:
    return next(candidate for candidate in FAMILY_SEQUENCE_QUARANTINE_KEYS if candidate != key)


@pytest.mark.parametrize("key", sorted(FAMILY_SEQUENCE_QUARANTINE_KEYS))
def test_family_sequence_quarantine_is_data_bound_and_closed(key: str):
    spec, _present, expected = FAMILY_SEQUENCE_QUARANTINE_KEYS[key]
    defaults = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"]
    tokens, boundaries = spec(defaults)
    assert tokens == expected
    assert boundaries == FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES
    assert len(set(tokens)) == len(tokens)
    spec_payload = defaults[key]
    assert spec_payload["production_authority"] == "java_http"
    assert spec_payload["observe_mode"] == "shadow_receipt"
    assert spec_payload["production_route"] == "forbidden"
    assert spec_payload["graph_write"] == "forbidden"
    assert spec_payload["import_eligible"] is False
    assert spec_payload["auto_promote"] == "forbidden"
    assert spec_payload["deployed"] is False
    assert spec_payload["decision_point"] == 6
    assert spec_payload["independently_selectable"] == ["header", "references"]
    assert spec_payload["proof_level"] == "contract"
    assert spec_payload["data_binding"] == "data/parse/native_visual_families.yaml"
    assert spec_payload["schema_version"] == "native-visual-families.v1"
    assert spec_payload["bundled_model_bytes"] is False
    assert spec_payload["model_root_env"] == "DA_GROBID_MODEL_ROOT"
    assert spec_payload["independent_oracle"] == "crates/da-parse/tests/native_visual_family_same_n.rs"
    assert spec_payload["real_model_harness"] == "crates/da-adapters/tests/native_visual_family_real_model.rs"
    assert spec_payload["offline_limits"]


def test_visual_family_sequence_quarantine_pins_the_fulltext_abi():
    # figure/table re-use the S05 fulltext ABI unchanged: 27 emitted columns and
    # only 0..=25 referenced by the frozen models' own templates.
    spec_payload = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"][
        "visual_family_sequence_quarantine"
    ]
    assert spec_payload["emitted_columns"] == 27
    assert spec_payload["referenced_columns"] == 26
    assert spec_payload["grouping_rule"] == "tagging_token_clusteror_is_beginning"
    assert spec_payload["caption_link_resolve_owner"] == "native_callout_matching"
    assert spec_payload["caption_link_empty_inventory"] == "not_observed"


def test_funding_sequence_quarantine_pins_its_own_abi():
    spec_payload = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"][
        "funding_sequence_quarantine"
    ]
    assert spec_payload["emitted_columns"] == 17
    assert spec_payload["referenced_columns"] == 17
    assert spec_payload["label_column_index"] == 17
    assert spec_payload["label_column_value"] == "0"
    assert spec_payload["base_labels"] == 10
    assert spec_payload["label_spellings"] == 20
    assert spec_payload["gazetteer_lanes"] == ["knownFunder", "knownInfrastructure"]
    assert spec_payload["gazetteer_coverage_source"] == "caller_supplied_typed_spans"


def test_family_sequence_quarantines_are_distinct_from_every_sibling_key():
    # D362: name adjacency is not inheritance. The two S06 keys, the S08
    # visual-head key, the P2 region key and the two accountant keys are six
    # lanes; none may be merged into another.
    defaults = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"]
    visual = set(_visual_family_sequence_quarantine_spec(defaults)[0])
    funding = set(_funding_sequence_quarantine_spec(defaults)[0])
    assert len(visual) == 38
    assert len(funding) == 25
    assert visual.isdisjoint(funding)
    for sibling in (
        "visual_heads_quarantine",
        "fulltext_blocks_quarantine",
        "body_accounting_quarantine",
        "fulltext_sequence_quarantine",
        "callout_matching_quarantine",
    ):
        sibling_tokens = set(defaults[sibling]["tokens"])
        assert sibling_tokens
        assert visual.isdisjoint(sibling_tokens), sibling
        assert funding.isdisjoint(sibling_tokens), sibling


@pytest.mark.parametrize("key", sorted(FAMILY_SEQUENCE_QUARANTINE_KEYS))
def test_family_sequence_quarantine_spec_is_silent_without_data(key: str):
    # Missing or malformed DATA never invents a forbidden set or boundaries.
    spec, _present, expected = FAMILY_SEQUENCE_QUARANTINE_KEYS[key]
    assert spec(None) == ([], [])
    assert spec({}) == ([], [])
    assert spec({key: {}}) == ([], [])
    assert spec({key: {"tokens": [1]}}) == ([], [])
    assert spec({key: {"tokens": [expected[0], "  "]}}) == ([expected[0]], [])


@pytest.mark.parametrize("key", sorted(FAMILY_SEQUENCE_QUARANTINE_KEYS))
def test_family_sequence_quarantine_removed_key_is_not_silently_ignored(key: str):
    # The checker is fail-closed on the ledger: without DATA it stays silent.
    # That is exactly why the DATA<->Rust<->pytest equality assertions exist —
    # a removed key is caught there, never quietly accepted as "no atoms".
    spec, _present, expected = FAMILY_SEQUENCE_QUARANTINE_KEYS[key]
    defaults = _load(ROOT / "data/parse/grobid_compatibility.yaml")["defaults"]
    without_key = {name: value for name, value in defaults.items() if name != key}
    assert spec(without_key) == ([], [])
    assert spec(without_key)[0] != expected


@pytest.mark.parametrize("key,token", FAMILY_SEQUENCE_QUARANTINE_TOKEN_CASES)
def test_checker_flags_family_sequence_atom_in_every_boundary(tmp_path: Path, key: str, token: str):
    _copy_checker_surface(tmp_path)
    payload = (
        f"let _x = {token}(input);\n"
        if "_" in token
        else f"let _x: {token} = build();\n"
    )
    for relative in FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    matching = _family_sequence_quarantine_flags(flags, key)
    assert len(matching) == len(FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES), flags
    assert all(f"#{key}.{token}" in item.evidence for item in matching), flags
    # Adjacent lanes stay independent: the other S06 key and the sibling
    # quarantines never flag this atom.
    assert _family_sequence_quarantine_flags(flags, _other_family_key(key)) == [], flags
    assert _fulltext_sequence_quarantine_flags(flags) == [], flags
    assert _visual_heads_quarantine_flags(flags) == [], flags
    assert _bibliography_sequence_quarantine_flags(flags) == [], flags


@pytest.mark.parametrize("key", sorted(FAMILY_SEQUENCE_QUARANTINE_KEYS))
@pytest.mark.parametrize(
    "payload",
    [
        "// decode_figure_sequence is shadow-only prose\n",
        "/// doc NativeCaptionLinks prose\n",
        "let x = 1; // render_funding_features prose\n",
        "let _r = decode_figure_sequence_legacy(input);\n",
        "let _f = decode_funding_sequence_legacy(input);\n",
        "let _x = NativeCaptionLinksBuilder;\n",
        "let _x = FundingFeatureRowExtra;\n",
        "decode_table_sequence is prose without a call\n",
        "// NativeFundingGazetteerCoverage prose\n",
    ],
)
def test_checker_family_sequence_quarantine_ignores_prose_and_prefixed(
    tmp_path: Path, key: str, payload: str
):
    _copy_checker_surface(tmp_path)
    for relative in FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _family_sequence_quarantine_flags(flags, key) == [], flags


@pytest.mark.parametrize("key", sorted(FAMILY_SEQUENCE_QUARANTINE_KEYS))
def test_checker_clean_root_has_no_family_sequence_quarantine_flags(key: str):
    assert _family_sequence_quarantine_flags(_check_scholarly_parse_plane(ROOT), key) == []


@pytest.mark.parametrize("key", sorted(FAMILY_SEQUENCE_QUARANTINE_KEYS))
def test_checker_family_sequence_quarantine_stays_silent_without_ledger(
    tmp_path: Path, key: str
):
    _copy_checker_surface(tmp_path)
    (tmp_path / "data/parse/grobid_compatibility.yaml").unlink()
    target = tmp_path / FAMILY_SEQUENCE_QUARANTINE_BOUNDARIES[0]
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(f"let _x = {FAMILY_SEQUENCE_QUARANTINE_KEYS[key][2][0]}(input);\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    assert _family_sequence_quarantine_flags(flags, key) == [], flags
    assert any(
        item.flag
        in {
            "scholarly_parse_plane_evidence_missing",
            "scholarly_parse_plane_backend_identity",
        }
        for item in flags
    ), flags

# ───── S08/T01: visual heads stay shadow, out of parse-pdf source_cli and ParserPort ─────

VISUAL_HEADS_QUARANTINE_TOKENS = [
    "detect_figure_heads",
    "detect_table_heads",
    "detect_formula_heads",
    "run_visual_heads_sequence",
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
    "run_visual_heads_sequence": "let _v = run_visual_heads_sequence(input);\n",
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


# ───── S07/T04: the parity ledger's denominators are held on the governor surface ─────
# The carrier `data/parse/grobid_compatibility.yaml` is the only place these numbers
# live, and its digest is pinned on three declared surfaces: the policy's
# `rest_batch_asset_quarantine.source_bindings`, the policy's `bindings.source_data`,
# and the positive fixture's `source_data`. The Rust contract owns the per-row
# three-link binding (java sha256 + rust symbol + independent test); the governor
# surface owns the counter block, so a substituted denominator is a named flag
# rather than a cleaner-looking report.

PARITY_LEDGER_CONTRACT = "grobid-java-rust-parity-ledger.v1"


def _parity_ledger_flags(flags):
    return [
        item
        for item in flags
        if item.flag.startswith("scholarly_parse_plane_parity_ledger")
    ]


def _pinned_digests(node, relative: str) -> list:
    """Every `{path, sha256}` record for `relative`, wherever it is nested."""
    found = []
    if isinstance(node, dict):
        if node.get("path") == relative and "sha256" in node:
            found.append(str(node["sha256"]))
        for value in node.values():
            found.extend(_pinned_digests(value, relative))
    elif isinstance(node, list):
        for item in node:
            found.extend(_pinned_digests(item, relative))
    return found


def test_parity_carrier_is_pinned_to_its_disk_digest_on_every_declared_surface():
    carrier = "data/parse/grobid_compatibility.yaml"
    disk = _sha256(ROOT / carrier)
    policy_pins = _pinned_digests(_load(POLICY_PATH), carrier)
    # Both policy surfaces pin the carrier: the rest-batch source bindings and the
    # source_data list the positive fixture mirrors. One pin alone is a partial
    # binding, so the floor is asserted rather than a single occurrence.
    assert len(policy_pins) >= 2, policy_pins
    assert set(policy_pins) == {disk}, policy_pins
    fixture_pins = _pinned_digests(_load(POSITIVE_PATH), carrier)
    assert set(fixture_pins) == {disk}, fixture_pins


def test_parity_ledger_scope_denominators_equal_the_carrier_product_scope_census():
    carrier = _load(ROOT / "data/parse/grobid_compatibility.yaml")
    ledger = carrier["parity_ledger"]
    assert ledger["contract_id"] == PARITY_LEDGER_CONTRACT
    census: dict[str, int] = {}
    for cap in carrier["capabilities"]:
        scope = str(cap.get("product_scope") or "")
        census[scope] = census.get(scope, 0) + 1
    # `capabilities[]` is the sole capability denominator, so the declared scope
    # counters must be exactly its census — not merely a total that happens to fit.
    assert ledger["denominators"]["scope"] == census
    assert ledger["denominators"]["capabilities"] == len(carrier["capabilities"])
    # Positive polarity at report level: the live root carries no parity flag.
    assert _parity_ledger_flags(_check_scholarly_parse_plane(ROOT)) == []


@pytest.mark.parametrize(
    "scope_key", ["required", "supporting", "not_compatibility_requirement", "deferred"]
)
def test_checker_flags_any_substituted_parity_scope_counter(tmp_path: Path, scope_key: str):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    payload["parity_ledger"]["denominators"]["scope"][scope_key] += 1
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _parity_ledger_flags(_check_scholarly_parse_plane(tmp_path))
    assert any(
        item.flag == "scholarly_parse_plane_parity_ledger_denominator"
        and "parity_ledger.denominators.scope" in item.evidence
        for item in flags
    ), (scope_key, flags)


def test_checker_reports_an_absent_parity_ledger_as_a_named_flag(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "data/parse/grobid_compatibility.yaml"
    payload = yaml.safe_load(path.read_text(encoding="utf-8"))
    assert payload.pop("parity_ledger"), "the removal anchor did not land"
    path.write_text(yaml.safe_dump(payload), encoding="utf-8")
    flags = _parity_ledger_flags(_check_scholarly_parse_plane(tmp_path))
    assert [item.flag for item in flags] == [
        "scholarly_parse_plane_parity_ledger_missing"
    ], flags
    assert "data/parse/grobid_compatibility.yaml#parity_ledger" in flags[0].evidence


# --- M511/S08/T05: frozen external release-probe lane -------------------------

PROBE_ARTIFACT = "artifacts/parse_compat/external_holdout_release_probe.json"
PROBE_IDENTITY = "external-holdout-release-probe.v1"


def test_release_probe_pin_matches_disk_on_both_surfaces():
    policy = _load(POLICY_PATH)
    fixture = _load(POSITIVE_PATH)
    disk = _sha256(ROOT / PROBE_ARTIFACT)
    canonical = {item["path"]: item for item in policy["bindings"]["canonical_artifacts"]}
    evidence = {(item["path"], item["identity"]) for item in policy["bindings"]["integration_evidence"]}
    assert canonical[PROBE_ARTIFACT]["sha256"] == disk
    assert (PROBE_ARTIFACT, PROBE_IDENTITY) in evidence
    fixture_canonical = {item["path"]: item for item in fixture["canonical_artifacts"]}
    fixture_evidence = {(item["path"], item["identity"]) for item in fixture["integration_evidence"]}
    assert fixture_canonical[PROBE_ARTIFACT]["sha256"] == disk
    assert fixture_canonical[PROBE_ARTIFACT]["identity"] == PROBE_IDENTITY
    assert (PROBE_ARTIFACT, PROBE_IDENTITY) in fixture_evidence


def test_release_probe_clean_root_has_no_probe_flags():
    assert _release_probe_flags(_check_scholarly_parse_plane(ROOT)) == []


def test_release_probe_lane_failure_anchors_are_declared():
    spec = _load(POLICY_PATH)["release_probe_lane"]
    assert spec["artifact_identity"] == PROBE_IDENTITY
    assert spec["manifest"] == "data/parse/acceptance_manifest.yaml"
    assert spec["holdout_manifest"] == "data/parse/holdout_manifests.yaml"
    assert spec["expected_values"]["release_decision_package.human_release_review.decision"] == "pending"
    assert set(spec["blocked_authorizations"]) == {
        "redistribution",
        "production_use",
        "model_distribution",
        "external_downloads",
        "graph_write_or_import",
    }
    assert set(spec["blocked_authorizations"].values()) == {"blocked"}


def test_release_probe_artifact_missing_is_named(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    _probe_artifact_path(tmp_path).unlink()
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "lane_missing")
    assert any(PROBE_ARTIFACT in item.evidence for item in flags), flags


def test_release_probe_artifact_mutation_names_the_artifact(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["measured_scope"] = "mutated"
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _check_scholarly_parse_plane(tmp_path)
    # The live identity loop must traverse the new artifact by its
    # artifact-relative path and name it on substitution.
    assert any(
        item.flag == "scholarly_parse_plane_evidence_substitution" and PROBE_ARTIFACT in item.evidence
        for item in flags
    ), [item.flag for item in flags]


def test_release_probe_lane_absent_is_named(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    assert manifest["external_holdout_contract"].pop("release_probe"), "the removal anchor did not land"
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "lane_missing")
    assert any("external_holdout_contract.release_probe" in item.evidence for item in flags), flags


def test_release_probe_unmapped_artifact_is_named(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = tmp_path / "scripts/governor/checks/scholarly_parse_plane.yaml"
    policy = yaml.safe_load(path.read_text(encoding="utf-8"))
    before = len(policy["bindings"]["integration_evidence"])
    policy["bindings"]["integration_evidence"] = [
        item
        for item in policy["bindings"]["integration_evidence"]
        if item.get("identity") != PROBE_IDENTITY
    ]
    assert len(policy["bindings"]["integration_evidence"]) == before - 1
    path.write_text(yaml.safe_dump(policy), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "lane_missing")
    assert any(PROBE_IDENTITY in item.evidence for item in flags), flags


def test_release_probe_counter_drift_reports_expected_and_observed(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    lane = manifest["external_holdout_contract"]["release_probe"]
    lane["identity_digest"] = "sha256:" + "0" * 64
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    articles = [
        article
        for article in (
            yaml.safe_load(_probe_holdout_path(tmp_path).read_text(encoding="utf-8")) or {}
        ).get("articles", [])
        if isinstance(article, dict)
    ]
    expected_digest = _release_probe_digest(articles, lane)
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "release_probe.identity_digest" in item.evidence
        and expected_digest in item.evidence
        and "0000000000000000" in item.evidence
        for item in flags
    ), flags


def test_release_probe_document_counter_drift_is_flagged(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    measurable = _probe_measurable_population(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["evaluated_population"]["aggregate"]["documents"] = measurable - 1
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "evaluated_population.aggregate.documents" in item.evidence
        and f"expected={measurable}" in item.evidence
        and f"observed={measurable - 1}" in item.evidence
        for item in flags
    ), flags

def test_release_probe_measurable_counter_binds_to_the_subset(tmp_path: Path):
    """A measurable counter pinned to the frozen population is rejected, so the
    typed exceptions stay visible instead of being read as measured rows."""
    _copy_checker_surface(tmp_path)
    measurable = _probe_measurable_population(tmp_path)
    frozen = _probe_population(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["evaluated_population"]["documents"] = frozen
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "evaluated_population.documents" in item.evidence
        and f"expected={measurable}" in item.evidence
        and f"observed={frozen}" in item.evidence
        for item in flags
    ), flags


def test_release_probe_locator_census_must_match_the_population(tmp_path: Path):
    """A census row list shorter than the population is a named denominator failure."""
    _copy_checker_surface(tmp_path)
    documents = _probe_population(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["production_catalog_overlap"]["locators"] = payload["production_catalog_overlap"][
        "locators"
    ][: documents - 1]
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "production_catalog_overlap.locators" in item.evidence
        and f"expected={documents}" in item.evidence
        and f"observed={documents - 1}" in item.evidence
        for item in flags
    ), flags


def test_release_probe_present_counter_must_equal_its_locator_rows(tmp_path: Path):
    """A declaration the per-document rows do not back is named, not trusted."""
    _copy_checker_surface(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    rows = payload["production_catalog_overlap"]["locators"]
    recomputed = sum(row.get("sources_archive_present") is True for row in rows)
    payload["production_catalog_overlap"]["sources_archive_present_count"] = recomputed + 4
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "production_catalog_overlap.sources_archive_present_count" in item.evidence
        and f"expected={recomputed}" in item.evidence
        for item in flags
    ), flags


def test_release_probe_population_expectation_follows_the_manifest_data(tmp_path: Path):
    """Growing the DATA population must flag the frozen artifact, never pass silently."""
    _copy_checker_surface(tmp_path)
    documents = _probe_population(tmp_path)
    holdout_path = _probe_holdout_path(tmp_path)
    holdout = yaml.safe_load(holdout_path.read_text(encoding="utf-8"))
    holdout["articles"] = list(holdout["articles"]) + [
        {"article_id": "9999.00001"},
        {"article_id": "9999.00002"},
    ]
    holdout_path.write_text(yaml.safe_dump(holdout), encoding="utf-8")
    grown = documents + 2
    measurable = _probe_measurable_population(tmp_path)
    exceptions = _probe_exception_population(tmp_path)
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    # The expectation came from the article manifest, not a literal: the frozen
    # artifact still declares the old population and every carrier is named.
    assert any(
        "reference_population.documents" in item.evidence
        and f"expected={grown}" in item.evidence
        and f"observed={documents}" in item.evidence
        for item in flags
    ), flags
    # The measurable subset keeps its own denominator: the grown population must
    # be covered by reference_tei + reference_exceptions, so the flag names the
    # expected measurable count (grown - exceptions) against the declared subset
    # rather than demanding the frozen count.
    assert any(
        ".reference_tei" in item.evidence
        and f"expected={grown - exceptions}" in item.evidence
        and f"observed={measurable}" in item.evidence
        for item in flags
    ), flags
    # The two synthetic documents belong to neither half: the partition gap is
    # named per document, not silently absorbed.
    for synthetic in ("9999.00001", "9999.00002"):
        assert any(
            f"partition row for {synthetic}" in item.evidence for item in flags
        ), flags


def test_release_probe_policy_pins_the_population_carrier_not_the_count():
    spec = _load(POLICY_PATH)["release_probe_lane"]
    assert spec["holdout_manifest"] == "data/parse/holdout_manifests.yaml"
    assert spec["manifest"] == "data/parse/acceptance_manifest.yaml"
    assert "documents" not in spec, "the policy must reference DATA, not pin the count"
    assert "expected_documents" not in spec


def test_release_probe_identity_drift_is_flagged(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["schema_version"] = "external-holdout-release-probe.v2"
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        PROBE_ARTIFACT in item.evidence and "external-holdout-release-probe.v2" in item.evidence
        for item in flags
    ), flags


def test_release_probe_citation_unlock_while_pending_is_flagged(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    manifest["external_holdout_contract"]["citation_rule"]["citation_without_release_decision"] = "allowed"
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "citation_state")
    assert any(
        "external_holdout_contract.citation_rule.citation_without_release_decision" in item.evidence
        for item in flags
    ), flags


def test_release_probe_prohibited_authorization_unblocked_is_flagged(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    manifest["release_decision_package"]["prohibited_authorizations"]["redistribution"] = "authorized"
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "citation_state")
    assert any(
        "release_decision_package.prohibited_authorizations.redistribution" in item.evidence
        for item in flags
    ), flags


def test_release_probe_pending_decision_flip_is_flagged(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    manifest["release_decision_package"]["human_release_review"]["decision"] = "recorded"
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "citation_state")
    assert any("human_release_review.decision" in item.evidence for item in flags), flags


def test_release_probe_external_band_flip_is_flagged(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["external_holdout_band"]["external_bytes"] = "bytes_present"
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "external_holdout_band.external_bytes" in item.evidence and "bytes_present" in item.evidence
        for item in flags
    ), flags


def test_release_probe_reference_byte_drift_is_flagged(tmp_path: Path):
    _copy_checker_surface(tmp_path)
    lane = _load(_probe_lane_path(tmp_path))["external_holdout_contract"]["release_probe"]
    tei = _detach_linked_fixture(tmp_path / lane["reference_tei"][0]["path"])
    tei.write_bytes(tei.read_bytes() + b"\n")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any("reference_tei" in item.evidence for item in flags), flags

def test_release_probe_exception_unknown_reason_is_flagged(tmp_path: Path):
    """An exception reason from outside the closed vocabulary fails closed."""
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    lane = manifest["external_holdout_contract"]["release_probe"]
    lane["reference_exceptions"][0]["reason"] = "invented_reason"
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "reference_exceptions" in item.evidence
        and ".reason" in item.evidence
        and "invented_reason" in item.evidence
        for item in flags
    ), flags

def test_release_probe_exception_byte_drift_is_flagged(tmp_path: Path):
    """Exceptions carry frozen bytes; drifting exception bytes are not a skip."""
    _copy_checker_surface(tmp_path)
    lane = _load(_probe_lane_path(tmp_path))["external_holdout_contract"]["release_probe"]
    tei = _detach_linked_fixture(tmp_path / lane["reference_exceptions"][0]["path"])
    tei.write_bytes(tei.read_bytes() + b"\n")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any("reference_exceptions" in item.evidence for item in flags), flags

def test_release_probe_partition_duplicate_is_flagged(tmp_path: Path):
    """A document in both halves of the partition is named, not double counted."""
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    lane = manifest["external_holdout_contract"]["release_probe"]
    lane["reference_exceptions"].append(
        dict(lane["reference_tei"][0], reason="header_date_absent")
    )
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any("exactly one partition row" in item.evidence for item in flags), flags

def test_release_probe_partition_missing_row_is_flagged(tmp_path: Path):
    """Dropping a frozen document from both halves is named, not read as zero."""
    _copy_checker_surface(tmp_path)
    path = _probe_lane_path(tmp_path)
    manifest = yaml.safe_load(path.read_text(encoding="utf-8"))
    lane = manifest["external_holdout_contract"]["release_probe"]
    dropped = lane["reference_tei"].pop()["article_id"]
    path.write_text(yaml.safe_dump(manifest), encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(f"partition row for {dropped}" in item.evidence for item in flags), flags

def test_release_probe_artifact_exception_row_drift_is_flagged(tmp_path: Path):
    """The artifact's typed exception rows are compared to the manifest DATA."""
    _copy_checker_surface(tmp_path)
    path = _probe_artifact_path(tmp_path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["reference_population"]["exceptions"][0]["reason"] = "invented_reason"
    path.write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    flags = _release_probe_flags(_check_scholarly_parse_plane(tmp_path), "denominator")
    assert any(
        "reference_population.exceptions" in item.evidence
        and "invented_reason" in item.evidence
        for item in flags
    ), flags


def test_governor_validate_yaml_passes_for_the_probe_lane_surface():
    from governor.__main__ import _validate_yaml
    from governor.adapters.yaml_rule_repository import YAMLRuleRepository

    assert _validate_yaml(YAMLRuleRepository(ROOT / "scripts/governor/checks")) == 0


# --- M511/S09/T04: independent cross-surface qualification census -------------
#
# The machine-held half of the S09 qualification. These probes do not re-read a
# single artifact in isolation: they assert that every declared review surface of
# the GROBID absorption wave agrees with disk and with the other surfaces on the
# current revision. They reuse the S19 helpers (ROOT / _load / _sha256 /
# _source_bindings) and never call git — cross-surface equality is proven by
# reading the recorded literals, so a moved HEAD cannot make the census pass or
# fail silently. Each helper is paired with a negative probe that injects the
# drift the census claims to catch, so no probe is vacuous.

ROADMAP_PATH = ROOT / "doc/review/grobid-absorption-roadmap-2026-09-21.yaml"
AUDIT_PATH = ROOT / "doc/review/grobid-absorption-audit-2026-09-21.md"
MANIFEST_PATH = ROOT / "data/parse/acceptance_manifest.yaml"
CARRIER_PATH = ROOT / "data/parse/grobid_compatibility.yaml"
LICENSE_INVENTORY_PATH = ROOT / "data/parse/license_inventory.yaml"
FOLLOWUP_PATH = ROOT / "doc/review/grobid-absorption-followup.json"
HORIZON_PATH = ROOT / "doc/review/grobid-rust-horizon-review.json"
HORIZON_CONTRACT_PATH = ROOT / "scripts/governor/checks/grobid_rust_horizon_review.yaml"

# The five locks the whole wave must hold simultaneously. `deployed` and
# `graph_write` are not repeated verbatim on every surface: the acceptance
# manifest expresses a graph write as
# `release_decision_package.prohibited_authorizations.graph_write_or_import:
# blocked` and carries no deployment flag at all. The census therefore asserts
# that no surface contradicts a lock it declares and that the union of surfaces
# covers all five, rather than demanding one identical block on every file.
QUALIFICATION_LOCKS = {
    "import_eligible": False,
    "deployed": False,
    "production_route": "forbidden",
    "graph_write": "forbidden",
    "auto_promote": "forbidden",
}


def _normalize_blocked(value):
    """Map the manifest's `blocked` authorizations onto the lock vocabulary."""
    return "forbidden" if value == "blocked" else value


def _qualification_lock_faces() -> dict[str, dict]:
    """Read the three declared lock surfaces into one normalized mapping."""
    policy = _load(POLICY_PATH)
    manifest = _load(MANIFEST_PATH)
    roadmap = _load(ROADMAP_PATH)
    manifest_face = {
        key: value
        for key, value in manifest["policy_locks"].items()
        if key in QUALIFICATION_LOCKS
    }
    manifest_face["graph_write"] = manifest["release_decision_package"][
        "prohibited_authorizations"
    ]["graph_write_or_import"]
    return {
        "policy": {
            key: value
            for key, value in policy["locks"].items()
            if key in QUALIFICATION_LOCKS
        },
        "manifest": {
            key: _normalize_blocked(value) for key, value in manifest_face.items()
        },
        "roadmap": {
            key: value
            for key, value in roadmap["measured_state"]["locks"].items()
            if key in QUALIFICATION_LOCKS
        },
    }


def _lock_face_violations(faces: dict[str, dict]) -> list[str]:
    """Return human-readable contradictions in a set of normalized lock faces."""
    violations: list[str] = []
    for surface, face in sorted(faces.items()):
        for key, value in sorted(face.items()):
            if value != QUALIFICATION_LOCKS[key]:
                violations.append(f"{surface}:{key}={value!r}")
    declared: set[str] = set()
    for face in faces.values():
        declared |= set(face)
    for key in sorted(set(QUALIFICATION_LOCKS) - declared):
        violations.append(f"undeclared:{key}")
    return violations


def _unresolved_pass_violations(registry: dict, review: dict) -> list[str]:
    """Return `stage:flag` for every unresolved stage that claims a PASS."""
    unresolved = set(registry["unresolved_status"])
    violations: list[str] = []
    for stage in review["stages"]:
        if stage["status"] not in unresolved:
            continue
        if stage["implementation_pass"] is not False:
            violations.append(f"{stage['stage_id']}:implementation_pass")
        if stage["verification_pass"] is not False:
            violations.append(f"{stage['stage_id']}:verification_pass")
    return violations


def test_s09_census_policy_bindings_resolve_and_hash_on_disk():
    policy = _load(POLICY_PATH)
    bindings = _source_bindings(policy)
    assert len(bindings) == 18, [binding["path"] for binding in bindings]
    paths = [binding["path"] for binding in bindings]
    assert len(set(paths)) == len(paths), "duplicate binding path"
    for binding in bindings:
        assert len(binding["sha256"]) == 64, binding["path"]
        path = ROOT / binding["path"]
        assert path.is_file(), binding["path"]
        assert _sha256(path) == binding["sha256"], binding["path"]


def test_s09_census_positive_mirror_pins_equal_policy_and_disk():
    policy = _load(POLICY_PATH)
    fixture = _load(POSITIVE_PATH)
    policy_pins = {
        binding["path"]: binding["sha256"] for binding in _source_bindings(policy)
    }
    mirror = list(fixture["source_data"]) + list(fixture["canonical_artifacts"])
    assert len(mirror) == len(policy_pins)
    for item in mirror:
        assert item["path"] in policy_pins, item["path"]
        assert item["sha256"] == policy_pins[item["path"]], item["path"]
        assert _sha256(ROOT / item["path"]) == item["sha256"], item["path"]


def test_s09_census_lock_faces_agree_across_three_surfaces():
    faces = _qualification_lock_faces()
    assert _lock_face_violations(faces) == []
    declared: set[str] = set()
    for face in faces.values():
        declared |= set(face)
    assert declared == set(QUALIFICATION_LOCKS), sorted(declared)
    core = {"import_eligible", "production_route", "auto_promote"}
    assert all(core <= set(face) for face in faces.values()), faces


def test_s09_census_lock_faces_reject_permissive_or_dropped_lock():
    faces = _qualification_lock_faces()
    permissive = copy.deepcopy(faces)
    permissive["policy"]["import_eligible"] = True
    assert "policy:import_eligible=True" in _lock_face_violations(permissive)
    dropped = copy.deepcopy(faces)
    for surface in dropped.values():
        surface.pop("graph_write", None)
    assert "undeclared:graph_write" in _lock_face_violations(dropped)


def test_s09_census_unresolved_stages_never_claim_pass():
    pairs = (
        (_load(HORIZON_CONTRACT_PATH)["registry"], _load(HORIZON_PATH)),
        (_load(ROADMAP_PATH)["registry"], _load(FOLLOWUP_PATH)),
    )
    unresolved_seen = 0
    for registry, review in pairs:
        assert registry["unresolved_status"], review["review_id"]
        assert review["stages"]
        allowed = set(registry["allow_status"])
        for stage in review["stages"]:
            assert stage["status"] in allowed, stage["stage_id"]
        assert _unresolved_pass_violations(registry, review) == [], review["review_id"]
        unresolved_seen += sum(
            1
            for stage in review["stages"]
            if stage["status"] in set(registry["unresolved_status"])
        )
    assert unresolved_seen >= 8, unresolved_seen
    resolved_claims = [
        stage
        for registry, review in pairs
        for stage in review["stages"]
        if stage["status"] not in set(registry["unresolved_status"])
        and stage["implementation_pass"] is True
    ]
    assert resolved_claims, "the vocabulary must discriminate resolved from unresolved"


def test_s09_census_unresolved_probe_rejects_pass_claim():
    registry = {"unresolved_status": ["pending", "in_progress", "blocked"]}
    review = {
        "stages": [
            {
                "stage_id": "H99",
                "status": "pending",
                "implementation_pass": True,
                "verification_pass": False,
            },
            {
                "stage_id": "H98",
                "status": "in_progress",
                "implementation_pass": False,
                "verification_pass": True,
            },
        ]
    }
    assert _unresolved_pass_violations(registry, review) == [
        "H99:implementation_pass",
        "H98:verification_pass",
    ]


def test_s09_census_release_decision_package_still_awaits_human_review():
    manifest = _load(MANIFEST_PATH)
    package = manifest["release_decision_package"]
    assert package["package_status"] == "pending_human_review"
    review = package["human_release_review"]
    assert review["required"] is True
    assert review["reviewer"] is None
    assert review["decision"] == "pending"
    assert review["prose_alone_authorizes_release"] is False
    prohibited = package["prohibited_authorizations"]
    assert len(prohibited) == 5, prohibited
    assert set(prohibited.values()) == {"blocked"}, prohibited
    # The governor probe lane must declare the same pending decision and the same
    # blocked set, so a manifest-only edit cannot silently unlock a release.
    lane = _load(POLICY_PATH)["release_probe_lane"]
    assert (
        lane["expected_values"][
            "release_decision_package.human_release_review.decision"
        ]
        == "pending"
    )
    assert set(lane["blocked_authorizations"]) == set(prohibited)
    assert set(lane["blocked_authorizations"].values()) == {"blocked"}


def test_s09_census_measured_denominators_match_their_carriers():
    measured = _load(ROADMAP_PATH)["measured_state"]
    capabilities = _load(CARRIER_PATH)["capabilities"]
    assert len(capabilities) == 38, len(capabilities)
    assert len({row["id"] for row in capabilities}) == len(capabilities)
    assert measured["parity_ledger"]["rows"] == len(capabilities)
    assert measured["capability_ledger"]["total"] == len(capabilities)
    inventory = _load(LICENSE_INVENTORY_PATH)
    components = inventory["components"]
    assert measured["license_inventory"]["canonical"] == (
        "data/parse/license_inventory.yaml"
    )
    assert len(components) == measured["license_inventory"]["components"]
    assert len({component["component_id"] for component in components}) == len(
        components
    ), "duplicate license identity"
    observed_status = {}
    for component in components:
        observed_status[component["license_status"]] = (
            observed_status.get(component["license_status"], 0) + 1
        )
    assert observed_status == measured["license_inventory"]["license_status"], (
        observed_status
    )


def test_s09_census_audit_qualification_revision_is_bound_to_reviewed_source():
    audit = AUDIT_PATH.read_text(encoding="utf-8")
    revisions = re.findall(r"Ревизия квалификации:\s*([0-9a-f]{40})", audit)
    assert len(revisions) == 1, revisions
    qualification = revisions[0]
    reviewed_source = _load(ROADMAP_PATH)["reviewed_source"]
    assert qualification in reviewed_source, (qualification, reviewed_source)
    # The final gate revision is recorded in the same section. When present it
    # must be a complete 40-hex literal, never an abbreviated or prose label.
    final_gate = re.findall(r"Ревизия финального гейта:\s*([0-9a-f]{40})", audit)
    assert len(final_gate) <= 1, final_gate
    if final_gate:
        assert len(final_gate[0]) == 40


def test_s09_census_revision_label_resolution_is_typed_and_content_bound():
    ledger = _load(ROADMAP_PATH)["measured_state"]["parity_ledger"]
    assert ledger["rows_are_the_capability_denominator"] is True
    assert ledger["revision_label_discrepancy_resolved"] is True
    resolution = ledger["revision_label_resolution"]
    assert resolution["binding_authority"] == "content_sha256"
    assert resolution["kinds_are_distinct"] is True
    assert resolution["carrier_untouched"] is True
    identities = resolution["identities"]
    assert len(identities) == 3, identities
    assert len({item["kind"] for item in identities}) == 3, identities
    by_identity = {item["identity"]: item for item in identities}
    assert set(by_identity) == {
        "authority_label",
        "capture_declared_revision",
        "observed_checkout_revision",
    }
    assert (
        by_identity["capture_declared_revision"]["value"]
        == ledger["capture_declared_revision"]
    )
    assert (
        by_identity["observed_checkout_revision"]["value"]
        == ledger["observed_checkout_revision"]
    )
    assert ledger["observed_checkout_revision"] != ledger["capture_declared_revision"]


# --- M512/S03/T05: lane pin census and live-root lane silence -----------------
#
# T04 moved both the accepted manifest and the release-probe artifact, so every
# SHA-256 pin on the three governor lane surfaces went stale at once: the
# manifest is pinned in `bindings.source_data` of `scholarly_parse_plane.yaml`
# and of `external_holdout_package.yaml` and mirrored in the positive fixture,
# while the probe artifact is byte-pinned in the `canonical_artifacts` of both
# scholarly surfaces. A stale pin only fails as `..._evidence_substitution` or
# `..._binding_mismatch` at the next `--phase preflight`, which is too late to
# be noticed by the run that moved the bytes. These probes read the shipped DATA
# and the disk bytes, then run the report's own lane flag sources over the live
# ROOT, so a re-pin that missed a surface or a lane surface that started
# flagging the live tree fails here.

EXTERNAL_HOLDOUT_POLICY_PATH = (
    ROOT / "scripts/governor/checks/external_holdout_package.yaml"
)
LANE_SURFACES = {
    "scholarly policy": POLICY_PATH,
    "scholarly fixture": POSITIVE_PATH,
    "external holdout policy": EXTERNAL_HOLDOUT_POLICY_PATH,
}
LANE_FLAG_PREFIXES = (
    "scholarly_parse_plane",
    "external_holdout_package",
    "release_probe",
)


def _lane_named_flags(flags):
    """Only the findings a lane check can emit, from a report or a check.

    A preflight report is shared with unrelated dimensions (`logs/*` schema
    drift, `target/` bloat, GSD planning projections); those are not lane
    findings, so the probe filters by flag name instead of by evidence text --
    an unrelated flag may legitimately mention a lane artifact in its evidence.
    """
    def _name(flag):
        if isinstance(flag, dict):
            return str(flag.get("flag", ""))
        return str(getattr(flag, "flag", ""))

    return [flag for flag in flags if _name(flag).startswith(LANE_FLAG_PREFIXES)]


def test_lane_policy_pins_match_the_disk_digests_t04_measured():
    manifest = "data/parse/acceptance_manifest.yaml"
    manifest_digest = _sha256(ROOT / manifest)
    probe_digest = _sha256(ROOT / PROBE_ARTIFACT)
    # The accepted manifest is pinned on all three lane surfaces.
    for name, surface in LANE_SURFACES.items():
        pins = _pinned_digests(_load(surface), manifest)
        assert pins, f"{name} declares no pin for {manifest}"
        assert set(pins) == {manifest_digest}, (name, pins)
    # The probe artifact is byte-pinned on both scholarly surfaces.
    for name in ("scholarly policy", "scholarly fixture"):
        pins = _pinned_digests(_load(LANE_SURFACES[name]), PROBE_ARTIFACT)
        assert pins, f"{name} declares no pin for {PROBE_ARTIFACT}"
        assert set(pins) == {probe_digest}, (name, pins)


def test_live_root_lane_flag_sources_report_nothing():
    """The report's lane flag sources stay silent on the shipped tree.

    `build_report` composes the scholarly dimension from
    `_check_scholarly_parse_plane` and the external holdout contract from
    `TrajectoryService._check_external_holdout_package`; running both over ROOT
    is the lane half of a real `--phase preflight` without the
    environment-dependent resource/disk flags the census filters out.
    """
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = ROOT
    assert _lane_named_flags(_check_scholarly_parse_plane(ROOT)) == []
    assert _lane_named_flags(service._check_external_holdout_package()) == []


# --- M512/S04/T04: no claim surface may assert a superseded census -----------
#
# T04 proves the absence of stale five-document claims by scanning the
# census-consuming claim surfaces for the superseded formulations and by
# re-deriving every declared count from the manifest and the frozen artifact.
# The scanned set is the current-fact surfaces a reader (or governor) consumes;
# the dated historical records (ADR-076's dated correction, the dated audit,
# the S08 decision package, CHANGELOG, the dated pilot report) restate the S08
# pilot as history and are deliberately out of the scan set. The negative probe
# below injects a superseded formulation so the scan cannot pass vacuously.

EXECUTED_POPULATION_BINDING_PATH = (
    ROOT / "scripts/governor/checks/executed_population_binding.yaml"
)
HOLDOUT_MANIFEST_PATH = ROOT / "data/parse/holdout_manifests.yaml"

SUPERSEDED_CENSUS_PATTERNS = {
    # S08 pilot artifact identity: sha, byte size and identity digest.
    "superseded_artifact_sha": r"30479a60",
    "superseded_artifact_bytes": r"36\s?511",
    "superseded_identity_digest": r"22df1b7e",
    # Compiled-in population literals the lane used to carry.
    "compiled_in_document_count": r"SYNTHETIC_DOCUMENT_COUNT",
    "documents_equals_five": r"documents\s*=\s*5\b",
    "documents_equals_forty": r"documents=40",
    "five_document_sha_pin": r"(5|five)\s+sha256",
    "five_of_five_documents": r"5/5\s*(документ|document)",
    "fifty_document_sample": r"(?<![0-9])50[-\s]document",
    "fifty_row_sample": r"(?<![0-9])50\s*rows",
}

CENSUS_CLAIM_SURFACES = (
    "data/parse/acceptance_manifest.yaml",
    "data/parse/holdout_manifests.yaml",
    "data/parse/license_inventory.yaml",
    "scripts/governor/checks/executed_population_binding.yaml",
    "doc/review/grobid-absorption-roadmap-2026-09-21.yaml",
    "doc/review/grobid-absorption-followup.json",
    "crates/da-parse/tests/external_holdout_probe_semantics.rs",
    "crates/da-parse/tests/external_holdout_probe_contract.rs",
    "crates/da-parse/tests/external_holdout_package_contract.rs",
    "crates/da-cli/tests/external_holdout_capture_test.rs",
    "crates/da-cli/tests/external_holdout_release_probe_test.rs",
    "crates/da-cli/examples/external_holdout_capture.rs",
    "crates/da-cli/examples/external_holdout_release_probe.rs",
)


def _superseded_census_hits(text: str) -> list[str]:
    """Every superseded census formulation in `text`, named by its pattern."""
    return [
        f"{name}: {match.group(0)!r}"
        for name, pattern in SUPERSEDED_CENSUS_PATTERNS.items()
        for match in re.finditer(pattern, text, re.IGNORECASE)
    ]


def _measured_census() -> tuple[int, int, int]:
    """`(measurable, exceptions, declared)` read from the shared manifest."""
    lane = _load(MANIFEST_PATH)["external_holdout_contract"]["release_probe"]
    measurable = len(lane["reference_tei"])
    exceptions = len(lane["reference_exceptions"])
    return measurable, exceptions, measurable + exceptions


def test_m512_s04_census_claim_surfaces_carry_no_superseded_document_count():
    measurable, exceptions, declared = _measured_census()
    # The measured census of the M512 expansion, pinned so an unintended drift is
    # visible here instead of only at the next governor preflight.
    assert (measurable, exceptions, declared) == (531, 79, 610)
    # Every other census surface agrees with the manifest, not with a literal.
    assert len(_load(HOLDOUT_MANIFEST_PATH)["articles"]) == declared
    artifact = json.loads((ROOT / PROBE_ARTIFACT).read_text(encoding="utf-8"))
    assert artifact["reference_population"]["documents"] == declared
    assert artifact["executed_population"]["documents"] == measurable
    assert artifact["evaluated_population"]["documents"] == measurable
    # The governor census binding names the same two surfaces and keys, so the
    # check and this probe cannot disagree about where the count comes from.
    binding = _load(EXECUTED_POPULATION_BINDING_PATH)["census_binding"]
    assert binding["manifest"] == "data/parse/acceptance_manifest.yaml"
    assert binding["report"] == PROBE_ARTIFACT
    assert binding["measurable_key"] == "reference_tei"
    assert binding["exception_key"] == "reference_exceptions"
    # No current-fact surface restates the superseded S08 pilot or a literal.
    hits = {
        surface: _superseded_census_hits((ROOT / surface).read_text(encoding="utf-8"))
        for surface in CENSUS_CLAIM_SURFACES
    }
    assert hits == {surface: [] for surface in CENSUS_CLAIM_SURFACES}, {
        surface: found for surface, found in hits.items() if found
    }


def test_m512_s04_census_claim_scan_rejects_a_reintroduced_stale_claim():
    """Negative probe: a reintroduced superseded claim must be caught."""
    stale = (
        "Измерено: 5 sha256-пиннутых не-gold TEI дают 731 → 705 admitted; "
        "артефакт sha256 30479a60…, identity sha256:22df1b7e…."
    )
    found = _superseded_census_hits(stale)
    names = {item.split(":", 1)[0] for item in found}
    assert "five_document_sha_pin" in names, found
    assert "superseded_artifact_sha" in names, found
    assert "superseded_identity_digest" in names, found
    # The measured formulation is not itself a hit.
    assert _superseded_census_hits(
        "610 объявленных = 531 измеримых + 79 типизированных исключений"
    ) == []
