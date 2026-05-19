from __future__ import annotations

import json

from arxiv_archive.source_asset_manifest import (
    AssetRecord,
    PreservedSourceFile,
    SourceAssetManifest,
    SourceSpan,
    validate_source_asset_manifest,
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
