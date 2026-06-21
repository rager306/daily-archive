"""Level 4 orchestration with dispatch strategy seam (ADR-033 Step 6, D085).

The orchestrator runs a :class:`~research_graph.pipeline.types.Pipeline`
**synchronously by default** (ADR-017: full queue activation deferred to Phase
4) but exposes a ``dispatch_protocol`` strategy seam so queue-backed dispatch
becomes a strategy swap, not a pipeline rewrite.

Two strategies ship here (D085):

* :class:`SyncDispatch` (default) — calls ``stage.run(context)`` directly.
* :class:`QueueDispatch` — a THIN adapter over the existing
  :class:`~research_graph.workflows.universal_kb.queue.UniversalKBQueue`
  (``enqueue → claim → complete``). It reuses the queue; it does NOT duplicate
  queue logic. Queue-backed dispatch is not activated in Phase 2 (ADR-017), but
  the adapter exists so Phase 4 activation is a strategy swap.

Resource-aware admission (:func:`can_dispatch`) is an injectable LLM-lane check
callable. Phase 2 implements ONLY the simple LLM-lane hook (ADR-027 §5); the
full 3-lane scheduler (CPU/IO) is deferred to Phase 4. NOT built here: full
3-lane scheduler, agent FSM (ADR-026, Phase 6), PostgreSQL migration (M066).
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any, Protocol, runtime_checkable

from research_graph.pipeline.types import (
    PIPELINE_CONTRACT_VERSION,
    Pipeline,
    PipelineContext,
    PipelineStage,
    ResourceProfile,
)

#: Injectable LLM-lane admission callable: ``(profile, context) -> bool``.
#: Returns True when the LLM lane is free for this stage. Phase 2 default is
#: the trivial always-True hook (no live provider); a real MiniMax/GLM
#: token-plan check (ADR-027 §5) is injected by the orchestrator caller.
LLMLaneCheck = Callable[[ResourceProfile, PipelineContext], bool]


def _default_llm_lane_check(profile: ResourceProfile, context: PipelineContext) -> bool:
    """Phase 2 trivial hook: never blocks (no live provider in foundation slice).

    A real check (ADR-027 §5: MiniMax token_plan before each call) replaces
    this callable in Phase 4 scheduler activation.
    """
    return True


@runtime_checkable
class DispatchProtocol(Protocol):
    """Strategy seam for how a stage is executed (D085).

    Sync vs queue-backed dispatch is a strategy, not a pipeline property. The
    orchestrator holds one :class:`DispatchProtocol` and delegates every stage
    to it. Adding a new strategy (e.g. ``PrefectDispatch`` if Phase 4 profiling
    demands it, per D087) means implementing this protocol — no pipeline edit.
    """

    def admit(self, stage: PipelineStage, context: PipelineContext) -> bool:
        """Return True when the stage's resource lane can be dispatched now."""
        ...

    def dispatch(self, stage: PipelineStage, context: PipelineContext) -> PipelineContext:
        """Execute ``stage`` and return the updated context."""
        ...


@dataclass(frozen=True)
class SyncDispatch:
    """Default synchronous dispatch (ADR-017: queue deferred).

    Executes ``stage.run(context)`` directly. The LLM-lane admission check is
    injectable; Phase 2 uses the trivial always-True hook.
    """

    llm_lane_check: LLMLaneCheck = _default_llm_lane_check

    def admit(self, stage: PipelineStage, context: PipelineContext) -> bool:
        if stage.resource_profile.llm_required:
            return self.llm_lane_check(stage.resource_profile, context)
        return True

    def dispatch(self, stage: PipelineStage, context: PipelineContext) -> PipelineContext:
        return stage.run(context)


