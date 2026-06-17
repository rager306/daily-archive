"""Article artifact worker pool (M050).

Per M048 patterns-review 01 §4.2 (ActiveGraph serial audit + parallel
workers) and M050 (Bounded LLM Helper v2 Worker Pool):

- A bounded ProcessPoolExecutor (1-2 workers, NOT distributed) consumes
  ArticleArtifactWorkRequest objects emitted by
  research_graph.papers.artifacts.minimax_boundary.request_article_artifact_classification.

- Each worker calls a pluggable `Transport` to actually invoke MiniMax.
  Two transports are provided:
    * `HttpTransport` — real Anthropic-compatible POST to the registered
      endpoint. Reads auth from environment variable
      `MINIMAX_ARTIFACT_API_KEY` at call time (NOT at module load).
    * `MockTransport` — returns a synthetic valid tool-use response
      derived from the input structure. Used in tests and in the absence
      of `MINIMAX_ARTIFACT_API_KEY`.

- Worker pool outputs `ArticleArtifactWorkCompleted` events. They are
  written to `artifacts/m050-work-requests/<work_id>.json` (content-addressed
  artifact storage, per M048 pattern 3.4).

- No network call is made without explicit transport. No graph writes.
  No promotion authority (ADR-006 binding). All 5 safety defaults
  remain false on every output.

This module does NOT mutate graph state. It is the worker layer of
the M050 pattern; the reducer (`research_graph.papers.artifacts.reducer`) is the
idempotent merge layer.

Formerly: src/arxiv_archive/article_artifact_worker.py
"""

from __future__ import annotations

import datetime
import json
import os
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

import yaml

from research_graph.papers.artifacts.minimax_boundary import (
    ArticleArtifactWorkRequest,
    MiniMaxArtifactHelperResult,
    validate_article_artifact_minimax_response,
)
from arxiv_archive.models_registry import (
    get_model,
    load_models_registry,
)

# Default content-addressed artifact storage location.
DEFAULT_WORK_REQUEST_DIR = Path("artifacts/m050-work-requests")


@dataclass(frozen=True, repr=False)
class ArticleArtifactWorkCompleted:
    """M050 work.completed event: deterministic work_id + helper result + diagnostics.

    Diagnostic-only output (ADR-006): no graph writes, no promotion authority.
    The reducer (`research_graph.papers.artifacts.reducer`) consumes these events.
    """

    work_id: str
    binding_id: str
    model_id: str
    helper_result: MiniMaxArtifactHelperResult
    started_at: str  # ISO 8601
    completed_at: str  # ISO 8601
    transport: str
    cache_hit: bool
    diagnostics: dict[str, Any]

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "work_id": self.work_id,
            "binding_id": self.binding_id,
            "model_id": self.model_id,
            "transport": self.transport,
            "cache_hit": self.cache_hit,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "diagnostics": dict(self.diagnostics),
            "result": self.helper_result.to_sanitized_dict(),
            "graph_import_allowed": False,
            "graphdb_written": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
            "import_eligible": False,
        }


class Transport(Protocol):
    """Pluggable transport for MiniMax calls.

    Real implementations POST to MiniMax; test implementations return
    synthetic valid responses. Per M048 patterns-review 01 §4.2: workers
    are pluggable, NOT hardcoded to one provider.
    """

    def send(self, structured_request: Any) -> list[dict[str, Any]]:
        """Send structured_request to MiniMax, return content_blocks list."""
        ...


class HttpTransport:
    """Real Anthropic-compatible MiniMax transport. Reads auth from env at call time."""

    def __init__(self, *, timeout_seconds: int = 30, auth_env_var: str = "MINIMAX_ARTIFACT_API_KEY") -> None:
        self.timeout_seconds = timeout_seconds
        self.auth_env_var = auth_env_var

    def send(self, structured_request: Any) -> list[dict[str, Any]]:
        import urllib.request

        auth = os.environ.get(self.auth_env_var)
        if not auth:
            raise RuntimeError(
                f"{self.auth_env_var} not set; cannot perform live MiniMax call"
            )

        endpoint = structured_request.endpoint
        body = json.dumps(structured_request.body).encode("utf-8")
        request = urllib.request.Request(
            endpoint,
            data=body,
            headers={
                "Content-Type": "application/json",
                "x-api-key": auth,  # Anthropic-compatible header
                "anthropic-version": "2023-06-01",
            },
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=self.timeout_seconds) as response:
            payload = json.loads(response.read().decode("utf-8"))
        return list(payload.get("content", []))


class MockTransport:
    """Mock transport that returns a synthetic valid tool-use response.

    Used in tests and when no API key is available. Does NOT perform network I/O.
    Produces a response that should pass validate_article_artifact_minimax_response
    for the input structure.
    """

    def send(self, structured_request: Any) -> list[dict[str, Any]]:
        paper_id = structured_request.body.get("messages", [{}])[0].get("content", "")
        # Best-effort: parse paper_id from the prompt (which is JSON-encoded).
        # Default to "mock-paper" if not parseable.
        try:
            prompt_obj = json.loads(paper_id)
            paper_id = prompt_obj.get("redacted_structure_summary", {}).get("paper_id", "mock-paper")
        except (json.JSONDecodeError, TypeError):
            paper_id = "mock-paper"

        helper_limit = structured_request.body.get("tools", [{}])[0].get("input_schema", {}).get("properties", {}).get("helper_limit", 24)
        if not isinstance(helper_limit, int):
            helper_limit = 24

        artifact_hints = [
            {
                "artifact_id": f"mock-{paper_id}:artifact:dataset:1",
                "artifact_type": "dataset",
                "review_state": "review_required",
                "confidence_label": "needs_review",
                "evidence_span_ids": [f"mock-{paper_id}:span:section-methods"],
                "diagnostic_codes": ["mock_helper_evidence"],
            }
        ]
        tool_input = {
            "schema_version": "m023-minimax-artifact-helper.v1",
            "source_schema_version": "m023-redacted-article-structure.v1",
            "manifest_schema_version": "m023-article-artifacts.v1",
            "input_sha256": "mock-input-sha256",
            "artifact_hints": artifact_hints,
            "helper_limit": helper_limit,
            "minimax_source_of_truth": False,
            "promoted_to_fact": False,
            "import_eligible": False,
        }
        return [
            {
                "type": "tool_use",
                "name": "record_article_artifact_hints",
                "input": tool_input,
            }
        ]


