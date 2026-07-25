"""Tests for M055 parser benchmark S02 GROBID-only baseline."""

from __future__ import annotations

import json
import sys
import urllib.error
from pathlib import Path
from unittest import mock

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

# pyrefly: ignore [missing-import]
import benchmark_m055_grobid_only as grobid_only  # noqa: E402  # ty:ignore[unresolved-import]

SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


class FakeResponse:
    def __init__(self, body: bytes = b"", status: int = 200) -> None:
        self._body = body
        self.status = status

    def __enter__(self) -> FakeResponse:
        return self

    def __exit__(self, *args: object) -> None:
        return None

    def read(self) -> bytes:
        return self._body


def tei_payload(*, refs: int = 1, bibls: int = 1, bodies: int = 1, padding: int = 1200) -> str:
    ref_xml = "".join("<ref>r</ref>" for _ in range(refs))
    bibl_xml = "".join(
        "<biblStruct><analytic><title>T</title></analytic></biblStruct>" for _ in range(bibls)
    )
    body_xml = "".join(f"<p>body {index}</p>" for index in range(bodies))
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<TEI xmlns="http://www.tei-c.org/ns/1.0">
  <teiHeader>
    <fileDesc>
      <titleStmt><title>Header Title</title><author>A One</author><author>A Two</author></titleStmt>
      <sourceDesc><listBibl>{bibl_xml}</listBibl></sourceDesc>
    </fileDesc>
    <profileDesc><abstract>Abstract text</abstract></profileDesc>
  </teiHeader>
  <text><body>{body_xml}{ref_xml}<p>{"x" * padding}</p></body></text>
