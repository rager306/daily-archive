"""Property hardening for the M024 metadata-only article asset contract."""

from __future__ import annotations

import json
from copy import deepcopy
from typing import Any

from hypothesis import given, settings
from hypothesis import strategies as st

from research_graph.infrastructure.papers.assets import (
    ALLOWED_ASSET_TYPES,
    ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION,
    build_article_asset_manifest,
    summarize_article_assets,
    to_json,
    validate_article_asset_manifest,
)

PAPER_IDS = st.from_regex(r"26[0-9]{2}\.assets-[a-f0-9]{4}", fullmatch=True)
RUN_IDS = st.from_regex(r"m024-assets-property-[a-f0-9]{6}", fullmatch=True)
HEX64 = st.text(alphabet="0123456789abcdef", min_size=64, max_size=64)
ASSET_TYPES = st.sampled_from(sorted(ALLOWED_ASSET_TYPES))
PRESERVATION_STATES = st.sampled_from(
    ["placeholder_only", "source_linked", "linked_not_extracted", "blocked", "missing_source"]
)
INTERPRETATION_STATUSES = st.sampled_from(
    [
        "not_interpreted",
        "needs_human_review",
        "interpretation_deferred",
        "metadata_only",
        "review_required",
        "blocked",
    ]
)
FORBIDDEN_KEY_VALUES = st.sampled_from(
    [
        "caption_text",
        "table_text",
        "equation_text",
        "image_bytes",
        "base64",
        "embedding",
        "vector",
        "payload",
        "api_key",
    ]
)
FORBIDDEN_SENTINEL = "PROPERTY_FORBIDDEN_SENTINEL_DO_NOT_ECHO"
UNSAFE_FLAGS = (
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "import_eligible",
    "promoted_to_fact",
)


def source_ref(paper_id: str, source_index: int, sha256: str = "a" * 64) -> dict[str, Any]:
    return {
        "source_id": f"{paper_id}:source:{source_index:04d}",
        "paper_id": paper_id,
        "source_path": f"fixtures/articles/{paper_id}-{source_index:04d}.pdf",
        "source_type": "pdf",
        "source_role": "original_pdf",
        "media_type": "application/pdf",
        "sha256": sha256,
        "byte_size": 1024 + source_index,
        "parser_name": "pdf_metadata_loader",
        "loader_name": "article_loader",
        "load_outcome": "loaded_metadata_only",
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
    }


def source_span(
    paper_id: str, source_id: str, index: int, span_hash: str = "b" * 64
) -> dict[str, Any]:
    return {
        "span_id": f"{paper_id}:span:asset-{index:04d}",
        "source_id": source_id,
        "coordinate_space": "page_bbox",
        "char_start": None,
        "char_end": None,
        "page_start": index + 1,
        "page_end": index + 1,
        "bbox": [72.0, 120.0, 468.0, 360.0],
        "span_hash": span_hash,
        "raw_text_embedded": False,
    }


def page_index_for_placeholders(
    paper_id: str, placeholders: list[dict[str, Any]]
) -> dict[str, Any]:
    nodes: list[dict[str, Any]] = []
    anchors: list[dict[str, Any]] = []
    for placeholder in placeholders:
        node_id = str(placeholder.get("page_index_node_id"))
        anchor_id = str(placeholder.get("page_index_anchor_id"))
        if not node_id.startswith("missing-"):
            nodes.append(
                {
                    "node_id": node_id,
                    "paper_id": paper_id,
                    "node_type": "artifact",
                    "summary": {"artifact_type": placeholder.get("asset_type")},
                    "source_ref_ids": [placeholder.get("source_file_id")],
                    "source_span": placeholder.get("source_span"),
                    "anchor_ids": [anchor_id],
                    "import_eligible": False,
                    "promoted_to_fact": False,
                }
            )
        if not anchor_id.startswith("missing-"):
            anchors.append(
                {
                    "anchor_id": anchor_id,
                    "node_id": node_id,
                    "paper_id": paper_id,
                    "source_id": placeholder.get("source_file_id"),
                    "raw_text_embedded": False,
                    "import_eligible": False,
                    "promoted_to_fact": False,
                }
            )
    return {
        "schema_version": "m024-article-page-index.v1",
        "manifest_path": "artifacts/property-page-index.json",
        "manifest_sha256": "c" * 64,
        "nodes": nodes,
        "anchors": anchors,
        "bridge_subtree": {
            "status": "review_only_not_import_eligible",
            "trusted_kg_import_allowed": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
        },
    }


