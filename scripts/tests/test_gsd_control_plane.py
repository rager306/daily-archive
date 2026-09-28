"""Control-plane health check: oracle decisions, reader bounds, and wiring.

The oracle is pure, so most cases are decided against hand-written observations.
The reader is exercised against a tmp_path fixture that mimics the engine's store
layout, and the trajectory wiring is checked end to end once.
"""

import json
from pathlib import Path

import yaml

from governor.adapters.gsd_control_plane_reader import GsdControlPlaneReader
from governor.application.trajectory_service import TrajectoryService
from governor.checks.gsd_control_plane import evaluate

CONTRACT_FIXTURE = {
    "schema_version": "gsd-control-plane.v1",
    "window_days": 7,
    "thresholds": {
        "projection_contention_max": 0,
        "liveness_wedge_max": 0,
        "lease_conflict_max": 0,
        "tech_verdict_block_max": 0,
        "unretained_exec_captures_max": 0,
        "unretained_quarantine_runs_max": 0,
        "tmp_scratch_dirs_max": 200,
        "commit_hook_failures_max": 0,
    },
    "retention": {
        "days_exec": 30,
        "days_quarantine": 30,
        "days_activity": 14,
        "min_age_hours": 24,
    },
    "signals": [
        "contract_invalid",
        "observations_incomplete",
        "projection_contention",
        "liveness_wedge",
        "lease_conflict",
        "tech_verdict_block",
        "orphan_projection_dir",
        "unretained_exec_captures",
        "unretained_quarantine_runs",
        "commit_hook_failures",
        "tmp_scratch_bloat",
        "doctor_baseline_missing",
        "doctor_new_divergence",
    ],
    "journal_markers": {
        "projection_contention": "Projection mutation remains fenced",
        "liveness_wedge": "liveness backstop tripped",
        "lease_conflict": "requires the current held lease",
        "tech_verdict_block": "passing host Technical Verdict",
    },
    "high_severity_signals": [
        "projection_contention",
        "lease_conflict",
        "orphan_projection_dir",
        "doctor_new_divergence",
        "commit_hook_failures",
    ],
    "doctor_baseline_path": "scripts/governor/checks/gsd_doctor_baseline.json",
}


def _observations(**overrides):
    base = {
        "journal_counts": {
            "projection_contention": 0,
            "liveness_wedge": 0,
            "lease_conflict": 0,
            "tech_verdict_block": 0,
        },
        "unretained_exec_captures": 0,
        "unretained_quarantine_runs": 0,
        "tmp_scratch_dirs": 0,
        "orphan_projection_dir": False,
        "doctor_latest": {"ts": "2026-09-27T10:00:00Z", "ok": False, "errors": 0, "warnings": 0, "issues": []},
        "doctor_baseline_entries": [],
        "doctor_baseline_present": True,
        "commit_failures": 0,
        "commit_failure_hooks": {},
        "rust_verification": {"rust_tasks": 0, "rust_tasks_with_fmt_check": 0},
    }
    base.update(overrides)
    return base


# ── fail-closed contract handling ──────────────────────────────────────────


def test_missing_contract_is_a_high_flag_not_a_silent_pass() -> None:
    findings = evaluate(None, _observations())
    assert [finding.signal for finding in findings] == ["contract_invalid"]
    assert findings[0].severity == "high"


def test_empty_signal_vocabulary_cannot_decide() -> None:
    contract = dict(CONTRACT_FIXTURE, signals=[])
    findings = evaluate(contract, _observations())
    assert [finding.signal for finding in findings] == ["contract_invalid"]


def test_missing_threshold_and_non_integer_threshold_are_refused() -> None:
    for broken in (
        {k: v for k, v in CONTRACT_FIXTURE["thresholds"].items() if k != "lease_conflict_max"},
        dict(CONTRACT_FIXTURE["thresholds"], projection_contention_max="0"),
    ):
        contract = dict(CONTRACT_FIXTURE, thresholds=broken)
        assert [finding.signal for finding in evaluate(contract, _observations())] == [
            "contract_invalid"
        ]


def test_missing_journal_marker_is_refused() -> None:
    contract = dict(
        CONTRACT_FIXTURE,
        journal_markers={"projection_contention": "x", "liveness_wedge": "y", "lease_conflict": "z"},
    )
    assert [finding.signal for finding in evaluate(contract, _observations())] == [
        "contract_invalid"
    ]


