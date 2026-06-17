"""Contract tests for metadata-only article PageIndex navigation.

S03 defines a new PageIndex surface over redacted typed article structures. The
contract is intentionally JSON-native and metadata-only: it preserves stable
section/artifact IDs, source span hashes, and navigation anchors without carrying
article prose, captions, equations, binary payloads, vectors, embeddings, model
output, secrets, or graph-import eligibility.
"""

from __future__ import annotations

import importlib
import json
from copy import deepcopy
from pathlib import Path

import pytest

ARTICLE_FIXTURES = Path(__file__).parent / "fixtures" / "article_artifacts"
PAGE_INDEX_FIXTURES = Path(__file__).parent / "fixtures" / "article_page_index"
FORBIDDEN_SENTINEL = "FORBIDDEN_SENTINEL_DO_NOT_ECHO"



@pytest.fixture()
def page_index_contract():
    """Load the future article_page_index module without failing collection.

    T01 is intentionally test-first: until S03 implementation lands, the
    contract is reported as an expected failure instead of a pytest import
    collection error. Once the module exists, these tests execute normally.
    """

    try:
        return importlib.import_module("research_graph.papers.indexing.page_index")
    except ModuleNotFoundError as exc:
        if exc.name == "research_graph.papers.indexing.page_index":
            pytest.xfail("research_graph.papers.indexing.page_index is not implemented yet")
        raise

FORBIDDEN_PAYLOAD_FRAGMENTS = (
    '"text":',
    '"raw_text":',
    '"chunk_text":',
    '"paper_text":',
    '"claim_text":',
    '"section_text":',
    '"caption_text":',
    '"table_text":',
    '"equation_text":',
    '"model_output":',
    '"raw_model_output":',
    '"raw_minimax_response":',
    '"base64":',
    '"binary":',
    '"bytes":',
    '"image_bytes":',
    '"payload":',
    '"embedding":',
    '"embeddings":',
    '"vector":',
    '"vectors":',
    '"secret":',
    '"secrets":',
    '"token":',
    '"tokens":',
    '"api_key":',
    '"credentials":',
    '"optimizer_trace":',
    '"optimizer_traces":',
    '"source_of_truth":',
    '"trusted_kg_import_allowed": true',
    '"ladybugdb_written": true',
    '"production_import_attempted": true',
    '"import_eligible": true',
    '"promoted_to_fact": true',
)


