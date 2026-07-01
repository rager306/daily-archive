from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/run_m198_validation_package.py"
GATES = ROOT / "data/architecture-assessment/m198-gitnexus-impact-gates.json"

BOUNDARIES = {
    "graph_writes_allowed": False,
    "schema_migration_allowed": False,
    "import_eligible": False,
    "production_graph_import": False,
    "queue_dependency_semantic_change": False,
    "smoke_semantic_change": False,
    "rehearsal_semantic_change": False,
    "retired_graph_readiness_shim_restored": False,
}


def _write(path: Path, value: dict[str, Any]) -> Path:
    path.write_text(json.dumps(value), encoding="utf-8")
    return path


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _rehearsal(**overrides: Any) -> dict[str, Any]:
    data: dict[str, Any] = {
        "schema_version": "m198.readiness_rehearsal.v1",
        "verdict": "ready",
        "ready": True,
        "metadata_only": True,
        "payload_policy_confirmed": True,
        "boundary_confirmations": dict(BOUNDARIES),
        "blockers": [],
        "warnings": [],
    }
    data.update(overrides)
    return data


def _smoke(**overrides: Any) -> dict[str, Any]:
    data: dict[str, Any] = {
        "schema_version": "m198.smoke_parity_audit.v1",
        "status": "pass",
        "ready": True,
        "metadata_only": True,
        "payload_policy_confirmed": True,
        "blockers": [],
        "warnings": [],
    }
    data.update(overrides)
    return data


def _backend(**overrides: Any) -> dict[str, Any]:
    data: dict[str, Any] = {
        "schema_version": "m198.disabled_backend_safety.v1",
        "status": "pass",
        "ready": True,
        "metadata_only": True,
        "payload_policy_confirmed": True,
        "blockers": [],
        "warnings": [],
    }
    data.update(overrides)
    return data


def _run(tmp_path: Path, *, rehearsal: Path, smoke: Path, backend: Path, gates: Path = GATES) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--impact-gates",
            str(gates),
            "--rehearsal",
            str(rehearsal),
            "--smoke-parity",
            str(smoke),
            "--disabled-backend",
            str(backend),
            "--package",
            str(tmp_path / "package.json"),
            "--markdown",
            str(tmp_path / "package.md"),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def _valid_inputs(tmp_path: Path) -> tuple[Path, Path, Path]:
    return (
        _write(tmp_path / "rehearsal.json", _rehearsal()),
        _write(tmp_path / "smoke.json", _smoke()),
        _write(tmp_path / "backend.json", _backend()),
    )


def test_validation_package_passes_all_ready_inputs(tmp_path: Path) -> None:
    rehearsal, smoke, backend = _valid_inputs(tmp_path)

    completed = _run(tmp_path, rehearsal=rehearsal, smoke=smoke, backend=backend)

    assert completed.returncode == 0, completed.stderr
    assert "status=pass" in completed.stdout
    package = _load(tmp_path / "package.json")
    assert package["schema_version"] == "m198.validation_package.v1"
    assert package["status"] == "pass"
    assert package["ready"] is True
    assert package["metadata_only"] is True
    assert package["payload_policy_confirmed"] is True
    assert package["gitnexus_gate_summary"]["repo"] == "daily-archive"
    assert "S18 milestone closeout readiness" in package["downstream_handoff"]
    assert "Status: `pass`" in (tmp_path / "package.md").read_text(encoding="utf-8")


def test_validation_package_fails_missing_artifact(tmp_path: Path) -> None:
    rehearsal, smoke, _backend_path = _valid_inputs(tmp_path)
    missing_backend = tmp_path / "missing-backend.json"

    completed = _run(tmp_path, rehearsal=rehearsal, smoke=smoke, backend=missing_backend)

    assert completed.returncode == 2
    package = _load(tmp_path / "package.json")
    assert package["status"] == "fail"
    assert any("missing artifact" in blocker for blocker in package["blockers"])


def test_validation_package_fails_failed_smoke_parity(tmp_path: Path) -> None:
    rehearsal, _smoke_path, backend = _valid_inputs(tmp_path)
    smoke = _write(tmp_path / "smoke-fail.json", _smoke(status="fail", ready=False, blockers=["smoke_boundary missing"]))

    completed = _run(tmp_path, rehearsal=rehearsal, smoke=smoke, backend=backend)

    assert completed.returncode == 2
    package = _load(tmp_path / "package.json")
    assert "smoke_boundary missing" in package["blockers"]
    assert "smoke_parity status is fail" in package["blockers"]


def test_validation_package_fails_failed_disabled_backend_safety(tmp_path: Path) -> None:
    rehearsal, smoke, _backend_path = _valid_inputs(tmp_path)
    backend = _write(
        tmp_path / "backend-fail.json",
        _backend(status="fail", ready=False, blockers=["disabled_ladybug:import_eligible=True"]),
    )

    completed = _run(tmp_path, rehearsal=rehearsal, smoke=smoke, backend=backend)

    assert completed.returncode == 2
    package = _load(tmp_path / "package.json")
    assert "disabled_ladybug:import_eligible=True" in package["blockers"]
    assert "disabled_backend status is fail" in package["blockers"]


def test_validation_package_fails_unsupported_schema(tmp_path: Path) -> None:
    rehearsal = _write(tmp_path / "rehearsal.json", _rehearsal(schema_version="wrong"))
    smoke = _write(tmp_path / "smoke.json", _smoke())
    backend = _write(tmp_path / "backend.json", _backend())

    completed = _run(tmp_path, rehearsal=rehearsal, smoke=smoke, backend=backend)

    assert completed.returncode == 2
    package = _load(tmp_path / "package.json")
    assert any("expected m198.readiness_rehearsal.v1" in blocker for blocker in package["blockers"])


def test_validation_package_fails_boundary_leakage(tmp_path: Path) -> None:
    boundaries = dict(BOUNDARIES)
    boundaries["production_graph_import"] = True
    rehearsal = _write(tmp_path / "rehearsal.json", _rehearsal(boundary_confirmations=boundaries))
    smoke = _write(tmp_path / "smoke.json", _smoke())
    backend = _write(tmp_path / "backend.json", _backend())

    completed = _run(tmp_path, rehearsal=rehearsal, smoke=smoke, backend=backend)

    assert completed.returncode == 2
    package = _load(tmp_path / "package.json")
    assert "boundary production_graph_import is not false" in package["blockers"]
