"""Application-owned single-source analysis use case (M200 S01 / D114).

``analyze_source`` is the canonical no-write entry for one preserved source:

1. Seed :class:`PipelineContext` with caller-provided ``text_parts``
   (structure already extracted by the caller / adapter — keeps this module
   free of infrastructure parser imports).
2. Compose the paper pipeline via :func:`build_wired_paper_pipeline`
   (existing composition root, D086 / D117).
3. Execute through :class:`PipelineOrchestrator` + :class:`SyncDispatch`
   (existing L4 seam — not a second composition root).

Fail-closed: no graph writes, no import authorization, no network. Empty
``text_parts`` yields ``status="empty"`` without running stages.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field
from typing import Any, Literal

from research_graph.application.orchestrator import PipelineOrchestrator, SyncDispatch
from research_graph.application.profiles.paper import (
    PAPER_STAGE_ORDER,
    build_wired_paper_pipeline,
)
from research_graph.application.types import Pipeline, PipelineContext

AnalyzeSourceStatus = Literal["done", "empty", "failed"]


@dataclass(frozen=True)
class AnalyzeSourceRequest:
    """Input for one no-write source analysis."""

    source_id: str
    text_parts: Sequence[str] = ()
    keyword_top_k: int = 20


@dataclass(frozen=True)
class AnalyzeSourceResult:
    """Typed result of :class:`AnalyzeSourceUseCase` (metadata-only, no body dump)."""

    source_id: str
    status: AnalyzeSourceStatus
    stage_names: tuple[str, ...] = ()
    stage_output_keys: tuple[str, ...] = ()
    pipeline_context: PipelineContext | None = None
    diagnostic: str | None = None
    safety: dict[str, bool] = field(
        default_factory=lambda: {
            "graph_writes_authorized": False,
            "production_import_authorized": False,
            "fact_promotion_authorized": False,
            "external_network_authorized": False,
            "llm_calls_authorized": False,
        }
    )


class AnalyzeSourceUseCase:
    """Run one source through the canonical paper pipeline + orchestrator."""

    def __init__(
        self,
        *,
        pipeline: Pipeline | None = None,
        pipeline_factory: Any | None = None,
        orchestrator_factory: Any | None = None,
    ) -> None:
        """Optional injection for tests; production uses defaults.

        ``pipeline`` — prebuilt pipeline (skips factory).
        ``pipeline_factory`` — callable ``(**kwargs) -> Pipeline`` (default
        :func:`build_wired_paper_pipeline`).
        ``orchestrator_factory`` — callable ``(pipeline) -> PipelineOrchestrator``.
        """
        self._pipeline = pipeline
        self._pipeline_factory = pipeline_factory or build_wired_paper_pipeline
        self._orchestrator_factory = orchestrator_factory or (
            lambda pipe: PipelineOrchestrator(pipeline=pipe, dispatch=SyncDispatch())
        )

    def run(self, request: AnalyzeSourceRequest) -> AnalyzeSourceResult:
        """Execute the tracer. Never authorizes graph/import writes."""
        source_id = request.source_id
        text_parts = tuple(part for part in request.text_parts if part and str(part).strip())

        if not text_parts:
            return AnalyzeSourceResult(
                source_id=source_id,
                status="empty",
                stage_names=(),
                stage_output_keys=(),
                diagnostic="empty_text_parts",
            )

        try:
            pipeline = self._pipeline or self._pipeline_factory(
                source_id=source_id,
                keyword_top_k=request.keyword_top_k,
            )
            orchestrator = self._orchestrator_factory(pipeline)
            seed = PipelineContext(
                source_id=source_id,
                stage_manifest=pipeline.manifest(),
            )
            # Seed structure as text_parts (caller-owned structure extraction).
            from dataclasses import replace

            seed = replace(seed, stage_outputs={"text_parts": list(text_parts)})
            ctx = orchestrator.run(seed)
        except Exception as exc:  # noqa: BLE001 — use-case boundary
            return AnalyzeSourceResult(
                source_id=source_id,
                status="failed",
                diagnostic=f"analyze_source_failed:{type(exc).__name__}",
            )

        stage_names = tuple(stage.stage_name for stage in pipeline.stages)
        output_keys = tuple(sorted(ctx.stage_outputs.keys()))
        return AnalyzeSourceResult(
            source_id=source_id,
            status="done",
            stage_names=stage_names,
            stage_output_keys=output_keys,
            pipeline_context=ctx,
            diagnostic=None,
        )


def analyze_source(
    source_id: str,
    text_parts: Sequence[str],
    *,
    keyword_top_k: int = 20,
) -> AnalyzeSourceResult:
    """Module-level convenience wrapper around :class:`AnalyzeSourceUseCase`."""
    return AnalyzeSourceUseCase().run(
        AnalyzeSourceRequest(
            source_id=source_id,
            text_parts=text_parts,
            keyword_top_k=keyword_top_k,
        )
    )


__all__ = [
    "AnalyzeSourceRequest",
    "AnalyzeSourceResult",
    "AnalyzeSourceStatus",
    "AnalyzeSourceUseCase",
    "PAPER_STAGE_ORDER",
    "analyze_source",
]
