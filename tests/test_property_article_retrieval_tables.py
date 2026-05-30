"""Property hardening for S06 retrieval/table benchmark manifests.

These tests intentionally stay metadata-only and bounded: generated examples use
small in-memory records, tracked fixture provenance, and CPU-only deterministic
serialization paths.  Failures should point to stable diagnostic codes/paths
rather than raw article/table payloads.
"""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

from hypothesis import given, settings
from hypothesis import strategies as st

from arxiv_archive.article_retrieval_tables import (
    ALLOWED_BENCHMARK_STATUSES,
    DIAGNOSTIC_COUNTER_KEYS,
    build_article_retrieval_table_manifest,
    summarize_article_retrieval_tables,
    to_json,
    to_redacted_dict,
    validate_article_retrieval_table_manifest,
)

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_retrieval_tables"
FORBIDDEN_KEYS = st.sampled_from(
    [
        "text",
        "raw_text",
        "caption_text",
        "table_text",
        "payload",
        "embedding",
        "vector",
        "api_key",
        "optimizer_trace",
    ]
)
FORBIDDEN_SNIPPETS = (
    "FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO",
    "FORBIDDEN_TABLE_TEXT_DO_NOT_ECHO",
    "FORBIDDEN_CAPTION_TEXT_DO_NOT_ECHO",
    "embedding=[",
    "vector=[",
    "api_key=",
    "token=",
)
SAFE_STATUSES = st.sampled_from(sorted(ALLOWED_BENCHMARK_STATUSES))
SCORES = st.sampled_from([0.0, 0.25, 0.5, 0.75, 1.0])
SECTION_TYPES = st.sampled_from(["abstract", "introduction", "methods", "results", "discussion", "conclusion"])


def _load_minimal_manifest() -> dict[str, Any]:
    return json.loads((FIXTURES_DIR / "minimal_manifest.json").read_text(encoding="utf-8"))


