"""Contract tests for M028 S04 universal loader evidence bundles."""

from __future__ import annotations

import importlib.util
import json
from copy import deepcopy
from pathlib import Path
from types import ModuleType

import pytest

REPO_ROOT = Path(__file__).parents[1]
BUILD_SCRIPT_PATH = REPO_ROOT / "scripts" / "build_m028_universal_loader_evidence_bundles.py"
REAL_CORPUS_DIR = REPO_ROOT / "data" / "article_corpora" / "m028-universal-loader-runtime-smoke-v1"


def _load_script() -> ModuleType:
    spec = importlib.util.spec_from_file_location("build_m028_universal_loader_evidence_bundles", BUILD_SCRIPT_PATH)
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


def _selection() -> dict[str, object]:
    refs: list[dict[str, object]] = [
        {
            "ref_id": "R01",
            "url": "https://arxiv.org/pdf/2605.20897.pdf",
            "canonical_url": "https://arxiv.org/abs/2605.20897",
            "source_kind": "arxiv_pdf_url",
            "normalized_identity": "arxiv:2605.20897",
            "loader_owns_selection": False,
            "selection_policy": "fixture",
        },
        {
            "ref_id": "R02",
            "url": "https://arxiv.org/abs/2605.21401",
            "canonical_url": "https://arxiv.org/abs/2605.21401",
            "source_kind": "arxiv_abs_url",
            "normalized_identity": "arxiv:2605.21401",
            "loader_owns_selection": False,
            "selection_policy": "fixture",
        },
        {
            "ref_id": "R03",
            "url": "https://www.nature.com/articles/example",
            "canonical_url": "https://www.nature.com/articles/example",
            "source_kind": "nature_article_url",
            "normalized_identity": "nature:articles_example",
            "loader_owns_selection": False,
            "selection_policy": "fixture",
        },
        {
            "ref_id": "R07",
            "url": "https://developer.nvidia.com/blog/example/?linkId=fixture",
            "canonical_url": "https://developer.nvidia.com/blog/example/",
            "source_kind": "company_blog_url",
            "normalized_identity": "company_blog:nvidia:example",
            "loader_owns_selection": False,
            "selection_policy": "fixture",
        },
        {
            "ref_id": "R10",
            "url": "https://arxiv.org/abs/2605.20897",
            "canonical_url": "https://arxiv.org/abs/2605.20897",
            "source_kind": "arxiv_abs_url",
            "normalized_identity": "arxiv:2605.20897",
            "loader_owns_selection": False,
            "selection_policy": "fixture",
        },
    ]
    return {"schema_version": "m028.selection.fixture", "refs": refs}


def _source_family(source_kind: str) -> str:
    if source_kind.startswith("arxiv_"):
        return "arxiv"
    if source_kind == "nature_article_url":
        return "nature"
    return "company_blog"


def _variant(source_kind: str) -> str:
    if source_kind == "arxiv_pdf_url":
        return "pdf_url"
    if source_kind == "arxiv_abs_url":
        return "abs_url"
    return source_kind


def _summary_for(selection: dict[str, object], schema_version: str) -> dict[str, object]:
    refs = selection["refs"]
    assert isinstance(refs, list)
    source_kind_counts: dict[str, int] = {}
    for ref in refs:
        assert isinstance(ref, dict)
        source_kind = str(ref["source_kind"])
        source_kind_counts[source_kind] = source_kind_counts.get(source_kind, 0) + 1
    return {
        "schema_version": schema_version,
        "url_ref_count": len(refs),
        "ref_count": len(refs),
        "normalized_identity_count": len({str(ref["normalized_identity"]) for ref in refs if isinstance(ref, dict)}),
        "source_kind_counts": source_kind_counts,
        "safety_flags": {"raw_article_text_embedded": False, "source_payload_embedded": False},
    }


