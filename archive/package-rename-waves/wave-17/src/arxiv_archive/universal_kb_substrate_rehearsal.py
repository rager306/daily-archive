# Formerly: src/arxiv_archive/universal_kb_substrate_rehearsal.py

"""No-write substrate rehearsal for M035 Universal KB readiness handoff.

This module intentionally does not select, initialize, or write to any GraphDB.
It only packages metadata-only readiness diagnostics for local inspection.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from typing import Any

from arxiv_archive.universal_kb_contracts import CandidatePacket, SafetyFlags, ToolInvocationRecord
from arxiv_archive.universal_kb_queue import UniversalKBQueue

_DIAGNOSTIC_STATES = frozenset({"pending", "diagnostic_only", "diagnostics_only", "needs_review"})


def _tuple(values: tuple[str, ...] | list[str] | None) -> tuple[str, ...]:
    if values is None:
        return ()
    return tuple(str(value) for value in values)


def _safe(value: Any) -> Any:
    if isinstance(value, SafetyFlags):
        return value.to_dict()
    if isinstance(value, tuple):
        return list(value)
    if hasattr(value, "__dataclass_fields__"):
        return {key: _safe(item) for key, item in asdict(value).items()}
    return value


@dataclass(frozen=True, slots=True)
class ReadinessHandoff:
    """Metadata-only handoff produced by the no-write rehearsal."""

    candidate_id: str
    candidate_type: str
    evidence_refs: tuple[str, ...]
    review_trace_ref: str
    queue_job_id: str
    queue_status: str
    model: str
    prompt_version: str
    readiness_state: str
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    dry_run_only: bool = True
    graph_write_allowed: bool = False
    promotion_allowed: bool = False
    production_import_attempted: bool = False

    def __post_init__(self) -> None:
        for field_name in (
            "candidate_id",
            "candidate_type",
            "review_trace_ref",
            "queue_job_id",
            "queue_status",
            "model",
            "prompt_version",
            "readiness_state",
        ):
            if not str(getattr(self, field_name)).strip():
                raise ValueError(f"{field_name} must be non-empty")
        object.__setattr__(self, "evidence_refs", _tuple(self.evidence_refs))
        if self.readiness_state not in _DIAGNOSTIC_STATES:
            raise ValueError("no-write rehearsal readiness must remain diagnostic-only")
        if not self.dry_run_only or self.graph_write_allowed or self.promotion_allowed or self.production_import_attempted:
            raise ValueError("no-write rehearsal cannot grant write, import, or promotion authority")
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return _safe(self)


class NoWriteSubstrateRehearsal:
    """Builds local readiness handoff metadata from queue and helper traces."""

    def __init__(self, queue: UniversalKBQueue) -> None:
        self.queue = queue

    def build_handoff(
        self,
        *,
        candidate: CandidatePacket,
        review_trace: ToolInvocationRecord,
        queue_job_id: str,
    ) -> ReadinessHandoff:
        candidate.assert_no_write()
        trace = review_trace.to_sanitized_dict()
        if not trace.get("helper_evidence_only") or trace.get("minimax_source_of_truth"):
            raise ValueError("review trace must remain helper evidence only")
        inspected = self.queue.inspect(queue_job_id)
        queue_status = str(inspected["job"]["status"])
        return ReadinessHandoff(
            candidate_id=candidate.candidate_id,
            candidate_type=candidate.candidate_type,
            evidence_refs=candidate.evidence_refs,
            review_trace_ref=review_trace.invocation_id,
            queue_job_id=queue_job_id,
            queue_status=queue_status,
            model=review_trace.model,
            prompt_version=review_trace.prompt_version,
            readiness_state="diagnostics_only",
        )
