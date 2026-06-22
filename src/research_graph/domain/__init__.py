"""Domain core: typed models and Ports (ADR-033, D086/D088 hexagonal overlay).

The ``domain`` package is the hexagonal Core — it depends only on the stdlib and
on typed schema models. It defines NO infrastructure (no LLM calls, no graph
writes, no network). The Port Protocols are the seams that infrastructure
Adapters implement; the application layer depends on the Ports, never on
concrete adapters (D086 Port rule).

D088: the full-text provider seam is :class:`FullTextProviderPort` (MDConverter
backends); the premature :class:`PDFParserPort` was removed (single
implementation, no planned migration).
"""

from __future__ import annotations

from research_graph.domain.ports import (
    ConversionResult,
    FullTextProviderPort,
    GraphDBPort,
    LLMClientPort,
)

__all__ = [
    "ConversionResult",
    "FullTextProviderPort",
    "GraphDBPort",
    "LLMClientPort",
]
