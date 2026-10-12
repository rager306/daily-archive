"""Tests for drift-flag ownership (M516-th0oby/S12/T05).

The milestone criterion "every preflight flag names an owner" is only
machine-checkable once each flag CLASS has a declared owner. These tests pin the
three halves of that contract:

  - the DATA registry `scripts/governor/checks/drift_ownership.yaml` (keyed by
    flag CLASS, carrying no `dimension`/`severity` -- it is not a check rule);
  - the fail-closed audit `TrajectoryService._check_drift_ownership`, which turns
    an unowned flag of a required severity into a `drift_flag_owner_missing`
    finding instead of a silent default;
  - the live tree: every flag the committed tree emits resolves to an owner.

Run with: PYTHONPATH=scripts .venv/bin/python -m pytest -q scripts/tests/test_drift_ownership.py
"""

import inspect
import sys
from pathlib import Path

import pytest
import yaml

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from governor.adapters import (
    FilesystemADRRepository,
    FilesystemCodeScanner,
    FilesystemGsdPlanningReader,
    FilesystemMilestoneReader,
    GitReader,
    YAMLRuleRepository,
)
from governor.application import AuditService, TrajectoryService
from governor.domain.trajectory import DriftFlag

_REPO_ROOT = _SCRIPTS_DIR.parent
_REGISTRY_RELATIVE = "scripts/governor/checks/drift_ownership.yaml"
_MISSING_FLAG = "drift_flag_owner_missing"


def _service(root: Path) -> TrajectoryService:
    """A service with only `.root` wired, exactly as the governor's own tests do.

    `__new__` skips `__init__`, so the ownership cache must resolve through
    `getattr` rather than an instance attribute set in the constructor.
    """
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = Path(root)
    return service


def _registry_document(owners: dict, required=("high",)) -> dict:
    return {
        "schema_version": "drift-ownership.v1",
        "required_for_severities": list(required),
        "owners": dict(owners),
    }


def _write_registry(root: Path, document) -> None:
    path = Path(root) / _REGISTRY_RELATIVE
    path.parent.mkdir(parents=True, exist_ok=True)
    text = (
        document
        if isinstance(document, str)
        else yaml.safe_dump(document, sort_keys=False)
    )
    path.write_text(text, encoding="utf-8")


def _recursive_keys(node):
    """Every mapping key anywhere in a YAML document, for the D492 scan."""
    if isinstance(node, dict):
        for key, value in node.items():
            yield key
            yield from _recursive_keys(value)
    elif isinstance(node, list):
        for item in node:
            yield from _recursive_keys(item)


def _assert_fails_closed(findings, class_name: str) -> None:
    """The honest fail-closed outcome for an unowned required flag.

    With an empty owner map the audit also reports its own finding class (the
    sentinel seed), so the assertion is on the contract -- a
    `drift_flag_owner_missing` finding that names the orphan -- not on a count.
    """
    assert findings, "an unowned high flag must fail closed, not pass silently"
    assert all(f.flag == _MISSING_FLAG for f in findings)
    assert any(class_name in f.evidence for f in findings)


@pytest.fixture(scope="module")
def live_report():
    """The committed tree's preflight report, built once (it is the expensive path)."""
    root = _REPO_ROOT
    checks_dir = root / "scripts" / "governor" / "checks"
    rule_repo = YAMLRuleRepository(checks_dir)
    service = TrajectoryService(
        project_root=root,
        audit_service=AuditService(
            scanner=FilesystemCodeScanner(root), rule_repo=rule_repo
        ),
        rule_repo=rule_repo,
        adr_repo=FilesystemADRRepository(root),
        milestone_reader=FilesystemMilestoneReader(root),
        gsd_planning_reader=FilesystemGsdPlanningReader(root),
        git_reader=GitReader(root),
    )
    return service, service.build_report(phase="preflight")


# ── the registry DATA ────────────────────────────────────────────────────────

