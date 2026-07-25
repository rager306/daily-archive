"""Property hardening for metadata-only Article PageIndex manifests."""

from __future__ import annotations

import json
from copy import deepcopy
from typing import Any

from adaptix import Retort
from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.infrastructure.papers.artifacts.models import (
    REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION,
)
from research_graph.infrastructure.papers.indexing.page_index import (
    ALLOWED_PAGE_INDEX_COORDINATE_SPACES,
    ALLOWED_PAGE_INDEX_SECTION_TYPES,
    ARTICLE_PAGE_INDEX_SCHEMA_VERSION,
    ArticlePageIndexAnchor,
    ArticlePageIndexDiagnostic,
    ArticlePageIndexNode,
    ArticlePageIndexSourceSpan,
    build_article_page_index_from_structure,
    node_by_id,
    path_to,
    to_json,
    validate_article_page_index,
    walk_next,
)

PAGE_INDEX_RETORT = Retort()
HEX64 = st.text(alphabet="0123456789abcdef", min_size=64, max_size=64)
SAFE_SLUG = st.from_regex(r"[a-z][a-z0-9]{2,10}", fullmatch=True)
PAPER_IDS = st.from_regex(r"property-paper-[a-f0-9]{4,10}", fullmatch=True)
FORBIDDEN_PAYLOAD_KEYS = st.sampled_from(
    [
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
        "section_text",
        "caption_text",
        "table_text",
        "equation_text",
        "model_output",
        "raw_model_output",
        "raw_minimax_response",
        "base64",
        "binary",
        "bytes",
        "image_bytes",
        "payload",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "secrets",
        "token",
        "tokens",
        "api_key",
        "credentials",
        "optimizer_trace",
        "optimizer_traces",
    ]
)
FORBIDDEN_SENTINELS = (
    "FORBIDDEN_SENTINEL_DO_NOT_ECHO",
    "```raw snippet```",
    "sk-test-secret",
    "embedding=[0.1, 0.2]",
    "vector=[0.3, 0.4]",
)
UNSAFE_FLAGS = st.sampled_from(
    [
        "raw_text_included",
        "raw_binary_included",
        "base64_included",
        "model_outputs_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "optimizer_traces_included",
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
        "import_eligible",
        "promoted_to_fact",
    ]
)


