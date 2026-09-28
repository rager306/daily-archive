"""Readers and signal for the commit-hook failure class.

The class is not benign: a failed auto-commit cannot be retried by the engine, so the
task attempt dies with a claim mismatch and auto-mode wedges. Two things must hold:
the reader names only hooks that actually refused a commit, and the signal clears when
a later commit resolves the failures.
"""

import json
import time
from datetime import datetime, timezone
from pathlib import Path

from governor.adapters.git_action_failures_reader import (
    GitActionFailuresReader,
    hook_counts,
    refusing_hooks,
)
from governor.adapters.task_verification_reader import rust_verification_coverage
from governor.checks.gsd_control_plane import evaluate
from governor.application.trajectory_service import TrajectoryService

BLOCK_WITH_TRAILING_SKIPPED_HOOK = """[2026-09-27T04:38:26.411Z] action=commit
project: git commit failed: cargo fmt (kg-* + da-*)..........................................Failed
- hook id: cargo-fmt
- exit code: 1

Diff in /repo/crates/da-cli/tests/example_test.rs:1499:
-        .args(["a", "b"])
+        .args([
+            "a",
+            "b",
+        ])

cargo clippy (lib only, no --all-targets, avoids rocksdb).........................Passed
docs drift check (schema tables present)................................(no files to check)Skipped
- hook id: docs-drift-check
"""

BLOCK_WITH_TWO_REFUSALS = """[2026-09-20T06:00:00.000Z] action=commit
project: git commit failed: cargo fmt (kg-* + da-*)......Failed
- hook id: cargo-fmt
- exit code: 1

cargo clippy tests (test modules compile)...............Failed
- hook id: cargo-clippy-tests
- exit code: 101
"""


def _log(root: Path, *blocks: str) -> None:
    path = root / ".gsd" / "git-action-failures.log"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(blocks), encoding="utf-8")


def test_trailing_skipped_hook_is_not_counted_as_a_refusal() -> None:
    # Measured on the real log: `docs-drift-check` is written at the end of all 188
    # blocks without an exit code and has never refused a commit. Counting bare
    # `hook id` lines named a hook that never failed.
    assert refusing_hooks(BLOCK_WITH_TRAILING_SKIPPED_HOOK) == ("cargo-fmt",)


def test_every_refusing_hook_in_a_block_is_reported() -> None:
    assert refusing_hooks(BLOCK_WITH_TWO_REFUSALS) == ("cargo-fmt", "cargo-clippy-tests")


def test_reader_ignores_failures_at_or_before_the_commit_boundary(tmp_path: Path) -> None:
    _log(tmp_path, BLOCK_WITH_TRAILING_SKIPPED_HOOK, BLOCK_WITH_TWO_REFUSALS)
    reader = GitActionFailuresReader(tmp_path)
    # Nothing is newer than 2026-09-27T05:00Z except nothing: 04:38 is older.
    since = datetime(2026, 9, 27, 5, 0, tzinfo=timezone.utc)
    assert reader.failures_since(since) == []
    older = datetime(2026, 9, 27, 4, 0, tzinfo=timezone.utc)
    assert [f.timestamp for f in reader.failures_since(older)] == ["2026-09-27T04:38:26.411Z"]


def test_reader_is_best_effort_on_a_missing_log(tmp_path: Path) -> None:
    assert GitActionFailuresReader(tmp_path).failures_since(None) == []


def test_hook_counts_name_the_dominant_hook(tmp_path: Path) -> None:
    _log(tmp_path, BLOCK_WITH_TRAILING_SKIPPED_HOOK, BLOCK_WITH_TWO_REFUSALS)
    failures = GitActionFailuresReader(tmp_path).failures_since(None)
    assert hook_counts(failures) == {"cargo-fmt": 2, "cargo-clippy-tests": 1}


def _verify(root: Path, name: str, commands: list[str], age_days: float) -> None:
    phase = root / ".gsd" / "phases" / "999-abc-def"
    phase.mkdir(parents=True, exist_ok=True)
    stamp = int((time.time() - age_days * 86400) * 1000)
    (phase / name).write_text(
        json.dumps(
            {
                "schemaVersion": 1,
                "taskId": "T01",
                "passed": True,
                "timestamp": stamp,
                "checks": [{"command": c, "exitCode": 0, "verdict": "pass"} for c in commands],
            }
        ),
        encoding="utf-8",
    )


