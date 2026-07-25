#!/usr/bin/env python3
"""Recompute REAL canary gold↔hybrid resolvability (not demo).

Usage::

    uv run python scripts/verify_canary_gold_hybrid_resolvability.py
    uv run python scripts/verify_canary_gold_hybrid_resolvability.py --json \\
        --output artifacts/etl/canary-gold-hybrid-resolvability.v1.json

Never import. Distinguishes demo vs real metric_mode.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.canary_gold_hybrid_join import (
    evaluate_joined_canary_resolvability,
    load_gold_jsonl,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/etl/canary-gold-hybrid-resolvability.v1.json")
DEFAULT_GOLD = [
    Path("artifacts/m072-reviewed-extraction-benchmark/fixtures/train-gold.jsonl"),
    Path("artifacts/m072-reviewed-extraction-benchmark/fixtures/validation-gold.jsonl"),
]
DEFAULT_BODY_ROOT = Path("artifacts/m213-hybrid-gate")


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Real gold+hybrid resolvability report")
    p.add_argument("--repo-root", type=Path, default=ROOT)
    p.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    p.add_argument("--body-root", action="append", default=None)
    p.add_argument("--gold", action="append", default=None)
    p.add_argument("--target-rate", type=float, default=0.95)
    p.add_argument("--json", action="store_true")
    args = p.parse_args(argv)
    repo = Path(args.repo_root)

    gold_paths = [repo / Path(g) for g in (args.gold or DEFAULT_GOLD)]
    body_roots = [repo / Path(b) for b in (args.body_root or [DEFAULT_BODY_ROOT])]
    gold_rows: list[dict] = []
    for gp in gold_paths:
        if gp.is_file():
            gold_rows.extend(load_gold_jsonl(gp))

    pkg = evaluate_joined_canary_resolvability(
        gold_rows=gold_rows,
        body_roots=body_roots,
        target_rate=float(args.target_rate),
    )
    payload = pkg.to_dict()
    payload["metric_mode"] = "real_gold_hybrid_join"
    payload["demo_metric"] = False
    payload["import_eligible"] = False
    payload["governor"] = {
        "demo_vs_real": "REAL join metric (not demo_placeholder)",
        "target_met_policy": "requires real mode + min_n + rate>=target",
        "char_only_alert": (payload.get("resolvability") or {}).get("alerts"),
    }

    out = args.output if args.output.is_absolute() else (repo / args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    r = payload.get("resolvability") or {}
    line = (
        "canary-gold-hybrid-resolvability | "
        f"mode: real_gold_hybrid_join | "
        f"joined: {payload.get('joined_count')} | "
        f"entity: {payload.get('entity_grounded')}/{payload.get('entity_total')} | "
        f"rel: {payload.get('relation_grounded')}/{payload.get('relation_total')} | "
        f"rate: {r.get('resolvability_rate')} | "
        f"target_met: {str(r.get('target_met')).lower()} | "
        f"page_bbox: {r.get('page_or_bbox_count')} | "
        f"alerts: {len(r.get('alerts') or [])} | "
        "import_eligible: false\n"
    )
    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(line)
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
