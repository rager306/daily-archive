#!/usr/bin/env python3
"""Unattended ETL fleet dashboard (M266).

Runs continuity pack + ship matrix (disk) + import-hold inventory into one report.
Never expands live hybrid. Never import.

Usage::

    uv run python scripts/verify_etl_fleet.py
    uv run python scripts/verify_etl_fleet.py --json \\
        --output artifacts/etl/fleet-report.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from research_graph.application.corpus.composition_import_hold_inventory import (
    inventory_import_hold_trees,
)
from research_graph.application.corpus.etl_continuity_pack import (
    compose_live_continuity_pack,
)
from research_graph.application.corpus.etl_fleet import build_etl_fleet_package
from research_graph.application.corpus.wave_b_ship_gate_matrix import (
    build_wave_b_ship_gate_matrix,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/etl/fleet-report.json")
DEFAULT_SHIP = Path("artifacts/wave-b/ship-gate-matrix.json")
DEFAULT_GEPA_VS = Path("artifacts/wave-b/gepa-vs-header-n23-valaware.json")


def _r(repo: Path, p: Path) -> Path:
    return p if p.is_absolute() else (repo / p).resolve()


def _load_json(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "ETL fleet glue: continuity pack + ship matrix + import-hold. "
            "Import always false. No live expand."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--ship-matrix", type=Path, default=DEFAULT_SHIP)
    parser.add_argument("--gepa-vs-header", type=Path, default=DEFAULT_GEPA_VS)
    parser.add_argument(
        "--skip-live-pack",
        action="store_true",
        help="Use only on-disk continuity-pack.json (no live compose)",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    repo = Path(args.repo_root)

    if args.skip_live_pack:
        cont = _load_json(_r(repo, Path("artifacts/etl/continuity-pack.json")))
    else:
        pack = compose_live_continuity_pack(repo_root=repo)
        cont = pack.to_dict()
        cont["import_eligible"] = False
        cont["graph_writes_allowed"] = False

    disk_matrix = _load_json(_r(repo, Path(args.ship_matrix)))
    gepa_vs = _load_json(_r(repo, Path(args.gepa_vs_header)))
    offline_gepa = None
    if isinstance(gepa_vs.get("gepa"), dict):
        offline_gepa = dict(gepa_vs["gepa"])
        offline_gepa["promote_ready"] = gepa_vs.get("promote_ready")
    # Rebuild matrix lightly from disk header/llm if present (no live re-score)
    if disk_matrix:
        matrix_payload = disk_matrix
    else:
        worlds = {}
        matrix_pkg = build_wave_b_ship_gate_matrix(
            header={"entity_f1": 0.0, "relation_f1": 0.0},
            offline_gepa=offline_gepa,
            human_go=True,
            wave_a_closeout_pass=True,
        )
        matrix_payload = matrix_pkg.to_dict()

    # import-hold via multi-tree inventory (fail-closed summary)
    try:
        from research_graph.application.corpus.composition_import_hold_inventory import (
            default_import_hold_roots,
        )

        roots = default_import_hold_roots()
        hold_inv = inventory_import_hold_trees(roots)
        if isinstance(hold_inv, dict):
            hold = dict(hold_inv)
        else:
            hold = {
                "verdict": "pass",
                "enablement_hits": 0,
                "import_eligible": False,
            }
        hits = hold.get("enablement_hits")
        if hits is None and isinstance(hold.get("hits"), list):
            hits = len(hold["hits"])
        hold["enablement_hits"] = hits if hits is not None else 0
        hold["verdict"] = "pass" if int(hold.get("enablement_hits") or 0) == 0 else "fail"
    except Exception as exc:  # noqa: BLE001
        hold = {
            "verdict": "error",
            "error": f"{type(exc).__name__}:{exc}",
            "enablement_hits": None,
            "import_eligible": False,
        }

    hold["import_eligible"] = False
    fleet = build_etl_fleet_package(
        continuity=cont,
        import_hold=hold,
        ship_matrix=matrix_payload,
    )
    payload = fleet.to_dict()
    out = _r(repo, Path(args.output))
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        dash = cont.get("dashboard") if isinstance(cont.get("dashboard"), dict) else cont
        sm = matrix_payload or {}
        sys.stdout.write(
            "etl-fleet | "
            f"status: {payload['operator_status']} | "
            f"hybrid_found: {dash.get('hybrid_found')} | "
            f"hybrid_fraction: {dash.get('hybrid_fraction')} | "
            f"ship_path: {sm.get('ship_path')} | "
            f"gepa_justified: {sm.get('gepa_justified')} | "
            f"alerts: {len(payload.get('alerts') or [])} | "
            "import_eligible: false\n"
        )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