def _load_fixture(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def _basic_structure() -> dict:
    return _load_fixture(ARTICLE_FIXTURES / "basic_article_structure.json")


def _malformed_structure() -> dict:
    return _load_fixture(PAGE_INDEX_FIXTURES / "malformed_structure.json")


def _assert_metadata_only(payload: dict | list[dict]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for fragment in FORBIDDEN_PAYLOAD_FRAGMENTS:
        assert fragment not in serialized
    assert FORBIDDEN_SENTINEL not in serialized


def _diagnostic_codes(page_index: dict) -> set[str]:
    return {diagnostic["code"] for diagnostic in page_index.get("diagnostics", [])}


def _diagnostic_paths(page_index: dict) -> set[str]:
    return {diagnostic["json_path"] for diagnostic in page_index.get("diagnostics", [])}


def test_builds_deterministic_article_page_index_from_redacted_structure(page_index_contract) -> None:
    page_index = page_index_contract.build_article_page_index_from_structure(_basic_structure())

    assert page_index_contract.validate_article_page_index(page_index) == []
    assert page_index["schema_version"] == page_index_contract.ARTICLE_PAGE_INDEX_SCHEMA_VERSION
    assert page_index["paper_id"] == "fixture-paper-0001"
    assert page_index["summary"] == {
        "node_count": 6,
        "anchor_count": 7,
        "missing_parent_count": 0,
        "missing_span_count": 0,
        "fallback_count": 0,
        "blocker_count": 0,
        "import_eligible_count": 0,
    }
    assert page_index["bridge_subtree"] == {
        "status": "review_only_not_import_eligible",
        "source_slice": "M024-0xjwh9/S02",
        "graph_import_claim": False,
        "trusted_kg_import_allowed": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }
    assert page_index["diagnostics"] == []
    assert page_index["import_eligible_count"] == 0
    assert page_index["promoted_to_fact_count"] == 0

    assert [node["node_id"] for node in page_index["nodes"]] == [
        "fixture-paper-0001:page-index:section:root",
        "fixture-paper-0001:page-index:section:methods",
        "fixture-paper-0001:page-index:artifact:equation:0001",
        "fixture-paper-0001:page-index:section:results",
        "fixture-paper-0001:page-index:artifact:figure:0001",
        "fixture-paper-0001:page-index:artifact:reference:0001",
    ]
    assert [anchor["anchor_id"] for anchor in page_index["anchors"]] == [
        "fixture-paper-0001:page-index-anchor:section-root",
        "fixture-paper-0001:page-index-anchor:section-methods",
        "fixture-paper-0001:page-index-anchor:equation-0001",
        "fixture-paper-0001:page-index-anchor:section-results",
        "fixture-paper-0001:page-index-anchor:figure-0001",
        "fixture-paper-0001:page-index-anchor:caption-figure-0001",
        "fixture-paper-0001:page-index-anchor:citation-0001",
    ]
    _assert_metadata_only(page_index)


def test_navigation_helpers_return_parent_child_next_and_paths(page_index_contract) -> None:
    page_index = page_index_contract.build_article_page_index_from_structure(_basic_structure())

    root = page_index_contract.node_by_id(page_index, "fixture-paper-0001:page-index:section:root")
    methods = page_index_contract.node_by_id(page_index, "fixture-paper-0001:page-index:section:methods")
    results = page_index_contract.node_by_id(page_index, "fixture-paper-0001:page-index:section:results")
    figure = page_index_contract.node_by_id(page_index, "fixture-paper-0001:page-index:artifact:figure:0001")

    assert root is not None
    assert methods is not None
    assert results is not None
    assert figure is not None
    assert root["parent_id"] is None
    assert root["children_ids"] == [
        "fixture-paper-0001:page-index:section:methods",
        "fixture-paper-0001:page-index:section:results",
    ]
    assert methods["parent_id"] == root["node_id"]
    assert figure["parent_id"] == results["node_id"]
    assert methods["next_id"] == "fixture-paper-0001:page-index:artifact:equation:0001"
    assert [node["node_id"] for node in page_index_contract.children_of(page_index, root["node_id"])] == root["children_ids"]
    assert page_index_contract.path_to(page_index, figure["node_id"]) == [
        "fixture-paper-0001:page-index:section:root",
        "fixture-paper-0001:page-index:section:results",
        "fixture-paper-0001:page-index:artifact:figure:0001",
    ]
    assert [node["node_id"] for node in page_index_contract.walk_next(page_index)] == [node["node_id"] for node in page_index["nodes"]]


def test_nodes_and_anchors_preserve_source_span_hash_provenance_and_section_vocabulary(page_index_contract) -> None:
    page_index = page_index_contract.build_article_page_index_from_structure(_basic_structure())
    section_nodes = [node for node in page_index["nodes"] if node["node_type"] == "section"]

    assert page_index_contract.ALLOWED_PAGE_INDEX_SECTION_TYPES == frozenset(
        {"root", "abstract", "introduction", "background", "methods", "results", "discussion", "conclusion", "appendix", "unknown"}
    )
    assert {node["summary"]["section_type"] for node in section_nodes} <= page_index_contract.ALLOWED_PAGE_INDEX_SECTION_TYPES

    methods = page_index_contract.node_by_id(page_index, "fixture-paper-0001:page-index:section:methods")
    assert methods is not None
    assert methods["source_ref_ids"] == ["fixture-paper-0001:source:normalized-md"]
    assert methods["source_span"] == {
        "span_id": "fixture-paper-0001:span:section-methods",
        "source_id": "fixture-paper-0001:source:normalized-md",
        "coordinate_space": "normalized_markdown_char",
        "char_start": 100,
        "char_end": 180,
        "page_start": None,
        "page_end": None,
        "bbox": None,
        "span_hash": "3333333333333333333333333333333333333333333333333333333333333333",
        "raw_text_embedded": False,
    }

    figure_anchors = [
        anchor for anchor in page_index["anchors"]
        if anchor["node_id"] == "fixture-paper-0001:page-index:artifact:figure:0001"
    ]
    assert [anchor["span_id"] for anchor in figure_anchors] == [
        "fixture-paper-0001:span:figure-0001",
        "fixture-paper-0001:span:caption-figure-0001",
    ]
    assert all(anchor["span_hash"] for anchor in figure_anchors)
    assert all(anchor["raw_text_embedded"] is False for anchor in figure_anchors)
    _assert_metadata_only(page_index["anchors"])


def test_malformed_structure_reports_stable_redacted_diagnostics(page_index_contract) -> None:
    page_index = page_index_contract.build_article_page_index_from_structure(_malformed_structure())
    diagnostics = page_index_contract.validate_article_page_index(page_index)
    combined_diagnostics = page_index["diagnostics"] + diagnostics
    diagnostic_payload = {"diagnostics": combined_diagnostics}
    codes = {diagnostic["code"] for diagnostic in combined_diagnostics}
    paths = {diagnostic["json_path"] for diagnostic in combined_diagnostics}

    assert {
        "duplicate_section_id",
        "missing_parent",
        "missing_span",
        "unsupported_section_type",
        "forbidden_payload_key",
        "unsafe_import_flag_true:trusted_kg_import_allowed",
        "artifact_missing_section_parent",
    } <= codes
    assert "/sections[2]/section_id" in paths
    assert "/sections[2]/parent_section_id" in paths
    assert "/sections[2]/span_id" in paths
    assert "/sections[2]/section_type" in paths
    assert "/artifact_placeholders[0]/caption_text" in paths
    assert "/safety_flags/trusted_kg_import_allowed" in paths
    assert page_index["summary"]["missing_parent_count"] == 2
    assert page_index["summary"]["missing_span_count"] == 2
    assert page_index["summary"]["blocker_count"] >= 1
    assert page_index["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(diagnostic_payload)


def test_empty_or_no_section_structure_creates_metadata_only_fallback_node(page_index_contract) -> None:
    structure = _basic_structure()
    structure["sections"] = []
    structure["artifact_placeholders"] = []
    structure["safe_spans"] = []

    page_index = page_index_contract.build_article_page_index_from_structure(structure)

    assert page_index_contract.validate_article_page_index(page_index) == []
    assert page_index["summary"] == {
        "node_count": 1,
        "anchor_count": 0,
        "missing_parent_count": 0,
        "missing_span_count": 0,
        "fallback_count": 1,
        "blocker_count": 0,
        "import_eligible_count": 0,
    }
    assert page_index["nodes"] == [
        {
            "node_id": "fixture-paper-0001:page-index:fallback:no-sections",
            "paper_id": "fixture-paper-0001",
            "node_type": "fallback",
            "source_id": None,
            "parent_id": None,
            "children_ids": [],
            "next_id": None,
            "path": ["fixture-paper-0001:page-index:fallback:no-sections"],
            "order": 0,
            "summary": {"fallback_reason": "no_sections"},
            "source_ref_ids": [],
            "source_span": None,
            "anchor_ids": [],
            "import_eligible": False,
            "promoted_to_fact": False,
        }
    ]
    assert _diagnostic_codes(page_index) == {"no_sections_fallback"}
    assert _diagnostic_paths(page_index) == {"/sections"}
    _assert_metadata_only(page_index)


@pytest.mark.parametrize(
    ("mutation", "expected_code", "expected_path"),
    [
        (
            lambda structure: structure["sections"][1].update(parent_section_id="fixture-paper-0001:section:missing"),
            "missing_parent",
            "/sections[1]/parent_section_id",
        ),
        (
            lambda structure: structure["sections"][1].update(span_id="fixture-paper-0001:span:missing"),
            "missing_span",
            "/sections[1]/span_id",
        ),
        (
            lambda structure: structure["sections"].append(deepcopy(structure["sections"][1])),
            "duplicate_section_id",
            "/sections[3]/section_id",
        ),
        (
            lambda structure: structure["sections"][1].update(section_type="unsupported_appendix_type"),
            "unsupported_section_type",
            "/sections[1]/section_type",
        ),
        (
            lambda structure: structure["artifact_placeholders"][0].update(caption_text=FORBIDDEN_SENTINEL),
            "forbidden_payload_key",
            "/artifact_placeholders[0]/caption_text",
        ),
        (
            lambda structure: structure["safety_flags"].update(production_import_attempted=True),
            "unsafe_import_flag_true:production_import_attempted",
            "/safety_flags/production_import_attempted",
        ),
    ],
)
def test_negative_structure_mutations_have_specific_redacted_diagnostics(page_index_contract, mutation, expected_code: str, expected_path: str) -> None:
    structure = _basic_structure()
    mutation(structure)

    page_index = page_index_contract.build_article_page_index_from_structure(structure)

    assert expected_code in _diagnostic_codes(page_index)
    assert expected_path in _diagnostic_paths(page_index)
    assert page_index["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(page_index)
