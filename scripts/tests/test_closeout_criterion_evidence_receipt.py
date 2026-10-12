"""Contract test and generator for the M516-th0oby/S13 closeout-criterion receipt.

Milestone success criterion SC7 clause 1 is *"governor preflight reports no high
severity flag at closeout"*. S13 makes that clause judgeable by recording a
disposition for every high-capable flag CLASS (T01 DATA registry, T02 oracle,
T03 wiring). This file owns the durable evidence receipt a milestone validator
reads for the clause:

  - ``build_receipt(root)`` builds the preflight report IN-PROCESS -- never via
    ``governor --phase preflight``, which is a side-effecting path that rewrites
    the tracked ``artifacts/project-trajectory/trajectory-report.json`` -- and
    projects the live high-severity flag set plus the S13 reconciliation into a
    receipt. It FAILS LOUDLY (raises ``ReceiptError``) rather than writing a
    green receipt when a live high class has no record, carries a disposition
    outside the closed vocabulary, or the reconciliation reports any finding.
  - ``validate_receipt(doc, registry)`` is the pure contract validator the tests
    assert against, so the committed artifact and the inline negative fixtures
    exercise one definition.

Regenerate the artifact with:

    PYTHONPATH=scripts .venv/bin/python scripts/tests/test_closeout_criterion_evidence_receipt.py

The receipt states honestly that SC7 clause 1 is satisfied by RECORDED DOWNGRADE
(D572), not by clearing any flag, and it records the live preflight verdict and
exit code AS OBSERVED -- it never fabricates a green or blocked value. On this
tree the live preflight verdict is ``drift_risk`` (exit 0): the reverse-ADR
audit currently reports no *blocking* violation, so the plan-time expectation of
``blocked`` (exit 1) no longer holds. S13 changes nothing about that.

Run with: PYTHONPATH=scripts .venv/bin/python -m pytest -q scripts/tests/test_closeout_criterion_evidence_receipt.py
"""

from __future__ import annotations

import copy
import json
import sys
from datetime import datetime, timezone
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
    build_closeout_criterion_disposition,
)
from governor.adapters.closeout_criterion_disposition import (
    DISPOSITION_VOCABULARY,
    CloseoutCriterionDisposition,
    DispositionFinding,
)
from governor.application import AuditService, TrajectoryService

_REPO_ROOT = _SCRIPTS_DIR.parent

#: The declared schema of the receipt this file generates.
_SCHEMA = "m516-s13-closeout-criterion-evidence.v1"

#: The DATA registry the receipt resolves every live high class against.
_REGISTRY_RELATIVE = "scripts/governor/checks/closeout_criterion_dispositions.yaml"

#: The committed receipt.
_RECEIPT_RELATIVE = "artifacts/observability/m516_s13_closeout_criterion_evidence.json"

#: The audit's own finding class; it is also the class the audit seeds itself
#: with, so a report that carries it has an unresolved class.
_SENTINEL = "closeout_criterion_undisposed"

#: The closed verdict vocabulary (governor.domain.trajectory.TrajectoryVerdict).
_VERDICTS = ("on_track", "drift_risk", "blocked")

#: The three pinned scorecard axes. The substring is what must not move: S13 is
#: a new check, not an axis, so it may not shift any of these numbers.
_PINNED_AXES = {
    "measurability": "5/21",
    "declaration_sync_gap": "gaps",
    "feature_gated_coverage": "19/31",
}


class ReceiptError(RuntimeError):
    """The live tree does not satisfy SC7 clause 1; no receipt is written."""


# ── the generator ─────────────────────────────────────────────────────────────

def _flag_class(flag_id: object) -> str:
    """The flag CLASS: the id before any parameterized ``@`` suffix."""
    return str(flag_id).split("@", 1)[0]


def _live_report(root: Path):
    """The committed tree's preflight report, built exactly as the governor does.

    Mirrors ``scripts/tests/test_drift_ownership.py::live_report``: the real
    hexagonal adapters plus ``AuditService(FilesystemCodeScanner,
    YAMLRuleRepository)``. Built in-process so the receipt never depends on the
    CLI's exit code (which is a by-design nonzero path on a blocked tree).
    """
    root = Path(root)
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


