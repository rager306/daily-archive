"""Paper domain pipeline profile (ADR-029 + ADR-033 Step 5).

Assembles the Core-then-Modes extraction chain for scientific papers:

    StatisticalPreProcessor → CoreEntityExtractor → BinaryRelationDetector
        → RelationTypeClassifier → EvidenceLinker

This is the paper domain's answer to EP-7 (Extraction Stages, ADR-029). All
five stages are the Level 2 building blocks from
:mod:`research_graph.pipeline.primitives`; this profile only orders and
configures them. LLM-calling stages stay stubbed (``llm_client=None``) until a
real client is injected by the orchestrator in M103 S03.
"""

from __future__ import annotations

from research_graph.pipeline.primitives import (
    BinaryRelationDetector,
    CoreEntityExtractor,
    EvidenceLinker,
    RelationTypeClassifier,
    StatisticalPreProcessor,
)
from research_graph.pipeline.types import Pipeline

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
    drafts until the S03 prototype injects a real MiniMax client.
    """
    stages = (
        StatisticalPreProcessor(keyword_top_k=keyword_top_k),
        CoreEntityExtractor(),  # stubbed: llm_client=None
        BinaryRelationDetector(),
        RelationTypeClassifier(),  # stubbed: llm_client=None
        EvidenceLinker(),
    )
    return Pipeline(stages=stages, source_id=source_id)


__all__ = ["PAPER_STAGE_ORDER", "build_paper_pipeline"]