def test_live_registry_carries_no_dimension_or_severity() -> None:
    """An ownership registry must not masquerade as a quality gate (D492)."""
    document = yaml.safe_load(
        (_REPO_ROOT / _REGISTRY_RELATIVE).read_text(encoding="utf-8")
    )
    keys = list(_recursive_keys(document))
    assert "dimension" not in keys
    assert "severity" not in keys
    assert document["schema_version"] == "drift-ownership.v1"


def test_live_registry_requires_high_severity_and_owns_its_own_finding() -> None:
    document = yaml.safe_load(
        (_REPO_ROOT / _REGISTRY_RELATIVE).read_text(encoding="utf-8")
    )
    assert document["required_for_severities"] == ["high"]
    # The audit seeds itself with the finding it can emit; a registry that did not
    # declare that class would make the audit self-trigger, which the plan forbids.
    assert document["owners"][_MISSING_FLAG] == "governor-ownership"
    assert all(
        isinstance(owner, str) and owner
        for owner in document["owners"].values()
    )


def test_live_registry_owns_the_gsd_exec_evidence_classes() -> None:
    """The GSD exec-capture evidence classes (D463) each name a declared owner.

    `_check_gsd_exec_evidence` emits both of these at `high`, and the live-tree
    test is window-dependent (a timeout capture only fires while it is after the
    latest commit), so pin the classes here as a stable guard: a rename or a
    dropped owner fails by name even when no capture currently trips the check.
    """
    document = yaml.safe_load(
        (_REPO_ROOT / _REGISTRY_RELATIVE).read_text(encoding="utf-8")
    )
    owners = document["owners"]
    for flag_class in (
        "gsd_exec_false_success_evidence",
        "gsd_exec_timeout_evidence",
    ):
        assert owners.get(flag_class), f"{flag_class} must name a declared owner"


def test_live_registry_owns_every_high_class_the_exec_check_emits() -> None:
    """Every `high` flag `_check_gsd_exec_evidence` can emit resolves to an owner.

    The live report may not currently emit a given class, so assert the pairing
    against the source of the check rather than against the windowed report.
    """
    from governor.application.trajectory_service import TrajectoryService as _Svc

    source = inspect.getsource(_Svc._check_gsd_exec_evidence)
    owners = yaml.safe_load(
        (_REPO_ROOT / _REGISTRY_RELATIVE).read_text(encoding="utf-8")
    )["owners"]
    for flag_class in (
        "gsd_exec_false_success_evidence",
        "gsd_exec_timeout_evidence",
    ):
        assert f'flag="{flag_class}"' in source, f"{flag_class} is no longer emitted"
        assert owners.get(flag_class), f"{flag_class} must name a declared owner"


# ── the serialized owner field ───────────────────────────────────────────────

def test_drift_flag_owner_defaults_to_none() -> None:
    """The defaulted field keeps every pre-existing construction valid."""
    flag = DriftFlag(flag="scorecard_momentum_drift", severity="medium")
    assert flag.owner is None


def test_flag_to_dict_emits_the_declared_owner() -> None:
    service = _service(_REPO_ROOT)
    payload = service._flag_to_dict(
        DriftFlag("scorecard_momentum_drift", "medium", "momentum dipped"), set()
    )
    assert payload["owner"] == "milestone-scorecard"


# ── the fail-closed audit ────────────────────────────────────────────────────

def test_unowned_required_flag_produces_owner_missing(tmp_path: Path) -> None:
    _write_registry(tmp_path, _registry_document({_MISSING_FLAG: "governor-ownership"}))
    findings = _service(tmp_path)._check_drift_ownership(
        [DriftFlag("orphan_class", "high", "a new check shipped an orphan signal")]
    )
    assert len(findings) == 1
    assert findings[0].flag == _MISSING_FLAG
    assert findings[0].severity == "high"
    assert "orphan_class" in findings[0].evidence


