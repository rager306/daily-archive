from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REHEARSAL = ROOT / "scripts/run_m198_readiness_rehearsal.py"
AUDIT = ROOT / "scripts/run_m198_smoke_parity_audit.py"


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _run_rehearsal(tmp_path: Path, mode: str) -> Path:
    summary = tmp_path / f"{mode}-summary.json"
    completed = subprocess.run(
        [
            sys.executable,
            str(REHEARSAL),
            "--workdir",
            str(tmp_path / f"{mode}-work"),
            "--summary",
            str(summary),
            "--markdown",
            str(tmp_path / f"{mode}-summary.md"),
            "--mode",
            mode,
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    assert completed.returncode in (0, 2), completed.stderr
    assert summary.exists()
    return summary


def _run_audit(tmp_path: Path, summary: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(AUDIT),
            "--rehearsal",
            str(summary),
            "--audit",
            str(tmp_path / "smoke-audit.json"),
            "--markdown",
            str(tmp_path / "smoke-audit.md"),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def test_smoke_parity_audit_passes_ready_rehearsal(tmp_path: Path) -> None:
    summary = _run_rehearsal(tmp_path, "ready")

    completed = _run_audit(tmp_path, summary)

    assert completed.returncode == 0, completed.stderr
    audit = _load(tmp_path / "smoke-audit.json")
    assert audit["schema_version"] == "m198.smoke_parity_audit.v1"
    assert audit["status"] == "pass"
    assert audit["smoke_boundary_present"] is True
    assert audit["metadata_only"] is True
    assert all(check["passed"] for check in audit["checks"])
    assert "S16 end-to-end validation package" in audit["downstream_handoff"]


def test_smoke_parity_audit_fails_missing_smoke_boundary(tmp_path: Path) -> None:
    summary = _run_rehearsal(tmp_path, "missing_source")

    completed = _run_audit(tmp_path, summary)

    assert completed.returncode == 2
    audit = _load(tmp_path / "smoke-audit.json")
    assert audit["status"] == "fail"
    assert audit["smoke_boundary_present"] is False
    assert any(check["name"] == "smoke_boundary_source" and not check["passed"] for check in audit["checks"])
    assert any("smoke_boundary missing" in blocker for blocker in audit["blockers"])


def test_smoke_parity_audit_propagates_blocked_rehearsal(tmp_path: Path) -> None:
    summary = _run_rehearsal(tmp_path, "blocker")

    completed = _run_audit(tmp_path, summary)

    assert completed.returncode == 2
    audit = _load(tmp_path / "smoke-audit.json")
    assert audit["status"] == "fail"
    assert audit["readiness_verdict"] == "blocked"
    assert any("graph_writes_allowed=True" in blocker for blocker in audit["blockers"])
    assert audit["smoke_boundary_present"] is True


def test_smoke_parity_audit_fails_smoke_semantic_change_leakage(tmp_path: Path) -> None:
    summary = _run_rehearsal(tmp_path, "ready")
    rehearsal = _load(summary)
    rehearsal["boundary_confirmations"]["smoke_semantic_change"] = True
    summary.write_text(json.dumps(rehearsal), encoding="utf-8")

    completed = _run_audit(tmp_path, summary)

    assert completed.returncode == 2
    audit = _load(tmp_path / "smoke-audit.json")
    assert audit["status"] == "fail"
    assert any(check["name"] == "no_write_import_boundaries" and not check["passed"] for check in audit["checks"])
    assert any("smoke_semantic_change" in blocker for blocker in audit["blockers"])


def test_smoke_parity_audit_rejects_wrong_rehearsal_schema(tmp_path: Path) -> None:
    summary = tmp_path / "bad-summary.json"
    summary.write_text(json.dumps({"schema_version": "wrong"}), encoding="utf-8")

    completed = _run_audit(tmp_path, summary)

    assert completed.returncode != 0
    assert "expected m198.readiness_rehearsal.v1" in completed.stderr
    assert not (tmp_path / "smoke-audit.json").exists()
