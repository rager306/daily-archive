"""Contract tests for S07 10-document metadata-only batch validation."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

from research_graph.infrastructure.papers.artifacts import batch_validation as batch

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_batch_validation"

FORBIDDEN_SENTINELS = (
    "FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO",
    "FORBIDDEN_CHUNK_TEXT",
    "FORBIDDEN_TABLE_TEXT_DO_NOT_ECHO",
    "secret-token",
    "api_key=",
    "token=",
)
FORBIDDEN_EXACT_KEYS = {
    "text",
    "raw_text",
    "chunk",
    "chunks",
    "chunk_text",
    "paper_text",
    "caption_text",
    "table_text",
    "bytes",
    "base64",
    "embedding",
    "embeddings",
    "vector",
    "vectors",
    "token",
    "tokens",
    "secret",
    "secrets",
    "api_key",
    "credentials",
}
UNSAFE_TRUE_FRAGMENTS = (
    '"trusted_kg_import_allowed": true',
    '"ladybugdb_written": true',
    '"production_import_attempted": true',
    '"production_write_attempted": true',
    '"graph_import_claim": true',
    '"raw_payloads_included": true',
    '"embeddings_included": true',
    '"vectors_included": true',
    '"import_eligible": true',
    '"promoted_to_fact": true',
)
EXPECTED_DIAGNOSTIC_COUNTER_KEYS = set(batch.DIAGNOSTIC_COUNTER_KEYS)
EXPECTED_SUBTREES = set(batch.SUBTREE_NAMES)


def _load_fixture(name: str) -> dict[str, Any]:
    return json.loads((FIXTURES_DIR / name).read_text(encoding="utf-8"))


def _walk_keys(value: object) -> list[str]:
    if isinstance(value, dict):
        keys = list(value.keys())
        for child in value.values():
            keys.extend(_walk_keys(child))
        return keys
    if isinstance(value, list):
        keys: list[str] = []
        for child in value:
            keys.extend(_walk_keys(child))
        return keys
    return []


def _codes(report: dict[str, Any]) -> set[str]:
    return {str(diagnostic["code"]) for diagnostic in report["diagnostics"]}


def _paths(report: dict[str, Any]) -> set[str]:
    return {str(diagnostic["json_path"]) for diagnostic in report["diagnostics"]}


def _assert_metadata_only(payload: dict[str, Any]) -> None:
    serialized = json.dumps(payload, sort_keys=True)
    for sentinel in FORBIDDEN_SENTINELS:
        assert sentinel not in serialized
    for fragment in UNSAFE_TRUE_FRAGMENTS:
        assert fragment not in serialized
    assert not (set(_walk_keys(payload)) & FORBIDDEN_EXACT_KEYS)


def test_ten_document_report_is_deterministic_metadata_only_and_review_ready() -> None:
    manifest = _load_fixture("ten_document_manifest.json")

    report = batch.build_article_batch_validation_report(manifest)
    report_again = batch.build_article_batch_validation_report(deepcopy(manifest))

    assert batch.to_json(report) == batch.to_json(report_again)
    assert batch.report_fingerprint(report) == batch.report_fingerprint(report_again)
    assert report["schema_version"] == "m024-article-batch-validation.v1"
    assert report["expected_document_count"] == 10
    assert report["aggregate_diagnostics"] == {
        "document_count": 10,
        "ready_document_count": 10,
        "blocked_document_count": 0,
        "diagnostic_count": 0,
        "diagnostic_counts": dict.fromkeys(batch.DIAGNOSTIC_COUNTER_KEYS, 0),
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "ladybugdb_written_count": 0,
        "production_write_attempted_count": 0,
        "graph_import_attempted_count": 0,
    }
    assert report["safety_counters"] == batch.default_safety_counters()
    assert report["safety_flags"] == batch.default_safety_flags()
    assert report["recommendation"] == "proceed_to_20_document_scale_review_only"
    assert all(row["status"] == "ready_review_only" for row in report["document_status_rows"])
    assert {row["document_id"] for row in report["document_status_rows"]} == {
        f"fixture-paper-{index:04d}" for index in range(1, 11)
    }
    assert set(report["coverage_distributions"]) == EXPECTED_SUBTREES
    assert report["coverage_distributions"]["retrieval_tables"]["documents_with_records"] == 10
    assert batch.validate_article_batch_validation_report(report) == []
    _assert_metadata_only(report)


def test_unsafe_manifest_fails_closed_with_path_addressed_blockers_and_redaction() -> None:
    manifest = _load_fixture("unsafe_document_manifest.json")

    report = batch.build_article_batch_validation_report(manifest)

    assert report["aggregate_diagnostics"]["document_count"] == 3
    assert report["aggregate_diagnostics"]["blocked_document_count"] >= 2
    counts = report["aggregate_diagnostics"]["diagnostic_counts"]
    assert set(counts) == EXPECTED_DIAGNOSTIC_COUNTER_KEYS
    assert counts["batch_size_mismatch_count"] == 1
    assert counts["duplicate_document_id_count"] == 1
    assert counts["duplicate_source_id_count"] == 1
    assert counts["missing_source_path_count"] == 1
    assert counts["missing_source_hash_count"] == 1
    assert counts["forbidden_payload_detection_count"] >= 3
    assert counts["unsafe_authorization_count"] >= 3
    assert counts["unsafe_readiness_count"] >= 1
    assert counts["stale_artifact_count"] == 1
    assert report["safety_counters"] == batch.default_safety_counters()
    assert report["recommendation"] == "stop_graph_import_unsafe_evidence"
    assert "forbidden_payload_key:raw_text" in _codes(report)
    assert "forbidden_payload_key:chunks" in _codes(report)
    assert "forbidden_payload_key:embedding" in _codes(report)
    assert "unsafe_authorization_flag:production_import_attempted" in _codes(report)
    assert "unsafe_authorization_flag:ladybugdb_written" in _codes(report)
    assert "$.documents[1].source_path" in _paths(report)
    assert "$.documents[1].source_sha256" in _paths(report)
    assert "$.documents[2].freshness" in _paths(report)
    assert batch.validate_article_batch_validation_report(report) == []
    _assert_metadata_only(report)


@pytest.mark.parametrize(
    ("documents", "expected_code", "expected_recommendation"),
    [
        ([], "empty_batch", "repeat_10_document_batch_after_repairs"),
        (
            [_load_fixture("ten_document_manifest.json")["documents"][0]],
            "batch_size_mismatch",
            "repeat_10_document_batch_after_repairs",
        ),
    ],
)
def test_empty_and_fewer_than_ten_batches_are_blocked(
    documents: list[dict[str, Any]], expected_code: str, expected_recommendation: str
) -> None:
    report = batch.build_article_batch_validation_report(
        {"batch_id": "boundary-batch", "run_id": "boundary-run", "documents": documents}
    )

    assert expected_code in _codes(report)
    assert report["recommendation"] == expected_recommendation
    assert report["safety_counters"] == batch.default_safety_counters()
    _assert_metadata_only(report)


def test_missing_path_or_checksum_blocks_only_that_document_and_downgrades_recommendation() -> None:
    manifest = _load_fixture("ten_document_manifest.json")
    manifest["documents"][4]["source_path"] = ""
    manifest["documents"][4]["source_sha256"] = ""

    report = batch.build_article_batch_validation_report(manifest)

    blocked_rows = [
        row for row in report["document_status_rows"] if row["status"] == "blocked_review_only"
    ]
    ready_rows = [
        row for row in report["document_status_rows"] if row["status"] == "ready_review_only"
    ]
    assert [row["document_id"] for row in blocked_rows] == ["fixture-paper-0005"]
    assert len(ready_rows) == 9
    assert report["recommendation"] == "collect_missing_local_sources"
    assert "missing_source_path" in blocked_rows[0]["diagnostic_codes"]
    assert "missing_source_hash" in blocked_rows[0]["diagnostic_codes"]
    _assert_metadata_only(report)


def test_malformed_subtree_and_stale_freshness_block_report_without_exceptions() -> None:
    manifest = _load_fixture("ten_document_manifest.json")
    manifest["documents"][0]["subtrees"]["assets"] = {"status": "unknown_status", "record_count": 0}
    manifest["documents"][1]["subtrees"] = "not-a-subtree-map"
    manifest["documents"][2]["freshness"] = {"status": "stale", "stale_artifact_count": 2}

    report = batch.build_article_batch_validation_report(manifest)

    assert "malformed_subtree_status" in _codes(report)
    assert "malformed_subtrees" in _codes(report)
    assert "stale_artifact" in _codes(report)
    assert report["aggregate_diagnostics"]["blocked_document_count"] == 3
    assert report["recommendation"] == "repeat_10_document_batch_after_repairs"
    _assert_metadata_only(report)


def test_malformed_manifest_shape_returns_stable_diagnostics() -> None:
    report = batch.build_article_batch_validation_report(
        {"batch_id": "bad", "documents": "not-a-list"}
    )

    assert "malformed_documents" in _codes(report)
    assert report["aggregate_diagnostics"]["document_count"] == 0
    assert report["recommendation"] == "repeat_10_document_batch_after_repairs"
    assert report["safety_counters"] == batch.default_safety_counters()
    _assert_metadata_only(report)
