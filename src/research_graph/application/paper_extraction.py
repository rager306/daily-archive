"""Application-owned one-paper semantic extraction pilot (M201 S04).

Runs preserved paper chunks through :func:`build_wired_paper_pipeline` with an
injected :class:`~research_graph.domain.ports.LLMClientPort`, seeds
``evidence_anchor`` for :class:`~research_graph.application.primitives.EvidenceLinker`,
and returns fail-closed candidate :class:`~research_graph.domain.schema.ExtractionPatch`.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field

from research_graph.application.extraction_failures import (
    ExtractionFailureRecord,
    classify_extraction_failure,
)
from research_graph.application.orchestrator import PipelineOrchestrator, SyncDispatch
from research_graph.application.profiles.paper import build_wired_paper_pipeline
from research_graph.application.types import PipelineContext
from research_graph.domain.ports import LLMClientPort
from research_graph.domain.schema import ExtractionPatch, TypedEntity, TypedRelation


@dataclass(frozen=True)
class PaperChunk:
    """One preserved structure unit for pilot extraction."""

    text: str
    semantic_chunk_id: str
    page_index_node_id: str = ""
    node_path: tuple[str, ...] = ()


@dataclass(frozen=True)
class PaperExtractionRequest:
    source_id: str
    chunks: Sequence[PaperChunk]
    keyword_top_k: int = 20


@dataclass(frozen=True)
class PaperExtractionResult:
    source_id: str
    status: str  # done | empty | failed
    entity_count: int = 0
    relation_count: int = 0
    evidence_linked_count: int = 0
    extraction_patch: ExtractionPatch | None = None
    stage_output_keys: tuple[str, ...] = ()
    diagnostic: str | None = None
    failure: ExtractionFailureRecord | None = None
    safety: dict[str, bool] = field(
        default_factory=lambda: {
            "graph_writes_authorized": False,
            "production_import_authorized": False,
            "fact_promotion_authorized": False,
            "external_network_authorized": False,
            "llm_calls_authorized": True,
        }
    )


class PaperExtractionUseCase:
    """One-paper typed candidate extraction over the paper pipeline."""

    def __init__(self, *, llm_provider: LLMClientPort) -> None:
        self._llm_provider = llm_provider

    def run(self, request: PaperExtractionRequest) -> PaperExtractionResult:
        chunks = [c for c in request.chunks if c.text and c.text.strip()]
        if not chunks:
            return PaperExtractionResult(
                source_id=request.source_id,
                status="empty",
                diagnostic="empty_chunks",
                failure=classify_extraction_failure(
                    status="empty", diagnostic="empty_chunks"
                ),
                safety={
                    "graph_writes_authorized": False,
                    "production_import_authorized": False,
                    "fact_promotion_authorized": False,
                    "external_network_authorized": False,
                    "llm_calls_authorized": False,
                },
            )

        text_parts = [c.text for c in chunks]
        # Anchor evidence to the first chunk (stable pilot policy; multi-anchor later).
        anchor_chunk = chunks[0]
        evidence_anchor = {
            "semantic_chunk_id": anchor_chunk.semantic_chunk_id,
            "page_index_node_id": anchor_chunk.page_index_node_id,
            "node_path": list(anchor_chunk.node_path),
        }

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
                stage_outputs={
                    "text_parts": text_parts,
                    "evidence_anchor": evidence_anchor,
                },
            )
            ctx = orch.run(seed)
        except Exception as exc:  # noqa: BLE001
            diagnostic = f"paper_extraction_failed:{type(exc).__name__}"
            return PaperExtractionResult(
                source_id=request.source_id,
                status="failed",
                diagnostic=diagnostic,
                failure=classify_extraction_failure(
                    status="failed", diagnostic=diagnostic
                ),
            )

        # Prefer evidence-linked patch when present.
        patch = ctx.stage_outputs.get("evidence_linker")
        if patch is None:
            patch = ctx.stage_outputs.get("core_entity_extractor")

        entities: list[TypedEntity] = []
        relations: list[TypedRelation] = []
        if isinstance(patch, ExtractionPatch):
            entities = list(patch.entities)
            relations = list(patch.relations)
        elif patch is not None:
            entities = list(getattr(patch, "entities", ()) or ())
            relations = list(getattr(patch, "relations", ()) or ())

        evidence_linked = sum(1 for e in entities if getattr(e, "evidence_path", None) is not None)
        evidence_linked += sum(1 for r in relations if getattr(r, "evidence_path", None) is not None)

        client_diag = getattr(self._llm_provider, "last_diagnostics", None)
        if not isinstance(client_diag, dict):
            client_diag = None
        failure = classify_extraction_failure(
            status="done",
            entity_count=len(entities),
            relation_count=len(relations),
            evidence_linked_count=evidence_linked,
            client_diagnostics=client_diag,
        )
        return PaperExtractionResult(
            source_id=request.source_id,
            status="done",
            entity_count=len(entities),
            relation_count=len(relations),
            evidence_linked_count=evidence_linked,
            extraction_patch=patch if isinstance(patch, ExtractionPatch) else None,
            stage_output_keys=tuple(sorted(ctx.stage_outputs.keys())),
            diagnostic=None if failure is None else failure.message,
            failure=failure,
        )


__all__ = [
    "PaperChunk",
    "PaperExtractionRequest",
    "PaperExtractionResult",
    "PaperExtractionUseCase",
]
