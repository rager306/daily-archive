"""Contract tests for M028 S03 PDF acquisition diagnostics."""

from __future__ import annotations

import importlib.util
import json
from copy import deepcopy
from pathlib import Path
from types import ModuleType

import pytest

REPO_ROOT = Path(__file__).parents[1]
BUILD_SCRIPT_PATH = REPO_ROOT / "scripts" / "build_m028_pdf_acquisition_diagnostics.py"
REAL_CORPUS_DIR = REPO_ROOT / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"


def _load_script() -> ModuleType:
    spec = importlib.util.spec_from_file_location("build_m028_pdf_acquisition_diagnostics", BUILD_SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _write_json(path: Path, payload: dict[str, object]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def _write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n", encoding="utf-8")


def _read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_jsonl(path: Path) -> list[dict[str, object]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def _sha256(path: Path) -> str:
    import hashlib

    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _fixture_selection() -> dict[str, object]:
    refs: list[dict[str, object]] = [
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
            "url": "https://arxiv.org/abs/2605.21401",
            "canonical_url": "https://arxiv.org/abs/2605.21401",
            "source_kind": "arxiv_abs_url",
            "normalized_identity": "arxiv:2605.21401",
            "arxiv_id": "2605.21401",
            "arxiv_unversioned_id": "2605.21401",
        },
        {
            "ref_id": "R03",
            "url": "https://www.nature.com/articles/example",
            "canonical_url": "https://www.nature.com/articles/example",
            "source_kind": "nature_article_url",
            "normalized_identity": "nature:articles_example",
        },
        {
            "ref_id": "R07",
            "url": "https://developer.nvidia.com/blog/example/?linkId=fixture",
            "canonical_url": "https://developer.nvidia.com/blog/example/",
            "source_kind": "company_blog_url",
            "normalized_identity": "company_blog:nvidia:example",
        },
        {
            "ref_id": "R10",
            "url": "https://arxiv.org/abs/2605.20897",
            "canonical_url": "https://arxiv.org/abs/2605.20897",
            "source_kind": "arxiv_abs_url",
            "normalized_identity": "arxiv:2605.20897",
            "arxiv_id": "2605.20897",
            "arxiv_unversioned_id": "2605.20897",
        },
    ]
    return {"schema_version": "m028.selection.v1", "refs": refs}


def _fixture_files(root: Path, *, malformed_pdf: bool = False) -> dict[str, Path]:
    sources = root / "sources"
    sources.mkdir()
    paths = {
        "R01": sources / "R01.pdf",
        "R02": sources / "R02.html",
        "R03": sources / "R03.html",
        "R07": sources / "R07.html",
        "R10": sources / "R10.html",
    }
    paths["R01"].write_bytes((b"not a pdf\n" if malformed_pdf else b"%PDF-1.4\n") + b"local fixture bytes only\n")
    paths["R02"].write_text("<meta name='citation_pdf_url' content='https://arxiv.org/pdf/2605.21401'>", encoding="utf-8")
    paths["R03"].write_text("<meta name='citation_pdf_url' content='https://www.nature.com/articles/example.pdf'>", encoding="utf-8")
    paths["R07"].write_text("<html><body>blog body must not leak</body></html>", encoding="utf-8")
    paths["R10"].write_text("<meta name='citation_pdf_url' content='https://arxiv.org/pdf/2605.20897'>", encoding="utf-8")
    return paths


def _acquisition_rows(selection: dict[str, object], paths: dict[str, Path], root: Path) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    refs = selection["refs"]
    assert isinstance(refs, list)
    for ref in refs:
        assert isinstance(ref, dict)
        ref_id = str(ref["ref_id"])
        path = paths[ref_id]
        rows.append(
            {
                "ref_id": ref_id,
                "url": ref["url"],
                "canonical_url": ref["canonical_url"],
                "source_kind": ref["source_kind"],
                "normalized_identity": ref["normalized_identity"],
                "artifact_path": str(path.relative_to(root)),
                "content_type": "application/pdf" if path.suffix == ".pdf" else "text/html; charset=utf-8",
                "byte_count": path.stat().st_size,
                "sha256": _sha256(path),
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


def _metadata_rows(selection: dict[str, object], acquisition_rows: list[dict[str, object]]) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    acquisition_by_ref = {str(row["ref_id"]): row for row in acquisition_rows}
    refs = selection["refs"]
    assert isinstance(refs, list)
    for ref in refs:
        assert isinstance(ref, dict)
        ref_id = str(ref["ref_id"])
        source_kind = str(ref["source_kind"])
        pdf_url: str | None = None
        pdf_source: str | None = None
        if source_kind == "arxiv_abs_url":
            pdf_url = f"https://arxiv.org/pdf/{ref['arxiv_unversioned_id']}"
            pdf_source = "citation_pdf_url"
        elif source_kind == "nature_article_url":
            pdf_url = "https://www.nature.com/articles/example.pdf"
            pdf_source = "citation_pdf_url"
        rows.append(
            {
                "schema_version": "m028.source-metadata-event.v1",
                "ref_id": ref_id,
                "url": ref["url"],
                "source_kind": source_kind,
                "source_family": "arxiv" if source_kind.startswith("arxiv_") else ("nature" if source_kind == "nature_article_url" else "company_blog"),
                "normalized_identity": ref["normalized_identity"],
                "canonical_url": ref["canonical_url"],
                "url_variant": "pdf_url" if source_kind == "arxiv_pdf_url" else ("abs_url" if source_kind == "arxiv_abs_url" else source_kind),
                "acquisition": {"status": "captured", "terminal": True, "captured": True, "failure_code": None, "http_status": 200},
                "artifact": {
                    "path": acquisition_by_ref[ref_id]["artifact_path"],
                    "exists": True,
                    "content_type": acquisition_by_ref[ref_id]["content_type"],
                    "byte_count": acquisition_by_ref[ref_id]["byte_count"],
                    "sha256": acquisition_by_ref[ref_id]["sha256"],
                    "checksum_verified": True,
                    "payload_embedded": False,
                },
                "optional_metadata": {
                    "pdf_url": {
                        "status": "present" if pdf_url else "missing",
                        "value": pdf_url,
                        "source": pdf_source,
                        "missing_reason": None if pdf_url else ("not_applicable" if not source_kind.startswith("arxiv_") else "not_found"),
                    }
                },
                "safety_flags": {"source_payload_embedded": False, "binary_payload_embedded": False},
                "diagnostics": [],
            }
        )
    return rows


def _metadata_summary(selection: dict[str, object]) -> dict[str, object]:
    refs = selection["refs"]
    assert isinstance(refs, list)
    source_kind_counts: dict[str, int] = {}
    identities = {str(ref["normalized_identity"]) for ref in refs if isinstance(ref, dict)}
    for ref in refs:
        assert isinstance(ref, dict)
        source_kind_counts[str(ref["source_kind"])] = source_kind_counts.get(str(ref["source_kind"]), 0) + 1
    return {
        "schema_version": "m028.source-metadata-summary.v1",
        "url_ref_count": len(refs),
        "ref_count": len(refs),
        "normalized_identity_count": len(identities),
        "source_kind_counts": source_kind_counts,
        "safety_flags": {"source_payload_embedded": False, "binary_payload_embedded": False},
    }


def _write_inputs(root: Path, *, malformed_pdf: bool = False) -> tuple[Path, Path, Path, Path, dict[str, object], list[dict[str, object]], list[dict[str, object]], dict[str, object]]:
    selection = _fixture_selection()
    paths = _fixture_files(root, malformed_pdf=malformed_pdf)
    acquisition = _acquisition_rows(selection, paths, root)
    metadata = _metadata_rows(selection, acquisition)
    summary = _metadata_summary(selection)
    selection_path = root / "selection.json"
    acquisition_path = root / "source-acquisition-events.jsonl"
    metadata_path = root / "source-metadata-events.jsonl"
    summary_path = root / "source-metadata-summary.json"
    _write_json(selection_path, selection)
    _write_jsonl(acquisition_path, acquisition)
    _write_jsonl(metadata_path, metadata)
    _write_json(summary_path, summary)
    return selection_path, acquisition_path, metadata_path, summary_path, selection, acquisition, metadata, summary


def test_builds_metadata_only_pdf_diagnostics_for_fixture_refs(tmp_path: Path) -> None:
    script = _load_script()
    selection_path, acquisition_path, metadata_path, summary_path, *_ = _write_inputs(tmp_path)

    events, summary = script.build_pdf_acquisition_outputs(
        selection_path,
        acquisition_path,
        metadata_path,
        summary_path,
        tmp_path / "out",
        repo_root=tmp_path,
        expected_ref_count=5,
        expected_identity_count=4,
        expected_source_kind_counts={"arxiv_abs_url": 2, "arxiv_pdf_url": 1, "company_blog_url": 1, "nature_article_url": 1},
    )

    by_ref = {event["ref_id"]: event for event in events}
    assert summary["url_ref_count"] == 5
    assert summary["normalized_identity_count"] == 4
    assert summary["duplicate_identity_group_count"] == 1
    assert summary["pdf_status_counts"] == {
        "acquired_existing_pdf": 1,
        "not_applicable": 2,
        "not_acquired": 2,
    }
    assert summary["non_acquired_reason_counts"] == {
        "arxiv_abs_no_local_pdf_artifact": 2,
        "not_applicable_non_arxiv_pdf_source": 2,
    }
    assert by_ref["R01"]["pdf_acquisition"]["status"] == "acquired_existing_pdf"
    assert by_ref["R01"]["pdf_artifact"]["checksum_verified"] is True
    assert by_ref["R01"]["pdf_artifact"]["signature_verified"] is True
    assert by_ref["R02"]["candidate_pdf"]["url"] == "https://arxiv.org/pdf/2605.21401"
    assert by_ref["R02"]["pdf_acquisition"]["reason"] == "arxiv_abs_no_local_pdf_artifact"
    assert by_ref["R10"]["pdf_acquisition"]["status"] == "not_acquired"
    assert by_ref["R10"]["identity_group"]["ref_ids"] == ["R01", "R10"]
    assert by_ref["R03"]["candidate_pdf"]["is_candidate"] is False
    assert by_ref["R03"]["pdf_acquisition"]["reason"] == "not_applicable_non_arxiv_pdf_source"
    assert by_ref["R07"]["pdf_acquisition"]["reason"] == "not_applicable_non_arxiv_pdf_source"
    assert all(flag is False for flag in summary["safety_flags"].values())
    serialized = (tmp_path / "out" / "pdf-acquisition-summary.json").read_text() + (tmp_path / "out" / "pdf-acquisition-events.jsonl").read_text()
    for forbidden in ["%PDF-", "<html", "</html>", "blog body must not leak", "raw_text", "body", "payload"]:
        assert forbidden.lower() not in serialized.lower()


def test_malformed_existing_pdf_signature_becomes_typed_diagnostic(tmp_path: Path) -> None:
    script = _load_script()
    selection_path, acquisition_path, metadata_path, summary_path, *_ = _write_inputs(tmp_path, malformed_pdf=True)

    events, summary = script.build_pdf_acquisition_outputs(
        selection_path,
        acquisition_path,
        metadata_path,
        summary_path,
        tmp_path / "out",
        repo_root=tmp_path,
        expected_ref_count=5,
        expected_identity_count=4,
        expected_source_kind_counts={"arxiv_abs_url": 2, "arxiv_pdf_url": 1, "company_blog_url": 1, "nature_article_url": 1},
    )

    r01 = next(event for event in events if event["ref_id"] == "R01")
    assert r01["pdf_acquisition"] == {"status": "not_acquired", "terminal": True, "reason": "malformed_existing_pdf_signature"}
    assert r01["pdf_artifact"]["signature_verified"] is False
    assert any(item["code"] == "malformed_existing_pdf_signature" for item in r01["diagnostics"])
    assert summary["diagnostic_counts"]["malformed_existing_pdf_signature"] == 1


def test_missing_acquisition_linkage_is_stable_input_error(tmp_path: Path) -> None:
    script = _load_script()
    selection_path, acquisition_path, metadata_path, summary_path, selection, acquisition, *_ = _write_inputs(tmp_path)
    _write_jsonl(acquisition_path, [row for row in acquisition if row["ref_id"] != "R10"])

    with pytest.raises(script.PdfDiagnosticInputError, match="acquisition_ref_set_mismatch"):
        script.build_pdf_acquisition_outputs(
            selection_path,
            acquisition_path,
            metadata_path,
            summary_path,
            tmp_path / "out",
            repo_root=tmp_path,
            expected_ref_count=5,
            expected_identity_count=4,
            expected_source_kind_counts={"arxiv_abs_url": 2, "arxiv_pdf_url": 1, "company_blog_url": 1, "nature_article_url": 1},
        )


def test_checksum_mismatch_records_typed_non_acquired_reason(tmp_path: Path) -> None:
    script = _load_script()
    selection_path, acquisition_path, metadata_path, summary_path, selection, acquisition, *_ = _write_inputs(tmp_path)
    drifted = deepcopy(acquisition)
    drifted[0]["sha256"] = "0" * 64
    _write_jsonl(acquisition_path, drifted)

    events, summary = script.build_pdf_acquisition_outputs(
        selection_path,
        acquisition_path,
        metadata_path,
        summary_path,
        tmp_path / "out",
        repo_root=tmp_path,
        expected_ref_count=5,
        expected_identity_count=4,
        expected_source_kind_counts={"arxiv_abs_url": 2, "arxiv_pdf_url": 1, "company_blog_url": 1, "nature_article_url": 1},
    )

    r01 = next(event for event in events if event["ref_id"] == "R01")
    assert r01["pdf_acquisition"]["reason"] == "artifact_checksum_mismatch"
    assert r01["pdf_artifact"]["checksum_verified"] is False
    assert summary["diagnostic_counts"]["artifact_checksum_mismatch"] == 1


def test_real_corpus_regeneration_contract() -> None:
    script = _load_script()

    events, summary = script.build_pdf_acquisition_outputs(
        REAL_CORPUS_DIR / "selection.json",
        REAL_CORPUS_DIR / "source-acquisition-events.jsonl",
        REAL_CORPUS_DIR / "source-metadata-events.jsonl",
        REAL_CORPUS_DIR / "source-metadata-summary.json",
        REAL_CORPUS_DIR,
        repo_root=REPO_ROOT,
    )

    by_ref = {event["ref_id"]: event for event in events}
    assert summary["url_ref_count"] == 21
    assert summary["normalized_identity_count"] == 20
    assert summary["source_kind_counts"] == {"arxiv_abs_url": 15, "arxiv_pdf_url": 4, "company_blog_url": 1, "nature_article_url": 1}
    assert summary["pdf_status_counts"] == {"acquired_existing_pdf": 4, "not_applicable": 2, "not_acquired": 15}
    assert summary["non_acquired_reason_counts"] == {
        "arxiv_abs_no_local_pdf_artifact": 15,
        "not_applicable_non_arxiv_pdf_source": 2,
    }
    assert by_ref["R01"]["identity_group"]["ref_ids"] == ["R01", "R10"]
    assert by_ref["R10"]["pdf_acquisition"]["status"] == "not_acquired"
    assert by_ref["R03"]["candidate_pdf"]["metadata_pdf_url_present"] is True
    assert by_ref["R03"]["pdf_acquisition"]["reason"] == "not_applicable_non_arxiv_pdf_source"
    assert by_ref["R07"]["candidate_pdf"]["is_candidate"] is False
    assert all(by_ref[ref_id]["pdf_artifact"]["signature_verified"] is True for ref_id in ["R01", "R08", "R12", "R13"])
    report = (REAL_CORPUS_DIR / "pdf-acquisition-report.md").read_text(encoding="utf-8")
    assert "## Failure Modes" in report
    assert "## Load Profile" in report
    assert "## Negative Tests" in report
    serialized = (REAL_CORPUS_DIR / "pdf-acquisition-events.jsonl").read_text(encoding="utf-8") + (REAL_CORPUS_DIR / "pdf-acquisition-summary.json").read_text(encoding="utf-8")
    for forbidden in ["%PDF-", "<html", "</html>", "raw_text", "chunk_text", "trusted_fact"]:
        assert forbidden.lower() not in serialized.lower()