</TEI>
"""


def make_manifest(tmp_path: Path, entries: int = 1) -> Path:
    pdfs = []
    for index in range(entries):
        arxiv_id = f"2401.0000{index}"
        pdf_path = tmp_path / f"{arxiv_id}.pdf"
        pdf_path.write_bytes(b"%PDF-1.4\nfixture\n")
        pdfs.append(
            {
                "article_key": arxiv_id,
                "arxiv_id": arxiv_id,
                "category": "cs-cl",
                "path": str(pdf_path),
                "sha256": grobid_only._sha256(pdf_path),
                "size_bytes": pdf_path.stat().st_size,
                "target_index": index,
            }
        )
    manifest_path = tmp_path / "corpus-manifest.json"
    manifest_path.write_text(json.dumps({"pdfs": pdfs}), encoding="utf-8")
    return manifest_path


def test_probe_grobid_pdf_success(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    pdf_path = tmp_path / "paper.pdf"
    pdf_path.write_bytes(b"%PDF-1.4\n")
    tei = tei_payload().encode("utf-8")

    def fake_urlopen(request: object, timeout: int) -> FakeResponse:
        assert timeout == 60
        assert request is not None
        return FakeResponse(tei, status=200)

    monkeypatch.setattr(grobid_only.urllib.request, "urlopen", fake_urlopen)

    result = grobid_only._probe_grobid_pdf(
        pdf_path, "http://grobid.test/api/processHeaderDocument", 60
    )

    assert result["tei_text"].startswith('<?xml version="1.0"')
    assert result["http_status"] == 200
    assert result["bytes"] == len(tei)
    assert result["duration_ms"] >= 0
    assert result["error"] is None


def test_probe_grobid_pdf_fail_closed(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    pdf_path = tmp_path / "paper.pdf"
    pdf_path.write_bytes(b"%PDF-1.4\n")

    def fake_urlopen(request: object, timeout: int) -> FakeResponse:
        raise urllib.error.URLError("connection refused")

    monkeypatch.setattr(grobid_only.urllib.request, "urlopen", fake_urlopen)

    result = grobid_only._probe_grobid_pdf(
        pdf_path, "http://grobid.test/api/processHeaderDocument", 60
    )

    assert result["tei_text"] == ""
    assert result["http_status"] is None
    assert result["bytes"] == 0
    assert result["duration_ms"] >= 0
    assert "URLError" in result["error"]


def test_extract_tei_metrics_small_tei() -> None:
    metrics = grobid_only._extract_tei_metrics(tei_payload(padding=10))

    assert metrics["tei_size_bytes"] < grobid_only.LOW_QUALITY_MIN_TEI_BYTES
    assert metrics["header_title_present"] is True
    assert metrics["header_author_count"] == 2
    assert metrics["abstract_present"] is True
    assert grobid_only._low_quality_source_criteria(metrics) is True


def test_extract_tei_metrics_zero_refs() -> None:
    metrics = grobid_only._extract_tei_metrics(tei_payload(refs=0, padding=1600))

    assert metrics["ref_count"] == 0
    assert metrics["body_element_count"] > 0
    assert grobid_only._low_quality_source_criteria(metrics) is True


def test_low_quality_source_criteria_combinations() -> None:
    good = {"tei_size_bytes": 2048, "ref_count": 1, "body_element_count": 1}
    small = {"tei_size_bytes": 1023, "ref_count": 1, "body_element_count": 1}
    zero_refs = {"tei_size_bytes": 2048, "ref_count": 0, "body_element_count": 1}
    zero_body = {"tei_size_bytes": 2048, "ref_count": 1, "body_element_count": 0}

    assert grobid_only._low_quality_source_criteria(good) is False
    assert grobid_only._low_quality_source_criteria(small) is True
    assert grobid_only._low_quality_source_criteria(zero_refs) is True
    assert grobid_only._low_quality_source_criteria(zero_body) is True


def test_probe_grobid_only_aggregate_counts(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    manifest = make_manifest(tmp_path, entries=2)
    payloads = [tei_payload(padding=1400), tei_payload(refs=0, padding=1400)]

    def fake_probe(pdf_path: Path, endpoint: str, timeout: int) -> dict[str, object]:
        tei = payloads.pop(0)
        return {
            "tei_text": tei,
            "http_status": 200,
            "bytes": len(tei),
            "duration_ms": 7,
            "error": None,
        }

    monkeypatch.setattr(grobid_only, "_probe_grobid_pdf", fake_probe)

    summary = grobid_only.probe_grobid_only(
        manifest, tmp_path / "out", grobid_url="http://grobid.test", max_retries=3, timeout=60
    )

    assert summary["aggregate_counts"]["success"] == 1
    assert summary["aggregate_counts"]["low_quality_source"] == 1
    assert summary["total_pdfs"] == 2
    assert len(list((tmp_path / "out" / "per-pdf").glob("*.json"))) == 2


def test_atomic_tei_write_pattern(tmp_path: Path) -> None:
    target = tmp_path / "out" / "tei" / "paper.tei.xml"
    grobid_only._atomic_write_bytes(target, b"old")
    grobid_only._atomic_write_bytes(target, b"new")

    assert target.read_bytes() == b"new"
    assert not list(target.parent.glob("*.tmp"))


def test_5_safety_defaults_all_false(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    manifest = make_manifest(tmp_path)
    monkeypatch.setattr(
        grobid_only,
        "_probe_grobid_pdf",
        lambda *args, **kwargs: {
            "tei_text": tei_payload(padding=1400),
            "http_status": 200,
            "bytes": 1400,
            "duration_ms": 1,
            "error": None,
        },
    )

    summary = grobid_only.probe_grobid_only(
        manifest, tmp_path / "out", grobid_url="http://grobid.test"
    )
    packet = json.loads(
        next((tmp_path / "out" / "per-pdf").glob("*.json")).read_text(encoding="utf-8")
    )

    assert set(summary["safety_defaults"]) == SAFETY_KEYS
    assert set(packet["safety_defaults"]) == SAFETY_KEYS
    assert all(value is False for value in summary["safety_defaults"].values())
    assert all(value is False for value in packet["safety_defaults"].values())


def test_m022_repair_candidate_flag(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    manifest = make_manifest(tmp_path)
    monkeypatch.setattr(
        grobid_only,
        "_probe_grobid_pdf",
        lambda *args, **kwargs: {
            "tei_text": tei_payload(refs=0, padding=1400),
            "http_status": 200,
            "bytes": 1400,
            "duration_ms": 1,
            "error": None,
        },
    )

    grobid_only.probe_grobid_only(manifest, tmp_path / "out", grobid_url="http://grobid.test")
    packet = json.loads(
        next((tmp_path / "out" / "per-pdf").glob("*.json")).read_text(encoding="utf-8")
    )

    assert packet["status"] == "low_quality_source"
    assert packet["low_quality_source"] is True
    assert packet["m022_repair_candidate"] is True


def test_cli_dry_run_no_urllib_call(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    manifest = make_manifest(tmp_path)
    urlopen = mock.Mock(side_effect=AssertionError("urlopen should not be called"))
    monkeypatch.setattr(grobid_only.urllib.request, "urlopen", urlopen)

    exit_code = grobid_only.main(
        [
            "--corpus-manifest",
            str(manifest),
            "--output-dir",
            str(tmp_path / "out"),
            "--grobid-url",
            "http://grobid.test",
            "--dry-run",
        ]
    )

    assert exit_code == 0
    assert urlopen.call_count == 0
    captured = capsys.readouterr()
    assert "dry_run_skipped_grobid_call" not in captured.out
    packet = json.loads(
        next((tmp_path / "out" / "per-pdf").glob("*.json")).read_text(encoding="utf-8")
    )
    assert packet["status"] == "grobid_unavailable"


def test_idempotent_summary(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    manifest = make_manifest(tmp_path)

    monkeypatch.setattr(
        grobid_only,
        "_probe_grobid_pdf",
        lambda *args, **kwargs: {
            "tei_text": tei_payload(padding=1400),
            "http_status": 200,
            "bytes": 1400,
            "duration_ms": 1,
            "error": None,
        },
    )

    first = grobid_only.probe_grobid_only(
        manifest, tmp_path / "out", grobid_url="http://grobid.test"
    )
    second = grobid_only.probe_grobid_only(
        manifest, tmp_path / "out", grobid_url="http://grobid.test"
    )

    assert first["aggregate_counts"] == second["aggregate_counts"]
    assert first["per_pdf_statuses"] == second["per_pdf_statuses"]
    assert len(list((tmp_path / "out" / "per-pdf").glob("*.json"))) == 1
    assert (tmp_path / "out" / "summary.json").exists()
