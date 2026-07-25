"""Optional composition wire for paper summary stage (M254 S04 / M255 S03).

Stage is OFF by default. Application package remains pure;
this module exposes enablement policy and an optional runner that may call
an injectible summarizer only when explicitly enabled.
"""

from __future__ import annotations

from typing import Any, Protocol

from research_graph.application.corpus.paper_summary_stage import (
    PaperSummaryStagePackage,
    build_paper_summary_stage,
)

DEFAULT_SUMMARY_STAGE_ENABLED = False


class _SummarizerPort(Protocol):
    def summarize(self, title: str, abstract: str) -> Any: ...


def should_run_summary_stage(*, enabled: bool | None = None) -> bool:
    """Return whether optional summary stage should run.

    ``None`` → default off. Explicit True required to enable.
    """
    if enabled is None:
        return DEFAULT_SUMMARY_STAGE_ENABLED
    return bool(enabled)


def run_optional_paper_summary_stage(
    *,
    paper_id: str,
    title: str,
    abstract: str,
    enabled: bool | None = None,
    summarizer: _SummarizerPort | None = None,
    role: str = "default",
    binding_id: str = "paper-summary-generate-default",
    model_name: str = "agnes-ai/agnes-2.0-flash",
) -> PaperSummaryStagePackage | None:
    """Run optional summary stage.

    Returns ``None`` when disabled. When enabled, requires ``summarizer`` and
    builds a fail-closed :class:`PaperSummaryStagePackage` (import always false).
    """
    if not should_run_summary_stage(enabled=enabled):
        return None
    if summarizer is None:
        return build_paper_summary_stage(
            paper_id=paper_id,
            title=title,
            abstract=abstract,
            headline="",
            what_it_does="",
            why_it_matters="",
            analogy="",
            binding_id=binding_id,
            model_name=model_name,
            role=role,
            error="not_generated",
        )
    try:
        summary = summarizer.summarize(title, abstract)
        return build_paper_summary_stage(
            paper_id=paper_id,
            title=title,
            abstract=abstract,
            headline=str(getattr(summary, "headline", "") or ""),
            what_it_does=str(getattr(summary, "what_it_does", "") or ""),
            why_it_matters=str(getattr(summary, "why_it_matters", "") or ""),
            analogy=str(getattr(summary, "analogy", "") or ""),
            binding_id=binding_id,
            model_name=model_name,
            role=role,
            error=None,
        )
    except Exception as exc:  # noqa: BLE001 - stage fail-closed
        return build_paper_summary_stage(
            paper_id=paper_id,
            title=title,
            abstract=abstract,
            headline="",
            what_it_does="",
            why_it_matters="",
            analogy="",
            binding_id=binding_id,
            model_name=model_name,
            role=role,
            error=f"summary_failed:{type(exc).__name__}",
        )


__all__ = [
    "DEFAULT_SUMMARY_STAGE_ENABLED",
    "run_optional_paper_summary_stage",
    "should_run_summary_stage",
]