def placeholder(
    paper_id: str,
    index: int,
    *,
    source_id: str,
    asset_type: str,
    source_asset_ref: str | None = None,
    preservation_state: str = "placeholder_only",
    interpretation_status: str = "not_interpreted",
    include_source_ref: bool = True,
    include_page_ref: bool = True,
    include_anchor_ref: bool = True,
    include_span: bool = True,
) -> dict[str, Any]:
    ref = source_asset_ref or f"{asset_type}:{index + 1}"
    result: dict[str, Any] = {
        "source_asset_ref": ref,
        "asset_type": asset_type,
        "page_index_node_id": f"{paper_id}:page-index:artifact:{asset_type}:{index:04d}"
        if include_page_ref
        else f"missing-node-{index:04d}",
        "page_index_anchor_id": f"{paper_id}:page-index-anchor:{asset_type}:{index:04d}"
        if include_anchor_ref
        else f"missing-anchor-{index:04d}",
        "source_file_id": source_id if include_source_ref else f"missing-source-{index:04d}",
        "source_span_id": f"{paper_id}:span:asset-{index:04d}",
        "preservation_state": preservation_state,
        "interpretation_status": interpretation_status,
    }
    if include_span:
        result["source_span"] = source_span(paper_id, source_id, index)
    return result


def assert_metadata_only(payload: dict[str, Any]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    assert FORBIDDEN_SENTINEL not in serialized
    for key in (
        "caption_text",
        "table_text",
        "equation_text",
        "image_bytes",
        "base64",
        "embedding",
        "vector",
        "payload",
        "api_key",
    ):
        assert f'"{key}":' not in serialized
    for unsafe_fragment in (
        '"trusted_kg_import_allowed": true',
        '"ladybugdb_written": true',
        '"production_import_attempted": true',
        '"import_eligible": true',
        '"promoted_to_fact": true',
    ):
        assert unsafe_fragment not in serialized


def diagnostic_codes(manifest: dict[str, Any]) -> set[str]:
    return {str(diagnostic["code"]) for diagnostic in manifest.get("diagnostics", [])}


@settings(max_examples=60)
@given(paper_id=PAPER_IDS, run_id=RUN_IDS, asset_type=ASSET_TYPES, sha256=HEX64)
def test_single_asset_manifest_serialization_roundtrip_is_deterministic(
    paper_id: str, run_id: str, asset_type: str, sha256: str
) -> None:
    source = source_ref(paper_id, 1, sha256)
    item = placeholder(paper_id, 0, source_id=source["source_id"], asset_type=asset_type)
    payload = {
        "paper_id": paper_id,
        "run_id": run_id,
        "source_refs": [source],
        "page_index": page_index_for_placeholders(paper_id, [item]),
        "asset_placeholders": [item],
    }

    manifest = build_article_asset_manifest(payload)
    serialized_once = to_json(manifest)
    serialized_twice = to_json(json.loads(serialized_once))

    assert serialized_once == serialized_twice
    assert json.loads(serialized_once) == manifest
    assert validate_article_asset_manifest(manifest) == []
    assert manifest["schema_version"] == ARTICLE_ASSET_MANIFEST_SCHEMA_VERSION
    assert (
        manifest["assets"][0]["asset_id"] == f"{paper_id}:asset:{asset_type.replace('_', '-')}:0001"
    )
    assert manifest["summary"]["asset_count"] == 1
    assert manifest["summary"]["hash_coverage_rate"] == 1.0
    assert manifest["summary"]["page_index_anchor_coverage_rate"] == 1.0
    assert manifest["summary"]["source_span_coverage_rate"] == 1.0
    assert_metadata_only(manifest)


@settings(max_examples=60)
@given(paper_id=PAPER_IDS, run_id=RUN_IDS)
def test_empty_asset_lists_have_bounded_zero_coverage_and_no_blockers(
    paper_id: str, run_id: str
) -> None:
    manifest = build_article_asset_manifest(
        {
            "paper_id": paper_id,
            "run_id": run_id,
            "source_refs": [source_ref(paper_id, 1)],
            "page_index": page_index_for_placeholders(paper_id, []),
            "asset_placeholders": [],
        }
    )

    assert validate_article_asset_manifest(manifest) == []
    assert manifest["assets"] == []
    assert manifest["summary"]["asset_count"] == 0
    assert manifest["summary"]["hash_coverage_rate"] == 0.0
    assert manifest["summary"]["page_index_anchor_coverage_rate"] == 0.0
    assert manifest["summary"]["source_span_coverage_rate"] == 0.0
    assert manifest["summary"]["blocker_count"] == 0
    assert manifest["subtree"]["status"] == "review_only_not_import_eligible"
    assert_metadata_only(manifest)


@settings(max_examples=90)
@given(
    paper_id=PAPER_IDS,
    run_id=RUN_IDS,
    asset_type=ASSET_TYPES,
    preservation_state=PRESERVATION_STATES,
    interpretation_status=INTERPRETATION_STATUSES,
    missing_source=st.booleans(),
    missing_page=st.booleans(),
    missing_anchor=st.booleans(),
    missing_span=st.booleans(),
)
def test_generated_metadata_manifests_fail_closed_for_missing_or_blocked_records(
    paper_id: str,
    run_id: str,
    asset_type: str,
    preservation_state: str,
    interpretation_status: str,
    missing_source: bool,
    missing_page: bool,
    missing_anchor: bool,
    missing_span: bool,
) -> None:
    source = source_ref(paper_id, 1)
    item = placeholder(
        paper_id,
        0,
        source_id=source["source_id"],
        asset_type=asset_type,
        preservation_state=preservation_state,
        interpretation_status=interpretation_status,
        include_source_ref=not missing_source,
        include_page_ref=not missing_page,
        include_anchor_ref=not missing_anchor,
        include_span=not missing_span,
    )

    manifest = build_article_asset_manifest(
        {
            "paper_id": paper_id,
            "run_id": run_id,
            "source_refs": [source],
            "page_index": page_index_for_placeholders(paper_id, [item]),
            "asset_placeholders": [item],
        }
    )

    assert validate_article_asset_manifest(manifest) == []
    expected_blocked = missing_source or missing_page or missing_anchor or missing_span
    assert (manifest["summary"]["blocker_count"] > 0) is expected_blocked
    assert manifest["subtree"]["status"] == (
        "blocked" if expected_blocked else "review_only_not_import_eligible"
    )
    assert manifest["import_eligible_count"] == 0
    assert manifest["promoted_to_fact_count"] == 0
    assert manifest["summary"]["import_ineligible_count"] == manifest["summary"]["asset_count"]
    assert 0.0 <= manifest["summary"]["hash_coverage_rate"] <= 1.0
    assert 0.0 <= manifest["summary"]["page_index_anchor_coverage_rate"] <= 1.0
    assert 0.0 <= manifest["summary"]["source_span_coverage_rate"] <= 1.0
    assert_metadata_only(manifest)


@settings(max_examples=70)
@given(
    paper_id=PAPER_IDS,
    run_id=RUN_IDS,
    forbidden_key=FORBIDDEN_KEY_VALUES,
    unsafe_flag=st.sampled_from(UNSAFE_FLAGS),
)
def test_generated_forbidden_keys_and_unsafe_flags_are_redacted_and_fail_closed(
    paper_id: str, run_id: str, forbidden_key: str, unsafe_flag: str
) -> None:
    source = source_ref(paper_id, 1)
    source[forbidden_key] = FORBIDDEN_SENTINEL
    source[unsafe_flag] = True
    item = placeholder(paper_id, 0, source_id=source["source_id"], asset_type="figure")
    item[forbidden_key] = FORBIDDEN_SENTINEL
    item[unsafe_flag] = True

    manifest = build_article_asset_manifest(
        {
            "paper_id": paper_id,
            "run_id": run_id,
            "source_refs": [source],
            "page_index": page_index_for_placeholders(paper_id, [item]),
            "asset_placeholders": [item],
            "readiness_status": "ready_for_import",
        }
    )

    codes = diagnostic_codes(manifest)
    assert "forbidden_payload_key" in codes
    assert "unsafe_readiness_status" in codes
    assert manifest["summary"]["blocker_count"] >= 1
    assert manifest["subtree"]["status"] == "blocked"
    assert manifest["subtree"]["trusted_kg_import_allowed"] is False
    assert manifest["subtree"]["ladybugdb_written"] is False
    assert manifest["subtree"]["production_import_attempted"] is False
    assert validate_article_asset_manifest(manifest) == []
    assert_metadata_only(manifest)


@settings(max_examples=60)
@given(paper_id=PAPER_IDS, run_id=RUN_IDS, asset_type=ASSET_TYPES)
def test_duplicate_ids_invalid_vocabularies_and_malformed_hashes_return_stable_diagnostics(
    paper_id: str, run_id: str, asset_type: str
) -> None:
    source = source_ref(paper_id, 1, sha256="not-a-sha256")
    first = placeholder(
        paper_id,
        0,
        source_id=source["source_id"],
        asset_type=asset_type,
        source_asset_ref=f"{asset_type}:1",
    )
    second = deepcopy(first)
    second["interpretation_status"] = "interpreted_as_truth"
    second["preservation_state"] = "production_imported"
    second["source_span"] = source_span(paper_id, source["source_id"], 1, span_hash="bad-span-hash")

    manifest = build_article_asset_manifest(
        {
            "paper_id": paper_id,
            "run_id": run_id,
            "source_refs": [source],
            "page_index": page_index_for_placeholders(paper_id, [first, second]),
            "asset_placeholders": [first, second],
        }
    )

    assert {
        "duplicate_asset_id",
        "malformed_sha256",
        "malformed_span_hash",
        "invalid_interpretation_status",
        "invalid_preservation_state",
    } <= diagnostic_codes(manifest)
    assert manifest["summary"]["blocker_count"] >= 5
    assert manifest["subtree"]["status"] == "blocked"
    validation_codes = {
        str(diagnostic["code"]) for diagnostic in validate_article_asset_manifest(manifest)
    }
    assert {
        "duplicate_asset_id",
        "malformed_sha256",
        "invalid_interpretation_status",
        "invalid_preservation_state",
    } <= validation_codes
    assert_metadata_only(manifest)


@settings(max_examples=80)
@given(
    assets=st.lists(
        st.fixed_dictionaries(
            {
                "asset_type": ASSET_TYPES,
                "source_sha256": st.one_of(HEX64, st.none(), st.just("bad")),
                "page_index_anchor_id": st.one_of(
                    st.from_regex(r"anchor:[a-f0-9]{4}", fullmatch=True), st.none(), st.just("")
                ),
                "source_span": st.one_of(
                    st.none(),
                    st.fixed_dictionaries(
                        {"span_id": st.from_regex(r"span:[a-f0-9]{4}", fullmatch=True)}
                    ),
                    st.just({}),
                ),
            }
        ),
        min_size=0,
        max_size=8,
    )
)
def test_summary_coverage_rates_are_bounded_for_generated_asset_lists(
    assets: list[dict[str, Any]],
) -> None:
    diagnostics = [
        {"code": "generated_blocker", "blocks_import": True},
        {"code": "generated_warning", "blocks_import": False},
    ]

    summary = summarize_article_assets(assets, source_refs=[], diagnostics=diagnostics)

    assert summary["asset_count"] == len(assets)
    assert 0.0 <= summary["hash_coverage_rate"] <= 1.0
    assert 0.0 <= summary["page_index_anchor_coverage_rate"] <= 1.0
    assert 0.0 <= summary["source_span_coverage_rate"] <= 1.0
    assert summary["blocker_count"] == 1
    assert summary["import_ineligible_count"] == len(assets)
    if not assets:
        assert summary["hash_coverage_rate"] == 0.0
        assert summary["page_index_anchor_coverage_rate"] == 0.0
        assert summary["source_span_coverage_rate"] == 0.0
