from datetime import datetime, timezone
import json
from pathlib import Path

from governor.adapters.gsd_exec_reader import GsdExecReader
from governor.application.trajectory_service import TrajectoryService


def _evidence(root: Path, evidence_id: str, *, exit_code: int, output: str, finished_at: str) -> None:
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
