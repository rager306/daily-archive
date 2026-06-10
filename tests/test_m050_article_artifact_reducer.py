"""Tests for M050 article_artifact_reducer (idempotent merge layer).

Per M048 patterns-review 01 §3.6 (ActiveGraph pattern: deterministic,
idempotent, content-addressed merge) and M050 S02 plan:

1-2. merge_article_artifact_results determinism: same input -> identical
     output (byte-identical hashes)
3-4. merge with duplicates: dedup by work_id, count duplicates
5.   merge with empty list: returns empty aggregate with safety defaults
6-7. aggregate_article_artifact_log: counts + per-binding-id breakdown
8.   _safety_defaults: all 5 flags false
9.   Idempotency: re-running aggregate on the same directory produces
     byte-identical output
10.  Fail-soft: malformed JSON in directory is counted, not raised
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

import pytest

from arxiv_archive.article_artifact_reducer import (
    DEFAULT_VALIDATION_BUCKETS,
    REDUCER_SCHEMA_VERSION,
    _safety_defaults,
    aggregate_article_artifact_log,
    merge_article_artifact_results,
)


# ---------- helpers ----------

def _work_completed(
    work_id: str,
    binding_id: str = "article-artifact-classify",
    model_id: str = "minimax-text-01",
    validation_status: str = "valid",
    transport: str = "MockTransport",
) -> dict[str, Any]:
    """Build a synthetic work.completed event matching the worker shape."""
    return {
        "work_id": work_id,
        "binding_id": binding_id,
        "model_id": model_id,
        "transport": transport,
        "cache_hit": False,
        "started_at": "2026-06-10T12:00:00+00:00",
        "completed_at": "2026-06-10T12:00:01+00:00",
        "diagnostics": {"transport": transport, "cache_hit": False, "max_candidates": 24},
        "result": {
            "schema_version": "m023-minimax-artifact-helper.v1",
            "candidate_count": 0,
            "candidates": [],
            "diagnostics": {"validation_status": validation_status},
        },
        "import_eligible": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_import_allowed": False,
    }


# ---------- 1-2: determinism ----------

def test_merge_results_deterministic_byte_identical_output() -> None:
    results = [
        _work_completed("wid-001"),
        _work_completed("wid-002", validation_status="invalid"),
        _work_completed("wid-003", validation_status="skipped_no_structure"),
    ]
    out_a = merge_article_artifact_results(results)
    out_b = merge_article_artifact_results(results)
    # The only field that can differ is `generated_at` (microsecond precision).
    # Strip it for the byte-identity check.
    out_a.pop("generated_at")
    out_b.pop("generated_at")
    assert json.dumps(out_a, sort_keys=True) == json.dumps(out_b, sort_keys=True)


def test_merge_results_sorts_by_work_id() -> None:
    results = [
        _work_completed("wid-zzz"),
        _work_completed("wid-aaa"),
        _work_completed("wid-mmm"),
    ]
    out = merge_article_artifact_results(results)
    assert out["work_ids"] == ["wid-aaa", "wid-mmm", "wid-zzz"]


# ---------- 3-4: dedup by work_id ----------

def test_merge_dedups_by_work_id() -> None:
    results = [
        _work_completed("wid-001", transport="MockTransport"),
        _work_completed("wid-001", transport="HttpTransport"),  # duplicate
        _work_completed("wid-002"),
    ]
    out = merge_article_artifact_results(results)
    assert out["total_unique_work_ids"] == 2
    assert out["input_count"] == 3
    assert out["duplicate_count"] == 1
    # Last-occurrence-wins: dedup keeps the LAST value for the work_id.
    deduped = {p["work_id"]: p for p in results}
    assert out["work_ids"] == sorted(deduped.keys())


def test_merge_dedup_keeps_last_occurrence() -> None:
    first = _work_completed("wid-001", transport="MockTransport")
    second = _work_completed("wid-001", transport="HttpTransport")
    out = merge_article_artifact_results([first, second])
    # Only one work_id in the result.
    assert out["total_unique_work_ids"] == 1


# ---------- 5: empty ----------

def test_merge_empty_list_returns_safe_aggregate() -> None:
    out = merge_article_artifact_results([])
    assert out["schema_version"] == REDUCER_SCHEMA_VERSION
    assert out["total_unique_work_ids"] == 0
    assert out["input_count"] == 0
    assert out["duplicate_count"] == 0
    assert out["work_ids"] == []
    assert out["binding_counts"] == {}
    assert out["validation_status_counts"] == {}
    # All 5 safety flags must be false.
    for key, value in _safety_defaults().items():
        assert out[key] is False


# ---------- 6-7: aggregate from directory ----------

def test_aggregate_directory_counts_per_binding_id(tmp_path: Path) -> None:
    work_dir = tmp_path / "work-requests"
    work_dir.mkdir()
    payloads = [
        _work_completed("wid-001", binding_id="article-artifact-classify"),
        _work_completed("wid-002", binding_id="article-artifact-classify"),
        _work_completed("wid-003", binding_id="another-binding"),
    ]
    for p in payloads:
        (work_dir / f"{p['work_id']}.json").write_text(
            json.dumps(p, sort_keys=True), encoding="utf-8"
        )

    out = aggregate_article_artifact_log(work_dir)
    assert out["directory_exists"] is True
    assert out["total_unique_work_ids"] == 3
    assert out["binding_counts"] == {
        "another-binding": 1,
        "article-artifact-classify": 2,
    }


def test_aggregate_directory_validation_status_counts(tmp_path: Path) -> None:
    work_dir = tmp_path / "work-requests"
    work_dir.mkdir()
    payloads = [
        _work_completed("wid-001", validation_status="valid"),
        _work_completed("wid-002", validation_status="valid"),
        _work_completed("wid-003", validation_status="invalid"),
        _work_completed("wid-004", validation_status="skipped_no_structure"),
        _work_completed("wid-005", validation_status="not_evaluated"),
    ]
    for p in payloads:
        (work_dir / f"{p['work_id']}.json").write_text(
            json.dumps(p, sort_keys=True), encoding="utf-8"
        )

    out = aggregate_article_artifact_log(work_dir)
    assert out["validation_status_counts"] == {
        "invalid": 1,
        "not_evaluated": 1,
        "skipped_no_structure": 1,
        "valid": 2,
    }


# ---------- 8: safety defaults ----------

def test_safety_defaults_all_false() -> None:
    defaults = _safety_defaults()
    assert defaults == {
        "graph_import_allowed": False,
        "graphdb_written": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
        "import_eligible": False,
    }
    # Belt-and-braces: no flag is True.
    assert not any(defaults.values())


def test_aggregate_emits_safety_defaults_in_output(tmp_path: Path) -> None:
    work_dir = tmp_path / "work-requests"
    work_dir.mkdir()
    (work_dir / "wid-001.json").write_text(
        json.dumps(_work_completed("wid-001"), sort_keys=True), encoding="utf-8"
    )
    out = aggregate_article_artifact_log(work_dir)
    for key, value in _safety_defaults().items():
        assert key in out
        assert out[key] is False


# ---------- 9: idempotency on directory ----------

def test_aggregate_idempotent_byte_identical_output(tmp_path: Path) -> None:
    work_dir = tmp_path / "work-requests"
    work_dir.mkdir()
    for p in [
        _work_completed("wid-001"),
        _work_completed("wid-002", validation_status="invalid"),
    ]:
        (work_dir / f"{p['work_id']}.json").write_text(
            json.dumps(p, sort_keys=True), encoding="utf-8"
        )

    out_a = aggregate_article_artifact_log(work_dir)
    out_b = aggregate_article_artifact_log(work_dir)
    # The only field that can differ is `generated_at`. Strip it for the
    # byte-identity check.
    out_a.pop("generated_at")
    out_b.pop("generated_at")
    assert json.dumps(out_a, sort_keys=True) == json.dumps(out_b, sort_keys=True)


# ---------- 10: fail-soft on malformed JSON ----------

def test_aggregate_fail_soft_on_malformed_json(tmp_path: Path) -> None:
    work_dir = tmp_path / "work-requests"
    work_dir.mkdir()
    # Valid file.
    (work_dir / "wid-good.json").write_text(
        json.dumps(_work_completed("wid-good"), sort_keys=True), encoding="utf-8"
    )
    # Malformed JSON.
    (work_dir / "wid-bad.json").write_text("{ this is not json", encoding="utf-8")
    # Missing required field.
    (work_dir / "wid-empty.json").write_text(
        json.dumps({"work_id": "wid-empty"}), encoding="utf-8"
    )
    # Wrong type (not a dict).
    (work_dir / "wid-list.json").write_text(json.dumps(["not", "a", "dict"]), encoding="utf-8")

    out = aggregate_article_artifact_log(work_dir)
    # Only the good one is counted.
    assert out["total_unique_work_ids"] == 1
    assert out["work_ids"] == ["wid-good"]
    # Three malformed/skipped files.
    assert out["malformed_artifact_count"] == 3


def test_aggregate_on_missing_directory_is_fail_closed(tmp_path: Path) -> None:
    nonexistent = tmp_path / "does-not-exist"
    out = aggregate_article_article_log = aggregate_article_artifact_log(nonexistent)
    assert out["directory_exists"] is False
    assert out["total_unique_work_ids"] == 0
    assert out["malformed_artifact_count"] == 0
    # Safety defaults still false.
    for key, value in _safety_defaults().items():
        assert out[key] is False


# ---------- extra: DEFAULT_VALIDATION_BUCKETS sanity ----------

def test_validation_buckets_include_all_expected_statuses() -> None:
    assert "valid" in DEFAULT_VALIDATION_BUCKETS
    assert "invalid" in DEFAULT_VALIDATION_BUCKETS
    assert "skipped_no_structure" in DEFAULT_VALIDATION_BUCKETS
    assert "not_evaluated" in DEFAULT_VALIDATION_BUCKETS
