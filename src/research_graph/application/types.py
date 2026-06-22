"""Level 1 universal pipeline primitives (ADR-033 Step 3).

This module is the domain-agnostic foundation of the typed extraction pipeline.
It defines the contracts every stage carries and the seam that makes the
pipeline compatible with the existing :class:`UniversalKBQueue` without
duplicating queue logic (D085).

Design rules:

* **stdlib dataclasses** (``@dataclass(frozen=True)``) — no Pydantic for
  pipeline types (ADR-033 §2.4).
* **Fail-closed by default** — no graph writes, no import authorization; this
  module is infrastructure, not a safety surface, but stages it frames must
  never bypass the Review Gate (ADR-005, §6.3 invariant #4).
* **Queue-compatible seams** — ``PipelineStage.stage_name`` maps to
  ``UniversalKBQueue.ProcessingJob.stage``; ``StageManifest`` is
  queue-submittable (stage, contract_version, output_contract) per D085.
* **Phase-2 scope only** — full 3-lane scheduler (ADR-027 §5) and agent FSM
  (ADR-026) are NOT built here. ``ResourceState`` and ``StatisticalContext``
  are intentionally placeholders: they are filled in by later steps
  (ADR-024 Step 8, ADR-027 Phase 4), not constructed in this foundation slice.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Protocol, runtime_checkable

#: Schema/stage contract version carried by every stage manifest. Bumped when a
#: stage's output contract changes in a way that invalidates older queue rows
#: (D085: queue stale-detection keys on contract_version). Matches the
#: ``contract_version`` column already accepted by ``UniversalKBQueue.enqueue``.
PIPELINE_CONTRACT_VERSION: str = "pipeline.types.v1"


@dataclass(frozen=True)
class ResourceProfile:
    """Declares what resources a stage/job needs (ADR-027 §2.2).

    Carried by every :class:`PipelineStage` so the future 3-lane scheduler can
    do admission control without inspecting stage internals. Phase 2 uses only
    the ``llm_*`` fields (simple LLM-lane check, ADR-027 §5); the ``cpu_*`` and
    ``io_*`` fields exist now so adding CPU/IO lanes in Phase 4 is a scheduler
    change, not a pipeline rewrite.

    Field names and semantics match ADR-027 §2.2 exactly so this type can be
    serialized into a queue row without an adapter.
    """

    # LLM requirements
    llm_required: bool = False
    llm_provider: str | None = None  # "minimax" | "glm" | None = any
    estimated_tokens: int = 0

    # CPU requirements
    cpu_required: bool = False
    cpu_intensity: str = "light"  # "light" | "medium" | "heavy"

    # I/O requirements
    io_required: bool = False
    io_type: str | None = None  # "network" | "disk" | "graph_write"


@dataclass(frozen=True)
class PipelineContext:
    """Mutable-per-stage carrier threaded through a :class:`Pipeline` run.

    Holds the source identity, a placeholder for the statistical pre-processing
    context (filled by ADR-024 Step 8 — ``statistical_context.py``), the
    assembled stage manifest, and a placeholder for the resource-state snapshot
    the scheduler will publish (ADR-027 §5, Phase 4). All placeholders are
    ``None`` in this foundation slice; stages must tolerate absence.

    ``stage_outputs`` accumulates each stage's result keyed by ``stage_name`` so
    downstream stages and tests can inspect the chain deterministically.
    """

    source_id: str
    stage_manifest: tuple[StageManifest, ...] = ()
    statistical_context: Any | None = None  # placeholder: ADR-024 Step 8
    resource_state: Any | None = None  # placeholder: ADR-027 §5 Phase 4
    stage_outputs: dict[str, Any] = field(default_factory=dict)

    def with_output(self, stage_name: str, output: Any) -> PipelineContext:
        """Return a copy of the context with one stage's output recorded.

        Frozen-dataclass-safe: returns a new instance rather than mutating.
        """
        new_outputs = {**self.stage_outputs, stage_name: output}
        return dataclass_replace(self, stage_outputs=new_outputs)


def dataclass_replace(instance: PipelineContext, **changes: Any) -> PipelineContext:
    """``dataclasses.replace`` wrapper kept local for readability."""
    from dataclasses import replace

    return replace(instance, **changes)


@dataclass(frozen=True)
class StageManifest:
    """Queue-submittable description of a single stage (D085).

    Mirrors the fields ``UniversalKBQueue`` needs to enqueue and stale-detect a
    job: a queue-compatible ``stage`` name, a ``contract_version`` (so evolving
    a stage's output contract invalidates stale rows), the stage's
    :class:`ResourceProfile`, and a human-readable ``output_contract`` string.

    This is deliberately metadata, not behaviour: it serializes cleanly to a
    queue row and lets the orchestrator decide dispatch without importing the
    stage implementation.
    """

    stage_name: str
    contract_version: str
    resource_profile: ResourceProfile
    output_contract: str


@runtime_checkable
class PipelineStage(Protocol):
    """Abstract contract for a single processing stage.

    Every stage exposes a queue-compatible ``stage_name`` (matching
    ``UniversalKBQueue.ProcessingJob.stage``), a :class:`ResourceProfile`
    declaring its lane, and a pure ``run`` that transforms a
    :class:`PipelineContext`. Stages must be fail-closed: they never authorize
    graph writes; their outputs are candidate evidence, not truth (§6.3 #4).

    Concrete stages are frozen dataclasses implementing this protocol
    structurally (no inheritance required) — see :mod:`research_graph.application
    .primitives` (ADR-033 Step 4, M103 S02 T02).
    """

    stage_name: str
    resource_profile: ResourceProfile

    def run(self, context: PipelineContext) -> PipelineContext:
        """Execute the stage, returning an updated context.

        Implementations must be deterministic given their inputs and must not
        perform graph writes or import authorization. LLM-calling stages gate
        every call behind the statistical-first pre-processor (ADR-024) and the
        Phase-2 LLM-lane check (ADR-027 §5).
        """
        ...


@dataclass(frozen=True)
class Pipeline:
    """Ordered sequence of stages executed synchronously (ADR-033 Level 1).

    ``run`` threads a :class:`PipelineContext` through each stage in declared
    order and exposes every stage's :class:`ResourceProfile` so a future
    scheduler can read admission requirements without coupling to stage code.

    This is synchronous only (ADR-017: queue deferred). Queue-backed dispatch
    is a strategy on the orchestrator (D085), not on the pipeline itself.
    """

    stages: tuple[PipelineStage, ...] = ()
    source_id: str = ""

    def with_stage(self, stage: PipelineStage) -> Pipeline:
        """Return a copy of the pipeline with ``stage`` appended."""
        new_stages = (*self.stages, stage)
        return Pipeline(stages=new_stages, source_id=self.source_id)

    def resource_profiles(self) -> tuple[tuple[str, ResourceProfile], ...]:
        """Per-stage ``(stage_name, resource_profile)`` for scheduler admission."""
        return tuple((stage.stage_name, stage.resource_profile) for stage in self.stages)

    def manifest(
        self, contract_version: str = PIPELINE_CONTRACT_VERSION
    ) -> tuple[StageManifest, ...]:
        """Build a queue-submittable manifest for every stage (D085)."""
        return tuple(
            StageManifest(
                stage_name=stage.stage_name,
                contract_version=contract_version,
                resource_profile=stage.resource_profile,
                output_contract=stage.resource_profile.io_type or "typed_drafts",
            )
            for stage in self.stages
        )

    def run(self, context: PipelineContext | None = None) -> PipelineContext:
        """Execute all stages in order, threading the context through.

        Raises ``ValueError`` if the pipeline has no stages. The initial context
        defaults to one carrying this pipeline's ``source_id`` and manifest.
        """
        if not self.stages:
            raise ValueError("Pipeline.run requires at least one stage")
        ctx = (
            context
            if context is not None
            else PipelineContext(
                source_id=self.source_id,
                stage_manifest=self.manifest(),
            )
        )
        for stage in self.stages:
            ctx = stage.run(ctx)
        return ctx


__all__ = [
    "PIPELINE_CONTRACT_VERSION",
    "Pipeline",
    "PipelineContext",
    "PipelineStage",
    "ResourceProfile",
    "StageManifest",
]