def _resolve_default_transport() -> Transport:
    """Resolve the default transport based on environment."""
    if os.environ.get("MINIMAX_ARTIFACT_API_KEY"):
        return HttpTransport()
    return MockTransport()


def process_work_request(
    work_request: ArticleArtifactWorkRequest,
    *,
    structure: dict[str, Any] | None = None,
    transport: Transport | None = None,
    storage_dir: Path | None = None,
) -> ArticleArtifactWorkCompleted:
    """Process a single work request via the given transport.

    Per M048 patterns-review 01 §4.2 ActiveGraph pattern 3.1 (serial audit,
    parallel workers) and 3.4 (content-addressed artifacts):

    1. Resolve transport (HttpTransport if API key set, else MockTransport)
    2. Call transport.send(helper_request.structured_request) → content_blocks
    3. Validate response via validate_article_artifact_minimax_response
    4. Persist ArticleArtifactWorkCompleted to artifacts/m050-work-requests/<work_id>.json
    5. Return the completed event for the reducer

    `structure` is required to validate the response; if not provided,
    derived from the helper request's input_sha256 (test fixture should pass
    the original structure).
    """
    if transport is None:
        transport = _resolve_default_transport()

    started_at = datetime.datetime.now(tz=datetime.timezone.utc).isoformat()

    if structure is None:
        # Tests that don't pass structure fall back to using the helper_request
        # diagnostics. The real validation cannot run without the original
        # structure, so we record a diagnostic.
        helper_result = MiniMaxArtifactHelperResult(
            candidates=(),
            diagnostics={
                "validation_status": "skipped_no_structure",
                "work_id": work_request.work_id,
            },
        )
    else:
        # Call the transport.
        content_blocks = transport.send(work_request.helper_request.structured_request)
        helper_result = validate_article_artifact_minimax_response(
            content_blocks,
            structure=structure,
            max_candidates=work_request.max_candidates,
        )

    completed_at = datetime.datetime.now(tz=datetime.timezone.utc).isoformat()

    cache_hit = False  # future: M050 fingerprint cache lookup
    work_completed = ArticleArtifactWorkCompleted(
        work_id=work_request.work_id,
        binding_id=work_request.binding_id,
        model_id=work_request.model_id,
        helper_result=helper_result,
        started_at=started_at,
        completed_at=completed_at,
        transport=type(transport).__name__,
        cache_hit=cache_hit,
        diagnostics={
            "transport": type(transport).__name__,
            "cache_hit": cache_hit,
            "max_candidates": work_request.max_candidates,
        },
    )

    # Persist content-addressed artifact (per M048 pattern 3.4).
    target_dir = storage_dir if storage_dir is not None else DEFAULT_WORK_REQUEST_DIR
    target_path = target_dir / f"{work_request.work_id}.json"
    target_path.parent.mkdir(parents=True, exist_ok=True)
    target_path.write_text(
        json.dumps(work_completed.to_sanitized_dict(), indent=2, sort_keys=True, ensure_ascii=False),
        encoding="utf-8",
    )

    return work_completed


def run_worker_pool(
    work_requests: list[ArticleArtifactWorkRequest],
    *,
    structures: dict[str, dict[str, Any]] | None = None,
    transport: Transport | None = None,
    max_workers: int = 1,
    storage_dir: Path | None = None,
) -> list[ArticleArtifactWorkCompleted]:
    """Process a list of work requests via bounded ProcessPoolExecutor.

    Per M048 patterns-review 01 §4.2: bounded executor (1-2 workers, NOT
    distributed). If max_workers=1 (default), runs sequentially in the
    current process (simpler for tests). For >1 workers, uses
    concurrent.futures.ProcessPoolExecutor.

    `structures` maps work_id → original structure dict. Required for
    validation; work_ids without corresponding structures will be
    processed with `skipped_no_structure` diagnostic.
    """
    if structures is None:
        structures = {}

    if max_workers <= 1:
        return [
            process_work_request(
                wr,
                structure=structures.get(wr.work_id),
                transport=transport,
                storage_dir=storage_dir,
            )
            for wr in work_requests
        ]

    # Bounded ProcessPoolExecutor for parallel workers.
    completed: list[ArticleArtifactWorkCompleted] = []
    with ProcessPoolExecutor(max_workers=max_workers) as executor:
        futures = {
            executor.submit(
                _process_in_subprocess,
                wr,
                structures.get(wr.work_id),
            ): wr
            for wr in work_requests
        }
        for future, wr in futures.items():
            completed.append(future.result())
    return completed


def _process_in_subprocess(
    work_request: ArticleArtifactWorkRequest,
    structure: dict[str, Any] | None,
) -> ArticleArtifactWorkCompleted:
    """Subprocess entry point. ProcessPoolExecutor workers run this."""
    return process_work_request(work_request, structure=structure)


__all__ = [
    "DEFAULT_WORK_REQUEST_DIR",
    "ArticleArtifactWorkCompleted",
    "HttpTransport",
    "MockTransport",
    "Transport",
    "process_work_request",
    "run_worker_pool",
]