def test_incomplete_observations_are_refused() -> None:
    observations = _observations()
    del observations["unretained_quarantine_runs"]
    assert [finding.signal for finding in evaluate(CONTRACT_FIXTURE, observations)] == [
        "observations_incomplete"
    ]


def test_undeclared_signal_is_a_contract_violation() -> None:
    contract = dict(CONTRACT_FIXTURE)
    contract["signals"] = [s for s in CONTRACT_FIXTURE["signals"] if s != "orphan_projection_dir"]
    findings = evaluate(contract, _observations(orphan_projection_dir=True))
    assert [finding.signal for finding in findings] == ["contract_invalid"]
    assert "undeclared signal" in findings[0].evidence


# ── measured-clean and threshold breaches ──────────────────────────────────


def test_clean_control_plane_reports_nothing() -> None:
    # An empty finding list must mean "measured and clean".
    assert evaluate(CONTRACT_FIXTURE, _observations()) == []


def test_projection_contention_is_high_and_names_the_marker() -> None:
    observations = _observations(
        journal_counts={
            "projection_contention": 3,
            "liveness_wedge": 0,
            "lease_conflict": 0,
            "tech_verdict_block": 0,
        }
    )
    findings = evaluate(CONTRACT_FIXTURE, observations)
    assert [finding.signal for finding in findings] == ["projection_contention"]
    assert findings[0].severity == "high"
    assert "3 occurrence(s)" in findings[0].evidence
    assert "Projection mutation remains fenced" in findings[0].evidence


def test_lease_conflict_is_high_while_wedge_and_verdict_are_medium() -> None:
    observations = _observations(
        journal_counts={
            "projection_contention": 0,
            "liveness_wedge": 2,
            "lease_conflict": 5,
            "tech_verdict_block": 1,
        }
    )
    severities = {finding.signal: finding.severity for finding in evaluate(CONTRACT_FIXTURE, observations)}
    assert severities == {
        "liveness_wedge": "medium",
        "lease_conflict": "high",
        "tech_verdict_block": "medium",
    }


def test_unretained_entries_flag_only_above_threshold() -> None:
    at_limit = _observations(
        unretained_exec_captures=0, unretained_quarantine_runs=0, tmp_scratch_dirs=200
    )
    assert evaluate(CONTRACT_FIXTURE, at_limit) == []
    over = _observations(
        unretained_exec_captures=1, unretained_quarantine_runs=2, tmp_scratch_dirs=201
    )
    findings = evaluate(CONTRACT_FIXTURE, over)
    assert {finding.signal for finding in findings} == {
        "unretained_exec_captures",
        "unretained_quarantine_runs",
        "tmp_scratch_bloat",
    }
    # Every retention signal must name the command that clears it.
    assert all("governor hygiene" in finding.evidence for finding in findings)


def test_missing_retention_block_is_refused() -> None:
    contract = dict(CONTRACT_FIXTURE)
    contract.pop("retention")
    assert [finding.signal for finding in evaluate(contract, _observations())] == [
        "contract_invalid"
    ]
    broken = dict(CONTRACT_FIXTURE, retention={**CONTRACT_FIXTURE["retention"], "days_exec": -1})
    assert [finding.signal for finding in evaluate(broken, _observations())] == [
        "contract_invalid"
    ]


def test_orphan_projection_dir_is_high() -> None:
    findings = evaluate(CONTRACT_FIXTURE, _observations(orphan_projection_dir=True))
    assert findings[0].signal == "orphan_projection_dir"
    assert findings[0].severity == "high"
    assert "misplaced" in findings[0].evidence


# ── doctor baseline ────────────────────────────────────────────────────────


def test_absent_doctor_baseline_is_reported_as_medium() -> None:
    observations = _observations(
        doctor_baseline_present=False,
        doctor_latest={"ok": False, "errors": 93, "warnings": 13, "issues": []},
    )
    findings = evaluate(CONTRACT_FIXTURE, observations)
    assert [finding.signal for finding in findings] == ["doctor_baseline_missing"]
    assert findings[0].severity == "medium"
    assert "errors=93" in findings[0].evidence


