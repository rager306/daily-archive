"""M284 S04: import evidence chain — fail-closed, user_go never flips import."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.import_evidence_chain import (
    build_import_evidence_chain,
)


def _green_inputs() -> dict:
    return {
        "evidence_dashboard": {
            "evidence_ready_ok": True,
            "page_or_bbox_count": 69,
            "char_only_count": 0,
            "weak_structure_ir": False,
            "structure_signal": "ready_for_structure_review",
            "evidence_ready_blockers": [],
            "alerts": [],
        },
        "prediction_resolvability": {
            "resolvability_rate": 0.74,
            "page_or_bbox_count": 71,
            "char_only_count": 0,
            "alerts": [],
        },
        "structure_readiness": {
            "structure_signal": "ready_for_structure_review",
            "weak_structure_ir": False,
        },
        "import_hold": {"verdict": "pass", "enablement_hits": 0},
        "e5_optional": {"header_entities_total": 24, "alerts": []},
    }


def test_chain_green_without_user_go_still_import_false() -> None:
    pkg = build_import_evidence_chain(**_green_inputs(), user_go=False)
    assert pkg.chain_green is True
    assert pkg.import_eligible is False
    assert pkg.graph_write_allowed is False
    assert "user_go_required_for_graph_write" in pkg.blockers
    assert pkg.evidence_ready_ok is True
    assert pkg.verification_ready_ok is True


def test_user_go_alone_never_flips_import_eligible() -> None:
    pkg = build_import_evidence_chain(**_green_inputs(), user_go=True)
    assert pkg.user_go is True
    assert pkg.import_eligible is False
    assert pkg.graph_write_allowed is False
    assert "user_go_does_not_flip_import_eligible_d127" in pkg.alerts
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["graph_write_allowed"] is False


def test_cannot_construct_with_import_true() -> None:
    from research_graph.application.corpus.import_evidence_chain import (
        ImportEvidenceChainPackage,
    )

    with pytest.raises(ValueError, match="cannot authorize"):
        ImportEvidenceChainPackage(
            schema_version="x",
            evidence_ready_ok=True,
            verification_ready_ok=True,
            prediction_resolvability_rate=1.0,
            page_or_bbox_count=1,
            char_only_count=0,
            structure_signal="ok",
            weak_structure_ir=False,
            import_hold_verdict="pass",
            import_hold_hits=0,
            e5_header_entities=0,
            user_go=True,
            chain_green=True,
            import_eligible=True,
            graph_write_allowed=False,
            blockers=(),
            alerts=(),
            diagnostics=(),
            seams=(),
        )


def test_blockers_when_evidence_not_ready() -> None:
    pkg = build_import_evidence_chain(
        evidence_dashboard={
            "evidence_ready_ok": False,
            "page_or_bbox_count": 0,
            "char_only_count": 50,
            "weak_structure_ir": True,
            "evidence_ready_blockers": ["char_only_no_page_bbox"],
            "alerts": [],
        },
        prediction_resolvability={"resolvability_rate": 0.5, "page_or_bbox_count": 0},
        import_hold={"verdict": "pass", "enablement_hits": 0},
        user_go=False,
    )
    assert pkg.chain_green is False
    assert "evidence_not_ready" in pkg.blockers
    assert "page_or_bbox_count_zero" in pkg.blockers
    assert "weak_structure_ir" in pkg.blockers
    assert pkg.import_eligible is False
