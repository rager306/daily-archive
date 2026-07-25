"""Shadow runtime parity between legacy day metadata and analyze_source (M200 S02).

Compares **metadata only** (ids, counts, status) — never rewrites M001 session
or queue artifacts. Canonical path is :func:`analyze_source` /
:class:`AnalyzeSourceUseCase`; legacy path is represented by a pure
:class:`LegacyMetadataSnapshot` produced by the caller (typically from an
in-memory :class:`~research_graph.application.analysis.DailyAnalysis` or a
read-only load of existing files).

No filesystem writes. No network. Fail-closed: mismatches are reported, never
silently coerced to success.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass, field
from datetime import date
from typing import Literal

from research_graph.application.analyze_source import (
    AnalyzeSourceRequest,
    AnalyzeSourceResult,
    AnalyzeSourceUseCase,
)

ParityStatus = Literal["match", "mismatch", "skipped"]


@dataclass(frozen=True)
class LegacyMetadataSnapshot:
    """Metadata-only view of a legacy daily analysis run (M001 shape)."""

    run_date: date
    paper_ids: tuple[str, ...]
    status: str
    papers_fetched: int


@dataclass(frozen=True)
class SourceTextRecord:
    """One preserved source with structure already reduced to text_parts."""

    source_id: str
    text_parts: tuple[str, ...] = ()


@dataclass(frozen=True)
class ShadowParityReport:
    """Result of comparing legacy metadata to canonical analyze_source runs."""

    run_date: date
    match: bool
    status: ParityStatus
    legacy_paper_count: int
    canonical_done_count: int
    canonical_empty_count: int
    canonical_failed_count: int
    differences: tuple[str, ...] = ()
    canonical_results: tuple[AnalyzeSourceResult, ...] = ()
    safety: dict[str, bool] = field(
        default_factory=lambda: {
            "graph_writes_authorized": False,
            "production_import_authorized": False,
            "fact_promotion_authorized": False,
            "external_network_authorized": False,
            "llm_calls_authorized": False,
            "primary_pipeline_artifacts_mutated": False,
        }
    )


def compare_day_shadow(
    *,
    legacy: LegacyMetadataSnapshot,
    canonical_results: Sequence[AnalyzeSourceResult],
) -> ShadowParityReport:
    """Pure metadata compare — no I/O, no pipeline execution."""
    differences: list[str] = []
    results = tuple(canonical_results)

    done_ids = tuple(r.source_id for r in results if r.status == "done")
    empty_ids = tuple(r.source_id for r in results if r.status == "empty")
    failed_ids = tuple(r.source_id for r in results if r.status == "failed")

    legacy_ids = tuple(legacy.paper_ids)
    legacy_set = set(legacy_ids)

    if legacy.papers_fetched != len(legacy_ids):
        differences.append(
            f"legacy_internal_count_mismatch:papers_fetched={legacy.papers_fetched}"
            f":ids={len(legacy_ids)}"
        )

    if legacy.status == "empty" and not legacy_ids:
        # Both empty day — match if no canonical done/failed
        if done_ids or failed_ids:
            differences.append(
                f"legacy_empty_but_canonical_activity:done={len(done_ids)}:failed={len(failed_ids)}"
            )
    else:
        if len(legacy_ids) != len(done_ids) + len(empty_ids) + len(failed_ids):
            # Allow empty canonical for sources without text; count all results
            if len(legacy_ids) != len(results):
                differences.append(
                    f"count_mismatch:legacy={len(legacy_ids)}:canonical={len(results)}"
                )

        missing = sorted(legacy_set - {r.source_id for r in results})
        extra = sorted({r.source_id for r in results} - legacy_set)
        if missing:
            differences.append(f"missing_in_canonical:{','.join(missing)}")
        if extra:
            differences.append(f"extra_in_canonical:{','.join(extra)}")

        # For sources present in both: prefer done; empty is ok if no text
        for r in results:
            if r.source_id in legacy_set and r.status == "failed":
                differences.append(f"canonical_failed:{r.source_id}:{r.diagnostic or 'unknown'}")

    match = not differences
    return ShadowParityReport(
        run_date=legacy.run_date,
        match=match,
        status="match" if match else "mismatch",
        legacy_paper_count=len(legacy_ids),
        canonical_done_count=len(done_ids),
        canonical_empty_count=len(empty_ids),
        canonical_failed_count=len(failed_ids),
        differences=tuple(differences),
        canonical_results=results,
    )


def run_shadow_parity(
    *,
    legacy: LegacyMetadataSnapshot,
    sources: Sequence[SourceTextRecord],
    use_case: AnalyzeSourceUseCase | None = None,
) -> ShadowParityReport:
    """Run analyze_source per source, then pure-compare to legacy metadata.

    Does **not** write session/queue artifacts. ``primary_pipeline_artifacts_mutated`` stays False.
    """
    uc = use_case or AnalyzeSourceUseCase()
    results: list[AnalyzeSourceResult] = []
    for src in sources:
        results.append(
            uc.run(
                AnalyzeSourceRequest(
                    source_id=src.source_id,
                    text_parts=src.text_parts,
                )
            )
        )
    return compare_day_shadow(legacy=legacy, canonical_results=results)


__all__ = [
    "LegacyMetadataSnapshot",
    "ParityStatus",
    "ShadowParityReport",
    "SourceTextRecord",
    "compare_day_shadow",
    "run_shadow_parity",
]
