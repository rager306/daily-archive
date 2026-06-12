"""Read-only RLM document workflow harness (M052 S01)."""

from __future__ import annotations

import copy
import datetime
import hashlib
import json
from dataclasses import InitVar, dataclass, field
from typing import Any, Literal

from arxiv_archive.article_artifact_minimax import request_article_artifact_classification
from arxiv_archive.article_artifact_reducer import _safety_defaults, merge_article_artifact_results
from arxiv_archive.article_artifact_worker import MockTransport, run_worker_pool

REDUCER_SCHEMA_VERSION = "m052-rlm-workflow.v1"
WorkflowStepType = Literal["section_navigate", "span_visit", "helper_invoke"]
_SAFETY_KEYS: tuple[str, ...] = (
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
)


def _canonical_json(value: Any) -> str:
    encoder = json.JSONEncoder(ensure_ascii=False, sort_keys=True, separators=(",", ":"), default=str)
    return encoder.encode(value)


def _sha256_prefix(value: Any, *, length: int = 16) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()[:length]


def _deterministic_timestamp(step_index: int, *, offset_ms: int = 0) -> str:
    instant = datetime.datetime(2000, 1, 1, tzinfo=datetime.timezone.utc)
    instant += datetime.timedelta(seconds=step_index, milliseconds=offset_ms)
    return instant.isoformat()


def _all_false_safety_defaults() -> dict[str, bool]:
    safety = _safety_defaults()
    return {key: bool(safety.get(key, False)) and False for key in _SAFETY_KEYS}


@dataclass(frozen=True)
class WorkflowTrajectoryStep:
    """One deterministic, sanitized RLM workflow step."""

    step_type: WorkflowStepType
    section_id: str | None = None
    span_id: str | None = None
    work_id: str | None = None
    started_at: str | None = None
    completed_at: str | None = None
    diagnostics: dict[str, Any] = field(default_factory=dict)
    step_id: str = field(init=False)
    safety_defaults: dict[str, bool] = field(init=False)
    run_id: InitVar[str] = ""
    step_index: InitVar[int] = 0

    def __post_init__(self, run_id: str, step_index: int) -> None:
        populated = [self.section_id is not None, self.span_id is not None, self.work_id is not None]
        if sum(populated) != 1:
            raise ValueError("exactly one of section_id, span_id, or work_id must be set")
        if self.step_type == "section_navigate" and self.section_id is None:
            raise ValueError("section_navigate requires section_id")
        if self.step_type == "span_visit" and self.span_id is None:
            raise ValueError("span_visit requires span_id")
        if self.step_type == "helper_invoke" and self.work_id is None:
            raise ValueError("helper_invoke requires work_id")
        content = {
            "run_id": run_id,
            "step_index": step_index,
            "step_type": self.step_type,
            "section_id": self.section_id,
            "span_id": self.span_id,
            "work_id": self.work_id,
            "diagnostics": self.diagnostics,
        }
        object.__setattr__(self, "step_id", _sha256_prefix(content))
        object.__setattr__(self, "safety_defaults", _all_false_safety_defaults())
        if self.started_at is None:
            object.__setattr__(self, "started_at", _deterministic_timestamp(step_index))
        if self.completed_at is None:
            object.__setattr__(self, "completed_at", _deterministic_timestamp(step_index, offset_ms=1))

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "step_id": self.step_id,
            "step_type": self.step_type,
            "section_id": self.section_id,
            "span_id": self.span_id,
            "work_id": self.work_id,
            "started_at": self.started_at,
            "completed_at": self.completed_at,
            "diagnostics": dict(self.diagnostics),
            "safety_defaults": dict(self.safety_defaults),
        }


@dataclass(frozen=True)
class WorkflowTrajectory:
    """Deterministic audit trail emitted by the RLM workflow harness."""

    run_id: str
    work_ids: tuple[str, ...]
    steps: tuple[WorkflowTrajectoryStep, ...]
    schema_version: str = REDUCER_SCHEMA_VERSION
    aggregate_safety_defaults: dict[str, bool] = field(default_factory=_all_false_safety_defaults)

    def __post_init__(self) -> None:
        object.__setattr__(self, "work_ids", tuple(self.work_ids))
        object.__setattr__(self, "steps", tuple(self.steps))
        object.__setattr__(self, "aggregate_safety_defaults", _all_false_safety_defaults())

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "run_id": self.run_id,
            "work_ids": list(self.work_ids),
            "steps": [step.to_sanitized_dict() for step in self.steps],
            "aggregate_safety_defaults": dict(self.aggregate_safety_defaults),
        }


