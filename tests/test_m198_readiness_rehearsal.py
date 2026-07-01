from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REHEARSAL = ROOT / "scripts/run_m198_readiness_rehearsal.py"


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _run(tmp_path: Path, mode: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(REHEARSAL),
            "--workdir",
            str(tmp_path / "work"),
            "--summary",
            str(tmp_path / "summary.json"),
            "--markdown",
            str(tmp_path / "summary.md"),
            "--mode",
            mode,
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def test_readiness_rehearsal_ready(tmp_path: Path) -> None:
    completed = _run(tmp_path, "ready")

    assert completed.returncode == 0, completed.stderr
    assert "verdict=ready" in completed.stdout
    summary = _load(tmp_path / "summary.json")
    assert summary["schema_version"] == "m198.readiness_rehearsal.v1"
    assert summary["verdict"] == "ready"
    assert summary["ready"] is True
    assert summary["metadata_only"] is True
    assert summary["payload_policy_confirmed"] is True
    assert [command["exit_code"] for command in summary["command_log"]] == [0, 0, 0]
    for path in summary["artifacts"].values():
        if isinstance(path, list):
            assert all(Path(item).exists() for item in path)
        else:
            assert Path(path).exists()
    markdown = (tmp_path / "summary.md").read_text(encoding="utf-8")
    assert "# M198 Readiness Rehearsal" in markdown
    assert "S14 smoke parity audit" in markdown


def test_readiness_rehearsal_propagates_blocker(tmp_path: Path) -> None:
    completed = _run(tmp_path, "blocker")

    assert completed.returncode == 2
    summary = _load(tmp_path / "summary.json")
    assert summary["verdict"] == "blocked"
    assert summary["ready"] is False
    assert summary["command_log"][0]["exit_code"] == 2
    assert summary["command_log"][-1]["exit_code"] == 2
    assert any("graph_writes_allowed=True" in blocker for blocker in summary["blockers"])
    assert summary["boundary_confirmations"]["graph_writes_allowed"] is False


def test_readiness_rehearsal_missing_source_fails(tmp_path: Path) -> None:
    completed = _run(tmp_path, "missing_source")

    assert completed.returncode == 2
    summary = _load(tmp_path / "summary.json")
    assert summary["verdict"] == "blocked"
    assert any("missing required source kind: smoke_boundary" in blocker for blocker in summary["blockers"])
    assert len(summary["artifacts"]["evidence_files"]) == 4


def test_readiness_rehearsal_payload_leak_fails(tmp_path: Path) -> None:
    completed = _run(tmp_path, "payload_leak")

    assert completed.returncode == 2
    summary = _load(tmp_path / "summary.json")
    assert summary["verdict"] == "blocked"
    assert any("forbidden payload term: vector_payload" in blocker for blocker in summary["blockers"])
    assert summary["payload_policy_confirmed"] is True
    assert summary["metadata_only"] is True


def test_readiness_rehearsal_keeps_no_write_boundaries(tmp_path: Path) -> None:
    completed = _run(tmp_path, "ready")

    assert completed.returncode == 0
    summary = _load(tmp_path / "summary.json")
    assert summary["boundary_confirmations"] == {
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "production_graph_import": False,
        "queue_dependency_semantic_change": False,
        "smoke_semantic_change": False,
        "rehearsal_semantic_change": False,
        "retired_graph_readiness_shim_restored": False,
    }