def _write_inputs(root: Path) -> tuple[Path, Path, Path, Path, Path, Path, dict[str, int]]:
    selection = _selection()
    refs = selection["refs"]
    assert isinstance(refs, list)
    source_counts: dict[str, int] = {}
    source_rows: list[dict[str, object]] = []
    metadata_rows: list[dict[str, object]] = []
    pdf_rows: list[dict[str, object]] = []
    identity_groups: dict[str, list[str]] = {}
    for ref in refs:
        assert isinstance(ref, dict)
        identity_groups.setdefault(str(ref["normalized_identity"]), []).append(str(ref["ref_id"]))
    for ref in refs:
        assert isinstance(ref, dict)
        ref_id = str(ref["ref_id"])
        source_kind = str(ref["source_kind"])
        source_counts[source_kind] = source_counts.get(source_kind, 0) + 1
        family = _source_family(source_kind)
        artifact_path = f"sources/{ref_id}.dat"
        source_rows.append(
            {
                "ref_id": ref_id,
                "url": ref["url"],
                "canonical_url": ref["canonical_url"],
                "source_kind": source_kind,
                "normalized_identity": ref["normalized_identity"],
                "artifact_path": artifact_path,
                "content_type": "application/pdf" if source_kind == "arxiv_pdf_url" else "text/html; charset=utf-8",
                "byte_count": 42,
                "sha256": "0" * 64,
                "status": "captured",
                "terminal": True,
                "http_status": 200,
                "failure_code": None,
            }
        )
        metadata_rows.append(
            {
                "schema_version": "m028.source-metadata-event.v1",
                "ref_id": ref_id,
                "url": ref["url"],
                "canonical_url": ref["canonical_url"],
                "source_kind": source_kind,
                "source_family": family,
                "normalized_identity": ref["normalized_identity"],
                "url_variant": _variant(source_kind),
                "metadata_status": "metadata_available",
                "optional_metadata_gaps": [],
                "artifact": {
                    "path": artifact_path,
                    "content_type": "application/pdf" if source_kind == "arxiv_pdf_url" else "text/html; charset=utf-8",
                    "byte_count": 42,
                    "sha256": "0" * 64,
                    "checksum_verified": True,
                    "payload_embedded": False,
                },
                "safety_flags": {"source_payload_embedded": False, "binary_payload_embedded": False, "model_output_embedded": False},
                "diagnostics": [],
            }
        )
        if source_kind == "arxiv_pdf_url":
            pdf_status = "acquired_existing_pdf"
            reason = "existing_pdf_checksum_signature_verified"
            pdf_artifact = {
                "path": artifact_path,
                "content_type": "application/pdf",
                "byte_count": 42,
                "sha256": "0" * 64,
                "checksum_verified": True,
                "signature_verified": True,
                "bytes_embedded": False,
            }
        elif source_kind == "arxiv_abs_url":
            pdf_status = "not_acquired"
            reason = "arxiv_abs_no_local_pdf_artifact"
            pdf_artifact = {"path": None, "content_type": None, "byte_count": None, "sha256": None, "checksum_verified": False, "signature_verified": False}
        else:
            pdf_status = "not_applicable"
            reason = "not_applicable_non_arxiv_pdf_source"
            pdf_artifact = {"path": None, "content_type": None, "byte_count": None, "sha256": None, "checksum_verified": False, "signature_verified": False}
        pdf_rows.append(
            {
                "schema_version": "m028.pdf-acquisition-event.v1",
                "ref_id": ref_id,
                "url": ref["url"],
                "canonical_url": ref["canonical_url"],
                "source_kind": source_kind,
                "source_family": family,
                "normalized_identity": ref["normalized_identity"],
                "url_variant": _variant(source_kind),
                "candidate_pdf": {"candidate_kind": "explicit_arxiv_pdf_url" if source_kind == "arxiv_pdf_url" else "not_fixture"},
                "pdf_acquisition": {"status": pdf_status, "terminal": True, "reason": reason},
                "pdf_artifact": pdf_artifact,
                "safety_flags": {"raw_pdf_bytes_embedded": False, "chunk_content_embedded": False, "ladybugdb_written": False},
                "diagnostics": [],
            }
        )
    selection_path = root / "selection.json"
    source_path = root / "source-acquisition-events.jsonl"
    metadata_path = root / "source-metadata-events.jsonl"
    metadata_summary_path = root / "source-metadata-summary.json"
    pdf_path = root / "pdf-acquisition-events.jsonl"
    pdf_summary_path = root / "pdf-acquisition-summary.json"
    _write_json(selection_path, selection)
    _write_jsonl(source_path, source_rows)
    _write_jsonl(metadata_path, metadata_rows)
    _write_json(metadata_summary_path, _summary_for(selection, "m028.source-metadata-summary.v1"))
    _write_jsonl(pdf_path, pdf_rows)
    _write_json(pdf_summary_path, _summary_for(selection, "m028.pdf-acquisition-summary.v1"))
    return selection_path, source_path, metadata_path, metadata_summary_path, pdf_path, pdf_summary_path, source_counts