def _live_high_entries(
    drift_flags: list[dict], records: dict, required: set[str]
) -> list[dict]:
    """Resolve the live high-severity flags to registry records, failing loudly.

    Raises ``ReceiptError`` for a live high class with no record and for a record
    whose disposition is outside the closed vocabulary -- the two ways the
    criterion is not satisfied by the receipt's own fields. Only severities in
    ``required`` are held to the criterion (fail-closed to ``{"high"}`` by the
    caller when the registry declares none).
    """
    entries: list[dict] = []
    live = sorted(
        (f for f in drift_flags if f.get("severity") in required),
        key=lambda f: (_flag_class(f.get("flag", "")), str(f.get("flag", ""))),
    )
    for flag in live:
        flag_id = str(flag.get("flag", ""))
        flag_class = _flag_class(flag_id)
        record = records.get(flag_class)
        if not isinstance(record, dict):
            raise ReceiptError(
                f"live high-severity flag class {flag_class!r} has no record in "
                f"{_REGISTRY_RELATIVE}; refusing to write a green receipt"
            )
        disposition = record.get("disposition")
        if disposition not in DISPOSITION_VOCABULARY:
            raise ReceiptError(
                f"live high-severity flag class {flag_class!r} carries disposition "
                f"{disposition!r}, outside {sorted(DISPOSITION_VOCABULARY)}"
            )
        entries.append(
            {
                "flag": flag_id,
                "flag_class": flag_class,
                "severity": flag.get("severity"),
                "owner": record.get("owner"),
                "disposition": disposition,
                "reason": record.get("reason"),
                "decision_ref": record.get("decision_ref"),
            }
        )
    return entries


def _project_receipt(
    *,
    verdict: str,
    phase: str,
    drift_flags: list[dict],
    scorecard: dict,
    registry: dict,
    reconciliation: CloseoutCriterionDisposition,
    generated_at: str,
) -> dict:
    """Project a live report + registry + reconciliation into the receipt.

    Pure and side-effect free, so the fail-loud contract is testable with inline
    fixtures instead of a full tree build. Raises ``ReceiptError`` when the
    criterion is not satisfied on the inputs.
    """
    records = registry["records"]
    required = set(registry.get("required_for_severities") or ["high"])

    entries = _live_high_entries(drift_flags, records, required)

    emitted = [f for f in drift_flags if f.get("flag") == _SENTINEL]
    if reconciliation.findings or emitted:
        raise ReceiptError(
            "the S13 closeout-criterion reconciliation reported findings on the "
            f"live tree ({reconciliation.to_dict()}); refusing to write a green "
            "receipt"
        )

    axes = {
        entry["axis"]: entry["value"]
        for entry in scorecard.get("axes", [])
        if isinstance(entry, dict) and entry.get("axis")
    }
    cli_exit_code = 1 if verdict == "blocked" else 0

    return {
        "schema_version": _SCHEMA,
        "milestone": "M516-th0oby",
        "slice": "S13",
        "task": "T04",
        "generated_at": generated_at,
        "phase": phase,
        "verdict": verdict,
        "cli_exit_code": cli_exit_code,
        "criterion": {
            "id": "SC7",
            "clause": 1,
            "statement": (
                "governor preflight reports no high severity flag at closeout"
            ),
            "resolution": "satisfied_by_recorded_downgrade",
            "decision_ref": "D572",
            "note": (
                "Every live high-severity flag CLASS carries a recorded "
                "`downgraded` disposition (owner + reason + decision_ref D572); "
                "nothing is suppressed, no severity is changed and no check is "
                "weakened (D569). The clause is therefore satisfied by recorded "
                "downgrade, not by clearing a flag."
            ),
        },
        "registry_path": _REGISTRY_RELATIVE,
        "registry_record_count": len(records),
        "live_high_flags": entries,
        "live_high_flag_count": len(entries),
        "undisposed_count": len(reconciliation.undisposed),
        "contradiction_count": len(reconciliation.contradictions),
        "malformed_count": len(reconciliation.malformed),
        "finding_count": reconciliation.count,
        "scorecard_axes": {
            "measurability": axes.get("measurability"),
            "declaration_sync_gap": axes.get("declaration_sync_gap"),
            "feature_gated_coverage": axes.get("feature_gated_coverage"),
        },
        "verdict_note": (
            "The live `governor --phase preflight` verdict is recorded AS "
            f"OBSERVED ({verdict}); the CLI exit code is derived by the CLI's own "
            "rule (`scripts/governor/__main__.py`: 1 iff verdict == blocked). "
            "The plan-time expectation of `blocked` (exit 1) no longer holds "
            "because the reverse-ADR audit currently reports no blocking "
            "violation; S13 changes nothing about the verdict."
        ),
    }


