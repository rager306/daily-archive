"""Wave A continuity / operator readiness report.

Composes catalog↔hybrid body coverage (M241/M242) and preprocess fleet metrics into one fail-closed package with a non-authorizing readiness_signal.

readiness_signal is operator guidance only — never sets import_eligible.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

from research_graph.application.corpus.etl_body_coverage_audit import (
    EtlBodyCoveragePackage,
    audit_catalog_body_coverage,
)
from research_graph.application.corpus.etl_preprocess_fleet_audit import (
    EtlPreprocessFleetPackage,
    audit_preprocess_fleet,
)

SCHEMA_VERSION = "etl-continuity-readiness.v1"

ReadinessSignal = Literal["blocked", "repair", "ready_for_review"]


def derive_readiness_signal(
    *,
    hybrid_body_found: int,
    article_count: int,
    preprocess_body_count: int,
    preprocess_error_count: int,
    quality_ok: int,
    quality_soft: int,
    gaps: Sequence[str],
) -> ReadinessSignal:
    """Derive non-authorizing readiness signal from Wave A metrics.

    Rules (conservative, import never implied):
    - blocked: no hybrid bodies, missing index, or preprocess total failure
    - ready_for_review: hybrid_found >= 10, preprocess bodies match hybrid_found,
      preprocess errors 0, and soft_signal fraction <= 0.5 when bodies > 0
    - repair: everything else with partial progress
    """
    gap_set = set(gaps)
    if "catalog_index_missing" in gap_set:
        return "blocked"
    if hybrid_body_found <= 0 or "no_hybrid_bodies_under_body_roots" in gap_set:
        return "blocked"
    if preprocess_body_count <= 0 and hybrid_body_found > 0:
        return "blocked"
    if preprocess_error_count > 0 and preprocess_error_count >= preprocess_body_count:
        return "blocked"

    soft_frac = 0.0
    total_q = quality_ok + quality_soft
    if total_q > 0:
        soft_frac = quality_soft / total_q

    if (
        hybrid_body_found >= 10
        and preprocess_body_count >= hybrid_body_found
        and preprocess_error_count == 0
        and soft_frac <= 0.5
    ):
        return "ready_for_review"

    return "repair"


@dataclass(frozen=True, slots=True)
class EtlContinuityReadinessPackage:
    schema_version: str
    readiness_signal: ReadinessSignal
    coverage: EtlBodyCoveragePackage
    preprocess: EtlPreprocessFleetPackage
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("continuity readiness cannot authorize import/writes")
        if self.coverage.import_eligible or self.coverage.graph_writes_allowed:
            raise ValueError("coverage package cannot authorize import inside continuity")
        if self.preprocess.import_eligible or self.preprocess.graph_writes_allowed:
            raise ValueError(
                "preprocess package cannot authorize import inside continuity"
            )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "readiness_signal": self.readiness_signal,
            "coverage": self.coverage.to_dict(),
            "preprocess": self.preprocess.to_dict(),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A continuity report only; readiness_signal is not import "
                "authorization; not graph write; not extraction quality claim"
            ),
        }


def build_continuity_readiness(
    *,
    catalog_index_path: Path,
    body_roots: Sequence[Path],
    catalog_root: Path | None = None,
    sample_limit: int = 12,
) -> EtlContinuityReadinessPackage:
    """Build composed Wave A continuity readiness package (read-only)."""
    coverage = audit_catalog_body_coverage(
        catalog_index_path=catalog_index_path,
        body_roots=body_roots,
        catalog_root=catalog_root,
        sample_limit=sample_limit,
    )
    preprocess = audit_preprocess_fleet(
        body_roots=body_roots,
        sample_limit=sample_limit,
    )
    q = preprocess.quality_status_counts
    signal = derive_readiness_signal(
        hybrid_body_found=coverage.hybrid_body_found,
        article_count=coverage.article_count,
        preprocess_body_count=preprocess.body_count,
        preprocess_error_count=preprocess.error_count,
        quality_ok=int(q.get("ok", 0)),
        quality_soft=int(q.get("soft_signal", 0)),
        gaps=coverage.gaps,
    )
    diagnostics = (
        f"readiness_signal:{signal}",
        f"hybrid_found:{coverage.hybrid_body_found}",
        f"preprocess_bodies:{preprocess.body_count}",
        f"preprocess_errors:{preprocess.error_count}",
        "import_write_fail_closed",
        "wave_a_continuity_only",
    )
    return EtlContinuityReadinessPackage(
        schema_version=SCHEMA_VERSION,
        readiness_signal=signal,
        coverage=coverage,
        preprocess=preprocess,
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "EtlContinuityReadinessPackage",
    "ReadinessSignal",
    "build_continuity_readiness",
    "derive_readiness_signal",
]
