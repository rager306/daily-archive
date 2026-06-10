"""Tests for M055deep S03 corpus expansion to 20 PDFs."""

from __future__ import annotations

import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import build_m055deep_corpus_manifest_20 as corpus20  # noqa: E402

ACQUISITION_LOG = ROOT / "artifacts" / "m055deep-parser-benchmark" / "acquisition-log.json"
CORPUS_MANIFEST = ROOT / "artifacts" / "m055deep-parser-benchmark" / "corpus-manifest-20.json"
SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


def _read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _manifest() -> dict[str, object]:
    assert CORPUS_MANIFEST.exists()
    return _read_json(CORPUS_MANIFEST)


def test_acquisition_min_4_pdfs() -> None:
    log = _read_json(ACQUISITION_LOG)
    entries = log["entries"]
    acquired = [entry for entry in entries if entry["status"] == "acquired"]

    assert len(entries) == 6
    assert log["counts"]["acquired"] >= 4
    assert len(acquired) >= 4
    for entry in acquired:
        local_path = ROOT / entry["local_path"]
        assert local_path.exists()
        assert local_path.stat().st_size == entry["bytes"]


def test_corpus_manifest_20_pdfs() -> None:
    manifest = _manifest()
    pdfs = manifest["pdfs"]

    assert manifest["actual_total"] == len(pdfs)
    assert manifest["actual_total"] == 20
    assert manifest["actual_total"] >= manifest["minimum_total"]
    assert manifest["actual_new_acquisitions"] == 6
    assert manifest["actual_new_acquisitions"] >= manifest["minimum_new_acquisitions"]
    assert manifest["source_milestone_counts"] == {"M027/M041": 9, "M051": 5, "M055deep": 6}


def test_corpus_manifest_idempotent(tmp_path: Path) -> None:
    output = tmp_path / "corpus-manifest-20.json"

    first_payload = corpus20.build_manifest()
    corpus20.write_manifest(first_payload, output)
    first_text = output.read_text(encoding="utf-8")

    second_payload = corpus20.build_manifest()
    corpus20.write_manifest(second_payload, output)
    second_text = output.read_text(encoding="utf-8")

    assert second_text == first_text


def test_corpus_manifest_safety_defaults() -> None:
    manifest = _manifest()

    assert set(manifest["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in manifest["safety_defaults"].values())


def test_corpus_manifest_per_pdf_source_milestone() -> None:
    manifest = _manifest()
    pdfs = manifest["pdfs"]
    counts = Counter(entry["source_milestone"] for entry in pdfs)

    assert counts == {"M051": 5, "M027/M041": 9, "M055deep": 6}
    for entry in pdfs:
        assert entry["source_milestone"] in {"M051", "M027/M041", "M055deep"}
        assert entry["arxiv_id"]
        assert (ROOT / entry["path"]).exists()
        assert isinstance(entry["pages_estimate"], int)
        assert entry["pages_estimate"] >= 1


def test_corpus_manifest_per_category_counts() -> None:
    manifest = _manifest()
    pdfs = manifest["pdfs"]
    counts = Counter(entry["category"] for entry in pdfs)

    assert dict(sorted(counts.items())) == manifest["category_counts"]
    assert manifest["category_counts"] == {
        "cs-ai": 2,
        "cs-cl": 4,
        "cs-cv": 3,
        "cs-lg": 4,
        "mixed-source": 7,
    }
