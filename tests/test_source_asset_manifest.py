from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.source_asset_manifest import (
    AssetRecord,
    PreservedSourceFile,
    SourceAssetManifest,
    SourceSpan,
    attach_annotation_asset_links,
    preserve_source_assets_for_paper,
    preserve_source_assets_manifest,
    validate_source_asset_manifest,
    write_source_asset_run,
)

VALID_SHA = "a" * 64


def _source_file() -> PreservedSourceFile:
    return PreservedSourceFile(
        source_file_id="p1:source:pdf",
        paper_id="p1",
        source_role="original_pdf",
        original_path="/cache/p1.pdf",
        workspace_path="papers/p1/source/p1.pdf",
        sha256=VALID_SHA,
        byte_size=1234,
        media_type="application/pdf",
        provenance={"source": "gold_manifest_required_path"},
    )


def _asset() -> AssetRecord:
    return AssetRecord(
        asset_id="p1:asset:figure:0001",
        paper_id="p1",
        asset_type="figure",
        extraction_state="linked_not_extracted",
        source_file_id="p1:source:pdf",
        chunk_id="p1:chunk-0001",
        source_artifact="normalized_markdown:p1",
        source_span=SourceSpan(coordinate_space="normalized_markdown", char_start=10, char_end=30),
        provenance={"created_from": "s04_asset_link_hint"},
        warning_codes=("asset_manifest_required",),
    )


def _manifest() -> dict:
    return SourceAssetManifest(
        paper_id="p1",
        workspace_root=".gsd/milestones/M005-dlko4z/slices/S05/run-evidence/papers/p1",
        source_files=(_source_file(),),
        assets=(_asset(),),
    ).to_contract()


def test_source_span_serializes_coordinates_without_content() -> None:
    span = SourceSpan(
        coordinate_space="normalized_markdown",
        char_start=5,
        char_end=15,
        page_start=1,
        page_end=1,
        bbox=(1.0, 2.0, 3.0, 4.0),
    )

    record = span.to_contract()

    assert record == {
        "coordinate_space": "normalized_markdown",
        "char_start": 5,
        "char_end": 15,
        "page_start": 1,
        "page_end": 1,
        "bbox": [1.0, 2.0, 3.0, 4.0],
    }
    assert "text" not in json.dumps(record)


def test_preserved_source_file_serializes_hash_and_redaction_flags() -> None:
    record = _source_file().to_contract()

    assert record["source_file_id"] == "p1:source:pdf"
    assert record["sha256"] == VALID_SHA
    assert record["byte_size"] == 1234
    assert record["media_type"] == "application/pdf"
    assert record["redaction"] == {
        "raw_text_included": False,
        "chunk_text_included": False,
        "raw_binary_included": False,
        "base64_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "optimizer_traces_included": False,
    }
    assert record["production_import_attempted"] is False
    assert record["ladybugdb_written"] is False
    assert "raw pdf bytes" not in json.dumps(record)


def test_asset_record_is_non_fact_and_excluded_from_import() -> None:
    record = _asset().to_contract()

    assert record["asset_id"] == "p1:asset:figure:0001"
    assert record["asset_type"] == "figure"
    assert record["extraction_state"] == "linked_not_extracted"
    assert record["chunk_id"] == "p1:chunk-0001"
    assert record["promoted_to_fact"] is False
    assert "trusted_kg_import" not in record["allowed_uses"]
    assert "trusted_kg_import" in record["excluded_uses"]
    assert "embedding_generation" in record["excluded_uses"]
    assert record["redaction"]["raw_binary_included"] is False
    assert record["warnings"][0]["code"] == "asset_manifest_required"
    assert "figure image bytes" not in json.dumps(record)


