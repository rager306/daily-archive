"""Tests for the closeout-criterion disposition wiring (M516-th0oby/S13/T03).

The S13/T02 oracle (``governor.adapters.closeout_criterion_disposition``) is a
pure reader; an oracle that is never invoked is a dead rule. These tests pin the
three halves of the wiring contract:

  - ``TrajectoryService._check_closeout_criterion_disposition`` is invoked by
    ``build_report``, so the reconciliation appears in the report the milestone
    closeout actually reads;
  - the check is fail-closed: an undisposed live high class, a ``measured``
    record for a live class, and a missing/malformed registry each yield a
    ``closeout_criterion_undisposed`` finding instead of a silent pass;
  - the live tree: every high class the committed tree emits resolves to a
    ``downgraded`` record, and the audit is silent -- i.e. milestone success
    criterion SC7 clause 1 ("governor preflight reports no high severity flag at
    closeout") is satisfied by recorded downgrade, not by weakening a check
    (D569).

Run with: PYTHONPATH=scripts .venv/bin/python -m pytest -q scripts/tests/test_closeout_criterion_disposition_wiring.py
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
_DISPOSITIONS_RELATIVE = "scripts/governor/checks/closeout_criterion_dispositions.yaml"
_OWNERSHIP_RELATIVE = "scripts/governor/checks/drift_ownership.yaml"
#: The audit's own finding class; it is also the class the audit seeds itself
#: with, so the registry must declare it or the audit self-triggers.
_SENTINEL = "closeout_criterion_undisposed"

_REASON_VOCABULARY = [
    "pre_existing_structural",
    "pre_existing_environment",
    "severity_environment_dependent",
    "governor_process_finding",
]

def _service(root: Path) -> TrajectoryService:
    """A service with only `.root` wired, exactly as the governor's own tests do.

    `__new__` skips `__init__`, so the disposition cache must resolve through
    `getattr` rather than an instance attribute set in the constructor.
    """
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = Path(root)
    return service

def _record(
    disposition: str = "downgraded",
    owner: str = "some-owner",
    reason: str = "pre_existing_structural",
    decision_ref: str = "D572",
) -> dict:
    return {
        "disposition": disposition,
        "owner": owner,
        "reason": reason,
        "decision_ref": decision_ref,
        "rationale": "a recorded disposition for the test",
    }

def _document(records: dict) -> dict:
    """A minimal well-formed registry document, shaped like the live DATA."""
    return {
        "schema_version": "closeout-criterion-dispositions.v1",
        "disposition_vocabulary": ["measured", "downgraded"],
        "reason_vocabulary": list(_REASON_VOCABULARY),
        "required_record_fields": [
            "disposition",
            "owner",
            "reason",
            "decision_ref",
            "rationale",
        ],
        "required_for_severities": ["high"],
        "records": dict(records),
    }

def _write_registry(root: Path, document) -> None:
    path = Path(root) / _DISPOSITIONS_RELATIVE
    path.parent.mkdir(parents=True, exist_ok=True)
    text = (
        document
        if isinstance(document, str)
        else yaml.safe_dump(document, sort_keys=False)
    )
    path.write_text(text, encoding="utf-8")

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

# ── wiring ───────────────────────────────────────────────────────────────────

def test_check_is_wired_into_build_report() -> None:
    """A rule that is implemented but never invoked is a dead rule."""
    source = inspect.getsource(TrajectoryService.build_report)
    assert "_check_closeout_criterion_disposition(" in source

def test_check_runs_after_the_ownership_audit() -> None:
    """The disposition audit runs on the final flag list, after ownership."""
    source = inspect.getsource(TrajectoryService.build_report)
    ownership = source.index("_check_drift_ownership(")
    disposition = source.index("_check_closeout_criterion_disposition(")
    assert disposition > ownership

# ── fail-closed behaviour ────────────────────────────────────────────────────

def test_undisposed_live_high_flag_yields_one_finding_naming_it(
    tmp_path: Path,
) -> None:
    """A live high class with no record is exactly one undisposed finding."""
    _write_registry(tmp_path, _document({_SENTINEL: _record()}))
    findings = _service(tmp_path)._check_closeout_criterion_disposition(
        [DriftFlag("brand_new_high_class", "high", "a new check shipped")]
    )
    assert len(findings) == 1
    assert findings[0].flag == _SENTINEL
    assert findings[0].severity == "high"
    assert "brand_new_high_class" in findings[0].evidence
    assert "undisposed" in findings[0].evidence

def test_disposed_live_high_flag_is_silent(tmp_path: Path) -> None:
    _write_registry(
        tmp_path,
        _document({"owned_class": _record(), _SENTINEL: _record()}),
    )
    assert _service(tmp_path)._check_closeout_criterion_disposition(
        [DriftFlag("owned_class", "high", "live")]
    ) == []

def test_measured_record_for_a_live_class_yields_a_finding(tmp_path: Path) -> None:
    """`measured` claims the class is absent; a live class contradicts it."""
    _write_registry(
        tmp_path,
        _document(
            {
                "live_class": _record(disposition="measured"),
                _SENTINEL: _record(),
            }
        ),
    )
    findings = _service(tmp_path)._check_closeout_criterion_disposition(
        [DriftFlag("live_class", "high", "live")]
    )
    assert len(findings) == 1
    assert findings[0].flag == _SENTINEL
    assert "live_class" in findings[0].evidence
    assert "contradiction" in findings[0].evidence

def test_medium_flag_is_not_evaluated(tmp_path: Path) -> None:
    """Only `required_for_severities` fails closed; a medium flag may be silent."""
    _write_registry(tmp_path, _document({_SENTINEL: _record()}))
    assert _service(tmp_path)._check_closeout_criterion_disposition(
        [DriftFlag("medium_orphan", "medium", "drift")]
    ) == []

def test_missing_registry_fails_closed(tmp_path: Path) -> None:
    """No registry at all must fail closed, never report a vacuous green."""
    service = _service(tmp_path)
    assert not (tmp_path / _DISPOSITIONS_RELATIVE).exists()
    findings = service._check_closeout_criterion_disposition(
        [DriftFlag("orphan_class", "high", "live")]
    )
    assert findings, "a missing registry must fail closed, not pass silently"
    assert all(f.flag == _SENTINEL for f in findings)
    assert all(f.severity == "high" for f in findings)
    assert any("orphan_class" in f.evidence for f in findings)

def test_malformed_registry_fails_closed(tmp_path: Path) -> None:
    _write_registry(tmp_path, "records: not-a-mapping\n")
    findings = _service(tmp_path)._check_closeout_criterion_disposition(
        [DriftFlag("orphan_class", "high", "live")]
    )
    assert findings, "a malformed registry must fail closed, not pass silently"
    assert any("orphan_class" in f.evidence for f in findings)

def test_parameterized_live_id_resolves_through_its_class(tmp_path: Path) -> None:
    """`owned_class@M516` must resolve through the class key."""
    _write_registry(
        tmp_path,
        _document({"owned_class": _record(), _SENTINEL: _record()}),
    )
    assert _service(tmp_path)._check_closeout_criterion_disposition(
        [DriftFlag("owned_class@M516", "high", "live")]
    ) == []

def test_missing_registry_map_defaults_to_high(tmp_path: Path) -> None:
    """The cached read degrades fail-closed to `required_for_severities: [high]`."""
    service = _service(tmp_path)
    assert service._closeout_criterion_disposition_map()["required_for_severities"] == [
        "high"
    ]

# ── the sentinel class is owned ──────────────────────────────────────────────

def test_live_registry_owns_the_closeout_criterion_sentinel() -> None:
    """The new high finding class must itself name an owner (S12/D567).

    The disposition audit emits ``closeout_criterion_undisposed`` at ``high``;
    the S12 ownership registry must declare that class so the new high class is
    owned and ``_flag_to_dict`` stamps an owner onto it.
    """
    document = yaml.safe_load(
        (_REPO_ROOT / _OWNERSHIP_RELATIVE).read_text(encoding="utf-8")
    )
    assert document["owners"].get(_SENTINEL) == "governor-criterion"
    service = _service(_REPO_ROOT)
    assert service._drift_owner_for(_SENTINEL) is not None

# ── the live tree ────────────────────────────────────────────────────────────

def test_live_tree_high_flags_are_all_disposed(live_report) -> None:
    """SC7 clause 1: every live high class is absent or carries a downgrade."""
    service, report = live_report
    flags = [f for f in report.drift_flags]
    # (a) the fail-closed audit is silent on the live tree: nothing undisposed.
    assert service._check_closeout_criterion_disposition(
        [DriftFlag(f["flag"], f["severity"], f["evidence"]) for f in flags]
    ) == []
    # (b) every live high class resolves to a record whose disposition is
    #     `downgraded`. Read the live class set from the report -- it is
    #     environment-sensitive (gsd_exec_timeout_evidence became live from one
    #     timed-out capture, D568), so it must never be hardcoded.
    records = yaml.safe_load(
        (_REPO_ROOT / _DISPOSITIONS_RELATIVE).read_text(encoding="utf-8")
    )["records"]
    live_high_classes = {
        str(f["flag"]).split("@", 1)[0]
        for f in flags
        if f["severity"] == "high"
    }
    undisposed = sorted(
        flag_class
        for flag_class in live_high_classes
        if not isinstance(records.get(flag_class), dict)
        or records[flag_class].get("disposition") != "downgraded"
    )
    assert undisposed == [], f"live high classes without a downgrade: {undisposed}"
