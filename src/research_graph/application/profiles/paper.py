"""Paper domain pipeline profile (ADR-029 + ADR-033 Step 5).

Assembles the Core-then-Modes extraction chain for scientific papers:

    StatisticalPreProcessor → CoreEntityExtractor → BinaryRelationDetector
        → RelationTypeClassifier → EvidenceLinker

This is the paper domain's answer to EP-7 (Extraction Stages, ADR-029). All
five stages are the Level 2 building blocks from
:mod:`research_graph.application.primitives`; this profile only orders and
configures them. LLM-calling stages stay stubbed (``llm_client=None``) until a
real client is injected by the orchestrator in M103 S03.
"""

from __future__ import annotations

from typing import Any

from research_graph.application.primitives import (
    BinaryRelationDetector,
    CoreEntityExtractor,
    EvidenceLinker,
    KeywordExtractorFn,
    RelationTypeClassifier,
    StatisticalPreProcessor,
)
from research_graph.application.types import Pipeline
from research_graph.domain.ports import LLMClientPort

#: Stage order for the paper extraction pipeline (Core-then-Modes, ADR-029).
#: Order is load-bearing: the statistical pre-processor must run before any
#: LLM stage (ADR-024 statistical-first invariant, §6.3 #1).
PAPER_STAGE_ORDER: tuple[str, ...] = (
    "statistical_pre_processor",
    "core_entity_extractor",
    "binary_relation_detector",
    "relation_type_classifier",
    "evidence_linker",
)


def build_paper_pipeline(*, source_id: str = "", keyword_top_k: int = 20) -> Pipeline:
    """Build the paper-domain extraction pipeline (ADR-029, ADR-033 Step 5).

    Returns a synchronous :class:`Pipeline` whose stages run in
    :data:`PAPER_STAGE_ORDER`. LLM stages are stubbed (no ``llm_client``) —
    they declare the LLM lane and the Adaptix seam but emit empty fail-closed
    drafts until the M103 S03 prototype (or :func:`build_wired_paper_pipeline`)
    injects a real client.
    """
    stages = (
        StatisticalPreProcessor(keyword_top_k=keyword_top_k),
        CoreEntityExtractor(),  # stubbed: llm_client=None
        BinaryRelationDetector(),
        RelationTypeClassifier(),  # stubbed: llm_client=None
        EvidenceLinker(),
    )
    return Pipeline(stages=stages, source_id=source_id)


def build_wired_paper_pipeline(
    *,
    llm_provider: LLMClientPort | None = None,
    keyword_extractor: KeywordExtractorFn | None = None,
    source_id: str = "",
    keyword_top_k: int = 20,
) -> Pipeline:
    """Composition root (D086): wire Ports + infrastructure callables into the pipeline.

    The single wiring point for the paper use case. It adapts the domain
    :class:`LLMClientPort` (``extract(prompt, kind, *, context)``) to the
    ``Callable[[str, dict], dict]`` the application stages expect, and injects
    it into the LLM stages. It ALSO injects the keyword-extraction callable
    (the concrete YAKE :class:`KeywordExtractor` is infrastructure; this is the
    ONE place it touches the application — through injection, never import).

    With both ``llm_provider`` and ``keyword_extractor`` ``None`` it falls back
    to the stub pipeline (:func:`build_paper_pipeline`) — back-compatible.
    Infrastructure code (prototype script, future CLI) calls this with concrete
    adapters; the application stages never see concrete types, only the
    Port/callable contracts threaded through this function.
    """
    if llm_provider is None and keyword_extractor is None:
        return build_paper_pipeline(source_id=source_id, keyword_top_k=keyword_top_k)

    llm_client = _adapt_llm_provider(llm_provider) if llm_provider is not None else None
    stages = (
        StatisticalPreProcessor(keyword_top_k=keyword_top_k, keyword_extractor=keyword_extractor),
        CoreEntityExtractor(llm_client=llm_client),
        BinaryRelationDetector(),
        RelationTypeClassifier(llm_client=llm_client),
        EvidenceLinker(),
    )
    return Pipeline(stages=stages, source_id=source_id)


def _adapt_llm_provider(provider: LLMClientPort):
    """Adapt LLMClientPort.extract to the Callable[prompt, snapshot]->dict stages expect."""

    def _client(prompt: str, snapshot: dict[str, Any]) -> dict[str, Any]:
        kind = snapshot.get("extraction_kind", "entities")
        return provider.extract(prompt, kind, context=snapshot)

    return _client


__all__ = ["PAPER_STAGE_ORDER", "build_paper_pipeline", "build_wired_paper_pipeline"]
