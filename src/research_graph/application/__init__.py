"""Modular typed pipeline framework (ADR-033).

Level 1 universal primitives live in :mod:`research_graph.application.types`
(``ResourceProfile``, ``PipelineContext``, ``PipelineStage``, ``StageManifest``,
``Pipeline``). They are domain-agnostic infrastructure: no LLM calls, no graph
writes, no schema coupling beyond passing typed drafts through.

Higher levels (primitives, profiles, orchestrator) are added by later slices
per ADR-033 §2.5 and D085 (queue/scheduler seams).
"""

from __future__ import annotations

from research_graph.application.types import (
    Pipeline,
    PipelineContext,
    PipelineStage,
    ResourceProfile,
    StageManifest,
)

__all__ = [
    "Pipeline",
    "PipelineContext",
    "PipelineStage",
    "ResourceProfile",
    "StageManifest",
]
