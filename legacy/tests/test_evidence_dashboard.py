"""M283: evidence dashboard hygiene — metric_mode, page_or_bbox, weak IR."""

from __future__ import annotations

from research_graph.application.corpus.evidence_dashboard import (
    build_evidence_dashboard,
)


def test_dashboard_blocks_char_only_without_ok() -> None:
    pkg = build_evidence_dashboard(
        resolvability={
            "metric_mode": "real_gold_hybrid_join",
            "demo_metric": False,
            "resolvability_rate": 1.0,
            "target_rate": 0.95,
            "target_met": True,
            "page_or_bbox_count": 0,
            "char_only_count": 50,
            "total_rows": 50,
            "relation_grounded_ratio": 1.0,
            "alerts": ["char_only_no_page_bbox"],
        },
        structure_readiness={
            "structure_signal": "ready_for_structure_review",
            "weak_structure_ir": False,
            "ir_hard_count": 5,
            "alerts": [],
        },
        allow_char_only_ok=False,
    )
    assert pkg.import_eligible is False
    assert pkg.metric_mode == "real_gold_hybrid_join"
    assert pkg.page_or_bbox_count == 0
    assert pkg.evidence_ready_ok is False
    assert "char_only_no_page_bbox" in pkg.evidence_ready_blockers


def test_dashboard_blocks_weak_structure_ir() -> None:
    pkg = build_evidence_dashboard(
        resolvability={
            "metric_mode": "real_gold_hybrid_join",
            "demo_metric": False,
            "resolvability_rate": 1.0,
            "target_met": True,
            "page_or_bbox_count": 40,
            "char_only_count": 0,
            "total_rows": 40,
            "alerts": [],
        },
        structure_readiness={
            "structure_signal": "partial",
            "weak_structure_ir": True,
            "ir_hard_count": 0,
            "alerts": ["weak_structure_ir:gate_pass_ir_hard_0"],
        },
    )
    assert pkg.weak_structure_ir is True
    assert pkg.evidence_ready_ok is False
    assert "weak_structure_ir" in pkg.evidence_ready_blockers


def test_dashboard_ok_with_page_bbox() -> None:
    pkg = build_evidence_dashboard(
        resolvability={
            "metric_mode": "real_gold_hybrid_join",
            "demo_metric": False,
            "resolvability_rate": 0.98,
            "target_rate": 0.95,
            "target_met": True,
            "page_or_bbox_count": 40,
            "char_only_count": 2,
            "total_rows": 42,
            "relation_grounded_ratio": 0.9,
            "alerts": [],
        },
        structure_readiness={
            "structure_signal": "ready_for_structure_review",
            "weak_structure_ir": False,
            "ir_hard_count": 10,
            "alerts": [],
        },
    )
    assert pkg.evidence_ready_ok is True
    assert pkg.page_or_bbox_count == 40
    assert pkg.metric_mode == "real_gold_hybrid_join"
