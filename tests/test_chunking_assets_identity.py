from __future__ import annotations

import json
from pathlib import Path

from research_graph.infrastructure.identity.canonicalization import canonical_source_id
from research_graph.papers.chunking.chunker import parse_markdown_structure
from research_graph.papers.source_assets.registry import (
    attach_annotation_asset_links,
    preserve_source_assets_for_paper,
    validate_source_asset_manifest,
)
from research_graph.staging.graph_candidates import (
    DEFAULT_ROUTE_SPECS,
    LocatorSource,
    build_candidate_locator_artifact,
    validate_candidate_locator_artifact,
)

PAPER_ID = "boundary-fixture"
FIXTURE_DIR = Path(__file__).parent / "fixtures" / "chunking"
FIXTURE_MARKDOWN = FIXTURE_DIR / "full_text.md"


def _annotation_diagnostics_from_chunks(contract: dict) -> dict:
    return {
        "paper_id": contract["paper_id"],
        "chunk_annotation_coverage": [
            {
                "chunk_id": chunk["chunk_id"],
                "chunk_type": chunk["chunk_type"],
                "route": chunk["route"],
                "state": chunk["state"],
                "annotation_types": [
                    annotation["annotation_type"]
                    for annotation in contract["annotations"]
                    if annotation["chunk_id"] == chunk["chunk_id"]
                ],
                "confidence_classes": [
                    annotation["confidence_class"]
                    for annotation in contract["annotations"]
                    if annotation["chunk_id"] == chunk["chunk_id"]
                ],
                "warning_codes": [
                    warning["code"]
                    for warning in chunk.get("quality_warnings", [])
                    if isinstance(warning, dict)
                ],
            }
            for chunk in contract["chunks"]
        ],
    }


def _structure_diagnostics_from_chunks(contract: dict) -> dict:
    return {
        "paper_id": contract["paper_id"],
        "chunk_diagnostics": [
            {
                "chunk_id": chunk["chunk_id"],
                "source_span": chunk["source_span"],
            }
            for chunk in contract["chunks"]
        ],
    }


def test_chunking_assets_identity_and_staging_boundaries_compose_on_fixture(tmp_path: Path) -> None:
    markdown = FIXTURE_MARKDOWN.read_text(encoding="utf-8")

    chunk_contract = parse_markdown_structure(
        markdown,
        paper_id=PAPER_ID,
        title="Boundary Fixture",
        source_artifact=str(FIXTURE_MARKDOWN),
    ).to_contract()

    source_manifest = preserve_source_assets_for_paper(
        {"paper_id": PAPER_ID, "required_paths": [str(FIXTURE_DIR)]},
        workspace_root=tmp_path / "asset-workspace",
    ).to_contract()
    annotation_path = tmp_path / "annotation-diagnostics.jsonl"
    annotation_path.write_text(
        json.dumps(_annotation_diagnostics_from_chunks(chunk_contract)) + "\n", encoding="utf-8"
    )
    structure_path = tmp_path / "structure-diagnostics.jsonl"
    structure_path.write_text(
        json.dumps(_structure_diagnostics_from_chunks(chunk_contract)) + "\n", encoding="utf-8"
    )

    linked_manifest = attach_annotation_asset_links(
        (source_manifest,),
        annotation_diagnostics_path=annotation_path,
        structure_diagnostics_path=structure_path,
    )[0]
    locator_artifact = build_candidate_locator_artifact(
        run_id="boundary-smoke",
        paper_id=PAPER_ID,
        sources=(
            LocatorSource(
                source_id=canonical_source_id(PAPER_ID),
                paper_id=PAPER_ID,
                source_path=FIXTURE_MARKDOWN,
            ),
        ),
    )

    chunk_diagnostics = chunk_contract["diagnostics"]
    asset_diagnostics = linked_manifest["diagnostics"]
    locator_summary = locator_artifact["summary"]

    assert len(chunk_contract["chunks"]) == chunk_diagnostics["refused_chunk_count"]
    assert chunk_diagnostics["source_span_coverage"] == 1.0
    assert chunk_diagnostics["counts_by_route"]["claim_extraction"] >= 1
    assert chunk_diagnostics["counts_by_route"]["method_extraction"] >= 1
    assert chunk_diagnostics["counts_by_route"]["table_extraction"] == 1
    assert chunk_diagnostics["counts_by_route"]["citation_graph"] == 1
    assert chunk_diagnostics["import_eligible_chunk_count"] == 0

    assert validate_source_asset_manifest(linked_manifest).valid_manifest is True
    assert asset_diagnostics["source_file_count"] == 1
    assert asset_diagnostics["hash_coverage_rate"] == 1.0
    assert asset_diagnostics["asset_counts_by_type"] == {"figure": 1, "reference": 1, "table": 1}
    assert asset_diagnostics["extraction_state_counts"] == {"linked_not_extracted": 3}
    assert linked_manifest["promoted_to_fact_count"] == 0
    assert all(asset["promoted_to_fact"] is False for asset in linked_manifest["assets"])
    assert all("trusted_kg_import" in asset["excluded_uses"] for asset in linked_manifest["assets"])

    assert validate_candidate_locator_artifact(locator_artifact) == []
    assert locator_summary["locator_count"] == len(DEFAULT_ROUTE_SPECS)
    assert locator_summary["source_count"] == 1
    assert locator_summary["import_eligible_count"] == 0
    assert locator_summary["promoted_to_fact_count"] == 0
    assert locator_summary["ambiguous_span_count"] >= 1
    assert any(
        "overlapping_signal_window" in locator["diagnostic_codes"]
        for locator in locator_artifact["locators"]
    )

    serialized_outputs = json.dumps(
        {
            "chunk_contract": chunk_contract,
            "linked_manifest": linked_manifest,
            "locator_artifact": locator_artifact,
        }
    )
    assert "We claim that boundary smoke coverage" not in serialized_outputs
    assert "Figure 1: Boundary handoff" not in serialized_outputs
    assert "embedding_generation" in serialized_outputs