def test_owned_required_flag_is_silent(tmp_path: Path) -> None:
    _write_registry(
        tmp_path,
        _registry_document({"owned_class": "owner", _MISSING_FLAG: "governor-ownership"}),
    )
    assert _service(tmp_path)._check_drift_ownership(
        [DriftFlag("owned_class", "high")]
    ) == []


def test_medium_unowned_flag_does_not_fail_closed(tmp_path: Path) -> None:
    """Only `required_for_severities` fails closed; a medium flag may be unowned."""
    _write_registry(tmp_path, _registry_document({_MISSING_FLAG: "governor-ownership"}))
    assert _service(tmp_path)._check_drift_ownership(
        [DriftFlag("medium_orphan", "medium")]
    ) == []


def test_parameterized_flag_id_resolves_by_class(tmp_path: Path) -> None:
    """`artifact_file_missing@M514` must resolve through its class key."""
    _write_registry(
        tmp_path,
        _registry_document(
            {
                "artifact_file_missing": "catalog-owner",
                _MISSING_FLAG: "governor-ownership",
            }
        ),
    )
    service = _service(tmp_path)
    assert service._drift_owner_for("artifact_file_missing@M514") == "catalog-owner"
    assert service._check_drift_ownership(
        [DriftFlag("artifact_file_missing@M514", "high")]
    ) == []


def test_missing_registry_fails_closed(tmp_path: Path) -> None:
    """No registry at all must still hold `high` flags to ownership."""
    service = _service(tmp_path)
    assert not (tmp_path / _REGISTRY_RELATIVE).exists()
    assert service._drift_ownership_map()["required_for_severities"] == ["high"]
    _assert_fails_closed(
        service._check_drift_ownership([DriftFlag("orphan_class", "high")]),
        "orphan_class",
    )


def test_malformed_registry_fails_closed(tmp_path: Path) -> None:
    _write_registry(tmp_path, "owners: [this is not a mapping\n")
    service = _service(tmp_path)
    assert service._drift_ownership_map()["required_for_severities"] == ["high"]
    _assert_fails_closed(
        service._check_drift_ownership([DriftFlag("orphan_class", "high")]),
        "orphan_class",
    )


def test_registry_without_required_severities_fails_closed(tmp_path: Path) -> None:
    _write_registry(tmp_path, {"schema_version": "drift-ownership.v1", "owners": {}})
    service = _service(tmp_path)
    assert service._drift_ownership_map()["required_for_severities"] == ["high"]
    _assert_fails_closed(
        service._check_drift_ownership([DriftFlag("orphan_class", "high")]),
        "orphan_class",
    )


def test_audit_requires_its_own_class_to_be_owned(tmp_path: Path) -> None:
    """The audit seeds itself, so an unowned sentinel class is reported too."""
    _write_registry(tmp_path, _registry_document({"owned_class": "owner"}))
    findings = _service(tmp_path)._check_drift_ownership([DriftFlag("owned_class", "high")])
    assert len(findings) == 1
    assert findings[0].flag == _MISSING_FLAG
    assert _MISSING_FLAG in findings[0].evidence


# ── wiring + the live tree ───────────────────────────────────────────────────

def test_ownership_check_is_wired_into_build_report() -> None:
    """A rule that is implemented but never invoked is a dead rule."""
    source = inspect.getsource(TrajectoryService.build_report)
    assert "_check_drift_ownership(" in source


def test_live_tree_flags_all_resolve_to_an_owner(live_report) -> None:
    """The criterion itself: every flag the committed tree emits names an owner."""
    service, report = live_report
    flags = [f for f in report.drift_flags]
    assert flags, "the live tree must emit at least one flag for this test to bite"
    unowned = [f["flag"] for f in flags if not f.get("owner")]
    assert unowned == [], f"flags without a declared owner: {unowned}"
    # The fail-closed audit must be silent on the live tree: nothing to report.
    assert service._check_drift_ownership(
        [DriftFlag(f["flag"], f["severity"], f["evidence"]) for f in flags]
    ) == []
