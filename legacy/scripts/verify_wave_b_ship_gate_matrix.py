#!/usr/bin/env python3
"""Wave B ship-gate matrix operator (M260).

Composes floor / header / baseline / LLM compare into one decision artifact.
Never invents gold. Never opens import or GEPA without positive delta.

Usage::

    uv run python scripts/verify_wave_b_ship_gate_matrix.py
    uv run python scripts/verify_wave_b_ship_gate_matrix.py --json
    uv run python scripts/verify_wave_b_ship_gate_matrix.py \\
        --output artifacts/wave-b/ship-gate-matrix.json
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
from research_graph.application.corpus.etl_continuity_readiness import (
    build_continuity_readiness,
)
from research_graph.application.corpus.wave_a_closeout import evaluate_wave_a_closeout
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
from research_graph.application.corpus.wave_b_ship_gate_matrix import (
    build_wave_b_ship_gate_matrix,
)
from research_graph.application.reviewed_extraction_fixtures import (
    load_reviewed_extraction_split,
)
from research_graph.workflows.composition.etl_body_coverage import (
    DEFAULT_BODY_ROOTS,
    DEFAULT_CATALOG_INDEX,
    DEFAULT_CATALOG_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/wave-b/ship-gate-matrix.json")
DEFAULT_LLM_COMPARE = Path("artifacts/wave-b/constrained-select-header-vs-llm.json")
DEFAULT_HEADER_ARTIFACT = Path("artifacts/wave-b/constrained-header-select.json")


def _r(repo: Path, p: Path) -> Path:
    return p if p.is_absolute() else (repo / p).resolve()


def _load_json(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return raw if isinstance(raw, dict) else None


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


def _live_closeout(repo: Path) -> tuple[bool | None, str | None]:
    try:
        continuity = build_continuity_readiness(
            catalog_index_path=_r(repo, DEFAULT_CATALOG_INDEX),
            catalog_root=_r(repo, DEFAULT_CATALOG_ROOT),
            body_roots=tuple(_r(repo, Path(p)) for p in DEFAULT_BODY_ROOTS),
            sample_limit=8,
        )
        hold = inventory_import_hold_trees(default_import_hold_roots())
        hits = int(hold.get("enablement_hit_count") or 0)
        closeout = evaluate_wave_a_closeout(
            hybrid_found=continuity.coverage.hybrid_body_found,
            readiness_signal=continuity.readiness_signal,
            import_hold_hits=hits,
            preprocess_errors=continuity.preprocess.error_count,
            preprocess_body_count=continuity.preprocess.body_count,
            article_count=continuity.coverage.article_count,
            hybrid_fraction=float(continuity.coverage.hybrid_body_fraction),
        )
        return closeout.closeout_pass, closeout.closeout_signal
    except Exception:  # noqa: BLE001
        return None, None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B ship-gate matrix: floor + header + baseline + LLM compare. "
            "Import always false. GEPA closed unless LLM beats header."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--llm-compare", type=Path, default=DEFAULT_LLM_COMPARE)
    parser.add_argument("--header-artifact", type=Path, default=DEFAULT_HEADER_ARTIFACT)
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--skip-live-score",
        action="store_true",
        help="Use only on-disk artifacts (no re-score header/floor)",
    )
    parser.add_argument(
        "--gepa-vs-header",
        type=Path,
        default=Path("artifacts/wave-b/gepa-vs-header-n23.json"),
        help="Optional same-n GEPA vs header comparison artifact",
    )
    parser.add_argument(
        "--max-val-gap",
        type=float,
        default=0.35,
        help="Max train-val entity F1 gap for offline GEPA promote (D128)",
    )
    args = parser.parse_args(argv)
    repo = Path(args.repo_root)

    stamp_path = _r(
        repo,
        Path(args.stamp) if args.stamp is not None else Path(DEFAULT_HUMAN_GO_STAMP),
    )
    closeout_pass, closeout_signal = _live_closeout(repo)
    gate = evaluate_wave_b_gate_from_stamp(
        stamp_path,
        wave_a_closeout_pass=closeout_pass,
        wave_a_closeout_signal=closeout_signal,
    )
    stamp = read_human_go_stamp(stamp_path)
    human_go = bool(stamp and stamp.get("human_go") is True)

    floor_metrics: dict[str, Any] = {}
    header_payload: dict[str, Any] = {}
    joined_count: int | None = None
    body_ratio: float | None = None
    cand_ratio: float | None = None

    disk_header = _load_json(_r(repo, Path(args.header_artifact)))
    if not args.skip_live_score and human_go and gate.wave_b_gate_open:
        cases = _load_cases(repo)
        joined_count = len(cases)
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
            joined_count = int(header_payload.get("joined_count") or joined_count or 0)
            gpkg = audit_gold_body_grounding(cases=cases)
            body_ratio = float(gpkg.body_coverage_ratio)
            cand_ratio = float(gpkg.candidate_coverage_ratio)
    if not header_payload and disk_header:
        header_payload = disk_header
        floor_metrics = dict(disk_header.get("floor_metrics") or floor_metrics)
        joined_count = int(disk_header.get("joined_count") or joined_count or 0)
        if body_ratio is None:
            body_ratio = float(disk_header.get("floor_entity_f1") or 0) and None

    # grounding disk fallback
    if body_ratio is None or cand_ratio is None:
        gdisk = _load_json(_r(repo, Path("artifacts/wave-b/gold-body-grounding-audit.json")))
        if gdisk:
            body_ratio = body_ratio if body_ratio is not None else gdisk.get(
                "body_coverage_ratio", gdisk.get("body_ratio")
            )
            cand_ratio = cand_ratio if cand_ratio is not None else gdisk.get(
                "candidate_coverage_ratio", gdisk.get("cand_ratio")
            )

    baseline: dict[str, Any] = {
        "train_entity_f1": 0.925,
        "train_relation_f1": 0.6470588235294117,
        "note": (
            "fixture extraction baseline (verify_wave_b_extraction_baseline); "
            "not hybrid deploy path"
        ),
    }
    llm_compare = _load_json(_r(repo, Path(args.llm_compare)))
    gepa_vs = _load_json(_r(repo, Path(args.gepa_vs_header)))
    offline_gepa = None
    if isinstance(gepa_vs, dict):
        # prefer nested gepa view from compare package
        if isinstance(gepa_vs.get("gepa"), dict):
            offline_gepa = dict(gepa_vs["gepa"])
            offline_gepa["promote_ready"] = gepa_vs.get("promote_ready")
            offline_gepa["joined_count"] = gepa_vs.get("joined_count")
        else:
            offline_gepa = gepa_vs

    matrix = build_wave_b_ship_gate_matrix(
        floor=floor_metrics
        or (
            header_payload.get("floor_metrics")
            if isinstance(header_payload.get("floor_metrics"), dict)
            else None
        ),
        header=header_payload,
        baseline=baseline,
        llm_compare=llm_compare,
        offline_gepa=offline_gepa,
        joined_count=joined_count,
        grounding_body_ratio=float(body_ratio) if body_ratio is not None else None,
        grounding_cand_ratio=float(cand_ratio) if cand_ratio is not None else None,
        human_go=human_go,
        wave_a_closeout_pass=closeout_pass,
        max_val_gap=float(args.max_val_gap),
    )
    payload = matrix.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["gate_signal"] = gate.gate_signal
    payload["wave_a_closeout_signal"] = closeout_signal

    out = _r(repo, Path(args.output))
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        w = payload["worlds"]
        h = w.get("header_constrained_select") or {}
        f = w.get("floor_lexical_oracle") or {}
        l = w.get("llm_constrained_compare") or {}
        og = w.get("offline_gepa_instruction_select") or {}
        rel = payload.get("relation_status") or {}
        sys.stdout.write(
            "wave-b-ship-gate-matrix | "
            f"ship_ready: {str(payload['ship_ready']).lower()} | "
            f"ship_path: {payload['ship_path']} | "
            f"blocker: {payload['ship_blocker']} | "
            f"header: e={h.get('entity_f1')} r={h.get('relation_f1')} | "
            f"floor: e={f.get('entity_f1')} r={f.get('relation_f1')} | "
            f"llm: e={l.get('entity_f1')} r={l.get('relation_f1')} | "
            f"offline_gepa: e={og.get('entity_f1')} r={og.get('relation_f1')} "
            f"val_gap_ok={og.get('val_gap_ok')} | "
            f"gepa_justified: {str(payload['gepa_justified']).lower()} | "
            f"relation_path: {rel.get('path')} | "
            "import_eligible: false\n"
        )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
