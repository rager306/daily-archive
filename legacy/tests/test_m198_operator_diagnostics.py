from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DIAGNOSTICS = ROOT / "scripts/run_m198_operator_diagnostics.py"


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _index() -> dict[str, Any]:
    required = [
        "reactive_dry_run",
        "sync_no_write_rehearsal",
        "smoke_boundary",
        "graph_readiness_validate_only",
        "governance_ratchet",
    ]
    return {
        "schema_version": "m198.readiness_evidence_index.v1",
        "generated_at": "2026-06-30T00:00:00+00:00",
        "status": "pass",
        "required_source_kinds": required,
        "observed_source_kinds": required,
        "missing_source_kinds": [],
        "entry_count": len(required),
        "entries": [],
        "non_goal_coverage": [
            "production_graph_import",
            "schema_migration",
            "queue_dependency_semantic_change",
            "smoke_semantic_change",
            "rehearsal_semantic_change",
            "retired_graph_readiness_shim",
            "import_eligible_true",
        ],
        "warnings": [],
        "blockers": [],
        "metadata_only": True,
        "payload_policy": {
            "stores_paths": True,
            "stores_checksums": True,
            "stores_payload_text": False,
            "stores_embeddings": False,
            "stores_vectors": False,
            "stores_credentials": False,
            "stores_queue_database_bytes": False,
        },
    }


def _write_index(path: Path, index: dict[str, Any]) -> Path:
    path.write_text(json.dumps(index), encoding="utf-8")
    return path


def _run(index_path: Path, diagnostics_path: Path, markdown_path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(DIAGNOSTICS),
            "--index",
            str(index_path),
            "--diagnostics",
            str(diagnostics_path),
            "--markdown",
            str(markdown_path),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def test_operator_diagnostics_ready_index(tmp_path: Path) -> None:
    index_path = _write_index(tmp_path / "index.json", _index())
    diagnostics_path = tmp_path / "diagnostics.json"
    markdown_path = tmp_path / "diagnostics.md"

    completed = _run(index_path, diagnostics_path, markdown_path)

    assert completed.returncode == 0, completed.stderr
    assert "verdict=ready" in completed.stdout
    diagnostics = _load(diagnostics_path)
    assert diagnostics["schema_version"] == "m198.operator_diagnostics.v1"
    assert diagnostics["verdict"] == "ready"
    assert diagnostics["ready"] is True
    assert diagnostics["payload_policy_confirmed"] is True
    assert diagnostics["blockers"] == []
    assert diagnostics["warnings"] == []
    markdown = markdown_path.read_text(encoding="utf-8")
    assert "# M198 Operator Diagnostics" in markdown
    assert "Verdict: `ready`" in markdown
    assert "Proceed to S10 readiness report synthesis" in markdown


def test_operator_diagnostics_blocker_index(tmp_path: Path) -> None:
    index = _index()
    index["status"] = "fail"
    index["blockers"] = ["missing required source kind: smoke_boundary"]
    index["missing_source_kinds"] = ["smoke_boundary"]
    index_path = _write_index(tmp_path / "index.json", index)
    diagnostics_path = tmp_path / "diagnostics.json"
    markdown_path = tmp_path / "diagnostics.md"

    completed = _run(index_path, diagnostics_path, markdown_path)

    assert completed.returncode == 2
    diagnostics = _load(diagnostics_path)
    assert diagnostics["verdict"] == "blocked"
    assert diagnostics["ready"] is False
    assert "missing required source kind: smoke_boundary" in diagnostics["blockers"]
    assert any("Regenerate missing producer evidence" in item for item in diagnostics["next_actions"])
    assert "missing required source kind: smoke_boundary" in markdown_path.read_text(encoding="utf-8")


def test_operator_diagnostics_warning_index(tmp_path: Path) -> None:
    index = _index()
    index["warnings"] = ["extra source kinds indexed: disabled_backend"]
    index_path = _write_index(tmp_path / "index.json", index)
    diagnostics_path = tmp_path / "diagnostics.json"
    markdown_path = tmp_path / "diagnostics.md"

    completed = _run(index_path, diagnostics_path, markdown_path)

    assert completed.returncode == 0
    diagnostics = _load(diagnostics_path)
    assert diagnostics["verdict"] == "needs_attention"
    assert diagnostics["ready"] is False
    assert diagnostics["warnings"] == ["extra source kinds indexed: disabled_backend"]
    assert "Review indexed warnings" in "\n".join(diagnostics["next_actions"])


def test_operator_diagnostics_rejects_invalid_schema(tmp_path: Path) -> None:
    index = _index()
    index["schema_version"] = "wrong"
    index_path = _write_index(tmp_path / "index.json", index)
    diagnostics_path = tmp_path / "diagnostics.json"
    markdown_path = tmp_path / "diagnostics.md"

    completed = _run(index_path, diagnostics_path, markdown_path)

    assert completed.returncode != 0
    assert "expected m198.readiness_evidence_index.v1" in completed.stderr
    assert not diagnostics_path.exists()
    assert not markdown_path.exists()


def test_operator_diagnostics_blocks_payload_policy_violation(tmp_path: Path) -> None:
    index = _index()
    index["payload_policy"]["stores_payload_text"] = True
    index_path = _write_index(tmp_path / "index.json", index)
    diagnostics_path = tmp_path / "diagnostics.json"
    markdown_path = tmp_path / "diagnostics.md"

    completed = _run(index_path, diagnostics_path, markdown_path)

    assert completed.returncode == 2
    diagnostics = _load(diagnostics_path)
    assert diagnostics["verdict"] == "blocked"
    assert diagnostics["payload_policy_confirmed"] is False
    assert "payload_policy.stores_payload_text must be false" in diagnostics["blockers"]
    assert any("Fix evidence index payload policy" in item for item in diagnostics["next_actions"])
