"""Tests for M050 article_artifact_worker (work requester + worker pool).

Per M048 patterns-review 01 §4.2 and M050 spec:
- request_article_artifact_classification emits work_id
- process_work_request processes work via transport
- run_worker_pool supports bounded parallel
- Safety contract preserved: 5× false safety defaults
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]

VALID_STRUCTURE = {
    "schema_version": "m023-redacted-article-structure.v1",
    "paper_id": "1804.02767",
    "sections": [
        {
            "section_id": "1804.02767:section:methods",
            "parent_section_id": None,
            "section_type": "methods",
            "ordinal_path": [1],
            "span_id": "1804.02767:span:section-methods",
        }
    ],
    "artifact_placeholders": [],
    "structured_markers": [],
    "scientific_markers": [],
    "safe_spans": [],
    "source_refs": [],
    "safety_flags": {
        "helper_evidence_only": True,
        "raw_prompt_persisted": False,
        "raw_response_persisted": False,
        "import_eligible": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
    },
}


def test_request_emits_work_request_with_deterministic_work_id():
    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.models_registry import reset_cache

    reset_cache()
    wr1 = request_article_artifact_classification(VALID_STRUCTURE)
    wr2 = request_article_artifact_classification(VALID_STRUCTURE)
    assert wr1.work_id == wr2.work_id
    assert len(wr1.work_id) == 64  # sha256 hex digest
    assert wr1.binding_id == "article-artifact-classify"
    assert wr1.paper_id == "1804.02767"
    assert wr1.max_candidates == 24


def test_work_request_preserves_safety_defaults():
    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.models_registry import reset_cache

    reset_cache()
    wr = request_article_artifact_classification(VALID_STRUCTURE)
    sanitized = wr.to_sanitized_dict()
    assert sanitized["import_eligible"] is False
    assert sanitized["graph_import_allowed"] is False
    assert sanitized["production_import_attempted"] is False
    assert sanitized["ladybugdb_written"] is False
    assert sanitized["diagnostic_only"] is True


def test_work_request_with_run_id_changes_work_id():
    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.models_registry import reset_cache

    reset_cache()
    base = dict(structure=VALID_STRUCTURE)
    wr_a = request_article_artifact_classification(VALID_STRUCTURE, run_id="run-A")
    wr_b = request_article_artifact_classification(VALID_STRUCTURE, run_id="run-B")
    wr_none = request_article_artifact_classification(VALID_STRUCTURE)
    assert wr_a.work_id != wr_b.work_id
    assert wr_a.work_id != wr_none.work_id
    assert wr_b.work_id != wr_none.work_id


def test_process_work_request_uses_mock_transport_when_no_api_key():
    """No network: process_work_request with MockTransport produces a WorkCompleted."""
    import os

    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.article_artifact_worker import process_work_request, MockTransport
    from arxiv_archive.models_registry import reset_cache

    # Ensure no API key is set for the test.
    saved = os.environ.pop("MINIMAX_ARTIFACT_API_KEY", None)
    try:
        reset_cache()
        wr = request_article_artifact_classification(VALID_STRUCTURE)
        wc = process_work_request(wr, structure=VALID_STRUCTURE, transport=MockTransport())
        assert wc.work_id == wr.work_id
        assert wc.binding_id == wr.binding_id
        assert wc.model_id == wr.model_id
        assert wc.transport == "MockTransport"
        # Mock produces a tool_use response; validation may pass or fail
        # depending on input_sha256. The key is that the worker handled it.
        assert wc.helper_result.diagnostics is not None
    finally:
        if saved is not None:
            os.environ["MINIMAX_ARTIFACT_API_KEY"] = saved


def test_process_work_request_persists_artifact_to_storage_dir(tmp_path):
    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.article_artifact_worker import process_work_request, MockTransport
    from arxiv_archive.models_registry import reset_cache

    reset_cache()
    wr = request_article_artifact_classification(VALID_STRUCTURE)
    wc = process_work_request(
        wr,
        structure=VALID_STRUCTURE,
        transport=MockTransport(),
        storage_dir=tmp_path,
    )
    artifact_path = tmp_path / f"{wc.work_id}.json"
    assert artifact_path.exists()
    payload = json.loads(artifact_path.read_text(encoding="utf-8"))
    assert payload["work_id"] == wc.work_id
    assert payload["import_eligible"] is False
    assert payload["graph_import_allowed"] is False


def test_run_worker_pool_processes_sequentially_with_max_workers_1():
    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.article_artifact_worker import run_worker_pool, MockTransport
    from arxiv_archive.models_registry import reset_cache

    reset_cache()
    wr1 = request_article_artifact_classification(VALID_STRUCTURE, run_id="run-1")
    wr2 = request_article_artifact_classification(VALID_STRUCTURE, run_id="run-2")
    wr3 = request_article_artifact_classification(VALID_STRUCTURE, run_id="run-3")

    completed = run_worker_pool(
        [wr1, wr2, wr3],
        structures={
            wr1.work_id: VALID_STRUCTURE,
            wr2.work_id: VALID_STRUCTURE,
            wr3.work_id: VALID_STRUCTURE,
        },
        transport=MockTransport(),
        max_workers=1,
    )
    assert len(completed) == 3
    assert {c.work_id for c in completed} == {wr1.work_id, wr2.work_id, wr3.work_id}


def test_run_worker_pool_supports_max_workers_2():
    """Bounded ProcessPoolExecutor (max_workers=2) per M048 §4.2."""
    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.article_artifact_worker import run_worker_pool, MockTransport
    from arxiv_archive.models_registry import reset_cache

    reset_cache()
    work_requests = [
        request_article_artifact_classification(VALID_STRUCTURE, run_id=f"run-{i}")
        for i in range(4)
    ]
    completed = run_worker_pool(
        work_requests,
        structures={wr.work_id: VALID_STRUCTURE for wr in work_requests},
        transport=MockTransport(),
        max_workers=2,
    )
    assert len(completed) == 4
    assert all(c.work_id in {wr.work_id for wr in work_requests} for c in completed)


def test_mock_transport_returns_valid_tool_use_response():
    """MockTransport's synthetic response passes basic tool_use shape checks."""
    from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
    from arxiv_archive.article_artifact_worker import MockTransport
    from arxiv_archive.models_registry import reset_cache

    reset_cache()
    wr = request_article_artifact_classification(VALID_STRUCTURE)
    transport = MockTransport()
    content_blocks = transport.send(wr.helper_request.structured_request)
    assert len(content_blocks) >= 1
    tool_use_blocks = [b for b in content_blocks if b.get("type") == "tool_use"]
    assert len(tool_use_blocks) == 1
    tool_input = tool_use_blocks[0]["input"]
    assert tool_input["minimax_source_of_truth"] is False
    assert tool_input["promoted_to_fact"] is False
    assert tool_input["import_eligible"] is False


def test_existing_article_artifact_minimax_tests_still_pass():
    """Backward compat: existing 9 tests in test_article_artifact_minimax.py still pass."""
    result = subprocess.run(
        ["uv", "run", "pytest", "tests/test_article_artifact_minimax.py", "-q"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"existing tests broke: {result.stderr}"
    assert "9 passed" in result.stdout
