"""Application-owned single-chunk typed extraction tracer (M201 S01).

Runs one reviewed chunk through :func:`build_wired_paper_pipeline` with an
injected :class:`~research_graph.domain.ports.LLMClientPort` and
:class:`~research_graph.application.orchestrator.PipelineOrchestrator`.
Returns candidate :class:`~research_graph.domain.schema.ExtractionPatch` from
the core entity stage — fail-closed, no graph write.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field

from research_graph.application.orchestrator import PipelineOrchestrator, SyncDispatch
from research_graph.application.profiles.paper import build_wired_paper_pipeline
from research_graph.application.types import PipelineContext
from research_graph.domain.ports import LLMClientPort
from research_graph.domain.schema import ExtractionPatch


@dataclass(frozen=True)
class ChunkExtractionRequest:
    """One redacted chunk for typed candidate extraction."""

    source_id: str
    text_parts: Sequence[str]
    keyword_top_k: int = 20


@dataclass(frozen=True)
class ChunkExtractionResult:
    """Fail-closed candidate packet for one chunk."""

    source_id: str
    status: str  # done | empty | failed
    entity_count: int = 0
    relation_count: int = 0
    extraction_patch: ExtractionPatch | None = None
    stage_output_keys: tuple[str, ...] = ()
    diagnostic: str | None = None
    safety: dict[str, bool] = field(
        default_factory=lambda: {
            "graph_writes_authorized": False,
            "production_import_authorized": False,
            "fact_promotion_authorized": False,
            "external_network_authorized": False,
            "llm_calls_authorized": True,  # port may call LLM; still fail-closed candidates
        }
    )


class ChunkExtractionUseCase:
    """Trace one chunk through the paper pipeline with a real LLMClientPort."""

    def __init__(self, *, llm_provider: LLMClientPort) -> None:
        self._llm_provider = llm_provider

    def run(self, request: ChunkExtractionRequest) -> ChunkExtractionResult:
        text_parts = tuple(p for p in request.text_parts if p and str(p).strip())
        if not text_parts:
            return ChunkExtractionResult(
                source_id=request.source_id,
                status="empty",
                diagnostic="empty_text_parts",
                safety={
                    "graph_writes_authorized": False,
                    "production_import_authorized": False,
                    "fact_promotion_authorized": False,
                    "external_network_authorized": False,
                    "llm_calls_authorized": False,
                },
            )

        try:
            pipeline = build_wired_paper_pipeline(
                llm_provider=self._llm_provider,
                source_id=request.source_id,
                keyword_top_k=request.keyword_top_k,
            )
            orch = PipelineOrchestrator(pipeline=pipeline, dispatch=SyncDispatch())
            from dataclasses import replace

            seed = replace(
                PipelineContext(
                    source_id=request.source_id,
                    stage_manifest=pipeline.manifest(),
                ),
                stage_outputs={"text_parts": list(text_parts)},
            )
            ctx = orch.run(seed)
        except Exception as exc:  # noqa: BLE001 — use-case boundary
            return ChunkExtractionResult(
                source_id=request.source_id,
                status="failed",
                diagnostic=f"chunk_extraction_failed:{type(exc).__name__}",
            )

        patch = ctx.stage_outputs.get("core_entity_extractor")
        entity_count = 0
        relation_count = 0
        if isinstance(patch, ExtractionPatch):
            entity_count = len(patch.entities)
            relation_count = len(patch.relations)
        elif patch is not None:
            # Tolerate duck-typed patches in tests
            entity_count = len(getattr(patch, "entities", ()) or ())
            relation_count = len(getattr(patch, "relations", ()) or ())

        return ChunkExtractionResult(
            source_id=request.source_id,
            status="done",
            entity_count=entity_count,
            relation_count=relation_count,
            extraction_patch=patch if isinstance(patch, ExtractionPatch) else None,
            stage_output_keys=tuple(sorted(ctx.stage_outputs.keys())),
            diagnostic=None,
        )


__all__ = [
    "ChunkExtractionRequest",
    "ChunkExtractionResult",
    "ChunkExtractionUseCase",
]