def _base_structure(
    paper_id: str, section_slugs: list[str], *, include_artifacts: bool = True
) -> dict[str, Any]:
    root_id = f"{paper_id}:section:root"
    child_slugs = [slug for slug in section_slugs if slug != "root"]
    sections = [
        {
            "section_id": root_id,
            "parent_section_id": None,
            "section_type": "root",
            "ordinal_path": [],
            "span_id": f"{paper_id}:span:section-root",
        }
    ]
    spans = [
        {
            "span_id": f"{paper_id}:span:section-root",
            "source_id": f"{paper_id}:source:normalized-md",
            "coordinate_space": "normalized_markdown_char",
            "char_start": 0,
            "char_end": 10,
            "span_hash": "0" * 64,
            "raw_text_embedded": False,
        }
    ]
    for index, slug in enumerate(child_slugs, start=1):
        sections.append(
            {
                "section_id": f"{paper_id}:section:{slug}",
                "parent_section_id": root_id,  # pyrefly: ignore[bad-assignment]
                "section_type": "methods" if index % 2 else "results",
                "ordinal_path": [index],
                "span_id": f"{paper_id}:span:section-{slug}",
            }
        )
        spans.append(
            {
                "span_id": f"{paper_id}:span:section-{slug}",
                "source_id": f"{paper_id}:source:normalized-md",
                "coordinate_space": "normalized_markdown_char",
                "char_start": index * 100,
                "char_end": index * 100 + 25,
                "span_hash": f"{index % 16:x}" * 64,  # pyrefly: ignore[bad-assignment]
                "raw_text_embedded": False,
            }
        )

    artifact_placeholders: list[dict[str, Any]] = []
    if include_artifacts and child_slugs:
        artifact_slug = child_slugs[0]
        artifact_placeholders.append(
            {
                "artifact_id": f"{paper_id}:artifact:figure:0001",
                "artifact_type": "figure",
                "section_id": f"{paper_id}:section:{artifact_slug}",
                "span_id": f"{paper_id}:span:figure-0001",
                "caption_span_id": f"{paper_id}:span:caption-figure-0001",
                "candidate_link_targets": [f"{paper_id}:artifact:claim:0001"],
            }
        )
        spans.extend(
            [
                {
                    "span_id": f"{paper_id}:span:figure-0001",
                    "source_id": f"{paper_id}:source:normalized-md",
                    "coordinate_space": "page_bbox",
                    "page_start": 1,
                    "page_end": 1,
                    "bbox": [0.1, 0.2, 0.8, 0.6],  # pyrefly: ignore[bad-assignment]
                    "span_hash": "a" * 64,
                    "raw_text_embedded": False,
                },
                {
                    "span_id": f"{paper_id}:span:caption-figure-0001",
                    "source_id": f"{paper_id}:source:normalized-md",
                    "coordinate_space": "normalized_markdown_char",
                    "char_start": 900,
                    "char_end": 940,
                    "span_hash": "b" * 64,
                    "raw_text_embedded": False,
                },
            ]
        )

    return {
        "schema_version": REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION,
        "paper_id": paper_id,
        "source_refs": [
            {
                "source_id": f"{paper_id}:source:normalized-md",
                "paper_id": paper_id,
                "source_role": "normalized_markdown",
                "source_path": "fixtures/property/redacted-structure.json",
                "sha256": "1" * 64,
                "media_type": "application/json",
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
            }
        ],
        "sections": sections,
        "artifact_placeholders": artifact_placeholders,
        "safe_spans": spans,
        "safety_flags": {
            "raw_text_included": False,
            "raw_binary_included": False,
            "base64_included": False,
            "model_outputs_included": False,
            "embeddings_included": False,
            "vectors_included": False,
            "secrets_included": False,
            "optimizer_traces_included": False,
            "trusted_kg_import_allowed": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
        },
    }


def structure_strategy(*, include_artifacts: bool = True) -> st.SearchStrategy[dict[str, Any]]:
    return st.builds(
        _base_structure,
        PAPER_IDS,
        st.lists(SAFE_SLUG, min_size=1, max_size=5, unique=True).map(
            lambda slugs: ["root", *slugs]
        ),
        include_artifacts=st.just(include_artifacts),
    )


FORBIDDEN_PAYLOAD_KEYS_STRINGS = (
    "text",
    "raw_text",
    "chunk_text",
    "paper_text",
    "claim_text",
    "section_text",
    "caption_text",
    "table_text",
    "equation_text",
    "model_output",
    "raw_model_output",
    "raw_minimax_response",
    "base64",
    "binary",
    "bytes",
    "image_bytes",
    "payload",
    "embedding",
    "embeddings",
    "vector",
    "vectors",
    "secret",
    "secrets",
    "token",
    "tokens",
    "api_key",
    "credentials",
    "optimizer_trace",
    "optimizer_traces",
)


