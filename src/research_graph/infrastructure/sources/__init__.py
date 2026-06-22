"""Source infrastructure Adapters (D086/D88).

* :class:`~research_graph.infrastructure.sources.md_converter_adapter
  .MDConverterAdapter` wraps
  :class:`research_graph.infrastructure.corpus.sources.markdown_converter.MDConverter` behind
  :class:`research_graph.domain.ports.FullTextProviderPort`.
"""

from __future__ import annotations

from research_graph.infrastructure.sources.md_converter_adapter import MDConverterAdapter

__all__ = ["MDConverterAdapter"]
