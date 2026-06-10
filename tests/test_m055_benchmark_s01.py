"""Tests for M055 parser benchmark S01 deliverables."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import urllib.error
from pathlib import Path
from unittest import mock

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import benchmark_m055_availability_probe as availability  # noqa: E402
import benchmark_m055_corpus_manifest as corpus  # noqa: E402
import benchmark_m055_vendor_check as vendor_check  # noqa: E402

TARGET_SUBSET = ROOT / "artifacts" / "m054-pdf-acquisition" / "target-subset.json"
ACQUISITION_LOG = ROOT / "artifacts" / "m054-pdf-acquisition" / "acquisition-log.json"
SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


class FakeResponse:
    def __init__(self, body: bytes = b"true", status: int = 200) -> None:
        self._body = body
        self.status = status

    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *args: object) -> None:
        return None

    def read(self) -> bytes:
        return self._body

    def getcode(self) -> int:
        return self.status


def _without_generated_at(payload: dict[str, object]) -> dict[str, object]:
    clone = copy.deepcopy(payload)
    clone.pop("generated_at", None)
    return clone


def _assert_safety_defaults(payload: dict[str, object]) -> None:
    safety = payload["safety"]
    assert set(safety) == SAFETY_KEYS
    assert all(value is False for value in safety.values())


def test_vendor_check_grobid_present() -> None:
    result = vendor_check.check_grobid_vendor("/root/vendor-source/grobid")

    assert result["present"] is True
    assert result["has_changelog"] is True
    assert result["has_readme"] is True
    assert result["has_license"] is True
    assert result["has_dockerfile"] is True
    assert result["indexed"] is True


def test_vendor_check_opendataloader_present() -> None:
    result = vendor_check.check_opendataloader_vendor("/root/vendor-source/opendataloader-pdf")

    assert result["present"] is True
    assert result["has_changelog"] is True
    assert result["has_readme"] is True
    assert result["has_license"] is True
    assert result["indexed"] is True


def test_vendor_check_fail_closed_missing_path(tmp_path: Path) -> None:
    missing = tmp_path / "missing-vendor"

    result = vendor_check._check_vendor_source("missing", str(missing))

    assert result["present"] is False
    assert result["indexed"] is False
    assert result["status"] == "missing"
    assert result["has_changelog"] is False
    assert result["has_readme"] is False
    assert result["has_license"] is False


def test_vendor_check_safety_defaults() -> None:
    payload = vendor_check.run_vendor_check("/root/vendor-source")

    assert payload["schema_version"] == vendor_check.SCHEMA_VERSION
    _assert_safety_defaults(payload)


def test_availability_grobid_live() -> None:
    result = availability._probe_grobid("http://localhost:8070", timeout=5)
    if not result["available"]:
        pytest.xfail(f"live GROBID is unavailable: {result}")

    assert result["available"] is True
    assert result["http_status"] == 200
    assert isinstance(result["latency_ms"], int)


def test_availability_grobid_offline() -> None:
    with mock.patch(
        "benchmark_m055_availability_probe.urllib.request.urlopen",
        side_effect=urllib.error.URLError("offline"),
    ):
        result = availability._probe_grobid("http://127.0.0.1:1", timeout=1)

    assert result["available"] is False
    assert result["http_status"] is None
    assert result["error"] is not None


def test_availability_opendataloader_installed() -> None:
    result = availability._probe_opendataloader()
    if not result["installed"]:
        pytest.xfail(f"OpenDataLoader is not installed: {result['import_error']}")

    assert result["installed"] is True
    assert result["import_error"] is None


def test_corpus_manifest_5_pdfs(tmp_path: Path) -> None:
    output = tmp_path / "corpus-manifest.json"

    payload = corpus.build_corpus_manifest(TARGET_SUBSET, output)

    assert payload["schema_version"] == corpus.SCHEMA_VERSION
    assert payload["total_count"] == 5
    assert len(payload["pdfs"]) == 5
    assert output.is_file()
    for entry in payload["pdfs"]:
        assert len(entry["sha256"]) == 64
        assert entry["size_bytes"] > 0
        assert entry["category"] in {"cs-cv", "cs-cl", "cs-lg"}


def test_corpus_manifest_idempotent(tmp_path: Path) -> None:
    first_output = tmp_path / "first.json"
    second_output = tmp_path / "second.json"

    first = corpus.build_corpus_manifest(TARGET_SUBSET, first_output)
    second = corpus.build_corpus_manifest(TARGET_SUBSET, second_output)

    first_stable = json.dumps(_without_generated_at(first), sort_keys=True)
    second_stable = json.dumps(_without_generated_at(second), sort_keys=True)
    assert first_stable == second_stable


def test_corpus_manifest_safety_defaults(tmp_path: Path) -> None:
    output = tmp_path / "corpus-manifest.json"

    payload = corpus.build_corpus_manifest(TARGET_SUBSET, output)

    _assert_safety_defaults(payload)


def test_m050_m051_m052_m053_regression() -> None:
    acquisition = json.loads(ACQUISITION_LOG.read_text(encoding="utf-8"))
    assert acquisition["counts"]["acquired"] == 5
    assert len(acquisition["entries"]) == 5
    assert all(entry["status"] == "acquired" for entry in acquisition["entries"])

    regression_files = [
        "tests/test_m050_article_artifact_reducer.py",
        "tests/test_m050_article_artifact_worker.py",
        "tests/test_m050_e2e_pipeline.py",
        "tests/test_m052_rlm_workflow.py",
        "tests/test_m053_grobid_pilot.py",
    ]
    completed = subprocess.run(
        [sys.executable, "-m", "pytest", *regression_files, "-q"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )

    assert completed.returncode == 0, completed.stdout + completed.stderr
