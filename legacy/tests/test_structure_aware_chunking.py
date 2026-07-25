from __future__ import annotations

import json
from pathlib import Path

from research_graph.infrastructure.papers.chunking import (
    ChunkAnnotationSidecar,
    RouteEligibility,
    SourceSpan,
    StructuralElement,
    StructureAwareChunk,
    StructureAwarePackage,
    empty_structure_aware_package,
    measure_structure_aware_manifest,
    parse_markdown_structure,
    write_structure_aware_run,
)
from research_graph.infrastructure.repair.chunk_import_contract import validate_import_ready_package


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
    method = next(
        element
        for element in elements
        if element["section_path"] == ["Title", "Method"] and element["element_type"] == "section"
    )
    paragraph = next(element for element in elements if element["element_type"] == "paragraph")
    reference = next(
        element for element in elements if element["element_type"] == "reference_entry"
    )

    assert contract["paper"]["source_artifacts"] == ["normalized_markdown:p1"]
    assert method["parent_element_id"].startswith("p1:0001:section:title")
    assert paragraph["parent_element_id"] == method["element_id"]
    assert markdown[
        paragraph["source_span"]["char_start"] : paragraph["source_span"]["char_end"]
    ].strip() == ("The method paragraph keeps provenance.")
    assert reference["section_path"] == ["Title", "References"]
    assert reference["source_span"]["coordinate_space"] == "normalized_markdown"
    assert validate_import_ready_package(contract).valid_package is True
    serialized = json.dumps(contract)
    assert "The method paragraph keeps provenance" not in serialized
    assert "Example citation" not in serialized


def test_parse_markdown_structure_detects_tables_figures_equations_and_administrative_blocks() -> (
    None
):
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
    assert all(
        element["source_span"]["coordinate_space"] == "normalized_markdown" for element in elements
    )
    assert all("Access Paper" not in element["section_path"] for element in elements)


def test_parse_markdown_structure_assigns_conservative_routes_states_and_refusals() -> None:
    markdown = (
        "# Paper\n\n"
        "## Abstract\n\n"
        "Claim-like abstract prose.\n\n"
        "## Method\n\n"
        "Method prose.\n\n"
        "## References\n\n"
        "[1] Example.\n"
    )

    contract = parse_markdown_structure(
        markdown,
        paper_id="p4",
        title="Routes",
        source_artifact="normalized_markdown:p4",
    ).to_contract()
    chunks = contract["chunks"]
    diagnostics = contract["diagnostics"]

    assert any(
        chunk["route"] == "claim_extraction" and chunk["chunk_type"] == "claim_candidate"
        for chunk in chunks
    )
    assert any(
        chunk["route"] == "method_extraction" and chunk["chunk_type"] == "method_candidate"
        for chunk in chunks
    )
    assert any(
        chunk["route"] == "citation_graph" and chunk["chunk_type"] == "reference_entry"
        for chunk in chunks
    )
    assert all("trusted_kg_import" in chunk["excluded_uses"] for chunk in chunks)
    assert diagnostics["import_eligible_chunk_count"] == 0
    assert diagnostics["refused_chunk_count"] == len(chunks)
    assert diagnostics["counts_by_route"]["claim_extraction"] >= 1
    assert diagnostics["counts_by_route"]["method_extraction"] >= 1
    assert diagnostics["counts_by_route"]["citation_graph"] >= 1
    assert diagnostics["refusal_counts"]["claim_route_requires_review"] >= 1
    assert diagnostics["refusal_counts"]["method_route_requires_review"] >= 1
    assert diagnostics["refusal_counts"]["citation_route_requires_review"] >= 1
    validation = validate_import_ready_package(contract)
    assert validation.valid_package is True
    assert validation.import_ready is False