def build_receipt(root: Path | str) -> dict:
    """Build the SC7 clause 1 closeout evidence receipt from the live tree.

    Fails loudly (``ReceiptError``) rather than writing a green receipt when the
    live tree does not satisfy the criterion. Returns the receipt document; the
    caller serializes it.
    """
    root = Path(root)
    registry = yaml.safe_load(
        (root / _REGISTRY_RELATIVE).read_text(encoding="utf-8")
    )
    if not isinstance(registry, dict) or not isinstance(registry.get("records"), dict):
        raise ReceiptError(
            f"{_REGISTRY_RELATIVE} must parse to a mapping with a `records` block"
        )

    _service, report = _live_report(root)
    drift_flags = list(report.drift_flags)
    verdict = (
        report.verdict.value
        if hasattr(report.verdict, "value")
        else str(report.verdict)
    )

    seeds: list[tuple[object, object]] = [
        (f.get("flag"), f.get("severity")) for f in drift_flags
    ]
    seeds.append((_SENTINEL, "high"))
    reconciliation = build_closeout_criterion_disposition(root, seeds)

    return _project_receipt(
        verdict=verdict,
        phase=report.phase,
        drift_flags=drift_flags,
        scorecard=report.scorecard,
        registry=registry,
        reconciliation=reconciliation,
        generated_at=datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
    )


# ── the validator ─────────────────────────────────────────────────────────────

