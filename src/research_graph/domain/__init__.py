"""Domain core: typed models and Ports (ADR-033, D086 hexagonal overlay).

The ``domain`` package is the hexagonal Core — it depends only on the stdlib and
on typed schema models. It defines NO infrastructure (no LLM calls, no graph
writes, no parsing). The :class:`~research_graph.domain.ports.LLMClientPort`,
:class:`~research_graph.domain.ports.GraphDBPort`, and
:class:`~research_graph.domain.ports.PDFParserPort` Protocols are the seams that
infrastructure Adapters implement; the application layer depends on the Ports,
never on concrete adapters (D086 Port rule).
"""

from __future__ import annotations

from research_graph.domain.ports import (
    GraphDBPort,
    LLMClientPort,
    PDFParserPort,
)

__all__ = [
    "GraphDBPort",
    "LLMClientPort",
    "PDFParserPort",
]