@dataclass
class WorkflowResult:
    """Workflow trajectory plus M050 reducer aggregate and safety audit."""

    trajectory: WorkflowTrajectory
    aggregate_summary: dict[str, Any]
    safety_audit: dict[str, Any]

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "trajectory": self.trajectory.to_sanitized_dict(),
            "aggregate_summary": dict(self.aggregate_summary),
            "safety_audit": dict(self.safety_audit),
        }


def run_document_workflow(
    structure: dict[str, Any],
    page_index: Any,
    chunks: Any,
    evidence_paths: Any,
    *,
    run_id: str,
    max_steps: int = 16,
    max_candidates: int = 8,
) -> WorkflowResult:
    """Run section, span, then bounded helper phases over redacted structure."""
    del page_index, chunks, evidence_paths
    if not isinstance(structure, dict):
        raise TypeError("structure must be a dict")
    if not run_id:
        raise ValueError("run_id must be non-empty")
    if max_steps < 1:
        raise ValueError("max_steps must be positive")
    if max_candidates < 1:
        raise ValueError("max_candidates must be positive")

    steps: list[WorkflowTrajectoryStep] = []
    navigated_sections: list[dict[str, Any]] = []

    def append_step(step: WorkflowTrajectoryStep) -> bool:
        if len(steps) >= max_steps:
            return False
        steps.append(step)
        return True

    for section in _section_dicts(structure)[:8]:
        if len(steps) >= max_steps:
            break
        section_id = _string_or_none(section.get("section_id"))
        if section_id is None:
            continue
        navigated_sections.append(section)
        append_step(
            WorkflowTrajectoryStep(
                step_type="section_navigate",
                section_id=section_id,
                diagnostics={
                    "phase": "section_navigate",
                    "section_type": _string_or_none(section.get("section_type")),
                    "ordinal_path": list(section.get("ordinal_path", [])) if isinstance(section.get("ordinal_path"), list) else [],
                },
                run_id=run_id,
                step_index=len(steps),
            )
        )

    for section in navigated_sections:
        if len(steps) >= max_steps:
            break
        span_id, source = _span_for_section(structure, section)
        if span_id is None:
            continue
        append_step(
            WorkflowTrajectoryStep(
                step_type="span_visit",
                span_id=span_id,
                diagnostics={"phase": "span_visit", "source": source, "section_id": _string_or_none(section.get("section_id"))},
                run_id=run_id,
                step_index=len(steps),
            )
        )

    work_requests = []
    structures_by_work_id: dict[str, dict[str, Any]] = {}
    helper_sections = [section for section in navigated_sections if section.get("section_type") != "root"]
    for helper_index, section in enumerate(helper_sections[: min(4, max_steps // 4)]):
        if len(steps) >= max_steps:
            break
        subset = _synthetic_helper_structure(structure, section)
        request = request_article_artifact_classification(
            subset,
            max_candidates=max_candidates,
            run_id=f"{run_id}:helper:{helper_index}",
        )
        work_requests.append(request)
        structures_by_work_id[request.work_id] = subset
        append_step(
            WorkflowTrajectoryStep(
                step_type="helper_invoke",
                work_id=request.work_id,
                diagnostics={
                    "phase": "helper_invoke",
                    "binding_id": request.binding_id,
                    "model_id": request.model_id,
                    "section_id": _string_or_none(section.get("section_id")),
                    "synthetic_structure": True,
                },
                run_id=run_id,
                step_index=len(steps),
            )
        )

    if work_requests:
        completed = run_worker_pool(
            work_requests,
            structures=structures_by_work_id,
            transport=MockTransport(),
            max_workers=1,
        )
        aggregate_summary = merge_article_artifact_results([item.to_sanitized_dict() for item in completed])
    else:
        aggregate_summary = merge_article_artifact_results([])

    trajectory = WorkflowTrajectory(
        run_id=run_id,
        work_ids=tuple(request.work_id for request in work_requests),
        steps=tuple(steps),
    )
    return WorkflowResult(trajectory, aggregate_summary, _build_safety_audit(trajectory, aggregate_summary))


def _section_dicts(structure: dict[str, Any]) -> list[dict[str, Any]]:
    sections = structure.get("sections")
    return [section for section in sections if isinstance(section, dict)] if isinstance(sections, list) else []


def _paragraph_dicts(structure: dict[str, Any]) -> list[dict[str, Any]]:
    paragraphs = structure.get("paragraphs")
    return [paragraph for paragraph in paragraphs if isinstance(paragraph, dict)] if isinstance(paragraphs, list) else []


def _span_for_section(structure: dict[str, Any], section: dict[str, Any]) -> tuple[str | None, str]:
    section_id = _string_or_none(section.get("section_id"))
    for paragraph in _paragraph_dicts(structure):
        if _string_or_none(paragraph.get("section_id")) == section_id:
            span_id = _string_or_none(paragraph.get("span_id") or paragraph.get("paragraph_span_id"))
            if span_id is not None:
                return span_id, "paragraph"
    return _string_or_none(section.get("span_id")), "section_span_fallback"


def _synthetic_helper_structure(structure: dict[str, Any], section: dict[str, Any]) -> dict[str, Any]:
    subset = copy.deepcopy(structure)
    root_sections = [candidate for candidate in _section_dicts(structure) if candidate.get("section_type") == "root"]
    included_sections = root_sections + [section]
    included_ids = {_string_or_none(item.get("section_id")) for item in included_sections}
    included_ids.discard(None)
    subset["sections"] = included_sections
    for key in ("artifact_placeholders", "structured_markers", "scientific_markers", "paragraphs"):
        subset[key] = _filter_records_by_section(structure.get(key), included_ids)
    subset["safe_spans"] = _filter_safe_spans(structure, included_sections, subset)
    subset["paper_id"] = str(structure.get("paper_id", "unknown-paper"))
    subset["safety_flags"] = dict(structure.get("safety_flags", {}))
    return subset


def _filter_records_by_section(value: Any, section_ids: set[str | None]) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [copy.deepcopy(item) for item in value if isinstance(item, dict) and _string_or_none(item.get("section_id")) in section_ids]


def _filter_safe_spans(structure: dict[str, Any], included_sections: list[dict[str, Any]], subset: dict[str, Any]) -> list[dict[str, Any]]:
    span_ids = {_string_or_none(item.get("span_id")) for item in included_sections}
    span_ids.discard(None)
    for key in ("artifact_placeholders", "structured_markers", "scientific_markers", "paragraphs"):
        for item in subset.get(key, []):
            if isinstance(item, dict):
                span_ids.update(_string_or_none(item.get(span_key)) for span_key in ("span_id", "caption_span_id", "paragraph_span_id"))
    safe_spans = structure.get("safe_spans")
    if not isinstance(safe_spans, list):
        return []
    return [copy.deepcopy(span) for span in safe_spans if isinstance(span, dict) and _string_or_none(span.get("span_id")) in span_ids]


def _build_safety_audit(trajectory: WorkflowTrajectory, aggregate_summary: dict[str, Any]) -> dict[str, Any]:
    reducer_defaults = {key: aggregate_summary.get(key) for key in _SAFETY_KEYS}
    return {
        "all_step_safety_defaults_false": all(all(value is False for value in step.safety_defaults.values()) for step in trajectory.steps),
        "aggregate_safety_defaults": dict(trajectory.aggregate_safety_defaults),
        "reducer_safety_defaults": reducer_defaults,
        "all_reducer_safety_defaults_false": all(value is False for value in reducer_defaults.values()),
        "helper_output_is_review_only": True,
        "import_authority": "import is not authorized",
    }


def _string_or_none(value: Any) -> str | None:
    return value if isinstance(value, str) and value else None


__all__ = [
    "REDUCER_SCHEMA_VERSION",
    "WorkflowResult",
    "WorkflowTrajectory",
    "WorkflowTrajectoryStep",
    "run_document_workflow",
]
