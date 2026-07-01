from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "scripts/run_m198_readiness_report.py"


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
        "status": "pass",
        "required_source_kinds": required,
        "observed_source_kinds": required,
        "missing_source_kinds": [],
        "entry_count": len(required),
        "entries": [
            {"source_kind": "reactive_dry_run", "status": "pass", "drift_class": "expected"},
            {"source_kind": "sync_no_write_rehearsal", "status": "pass", "drift_class": "expected"},
            {"source_kind": "smoke_boundary", "status": "pass", "drift_class": "warning"},
            {"source_kind": "graph_readiness_validate_only", "status": "pass", "drift_class": "expected"},
            {"source_kind": "governance_ratchet", "status": "pass", "drift_class": "expected"},
        ],
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


def _diagnostics() -> dict[str, Any]:
    required = _index()["required_source_kinds"]
    return {
        "schema_version": "m198.operator_diagnostics.v1",
        "verdict": "ready",
        "ready": True,
        "index_status": "pass",
        "source_coverage": {
            "required_count": len(required),
            "observed_count": len(required),
            "missing_count": 0,
            "required_source_kinds": required,
            "observed_source_kinds": required,
            "missing_source_kinds": [],
        },
        "entry_count": len(required),
        "warnings": [],
        "blockers": [],
        "blocked_transitions": ["production_graph_import", "schema_migration"],
        "payload_policy_confirmed": True,
        "metadata_only": True,
        "next_actions": ["Proceed to S10 readiness report synthesis using the metadata-only diagnostics and index."],
    }


def _write(path: Path, value: dict[str, Any]) -> Path:
    path.write_text(json.dumps(value), encoding="utf-8")
    return path


def _run(index_path: Path, diagnostics_path: Path, report_path: Path, markdown_path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(REPORT),
            "--index",
            str(index_path),
            "--diagnostics",
            str(diagnostics_path),
            "--report",
            str(report_path),
            "--markdown",
            str(markdown_path),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )


def test_readiness_report_ready(tmp_path: Path) -> None:
    index_path = _write(tmp_path / "index.json", _index())
    diagnostics_path = _write(tmp_path / "diagnostics.json", _diagnostics())
    report_path = tmp_path / "report.json"
    markdown_path = tmp_path / "report.md"

    completed = _run(index_path, diagnostics_path, report_path, markdown_path)

    assert completed.returncode == 0, completed.stderr
    assert "verdict=ready" in completed.stdout
    report = _load(report_path)
    assert report["schema_version"] == "m198.readiness_report.v1"
    assert report["verdict"] == "ready"
    assert report["ready"] is True
    assert report["drift_summary"] == {"expected": 4, "warning": 1}
    assert report["payload_policy_confirmed"] is True
    assert "S11 no-write governance ratchets" in report["downstream_handoff"]
    markdown = markdown_path.read_text(encoding="utf-8")
    assert "# M198 Readiness Report" in markdown
    assert "Verdict: `ready`" in markdown
    assert "S13 realistic readiness rehearsal" in markdown


def test_readiness_report_needs_attention(tmp_path: Path) -> None:
    index = _index()
    index["warnings"] = ["extra source kinds indexed: disabled_backend"]
    diagnostics = _diagnostics()
    diagnostics["verdict"] = "needs_attention"
    diagnostics["ready"] = False
    diagnostics["warnings"] = ["extra source kinds indexed: disabled_backend"]
    index_path = _write(tmp_path / "index.json", index)
    diagnostics_path = _write(tmp_path / "diagnostics.json", diagnostics)
    report_path = tmp_path / "report.json"
    markdown_path = tmp_path / "report.md"

    completed = _run(index_path, diagnostics_path, report_path, markdown_path)

    assert completed.returncode == 0
    report = _load(report_path)
    assert report["verdict"] == "needs_attention"
    assert report["ready"] is False
    assert report["warnings"] == ["extra source kinds indexed: disabled_backend"]
    assert "Review warnings before adding S11 governance ratchets." in report["next_actions"]


def test_readiness_report_blocked(tmp_path: Path) -> None:
    index = _index()
    index["status"] = "fail"
    index["missing_source_kinds"] = ["smoke_boundary"]
    index["blockers"] = ["missing required source kind: smoke_boundary"]
    diagnostics = _diagnostics()
    diagnostics["verdict"] = "blocked"
    diagnostics["ready"] = False
    diagnostics["index_status"] = "fail"
    diagnostics["source_coverage"]["missing_source_kinds"] = ["smoke_boundary"]
    diagnostics["blockers"] = ["missing required source kind: smoke_boundary"]
    index_path = _write(tmp_path / "index.json", index)
    diagnostics_path = _write(tmp_path / "diagnostics.json", diagnostics)
    report_path = tmp_path / "report.json"
    markdown_path = tmp_path / "report.md"

    completed = _run(index_path, diagnostics_path, report_path, markdown_path)

    assert completed.returncode == 2
    report = _load(report_path)
    assert report["verdict"] == "blocked"
    assert report["ready"] is False
    assert "missing required source kind: smoke_boundary" in report["blockers"]
    assert any("Regenerate missing S03-S07" in item for item in report["next_actions"])
    assert "Verdict: `blocked`" in markdown_path.read_text(encoding="utf-8")


def test_readiness_report_rejects_schema_mismatch(tmp_path: Path) -> None:
    index = _index()
    index["schema_version"] = "wrong"
    index_path = _write(tmp_path / "index.json", index)
    diagnostics_path = _write(tmp_path / "diagnostics.json", _diagnostics())
    report_path = tmp_path / "report.json"
    markdown_path = tmp_path / "report.md"

    completed = _run(index_path, diagnostics_path, report_path, markdown_path)

    assert completed.returncode != 0
    assert "expected m198.readiness_evidence_index.v1" in completed.stderr
    assert not report_path.exists()
    assert not markdown_path.exists()


def test_readiness_report_blocks_diagnostics_index_disagreement(tmp_path: Path) -> None:
    diagnostics = _diagnostics()
    diagnostics["index_status"] = "fail"
    index_path = _write(tmp_path / "index.json", _index())
    diagnostics_path = _write(tmp_path / "diagnostics.json", diagnostics)
    report_path = tmp_path / "report.json"
    markdown_path = tmp_path / "report.md"

    completed = _run(index_path, diagnostics_path, report_path, markdown_path)

    assert completed.returncode == 2
    report = _load(report_path)
    assert report["verdict"] == "blocked"
    assert "diagnostics index_status 'fail' disagrees with index status 'pass'" in report["disagreements"]
    assert "diagnostics index_status 'fail' disagrees with index status 'pass'" in report["blockers"]


def test_readiness_report_blocks_payload_policy_failure(tmp_path: Path) -> None:
    index = _index()
    index["payload_policy"]["stores_vectors"] = True
    diagnostics = _diagnostics()
    diagnostics["payload_policy_confirmed"] = False
    index_path = _write(tmp_path / "index.json", index)
    diagnostics_path = _write(tmp_path / "diagnostics.json", diagnostics)
    report_path = tmp_path / "report.json"
    markdown_path = tmp_path / "report.md"

    completed = _run(index_path, diagnostics_path, report_path, markdown_path)

    assert completed.returncode == 2
    report = _load(report_path)
    assert report["verdict"] == "blocked"
    assert report["payload_policy_confirmed"] is False
    assert "metadata-only payload policy is not confirmed" in report["blockers"]
    assert any("Fix metadata-only payload policy" in item for item in report["next_actions"])