def test_fixture_build_preserves_duplicate_identity_and_fail_closed_flags(tmp_path: Path) -> None:
    module = _load_script()
    selection, source, metadata, metadata_summary, pdf, pdf_summary, source_counts = _write_inputs(tmp_path)
    bundles, summary = module.build_universal_loader_evidence_outputs(
        selection,
        source,
        metadata,
        metadata_summary,
        pdf,
        pdf_summary,
        tmp_path,
        repo_root=tmp_path,
        expected_ref_count=5,
        expected_identity_count=4,
        expected_source_kind_counts=source_counts,
    )

    assert len(bundles) == 5
    assert summary["url_ref_count"] == 5
    assert summary["normalized_identity_count"] == 4
    assert summary["duplicate_identity_groups"] == [
        {
            "group_id": "identity:arxiv:2605.20897",
            "has_url_variants": True,
            "normalized_identity": "arxiv:2605.20897",
            "ref_ids": ["R01", "R10"],
            "url_ref_count": 2,
            "url_variants": ["pdf_url", "abs_url"],
        }
    ]
    assert all(value == 0 for value in summary["unsafe_claim_counts"].values())
    assert all(bundle["loader_evidence"]["kg_import_eligible"] is False for bundle in bundles)
    assert (tmp_path / "universal-loader-evidence-bundles.jsonl").exists()
    assert "## Failure Modes" in (tmp_path / "universal-loader-evidence-report.md").read_text(encoding="utf-8")


def test_missing_pdf_event_is_stable_input_error(tmp_path: Path) -> None:
    module = _load_script()
    selection, source, metadata, metadata_summary, pdf, pdf_summary, source_counts = _write_inputs(tmp_path)
    pdf_rows = _read_jsonl(pdf)
    _write_jsonl(pdf, pdf_rows[:-1])

    with pytest.raises(module.UniversalLoaderEvidenceInputError, match="pdf_ref_set_mismatch"):
        module.build_universal_loader_evidence_outputs(
            selection,
            source,
            metadata,
            metadata_summary,
            pdf,
            pdf_summary,
            tmp_path,
            repo_root=tmp_path,
            expected_ref_count=5,
            expected_identity_count=4,
            expected_source_kind_counts=source_counts,
        )


def test_upstream_unsafe_flag_is_stable_input_error(tmp_path: Path) -> None:
    module = _load_script()
    selection, source, metadata, metadata_summary, pdf, pdf_summary, source_counts = _write_inputs(tmp_path)
    pdf_rows = _read_jsonl(pdf)
    pdf_rows[0]["safety_flags"] = deepcopy(pdf_rows[0]["safety_flags"])
    assert isinstance(pdf_rows[0]["safety_flags"], dict)
    pdf_rows[0]["safety_flags"]["ladybugdb_written"] = True
    _write_jsonl(pdf, pdf_rows)

    with pytest.raises(module.UniversalLoaderEvidenceInputError, match="pdf_unsafe_claim:R01"):
        module.build_universal_loader_evidence_outputs(
            selection,
            source,
            metadata,
            metadata_summary,
            pdf,
            pdf_summary,
            tmp_path,
            repo_root=tmp_path,
            expected_ref_count=5,
            expected_identity_count=4,
            expected_source_kind_counts=source_counts,
        )


def test_real_corpus_build_contract(tmp_path: Path) -> None:
    module = _load_script()
    bundles, summary = module.build_universal_loader_evidence_outputs(
        REAL_CORPUS_DIR / "selection.json",
        REAL_CORPUS_DIR / "source-acquisition-events.jsonl",
        REAL_CORPUS_DIR / "source-metadata-events.jsonl",
        REAL_CORPUS_DIR / "source-metadata-summary.json",
        REAL_CORPUS_DIR / "pdf-acquisition-events.jsonl",
        REAL_CORPUS_DIR / "pdf-acquisition-summary.json",
        tmp_path,
        repo_root=REPO_ROOT,
    )

    assert len(bundles) == 21
    assert summary["url_ref_count"] == 21
    assert summary["normalized_identity_count"] == 20
    assert summary["duplicate_identity_groups"][0]["ref_ids"] == ["R01", "R10"]
    assert summary["source_kind_counts"] == {
        "arxiv_abs_url": 15,
        "arxiv_pdf_url": 4,
        "company_blog_url": 1,
        "nature_article_url": 1,
    }
    assert all(value == 0 for value in summary["unsafe_claim_counts"].values())
    assert _read_json(tmp_path / "universal-loader-evidence-summary.json")["schema_version"] == "m028.universal-loader-evidence-summary.v1"
