"""Statistical pre-processing context (ADR-024, ADR-033 Step 8).

Canonical home for the deterministic statistical context that precedes every
LLM extraction call (statistical-first invariant, §6.3 #1). The
:class:`StatisticalContext` carries YAKE keywords and co-occurrence pairs
produced by the CPU-lane :class:`~research_graph.pipeline.primitives
.StatisticalPreProcessor` and consumed by LLM-lane stages as prompt grounding.

This is the ADR-033 §2.6 ``evaluation/statistical_context.py`` module. The
pipeline primitives re-export this type so callers can import it from either
location (schema evolution, not duplication — §6.3 #6).
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class StatisticalContext:
    """Deterministic statistical pre-processing output (ADR-024).

    All fields are deterministic: no LLM, no embeddings in this foundation
    module. An optional embedding callable is a future ADR-024 enhancement;
    for now the LLM receives keyword/co-occurrence grounding only.

    Consumed by the LLM-lane extraction stages
    (:class:`~research_graph.pipeline.primitives.CoreEntityExtractor`,
    :class:`~research_graph.pipeline.primitives.RelationTypeClassifier`) which
    never call the LLM without this context (statistical-first).
    """

    keywords: tuple[tuple[str, float], ...] = ()
    co_occurrence: tuple[tuple[str, str, int], ...] = ()  # (term_a, term_b, count)


__all__ = ["StatisticalContext"]