def _assert_metadata_only(payload: dict[str, Any]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for snippet in FORBIDDEN_SNIPPETS:
        assert snippet not in serialized
    for forbidden in ["raw_text", "caption_text", "table_text", "embedding", "vector", "api_key"]:
        assert f'"{forbidden}"' not in serialized


def _base_unit(index: int, *, score: float, status: str, section_type: str) -> dict[str, Any]:
    fixture = _load_minimal_manifest()
    nodes = fixture["page_index_refs"]["node_ids"]
    anchors = fixture["page_index_refs"]["anchor_ids"]
    source_id = fixture["source_refs"][0]["source_id"]
    node_index = index % len(nodes)
    return {
        "unit_id": f"fixture-paper-0001:retrieval-unit:property:{index:04d}",
        "unit_family": "section_retrieval_unit",
        "section_type": section_type,
        "page_index_node_id": nodes[node_index],
        "page_index_anchor_id": anchors[node_index],
        "source_ref_ids": [source_id],
        "source_span_ids": [f"fixture-paper-0001:span:property:{index:04d}"],
        "provenance_ref_ids": [fixture["links_dedup_refs"]["metadata_signal_ids"][0]],
        "ranking_features": {
            "section_rank": index + 1,
            "provenance_signal_count": 1,
            "page_index_anchor_count": 1,
            "asset_ref_count": 0,
            "stable_tiebreaker": f"fixture-paper-0001:retrieval-unit:property:{index:04d}",
        },
        "benchmark_score": score,
        "benchmark_status": status,
        "diagnostic_codes": [],
    }


def _base_table_candidate(index: int, *, score: float, status: str) -> dict[str, Any]:
    fixture = _load_minimal_manifest()
    source_id = fixture["source_refs"][1]["source_id"]
    return {
        "candidate_id": f"fixture-paper-0001:table-candidate:property:{index:04d}",
        "candidate_family": "asset_page_index_table_candidate",
        "asset_id": fixture["asset_refs"]["asset_ids"][0],
        "page_index_node_id": "fixture-paper-0001:page-index:artifact:table:0001",
        "page_index_anchor_id": "fixture-paper-0001:page-index-anchor:table-0001",
        "source_ref_ids": [source_id],
        "source_span_ids": [f"fixture-paper-0001:span:table-property:{index:04d}"],
        "transformation_plan": {
            "status": "review_only_not_executed",
            "operation": "metadata_only_table_candidate",
            "raw_table_cells_included": False,
            "caption_included": False,
        },
        "ranking_features": {
            "asset_ref_count": 1,
            "page_index_anchor_count": 1,
            "source_ref_count": 1,
            "stable_tiebreaker": f"fixture-paper-0001:table-candidate:property:{index:04d}",
        },
        "benchmark_score": score,
        "benchmark_status": status,
        "diagnostic_codes": [],
    }


def _build_manifest(retrieval_units: list[dict[str, Any]], table_candidates: list[dict[str, Any]]) -> dict[str, Any]:
    fixture = _load_minimal_manifest()
    return build_article_retrieval_table_manifest(
        paper_id=fixture["paper_id"],
        run_id="m024-s06-property-test",
        source_refs=fixture["source_refs"],
        page_index_refs=fixture["page_index_refs"],
        asset_refs=fixture["asset_refs"],
        links_dedup_refs=fixture["links_dedup_refs"],
        retrieval_units=retrieval_units,
        table_candidates=table_candidates,
        manifest_path=fixture["manifest_path"],
    )


@st.composite
def retrieval_records(draw: st.DrawFn, *, min_size: int = 1, max_size: int = 5) -> list[dict[str, Any]]:
    scores = draw(st.lists(SCORES, min_size=min_size, max_size=max_size))
    statuses = draw(st.lists(SAFE_STATUSES, min_size=len(scores), max_size=len(scores)))
    sections = draw(st.lists(SECTION_TYPES, min_size=len(scores), max_size=len(scores)))
    records = [_base_unit(index, score=score, status=status, section_type=section) for index, (score, status, section) in enumerate(zip(scores, statuses, sections, strict=True))]
    return draw(st.permutations(records).map(list))


@st.composite
def table_records(draw: st.DrawFn, *, max_size: int = 3) -> list[dict[str, Any]]:
    count = draw(st.integers(min_value=0, max_value=max_size))
    scores = draw(st.lists(SCORES, min_size=count, max_size=count))
    statuses = draw(st.lists(SAFE_STATUSES, min_size=count, max_size=count))
    records = [_base_table_candidate(index, score=score, status=status) for index, (score, status) in enumerate(zip(scores, statuses, strict=True))]
    return draw(st.permutations(records).map(list))


@settings(max_examples=60, deadline=None)
@given(units=retrieval_records(), tables=table_records())
def test_generated_manifests_serialize_deterministically_and_rank_by_score_then_id(
    units: list[dict[str, Any]], tables: list[dict[str, Any]]
) -> None:
    manifest = _build_manifest(units, tables)
    json_once = to_json(manifest)
    json_twice = to_json(json.loads(json_once))

    assert json_once == json_twice
    assert json.loads(json_once) == to_redacted_dict(manifest)
    assert validate_article_retrieval_table_manifest(manifest) == []
    assert [unit["unit_id"] for unit in manifest["retrieval_units"]] == [
        unit["unit_id"] for unit in sorted(units, key=lambda unit: (-float(unit["benchmark_score"]), unit["unit_id"]))
    ]
    assert [unit["rank"] for unit in manifest["retrieval_units"]] == list(range(1, len(units) + 1))
    assert [candidate["candidate_id"] for candidate in manifest["table_candidates"]] == [
        candidate["candidate_id"]
        for candidate in sorted(tables, key=lambda candidate: (-float(candidate["benchmark_score"]), candidate["candidate_id"]))
    ]
    assert manifest["summary"]["retrieval_unit_count"] == len(units)
    assert manifest["summary"]["table_candidate_count"] == len(tables)
    assert set(manifest["summary"]["diagnostic_counts"]) == set(DIAGNOSTIC_COUNTER_KEYS)
    assert manifest["summary"]["import_eligible_count"] == 0
    assert manifest["summary"]["promoted_to_fact_count"] == 0
    assert manifest["summary"]["ladybugdb_written_count"] == 0
    assert manifest["summary"]["production_import_attempted_count"] == 0
    assert manifest["summary"]["graph_readiness_count"] == 0
    _assert_metadata_only(manifest)


@settings(max_examples=40, deadline=None)
@given(status=SAFE_STATUSES)
def test_safe_status_vocabulary_preserves_review_only_counts_and_zero_import_counters(status: str) -> None:
    manifest = _build_manifest(
        [_base_unit(0, score=0.5, status=status, section_type="methods")],
        [_base_table_candidate(0, score=0.5, status=status)],
    )
    summary = summarize_article_retrieval_tables(manifest)

    assert validate_article_retrieval_table_manifest(manifest) == []
    assert manifest["retrieval_units"][0]["benchmark_status"] == status
    assert manifest["table_candidates"][0]["benchmark_status"] == status
    assert summary["included_review_only_count"] == (2 if status == "included_review_only" else 0)
    assert summary["blocked_count"] == (2 if status == "blocked_review_only" else 0)
    assert summary["repair_required_count"] == (2 if status == "repair_required_review_only" else 0)
    for key in ("import_eligible_count", "promoted_to_fact_count", "ladybugdb_written_count", "production_import_attempted_count", "graph_readiness_count"):
        assert summary[key] == 0


@settings(max_examples=50, deadline=None)
@given(forbidden_key=FORBIDDEN_KEYS, nested=st.booleans())
def test_forbidden_payload_keys_are_diagnostic_paths_and_redacted_from_output(
    forbidden_key: str, nested: bool
) -> None:
    manifest = _build_manifest(
        [_base_unit(0, score=0.5, status="included_review_only", section_type="methods")],
        [_base_table_candidate(0, score=0.5, status="included_review_only")],
    )
    raw = deepcopy(manifest)
    if nested:
        raw["retrieval_units"][0][forbidden_key] = "FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO"
        expected_path = f"$.retrieval_units[0].{forbidden_key}"
    else:
        raw[forbidden_key] = "FORBIDDEN_TABLE_TEXT_DO_NOT_ECHO"
        expected_path = f"$.{forbidden_key}"

    diagnostics = [diagnostic.to_redacted_dict() for diagnostic in validate_article_retrieval_table_manifest(raw)]
    redacted = to_redacted_dict(raw)

    assert "forbidden_payload_key" in {diagnostic["code"] for diagnostic in diagnostics}
    assert expected_path in {diagnostic["json_path"] for diagnostic in diagnostics}
    assert all(diagnostic["blocks_import"] is True for diagnostic in diagnostics)
    _assert_metadata_only({"diagnostics": diagnostics})
    _assert_metadata_only(redacted)


@settings(max_examples=40, deadline=None)
@given(flag=st.sampled_from(["trusted_kg_import_allowed", "ladybugdb_written", "production_import_attempted", "import_eligible", "promoted_to_fact"]))
def test_unsafe_true_flags_are_rejected_but_redaction_clamps_to_fixed_zero_import_contract(
    flag: str,
) -> None:
    manifest = _build_manifest(
        [_base_unit(0, score=0.5, status="included_review_only", section_type="methods")],
        [_base_table_candidate(0, score=0.5, status="included_review_only")],
    )
    raw = deepcopy(manifest)
    if flag in {"import_eligible", "promoted_to_fact"}:
        raw["retrieval_units"][0][flag] = True
        expected_path = f"$.retrieval_units[0].{flag}"
    else:
        raw["bridge_subtree"][flag] = True
        expected_path = f"$.bridge_subtree.{flag}"

    diagnostics = [diagnostic.to_redacted_dict() for diagnostic in validate_article_retrieval_table_manifest(raw)]
    redacted = to_redacted_dict(raw)

    assert "unsafe_authorization" in {diagnostic["code"] for diagnostic in diagnostics}
    assert expected_path in {diagnostic["json_path"] for diagnostic in diagnostics}
    assert redacted["summary"]["import_eligible_count"] == 0
    assert redacted["summary"]["promoted_to_fact_count"] == 0
    assert redacted["summary"]["ladybugdb_written_count"] == 0
    assert redacted["summary"]["production_import_attempted_count"] == 0
    assert redacted["bridge_subtree"]["trusted_kg_import_allowed"] is False
    assert redacted["bridge_subtree"]["ladybugdb_written"] is False
    assert redacted["bridge_subtree"]["production_import_attempted"] is False
    _assert_metadata_only(redacted)


@settings(max_examples=30, deadline=None)
@given(raw_table_cells=st.booleans(), caption=st.booleans())
def test_table_candidate_transformation_plan_remains_metadata_only_or_reports_stable_diagnostics(
    raw_table_cells: bool, caption: bool
) -> None:
    candidate = _base_table_candidate(0, score=0.75, status="included_review_only")
    candidate["transformation_plan"]["raw_table_cells_included"] = raw_table_cells
    candidate["transformation_plan"]["caption_included"] = caption
    manifest = _build_manifest(
        [_base_unit(0, score=0.5, status="included_review_only", section_type="results")],
        [candidate],
    )
    diagnostics = [diagnostic.to_redacted_dict() for diagnostic in validate_article_retrieval_table_manifest(manifest)]
    paths = {diagnostic["json_path"] for diagnostic in diagnostics}

    if raw_table_cells:
        assert "$.table_candidates[0].transformation_plan.raw_table_cells_included" in paths
    if caption:
        assert "$.table_candidates[0].transformation_plan.caption_included" in paths
    if not raw_table_cells and not caption:
        assert diagnostics == []
    assert manifest["table_candidates"][0]["raw_table_embedded"] is False
    assert manifest["table_candidates"][0]["caption_embedded"] is False
    assert manifest["table_candidates"][0]["embedding_included"] is False
    assert manifest["table_candidates"][0]["vector_included"] is False
    assert manifest["table_candidates"][0]["import_eligible"] is False
    assert manifest["table_candidates"][0]["promoted_to_fact"] is False
    _assert_metadata_only(manifest)