def test_boundary_smoke_negative_paths_stay_diagnostic_and_non_importable(tmp_path: Path) -> None:
    missing_locator_artifact = build_candidate_locator_artifact(
        run_id="boundary-smoke-missing-source",
        paper_id="missing-paper",
        sources=(
            LocatorSource(
                source_id=canonical_source_id("missing-paper"),
                paper_id="missing-paper",
                source_path=tmp_path / "missing-full-text.md",
            ),
        ),
    )

    assert validate_candidate_locator_artifact(missing_locator_artifact) == []
    assert missing_locator_artifact["source_ledger"][0]["conversion_status"] == "blocked"
    assert missing_locator_artifact["summary"]["missing_span_count"] == len(DEFAULT_ROUTE_SPECS)
    assert missing_locator_artifact["summary"]["import_eligible_count"] == 0
    assert all(
        locator["state"] == "missing_span" for locator in missing_locator_artifact["locators"]
    )
    assert all(
        locator["import_eligible"] is False for locator in missing_locator_artifact["locators"]
    )

    markdown = FIXTURE_MARKDOWN.read_text(encoding="utf-8")
    chunk_contract = parse_markdown_structure(
        markdown,
        paper_id="negative-boundary",
        title="Negative Boundary",
        source_artifact=str(FIXTURE_MARKDOWN),
    ).to_contract()
    source_manifest = preserve_source_assets_for_paper(
        {"paper_id": "negative-boundary", "required_paths": [str(FIXTURE_DIR)]},
        workspace_root=tmp_path / "asset-workspace",
    ).to_contract()
    annotation_path = tmp_path / "annotation-diagnostics.jsonl"
    annotation_path.write_text(
        json.dumps(_annotation_diagnostics_from_chunks(chunk_contract)) + "\n", encoding="utf-8"
    )
    linked_manifest = attach_annotation_asset_links(
        (source_manifest,), annotation_diagnostics_path=annotation_path
    )[0]
    linked_manifest["assets"][0]["allowed_uses"].append("trusted_kg_import")

    validation = validate_source_asset_manifest(linked_manifest)

    assert validation.valid_manifest is False
    assert validation.refusal_counts["asset_allows_trusted_import"] == 1
    serialized_diagnostics = json.dumps(
        [diagnostic.__dict__ for diagnostic in validation.diagnostics]
    )
    assert "We claim that boundary smoke coverage" not in serialized_diagnostics