@dataclass(frozen=True)
class QueueDispatch:
    """Thin adapter over the existing :class:`UniversalKBQueue` (D085).

    Enqueues one job per stage, claims it as a worker, runs the stage, then
    completes the job with its output paths. REUSES the queue — does not
    duplicate its lease/retry/DAG logic. Not activated in Phase 2 (ADR-017);
    exists so Phase 4 queue activation is a strategy swap on the orchestrator.

    A real UniversalKBQueue instance is injected via ``queue``; the adapter
    only translates stage ↔ job rows. Stage output is still produced by calling
    ``stage.run`` in-process — the queue provides durability/admission, not
    remote execution (ADR-017 §2.4).
    """

    queue: Any  # UniversalKBQueue — typed loosely to avoid an infra import here
    worker_id: str = "orchestrator"
    contract_version: str = PIPELINE_CONTRACT_VERSION
    llm_lane_check: LLMLaneCheck = _default_llm_lane_check

    def admit(self, stage: PipelineStage, context: PipelineContext) -> bool:
        if stage.resource_profile.llm_required:
            return self.llm_lane_check(stage.resource_profile, context)
        return True

    def dispatch(self, stage: PipelineStage, context: PipelineContext) -> PipelineContext:
        job_id = f"{context.source_id}:{stage.stage_name}"
        self.queue.enqueue(
            job_id=job_id,
            stage=stage.stage_name,
            input_refs=(context.source_id,),
            input_hash=context.source_id,
            tool_version=stage.resource_profile.llm_provider or "cpu",
            contract_version=self.contract_version,
        )
        claimed = self.queue.claim(worker_id=self.worker_id, lease_seconds=300)
        try:
            if claimed is None:
                # No ready job — fall back to direct run (sync semantics preserved)
                return stage.run(context)
            result = stage.run(context)
            self.queue.complete(claimed["job_id"], worker_id=self.worker_id, output_paths=())
            return result
        except Exception:
            self.queue.fail_retryable(
                job_id,
                worker_id=self.worker_id,
                error_code="stage_dispatch_error",
                redacted_message="stage raised during queue-backed dispatch",
                retry_after="60s",
            )
            raise


def can_dispatch(
    stage: PipelineStage,
    context: PipelineContext,
    *,
    llm_lane_check: LLMLaneCheck = _default_llm_lane_check,
) -> bool:
    """Phase 2 resource-aware admission (ADR-027 §5).

    Checks ONLY the LLM lane (injectable). CPU/IO lanes arrive in Phase 4.
    Returns True for non-LLM stages; for LLM stages delegates to
    ``llm_lane_check``.
    """
    if stage.resource_profile.llm_required:
        return llm_lane_check(stage.resource_profile, context)
    return True


@dataclass(frozen=True)
class PipelineOrchestrator:
    """Runs a :class:`Pipeline` through a :class:`DispatchProtocol` (ADR-033 L4).

    SYNCHRONOUS by default (``dispatch`` = :class:`SyncDispatch`): threads a
    :class:`PipelineContext` through every stage in order, admitting each via
    the dispatch strategy. A stage that fails admission raises
    :class:`AdmissionError` (fail-closed — never silently skips).

    Swap ``dispatch`` to :class:`QueueDispatch` (Phase 4) or a future
    ``PrefectDispatch`` (D087) without touching the pipeline or its stages.
    """

    pipeline: Pipeline
    dispatch: DispatchProtocol = field(default_factory=SyncDispatch)

    def run(self, context: PipelineContext | None = None) -> PipelineContext:
        if not self.pipeline.stages:
            raise ValueError("PipelineOrchestrator requires a non-empty pipeline")
        ctx = (
            context
            if context is not None
            else PipelineContext(
                source_id=self.pipeline.source_id,
                stage_manifest=self.pipeline.manifest(),
            )
        )
        for stage in self.pipeline.stages:
            if not self.dispatch.admit(stage, ctx):
                raise AdmissionError(
                    f"stage {stage.stage_name!r} not admissible under current dispatch"
                )
            ctx = self.dispatch.dispatch(stage, ctx)
        return ctx


class AdmissionError(RuntimeError):
    """Raised when a stage cannot be admitted (fail-closed, never silent skip)."""


__all__ = [
    "AdmissionError",
    "DispatchProtocol",
    "LLMLaneCheck",
    "PipelineOrchestrator",
    "QueueDispatch",
    "SyncDispatch",
    "can_dispatch",
]