def test_scoped_baseline_suppresses_only_its_own_milestone() -> None:
    observations = _observations(
        doctor_latest={
            "ok": False,
            "errors": 93,
            "warnings": 13,
            "issues": [
                {
                    "code": "checkbox_db_status_divergence",
                    "severity": "error",
                    "message": "Task M465-sqt7w0/S01/T01 is closed in the database (status: skipped) but the markdown checkbox is unchecked.",
                },
                {
                    "code": "checkbox_db_status_divergence",
                    "severity": "error",
                    "message": "Task M600-abcdef/S01/T01 is closed in the database (status: skipped) but the markdown checkbox is unchecked.",
                },
                {
                    "code": "stale_uncommitted_changes",
                    "severity": "warning",
                    "message": "Working tree has uncommitted changes.",
                },
            ],
        },
        doctor_baseline_entries=[
            {
                "code": "checkbox_db_status_divergence",
                "scope": "milestones",
                "milestones": ["M465"],
            }
        ],
    )
    findings = evaluate(CONTRACT_FIXTURE, observations)
    assert [finding.signal for finding in findings] == ["doctor_new_divergence"]
    evidence = findings[0].evidence
    # The baselined M465 occurrence is silent; the live M600 one and the unscoped
    # code still surface — suppression must not become blanket.
    assert "checkbox_db_status_divergence@M600" in evidence
    assert "stale_uncommitted_changes@unscoped" in evidence
    assert "M465" not in evidence


def test_baseline_matches_milestone_named_as_a_phase_path() -> None:
    # Doctor messages name milestones either as `M465-xxxx/S01/T01` or inside a phase
    # path such as `phases/500-qk6nr1-.../500-02-RESEARCH.md`. Only the first shape was
    # recognised, so a scoped baseline could never cover the second.
    observations = _observations(
        doctor_latest={
            "ok": False,
            "errors": 1,
            "warnings": 1,
            "issues": [
                {
                    "code": "artifact_user_content_missing",
                    "severity": "warning",
                    "message": "Artifact `phases/500-qk6nr1-native-family-promotion-gate-dry-run/500-02-RESEARCH.md` is a user-authored RESEARCH file recorded in the database but missing from disk.",
                },
                {
                    "code": "artifact_user_content_missing",
                    "severity": "warning",
                    "message": "Artifact `phases/601-abcdef-live/601-02-RESEARCH.md` is a user-authored RESEARCH file recorded in the database but missing from disk.",
                },
            ],
        },
        doctor_baseline_entries=[
            {"code": "artifact_user_content_missing", "scope": "milestones", "milestones": ["M500"]}
        ],
    )
    evidence = evaluate(CONTRACT_FIXTURE, observations)[0].evidence
    assert "artifact_user_content_missing@M601" in evidence
    assert "M500" not in evidence


def test_code_wide_baseline_entry_suppresses_everywhere() -> None:
    observations = _observations(
        doctor_latest={
            "ok": False,
            "errors": 1,
            "warnings": 0,
            "issues": [
                {
                    "code": "artifact_user_content_missing",
                    "severity": "warning",
                    "message": "Artifact sample has no user content.",
                }
            ],
        },
        doctor_baseline_entries=[
            {"code": "artifact_user_content_missing", "scope": "code", "milestones": []}
        ],
    )
    assert evaluate(CONTRACT_FIXTURE, observations) == []


# ── reader bounds ──────────────────────────────────────────────────────────


def _journal(root: Path, day: str, *lines: str) -> None:
    journal_dir = root / ".gsd" / "journal"
    journal_dir.mkdir(parents=True, exist_ok=True)
    (journal_dir / f"{day}.jsonl").write_text("\n".join(lines), encoding="utf-8")


def test_reader_counts_markers_inside_the_window_only(tmp_path: Path) -> None:
    _journal(tmp_path, "2026-09-27", "Projection mutation remains fenced a", "Projection mutation remains fenced b")
    _journal(tmp_path, "2026-01-01", "Projection mutation remains fenced stale")
    reader = GsdControlPlaneReader(tmp_path, tmp_dir=tmp_path / "tmp")
    counts = reader.observe(7, CONTRACT_FIXTURE["journal_markers"])["journal_counts"]
    assert counts["projection_contention"] == 2


def test_reader_ignores_unparsable_journal_names(tmp_path: Path) -> None:
    _journal(tmp_path, "not-a-date", "Projection mutation remains fenced x")
    counts = GsdControlPlaneReader(tmp_path, tmp_dir=tmp_path / "tmp").observe(
        7, CONTRACT_FIXTURE["journal_markers"]
    )["journal_counts"]
    assert counts["projection_contention"] == 0


