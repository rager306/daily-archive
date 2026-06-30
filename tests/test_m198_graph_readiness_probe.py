from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / "scripts/run_m198_graph_readiness_probe.py"
CONTRACT = ROOT / "data/architecture-assessment/m198-readiness-evidence-contract.json"


def _load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _run_probe(
    review_dir: Path,
    events_path: Path,
    evidence_path: Path,
    *,
    skip_fixture: bool = False,
) -> subprocess.CompletedProcess[str]:
    command = [
        sys.executable,
        str(PROBE),
        "--review-dir",
        str(review_dir),
        "--events",
        str(events_path),
        "--evidence",
        str(evidence_path),
        "--correlation-id",
        "corr-graph",
    ]
    if skip_fixture:
        command.append("--skip-fixture")
    return subprocess.run(command, cwd=ROOT, text=True, capture_output=True)


def test_graph_readiness_probe_writes_contract_shaped_evidence(tmp_path: Path) -> None:
    review_dir = tmp_path / "review"
    events_path = tmp_path / "events.jsonl"
    evidence_path = tmp_path / "evidence.json"

    completed = _run_probe(review_dir, events_path, evidence_path)

    assert completed.returncode == 0, completed.stderr
    assert "retired_alias_absent=True" in completed.stdout
    evidence = _load(evidence_path)
    contract = _load(CONTRACT)
    for field in contract["required_fields"]:
        assert field in evidence
    assert evidence["schema_version"] == contract["schema_version"]
    assert evidence["source_kind"] == "graph_readiness_validate_only"
    assert evidence["correlation_id"] == "corr-graph"
    assert evidence["status"] == "pass"
    assert evidence["graph_writes_allowed"] is False
    assert evidence["schema_migration_allowed"] is False
    assert evidence["import_eligible"] is False
    assert evidence["diagnostics"]["validator_ok"] is True
    assert evidence["diagnostics"]["validate_only"] is True
    assert evidence["diagnostics"]["require_completed_review"] is True
    assert evidence["diagnostics"]["retired_alias_absent"] is True
    assert evidence["diagnostics"]["review_bundle_count"] == 1
    assert any(ref.endswith("independent-review-summary.md") for ref in evidence["evidence_refs"])
    assert any(ref.endswith("events.jsonl") for ref in evidence["evidence_refs"])


def test_graph_readiness_probe_rejects_missing_summary(tmp_path: Path) -> None:
    review_dir = tmp_path / "review"
    events_path = tmp_path / "events.jsonl"
    first_evidence = tmp_path / "first.json"
    completed = _run_probe(review_dir, events_path, first_evidence)
    assert completed.returncode == 0, completed.stderr
    (review_dir / "independent-review-summary.md").unlink()

    second_evidence = tmp_path / "second.json"
    completed = _run_probe(review_dir, events_path, second_evidence, skip_fixture=True)

    assert completed.returncode != 0
    assert "Missing review summary" in completed.stderr or "Missing review summary" in completed.stdout
    assert not second_evidence.exists()


def test_graph_readiness_probe_rejects_missing_completed_verdict(tmp_path: Path) -> None:
    review_dir = tmp_path / "review"
    events_path = tmp_path / "events.jsonl"
    first_evidence = tmp_path / "first.json"
    completed = _run_probe(review_dir, events_path, first_evidence)
    assert completed.returncode == 0, completed.stderr
    events_path.write_text("", encoding="utf-8")

    second_evidence = tmp_path / "second.json"
    completed = _run_probe(review_dir, events_path, second_evidence, skip_fixture=True)

    assert completed.returncode != 0
    assert "No independent_review.verdict event" in completed.stderr or "No independent_review.verdict event" in completed.stdout
    assert not second_evidence.exists()


def test_graph_readiness_probe_documents_retired_alias_absence(tmp_path: Path) -> None:
    retired_alias = ".".join(("arxiv_archive", "graph_readiness_review"))
    completed = subprocess.run(
        [sys.executable, "-m", retired_alias, "--help"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )

    assert completed.returncode != 0
    assert "No module named" in completed.stderr


def test_graph_readiness_probe_rejects_bad_import_flag(tmp_path: Path) -> None:
    review_dir = tmp_path / "review"
    events_path = tmp_path / "events.jsonl"
    first_evidence = tmp_path / "first.json"
    completed = _run_probe(review_dir, events_path, first_evidence)
    assert completed.returncode == 0, completed.stderr
    manifest_path = review_dir / "fixture-manifest.json"
    manifest = _load(manifest_path)
    manifest["import_eligible"] = True
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    second_evidence = tmp_path / "second.json"
    completed = _run_probe(review_dir, events_path, second_evidence, skip_fixture=True)

    assert completed.returncode != 0
    assert "import_eligible must be false" in completed.stderr
    assert not second_evidence.exists()


def test_graph_readiness_probe_rejects_forbidden_payload_terms(tmp_path: Path) -> None:
    review_dir = tmp_path / "review"
    events_path = tmp_path / "events.jsonl"
    first_evidence = tmp_path / "first.json"
    completed = _run_probe(review_dir, events_path, first_evidence)
    assert completed.returncode == 0, completed.stderr
    summary_path = review_dir / "independent-review-summary.md"
    summary_path.write_text(summary_path.read_text(encoding="utf-8") + "\napi_key\n", encoding="utf-8")

    second_evidence = tmp_path / "second.json"
    completed = _run_probe(review_dir, events_path, second_evidence, skip_fixture=True)

    assert completed.returncode != 0
    assert "forbidden payload term found: api_key" in completed.stderr
    assert not second_evidence.exists()