def test_parse_markdown_structure_routes_tables_and_administrative_metadata_without_import_permission() -> (
    None
):
    markdown = (
        "# Paper\n\n"
        "ORCID: 0000-0000-0000-0000\n\n"
        "## Results\n\n"
        "| Model | Score |\n|---|---|\n| A | 1.0 |\n"
    )

    contract = parse_markdown_structure(
        markdown,
        paper_id="p5",
        title="Tables",
        source_artifact="normalized_markdown:p5",
    ).to_contract()
    chunks = contract["chunks"]

    assert any(
        chunk["route"] == "table_extraction" and chunk["chunk_type"] == "table_context"
        for chunk in chunks
    )
    assert any(
        chunk["route"] == "metadata_graph" and chunk["chunk_type"] == "metadata" for chunk in chunks
    )
    assert all(chunk["state"] in {"repair_required", "ok_for_retrieval_only"} for chunk in chunks)
    assert all("trusted_kg_import" not in chunk["allowed_uses"] for chunk in chunks)
    assert contract["diagnostics"]["refusal_counts"]["table_route_requires_review"] == 1
    assert contract["diagnostics"]["refusal_counts"]["administrative_metadata_requires_review"] == 1
    assert validate_import_ready_package(contract).valid_package is True


def test_measure_structure_aware_manifest_writes_redacted_summary_and_diagnostics(
    tmp_path: Path,
) -> None:
    paper_dir = tmp_path / "p1"
    paper_dir.mkdir()
    (paper_dir / "full_text.md").write_text(
        "# Paper\n\n## Abstract\n\nClaim-like prose.\n", encoding="utf-8"
    )
    manifest = tmp_path / "manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "schema_version": "m005-gold-corpus-manifest.v1",
                "milestone": "M005-test",
                "papers": [
                    {
                        "paper_id": "p1",
                        "title": "Paper",
                        "categories": ["cs.AI"],
                        "source_artifacts": ["normalized_markdown:p1"],
                        "required_paths": [str(paper_dir)],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    out = tmp_path / "out"

    result = measure_structure_aware_manifest(manifest)
    write_structure_aware_run(result, out)

    summary = json.loads((out / "structure-aware-summary.json").read_text(encoding="utf-8"))
    records = (
        (out / "structure-aware-package-diagnostics.jsonl").read_text(encoding="utf-8").splitlines()
    )
    record = json.loads(records[0])
    assert summary["schema_version"] == "m005-structure-aware-run.v1"
    assert summary["paper_count"] == 1
    assert summary["valid_package_count"] == 1
    assert summary["import_ready_count"] == 0
    assert summary["raw_text_included"] is False
    assert summary["embeddings_included"] is False
    assert summary["ladybugdb_written"] is False
    assert summary["production_import_attempted"] is False
    assert record["schema_version"] == "m005-structure-aware-package-diagnostic.v1"
    assert record["counts_by_route"]["claim_extraction"] >= 1
    assert "Claim-like prose" not in json.dumps(summary)
    assert "Claim-like prose" not in json.dumps(record)


def test_written_structure_aware_diagnostics_include_redacted_chunk_level_evidence(
    tmp_path: Path,
) -> None:
    paper_dir = tmp_path / "p2"
    paper_dir.mkdir()
    (paper_dir / "full_text.md").write_text(
        "# Paper\n\n## Method\n\nMethod prose.\n", encoding="utf-8"
    )
    manifest = tmp_path / "manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "schema_version": "m005-gold-corpus-manifest.v1",
                "milestone": "M005-test",
                "papers": [
                    {
                        "paper_id": "p2",
                        "title": "Paper",
                        "categories": ["cs.AI"],
                        "source_artifacts": ["normalized_markdown:p2"],
                        "required_paths": [str(paper_dir / "full_text.md")],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    out = tmp_path / "out"

    write_structure_aware_run(measure_structure_aware_manifest(manifest), out)

    record = json.loads(
        (out / "structure-aware-package-diagnostics.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()[0]
    )
    chunk = record["chunk_diagnostics"][0]
    assert set(chunk) == {
        "chunk_id",
        "chunk_type",
        "route",
        "state",
        "source_span",
        "parent_element_ids",
        "section_path",
        "refusal_reasons",
    }
    assert chunk["source_span"]["coordinate_space"] == "normalized_markdown"
    assert chunk["parent_element_ids"]
    assert chunk["refusal_reasons"]
    assert record["source_span_coverage"] == 1.0
    assert record["parent_reference_resolution_rate"] == 1.0
    serialized = json.dumps(record)
    assert "Method prose" not in serialized
    assert "[0.1, 0.2]" not in serialized
    assert record["embeddings_included"] is False


def test_chunk_annotation_sidecar_serializes_redacted_non_fact_annotation() -> None:
    annotation = ChunkAnnotationSidecar(
        annotation_id="p1:ann:route:0001",
        paper_id="p1",
        chunk_id="p1:chunk-0001",
        method="deterministic_structure_metadata_v1",
        annotation_type="route_hint",
        values={"route": "claim_extraction", "source": "section_path"},
        confidence_class="deterministic",
        warning_codes=("review_required_before_import",),
    )

    record = annotation.to_contract()

    assert record["annotation_id"] == "p1:ann:route:0001"
    assert record["chunk_id"] == "p1:chunk-0001"
    assert record["promoted_to_fact"] is False
    assert record["redaction"] == {
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
    }
    assert record["warnings"][0]["code"] == "review_required_before_import"
    serialized = json.dumps(record)
    assert "raw_text" in serialized
    assert "This is the paper body" not in serialized
    assert "[0.1, 0.2]" not in serialized


def test_structure_aware_package_includes_annotation_sidecars_without_import_readiness() -> None:
    chunk = StructureAwareChunk(
        chunk_id="p1:chunk-0001",
        paper_id="p1",
        chunk_type="claim_candidate",
        parent_element_ids=("p1:el-0001",),
        section_path=("Abstract",),
        order_index=1,
        source_span=SourceSpan(char_start=0, char_end=42),
        source_artifact="normalized_markdown:p1",
        route_eligibility=RouteEligibility(
            route="claim_extraction",
            state="repair_required",
            allowed_uses=("routing_diagnostics", "review_only"),
            excluded_uses=("trusted_kg_import", "production_ladybugdb_write"),
            refusal_reasons=("claim_route_requires_review",),
        ),
        warning_codes=("claim_route_requires_review",),
    )
    element = StructuralElement(
        element_id="p1:el-0001",
        paper_id="p1",
        element_type="paragraph",
        section_path=("Abstract",),
        order_index=1,
        source_span=SourceSpan(char_start=0, char_end=42),
    )
    annotation = ChunkAnnotationSidecar(
        annotation_id="p1:ann:structural-type:0001",
        paper_id="p1",
        chunk_id=chunk.chunk_id,
        method="deterministic_structure_metadata_v1",
        annotation_type="structural_type",
        values={"chunk_type": "claim_candidate", "element_type": "paragraph"},
        confidence_class="deterministic",
    )

    package = StructureAwarePackage(
        paper_id="p1",
        title="Example",
        source_artifact="normalized_markdown:p1",
        elements=(element,),
        chunks=(chunk,),
        annotations=(annotation,),
    ).to_contract()
    validation = validate_import_ready_package(package)

    assert package["annotations"][0]["promoted_to_fact"] is False
    assert package["annotations"][0]["redaction"]["raw_text_included"] is False
    assert validation.valid_package is True
    assert validation.import_ready is False
    assert validation.import_eligible_chunk_count == 0


def test_parse_markdown_structure_generates_sidecars_from_chunk_metadata() -> None:
    markdown = (
        "# Paper\n\n"
        "## Results\n\n"
        "| Model | Score |\n|---|---|\n| A | 1.0 |\n\n"
        "Figure 1: Accuracy by model.\n"
    )

    contract = parse_markdown_structure(
        markdown,
        paper_id="p6",
        title="Annotations",
        source_artifact="normalized_markdown:p6",
    ).to_contract()
    annotations = contract["annotations"]
    diagnostics = contract["diagnostics"]

    assert annotations
    assert all(annotation["promoted_to_fact"] is False for annotation in annotations)
    assert all(annotation["redaction"]["raw_text_included"] is False for annotation in annotations)
    assert {annotation["annotation_type"] for annotation in annotations} >= {
        "section_role",
        "route_hint",
        "structural_type",
        "review_blocker",
        "asset_link_hint",
    }
    assert diagnostics["annotation_counts_by_type"]["section_role"] == len(contract["chunks"])
    assert diagnostics["annotation_counts_by_type"]["route_hint"] == len(contract["chunks"])
    assert diagnostics["annotation_counts_by_confidence"]["deterministic"] >= len(
        contract["chunks"]
    )
    assert diagnostics["annotation_warning_counts"]["asset_manifest_required"] == 2
    serialized = json.dumps(annotations)
    assert "Figure 1" not in serialized
    assert "Model" not in serialized


def test_measurement_records_include_annotation_diagnostics(tmp_path: Path) -> None:
    paper_dir = tmp_path / "p3"
    paper_dir.mkdir()
    (paper_dir / "full_text.md").write_text(
        "# Paper\n\n## Abstract\n\nClaim-like prose.\n", encoding="utf-8"
    )
    manifest = tmp_path / "manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "schema_version": "m005-gold-corpus-manifest.v1",
                "milestone": "M005-test",
                "papers": [
                    {
                        "paper_id": "p3",
                        "title": "Paper",
                        "categories": ["cs.AI"],
                        "source_artifacts": ["normalized_markdown:p3"],
                        "required_paths": [str(paper_dir / "full_text.md")],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    out = tmp_path / "out"

    write_structure_aware_run(measure_structure_aware_manifest(manifest), out)

    summary = json.loads((out / "structure-aware-summary.json").read_text(encoding="utf-8"))
    record = json.loads(
        (out / "structure-aware-package-diagnostics.jsonl")
        .read_text(encoding="utf-8")
        .splitlines()[0]
    )
    assert summary["annotation_count"] == record["annotation_count"]
    assert record["annotation_counts_by_type"]["section_role"] >= 1
    assert record["annotation_counts_by_type"]["route_hint"] >= 1
    assert record["annotation_counts_by_type"]["structural_type"] >= 1
    assert record["annotation_counts_by_type"]["review_blocker"] >= 1
    assert "Claim-like prose" not in json.dumps(record)


def test_annotation_contract_rejects_unresolved_chunk_reference() -> None:
    package = parse_markdown_structure(
        "# Paper\n\n## Abstract\n\nClaim-like prose.\n",
        paper_id="p7",
        title="Annotation boundary",
        source_artifact="normalized_markdown:p7",
    ).to_contract()
    package = json.loads(json.dumps(package))
    package["annotations"][0]["chunk_id"] = "p7:missing-chunk"

    validation = validate_import_ready_package(package)

    assert validation.valid_package is False
    assert "unresolved_annotation_chunk" in validation.refusal_counts
    assert all("Claim-like prose" not in diagnostic.reason for diagnostic in validation.diagnostics)


def test_annotation_contract_rejects_promoted_fact_sidecar() -> None:
    package = parse_markdown_structure(
        "# Paper\n\n## Method\n\nMethod prose.\n",
        paper_id="p8",
        title="Promoted annotation",
        source_artifact="normalized_markdown:p8",
    ).to_contract()
    package = json.loads(json.dumps(package))
    package["annotations"][0]["promoted_to_fact"] = True

    validation = validate_import_ready_package(package)

    assert validation.valid_package is False
    assert validation.refusal_counts["annotation_promoted_to_fact"] == 1
    assert validation.import_ready is False


def test_annotation_contract_rejects_nested_raw_text_leakage() -> None:
    leaked_value = "This raw sentence must never appear in diagnostics."
    package = parse_markdown_structure(
        "# Paper\n\n## Results\n\nResult prose.\n",
        paper_id="p9",
        title="Raw annotation",
        source_artifact="normalized_markdown:p9",
    ).to_contract()
    package = json.loads(json.dumps(package))
    package["annotations"][0]["values"]["raw_text"] = leaked_value

    validation = validate_import_ready_package(package)

    assert validation.valid_package is False
    assert validation.refusal_counts["raw_text_leakage"] >= 1
    serialized_diagnostics = json.dumps(
        [diagnostic.__dict__ for diagnostic in validation.diagnostics]
    )
    assert leaked_value not in serialized_diagnostics


def test_annotation_values_do_not_create_import_eligibility() -> None:
    package = parse_markdown_structure(
        "# Paper\n\n## Abstract\n\nClaim-like prose.\n",
        paper_id="p10",
        title="Annotation eligibility",
        source_artifact="normalized_markdown:p10",
    ).to_contract()
    package = json.loads(json.dumps(package))
    package["annotations"][0]["values"] = {
        "route": "claim_extraction",
        "state": "ok_for_graph",
        "allowed_uses": ["trusted_kg_import"],
        "note": "annotation metadata is not chunk authorization",
    }

    validation = validate_import_ready_package(package)

    assert validation.valid_package is True
    assert validation.import_eligible_chunk_count == 0
    assert validation.import_ready is False
    assert all("trusted_kg_import" not in chunk["allowed_uses"] for chunk in package["chunks"])
