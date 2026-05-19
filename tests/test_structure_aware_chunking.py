from __future__ import annotations

import json

from arxiv_archive.chunk_import_contract import validate_import_ready_package
from arxiv_archive.structure_aware_chunking import (
    RouteEligibility,
    SourceSpan,
    StructuralElement,
    StructureAwareChunk,
    empty_structure_aware_package,
)


def test_source_span_uses_normalized_markdown_coordinates() -> None:
    span = SourceSpan(char_start=3, char_end=12)

    assert span.to_contract() == {
        "coordinate_space": "normalized_markdown",
        "char_start": 3,
        "char_end": 12,
        "page_start": None,
        "page_end": None,
    }


def test_structure_aware_chunk_serializes_redacted_route_eligibility() -> None:
    chunk = StructureAwareChunk(
        chunk_id="p1:chunk-0001",
        paper_id="p1",
        chunk_type="retrieval_context",
        parent_element_ids=("p1:el-0001",),
        section_path=("Introduction",),
        order_index=1,
        source_span=SourceSpan(char_start=0, char_end=42),
        source_artifact="normalized_markdown:p1",
        route_eligibility=RouteEligibility(
            route="retrieval_only",
            state="ok_for_retrieval_only",
            allowed_uses=("retrieval_diagnostics", "review_only"),
            excluded_uses=("trusted_kg_import", "claim_extraction"),
            refusal_reasons=("skeleton_not_import_ready",),
        ),
    )

    record = chunk.to_contract()

    assert record["source_span"]["coordinate_space"] == "normalized_markdown"
    assert record["route"] == "retrieval_only"
    assert record["state"] == "ok_for_retrieval_only"
    assert record["redaction"] == {
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
    }
    serialized = json.dumps(record)
    assert "This bounded snippet" not in serialized
    assert "[0.1, 0.2]" not in serialized


def test_empty_structure_aware_package_is_valid_but_not_import_ready() -> None:
    package = empty_structure_aware_package(
        paper_id="p1",
        title="Example",
        markdown_length=128,
        source_artifact="normalized_markdown:p1",
    ).to_contract()
    validation = validate_import_ready_package(package)

    assert validation.valid_package is True
    assert validation.import_ready is False
    assert validation.import_eligible_chunk_count == 0
    assert package["diagnostics"]["raw_text_included"] is False
    assert package["diagnostics"]["embeddings_included"] is False
    assert package["diagnostics"]["ladybugdb_written"] is False
    assert package["diagnostics"]["production_import_attempted"] is False
    assert package["elements"][0]["source_span"] == {
        "coordinate_space": "normalized_markdown",
        "char_start": 0,
        "char_end": 128,
        "page_start": None,
        "page_end": None,
    }
    assert "Example" in package["paper"]["title"]
    assert "This is the paper body" not in json.dumps(package)


def test_structural_element_preserves_hierarchy_without_text() -> None:
    element = StructuralElement(
        element_id="p1:section:introduction",
        paper_id="p1",
        element_type="section",
        parent_element_id="p1:document",
        section_path=("Introduction",),
        order_index=1,
        source_span=SourceSpan(char_start=10, char_end=80),
        warning_codes=("heading_depth_inferred",),
    )

    record = element.to_contract()

    assert record["parent_element_id"] == "p1:document"
    assert record["section_path"] == ["Introduction"]
    assert record["warnings"][0]["code"] == "heading_depth_inferred"
    serialized = json.dumps(record)
    assert "text" not in serialized
    assert "embedding" not in serialized
