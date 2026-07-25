#!/usr/bin/env python3
"""Wave B gold-hybrid constrained pilot operator.

Candidate-first diagnostic + optional injectible select.
Default: lexical-oracle ceiling (no LLM) to measure candidate coverage.
Stamp-aware. Never DSPy. Never import.

Usage::

    uv run python scripts/verify_wave_b_gold_hybrid_constrained_pilot.py
    uv run python scripts/verify_wave_b_gold_hybrid_constrained_pilot.py --json
    uv run python scripts/verify_wave_b_gold_hybrid_constrained_pilot.py --no-stamp
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
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    score_gold_hybrid_constrained_pilot,
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
            "Wave B constrained candidate-select pilot. "
            "Default: lexical-oracle diagnostic (no LLM). Never DSPy/import."
        )
    )
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument(
        "--max-body-chars",
        type=int,
        default=8000,
        help="Truncate hybrid body window for candidate build",
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
            "schema_version": "wave-b-reviewed-gold-hybrid-constrained-pilot.v1",
            "wave": "B",
            "mode": "blocked_gate",
            "case_count": 0,
            "metrics": None,
            "floor_metrics": floor_metrics,
            "gate_verdict": floor_pkg.gate_verdict,
            "llm_used": False,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "wave_b_gate_open": gate.wave_b_gate_open,
            "human_go": gate.human_go,
            "joined_count": join.joined_count,
            "scored_case_count": 0,
            "operator_status": "blocked_gate",
            "note": "Stamp/gate closed; constrained pilot not scored",
        }
    else:
        pilot = score_gold_hybrid_constrained_pilot(
            cases=cases,
            use_lexical_oracle=True,
            floor_metrics=floor_metrics,
            max_body_chars=int(args.max_body_chars),
            llm_used=False,
        )
        payload = pilot.to_dict()
        payload["wave_b_gate_open"] = gate.wave_b_gate_open
        payload["human_go"] = gate.human_go
        payload["joined_count"] = join.joined_count
        payload["scored_case_count"] = pilot.case_count
        payload["operator_status"] = "oracle_ceiling"
        payload["note"] = (
            "Lexical-oracle over deterministic candidates (diagnostic ceiling). "
            "Not production LLM extract. Not import."
        )

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        raw_m = payload.get("metrics")
        raw_fm = payload.get("floor_metrics")
        m: dict = raw_m if isinstance(raw_m, dict) else {}
        fm: dict = raw_fm if isinstance(raw_fm, dict) else {}
        sys.stdout.write(
            "wave-b-gold-hybrid-constrained-pilot | "
            f"status: {payload.get('operator_status')} | "
            f"mode: {payload.get('mode')} | "
            f"joined: {payload.get('joined_count')} | "
            f"scored: {payload.get('scored_case_count')} | "
            f"entity_f1: {m.get('entity_f1')} | "
            f"relation_f1: {m.get('relation_f1')} | "
            f"floor_entity_f1: {fm.get('entity_f1')} | "
            f"floor_relation_f1: {fm.get('relation_f1')} | "
            f"gate: {payload.get('gate_verdict')} | "
            f"gate_open: {str(payload.get('wave_b_gate_open')).lower()} | "
            "llm: false | dspy: false | import_eligible: false | "
            "note: candidate_oracle_ceiling\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
