from __future__ import annotations

import json
import subprocess
import sys
from copy import deepcopy
from pathlib import Path
from typing import Any

import scripts.run_m198_disabled_backend_safety as safety

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/run_m198_disabled_backend_safety.py"


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _run(tmp_path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--audit",
            str(tmp_path / "disabled-backend-safety.json"),
            "--markdown",
            str(tmp_path / "disabled-backend-safety.md"),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def test_disabled_backend_safety_cli_passes(tmp_path: Path) -> None:
    completed = _run(tmp_path)

    assert completed.returncode == 0, completed.stderr
    assert "status=pass" in completed.stdout
    audit = _load(tmp_path / "disabled-backend-safety.json")
    assert audit["schema_version"] == "m198.disabled_backend_safety.v1"
    assert audit["status"] == "pass"
    assert audit["metadata_only"] is True
    assert audit["payload_policy_confirmed"] is True
    assert all(check["passed"] for check in audit["checks"])
    assert "S16 end-to-end validation package" in audit["downstream_handoff"]
    markdown = (tmp_path / "disabled-backend-safety.md").read_text(encoding="utf-8")
    assert "# M198 Disabled Backend Safety" in markdown
    assert "Status: `pass`" in markdown


def test_disabled_backend_safety_dry_run_is_metadata_only() -> None:
    summaries = safety.adapter_summaries()
    dry_run = next(item for item in summaries if item["name"] == "disabled_ladybug_dry_run")

    assert dry_run["node_ref_count"] == 1
    assert dry_run["edge_ref_count"] == 1
    assert dry_run["evidence_ref_count"] == 1
    assert "backend_projection_dry_run" in dry_run["diagnostic_codes"]
    assert "raw_text" not in json.dumps(dry_run)
    assert "embedding_payload" not in json.dumps(dry_run)


def test_disabled_backend_safety_unsafe_backend_name_fails_closed() -> None:
    summaries = safety.adapter_summaries()
    unsafe = next(item for item in summaries if item["name"] == "unsafe_backend_name")

    assert unsafe["backend"] == "disabled_backend"
    assert "backend_projection_configuration_invalid" in unsafe["diagnostic_codes"]
    assert all(value is False for value in unsafe["safety_flags"].values())


def test_disabled_backend_safety_blocks_import_eligibility_leakage() -> None:
    summaries = deepcopy(safety.adapter_summaries())
    summaries[0]["safety_flags"]["import_eligible"] = True

    audit = safety.build_audit(summaries)

    assert audit["status"] == "fail"
    assert audit["ready"] is False
    assert any("import_eligible=True" in blocker for blocker in audit["blockers"])
    assert any(check["name"] == "safety_flags_false" and not check["passed"] for check in audit["checks"])


def test_disabled_backend_safety_blocks_graph_write_leakage() -> None:
    summaries = deepcopy(safety.adapter_summaries())
    summaries[1]["safety_flags"]["graphdb_written"] = True

    audit = safety.build_audit(summaries)

    assert audit["status"] == "fail"
    assert any("graphdb_written=True" in blocker for blocker in audit["blockers"])


def test_disabled_backend_safety_blocks_payload_terms() -> None:
    summaries = deepcopy(safety.adapter_summaries())
    summaries[0]["diagnostic_codes"].append("vector_payload")

    audit = safety.build_audit(summaries)

    assert audit["status"] == "fail"
    assert audit["metadata_only"] is False
    assert any("vector_payload" in blocker for blocker in audit["blockers"])
