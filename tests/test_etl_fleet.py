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
            "dashboard": {"hybrid_found": 81, "hybrid_fraction": 0.35},
            "alerts": [],
            "import_eligible": False,
        },
        import_hold={"verdict": "pass", "enablement_hits": 0, "import_eligible": False},
        ship_matrix={
            "ship_path": "header_priority_constrained_select",
            "gepa_justified": False,
            "import_eligible": False,
        },
    )
    assert pkg.import_eligible is False
    assert pkg.operator_status == "ok"
    d = pkg.to_dict()
    assert d["import_eligible"] is False


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
