"""Tests for ETL fleet glue package (M266)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

from research_graph.application.corpus.etl_fleet import build_etl_fleet_package

ROOT = Path(__file__).resolve().parents[1]


def test_build_fleet_package_fail_closed() -> None:
    pkg = build_etl_fleet_package(
        continuity={
            "dashboard": {
                "hybrid_found": 81,
                "hybrid_fraction": 0.35,
                "multi_root_same_inode_count": 20,
            },
            "alerts": [],
            "import_eligible": False,
        },
        import_hold={"verdict": "pass", "enablement_hits": 0, "import_eligible": False},
        ship_matrix={
            "ship_path": "header_priority_constrained_select",
            "gepa_justified": False,
            "import_eligible": False,
            "worlds": {"context": {"joined_count": 23}},
        },
        quality_n={"all_match": True, "canonical_joined_count": 23, "mismatches": []},
    )
    assert pkg.import_eligible is False
    assert pkg.operator_status == "ok"
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["quality_n"]["canonical_joined_count"] == 23


def test_fleet_alerts_on_quality_n_mismatch() -> None:
    """Live-world n mismatch (gepa/header) is a hard fleet alert."""
    pkg = build_etl_fleet_package(
        continuity={"dashboard": {"hybrid_found": 1}, "alerts": [], "import_eligible": False},
        import_hold={"verdict": "pass", "enablement_hits": 0},
        ship_matrix={"ship_path": "header_priority_constrained_select", "gepa_justified": False},
        quality_n={
            "all_match": False,
            "canonical_joined_count": 23,
            "mismatches": ["gepa:20!=canonical:23"],
        },
    )
    assert pkg.operator_status == "alerts"
    assert any("quality_n_live_mismatch" in a for a in pkg.alerts)


def test_fleet_alerts_on_import_eligible() -> None:
    pkg = build_etl_fleet_package(
        continuity={"dashboard": {}, "alerts": ["x"], "import_eligible": True},
        import_hold={"verdict": "pass", "enablement_hits": 0},
    )
    assert any("import_eligible" in a for a in pkg.alerts)


def test_operator_help() -> None:
    script = ROOT / "scripts" / "verify_etl_fleet.py"
    proc = subprocess.run(
        [sys.executable, str(script), "--help"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    assert "fleet" in proc.stdout.lower() or "continuity" in proc.stdout.lower()



def test_fleet_llm_stale_is_soft_alert() -> None:
    """Only stale LLM n=20 should not hard-fail fleet status (M272)."""
    pkg = build_etl_fleet_package(
        continuity={
            "dashboard": {"hybrid_found": 81, "hybrid_fraction": 0.35},
            "alerts": [],
            "import_eligible": False,
        },
        import_hold={"verdict": "pass", "enablement_hits": 0},
        ship_matrix={
            "ship_path": "header_priority_constrained_select",
            "gepa_justified": False,
        },
        quality_n={
            "all_match": False,
            "canonical_joined_count": 23,
            "mismatches": ["llm:20!=canonical:23", "compare:20!=canonical:23"],
        },
    )
    assert pkg.quality_n is not None
    assert pkg.quality_n.get("live_all_match") is True
    assert pkg.quality_n.get("llm_stale") is True
    assert pkg.operator_status == "ok"
    assert any(a == "llm_compare_stale_n" for a in pkg.alerts)


def test_fleet_surfaces_evidence_dashboard() -> None:
    """M284: fleet pack surfaces evidence_ready_ok + page_or_bbox + weak IR."""
    pkg = build_etl_fleet_package(
        continuity={"dashboard": {"hybrid_found": 60}, "alerts": [], "import_eligible": False},
        import_hold={"verdict": "pass", "enablement_hits": 0},
        ship_matrix={"ship_path": "header_priority_constrained_select", "gepa_justified": False},
        quality_n={"all_match": True, "canonical_joined_count": 23, "mismatches": []},
        evidence_dashboard={
            "resolvability": {
                "metric_mode": "layout_page_bbox",
                "demo_metric": False,
                "resolvability_rate": 1.0,
                "target_rate": 0.95,
                "target_met": True,
                "page_or_bbox_count": 69,
                "char_only_count": 0,
                "total_rows": 23,
                "alerts": [],
            },
            "structure_readiness": {
                "structure_signal": "ready_for_structure_review",
                "weak_structure_ir": False,
                "ir_hard_count": 66,
                "alerts": [],
            },
        },
    )
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["evidence_dashboard"] is not None
    assert d["evidence_dashboard"]["evidence_ready_ok"] is True
    assert d["evidence_dashboard"]["page_or_bbox_count"] == 69
    assert d["evidence_dashboard"]["metric_mode"] == "layout_page_bbox"
    assert any("evidence_page_or_bbox:69" in x for x in d["diagnostics"])
    assert any("evidence_ready_ok:True" in x for x in d["diagnostics"])
    assert pkg.operator_status == "ok"


def test_fleet_alerts_on_evidence_blocker() -> None:
    """char-only without page/bbox is a fleet alert, not silent ok."""
    pkg = build_etl_fleet_package(
        continuity={"dashboard": {"hybrid_found": 60}, "alerts": [], "import_eligible": False},
        import_hold={"verdict": "pass", "enablement_hits": 0},
        evidence_dashboard={
            "resolvability": {
                "metric_mode": "real_gold_hybrid_join",
                "demo_metric": False,
                "resolvability_rate": 1.0,
                "target_met": True,
                "page_or_bbox_count": 0,
                "char_only_count": 50,
                "total_rows": 50,
                "alerts": ["char_only_no_page_bbox"],
            },
            "structure_readiness": {
                "structure_signal": "partial",
                "weak_structure_ir": True,
                "ir_hard_count": 0,
                "alerts": [],
            },
        },
    )
    d = pkg.to_dict()
    assert d["evidence_dashboard"]["evidence_ready_ok"] is False
    assert any("evidence_blocker:char_only_no_page_bbox" in a for a in d["alerts"])
    assert any("evidence_blocker:weak_structure_ir" in a for a in d["alerts"])
    assert pkg.operator_status == "alerts"