def _require(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def validate_receipt(doc: dict, registry: dict) -> list[str]:
    """Return the receipt's contract violations (empty means it is valid).

    Checks the receipt's fields only -- never its bytes -- so the nondeterministic
    ``generated_at`` never makes the artifact flaky. The registry is passed in so
    the "every live high class resolves to a record" and "live classes are a
    subset of the declared classes" invariants are checked against the same DATA
    the generator resolved against.
    """
    errors: list[str] = []
    _require(
        doc.get("schema_version") == _SCHEMA,
        f"schema_version must be {_SCHEMA!r}, got {doc.get('schema_version')!r}",
        errors,
    )
    _require(doc.get("phase") == "preflight", "phase must be 'preflight'", errors)

    records = registry.get("records") if isinstance(registry, dict) else None
    records = records if isinstance(records, dict) else {}

    for field in (
        "undisposed_count",
        "contradiction_count",
        "malformed_count",
        "finding_count",
    ):
        _require(
            doc.get(field) == 0,
            f"{field} must be 0, got {doc.get(field)!r}",
            errors,
        )

    # Verdict and exit code are recorded honestly and must be mutually
    # consistent: the CLI returns 1 iff the verdict is `blocked`.
    verdict = doc.get("verdict")
    _require(verdict in _VERDICTS, f"verdict {verdict!r} not in {list(_VERDICTS)}", errors)
    expected_exit = 1 if verdict == "blocked" else 0
    _require(
        doc.get("cli_exit_code") == expected_exit,
        f"cli_exit_code {doc.get('cli_exit_code')!r} is inconsistent with verdict "
        f"{verdict!r} (expected {expected_exit})",
        errors,
    )

    # Registry linkage.
    _require(
        doc.get("registry_path") == _REGISTRY_RELATIVE,
        f"registry_path must be {_REGISTRY_RELATIVE!r}",
        errors,
    )
    _require(
        doc.get("registry_record_count") == len(records),
        "registry_record_count does not match the registry's records block",
        errors,
    )

    # Every live high flag: high severity, closed-vocabulary disposition, owner +
    # reason + decision_ref present, and a record in the registry.
    flags = doc.get("live_high_flags")
    _require(isinstance(flags, list) and flags, "live_high_flags missing/empty", errors)
    flags = flags if isinstance(flags, list) else []
    classes: list[str] = []
    for entry in flags:
        if not isinstance(entry, dict):
            errors.append("live high flag entry is not an object")
            continue
        flag_class = entry.get("flag_class")
        classes.append(flag_class)
        _require(
            entry.get("severity") == "high",
            f"{flag_class}: severity must be 'high'",
            errors,
        )
        _require(
            entry.get("disposition") in DISPOSITION_VOCABULARY,
            f"{flag_class}: disposition {entry.get('disposition')!r} is outside "
            f"{sorted(DISPOSITION_VOCABULARY)}",
            errors,
        )
        for field in ("owner", "reason", "decision_ref"):
            _require(
                isinstance(entry.get(field), str) and entry[field].strip(),
                f"{flag_class}: {field} missing/empty",
                errors,
            )
        _require(
            flag_class in records,
            f"{flag_class}: no record in {_REGISTRY_RELATIVE}",
            errors,
        )
    _require(len(classes) == len(set(classes)), "live high flag classes are not unique", errors)
    undeclared = sorted(set(classes) - set(records))
    _require(
        not undeclared,
        f"live high classes not declared in the registry: {undeclared}",
        errors,
    )
    _require(
        doc.get("live_high_flag_count") == len(flags),
        "live_high_flag_count does not match live_high_flags",
        errors,
    )

    # The three pinned scorecard axes must be recorded and unmoved.
    axes = doc.get("scorecard_axes")
    _require(isinstance(axes, dict), "scorecard_axes missing", errors)
    axes = axes if isinstance(axes, dict) else {}
    for axis, needle in _PINNED_AXES.items():
        value = axes.get(axis)
        _require(
            isinstance(value, str) and needle in value,
            f"scorecard axis {axis} value {value!r} does not contain {needle!r}",
            errors,
        )

    # The criterion is honestly recorded as a recorded downgrade.
    criterion = doc.get("criterion")
    _require(isinstance(criterion, dict), "criterion missing", errors)
    if isinstance(criterion, dict):
        _require(
            criterion.get("resolution") == "satisfied_by_recorded_downgrade",
            "criterion resolution must be 'satisfied_by_recorded_downgrade'",
            errors,
        )
        _require(
            criterion.get("decision_ref") == "D572",
            "criterion decision_ref must be 'D572'",
            errors,
        )
    return errors


# ── fixtures ──────────────────────────────────────────────────────────────────

def _registry() -> dict:
    return yaml.safe_load((_REPO_ROOT / _REGISTRY_RELATIVE).read_text(encoding="utf-8"))


def _committed_receipt() -> dict:
    return json.loads((_REPO_ROOT / _RECEIPT_RELATIVE).read_text(encoding="utf-8"))


def _silent_reconciliation() -> CloseoutCriterionDisposition:
    return CloseoutCriterionDisposition(undisposed=(), contradictions=(), malformed=())


def _finding_reconciliation() -> CloseoutCriterionDisposition:
    return CloseoutCriterionDisposition(
        undisposed=(
            DispositionFinding(
                flag_class="brand_new_high", kind="undisposed", detail="no record"
            ),
        ),
        contradictions=(),
        malformed=(),
    )


def _scorecard() -> dict:
    return {
        "axes": [
            {"axis": "measurability", "value": "5/21 (23.8%)"},
            {"axis": "declaration_sync_gap", "value": "0 gaps"},
            {"axis": "feature_gated_coverage", "value": "19/31 (61.3%)"},
        ]
    }


def _live_flags() -> list[dict]:
    """A minimal live report flag list: one disposed high class, one medium."""
    return [
        {
            "flag": "gsd_exec_timeout_evidence",
            "severity": "high",
            "evidence": "a timed-out capture",
            "owner": "gsd-exec-evidence",
        },
        {
            "flag": "resource_target_bloat",
            "severity": "medium",
            "evidence": "target/ is large",
            "owner": "resource-safety",
        },
    ]


# ── the committed artifact ────────────────────────────────────────────────────

def test_committed_receipt_exists_and_satisfies_the_contract() -> None:
    """The artifact exists and passes every contract invariant."""
    assert (_REPO_ROOT / _RECEIPT_RELATIVE).is_file(), (
        f"{_RECEIPT_RELATIVE} is missing; regenerate it with "
        "`PYTHONPATH=scripts .venv/bin/python "
        "scripts/tests/test_closeout_criterion_evidence_receipt.py`"
    )
    errors = validate_receipt(_committed_receipt(), _registry())
    assert errors == [], f"receipt contract violations: {errors}"


def test_committed_receipt_live_high_classes_are_a_subset_of_the_registry() -> None:
    """Every class the receipt names is declared by the DATA registry."""
    receipt = _committed_receipt()
    records = _registry()["records"]
    classes = {entry["flag_class"] for entry in receipt["live_high_flags"]}
    assert classes, "the live tree must emit at least one high flag for this test to bite"
    assert classes <= set(records), f"undeclared classes: {sorted(classes - set(records))}"


def test_committed_receipt_records_the_verdict_honestly() -> None:
    """The verdict/exit pair is recorded as observed and is internally consistent.

    On this tree the live preflight is ``drift_risk`` (exit 0), not the plan-time
    ``blocked``/1: the reverse-ADR audit reports no blocking violation. The
    receipt must never fabricate a blocked value, so the invariant pinned here is
    the CLI's own rule (1 iff blocked), not a hardcoded number.
    """
    receipt = _committed_receipt()
    verdict = receipt["verdict"]
    assert verdict in _VERDICTS
    assert receipt["cli_exit_code"] == (1 if verdict == "blocked" else 0)
    assert receipt["criterion"]["resolution"] == "satisfied_by_recorded_downgrade"
    assert receipt["criterion"]["decision_ref"] == "D572"
    assert receipt["verdict_note"]


# ── negative fixtures (inline; the positive path is the artifact itself) ──────

def test_missing_schema_version_is_flagged() -> None:
    doc = _committed_receipt()
    doc.pop("schema_version")
    assert any("schema_version" in e for e in validate_receipt(doc, _registry()))


def test_undisposed_count_must_be_zero() -> None:
    doc = _committed_receipt()
    doc["undisposed_count"] = 1
    assert any("undisposed_count" in e for e in validate_receipt(doc, _registry()))


def test_finding_count_must_be_zero() -> None:
    doc = _committed_receipt()
    doc["finding_count"] = 1
    assert any("finding_count" in e for e in validate_receipt(doc, _registry()))


def test_disposition_outside_the_vocabulary_is_flagged() -> None:
    doc = _committed_receipt()
    doc["live_high_flags"][0]["disposition"] = "cleared"
    assert any("disposition" in e for e in validate_receipt(doc, _registry()))


def test_live_class_without_a_registry_record_is_flagged() -> None:
    doc = _committed_receipt()
    doc["live_high_flags"].append(
        {
            "flag": "brand_new_high",
            "flag_class": "brand_new_high",
            "severity": "high",
            "owner": "someone",
            "disposition": "downgraded",
            "reason": "pre_existing_structural",
            "decision_ref": "D572",
        }
    )
    doc["live_high_flag_count"] = len(doc["live_high_flags"])
    errors = validate_receipt(doc, _registry())
    assert any("brand_new_high" in e for e in errors)


def test_missing_owner_is_flagged() -> None:
    doc = _committed_receipt()
    doc["live_high_flags"][0]["owner"] = ""
    assert any("owner" in e for e in validate_receipt(doc, _registry()))


def test_empty_live_high_flags_is_flagged() -> None:
    doc = _committed_receipt()
    doc["live_high_flags"] = []
    doc["live_high_flag_count"] = 0
    assert any("live_high_flags" in e for e in validate_receipt(doc, _registry()))


def test_inconsistent_verdict_and_exit_code_is_flagged() -> None:
    doc = _committed_receipt()
    doc["verdict"] = "drift_risk"
    doc["cli_exit_code"] = 1
    assert any("cli_exit_code" in e for e in validate_receipt(doc, _registry()))


def test_blocked_verdict_requires_exit_code_one() -> None:
    doc = _committed_receipt()
    doc["verdict"] = "blocked"
    doc["cli_exit_code"] = 0
    assert any("cli_exit_code" in e for e in validate_receipt(doc, _registry()))


def test_moved_scorecard_axis_is_flagged() -> None:
    doc = _committed_receipt()
    doc["scorecard_axes"]["measurability"] = "6/21 (28.6%)"
    assert any("measurability" in e for e in validate_receipt(doc, _registry()))


def test_non_downgrade_resolution_is_flagged() -> None:
    doc = _committed_receipt()
    doc["criterion"]["resolution"] = "measured_absent"
    assert any("resolution" in e for e in validate_receipt(doc, _registry()))


# ── the generator fails loudly rather than writing a green receipt ────────────

def test_generator_refuses_an_undisposed_live_class() -> None:
    """A new high class with no record must raise, never write a green receipt."""
    with pytest.raises(ReceiptError, match="no record"):
        _live_high_entries(
            [{"flag": "brand_new_high", "severity": "high"}], {}, {"high"}
        )


def test_generator_refuses_a_disposition_outside_the_vocabulary() -> None:
    with pytest.raises(ReceiptError, match="outside"):
        _live_high_entries(
            [{"flag": "x", "severity": "high"}],
            {"x": {"disposition": "cleared", "owner": "o", "reason": "r", "decision_ref": "D"}},
            {"high"},
        )


def test_generator_ignores_medium_flags() -> None:
    """Only `required_for_severities` fails closed; a medium flag is not evaluated."""
    entries = _live_high_entries(
        [{"flag": "medium_only", "severity": "medium"}], {}, {"high"}
    )
    assert entries == []


def test_project_receipt_raises_on_any_reconciliation_finding() -> None:
    with pytest.raises(ReceiptError, match="findings"):
        _project_receipt(
            verdict="drift_risk",
            phase="preflight",
            drift_flags=_live_flags(),
            scorecard=_scorecard(),
            registry=_registry(),
            reconciliation=_finding_reconciliation(),
            generated_at="2026-10-11T00:00:00+00:00",
        )


def test_project_receipt_raises_on_an_emitted_sentinel() -> None:
    """A report carrying the audit's own finding class is an unresolved class."""
    flags = _live_flags() + [
        {"flag": _SENTINEL, "severity": "high", "evidence": "a finding"}
    ]
    with pytest.raises(ReceiptError, match="findings"):
        _project_receipt(
            verdict="drift_risk",
            phase="preflight",
            drift_flags=flags,
            scorecard=_scorecard(),
            registry=_registry(),
            reconciliation=_silent_reconciliation(),
            generated_at="2026-10-11T00:00:00+00:00",
        )


def test_project_receipt_projects_a_disposed_high_flag() -> None:
    """The positive path: one disposed high class projects a valid receipt."""
    receipt = _project_receipt(
        verdict="drift_risk",
        phase="preflight",
        drift_flags=_live_flags(),
        scorecard=_scorecard(),
        registry=_registry(),
        reconciliation=_silent_reconciliation(),
        generated_at="2026-10-11T00:00:00+00:00",
    )
    assert validate_receipt(receipt, _registry()) == []
    assert [e["flag_class"] for e in receipt["live_high_flags"]] == [
        "gsd_exec_timeout_evidence"
    ]
    assert receipt["live_high_flag_count"] == 1
    assert receipt["cli_exit_code"] == 0
    # The generator must not mutate its inputs.
    flags = _live_flags()
    snapshot = copy.deepcopy(flags)
    _project_receipt(
        verdict="drift_risk",
        phase="preflight",
        drift_flags=flags,
        scorecard=_scorecard(),
        registry=_registry(),
        reconciliation=_silent_reconciliation(),
        generated_at="2026-10-11T00:00:00+00:00",
    )
    assert flags == snapshot


if __name__ == "__main__":  # pragma: no cover - regeneration entry point
    receipt = build_receipt(_REPO_ROOT)
    path = _REPO_ROOT / _RECEIPT_RELATIVE
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(receipt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(
        f"wrote {_RECEIPT_RELATIVE}: verdict={receipt['verdict']} "
        f"exit={receipt['cli_exit_code']} "
        f"high={receipt['live_high_flag_count']} "
        f"undisposed={receipt['undisposed_count']} "
        f"findings={receipt['finding_count']}"
    )
