"""Doctor-baseline command: the supported way to accept engine-doctor divergence.

The service exists so nobody hand-edits the JSON and gets the schema wrong. The
behaviours that matter are the refusals: a baseline that can be widened carelessly
stops being a signal, which is the whole reason it was introduced.
"""

import json
from pathlib import Path

import pytest

from governor.application.doctor_baseline_service import (
    BASELINE_PATH,
    BaselineError,
    accept,
    load_baseline,
    report,
)

_CONTRACT = "doctor_baseline_path: scripts/governor/checks/gsd_doctor_baseline.json\n"


def _project(tmp_path: Path, *issues: dict, baseline: dict | None = None) -> Path:
    checks = tmp_path / "scripts" / "governor" / "checks"
    checks.mkdir(parents=True, exist_ok=True)
    (checks / "gsd_control_plane.yaml").write_text(_CONTRACT, encoding="utf-8")
    history = tmp_path / ".gsd" / "doctor-history.jsonl"
    history.parent.mkdir(parents=True, exist_ok=True)
    history.write_text(
        json.dumps(
            {
                "ts": "2026-09-27T10:29:10Z",
                "ok": False,
                "errors": 93,
                "warnings": 13,
                "issues": list(issues),
            }
        ),
        encoding="utf-8",
    )
    if baseline is not None:
        (tmp_path / BASELINE_PATH).write_text(json.dumps(baseline), encoding="utf-8")
    return tmp_path


def _issue(code: str, message: str, severity: str = "error") -> dict:
    return {"code": code, "message": message, "severity": severity}


_SKIPPED = _issue(
    "checkbox_db_status_divergence",
    "Task M465-sqt7w0/S01/T01 is closed in the database (status: skipped) but the markdown checkbox is unchecked.",
)

_STALE = _issue("stale_uncommitted_changes", "Uncommitted changes detected.", severity="warning")


def test_report_lists_divergence_not_covered_by_the_baseline(tmp_path: Path) -> None:
    # The reader must read the contract's baseline path; defaulting to the `.gsd`
    # location made every accepted divergence look unbaselined.
    root = _project(
        tmp_path,
        _SKIPPED,
        _STALE,
        baseline={
            "accepted": [
                {"code": "checkbox_db_status_divergence", "scope": "milestones", "milestones": ["M465"]}
            ]
        },
    )
    current = report(root)
    assert current.unbaselined == ("stale_uncommitted_changes@unscoped",)
    assert current.accepted_count == 1
    assert current.latest_errors == 93
    assert set(current.available_codes) == {
        "checkbox_db_status_divergence",
        "stale_uncommitted_changes",
    }


def test_accept_requires_a_code_the_latest_run_reports(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED)
    with pytest.raises(BaselineError, match="does not report"):
        accept(root, code="no_such_code", milestone="M465")


def test_accept_requires_a_scope(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED)
    with pytest.raises(BaselineError, match="specific --milestone or say --code-wide"):
        accept(root, code="checkbox_db_status_divergence")


def test_code_wide_acceptance_requires_a_reason(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED)
    with pytest.raises(BaselineError, match="pass --reason"):
        accept(root, code="checkbox_db_status_divergence", code_wide=True)


def test_milestone_must_look_like_a_milestone(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED)
    with pytest.raises(BaselineError, match="not a milestone id"):
        accept(root, code="checkbox_db_status_divergence", milestone="465")


def test_scoped_acceptance_merges_milestones_instead_of_replacing(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED)
    accept(root, code="checkbox_db_status_divergence", milestone="M465", evidence="legacy renderer")
    entry = accept(root, code="checkbox_db_status_divergence", milestone="M600", evidence="same shape")
    assert entry["milestones"] == ["M465", "M600"]
    assert entry["scope"] == "milestones"
    assert entry["evidence"] == "same shape"
    assert entry["recorded_at"]
    # One entry per (code, scope), not one per call.
    assert len(load_baseline(root)["accepted"]) == 1


def test_acceptance_clears_the_unbaselined_list(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED, _STALE)
    assert report(root).unbaselined == (
        "checkbox_db_status_divergence@M465",
        "stale_uncommitted_changes@unscoped",
    )
    accept(root, code="checkbox_db_status_divergence", milestone="M465")
    accept(root, code="stale_uncommitted_changes", code_wide=True, reason="transient by nature")
    assert report(root).unbaselined == ()


def test_acceptance_is_recorded_who_and_why(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED)
    accept(
        root,
        code="checkbox_db_status_divergence",
        milestone="M465",
        evidence="searched quarantine and backups",
        recorded_by="human",
    )
    written = json.loads((root / BASELINE_PATH).read_text(encoding="utf-8"))
    entry = written["accepted"][0]
    assert entry["recorded_by"] == "human"
    assert entry["evidence"] == "searched quarantine and backups"
    assert written["schema_version"] == "gsd-doctor-baseline.v1"


def test_unreadable_baseline_is_reported_not_overwritten(tmp_path: Path) -> None:
    root = _project(tmp_path, _SKIPPED)
    (root / BASELINE_PATH).write_text("{ not json", encoding="utf-8")
    with pytest.raises(BaselineError, match="not readable JSON"):
        load_baseline(root)
    with pytest.raises(BaselineError):
        report(root)
