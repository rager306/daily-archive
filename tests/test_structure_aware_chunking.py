from __future__ import annotations

import json

from arxiv_archive.chunk_import_contract import validate_import_ready_package
from arxiv_archive.structure_aware_chunking import (
    RouteEligibility,
    SourceSpan,
    StructuralElement,
    StructureAwareChunk,
    empty_structure_aware_package,
    parse_markdown_structure,
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


def test_parse_markdown_structure_preserves_absolute_spans_and_hierarchy() -> None:
    markdown = "# Title\n\n## Method\n\nThe method paragraph keeps provenance.\n\n## References\n\n[1] Example citation.\n"

    package = parse_markdown_structure(
        markdown,
        paper_id="p1",
        title="Example",
        source_artifact="normalized_markdown:p1",
    )
    contract = package.to_contract()
    elements = contract["elements"]
    method = next(element for element in elements if element["section_path"] == ["Title", "Method"] and element["element_type"] == "section")
    paragraph = next(element for element in elements if element["element_type"] == "paragraph")
    reference = next(element for element in elements if element["element_type"] == "reference_entry")

    assert contract["paper"]["source_artifacts"] == ["normalized_markdown:p1"]
    assert method["parent_element_id"].startswith("p1:0001:section:title")
    assert paragraph["parent_element_id"] == method["element_id"]
    assert markdown[paragraph["source_span"]["char_start"] : paragraph["source_span"]["char_end"]].strip() == (
        "The method paragraph keeps provenance."
    )
    assert reference["section_path"] == ["Title", "References"]
    assert reference["source_span"]["coordinate_space"] == "normalized_markdown"
    assert validate_import_ready_package(contract).valid_package is True
    serialized = json.dumps(contract)
    assert "The method paragraph keeps provenance" not in serialized
    assert "Example citation" not in serialized


def test_parse_markdown_structure_detects_tables_figures_equations_and_administrative_blocks() -> None:
    markdown = (
        "# Paper\n\n"
        "ORCID: 0000-0000-0000-0000\n\n"
        "## Results\n\n"
        "| Model | Score |\n|---|---|\n| A | 1.0 |\n\n"
        "Figure 1: Accuracy by model.\n\n"
        "x = y + z\n"
    )

    package = parse_markdown_structure(
        markdown,
        paper_id="p2",
        title="Kinds",
        source_artifact="normalized_markdown:p2",
    )
    elements = package.to_contract()["elements"]
    types = [element["element_type"] for element in elements]

    assert "administrative" in types
    assert "table" in types
    assert "figure_caption" in types
    assert "equation" in types
    for element in elements:
        span = element["source_span"]
        assert 0 <= span["char_start"] <= span["char_end"] <= len(markdown)
    assert "ORCID" not in json.dumps(package.to_contract())


def test_parse_landing_markdown_keeps_navigation_as_administrative_or_sections() -> None:
    markdown = "# Computer Science > Artificial Intelligence\n\n## Submission history\n\n## Access Paper:\n"

    package = parse_markdown_structure(
        markdown,
        paper_id="p3",
        title="Landing",
        source_artifact="normalized_markdown:p3",
    )
    elements = package.to_contract()["elements"]

    assert elements[0]["element_type"] == "document"
    assert [element["element_type"] for element in elements].count("administrative") == 3
    assert all(element["source_span"]["coordinate_space"] == "normalized_markdown" for element in elements)
    assert all("Access Paper" not in element["section_path"] for element in elements)
