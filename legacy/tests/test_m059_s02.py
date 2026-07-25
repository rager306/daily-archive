from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

# pyrefly: ignore [missing-import]
import m059_e2e_test as e2e  # noqa: E402  # ty:ignore[unresolved-import]
import m059_replay_ingest as replay  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]
import m059_validate_pdf_batch as validate_batch  # noqa: E402  # pyrefly: ignore [missing-import]  # ty:ignore[unresolved-import]

MANIFEST = ROOT / "artifacts/m054-pdf-acquisition/manifest.json"
DECISION = ROOT / "artifacts/m059-architecture/decision.md"
SAFETY_DEFAULTS = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}


def test_s02_files_exist() -> None:
    for path in [
        ROOT / "scripts/m059_validate_pdf_batch.py",
        ROOT / "scripts/m059_replay_ingest.py",
        ROOT / "scripts/m059_e2e_test.py",
        DECISION,
    ]:
        assert path.exists(), path


def test_safety_defaults_and_loopback_are_explicit() -> None:
    assert validate_batch.SAFETY_DEFAULTS == SAFETY_DEFAULTS
    assert replay.SAFETY_DEFAULTS == SAFETY_DEFAULTS
    assert validate_batch.DEFAULT_LOOPBACK_BASE_URL == "http://127.0.0.1:8070"
    assert replay.DEFAULT_LOOPBACK_BASE_URL == "http://127.0.0.1:8070"
    for path in [
        ROOT / "scripts/m059_validate_pdf_batch.py",
        ROOT / "scripts/m059_replay_ingest.py",
    ]:
        source = path.read_text(encoding="utf-8")
        assert "localhost" not in source.lower()


def test_validate_pdf_batch_grobid_m054() -> None:
    report = validate_batch.validate_batch(MANIFEST, "grobid")
    assert report.total == 5
    assert report.passed == 5
    assert report.failed == 0
    assert report.success_rate == 1.0
    assert report.missing_outputs == 0
    assert report.missing_fields == {}


def test_validate_pdf_batch_opendataloader_m054() -> None:
    report = validate_batch.validate_batch(MANIFEST, "opendataloader")
    assert report.total == 5
    assert report.passed == 5
    assert report.failed == 0
    assert report.success_rate == 1.0
    assert report.missing_outputs == 0
    assert report.missing_fields == {}


def test_replay_ingest_grobid_one_pdf_is_byte_identical_and_idempotent(tmp_path: Path) -> None:
    first = replay.replay_batch(
        MANIFEST,
        "grobid",
        output_suffix="pytest-replay",
        output_dir=tmp_path,
        arxiv_ids={"1804.02767"},
    )
    assert first.total == 1
    assert first.replayed == 1
    assert first.failed == 0
    assert first.byte_identical == 1
    assert first.results[0].source_sha256 == first.results[0].replay_sha256

    second = replay.replay_batch(
        MANIFEST,
        "grobid",
        output_suffix="pytest-replay",
        output_dir=tmp_path,
        arxiv_ids={"1804.02767"},
    )
    assert second.total == 1
    assert second.skipped == 1
    assert second.failed == 0
    assert second.byte_identical == 1
    assert second.results[0].source_sha256 == second.results[0].replay_sha256


def test_validate_pdf_batch_cli_outputs_aggregate() -> None:
    completed = subprocess.run(
        [
            sys.executable,
            "scripts/m059_validate_pdf_batch.py",
            "--manifest=artifacts/m054-pdf-acquisition/manifest.json",
            "--parser=grobid",
        ],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    assert (
        "aggregate batch=m054-pdf-acquisition parser=grobid total=5 passed=5 failed=0"
        in completed.stdout
    )


def test_e2e_runner_writes_reports_and_decision_exists(tmp_path: Path) -> None:
    summary = e2e.run_e2e(MANIFEST, tmp_path)
    assert summary["passed"] is True
    assert summary["validation_passed"] is True
    assert summary["replay_passed"] is True
    assert (ROOT / summary["validation_report"]).exists()
    assert (ROOT / summary["replay_report"]).exists()
    assert (ROOT / summary["e2e_report"]).exists()
    decision_text = DECISION.read_text(encoding="utf-8")
    assert "M061" in decision_text
    assert "manifest-gated" in decision_text
