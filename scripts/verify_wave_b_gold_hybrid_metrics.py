#!/usr/bin/env python3
"""Wave B gold-linked hybrid lexical metrics operator (M256 S03).

Joins M072 reviewed gold to hybrid bodies and scores deterministic
lexical gold-recovery (floor baseline). Stamp-aware gate.
No LLM, no DSPy, no import.

Usage::

    uv run python scripts/verify_wave_b_gold_hybrid_metrics.py
    uv run python scripts/verify_wave_b_gold_hybrid_metrics.py --json
    uv run python scripts/verify_wave_b_gold_hybrid_metrics.py --no-stamp
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
from research_graph.application.corpus.wave_b_gold_hybrid_join import (
    inventory_m072_gold_hybrid_join,
)
from research_graph.application.corpus.wave_b_gold_hybrid_lexical_metrics import (
    score_gold_hybrid_lexical_recovery,
)
from research_graph.application.extraction_ablations import load_m072_split
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B gold-linked hybrid lexical metrics. "
            "Stamp-aware; never LLM/DSPy/import. Floor baseline only."
        )
    )
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    if args.no_stamp:
        gate = evaluate_wave_b_gate(human_go=False)
    else:
        raw = args.stamp if args.stamp is not None else DEFAULT_HUMAN_GO_STAMP
        stamp_path = raw if raw.is_absolute() else (repo / raw).resolve()
        gate = evaluate_wave_b_gate_from_stamp(stamp_path)

    roots = tuple(
        (r if Path(r).is_absolute() else (repo / r).resolve()) for r in DEFAULT_BODY_ROOTS
    )

    # load gold from cwd-relative fixtures (repo root)
    # temporarily ensure cwd semantics for load_m072_split
    train_g, _ = load_m072_split("train")
    val_g, _ = load_m072_split("validation")
    gold_all = train_g + val_g
    gold_by_case = {str(r.get("case_id")): r for r in gold_all if r.get("case_id")}

    join = inventory_m072_gold_hybrid_join(
        gold_records=gold_all,
        body_roots=roots,
    )

    cases: list[dict] = []
    if gate.wave_b_gate_open and gate.human_go:
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

    metrics_pkg = score_gold_hybrid_lexical_recovery(cases=cases)
    payload = metrics_pkg.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["dspy_optimizer_enabled"] = False
    payload["llm_used"] = False
    payload["wave_b_gate_open"] = gate.wave_b_gate_open
    payload["human_go"] = gate.human_go
    payload["joined_count"] = join.joined_count
    payload["missing_hybrid_count"] = join.missing_hybrid_count
    payload["gold_case_count"] = join.gold_case_count
    payload["scored_case_count"] = metrics_pkg.case_count
    payload["operator_status"] = (
        "sampled" if (gate.wave_b_gate_open and gate.human_go) else "blocked_gate"
    )

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        m = metrics_pkg.metrics
        sys.stdout.write(
            "wave-b-gold-hybrid-metrics | "
            f"status: {payload['operator_status']} | "
            f"joined: {join.joined_count} | "
            f"scored: {metrics_pkg.case_count} | "
            f"entity_f1: {m.get('entity_f1')} | "
            f"relation_f1: {m.get('relation_f1')} | "
            f"entity_recall: {m.get('entity_recall')} | "
            f"gate: {metrics_pkg.gate_verdict} | "
            f"gate_open: {str(gate.wave_b_gate_open).lower()} | "
            "llm: false | dspy: false | import_eligible: false | "
            "note: lexical_floor_baseline\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