def test_reader_reports_orphan_dir_and_baseline_entries(tmp_path: Path) -> None:
    (tmp_path / ".gsd" / "gsd" / "phases").mkdir(parents=True)
    baseline = tmp_path / "baseline.json"
    baseline.write_text(
        json.dumps({"accepted": [{"code": "x", "milestones": ["M1"]}]}), encoding="utf-8"
    )
    observations = GsdControlPlaneReader(tmp_path, tmp_dir=tmp_path / "tmp").observe(
        7, CONTRACT_FIXTURE["journal_markers"], str(baseline)
    )
    assert observations["orphan_projection_dir"] is True
    assert observations["doctor_baseline_present"] is True
    assert observations["doctor_baseline_entries"] == [
        {"code": "x", "scope": "milestones", "milestones": ["M1"]}
    ]


def test_reader_takes_the_latest_doctor_record_and_samples_issues(tmp_path: Path) -> None:
    history = tmp_path / ".gsd" / "doctor-history.jsonl"
    history.parent.mkdir(parents=True)
    history.write_text(
        "\n".join(
            [
                json.dumps({"ts": "2026-09-01T00:00:00Z", "ok": True, "errors": 0}),
                "{not json",
                json.dumps(
                    {
                        "ts": "2026-09-27T10:29:10Z",
                        "ok": False,
                        "errors": 93,
                        "warnings": 13,
                        "issues": [
                            {"severity": "error", "code": "checkbox_db_status_divergence", "message": "M465"},
                        ],
                    }
                ),
            ]
        ),
        encoding="utf-8",
    )
    latest = GsdControlPlaneReader(tmp_path, tmp_dir=tmp_path / "tmp").observe(
        7, CONTRACT_FIXTURE["journal_markers"]
    )["doctor_latest"]
    assert latest["ts"] == "2026-09-27T10:29:10Z"
    assert latest["errors"] == 93
    assert latest["issues"] == [
        {"code": "checkbox_db_status_divergence", "severity": "error", "message": "M465"}
    ]


def test_reader_counts_scratch_dirs_by_prefix(tmp_path: Path) -> None:
    tmp_dir = tmp_path / "tmp"
    (tmp_dir / "governor-test-aaa").mkdir(parents=True)
    (tmp_dir / "governor-test-bbb").mkdir()
    (tmp_dir / "unrelated").mkdir()
    observations = GsdControlPlaneReader(tmp_path, tmp_dir=tmp_dir).observe(
        7, CONTRACT_FIXTURE["journal_markers"]
    )
    assert observations["tmp_scratch_dirs"] == 2


# ── wiring ─────────────────────────────────────────────────────────────────


def test_trajectory_flags_contention_end_to_end(tmp_path: Path) -> None:
    _journal(tmp_path, "2026-09-27", "Projection mutation remains fenced")
    contract_dir = tmp_path / "scripts" / "governor" / "checks"
    contract_dir.mkdir(parents=True)
    (contract_dir / "gsd_control_plane.yaml").write_text(
        yaml.safe_dump(CONTRACT_FIXTURE), encoding="utf-8"
    )
    (contract_dir / "gsd_doctor_baseline.json").write_text(
        json.dumps({"accepted": []}), encoding="utf-8"
    )
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = tmp_path
    service.git_reader = None
    flags = service._check_gsd_control_plane()
    signals = [flag.flag for flag in flags]
    assert "gsd_control_plane_projection_contention" in signals
    contention = next(f for f in flags if f.flag == "gsd_control_plane_projection_contention")
    assert contention.severity == "high"
    assert contention.dimension == "gsd_control_plane"


def test_trajectory_fails_closed_when_contract_is_missing(tmp_path: Path) -> None:
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = tmp_path
    service.git_reader = None
    flags = service._check_gsd_control_plane()
    assert [flag.flag for flag in flags] == ["gsd_control_plane_contract_invalid"]
    assert flags[0].severity == "high"


def test_trajectory_fails_closed_when_contract_is_malformed(tmp_path: Path) -> None:
    contract_dir = tmp_path / "scripts" / "governor" / "checks"
    contract_dir.mkdir(parents=True)
    (contract_dir / "gsd_control_plane.yaml").write_text("thresholds: {}\n", encoding="utf-8")
    service = TrajectoryService.__new__(TrajectoryService)
    service.root = tmp_path
    service.git_reader = None
    flags = service._check_gsd_control_plane()
    assert [flag.flag for flag in flags] == ["gsd_control_plane_contract_invalid"]