def _assert_metadata_only(payload: dict[str, Any] | list[dict[str, Any]]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for fragment in (f'"{key}":' for key in FORBIDDEN_PAYLOAD_KEYS_STRINGS):
        assert fragment not in serialized
    for sentinel in FORBIDDEN_SENTINELS:
        assert sentinel not in serialized
    assert "```" not in serialized
    assert '"trusted_kg_import_allowed": true' not in serialized
    assert '"ladybugdb_written": true' not in serialized
    assert '"production_import_attempted": true' not in serialized
    assert '"import_eligible": true' not in serialized
    assert '"promoted_to_fact": true' not in serialized


def _diagnostic_codes(page_index: dict[str, Any]) -> set[str]:
    return {diagnostic["code"] for diagnostic in page_index.get("diagnostics", [])}


# --- Property: dataclass/adaptix and deterministic serialization roundtrips ---


@settings(max_examples=60)
@given(
    span_hash=HEX64, coordinate_space=st.sampled_from(sorted(ALLOWED_PAGE_INDEX_COORDINATE_SPACES))
)
def test_source_span_adaptix_roundtrip_preserves_redacted_shape(
    span_hash: str, coordinate_space: str
) -> None:
    span = ArticlePageIndexSourceSpan(
        span_id="property:span:0001",
        source_id="property:source:normalized-md",
        coordinate_space=coordinate_space,
        char_start=0 if coordinate_space != "page_bbox" else None,
        char_end=10 if coordinate_space != "page_bbox" else None,
        page_start=1 if coordinate_space == "page_bbox" else None,
        page_end=1 if coordinate_space == "page_bbox" else None,
        bbox=(0.1, 0.2, 0.8, 0.6) if coordinate_space == "page_bbox" else None,
        span_hash=span_hash,
    )

    restored = PAGE_INDEX_RETORT.load(PAGE_INDEX_RETORT.dump(span), ArticlePageIndexSourceSpan)

    assert restored == span
    assert restored.to_redacted_dict() == span.to_redacted_dict()
    _assert_metadata_only(restored.to_redacted_dict())


@settings(max_examples=60)
@given(code=SAFE_SLUG, json_path=SAFE_SLUG.map(lambda value: f"/generated/{value}"))
def test_diagnostic_adaptix_roundtrip_preserves_stable_code_path_and_redaction(
    code: str, json_path: str
) -> None:
    diagnostic = ArticlePageIndexDiagnostic(
        code=code,
        json_path=json_path,
        severity="repair_required",
        object_id="property-object",
        blocks_import=True,
    )

    restored = PAGE_INDEX_RETORT.load(
        PAGE_INDEX_RETORT.dump(diagnostic), ArticlePageIndexDiagnostic
    )

    assert restored == diagnostic
    assert restored.to_redacted_dict() == diagnostic.to_redacted_dict()
    assert restored.to_redacted_dict()["blocks_import"] is True
    _assert_metadata_only({"diagnostics": [restored.to_redacted_dict()]})


@settings(max_examples=40)
@given(structure=structure_strategy())
def test_build_serialization_and_validation_are_deterministic(structure: dict[str, Any]) -> None:
    first = build_article_page_index_from_structure(deepcopy(structure))
    second = build_article_page_index_from_structure(deepcopy(structure))

    assert first == second
    assert first["schema_version"] == ARTICLE_PAGE_INDEX_SCHEMA_VERSION
    assert json.loads(to_json(first)) == first
    assert to_json(first) == to_json(second)
    assert validate_article_page_index(first) == []
    assert first["summary"]["node_count"] == len(first["nodes"])
    assert first["summary"]["anchor_count"] == len(first["anchors"])
    assert first["summary"]["import_eligible_count"] == 0
    assert first["import_eligible_count"] == 0
    assert first["promoted_to_fact_count"] == 0
    _assert_metadata_only(first)


@settings(max_examples=40)
@given(structure=structure_strategy())
def test_navigation_ids_paths_and_anchor_ids_follow_input_ordering_rules(
    structure: dict[str, Any],
) -> None:
    page_index = build_article_page_index_from_structure(structure)
    nodes = page_index["nodes"]
    anchors = page_index["anchors"]

    assert [node["order"] for node in nodes] == list(range(len(nodes)))
    assert [node["node_id"] for node in walk_next(page_index)] == [
        node["node_id"] for node in nodes
    ]
    assert len({node["node_id"] for node in nodes}) == len(nodes)
    assert len({anchor["anchor_id"] for anchor in anchors}) == len(anchors)
    for node in nodes:
        assert node_by_id(page_index, node["node_id"]) == node
        assert path_to(page_index, node["node_id"]) == node["path"]
        assert node["path"][-1] == node["node_id"]
        for anchor_id in node["anchor_ids"]:
            assert anchor_id in {anchor["anchor_id"] for anchor in anchors}
    for anchor in anchors:
        assert anchor["node_id"] in {node["node_id"] for node in nodes}
        assert anchor["anchor_id"].startswith(f"{page_index['paper_id']}:page-index-anchor:")
        assert anchor["import_eligible"] is False
        assert anchor["promoted_to_fact"] is False


# --- Property: fail-closed validation and redacted diagnostics under malformed inputs ---


def mutate_structure(
    structure: dict[str, Any], mutation: str, forbidden_key: str, unsafe_flag: str
) -> dict[str, Any]:
    mutated = deepcopy(structure)
    if mutation == "duplicate_section_id":
        mutated["sections"].append(dict(mutated["sections"][-1]))
    elif mutation == "missing_parent":
        mutated["sections"][-1]["parent_section_id"] = (
            f"{mutated['paper_id']}:section:missing-parent"
        )
    elif mutation == "missing_span":
        mutated["sections"][-1]["span_id"] = f"{mutated['paper_id']}:span:missing"
    elif mutation == "unsupported_section_type":
        mutated["sections"][-1]["section_type"] = "unsupported_appendix_type"
    elif mutation == "artifact_missing_section_parent":
        mutated.setdefault("artifact_placeholders", []).append(
            {
                "artifact_id": f"{mutated['paper_id']}:artifact:figure:9999",
                "artifact_type": "figure",
                "section_id": f"{mutated['paper_id']}:section:missing-artifact-parent",
                "span_id": f"{mutated['paper_id']}:span:missing-artifact-span",
            }
        )
    elif mutation == "forbidden_payload_key":
        mutated["sections"][-1][forbidden_key] = FORBIDDEN_SENTINELS[0]
    elif mutation == "unsafe_import_flag":
        mutated.setdefault("safety_flags", {})[unsafe_flag] = True
    elif mutation == "source_of_truth_claim":
        mutated["source_of_truth"] = "raw claim should not be trusted"
    elif mutation == "empty_sections":
        mutated["sections"] = []
        mutated["artifact_placeholders"] = []
        mutated["safe_spans"] = []
    else:  # pragma: no cover - keeps mutation list honest
        raise AssertionError(f"unknown mutation: {mutation}")
    return mutated


@settings(max_examples=120)
@given(
    structure=structure_strategy(),
    mutation=st.sampled_from(
        [
            "duplicate_section_id",
            "missing_parent",
            "missing_span",
            "unsupported_section_type",
            "artifact_missing_section_parent",
            "forbidden_payload_key",
            "unsafe_import_flag",
            "source_of_truth_claim",
            "empty_sections",
        ]
    ),
    forbidden_key=FORBIDDEN_PAYLOAD_KEYS,
    unsafe_flag=UNSAFE_FLAGS,
)
def test_mutated_structures_emit_stable_redacted_diagnostics_or_fallback(
    structure: dict[str, Any], mutation: str, forbidden_key: str, unsafe_flag: str
) -> None:
    page_index = build_article_page_index_from_structure(
        mutate_structure(structure, mutation, forbidden_key, unsafe_flag)
    )
    codes = _diagnostic_codes(page_index)

    if mutation == "empty_sections":
        assert codes == {"no_sections_fallback"}
        assert page_index["summary"]["fallback_count"] == 1
        assert page_index["summary"]["blocker_count"] == 0
    else:
        assert codes
        assert page_index["summary"]["blocker_count"] >= 1
    assert page_index["summary"]["import_eligible_count"] == 0
    assert page_index["import_eligible_count"] == 0
    assert page_index["production_import_attempted"] is False
    assert page_index["ladybugdb_written"] is False
    assert page_index["trusted_kg_import_allowed"] is False
    for diagnostic in page_index["diagnostics"]:
        assert str(diagnostic["json_path"]).startswith("/")
        assert "```" not in diagnostic["message"]
        assert FORBIDDEN_SENTINELS[0] not in json.dumps(diagnostic, sort_keys=True)
    _assert_metadata_only(page_index)


@settings(max_examples=80)
@given(
    page_index=structure_strategy().map(build_article_page_index_from_structure),
    unsafe_flag=UNSAFE_FLAGS,
)
def test_mutated_page_index_manifests_validate_fail_closed(
    page_index: dict[str, Any], unsafe_flag: str
) -> None:
    mutated = deepcopy(page_index)
    if unsafe_flag in {
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
    }:
        mutated[unsafe_flag] = True
    elif unsafe_flag == "import_eligible":
        mutated["nodes"][0]["import_eligible"] = True
    elif unsafe_flag == "promoted_to_fact":
        mutated["anchors"][0 if mutated["anchors"] else -1]["promoted_to_fact"] = (
            True if mutated["anchors"] else False
        )
        if not mutated["anchors"]:
            mutated["promoted_to_fact_count"] = 1
    else:
        mutated["bridge_subtree"]["trusted_kg_import_allowed"] = True

    diagnostics = validate_article_page_index(mutated)

    assert diagnostics
    assert all(diagnostic["blocks_import"] is True for diagnostic in diagnostics)
    assert all(str(diagnostic["json_path"]).startswith("/") for diagnostic in diagnostics)
    _assert_metadata_only({"diagnostics": diagnostics})


@settings(max_examples=40)
@given(
    section_type=st.text(min_size=1, max_size=24).filter(
        lambda value: value not in ALLOWED_PAGE_INDEX_SECTION_TYPES
    )
)
def test_unsupported_section_vocabularies_are_diagnostics_not_crashes(section_type: str) -> None:
    structure = _base_structure(
        "property-paper-vocab", ["root", "methods"], include_artifacts=False
    )
    structure["sections"][1]["section_type"] = section_type

    page_index = build_article_page_index_from_structure(structure)

    assert "unsupported_section_type" in _diagnostic_codes(page_index)
    assert page_index["nodes"][1]["summary"]["section_type"] == "unknown"
    assert page_index["summary"]["import_eligible_count"] == 0
    _assert_metadata_only(page_index)


@settings(max_examples=40)
@given(span_hash=HEX64)
def test_page_index_node_and_anchor_dataclasses_roundtrip_without_import_claims(
    span_hash: str,
) -> None:
    span = ArticlePageIndexSourceSpan(
        span_id="property:span:roundtrip",
        source_id="property:source:roundtrip",
        coordinate_space="normalized_markdown_char",
        char_start=1,
        char_end=2,
        span_hash=span_hash,
    )
    anchor = ArticlePageIndexAnchor(
        anchor_id="property-paper:page-index-anchor:section-methods",
        node_id="property-paper:page-index:section:methods",
        paper_id="property-paper",
        span_id=span.span_id,
        source_id=span.source_id,
        coordinate_space=span.coordinate_space,
        span_hash=span.span_hash,
        anchor_type="section",
    )
    node = ArticlePageIndexNode(
        node_id="property-paper:page-index:section:methods",
        paper_id="property-paper",
        node_type="section",
        source_id=span.source_id,
        parent_id=None,
        children_ids=(),
        next_id=None,
        path=("property-paper:page-index:section:methods",),
        order=0,
        summary={
            "section_id": "property-paper:section:methods",
            "section_type": "methods",
            "ordinal_path": [1],
        },
        source_ref_ids=("property:source:roundtrip",),
        source_span=span,
        anchor_ids=(anchor.anchor_id,),
    )

    restored_node = PAGE_INDEX_RETORT.load(PAGE_INDEX_RETORT.dump(node), ArticlePageIndexNode)
    restored_anchor = PAGE_INDEX_RETORT.load(PAGE_INDEX_RETORT.dump(anchor), ArticlePageIndexAnchor)

    assert restored_node == node
    assert restored_anchor == anchor
    assert restored_node.to_redacted_dict()["import_eligible"] is False
    assert restored_node.to_redacted_dict()["promoted_to_fact"] is False
    assert restored_anchor.to_redacted_dict()["import_eligible"] is False
    assert restored_anchor.to_redacted_dict()["promoted_to_fact"] is False
    _assert_metadata_only(
        {"node": restored_node.to_redacted_dict(), "anchor": restored_anchor.to_redacted_dict()}
    )
