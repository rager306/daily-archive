"""Bounded property coverage for S07 article batch validation safety invariants."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path
from typing import Any

from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st

from research_graph.papers.artifacts import batch_validation as batch
from arxiv_archive.validation_batch_provenance import (
    build_artifact_freshness_report,
    fingerprint_file,
)

FORBIDDEN_SENTINELS = (
    "FORBIDDEN_RAW_ARTICLE_TEXT_DO_NOT_ECHO",
    "FORBIDDEN_CHUNK_TEXT",
    "FORBIDDEN_TABLE_TEXT_DO_NOT_ECHO",
    "api_key=abc123",
    "token=abc123",
    "secret=abc123",
)
FORBIDDEN_KEYS = set(batch.FORBIDDEN_PAYLOAD_KEYS)
SAFE_SUBTREE_STATUSES = (
    "complete_review_only",
    "metadata_only",
    "review_only_not_import_eligible",
)
BLOCKING_SUBTREE_STATUSES = ("blocked", "repair_required", "not_attempted", "absent")


def _document(index: int, *, status: str = "complete_review_only", freshness: str = "fresh") -> dict[str, Any]:
    return {
        "document_id": f"prop-paper-{index:04d}",
        "paper_id": f"prop-paper-{index:04d}",
        "source_id": f"prop-paper-{index:04d}:source:normalized-md",
        "source_path": f"papers/prop-paper-{index:04d}/source/normalized.md",
        "source_sha256": f"{index + 1:064x}",
        "subtrees": {name: {"status": status, "record_count": index + 1} for name in batch.SUBTREE_NAMES},
        "freshness": {"status": freshness, "stale_artifact_count": 0 if freshness == "fresh" else 1},
    }


def _manifest(documents: list[dict[str, Any]]) -> dict[str, Any]:
    return {"batch_id": "property-batch", "run_id": "property-run", "documents": documents}


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


def _serialized(report: dict[str, Any]) -> str:
    return json.dumps(report, sort_keys=True)


def _assert_metadata_only(report: dict[str, Any]) -> None:
    serialized = _serialized(report)
    assert not (set(_walk_keys(report)) & FORBIDDEN_KEYS)
    for sentinel in FORBIDDEN_SENTINELS:
        assert sentinel not in serialized
    assert report["safety_counters"] == batch.default_safety_counters()
    assert report["safety_flags"] == batch.default_safety_flags()
    assert all(row["graph_import_attempted"] is False for row in report["document_status_rows"])
    assert all(row["ladybugdb_written"] is False for row in report["document_status_rows"])
    assert all(row["production_write_attempted"] is False for row in report["document_status_rows"])
    assert all(row["import_eligible"] is False for row in report["document_status_rows"])
    assert all(row["promoted_to_fact"] is False for row in report["document_status_rows"])


@settings(max_examples=25, suppress_health_check=[HealthCheck.too_slow])
@given(statuses=st.lists(st.sampled_from(SAFE_SUBTREE_STATUSES), min_size=10, max_size=10))
def test_generated_safe_metadata_reports_are_deterministic_sorted_and_fixed_zero(statuses: list[str]) -> None:
    documents = [_document(index, status=statuses[index]) for index in range(10)]
    shuffled = list(reversed(deepcopy(documents)))

    report = batch.build_article_batch_validation_report(_manifest(shuffled))
    report_again = batch.build_article_batch_validation_report(deepcopy(_manifest(shuffled)))

    assert batch.to_json(report) == batch.to_json(report_again)
    assert batch.report_fingerprint(report) == batch.report_fingerprint(report_again)
    assert [row["document_id"] for row in report["document_status_rows"]] == sorted(
        row["document_id"] for row in report["document_status_rows"]
    )
    assert report["aggregate_diagnostics"]["document_count"] == 10
    assert report["aggregate_diagnostics"]["blocked_document_count"] == 0
    assert report["aggregate_diagnostics"]["diagnostic_counts"] == dict.fromkeys(batch.DIAGNOSTIC_COUNTER_KEYS, 0)
    assert report["recommendation"] == "proceed_to_20_document_scale_review_only"
    assert batch.validate_article_batch_validation_report(report) == []
    _assert_metadata_only(report)


@settings(max_examples=20, suppress_health_check=[HealthCheck.too_slow])
@given(
    duplicate_index=st.integers(min_value=1, max_value=9),
    missing_hash_index=st.integers(min_value=0, max_value=9),
    blocking_status=st.sampled_from(BLOCKING_SUBTREE_STATUSES),
)
def test_generated_per_document_statuses_and_aggregate_counts_fail_closed(
    duplicate_index: int, missing_hash_index: int, blocking_status: str
) -> None:
    documents = [_document(index) for index in range(10)]
    documents[duplicate_index]["document_id"] = documents[0]["document_id"]
    documents[missing_hash_index]["source_sha256"] = ""
    documents[duplicate_index]["subtrees"]["loader"]["status"] = blocking_status

    report = batch.build_article_batch_validation_report(_manifest(documents))
    counts = report["aggregate_diagnostics"]["diagnostic_counts"]

    assert counts["duplicate_document_id_count"] == 1
    assert counts["missing_source_hash_count"] >= 1
    assert counts["blocked_subtree_count"] >= 1
    assert report["aggregate_diagnostics"]["blocked_document_count"] >= 1
    assert report["recommendation"] in {
        "collect_missing_local_sources",
        "repeat_10_document_batch_after_repairs",
    }
    assert all(diagnostic["json_path"].startswith("$") for diagnostic in report["diagnostics"])
    _assert_metadata_only(report)


@settings(max_examples=20, suppress_health_check=[HealthCheck.too_slow])
@given(
    forbidden_key=st.sampled_from(sorted(batch.FORBIDDEN_PAYLOAD_KEYS)),
    sentinel=st.sampled_from(FORBIDDEN_SENTINELS),
)
def test_generated_forbidden_payload_keys_are_path_addressed_without_payload_values(
    forbidden_key: str, sentinel: str
) -> None:
    documents = [_document(index) for index in range(10)]
    documents[3]["evidence"] = {forbidden_key: sentinel}

    report = batch.build_article_batch_validation_report(_manifest(documents))
    serialized = _serialized(report)
    matching = [
        diagnostic
        for diagnostic in report["diagnostics"]
        if diagnostic["code"] == f"forbidden_payload_key:{forbidden_key}"
    ]

    assert matching
    assert matching[0]["json_path"] == f"$.documents[3].evidence.{forbidden_key}"
    assert sentinel not in serialized
    assert report["aggregate_diagnostics"]["diagnostic_counts"]["forbidden_payload_detection_count"] >= 1
    assert report["recommendation"] == "repeat_10_document_batch_after_repairs"
    _assert_metadata_only(report)


def test_token_like_url_values_are_redacted_and_reported_by_path() -> None:
    documents = [_document(index) for index in range(10)]
    documents[2]["source_path"] = "https://example.invalid/paper.md?token=abc123"
    documents[2]["subtrees"]["assets"]["manifest_path"] = "artifact.json?api_key=abc123"

    report = batch.build_article_batch_validation_report(_manifest(documents))
    codes_by_path = {(diagnostic["code"], diagnostic["json_path"]) for diagnostic in report["diagnostics"]}

    assert ("forbidden_payload_value:sensitive_token", "$.documents[2].source_path") in codes_by_path
    assert (
        "forbidden_payload_value:sensitive_token",
        "$.documents[2].subtrees.assets.manifest_path",
    ) in codes_by_path
    assert "token=abc123" not in _serialized(report)
    assert "api_key=abc123" not in _serialized(report)
    assert report["document_status_rows"][2]["source_path"] == "<redacted-sensitive-value>"
    assert report["recommendation"] == "repeat_10_document_batch_after_repairs"
    _assert_metadata_only(report)


def test_stale_missing_malformed_and_unsafe_provenance_freshness_fail_closed(tmp_path: Path) -> None:
    source = tmp_path / "source.json"
    output = tmp_path / "report.json"
    source.write_text('{"input": true}\n', encoding="utf-8")
    output.write_text(json.dumps({"schema_version": batch.ARTICLE_BATCH_VALIDATION_SCHEMA_VERSION}) + "\n", encoding="utf-8")
    entry = {
        "schema_version": "m009-validation-cli-provenance.v1",
        "run_id": "freshness-property-run",
        "batch_id": "freshness-property-batch",
        "command": "validation-batch article-report",
        "inputs": [fingerprint_file(source)],
        "outputs": [fingerprint_file(output)],
        "expected_artifact_metadata": {"schema_version": batch.ARTICLE_BATCH_VALIDATION_SCHEMA_VERSION},
        "exit_code": 0,
        **batch.default_safety_flags(),
    }

    assert build_artifact_freshness_report(entry)["verdict"] == "fresh"

    output.write_text(json.dumps({"schema_version": "stale"}) + "\n", encoding="utf-8")
    stale = build_artifact_freshness_report(entry)
    assert stale["verdict"] == "stale"
    assert {diagnostic["code"] for diagnostic in stale["diagnostics"]} >= {
        "output_hash_changed",
        "output_size_changed",
        "artifact_metadata_mismatch",
    }

    output.unlink()
    missing = build_artifact_freshness_report(entry)
    assert missing["verdict"] == "missing"
    assert "missing_output" in {diagnostic["code"] for diagnostic in missing["diagnostics"]}

    malformed = build_artifact_freshness_report({"schema_version": "wrong", "outputs": "not-a-list"})
    assert malformed["verdict"] == "invalid_provenance"

    output.write_text(json.dumps({"schema_version": batch.ARTICLE_BATCH_VALIDATION_SCHEMA_VERSION}) + "\n", encoding="utf-8")
    unsafe_entry = dict(entry)
    unsafe_entry["production_import_attempted"] = True
    unsafe = build_artifact_freshness_report(unsafe_entry)
    assert unsafe["verdict"] == "invalid_provenance"
    assert "unsafe_safety_flag" in {diagnostic["code"] for diagnostic in unsafe["diagnostics"]}


def test_freshness_diagnostics_downgrade_batch_recommendation() -> None:
    documents = [_document(index) for index in range(10)]
    documents[5]["freshness"] = {"status": "stale", "stale_artifact_count": 1}

    report = batch.build_article_batch_validation_report(_manifest(documents))
    stale_row = next(row for row in report["document_status_rows"] if row["document_id"] == "prop-paper-0005")

    assert report["provenance_freshness_summary"] == {
        "status_counts": {"fresh": 9, "stale": 1},
        "stale_artifact_count": 1,
    }
    assert stale_row["status"] == "blocked_review_only"
    assert "stale_artifact" in stale_row["diagnostic_codes"]
    assert report["aggregate_diagnostics"]["diagnostic_counts"]["stale_artifact_count"] == 1
    assert report["recommendation"] == "repeat_10_document_batch_after_repairs"
    _assert_metadata_only(report)
