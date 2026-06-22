"""Tests for scripts/acquire_linked_target_pdfs.py (M054 / M051-aaw9j7).

Per M054/M051 scope:
- retry policy: max retries with exponential backoff
- timeout behavior
- fail-closed on persistent errors
- network error categorization
- log schema: explicit status per record (acquired / blocked /
  low_quality_source / network_error / dry_run)
- bounded concurrency: max_workers=1 sequential, max_workers=2 parallel
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from unittest import mock

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquire_linked_target_pdfs  # noqa: E402

# Minimal target-subset fixture (same schema as artifacts/m054-pdf-acquisition/target-subset.json).
TARGET_SUBSET = {
    "schema_version": "m054-pdf-acquisition-target-subset.v1",
    "milestone": "M051-aaw9j7",
    "records": [
        {
            "index": 0,
            "article_key": "1804.02767",
            "category": "cs-cv",
            "evidence_refs": [],
            "expected_arxiv_url": "https://arxiv.org/pdf/1804.02767",
            "expected_local_pdf_path": "data/test-fixture/cs-cv/1804.02767/source/1804.02767.pdf",
            "local_pdf_present_at_baseline": False,
        },
        {
            "index": 1,
            "article_key": "2108.12409",
            "category": "cs-cl",
            "evidence_refs": [],
            "expected_arxiv_url": "https://arxiv.org/pdf/2108.12409",
            "expected_local_pdf_path": "data/test-fixture/cs-cl/2108.12409/source/2108.12409.pdf",
            "local_pdf_present_at_baseline": False,
        },
    ],
}


@pytest.fixture
def target_subset_path(tmp_path):
    path = tmp_path / "target-subset.json"
    path.write_text(json.dumps(TARGET_SUBSET), encoding="utf-8")
    return path


@pytest.fixture
def log_path(tmp_path):
    return tmp_path / "acquisition-log.json"


def test_classify_exception_network_error():
    import urllib.error

    exc = urllib.error.URLError("connection refused")
    assert acquire_linked_target_pdfs._classify_exception(exc) == "network_error"


def test_classify_exception_http_4xx_is_blocked():
    import urllib.error

    exc = urllib.error.HTTPError("url", 404, "Not Found", {}, None)
    assert acquire_linked_target_pdfs._classify_exception(exc) == "blocked"


def test_classify_exception_http_5xx_is_network_error():
    import urllib.error

    exc = urllib.error.HTTPError("url", 503, "Service Unavailable", {}, None)
    assert acquire_linked_target_pdfs._classify_exception(exc) == "network_error"


def test_classify_exception_timeout_is_network_error():
    exc = TimeoutError("read timeout")
    assert acquire_linked_target_pdfs._classify_exception(exc) == "network_error"


def test_download_with_retry_succeeds_on_first_attempt(tmp_path):
    body = b"%PDF-1.4\nfake pdf content for testing"
    dest = tmp_path / "test.pdf"

    fake_response = mock.MagicMock()
    fake_response.__enter__.return_value.read.return_value = body
    fake_response.__enter__.return_value.status = 200
    fake_response.__enter__.return_value.headers = {"Content-Type": "application/pdf"}

    with mock.patch("urllib.request.urlopen", return_value=fake_response):
        result = acquire_linked_target_pdfs._download_with_retry("https://arxiv.org/pdf/test", dest)

    assert result["status"] == "acquired"
    assert result["http_status"] == 200
    assert result["bytes"] == len(body)
    assert result["sha256"]
    assert dest.exists()
    assert dest.read_bytes() == body


def test_download_with_retry_fails_closed_after_max_retries(tmp_path):
    """After max_retries exhausted, status is explicit (network_error/blocked)."""
    import urllib.error

    dest = tmp_path / "test.pdf"

    with mock.patch(
        "urllib.request.urlopen",
        side_effect=urllib.error.URLError("connection refused"),
    ):
        result = acquire_linked_target_pdfs._download_with_retry(
            "https://arxiv.org/pdf/test", dest, max_retries=2, timeout=1, backoff_base=1.0
        )

    assert result["status"] in {"network_error", "blocked"}
    assert len(result["attempts"]) == 2
    assert dest.exists() is False  # No file written on failure


def test_download_with_retry_persists_atomically(tmp_path):
    """On success, file is written atomically (no .tmp file lingering)."""
    body = b"%PDF-1.4\natomic write test"
    dest = tmp_path / "test.pdf"

    fake_response = mock.MagicMock()
    fake_response.__enter__.return_value.read.return_value = body
    fake_response.__enter__.return_value.status = 200
    fake_response.__enter__.return_value.headers = {}

    with mock.patch("urllib.request.urlopen", return_value=fake_response):
        acquire_linked_target_pdfs._download_with_retry("https://arxiv.org/pdf/test", dest)

    assert dest.exists()
    # No lingering .tmp files.
    assert list(tmp_path.glob("*.pdf.tmp")) == []


def test_process_record_skips_already_present_local_pdf(tmp_path):
    body = b"%PDF-1.4\nalready present"
    record = {
        "article_key": "1804.02767",
        "category": "cs-cv",
        "expected_arxiv_url": "https://arxiv.org/pdf/1804.02767",
        "expected_local_pdf_path": str(tmp_path / "1804.02767.pdf"),
    }
    (tmp_path / "1804.02767.pdf").write_bytes(body)

    log_entry = acquire_linked_target_pdfs._process_record(
        record, storage_root=tmp_path, max_retries=3, timeout=10, dry_run=False
    )
    assert log_entry["status"] == "acquired"
    assert log_entry["note"] == "local_pdf_already_present"
    assert log_entry["bytes"] == len(body)


def test_process_record_records_network_error_on_persistent_failure(tmp_path):
    import urllib.error

    record = {
        "article_key": "1804.02767",
        "category": "cs-cv",
        "expected_arxiv_url": "https://arxiv.org/pdf/1804.02767",
        "expected_local_pdf_path": str(tmp_path / "1804.02767.pdf"),
    }
    with mock.patch("urllib.request.urlopen", side_effect=urllib.error.URLError("refused")):
        log_entry = acquire_linked_target_pdfs._process_record(
            record, storage_root=tmp_path, max_retries=1, timeout=1, dry_run=False
        )
    assert log_entry["status"] == "network_error"
    assert log_entry["article_key"] == "1804.02767"
    assert "last_exception" in log_entry


def test_acquire_all_dry_run_writes_log_without_downloading(target_subset_path, log_path):
    """Dry run: no downloads, all records have status=dry_run."""
    log = acquire_linked_target_pdfs.acquire_all(
        target_subset_path,
        log_path=log_path,
        max_retries=1,
        timeout=5,
        max_workers=1,
        dry_run=True,
    )
    assert log["dry_run"] is True
    assert all(entry["status"] == "dry_run" for entry in log["entries"])
    assert log_path.exists()
    # Counts include 2 dry_run entries.
    assert log["counts"]["dry_run"] == 2


def test_acquire_all_serial_mode_processes_all_records(target_subset_path, log_path):
    """max_workers=1 → sequential processing; all records get log entries."""
    log = acquire_linked_target_pdfs.acquire_all(
        target_subset_path,
        log_path=log_path,
        max_retries=1,
        timeout=1,
        max_workers=1,
        dry_run=True,
    )
    assert len(log["entries"]) == 2
    assert log["max_workers"] == 1


def test_acquire_all_parallel_mode_processes_all_records(target_subset_path, log_path):
    """max_workers=2 → bounded ThreadPoolExecutor; all records get log entries."""
    log = acquire_linked_target_pdfs.acquire_all(
        target_subset_path,
        log_path=log_path,
        max_retries=1,
        timeout=1,
        max_workers=2,
        dry_run=True,
    )
    assert len(log["entries"]) == 2
    assert log["max_workers"] == 2


def test_acquire_all_log_schema_includes_all_required_fields(target_subset_path, log_path):
    log = acquire_linked_target_pdfs.acquire_all(
        target_subset_path,
        log_path=log_path,
        max_retries=1,
        timeout=1,
        max_workers=1,
        dry_run=True,
    )
    assert log["schema_version"] == "m054-pdf-acquisition-log.v1"
    assert "started_at" in log
    assert "completed_at" in log
    assert "counts" in log
    assert "entries" in log
    # Per-entry fields.
    for entry in log["entries"]:
        assert "article_key" in entry
        assert "url" in entry
        assert "status" in entry
        assert "started_at" in entry


def test_acquire_all_raises_on_missing_target_subset(tmp_path):
    nonexistent = tmp_path / "nonexistent.json"
    log_path = tmp_path / "log.json"
    with pytest.raises(FileNotFoundError):
        acquire_linked_target_pdfs.acquire_all(
            nonexistent, log_path=log_path, max_retries=1, timeout=1, max_workers=1, dry_run=True
        )


def test_real_target_subset_dry_run_completes(tmp_path):
    """Integration: dry run on the actual artifacts/m054-pdf-acquisition/target-subset.json."""
    actual = Path("artifacts/m054-pdf-acquisition/target-subset.json")
    if not actual.exists():
        pytest.skip("target-subset.json not present in this environment")
    log_path = tmp_path / "log.json"
    log = acquire_linked_target_pdfs.acquire_all(
        actual,
        log_path=log_path,
        max_retries=1,
        timeout=5,
        max_workers=1,
        dry_run=True,
    )
    # 5 records per M054 spec.
    assert len(log["entries"]) == 5
    assert all(e["status"] == "dry_run" for e in log["entries"])
