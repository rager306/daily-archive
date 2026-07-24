"""Wave A ETL continuity pack: composed dashboard for operators.

Composes body coverage (incl. multi-root taxonomy), hybrid-missing PDF readiness,
and Wave A closeout into one fail-closed report. Never authorizes import or batch.
"""

from __future__ import annotations

from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.etl_body_coverage_audit import EtlBodyCoveragePackage
from research_graph.application.corpus.etl_hybrid_missing_pdf_readiness import (
    HybridMissingPdfReadinessPackage,
)
from research_graph.application.corpus.wave_a_closeout import WaveACloseoutPackage

SCHEMA_VERSION = "etl-continuity-pack.v1"


@dataclass(frozen=True, slots=True)
class EtlContinuityPackPackage:
    schema_version: str
    coverage: EtlBodyCoveragePackage
    pdf_readiness: HybridMissingPdfReadinessPackage
    closeout: WaveACloseoutPackage
    dashboard: dict[str, Any]
    alerts: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("etl continuity pack cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "dashboard": dict(self.dashboard),
            "alerts": list(self.alerts),
            "coverage": self.coverage.to_dict(),
            "pdf_readiness": self.pdf_readiness.to_dict(),
            "closeout": self.closeout.to_dict(),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave A continuity pack / operator dashboard only. "
                "Alerts are observational; never import authorization."
            ),
        }


def build_etl_continuity_pack(
    *,
    coverage: EtlBodyCoveragePackage,
    pdf_readiness: HybridMissingPdfReadinessPackage,
    closeout: WaveACloseoutPackage,
) -> EtlContinuityPackPackage:
    """Compose coverage + PDF readiness + closeout into a dashboard pack."""
    multi_div = int(getattr(coverage, "multi_root_divergent_content_count", 0) or 0)
    multi_ids = int(getattr(coverage, "multi_root_paper_id_count", 0) or 0)
    multi_ident = int(getattr(coverage, "multi_root_identical_content_count", 0) or 0)
    expand_frac = float(pdf_readiness.expand_ready_fraction_of_missing)
    dashboard = {
        "hybrid_found": coverage.hybrid_body_found,
        "hybrid_missing": coverage.hybrid_body_missing,
        "hybrid_fraction": coverage.hybrid_body_fraction,
        "article_count": coverage.article_count,
        "hybrid_unique_paper_ids": coverage.hybrid_body_unique_paper_ids,
        "hybrid_artifact_files": coverage.hybrid_body_artifact_files,
        "multi_root_paper_id_count": multi_ids,
        "multi_root_identical_content_count": multi_ident,
        "multi_root_divergent_content_count": multi_div,
        "expand_ready_frac": expand_frac,
        "missing_with_local_pdf_count": pdf_readiness.missing_with_local_pdf_count,
        "missing_without_local_pdf_count": pdf_readiness.missing_without_local_pdf_count,
        "closeout_signal": closeout.closeout_signal,
        "closeout_pass": closeout.closeout_pass,
        "readiness_signal": closeout.readiness_signal,
        "import_eligible": False,
    }
    alerts: list[str] = []
    if multi_div > 0:
        alerts.append(f"multi_root_divergent_content:{multi_div}")
    if coverage.hybrid_body_found == 0 and coverage.article_count > 0:
        alerts.append("no_hybrid_bodies_joined")
    if pdf_readiness.hybrid_missing_count > 0 and expand_frac < 0.5:
        alerts.append(f"low_expand_ready_frac:{expand_frac}")
    if not closeout.closeout_pass:
        alerts.append(f"wave_a_not_closed:{closeout.closeout_signal}")
    diagnostics = (
        f"hybrid_found:{coverage.hybrid_body_found}",
        f"expand_ready_frac:{expand_frac}",
        f"multi_root_divergent:{multi_div}",
        f"closeout:{closeout.closeout_signal}",
        f"alerts:{len(alerts)}",
        "import_write_fail_closed",
        "wave_a_continuity_pack_only",
    )
    return EtlContinuityPackPackage(
        schema_version=SCHEMA_VERSION,
        coverage=coverage,
        pdf_readiness=pdf_readiness,
        closeout=closeout,
        dashboard=dashboard,
        alerts=tuple(alerts),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "EtlContinuityPackPackage",
    "build_etl_continuity_pack",
]
