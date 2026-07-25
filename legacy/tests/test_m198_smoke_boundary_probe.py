from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / "scripts/run_m198_smoke_boundary_probe.py"
CONTRACT = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _run_probe(
    artifact_dir: Path,
    evidence_path: Path,
    *,
    article_json: Path | None = None,
    smoke_result: Path | None = None,
    skip_run: bool = False,
) -> subprocess.CompletedProcess[str]:
    command = [
        sys.executable,
        str(PROBE),
        "--artifact-dir",
        str(artifact_dir),
        "--evidence",
        str(evidence_path),
        "--correlation-id",
        "corr-smoke",
    ]
    if article_json is not None:
        command.extend(["--article-json", str(article_json)])
    if smoke_result is not None:
        command.extend(["--smoke-result", str(smoke_result)])
    if skip_run:
        command.append("--skip-run")
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True)


def _default_article() -> dict:
    return {
        "candidate_id": "m198-smoke-fixture",
        "article_key": "m198-smoke-fixture",
        "title": "M198 Smoke Fixture",
        "abstract": "Metadata-only readiness smoke fixture.",
        "safety_flags": {
            "graph_write_allowed": False,
            "schema_migration_allowed": False,
            "import_eligible": False,
            "production_import_attempted": False,
            "promotion_allowed": False,
        },
    }


def _write_article(path: Path, article: dict) -> Path:
    path.write_text(json.dumps(article), encoding="utf-8")
    return path


def test_smoke_boundary_probe_writes_contract_shaped_evidence(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "smoke"
    evidence_path = tmp_path / "evidence.json"

    completed = _run_probe(artifact_dir, evidence_path)

    assert completed.returncode == 0, completed.stderr
    assert "queue_status=ready" in completed.stdout
    evidence = _load(evidence_path)
    contract = _load(CONTRACT)
    for field in contract["required_fields"]:
        assert field in evidence
    assert evidence["schema_version"] == contract["schema_version"]
    assert evidence["source_kind"] == "smoke_boundary"
    assert evidence["correlation_id"] == "corr-smoke"
    assert evidence["status"] == "pass"
    assert evidence["drift_class"] == "not_applicable"
    assert evidence["graph_writes_allowed"] is False
    assert evidence["schema_migration_allowed"] is False
    assert evidence["import_eligible"] is False
    assert evidence["queue_status"] == "ready"
    assert evidence["diagnostics"]["candidate_id"] == "m198-smoke-fixture"
    assert evidence["diagnostics"]["metadata_only"] is True
    assert evidence["diagnostics"]["promotion_allowed"] is False
    assert any(ref.endswith("continuity.json") for ref in evidence["evidence_refs"])
    assert any(ref.endswith("readiness_handoff.json") for ref in evidence["evidence_refs"])
    assert any(ref.endswith("queue_inspect.json") for ref in evidence["evidence_refs"])


def test_smoke_boundary_probe_rejects_missing_continuity(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "smoke"
    first_evidence = tmp_path / "first.json"
    completed = _run_probe(artifact_dir, first_evidence)
    assert completed.returncode == 0, completed.stderr
    smoke_result = artifact_dir / "smoke_result.json"
    article_dir = Path(_load(smoke_result)["artifact_dir"])
    (article_dir / "continuity.json").unlink()

    second_evidence = tmp_path / "second.json"
    completed = _run_probe(artifact_dir, second_evidence, smoke_result=smoke_result, skip_run=True)

    assert completed.returncode != 0
    assert "required smoke artifact missing: continuity.json" in completed.stderr
    assert not second_evidence.exists()


def test_smoke_boundary_probe_rejects_bad_import_flag(tmp_path: Path) -> None:
    article = _default_article()
    article["safety_flags"]["import_eligible"] = True
    article_path = _write_article(tmp_path / "article.json", article)

    completed = _run_probe(tmp_path / "smoke", tmp_path / "evidence.json", article_json=article_path)

    assert completed.returncode != 0
    assert "article safety flag import_eligible must be false" in completed.stderr


def test_smoke_boundary_probe_rejects_missing_candidate_id(tmp_path: Path) -> None:
    article = _default_article()
    article.pop("candidate_id")
    article_path = _write_article(tmp_path / "article.json", article)

    completed = _run_probe(tmp_path / "smoke", tmp_path / "evidence.json", article_json=article_path)

    assert completed.returncode != 0
    assert "candidate_id is required" in completed.stderr


def test_smoke_boundary_probe_rejects_forbidden_payload_terms(tmp_path: Path) -> None:
    artifact_dir = tmp_path / "smoke"
    first_evidence = tmp_path / "first.json"
    completed = _run_probe(artifact_dir, first_evidence)
    assert completed.returncode == 0, completed.stderr
    smoke_result = artifact_dir / "smoke_result.json"
    article_dir = Path(_load(smoke_result)["artifact_dir"])
    continuity_path = article_dir / "continuity.json"
    continuity = _load(continuity_path)
    continuity["diagnostics"] = ["api_key"]
    continuity_path.write_text(json.dumps(continuity), encoding="utf-8")

    second_evidence = tmp_path / "second.json"
    completed = _run_probe(artifact_dir, second_evidence, smoke_result=smoke_result, skip_run=True)

    assert completed.returncode != 0
    assert "forbidden payload term found: api_key" in completed.stderr
    assert not second_evidence.exists()
