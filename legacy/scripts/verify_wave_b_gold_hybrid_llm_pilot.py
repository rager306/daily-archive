#!/usr/bin/env python3
"""Wave B gold-linked hybrid LLM extraction pilot operator.

Joins reviewed gold to hybrid bodies, runs injectible 9router JSON extract,
scores vs gold and reports lexical floor comparison. Stamp-aware gate.
Never DSPy. Never import.

Usage::

    uv run python scripts/verify_wave_b_gold_hybrid_llm_pilot.py
    uv run python scripts/verify_wave_b_gold_hybrid_llm_pilot.py --json
    uv run python scripts/verify_wave_b_gold_hybrid_llm_pilot.py --no-stamp
    uv run python scripts/verify_wave_b_gold_hybrid_llm_pilot.py --model agnes-ai/agnes-2.0-flash
    uv run python scripts/verify_wave_b_gold_hybrid_llm_pilot.py --dry-run-floor-only
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
    GoldHybridJoinPackage,
    inventory_reviewed_gold_hybrid_join,
)
from research_graph.application.corpus.wave_b_gold_hybrid_lexical_metrics import (
    score_gold_hybrid_lexical_recovery,
)
from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    score_gold_hybrid_llm_pilot,
)
from research_graph.application.reviewed_extraction_fixtures import (
    load_reviewed_extraction_split,
)
from research_graph.infrastructure.llm.ninerouter_json_extract import (
    AGNES_25_PILOT_MODEL,
    AGNES_FREE_25_PILOT_MODEL,
    DEFAULT_PILOT_MODEL,
    QUALITY_PILOT_MODEL,
    NineRouterJsonExtractClient,
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
            "Wave B gold-hybrid LLM extract pilot. "
            "Stamp-aware; never DSPy/import. Compare to lexical floor."
        )
    )
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument(
        "--model",
        default=DEFAULT_PILOT_MODEL,
        help=(
            f"9router model id (default {DEFAULT_PILOT_MODEL}; "
            f"agnes2.5={AGNES_25_PILOT_MODEL}; "
            f"agnes2.5-free={AGNES_FREE_25_PILOT_MODEL}; "
            f"quality={QUALITY_PILOT_MODEL})"
        ),
    )
    parser.add_argument(
        "--max-body-chars",
        type=int,
        default=8000,
        help="Truncate hybrid body window for pilot cost control",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=0,
        help="Chat max_tokens (0 = 900 default, 1400 for agnes-ai-free/*)",
    )
    parser.add_argument(
        "--dry-run-floor-only",
        action="store_true",
        help="Score lexical floor only; do not call LLM",
    )
    parser.add_argument(
        "--case-limit",
        type=int,
        default=0,
        help="Optional max cases (0 = all joined)",
    )
    parser.add_argument(
        "--structured",
        action="store_true",
        default=True,
        help="Feed structured context pack (outline/sections/candidates; default)",
    )
    parser.add_argument(
        "--raw-body",
        action="store_true",
        help="Legacy: feed truncated raw hybrid markdown only",
    )
    parser.add_argument(
        "--max-followup-rounds",
        type=int,
        default=1,
        help="Allow model to request more sections N times (structured mode)",
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
    if args.case_limit and args.case_limit > 0:
        cases = cases[: args.case_limit]

    floor_pkg = score_gold_hybrid_lexical_recovery(cases=cases)
    floor_metrics = dict(floor_pkg.metrics)

    if args.dry_run_floor_only or not (gate.wave_b_gate_open and gate.human_go):
        payload = {
            "schema_version": "wave-b-reviewed-gold-hybrid-llm-pilot.v1",
            "wave": "B",
            "case_count": floor_pkg.case_count,
            "metrics": None,
            "floor_metrics": floor_metrics,
            "gate_verdict": floor_pkg.gate_verdict,
            "gate_reasons": list(floor_pkg.gate_reasons),
            "per_case": list(floor_pkg.per_case),
            "llm_used": False,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "model_id": "",
            "wave_b_gate_open": gate.wave_b_gate_open,
            "human_go": gate.human_go,
            "joined_count": join.joined_count,
            "scored_case_count": floor_pkg.case_count,
            "operator_status": (
                "floor_only"
                if args.dry_run_floor_only and gate.wave_b_gate_open and gate.human_go
                else "blocked_gate"
            ),
            "note": "LLM pilot not executed (dry-run or gate closed)",
        }
    else:
        model_id = str(args.model)
        max_tokens = int(args.max_tokens)
        if max_tokens <= 0:
            max_tokens = 1400 if model_id.startswith("agnes-ai-free/") else 900
        use_structured = not bool(args.raw_body)
        client = NineRouterJsonExtractClient(
            model=model_id,
            max_tokens=max_tokens,
            use_structured_context=use_structured,
            max_followup_rounds=int(args.max_followup_rounds),
        )
        pilot = score_gold_hybrid_llm_pilot(
            cases=cases,
            extract_fn=client.as_extract_fn(),
            floor_metrics=floor_metrics,
            model_id=model_id,
            max_body_chars=int(args.max_body_chars),
        )
        payload = pilot.to_dict()
        payload["wave_b_gate_open"] = gate.wave_b_gate_open
        payload["human_go"] = gate.human_go
        payload["joined_count"] = join.joined_count
        payload["scored_case_count"] = pilot.case_count
        payload["operator_status"] = "sampled"
        payload["context_mode"] = (
            "structured_context" if use_structured else "raw_body"
        )
        payload["last_extract_diagnostics"] = dict(client.last_diagnostics)

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
            "wave-b-gold-hybrid-llm-pilot | "
            f"status: {payload.get('operator_status')} | "
            f"joined: {payload.get('joined_count')} | "
            f"scored: {payload.get('scored_case_count')} | "
            f"entity_f1: {m.get('entity_f1')} | "
            f"relation_f1: {m.get('relation_f1')} | "
            f"floor_entity_f1: {fm.get('entity_f1')} | "
            f"floor_relation_f1: {fm.get('relation_f1')} | "
            f"gate: {payload.get('gate_verdict')} | "
            f"gate_open: {str(payload.get('wave_b_gate_open')).lower()} | "
            f"llm: {str(payload.get('llm_used')).lower()} | "
            f"model: {payload.get('model_id') or 'none'} | "
            "dspy: false | import_eligible: false\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