def test_source_asset_manifest_validates_redacted_contract() -> None:
    manifest = _manifest()
    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is True
    assert validation.passed is True
    assert manifest["schema_version"] == "m005-source-asset-manifest.v1"
    assert manifest["diagnostics"]["source_file_count"] == 1
    assert manifest["diagnostics"]["asset_count"] == 1
    assert manifest["diagnostics"]["hash_coverage_rate"] == 1.0
    assert manifest["diagnostics"]["asset_counts_by_type"] == {"figure": 1}
    assert manifest["diagnostics"]["extraction_state_counts"] == {"linked_not_extracted": 1}
    assert manifest["promoted_to_fact_count"] == 0
    assert manifest["raw_text_included"] is False
    assert manifest["raw_binary_included"] is False
    assert manifest["embeddings_included"] is False
    assert manifest["vectors_included"] is False
    assert manifest["ladybugdb_written"] is False
    assert manifest["production_import_attempted"] is False


def test_manifest_rejects_missing_required_fields_and_bad_hash() -> None:
    manifest = _manifest()
    del manifest["source_files"][0]["sha256"]
    manifest["assets"][0]["sha256"] = "not-a-sha"

    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is False
    assert validation.refusal_counts["missing_sha256"] == 1
    assert validation.refusal_counts["invalid_sha256"] == 1


def test_manifest_rejects_unresolved_source_file_reference() -> None:
    manifest = _manifest()
    manifest["assets"][0]["source_file_id"] = "p1:source:missing"

    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is False
    assert validation.refusal_counts["unresolved_source_file"] == 1


def test_manifest_rejects_asset_promoted_to_fact_or_import_allowed() -> None:
    manifest = _manifest()
    manifest["assets"][0]["promoted_to_fact"] = True
    manifest["assets"][0]["allowed_uses"].append("trusted_kg_import")

    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is False
    assert validation.refusal_counts["asset_promoted_to_fact"] == 1
    assert validation.refusal_counts["asset_allows_trusted_import"] == 1


def test_manifest_rejects_nested_raw_binary_or_embedding_leakage_without_echoing_value() -> None:
    leaked_value = "do not echo this raw payload"
    manifest = _manifest()
    manifest["assets"][0]["provenance"]["base64"] = leaked_value
    manifest["source_files"][0]["provenance"]["embedding"] = [0.1, 0.2]

    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is False
    assert validation.refusal_counts["raw_content_leakage"] >= 1
    assert validation.refusal_counts["embedding_leakage"] >= 1
    serialized_diagnostics = json.dumps([diagnostic.__dict__ for diagnostic in validation.diagnostics])
    assert leaked_value not in serialized_diagnostics
    assert "0.1" not in serialized_diagnostics


def test_manifest_rejects_unsafe_diagnostic_flags() -> None:
    manifest = _manifest()
    manifest["diagnostics"]["base64_included"] = True
    manifest["raw_binary_included"] = True

    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is False
    assert validation.refusal_counts["unsafe_base64_included"] == 1
    assert validation.refusal_counts["unsafe_raw_binary_included"] == 1


def test_preserve_source_assets_for_paper_copies_markdown_and_pdf(tmp_path: Path) -> None:
    paper_dir = tmp_path / "paper"
    paper_dir.mkdir()
    markdown = paper_dir / "full_text.md"
    pdf = paper_dir / "p1.pdf"
    markdown.write_text("# Paper\n\nBody text stays in copied file only.\n", encoding="utf-8")
    pdf.write_bytes(b"%PDF-1.4\nfixture\n")

    manifest = preserve_source_assets_for_paper(
        {"paper_id": "p1", "required_paths": [str(paper_dir)]},
        workspace_root=tmp_path / "out",
    ).to_contract()
    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is True
    assert manifest["diagnostics"]["source_file_count"] == 2
    assert manifest["diagnostics"]["hash_coverage_rate"] == 1.0
    roles = {source_file["source_role"] for source_file in manifest["source_files"]}
    assert roles == {"original_pdf", "normalized_markdown"}
    for source_file in manifest["source_files"]:
        preserved_path = Path(source_file["workspace_path"])
        assert preserved_path.exists()
        assert len(source_file["sha256"]) == 64
        assert source_file["byte_size"] == preserved_path.stat().st_size
    serialized = json.dumps(manifest)
    assert "Body text stays" not in serialized
    assert "%PDF" not in serialized


