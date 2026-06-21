"""Graph infrastructure Adapters (D086).

* :class:`~research_graph.infrastructure.graph.ladybug_adapter.LadybugAdapter`
  wraps :mod:`research_graph.graph.ladybug_client` behind
  :class:`~research_graph.domain.ports.GraphDBPort`. A FalkorDB adapter arrives
  in Phase 3 (ADR-030) and implements the same Port.
"""

from __future__ import annotations

from research_graph.infrastructure.graph.ladybug_adapter import LadybugAdapter

__all__ = ["LadybugAdapter"]
