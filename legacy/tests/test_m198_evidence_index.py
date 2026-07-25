from __future__ import annotations

import json
import subprocess
import sys
from hashlib import sha256
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
INDEXER = ROOT / "scripts/run_m198_evidence_index.py"
CONTRACT = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _sha(path: Path) -> str:
    return sha256(path.read_bytes()).hexdigest()


def _base(kind: str) -> dict[str, Any]:
    return {
        "schema_version": "m198.readiness_evidence.v1",
        "evidence_id": f"evidence-{kind}",
        "source_kind": kind,
        "correlation_id": "corr-source",
        "status": "pass",
        "drift_class": "expected" if kind == "governance_ratchet" else "not_applicable",
        "timestamp": "2026-06-30T00:00:00+00:00",
        "graph_writes_allowed": False,
        "schema_migration_allowed": False,
        "import_eligible": False,
        "evidence_refs": [f"artifact://{kind}.json"],
        "diagnostics": {},
        "non_goals": _load(CONTRACT)["blocked_transitions"],
        "source_checksums": {f"artifact://{kind}.json": "abc123"},
    }


def _fixtures() -> dict[str, dict[str, Any]]:
    return {
        "reactive_dry_run": _base("reactive_dry_run"),
        "sync_no_write_rehearsal": _base("sync_no_write_rehearsal"),
        "smoke_boundary": _base("smoke_boundary"),
        "graph_readiness_validate_only": _base("graph_readiness_validate_only"),
        "governance_ratchet": _base("governance_ratchet"),
    }


def _write_evidence(tmp_path: Path, fixtures: dict[str, dict[str, Any]]) -> list[Path]:
    paths: list[Path] = []
    for index, (kind, evidence) in enumerate(fixtures.items()):
        path = tmp_path / f"{index:02d}-{kind}.json"
        path.write_text(json.dumps(evidence, sort_keys=True), encoding="utf-8")
        paths.append(path)
    return paths


def _run_index(paths: list[Path], index_path: Path, *, expected_checksums: Path | None = None) -> subprocess.CompletedProcess[str]:
    command = [sys.executable, str(INDEXER), "--index", str(index_path)]
    for path in paths:
        command.extend(["--evidence", str(path)])
    if expected_checksums is not None:
        command.extend(["--expected-checksums", str(expected_checksums)])
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True)


def test_evidence_index_writes_metadata_only_full_index(tmp_path: Path) -> None:
    paths = _write_evidence(tmp_path, _fixtures())
    index_path = tmp_path / "index.json"

    completed = _run_index(paths, index_path)

    assert completed.returncode == 0, completed.stderr
    assert "status=pass" in completed.stdout
    index = _load(index_path)
    assert index["schema_version"] == "m198.readiness_evidence_index.v1"
    assert index["status"] == "pass"
    assert index["metadata_only"] is True
    assert index["entry_count"] == 5
    assert index["missing_source_kinds"] == []
    assert index["blockers"] == []
    assert index["payload_policy"]["stores_payload_text"] is False
    assert index["payload_policy"]["stores_embeddings"] is False
    assert sorted(index["observed_source_kinds"]) == sorted(_fixtures())
    for entry in index["entries"]:
        assert "file_checksum" in entry
        assert "evidence_ref_count" in entry
        assert "source_checksum_count" in entry
        assert "diagnostics" not in entry


def test_evidence_index_blocks_missing_required_source(tmp_path: Path) -> None:
    fixtures = _fixtures()
    fixtures.pop("smoke_boundary")
    paths = _write_evidence(tmp_path, fixtures)
    index_path = tmp_path / "index.json"

    completed = _run_index(paths, index_path)

    assert completed.returncode == 2
    index = _load(index_path)
    assert index["status"] == "fail"
    assert "smoke_boundary" in index["missing_source_kinds"]
    assert "missing required source kind: smoke_boundary" in index["blockers"]


def test_evidence_index_blocks_duplicate_source_kind(tmp_path: Path) -> None:
    fixtures = _fixtures()
    paths = _write_evidence(tmp_path, fixtures)
    duplicate = tmp_path / "duplicate.json"
    duplicate.write_text(json.dumps(fixtures["smoke_boundary"], sort_keys=True), encoding="utf-8")
    index_path = tmp_path / "index.json"

    completed = _run_index([*paths, duplicate], index_path)

    assert completed.returncode == 2
    index = _load(index_path)
    assert "duplicate source kind: smoke_boundary" in index["blockers"]


def test_evidence_index_blocks_checksum_mismatch(tmp_path: Path) -> None:
    paths = _write_evidence(tmp_path, _fixtures())
    checksum_map = {str(path): _sha(path) for path in paths}
    checksum_path = tmp_path / "checksums.json"
    checksum_path.write_text(json.dumps(checksum_map), encoding="utf-8")
    changed = _load(paths[0])
    changed["diagnostics"] = {"changed": True}
    paths[0].write_text(json.dumps(changed, sort_keys=True), encoding="utf-8")
    index_path = tmp_path / "index.json"

    completed = _run_index(paths, index_path, expected_checksums=checksum_path)

    assert completed.returncode == 2
    index = _load(index_path)
    assert f"checksum mismatch for {paths[0]}" in index["blockers"]


def test_evidence_index_blocks_forbidden_payload_terms(tmp_path: Path) -> None:
    fixtures = _fixtures()
    fixtures["governance_ratchet"]["diagnostics"] = {"leak": "api_key"}
    paths = _write_evidence(tmp_path, fixtures)
    index_path = tmp_path / "index.json"

    completed = _run_index(paths, index_path)

    assert completed.returncode == 2
    index = _load(index_path)
    assert f"{paths[-1]} contains forbidden payload term: api_key" in index["blockers"]


def test_evidence_index_blocks_enabled_import_flag(tmp_path: Path) -> None:
    fixtures = _fixtures()
    fixtures["graph_readiness_validate_only"]["import_eligible"] = True
    paths = _write_evidence(tmp_path, fixtures)
    index_path = tmp_path / "index.json"

    completed = _run_index(paths, index_path)

    assert completed.returncode == 2
    index = _load(index_path)
    assert "graph_readiness_validate_only has import_eligible=True" in index["blockers"]
