#!/usr/bin/env python3
"""Unattended ETL fleet dashboard (M266/M271).

Runs continuity pack + ship matrix + import-hold + quality n-contract.
Optional --rescore-quality rebuilds header/matrix/grounding on live gold join.

Never expands live hybrid. Never import.

Usage::

    uv run python scripts/verify_etl_fleet.py
    uv run python scripts/verify_etl_fleet.py --rescore-quality
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
    default_import_hold_roots,
    inventory_import_hold_trees,
)
from research_graph.application.corpus.etl_continuity_pack import (
    compose_live_continuity_pack,
)
from research_graph.application.corpus.etl_fleet import build_etl_fleet_package
from research_graph.application.corpus.wave_b_constrained_select import (
    header_priority_select,
)
from research_graph.application.corpus.wave_b_extraction_baseline import (
    DEFAULT_HUMAN_GO_STAMP,
    read_human_go_stamp,
)
from research_graph.application.corpus.wave_b_gate import evaluate_wave_b_gate_from_stamp
from research_graph.application.corpus.wave_b_gold_body_grounding_audit import (
    audit_gold_body_grounding,
)
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    score_gold_hybrid_constrained_pilot,
)
from research_graph.application.corpus.wave_b_gold_hybrid_join import (
    inventory_reviewed_gold_hybrid_join,
)
from research_graph.application.corpus.wave_b_gold_hybrid_lexical_metrics import (
    score_gold_hybrid_lexical_recovery,
)
from research_graph.application.corpus.wave_b_quality_n_contract import (
    evaluate_quality_n_contract,
    extract_joined_count,
)
from research_graph.application.corpus.wave_b_ship_gate_matrix import (
    build_wave_b_ship_gate_matrix,
)
from research_graph.application.reviewed_extraction_fixtures import (
    load_reviewed_extraction_split,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/etl/fleet-report.json")
DEFAULT_SHIP = Path("artifacts/wave-b/ship-gate-matrix.json")
DEFAULT_GEPA_VS = Path("artifacts/wave-b/gepa-vs-header-n23-valaware.json")
DEFAULT_GROUNDING = Path("artifacts/wave-b/gold-body-grounding-audit.json")
DEFAULT_LLM_COMPARE = Path("artifacts/wave-b/constrained-select-header-vs-llm.json")
DEFAULT_EVIDENCE_DASHBOARD = Path("artifacts/etl/evidence-dashboard.v1.json")


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


def _load_cases(repo: Path) -> list[dict[str, Any]]:
    roots = tuple(_r(repo, Path(p)) for p in DEFAULT_BODY_ROOTS)
    train_g, _ = load_reviewed_extraction_split("train")
    val_g, _ = load_reviewed_extraction_split("validation")
    gold_all = train_g + val_g
    gold_by = {str(r.get("case_id")): r for r in gold_all if r.get("case_id")}
    join = inventory_reviewed_gold_hybrid_join(gold_records=gold_all, body_roots=roots)
    cases: list[dict[str, Any]] = []
    for row in join.joined:
        case_id = str(row.get("case_id") or "")
        gold = gold_by.get(case_id)
        if gold is None:
            continue
        path = Path(str(row.get("body_path") or ""))
        try:
            body = path.read_text(encoding="utf-8") if path.is_file() else ""
        except OSError:
            body = ""
        cases.append(
            {
                "case_id": case_id,
                "paper_id": str(row.get("paper_id") or case_id),
                "gold": gold,
                "body_text": body,
            }
        )
    return cases


def _rescore_quality(
    repo: Path,
    *,
    gepa_vs: dict[str, Any],
    llm_compare: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    """Live header + grounding + matrix on same joined cases."""
    cases = _load_cases(repo)
    joined = len(cases)
    floor_metrics: dict[str, Any] = {}
    header_payload: dict[str, Any] = {}
    body_ratio = None
    cand_ratio = None
    ground_payload: dict[str, Any] = {}

    if cases:
        floor_pkg = score_gold_hybrid_lexical_recovery(cases=cases)
        floor_metrics = dict(floor_pkg.metrics)
        header_pilot = score_gold_hybrid_constrained_pilot(
            cases=cases,
            select_fn=header_priority_select,
            floor_metrics=floor_metrics,
            model_id="header_priority_select",
        )
        header_payload = header_pilot.to_dict()
        gpkg = audit_gold_body_grounding(cases=cases)
        ground_payload = gpkg.to_dict()
        body_ratio = float(gpkg.body_coverage_ratio)
        cand_ratio = float(gpkg.candidate_coverage_ratio)

    offline_gepa = None
    if isinstance(gepa_vs.get("gepa"), dict):
        offline_gepa = dict(gepa_vs["gepa"])
        offline_gepa["promote_ready"] = gepa_vs.get("promote_ready")
        offline_gepa["joined_count"] = gepa_vs.get("joined_count") or extract_joined_count(
            gepa_vs
        )

    stamp = read_human_go_stamp(_r(repo, Path(DEFAULT_HUMAN_GO_STAMP)))
    human_go = bool(stamp and stamp.get("human_go") is True)
    pack_disk = _load_json(_r(repo, Path("artifacts/etl/continuity-pack.json")))
    dash = pack_disk.get("dashboard") if isinstance(pack_disk.get("dashboard"), dict) else {}
    closeout_pass = bool(dash.get("closeout_pass", True))

    matrix = build_wave_b_ship_gate_matrix(
        floor=floor_metrics
        or (
            header_payload.get("floor_metrics")
            if isinstance(header_payload.get("floor_metrics"), dict)
            else None
        ),
        header=header_payload,
        llm_compare=llm_compare or None,
        offline_gepa=offline_gepa,
        joined_count=joined,
        grounding_body_ratio=body_ratio,
        grounding_cand_ratio=cand_ratio,
        human_go=human_go,
        wave_a_closeout_pass=closeout_pass,
    )
    matrix_payload = matrix.to_dict()
    matrix_payload["import_eligible"] = False
    matrix_payload["graph_writes_allowed"] = False
    matrix_payload["rescored"] = True
    matrix_payload["joined_count"] = joined

    ground_payload["joined_count"] = joined
    ground_payload["import_eligible"] = False
    ground_payload["rescored"] = True

    qn = evaluate_quality_n_contract(
        header_n=joined,
        llm_n=extract_joined_count(llm_compare),
        gepa_n=extract_joined_count(gepa_vs) or (
            int(offline_gepa["joined_count"])
            if offline_gepa and offline_gepa.get("joined_count") is not None
            else None
        ),
        grounding_n=joined,
        matrix_n=joined,
        compare_n=extract_joined_count(llm_compare),
        canonical=joined,
    ).to_dict()
    matrix_payload.setdefault("worlds", {})
    if isinstance(matrix_payload["worlds"], dict):
        matrix_payload["worlds"]["quality_n_contract"] = qn

    return matrix_payload, ground_payload, qn


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "ETL fleet glue: continuity pack + ship matrix + import-hold + quality n. "
            "Import always false. No live expand."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--ship-matrix", type=Path, default=DEFAULT_SHIP)
    parser.add_argument("--gepa-vs-header", type=Path, default=DEFAULT_GEPA_VS)
    parser.add_argument("--grounding", type=Path, default=DEFAULT_GROUNDING)
    parser.add_argument("--llm-compare", type=Path, default=DEFAULT_LLM_COMPARE)
    parser.add_argument(
        "--evidence-dashboard",
        type=Path,
        default=DEFAULT_EVIDENCE_DASHBOARD,
        help="Evidence dashboard artifact (resolvability + structure + page/bbox).",
    )
    parser.add_argument(
        "--skip-live-pack",
        action="store_true",
        help="Use only on-disk continuity-pack.json (no live compose)",
    )
    parser.add_argument(
        "--rescore-quality",
        action="store_true",
        help=(
            "Live rescore header/matrix/grounding on full gold-hybrid join "
            "(same-n contract). Writes matrix + grounding artifacts."
        ),
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
        # also refresh pack artifact for same_inode field
        pack_path = _r(repo, Path("artifacts/etl/continuity-pack.json"))
        pack_path.parent.mkdir(parents=True, exist_ok=True)
        pack_path.write_text(
            json.dumps(cont, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )

    gepa_vs = _load_json(_r(repo, Path(args.gepa_vs_header)))
    llm_compare = _load_json(_r(repo, Path(args.llm_compare)))
    ground = _load_json(_r(repo, Path(args.grounding)))
    quality_n: dict[str, Any] | None = None

    if args.rescore_quality:
        matrix_payload, ground, quality_n = _rescore_quality(
            repo, gepa_vs=gepa_vs, llm_compare=llm_compare
        )
        ship_path = _r(repo, Path(args.ship_matrix))
        ship_path.parent.mkdir(parents=True, exist_ok=True)
        ship_path.write_text(
            json.dumps(matrix_payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        gpath = _r(repo, Path(args.grounding))
        gpath.parent.mkdir(parents=True, exist_ok=True)
        gpath.write_text(
            json.dumps(ground, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    else:
        disk_matrix = _load_json(_r(repo, Path(args.ship_matrix)))
        offline_gepa = None
        if isinstance(gepa_vs.get("gepa"), dict):
            offline_gepa = dict(gepa_vs["gepa"])
            offline_gepa["promote_ready"] = gepa_vs.get("promote_ready")
            offline_gepa["joined_count"] = gepa_vs.get("joined_count")
        if disk_matrix:
            matrix_payload = disk_matrix
        else:
            matrix_pkg = build_wave_b_ship_gate_matrix(
                header={"entity_f1": 0.0, "relation_f1": 0.0},
                offline_gepa=offline_gepa,
                llm_compare=llm_compare or None,
                human_go=True,
                wave_a_closeout_pass=True,
            )
            matrix_payload = matrix_pkg.to_dict()
        quality_n = evaluate_quality_n_contract(
            header_n=extract_joined_count(matrix_payload),
            llm_n=extract_joined_count(llm_compare),
            gepa_n=extract_joined_count(gepa_vs),
            grounding_n=extract_joined_count(ground),
            matrix_n=extract_joined_count(matrix_payload),
            compare_n=extract_joined_count(llm_compare),
            canonical=extract_joined_count(matrix_payload),
        ).to_dict()

    try:
        roots = default_import_hold_roots()
        hold_inv = inventory_import_hold_trees(roots)
        hold = dict(hold_inv) if isinstance(hold_inv, dict) else {
            "verdict": "pass",
            "enablement_hits": 0,
            "import_eligible": False,
        }
        hits = hold.get("enablement_hits")
        if hits is None and isinstance(hold.get("hits"), list):
            hits = len(hold["hits"])
        if isinstance(hits, list):
            hits = len(hits)
        hold["enablement_hits"] = int(hits or 0)
        hold["verdict"] = "pass" if int(hold["enablement_hits"]) == 0 else "fail"
    except Exception as exc:  # noqa: BLE001
        hold = {
            "verdict": "error",
            "error": f"{type(exc).__name__}:{exc}",
            "enablement_hits": None,
            "import_eligible": False,
        }

    hold["import_eligible"] = False
    evidence_dashboard = _load_json(_r(repo, Path(args.evidence_dashboard))) or None
    fleet = build_etl_fleet_package(
        continuity=cont,
        import_hold=hold,
        ship_matrix=matrix_payload,
        quality_n=quality_n,
        grounding=ground,
        evidence_dashboard=evidence_dashboard,
    )
    payload = fleet.to_dict()
    payload["rescored_quality"] = bool(args.rescore_quality)
    out = _r(repo, Path(args.output))
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        dash = cont.get("dashboard") if isinstance(cont.get("dashboard"), dict) else cont
        sm = matrix_payload or {}
        qn = quality_n or {}
        sys.stdout.write(
            "etl-fleet | "
            f"status: {payload['operator_status']} | "
            f"hybrid_found: {dash.get('hybrid_found')} | "
            f"hybrid_fraction: {dash.get('hybrid_fraction')} | "
            f"same_inode: {dash.get('multi_root_same_inode_count')} | "
            f"ship_path: {sm.get('ship_path')} | "
            f"gepa_justified: {sm.get('gepa_justified')} | "
            f"quality_n: {qn.get('canonical_joined_count')} "
            f"match={qn.get('all_match')} | "
            f"rescore: {str(bool(args.rescore_quality)).lower()} | "
            f"alerts: {len(payload.get('alerts') or [])} | "
            "import_eligible: false\n"
        )
        if payload.get("alerts"):
            sys.stdout.write("  alerts: " + "; ".join(payload["alerts"][:6]) + "\n")
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
