"""TDD: ETL continuity pack dashboard composition."""

from __future__ import annotations

from research_graph.application.corpus.etl_body_coverage_audit import (
    EtlBodyCoveragePackage,
)
from research_graph.application.corpus.etl_continuity_pack import build_etl_continuity_pack
from research_graph.application.corpus.etl_hybrid_missing_pdf_readiness import (
    HybridMissingPdfReadinessPackage,
)
from research_graph.application.corpus.wave_a_closeout import WaveACloseoutPackage


def _coverage(**kwargs):
    base = dict(
        schema_version="etl-body-coverage-audit.v1",
        article_count=10,
        by_source_code={"arxiv": 10},
        hybrid_body_found=4,
        hybrid_body_missing=6,
        article_json_found=10,
        article_json_missing=0,
        body_roots_scanned=1,
        gaps=("partial_hybrid_body_coverage",),
        samples=(),
        diagnostics=(),
        hybrid_body_artifact_files=8,
        hybrid_body_unique_paper_ids=4,
        multi_root_paper_id_count=2,
        multi_root_identical_content_count=2,
        multi_root_divergent_content_count=0,
    )
    base.update(kwargs)
    return EtlBodyCoveragePackage(**base)


def _pdf(**kwargs):
    base = dict(
        schema_version="etl-hybrid-missing-pdf-readiness.v1",
        article_count=10,
        hybrid_found_count=4,
        hybrid_missing_count=6,
        missing_with_local_pdf_count=5,
        missing_without_local_pdf_count=1,
        expand_ready_sample=(),
        expand_blocked_sample=(),
        diagnostics=(),
    )
    base.update(kwargs)
    return HybridMissingPdfReadinessPackage(**base)


def _closeout(**kwargs):
    return WaveACloseoutPackage(
        schema_version=str(kwargs.get("schema_version", "wave-a-closeout.v1")),
        closeout_signal=kwargs.get("closeout_signal", "wave_a_closed"),  # type: ignore[arg-type]
        closeout_pass=bool(kwargs.get("closeout_pass", True)),
        hybrid_found=int(kwargs.get("hybrid_found", 40)),
        min_hybrid_found=int(kwargs.get("min_hybrid_found", 49)),
        hybrid_fraction=float(kwargs.get("hybrid_fraction", 0.213)),
        hybrid_fraction_residual_target=float(kwargs.get("hybrid_fraction_residual_target", 0.35)),
        hybrid_fraction_meets_residual_target=bool(kwargs.get("hybrid_fraction_meets_residual_target", False)),
        residual_coverage_pass=bool(kwargs.get("residual_coverage_pass", False)),
        readiness_signal=str(kwargs.get("readiness_signal", "ready_for_review")),
        import_hold_hits=int(kwargs.get("import_hold_hits", 0)),
        preprocess_errors=int(kwargs.get("preprocess_errors", 0)),
        preprocess_body_count=int(kwargs.get("preprocess_body_count", 41)),
        article_count=int(kwargs.get("article_count", 230)),
        diagnostics=tuple(kwargs.get("diagnostics", ())),
        operator_commands=tuple(kwargs.get("operator_commands", ())),
    )


def test_continuity_pack_dashboard_fields() -> None:
    pkg = build_etl_continuity_pack(
        coverage=_coverage(),
        pdf_readiness=_pdf(),
        closeout=_closeout(hybrid_fraction=0.4, hybrid_fraction_meets_residual_target=True),
        preprocess_body_count=8,
        preprocess_errors=0,
        preprocess_quality={"ok": 6, "soft_signal": 2},
        import_hold_hits=0,
        expand_gate={"gate_signal": "blocked", "effective_limit": 0},
    )
    assert pkg.import_eligible is False
    d = pkg.dashboard
    assert d["hybrid_found"] == 4
    assert d["expand_ready_frac"] == 0.8333  # package rounds to 4 decimals
    assert d["multi_root_divergent_content_count"] == 0
    assert d["closeout_pass"] is True
    assert d["preprocess_body_count"] == 8
    assert d["preprocess_errors"] == 0
    assert d["import_hold_hits"] == 0
    assert d["preprocess_quality"]["ok"] == 6
    assert d["expand_gate"]["gate_signal"] == "blocked"
    assert d["hybrid_fraction_residual_target"] == 0.35
    # residual met in this fixture closeout override
    assert "hybrid_fraction_below_residual_target" not in " ".join(pkg.alerts)


def test_continuity_pack_alerts_on_divergent_multi_root() -> None:
    pkg = build_etl_continuity_pack(
        coverage=_coverage(
            multi_root_divergent_content_count=2,
            multi_root_paper_id_count=2,
        ),
        pdf_readiness=_pdf(),
        closeout=_closeout(),
    )
    assert any(a.startswith("multi_root_divergent_content:") for a in pkg.alerts)
    assert pkg.to_dict()["import_eligible"] is False


def test_continuity_pack_alerts_residual_fraction() -> None:
    # Alert uses coverage.hybrid_body_fraction (found/article), not closeout field alone.
    pkg = build_etl_continuity_pack(
        coverage=_coverage(hybrid_body_found=2, hybrid_body_missing=8, article_count=10),
        pdf_readiness=_pdf(),
        closeout=_closeout(hybrid_fraction=0.2, hybrid_fraction_meets_residual_target=False),
    )
    assert float(pkg.dashboard["hybrid_fraction"]) < 0.35
    assert any(a.startswith("hybrid_fraction_below_residual_target:") for a in pkg.alerts)
