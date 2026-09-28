from datetime import datetime, timezone
import json
from pathlib import Path

from governor.adapters.gsd_exec_reader import GsdExecReader
from governor.application.trajectory_service import TrajectoryService


def _evidence(
    root: Path,
    evidence_id: str,
    *,
    exit_code: int,
    output: str,
    finished_at: str,
    timed_out: bool = False,
    duration_ms: int = 0,
) -> None:
    exec_dir = root / ".gsd" / "exec"
    exec_dir.mkdir(parents=True, exist_ok=True)
    stdout = exec_dir / f"{evidence_id}.stdout"
    stderr = exec_dir / f"{evidence_id}.stderr"
    stdout.write_text(output, encoding="utf-8")
    stderr.write_text("", encoding="utf-8")
    (exec_dir / f"{evidence_id}.meta.json").write_text(
        json.dumps(
            {
                "id": evidence_id,
                "purpose": "test verifier",
                "finished_at": finished_at,
                "exit_code": exit_code,
                "timed_out": timed_out,
                "duration_ms": duration_ms,
                "stdout_path": str(stdout),
                "stderr_path": str(stderr),
            }
        ),
        encoding="utf-8",
    )


def test_flags_exit_zero_with_explicit_test_failure_after_commit(tmp_path: Path) -> None:
    _evidence(
        tmp_path,
        "false-green",
        exit_code=0,
        output="test result: FAILED. 0 passed; 1 failed\ncontinued shell command",
        finished_at="2026-08-08T04:05:46Z",
    )
    findings = GsdExecReader(tmp_path).false_successes_since(
        datetime(2026, 8, 8, 4, 0, tzinfo=timezone.utc)
    )
    assert [finding.evidence_id for finding in findings] == ["false-green"]
    assert findings[0].marker == "test result: FAILED"


def test_ignores_real_failure_and_clean_success(tmp_path: Path) -> None:
    _evidence(
        tmp_path,
        "real-failure",
        exit_code=1,
        output="test result: FAILED",
        finished_at="2026-08-08T04:05:46Z",
    )
    _evidence(
        tmp_path,
        "clean-success",
        exit_code=0,
        output="test result: ok. 7 passed; 0 failed",
        finished_at="2026-08-08T04:05:47Z",
    )
    assert GsdExecReader(tmp_path).false_successes_since(None) == []


def test_commit_boundary_acknowledges_older_false_success(tmp_path: Path) -> None:
    _evidence(
        tmp_path,
        "reviewed-failure",
        exit_code=0,
        output="test result: FAILED",
        finished_at="2026-08-08T03:00:00Z",
    )
    findings = GsdExecReader(tmp_path).false_successes_since(
        datetime(2026, 8, 8, 4, 0, tzinfo=timezone.utc)
    )
    assert findings == []


def test_missing_or_corrupt_metadata_is_best_effort(tmp_path: Path) -> None:
    exec_dir = tmp_path / ".gsd" / "exec"
    exec_dir.mkdir(parents=True)
    (exec_dir / "broken.meta.json").write_text("{", encoding="utf-8")
    assert GsdExecReader(tmp_path).false_successes_since(None) == []


def test_trajectory_service_emits_high_flag_for_new_false_success(tmp_path: Path) -> None:
    _evidence(
        tmp_path,
        "new-false-green",
        exit_code=0,
        output="test result: FAILED. 0 passed; 1 failed",
        finished_at="2026-08-08T04:05:46Z",
    )

    class FakeGitReader:
        def is_repo(self) -> bool:
            return True

        def get_git_info(self):
            return type("Info", (), {"latest_commit_date": "2026-08-08 04:00:00 +0000"})()

    service = TrajectoryService.__new__(TrajectoryService)
    service.root = tmp_path
    service.git_reader = FakeGitReader()
    flags = service._check_gsd_exec_evidence()
    assert len(flags) == 1
    assert flags[0].flag == "gsd_exec_false_success_evidence"
    assert flags[0].severity == "high"
    assert "new-false-green" in flags[0].evidence

def test_timeouts_since_reports_timeout_even_when_exit_code_is_zero(tmp_path: Path) -> None:
    # The 2026-09-20..27 review found captures that reported exit_code 0 while
    # timed_out was true; an exit-code-only reader calls those clean.
    _evidence(
        tmp_path,
        "capped-with-zero-exit",
        exit_code=0,
        output="partial output before the cap",
        finished_at="2026-08-08T04:05:46Z",
        timed_out=True,
        duration_ms=600026,
    )
    _evidence(
        tmp_path,
        "capped-with-null-exit",
        exit_code=None,  # type: ignore[arg-type]
        output="killed",
        finished_at="2026-08-08T04:05:47Z",
        timed_out=True,
        duration_ms=300005,
    )
    _evidence(
        tmp_path,
        "finished-in-time",
        exit_code=0,
        output="ok",
        finished_at="2026-08-08T04:05:48Z",
        timed_out=False,
    )
    findings = GsdExecReader(tmp_path).timeouts_since(None)
    assert sorted(finding.evidence_id for finding in findings) == [
        "capped-with-null-exit",
        "capped-with-zero-exit",
    ]
    zero_exit = next(f for f in findings if f.evidence_id == "capped-with-zero-exit")
    assert zero_exit.exit_code == 0
    assert zero_exit.duration_ms == 600026

def test_timeout_respects_commit_boundary(tmp_path: Path) -> None:
    _evidence(
        tmp_path,
        "reviewed-timeout",
        exit_code=0,
        output="x",
        finished_at="2026-08-08T03:00:00Z",
        timed_out=True,
    )
    assert GsdExecReader(tmp_path).timeouts_since(
        datetime(2026, 8, 8, 4, 0, tzinfo=timezone.utc)
    ) == []

def test_trajectory_service_flags_timeout_evidence(tmp_path: Path) -> None:
    _evidence(
        tmp_path,
        "capped-run",
        exit_code=0,
        output="no failure marker here",
        finished_at="2026-08-08T04:05:46Z",
        timed_out=True,
    )

    class FakeGitReader:
        def is_repo(self) -> bool:
            return True

        def get_git_info(self):
            return type("Info", (), {"latest_commit_date": "2026-08-08 04:00:00 +0000"})()

    service = TrajectoryService.__new__(TrajectoryService)
    service.root = tmp_path
    service.git_reader = FakeGitReader()
    flags = service._check_gsd_exec_evidence()
    # No failure marker, so only the timeout flag fires; a timeout is never clean.
    assert [flag.flag for flag in flags] == ["gsd_exec_timeout_evidence"]
    assert flags[0].severity == "high"
    assert "capped-run" in flags[0].evidence
