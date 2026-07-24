"""Wave A ETL continuity pack: composed dashboard for operators.

Composes body coverage (incl. multi-root taxonomy), hybrid-missing PDF readiness,
Wave A closeout, preprocess/hold context, and optional expand_gate into one
fail-closed report. Never authorizes import or batch.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.application.corpus.etl_body_coverage_audit import EtlBodyCoveragePackage
from research_graph.application.corpus.etl_hybrid_missing_pdf_readiness import (
    HybridMissingPdfReadinessPackage,
    audit_hybrid_missing_pdf_readiness,
)
from research_graph.application.corpus.wave_a_closeout import WaveACloseoutPackage

SCHEMA_VERSION = "etl-continuity-pack.v2"

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
    preprocess_body_count: int | None = None,
    preprocess_errors: int | None = None,
    preprocess_quality: Mapping[str, int] | None = None,
    import_hold_hits: int | None = None,
    expand_gate: Mapping[str, Any] | None = None,
    continuity_readiness_signal: str | None = None,
) -> EtlContinuityPackPackage:
    """Compose coverage + PDF readiness + closeout into a dashboard pack.

    Optional cockpit fields (preprocess / hold / expand_gate) are dashboard-only
    and never authorize import or live batch.
    """
    multi_div = int(getattr(coverage, "multi_root_divergent_content_count", 0) or 0)
    multi_ids = int(getattr(coverage, "multi_root_paper_id_count", 0) or 0)
    multi_ident = int(getattr(coverage, "multi_root_identical_content_count", 0) or 0)
    multi_same_inode = int(getattr(coverage, "multi_root_same_inode_count", 0) or 0)
    expand_frac = float(pdf_readiness.expand_ready_fraction_of_missing)

    pre_bodies = (
        int(preprocess_body_count)
        if preprocess_body_count is not None
        else int(closeout.preprocess_body_count)
    )
    pre_errors = (
        int(preprocess_errors)
        if preprocess_errors is not None
        else int(closeout.preprocess_errors)
    )
    hold_hits = (
        int(import_hold_hits)
        if import_hold_hits is not None
        else int(closeout.import_hold_hits)
    )
    quality = dict(preprocess_quality or {})
    gate = dict(expand_gate or {})

    hybrid_fraction = float(coverage.hybrid_body_fraction)
    residual_target = float(getattr(closeout, "hybrid_fraction_residual_target", 0.35) or 0.35)
    dashboard: dict[str, Any] = {
        "hybrid_found": coverage.hybrid_body_found,
        "hybrid_missing": coverage.hybrid_body_missing,
        "hybrid_fraction": hybrid_fraction,
        "article_count": coverage.article_count,
        "hybrid_unique_paper_ids": coverage.hybrid_body_unique_paper_ids,
        "hybrid_artifact_files": coverage.hybrid_body_artifact_files,
        "multi_root_paper_id_count": multi_ids,
        "multi_root_identical_content_count": multi_ident,
        "multi_root_divergent_content_count": multi_div,
        "multi_root_same_inode_count": multi_same_inode,
        "expand_ready_frac": expand_frac,
        "missing_with_local_pdf_count": pdf_readiness.missing_with_local_pdf_count,
        "missing_without_local_pdf_count": pdf_readiness.missing_without_local_pdf_count,
        "closeout_signal": closeout.closeout_signal,
        "closeout_pass": closeout.closeout_pass,
        "readiness_signal": closeout.readiness_signal,
        "continuity_readiness_signal": continuity_readiness_signal or closeout.readiness_signal,
        "preprocess_body_count": pre_bodies,
        "preprocess_errors": pre_errors,
        "preprocess_quality": quality,
        "import_hold_hits": hold_hits,
        "hybrid_fraction_residual_target": residual_target,
        "hybrid_fraction_meets_residual_target": hybrid_fraction >= residual_target,
        "min_hybrid_found": closeout.min_hybrid_found,
        "expand_gate": gate,
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
    if pre_errors:
        alerts.append(f"preprocess_errors:{pre_errors}")
    if hold_hits:
        alerts.append(f"import_hold_hits:{hold_hits}")
    if hybrid_fraction < residual_target:
        alerts.append(
            f"hybrid_fraction_below_residual_target:{hybrid_fraction}<{residual_target}"
        )
    diagnostics = (
        f"hybrid_found:{coverage.hybrid_body_found}",
        f"hybrid_fraction:{hybrid_fraction}",
        f"residual_target:{residual_target}",
        f"expand_ready_frac:{expand_frac}",
        f"multi_root_divergent:{multi_div}",
        f"multi_root_same_inode:{multi_same_inode}",
        f"preprocess_bodies:{pre_bodies}",
        f"preprocess_errors:{pre_errors}",
        f"import_hold_hits:{hold_hits}",
        f"expand_gate:{gate.get('gate_signal') if gate else None}",
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


# Application-local path defaults (no workflows import — onion clean).
DEFAULT_PACK_CATALOG_INDEX = Path("data/article_catalog/index.json")
DEFAULT_PACK_CATALOG_ROOT = Path("data/article_catalog")
DEFAULT_PACK_BODY_ROOTS: tuple[Path, ...] = (
    Path("artifacts/m213-hybrid-gate/runs-live-expand"),
    Path("artifacts/m213-hybrid-gate/runs-live-20"),
    Path("artifacts/m213-hybrid-gate/runs-live"),
    Path("artifacts/m213-hybrid-gate/runs-live-scholarly-20"),
    Path("artifacts/m213-hybrid-gate/runs-live-scholarly"),
)


def compose_live_continuity_pack(
    *,
    repo_root: Path,
    catalog_index: Path | None = None,
    catalog_root: Path | None = None,
    body_roots: Sequence[Path] | None = None,
    expand_gate: Mapping[str, Any] | None = None,
    sample_limit: int = 8,
) -> EtlContinuityPackPackage:
    """Live operator composition: readiness + pdf + hold + closeout + pack.

    Defaults are application Path literals (not workflows imports). Never import.
    """
    from research_graph.application.corpus.composition_import_hold_inventory import (
        default_import_hold_roots,
        inventory_import_hold_trees,
    )
    from research_graph.application.corpus.etl_continuity_readiness import (
        build_continuity_readiness,
    )
    from research_graph.application.corpus.wave_a_closeout import evaluate_wave_a_closeout

    repo = Path(repo_root)

    def _r(p: Path) -> Path:
        return p if p.is_absolute() else (repo / p).resolve()

    index = _r(Path(catalog_index or DEFAULT_PACK_CATALOG_INDEX))
    cat_root = _r(Path(catalog_root or DEFAULT_PACK_CATALOG_ROOT))
    if body_roots is None:
        roots = tuple(_r(Path(p)) for p in DEFAULT_PACK_BODY_ROOTS)
    else:
        roots = tuple(_r(Path(p)) for p in body_roots)

    continuity = build_continuity_readiness(
        catalog_index_path=index,
        body_roots=roots,
        catalog_root=cat_root,
        sample_limit=sample_limit,
    )
    coverage = continuity.coverage
    preprocess = continuity.preprocess
    pdf = audit_hybrid_missing_pdf_readiness(
        catalog_index_path=index,
        catalog_root=cat_root,
        body_roots=roots,
    )
    hold = inventory_import_hold_trees(default_import_hold_roots())
    hold_hits = int(hold.get("enablement_hit_count") or 0)
    quality = dict(getattr(preprocess, "quality_status_counts", None) or {})

    closeout = evaluate_wave_a_closeout(
        hybrid_found=coverage.hybrid_body_found,
        readiness_signal=continuity.readiness_signal,
        import_hold_hits=hold_hits,
        preprocess_errors=int(preprocess.error_count),
        preprocess_body_count=int(preprocess.body_count),
        article_count=int(coverage.article_count),
        hybrid_fraction=float(coverage.hybrid_body_fraction),
    )
    return build_etl_continuity_pack(
        coverage=coverage,
        pdf_readiness=pdf,
        closeout=closeout,
        preprocess_body_count=int(preprocess.body_count),
        preprocess_errors=int(preprocess.error_count),
        preprocess_quality=quality,
        import_hold_hits=hold_hits,
        expand_gate=expand_gate,
        continuity_readiness_signal=continuity.readiness_signal,
    )


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_PACK_BODY_ROOTS",
    "DEFAULT_PACK_CATALOG_INDEX",
    "DEFAULT_PACK_CATALOG_ROOT",
    "EtlContinuityPackPackage",
    "build_etl_continuity_pack",
    "compose_live_continuity_pack",
]
