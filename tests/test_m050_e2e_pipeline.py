"""End-to-end pipeline test for M050: requester -> worker pool -> reducer.

Per M050 S02 plan, this exercises the full pipeline on real fixtures:

1. `request_article_artifact_classification` (M050 S01) emits 2
   `ArticleArtifactWorkRequest` objects from 1 structure.
2. `run_worker_pool` (M050 S01) processes them with `MockTransport`
   (no API key, no network).
3. `aggregate_article_artifact_log` (M050 S02) reads the
   content-addressed work-request directory and emits a reducer
   aggregate.

Assertions:
- 2 work_ids present in the aggregate
- work_ids match between request and result
- 5 safety defaults all false in the aggregate
- artifact files in the work-request directory all present
- aggregate output is deterministic on re-run (modulo generated_at)
- work_ids are sorted alphabetically in the aggregate

Uses tmp_path for the work-request directory to keep the working tree
clean (M050 test hygiene lesson from M027/M028 fix in commit 51d1885).
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from arxiv_archive.article_artifact_minimax import (
    ArticleArtifactWorkRequest,
    request_article_artifact_classification,
)
from arxiv_archive.article_artifact_reducer import (
    REDUCER_SCHEMA_VERSION,
    _safety_defaults,
    aggregate_article_artifact_log,
    merge_article_artifact_results,
)
from arxiv_archive.article_artifact_worker import (
    ArticleArtifactWorkCompleted,
    MockTransport,
    run_worker_pool,
)


# ---------- fixtures ----------

_FIXTURE_DIR = Path(__file__).parents[1] / "tests" / "fixtures" / "article_artifacts"
_FIXTURE_STRUCTURE = json.loads(
    (_FIXTURE_DIR / "basic_article_structure.json").read_text(encoding="utf-8")
)


def _synthetic_structure(paper_id: str = "e2e-mock-001") -> dict[str, Any]:
    """Build a valid article structure for the M050 pipeline.

    Uses the existing fixture as a base (which has the required
    schema_version, source_refs, sections, artifact_placeholders, and
    safety_flags fields) and overrides the paper_id so different test
    runs get different work_ids via the deterministic hash.
    """
    structure = json.loads(json.dumps(_FIXTURE_STRUCTURE))  # deep copy
    structure["paper_id"] = paper_id
    return structure


# ---------- e2e tests ----------

def test_e2e_two_work_requests_one_structure(tmp_path: Path) -> None:
    structure = _synthetic_structure("e2e-paper-A")

    # Step 1: requester emits 2 work requests for the same structure
    # (different binding_id and run_id yield different work_ids).
    request_a = request_article_artifact_classification(
        structure, max_candidates=2, binding_id="article-artifact-classify", run_id="e2e-run-1"
    )
    request_b = request_article_artifact_classification(
        structure, max_candidates=4, binding_id="article-artifact-classify", run_id="e2e-run-2"
    )
    assert isinstance(request_a, ArticleArtifactWorkRequest)
    assert isinstance(request_b, ArticleArtifactWorkRequest)
    assert request_a.work_id != request_b.work_id

    # Step 2: bounded worker pool (max_workers=1 sequential) with MockTransport.
    structures = {request_a.work_id: structure, request_b.work_id: structure}
    completed = run_worker_pool(
        [request_a, request_b],
        structures=structures,
        transport=MockTransport(),
        max_workers=1,
        storage_dir=tmp_path,
    )
    assert len(completed) == 2
    assert all(isinstance(c, ArticleArtifactWorkCompleted) for c in completed)

    # Step 3: reducer aggregates the work-request directory.
    aggregate = aggregate_article_artifact_log(tmp_path)
    assert aggregate["schema_version"] == REDUCER_SCHEMA_VERSION
    assert aggregate["directory_exists"] is True
    assert aggregate["total_unique_work_ids"] == 2
    # work_ids in the aggregate must match the request work_ids.
    assert set(aggregate["work_ids"]) == {request_a.work_id, request_b.work_id}
    # work_ids must be sorted alphabetically.
    assert aggregate["work_ids"] == sorted(aggregate["work_ids"])


def test_e2e_safety_defaults_in_full_aggregate(tmp_path: Path) -> None:
    structure = _synthetic_structure("e2e-paper-B")
    request = request_article_artifact_classification(
        structure, max_candidates=2, run_id="e2e-safety"
    )
    completed = run_worker_pool(
        [request], structures={request.work_id: structure}, transport=MockTransport(), storage_dir=tmp_path
    )
    assert len(completed) == 1

    # Worker output must already carry the 5 safety defaults (per ADR-006).
    worker_event = completed[0].to_sanitized_dict()
    for key, value in _safety_defaults().items():
        assert worker_event[key] is False

    # Reducer aggregate must also carry them.
    aggregate = aggregate_article_artifact_log(tmp_path)
    for key, value in _safety_defaults().items():
        assert key in aggregate
        assert aggregate[key] is False


def test_e2e_artifact_files_present_in_storage_dir(tmp_path: Path) -> None:
    structure = _synthetic_structure("e2e-paper-C")
    request = request_article_artifact_classification(
        structure, max_candidates=2, run_id="e2e-storage"
    )
    run_worker_pool(
        [request], structures={request.work_id: structure}, transport=MockTransport(), storage_dir=tmp_path
    )

    # The work-request file must exist at tmp_path/<work_id>.json
    artifact_path = tmp_path / f"{request.work_id}.json"
    assert artifact_path.exists()
    payload = json.loads(artifact_path.read_text(encoding="utf-8"))
    assert payload["work_id"] == request.work_id
    assert payload["binding_id"] == "article-artifact-classify"
    assert "result" in payload
    # 5 safety defaults carried in the artifact too.
    for key, value in _safety_defaults().items():
        assert payload[key] is False


def test_e2e_aggregate_deterministic_on_rerun(tmp_path: Path) -> None:
    structure = _synthetic_structure("e2e-paper-D")
    request = request_article_artifact_classification(
        structure, max_candidates=2, run_id="e2e-det"
    )
    run_worker_pool(
        [request], structures={request.work_id: structure}, transport=MockTransport(), storage_dir=tmp_path
    )

    # Re-aggregate twice. Only `generated_at` should differ.
    out_a = aggregate_article_artifact_log(tmp_path)
    out_b = aggregate_article_artifact_log(tmp_path)
    out_a.pop("generated_at")
    out_b.pop("generated_at")
    assert json.dumps(out_a, sort_keys=True) == json.dumps(out_b, sort_keys=True)


def test_e2e_merge_dedup_when_same_work_id_replayed(tmp_path: Path) -> None:
    """If the same work request is processed twice, the reducer must dedup.

    This is the ActiveGraph pattern 3.6 idempotency requirement:
    replaying a partial run produces the same aggregate as the full run.
    """
    structure = _synthetic_structure("e2e-paper-E")
    request = request_article_artifact_classification(
        structure, max_candidates=2, run_id="e2e-dedup"
    )
    # First run writes the artifact.
    run_worker_pool(
        [request], structures={request.work_id: structure}, transport=MockTransport(), storage_dir=tmp_path
    )
    # Second run re-processes the same work_id and overwrites the artifact.
    run_worker_pool(
        [request], structures={request.work_id: structure}, transport=MockTransport(), storage_dir=tmp_path
    )

    aggregate = aggregate_article_artifact_log(tmp_path)
    # Only 1 unique work_id; the second run is a dedup, not a new entry.
    assert aggregate["total_unique_work_ids"] == 1
    assert aggregate["work_ids"] == [request.work_id]


def test_e2e_no_safety_default_ever_flips_true(tmp_path: Path) -> None:
    """Belt-and-braces: across multiple work requests with different
    validation_status values, the reducer aggregate never flips a
    safety default to true.
    """
    structure = _synthetic_structure("e2e-paper-F")
    # 3 work requests, one each of valid/invalid/skipped_no_structure.
    requests = []
    for i, status in enumerate(("valid", "invalid", "skipped_no_structure")):
        # We can't directly set validation_status from the requester;
        # the worker records whatever the validator emits. For E2E,
        # we just want multiple work_ids in the aggregate. The
        # MockTransport will produce "valid" or "skipped" depending
        # on structure validity, but the safety-default invariant
        # is what we care about here.
        request = request_article_artifact_classification(
            structure, max_candidates=i + 1, run_id=f"e2e-safety-{i}"
        )
        requests.append(request)

    completed = run_worker_pool(
        requests,
        structures={r.work_id: structure for r in requests},
        transport=MockTransport(),
        storage_dir=tmp_path,
    )
    assert len(completed) == 3

    # None of the worker events may carry a True safety default.
    for event in completed:
        sanitized = event.to_sanitized_dict()
        for key, value in _safety_defaults().items():
            assert sanitized[key] is False, f"worker event flipped {key}"

    # Reducer aggregate must also keep them all False.
    aggregate = aggregate_article_artifact_log(tmp_path)
    for key, value in _safety_defaults().items():
        assert aggregate[key] is False, f"reducer flipped {key}"