def test_fmt_coverage_counts_only_cargo_verified_tasks(tmp_path: Path) -> None:
    _verify(tmp_path, "S01-T01-VERIFY.json", ["cargo test -p da-parse --test x"], 1)
    _verify(
        tmp_path,
        "S01-T02-VERIFY.json",
        ["cargo test -p da-parse --test y", "cargo fmt -p da-parse -- --check"],
        1,
    )
    _verify(tmp_path, "S01-T03-VERIFY.json", ["grep -c X crates/da-parse/src/lib.rs"], 1)
    coverage = rust_verification_coverage(tmp_path, 7)
    # The grep-only task is not Rust work for this purpose.
    assert coverage == {"rust_tasks": 2, "rust_tasks_with_fmt_check": 1}


def test_fmt_coverage_respects_the_window(tmp_path: Path) -> None:
    _verify(tmp_path, "S01-T01-VERIFY.json", ["cargo test -p da-parse --test x"], 1)
    _verify(tmp_path, "S01-T02-VERIFY.json", ["cargo test -p da-parse --test y"], 40)
    assert rust_verification_coverage(tmp_path, 7)["rust_tasks"] == 1
    assert rust_verification_coverage(tmp_path, 90)["rust_tasks"] == 2


_CONTRACT = {
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
        "doctor_latest": {"ok": False, "errors": 0, "warnings": 0, "issues": []},
        "doctor_baseline_entries": [],
        "doctor_baseline_present": True,
        "commit_failures": 0,
        "commit_failure_hooks": {},
        "rust_verification": {"rust_tasks": 0, "rust_tasks_with_fmt_check": 0},
    }
    base.update(overrides)
    return base


def test_commit_failures_signal_names_the_hook_and_the_cause() -> None:
    observations = _observations(
        commit_failures=3,
        commit_failure_hooks={"cargo-fmt": 3},
        rust_verification={"rust_tasks": 54, "rust_tasks_with_fmt_check": 10},
    )
    findings = evaluate(_CONTRACT, observations)
    assert [f.signal for f in findings] == ["commit_hook_failures"]
    assert findings[0].severity == "high"
    assert "cargo-fmt=3" in findings[0].evidence
    # The recorded verifications explain the cause instead of the report guessing.
    assert "44 of 54" in findings[0].evidence
    assert "cargo fmt" in findings[0].evidence


def test_commit_failures_signal_is_absent_when_the_tree_moved_on() -> None:
    assert evaluate(_CONTRACT, _observations()) == []


def test_commit_failures_without_recorded_verification_still_reports() -> None:
    observations = _observations(
        commit_failures=1,
        commit_failure_hooks={"governor-trajectory": 1},
        rust_verification=None,
    )
    findings = evaluate(_CONTRACT, observations)
    assert findings[0].signal == "commit_hook_failures"
    assert "verification unavailable" in findings[0].evidence


def test_wiring_passes_the_commit_boundary_into_the_reader(tmp_path: Path) -> None:
    # A commit after the failure must silence the signal, so the wiring has to thread
    # the last commit date into the reader rather than counting all history.
    contract = tmp_path / "scripts" / "governor" / "checks"
    contract.mkdir(parents=True)
    import yaml

    (contract / "gsd_control_plane.yaml").write_text(yaml.safe_dump(_CONTRACT), encoding="utf-8")
    (contract / "gsd_doctor_baseline.json").write_text(json.dumps({"accepted": []}), encoding="utf-8")
    _log(tmp_path, BLOCK_WITH_TRAILING_SKIPPED_HOOK)

    class FakeGitReader:
        def __init__(self, date: str) -> None:
            self._date = date

        def is_repo(self) -> bool:
            return True

        def get_git_info(self):
            return type("Info", (), {"latest_commit_date": self._date})()

    def flags_for(date: str):
        service = TrajectoryService.__new__(TrajectoryService)
        service.root = tmp_path
        service.git_reader = FakeGitReader(date)
        return [flag.flag for flag in service._check_gsd_control_plane()]

    # The failure block is stamped 2026-09-27T04:38:26Z. A commit boundary *earlier* than
    # it leaves the failure live; a commit *after* it resolves the class.
    commit_before_failure = datetime(2026, 9, 27, 4, 0, tzinfo=timezone.utc)
    commit_after_failure = datetime(2026, 9, 27, 5, 0, tzinfo=timezone.utc)
    assert "gsd_control_plane_commit_hook_failures" in flags_for(
        commit_before_failure.strftime("%Y-%m-%d %H:%M:%S +0000")
    )
    assert "gsd_control_plane_commit_hook_failures" not in flags_for(
        commit_after_failure.strftime("%Y-%m-%d %H:%M:%S +0000")
    )
