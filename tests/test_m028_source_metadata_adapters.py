"""Contract tests for the M028 source metadata adapter script."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
from types import ModuleType

import pytest

SCRIPT_PATH = Path(__file__).parents[1] / "scripts" / "build_m028_source_metadata_adapters.py"


def _load_script() -> ModuleType:
    spec = importlib.util.spec_from_file_location("build_m028_source_metadata_adapters", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8")


def _sha256(path: Path) -> str:
    import hashlib

    return hashlib.sha256(path.read_bytes()).hexdigest()


def _selection() -> dict[str, object]:
    return {
        "refs": [
            {
                "ref_id": "R01",
                "url": "https://arxiv.org/pdf/2605.20897.pdf",
                "canonical_url": "https://arxiv.org/abs/2605.20897",
                "source_kind": "arxiv_pdf_url",
                "normalized_identity": "arxiv:2605.20897",
                "arxiv_id": "2605.20897",
                "arxiv_unversioned_id": "2605.20897",
            },
            {
                "ref_id": "R02",
                "url": "https://arxiv.org/abs/2605.20897",
                "canonical_url": "https://arxiv.org/abs/2605.20897",
                "source_kind": "arxiv_abs_url",
                "normalized_identity": "arxiv:2605.20897",
                "arxiv_id": "2605.20897",
                "arxiv_unversioned_id": "2605.20897",
            },
            {
                "ref_id": "R03",
                "url": "https://developer.nvidia.com/blog/example/?linkId=123",
                "canonical_url": "https://developer.nvidia.com/blog/example/",
                "source_kind": "company_blog_url",
                "normalized_identity": "company_blog:nvidia:example",
            },
        ]
    }


def _artifact_files(root: Path) -> dict[str, Path]:
    sources = root / "sources"
    sources.mkdir()
    pdf = sources / "R01.pdf"
    pdf.write_bytes(b"%PDF-1.4\nfixture bytes are local only\n")
    arxiv_html = sources / "R02.html"
    arxiv_html.write_text(
        """
        <html><head>
          <title>Fallback title</title>
          <meta name="citation_title" content="Metadata Adapter Paper">
          <meta name="citation_author" content="Ada Lovelace">
          <meta name="citation_author" content="Grace Hopper">
          <meta name="citation_date" content="2026-05-20">
          <meta name="citation_arxiv_id" content="2605.20897">
          <meta name="citation_pdf_url" content="https://arxiv.org/pdf/2605.20897.pdf">
        </head><body>body must not be serialized</body></html>
        """,
        encoding="utf-8",
    )
    blog_html = sources / "R03.html"
    blog_html.write_text(
        """
        <html><head>
          <meta property="og:title" content="NVIDIA Blog Metadata">
          <meta name="author" content="NVIDIA">
          <meta property="article:published_time" content="2026-05-21T00:00:00Z">
        </head><body>blog body must not be serialized</body></html>
        """,
        encoding="utf-8",
    )
    return {"R01": pdf, "R02": arxiv_html, "R03": blog_html}


def _acquisition_rows(paths: dict[str, Path], root: Path) -> list[dict[str, object]]:
    rows = []
    kinds = {"R01": "arxiv_pdf_url", "R02": "arxiv_abs_url", "R03": "company_blog_url"}
    identities = {"R01": "arxiv:2605.20897", "R02": "arxiv:2605.20897", "R03": "company_blog:nvidia:example"}
    urls = {
        "R01": "https://arxiv.org/pdf/2605.20897.pdf",
        "R02": "https://arxiv.org/abs/2605.20897",
        "R03": "https://developer.nvidia.com/blog/example/?linkId=123",
    }
    canonicals = {
        "R01": "https://arxiv.org/abs/2605.20897",
        "R02": "https://arxiv.org/abs/2605.20897",
        "R03": "https://developer.nvidia.com/blog/example/",
    }
    for ref_id, artifact_path in paths.items():
        rows.append(
            {
                "ref_id": ref_id,
                "url": urls[ref_id],
                "canonical_url": canonicals[ref_id],
                "source_kind": kinds[ref_id],
                "normalized_identity": identities[ref_id],
                "artifact_path": str(artifact_path.relative_to(root)),
                "content_type": "application/pdf" if artifact_path.suffix == ".pdf" else "text/html; charset=utf-8",
                "byte_count": artifact_path.stat().st_size,
                "sha256": _sha256(artifact_path),
                "http_status": 200,
                "status": "captured",
                "terminal": True,
                "failure_code": None,
                "graph_write_attempted": False,
                "kg_readiness_claimed": False,
                "production_persistence_attempted": False,
            }
        )
    return rows


def test_build_outputs_preserve_refs_identities_and_metadata_only_payloads(tmp_path: Path) -> None:
    script = _load_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    out_dir = tmp_path / "out"
    paths = _artifact_files(tmp_path)
    _write_json(selection_path, _selection())
    _write_jsonl(acquisition_path, _acquisition_rows(paths, tmp_path))

    events, summary = script.build_metadata_outputs(selection_path, acquisition_path, out_dir, repo_root=tmp_path)

    assert summary["url_ref_count"] == 3
    assert summary["normalized_identity_count"] == 2
    assert summary["duplicate_identity_group_count"] == 1
    assert summary["source_kind_counts"] == {"arxiv_abs_url": 1, "arxiv_pdf_url": 1, "company_blog_url": 1}
    assert all(flag is False for flag in summary["safety_flags"].values())
    assert [event["ref_id"] for event in events] == ["R01", "R02", "R03"]
    assert events[0]["url_variant"] == "pdf_url"
    assert events[1]["identity_group"]["ref_ids"] == ["R01", "R02"]
    assert events[1]["optional_metadata"]["title"]["value"] == "Metadata Adapter Paper"
    assert events[2]["source_family"] == "company_blog"
    serialized = (out_dir / "source-metadata-summary.json").read_text() + (out_dir / "source-metadata-events.jsonl").read_text()
    for forbidden in ["<html", "</html>", "%PDF-", "raw_text", "chunk_text", "trusted_fact", "body must not be serialized"]:
        assert forbidden.lower() not in serialized.lower()


def test_missing_acquisition_event_is_blocked_not_silent(tmp_path: Path) -> None:
    script = _load_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    paths = _artifact_files(tmp_path)
    _write_json(selection_path, _selection())
    _write_jsonl(acquisition_path, _acquisition_rows(paths, tmp_path)[:2])

    events, summary = script.build_metadata_outputs(selection_path, acquisition_path, tmp_path / "out", repo_root=tmp_path)

    blocked = events[-1]
    assert blocked["ref_id"] == "R03"
    assert blocked["metadata_status"] == "blocked"
    assert blocked["acquisition"]["failure_code"] == "missing_acquisition_event"
    assert summary["diagnostic_counts"]["missing_acquisition_event"] == 1


def test_checksum_mismatch_records_diagnostic(tmp_path: Path) -> None:
    script = _load_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    paths = _artifact_files(tmp_path)
    rows = _acquisition_rows(paths, tmp_path)
    rows[1]["sha256"] = "0" * 64
    _write_json(selection_path, _selection())
    _write_jsonl(acquisition_path, rows)

    events, summary = script.build_metadata_outputs(selection_path, acquisition_path, tmp_path / "out", repo_root=tmp_path)

    arxiv_abs = events[1]
    assert arxiv_abs["metadata_status"] == "metadata_available_with_diagnostics"
    assert any(item["code"] == "artifact_checksum_mismatch" for item in arxiv_abs["diagnostics"])
    assert summary["diagnostic_counts"]["artifact_checksum_mismatch"] == 1


def test_rejects_malformed_selection(tmp_path: Path) -> None:
    script = _load_script()
    selection_path = tmp_path / "selection.json"
    acquisition_path = tmp_path / "source-acquisition-events.jsonl"
    _write_json(selection_path, {"refs": [{"ref_id": "R01"}]})
    _write_jsonl(acquisition_path, [])

    with pytest.raises(script.AdapterInputError, match="selection_ref_required_fields"):
        script.build_metadata_outputs(selection_path, acquisition_path, tmp_path / "out", repo_root=tmp_path)
