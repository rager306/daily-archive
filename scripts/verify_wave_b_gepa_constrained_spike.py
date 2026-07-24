#!/usr/bin/env python3
"""Wave B GEPA-shaped constrained spike operator (offline).

Runs local reflective mutation over constrained candidates.
Does NOT install gepa. Does NOT enable DSPy. Does NOT import to graph.

Usage::

    uv run python scripts/verify_wave_b_gepa_constrained_spike.py
    uv run python scripts/verify_wave_b_gepa_constrained_spike.py --json
    uv run python scripts/verify_wave_b_gepa_constrained_spike.py --no-stamp
    uv run python scripts/verify_wave_b_gepa_constrained_spike.py \\
        --output artifacts/wave-b/gepa-constrained-spike.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_extraction_baseline import (
    DEFAULT_HUMAN_GO_STAMP,
)
from research_graph.application.corpus.wave_b_gate import (
    evaluate_wave_b_gate,
    evaluate_wave_b_gate_from_stamp,
)
from research_graph.application.corpus.wave_b_gepa_constrained_spike import (
    gepa_package_available,
    offline_reflective_spike,
    try_gepa_optimize,
)
from research_graph.application.corpus.wave_b_gold_hybrid_join import (
    GoldHybridJoinPackage,
    inventory_reviewed_gold_hybrid_join,
)
from research_graph.application.corpus.wave_b_gold_hybrid_lexical_metrics import (
    score_gold_hybrid_lexical_recovery,
)
from research_graph.application.reviewed_extraction_fixtures import (
    load_reviewed_extraction_split,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def _load_cases(
    repo: Path, *, gate_open: bool, human_go: bool
) -> tuple[list[dict], GoldHybridJoinPackage]:
    roots = tuple(
        (r if Path(r).is_absolute() else (repo / r).resolve()) for r in DEFAULT_BODY_ROOTS
    )
    train_g, _ = load_reviewed_extraction_split("train")
    val_g, _ = load_reviewed_extraction_split("validation")
    gold_all = train_g + val_g
    gold_by_case = {str(r.get("case_id")): r for r in gold_all if r.get("case_id")}
    join = inventory_reviewed_gold_hybrid_join(gold_records=gold_all, body_roots=roots)

    cases: list[dict] = []
    if gate_open and human_go:
        for row in join.joined:
            case_id = str(row.get("case_id") or "")
            gold = gold_by_case.get(case_id)
            if gold is None:
                continue
            path = Path(str(row.get("body_path") or ""))
            try:
                body_text = path.read_text(encoding="utf-8") if path.is_file() else ""
            except OSError:
                body_text = ""
            cases.append(
                {
                    "case_id": case_id,
                    "paper_id": str(row.get("paper_id") or ""),
                    "gold": gold,
                    "body_text": body_text,
                }
            )
    return cases, join


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B GEPA-shaped offline reflective spike on constrained pilot. "
            "No DSPy. No import. gepa package optional and off by default."
        )
    )
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--max-body-chars", type=int, default=8000)
    parser.add_argument("--max-iterations", type=int, default=6)
    parser.add_argument(
        "--acceptance",
        type=str,
        default="val_aware",
        choices=["val_aware", "train"],
        help="Accept rule: val_aware (default) or legacy train-only",
    )
    parser.add_argument(
        "--min-support",
        type=int,
        default=2,
        help="Min reflective support for new TYPE_HINT (anti-overfit)",
    )
    parser.add_argument(
        "--max-type-hints",
        type=int,
        default=12,
        help="Cap TYPE_HINT lines in entity instruction",
    )
    parser.add_argument(
        "--max-val-gap",
        type=float,
        default=0.35,
        help="Gap used in spike accept filter diagnostics",
    )
    parser.add_argument(
        "--max-new-hints",
        type=int,
        default=3,
        help="Max new TYPE_HINT lines per iteration (gradual)",
    )
    parser.add_argument(
        "--train-blend",
        type=float,
        default=0.2,
        help="Train weight in val_aware composite score",
    )
    parser.add_argument(
        "--try-gepa-package",
        action="store_true",
        help="Probe optional gepa.optimize (still requires reflection_lm; dry status only)",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    if args.no_stamp:
        gate = evaluate_wave_b_gate(human_go=False)
    else:
        raw = args.stamp if args.stamp is not None else DEFAULT_HUMAN_GO_STAMP
        stamp_path = raw if raw.is_absolute() else (repo / raw).resolve()
        gate = evaluate_wave_b_gate_from_stamp(stamp_path)

    cases, join = _load_cases(
        repo, gate_open=gate.wave_b_gate_open, human_go=gate.human_go
    )
    floor_pkg = score_gold_hybrid_lexical_recovery(cases=cases)
    floor_metrics = dict(floor_pkg.metrics)

    if not (gate.wave_b_gate_open and gate.human_go):
        payload = {
            "schema_version": "wave-b-gepa-constrained-spike.v1",
            "wave": "B",
            "mode": "blocked_gate",
            "case_count": 0,
            "metrics": None,
            "floor_metrics": floor_metrics,
            "gepa_package_available": gepa_package_available(),
            "gepa_ran": False,
            "llm_used": False,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "wave_b_gate_open": gate.wave_b_gate_open,
            "human_go": gate.human_go,
            "joined_count": join.joined_count,
            "operator_status": "blocked_gate",
            "note": "Stamp/gate closed; GEPA spike not run",
        }
    else:
        spike = offline_reflective_spike(
            cases=cases,
            max_iterations=int(args.max_iterations),
            max_body_chars=int(args.max_body_chars),
            floor_metrics=floor_metrics,
            acceptance=str(args.acceptance),
            min_support=int(args.min_support),
            max_type_hints=int(args.max_type_hints),
            max_new_hints=int(args.max_new_hints),
            max_val_gap=float(args.max_val_gap),
            train_blend=float(args.train_blend),
        )
        payload = spike.to_dict()
        payload["wave_b_gate_open"] = gate.wave_b_gate_open
        payload["human_go"] = gate.human_go
        payload["joined_count"] = join.joined_count
        payload["operator_status"] = "offline_reflective_spike"
        if args.try_gepa_package:
            payload["gepa_package_probe"] = try_gepa_optimize(
                cases=cases,
                max_body_chars=int(args.max_body_chars),
                reflection_lm=None,
            )

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        seed_m = payload.get("seed_metrics") if isinstance(payload.get("seed_metrics"), dict) else {}
        best_m = payload.get("best_metrics") if isinstance(payload.get("best_metrics"), dict) else {}
        oracle_m = (
            payload.get("oracle_ceiling_metrics")
            if isinstance(payload.get("oracle_ceiling_metrics"), dict)
            else {}
        )
        floor_m = payload.get("floor_metrics") if isinstance(payload.get("floor_metrics"), dict) else {}
        cov = payload.get("coverage_summary") if isinstance(payload.get("coverage_summary"), dict) else {}
        train_cov = cov.get("train") if isinstance(cov.get("train"), dict) else {}
        sys.stdout.write(
            "wave-b-gepa-constrained-spike | "
            f"status: {payload.get('operator_status')} | "
            f"mode: {payload.get('mode')} | "
            f"joined: {payload.get('joined_count')} | "
            f"cases: {payload.get('case_count')} | "
            f"seed_entity_f1: {seed_m.get('entity_f1')} | "
            f"best_entity_f1: {best_m.get('entity_f1')} | "
            f"oracle_entity_f1: {oracle_m.get('entity_f1')} | "
            f"floor_entity_f1: {floor_m.get('entity_f1')} | "
            f"coverage_ratio: {train_cov.get('coverage_ratio')} | "
            f"gepa_pkg: {str(payload.get('gepa_package_available')).lower()} | "
            f"gepa_ran: {str(payload.get('gepa_ran')).lower()} | "
            "llm: false | dspy: false | import_eligible: false\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