def test_preserve_source_assets_for_paper_records_missing_sources(tmp_path: Path) -> None:
    manifest = preserve_source_assets_for_paper(
        {"paper_id": "missing", "required_paths": [str(tmp_path / "does-not-exist")]},
        workspace_root=tmp_path / "out",
    ).to_contract()
    validation = validate_source_asset_manifest(manifest)

    assert validation.valid_manifest is True
    assert manifest["source_files"] == []
    assert manifest["diagnostics"]["source_file_count"] == 0
    assert manifest["diagnostics"]["hash_coverage_rate"] == 0.0
    assert manifest["diagnostics"]["warning_counts"] == {
        "missing_normalized_markdown": 1,
        "missing_original_pdf": 1,
    }


def test_preserve_source_assets_manifest_writes_redacted_run_artifacts(tmp_path: Path) -> None:
    paper_dir = tmp_path / "paper"
    paper_dir.mkdir()
    (paper_dir / "full_text.md").write_text("# Paper\n\nRaw markdown content.\n", encoding="utf-8")
    (paper_dir / "p2.pdf").write_bytes(b"%PDF fixture")
    manifest_path = tmp_path / "gold.json"
    manifest_path.write_text(
        json.dumps({"papers": [{"paper_id": "p2", "required_paths": [str(paper_dir)]}]}),
        encoding="utf-8",
    )
    out = tmp_path / "out"

    result = preserve_source_assets_manifest(manifest_path, output_dir=out)
    write_source_asset_run(result, out)

    summary = json.loads((out / "source-preservation-summary.json").read_text(encoding="utf-8"))
    diagnostics = [json.loads(line) for line in (out / "source-asset-package-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()]
    per_paper_manifest = json.loads((out / "manifests" / "p2-source-assets.json").read_text(encoding="utf-8"))
    assert summary["paper_count"] == 1
    assert summary["valid_manifest_count"] == 1
    assert summary["source_file_count"] == 2
    assert summary["hash_coverage_rate"] == 1.0
    assert diagnostics[0]["valid_manifest"] is True
    assert diagnostics[0]["source_file_count"] == 2
    assert validate_source_asset_manifest(per_paper_manifest).valid_manifest is True
    serialized = json.dumps({"summary": summary, "diagnostics": diagnostics, "manifest": per_paper_manifest})
    assert "Raw markdown content" not in serialized
    assert "%PDF fixture" not in serialized
    assert summary["raw_binary_included"] is False
    assert summary["base64_included"] is False
    assert summary["production_import_attempted"] is False


def test_attach_annotation_asset_links_creates_redacted_assets(tmp_path: Path) -> None:
    paper_dir = tmp_path / "paper"
    paper_dir.mkdir()
    (paper_dir / "full_text.md").write_text("# Paper\n\nTable text stays in source only.\n", encoding="utf-8")
    manifest = preserve_source_assets_for_paper(
        {"paper_id": "p3", "required_paths": [str(paper_dir)]},
        workspace_root=tmp_path / "out",
    ).to_contract()
    annotation_path = tmp_path / "annotations.jsonl"
    annotation_path.write_text(
        json.dumps(
            {
                "paper_id": "p3",
                "chunk_annotation_coverage": [
                    {
                        "chunk_id": "p3:chunk-table",
                        "chunk_type": "table_context",
                        "route": "table_extraction",
                        "state": "repair_required",
                        "annotation_types": ["route_hint", "structural_type", "asset_link_hint"],
                        "confidence_classes": ["deterministic", "heuristic"],
                        "warning_codes": ["asset_manifest_required", "table_route_requires_review"],
                    },
                    {
                        "chunk_id": "p3:chunk-figure",
                        "chunk_type": "figure_caption_context",
                        "route": "retrieval_only",
                        "state": "ok_for_retrieval_only",
                        "annotation_types": ["structural_type", "asset_link_hint"],
                        "confidence_classes": ["deterministic", "heuristic"],
                        "warning_codes": ["asset_manifest_required", "figure_route_not_import_ready"],
                    },
                    {
                        "chunk_id": "p3:chunk-prose",
                        "chunk_type": "claim_candidate",
                        "route": "claim_extraction",
                        "state": "repair_required",
                        "annotation_types": ["route_hint"],
                        "confidence_classes": ["deterministic"],
                        "warning_codes": ["claim_route_requires_review"],
                    },
                ],
            }
        )
        + "\n",
        encoding="utf-8",
    )
    structure_path = tmp_path / "structure.jsonl"
    structure_path.write_text(
        json.dumps(
            {
                "paper_id": "p3",
                "chunk_diagnostics": [
                    {
                        "chunk_id": "p3:chunk-table",
                        "source_span": {
                            "coordinate_space": "normalized_markdown",
                            "char_start": 1,
                            "char_end": 10,
                            "page_start": None,
                            "page_end": None,
                        },
                    },
                    {
                        "chunk_id": "p3:chunk-figure",
                        "source_span": {
                            "coordinate_space": "normalized_markdown",
                            "char_start": 11,
                            "char_end": 20,
                            "page_start": None,
                            "page_end": None,
                        },
                    },
                ],
            }
        )
        + "\n",
        encoding="utf-8",
    )

    linked = attach_annotation_asset_links(
        (manifest,),
        annotation_diagnostics_path=annotation_path,
        structure_diagnostics_path=structure_path,
    )[0]
    validation = validate_source_asset_manifest(linked)

    assert validation.valid_manifest is True
    assert linked["diagnostics"]["asset_count"] == 2
    assert linked["diagnostics"]["asset_counts_by_type"] == {"figure": 1, "table": 1}
    assert linked["diagnostics"]["extraction_state_counts"] == {"linked_not_extracted": 2}
    assert {asset["asset_type"] for asset in linked["assets"]} == {"table", "figure"}
    assert all(asset["promoted_to_fact"] is False for asset in linked["assets"])
    assert all("trusted_kg_import" in asset["excluded_uses"] for asset in linked["assets"])
    assert all(asset["source_span"]["coordinate_space"] == "normalized_markdown" for asset in linked["assets"])
    serialized = json.dumps(linked)
    assert "Table text stays" not in serialized
    assert "base64" in serialized


def test_attach_annotation_asset_links_counts_reference_equation_and_metadata(tmp_path: Path) -> None:
    manifest = SourceAssetManifest(paper_id="p4", workspace_root=str(tmp_path / "papers" / "p4")).to_contract()
    annotation_path = tmp_path / "annotations.jsonl"
    annotation_path.write_text(
        json.dumps(
            {
                "paper_id": "p4",
                "chunk_annotation_coverage": [
                    {"chunk_id": "p4:eq", "chunk_type": "equation_context", "route": "retrieval_only", "state": "ok_for_retrieval_only", "warning_codes": []},
                    {"chunk_id": "p4:ref", "chunk_type": "reference_entry", "route": "citation_graph", "state": "repair_required", "warning_codes": []},
                    {"chunk_id": "p4:meta", "chunk_type": "metadata", "route": "metadata_graph", "state": "repair_required", "warning_codes": []},
                ],
            }
        )
        + "\n",
        encoding="utf-8",
    )

    linked = attach_annotation_asset_links((manifest,), annotation_diagnostics_path=annotation_path)[0]

    assert validate_source_asset_manifest(linked).valid_manifest is True
    assert linked["diagnostics"]["asset_counts_by_type"] == {"equation": 1, "metadata": 1, "reference": 1}
    assert linked["diagnostics"]["extraction_state_counts"] == {"linked_not_extracted": 3}
    assert all(asset["source_span"] is None for asset in linked["assets"])
    assert all(any(warning["code"] == "missing_source_span" for warning in asset["warnings"]) for asset in linked["assets"])
    assert all(any(warning["code"] == "missing_preserved_source_file" for warning in asset["warnings"]) for asset in linked["assets"])
