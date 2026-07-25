#!/usr/bin/env python3
"""Canary prediction resolvability operator (M284 S02).

Run LLM extraction on canary held-out bodies, ground predicted surfaces to
char spans, upgrade with layout page/bbox, measure prediction resolvability.

GT isolation: canary held-out only; never train. Never import. Never DSPy.

Usage::

    uv run python scripts/verify_canary_prediction_resolvability.py
    uv run python scripts/verify_canary_prediction_resolvability.py --limit 4
    uv run python scripts/verify_canary_prediction_resolvability.py --model glm-5.2
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

from research_graph.application.corpus.canary_gold_hybrid_join import (
    index_hybrid_bodies,
)
from research_graph.application.corpus.canary_prediction_resolvability import (
    evaluate_prediction_resolvability,
)
from research_graph.infrastructure.llm.ninerouter_json_extract import (
    NineRouterJsonExtractClient,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RUNS = Path("artifacts/etl/m283-hybrid-layout/runs")
DEFAULT_HELD_OUT = Path("artifacts/etl/canary-held-out-split.v1.json")
DEFAULT_OUTPUT = Path("artifacts/etl/canary-prediction-resolvability.v1.json")


def _load_held_out(repo: Path, path: Path) -> list[str]:
    p = path if path.is_absolute() else (repo / path)
    if not p.is_file():
        return []
    d = json.loads(p.read_text(encoding="utf-8"))
    return [str(x) for x in (d.get("held_out_ids") or []) if x]


def _build_cases(repo: Path, runs: Path, paper_ids: list[str]) -> list[dict]:
    index = index_hybrid_bodies([runs])
    cases: list[dict] = []
    for pid in paper_ids:
        body_path = index.get(pid)
        if not body_path:
            continue
        body_path = Path(body_path)
        body_text = body_path.read_text(encoding="utf-8") if body_path.is_file() else ""
        if not body_text:
            continue
        layout_path = body_path.with_name(f"{pid}.odl.layout.json")
        layout = None
        if layout_path.is_file():
            layout = json.loads(layout_path.read_text(encoding="utf-8"))
        cases.append(
            {
                "case_id": pid,
                "paper_id": pid,
                "body_text": body_text,
                "layout_json": layout,
            }
        )
    return cases


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Canary prediction resolvability")
    p.add_argument("--repo-root", type=Path, default=ROOT)
    p.add_argument("--runs", type=Path, default=DEFAULT_RUNS)
    p.add_argument("--held-out", type=Path, default=DEFAULT_HELD_OUT)
    p.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    p.add_argument("--limit", type=int, default=0, help="0 = all held-out")
    p.add_argument("--model", default="glm-5.2")
    p.add_argument("--max-tokens", type=int, default=1400)
    p.add_argument("--max-body-chars", type=int, default=8000)
    p.add_argument("--raw-body", action="store_true", help="raw body mode (no structured context)")
    p.add_argument("--json", action="store_true")
    args = p.parse_args(argv)
    repo = Path(args.repo_root)
    runs = args.runs if args.runs.is_absolute() else (repo / args.runs)
    held_out = _load_held_out(repo, args.held_out)
    if args.limit and args.limit > 0:
        held_out = held_out[: int(args.limit)]
    if not held_out:
        sys.stdout.write("canary-prediction-resolvability | no held-out ids\n")
        return 1

    cases = _build_cases(repo, runs, held_out)
    if not cases:
        sys.stdout.write("canary-prediction-resolvability | no cases with bodies\n")
        return 1

    client = NineRouterJsonExtractClient(
        model=args.model,
        max_tokens=int(args.max_tokens),
        use_structured_context=not bool(args.raw_body),
        max_followup_rounds=1,
    )
    predictions: list[dict] = []
    t0 = time.perf_counter()
    for case in cases:
        cid = case["case_id"]
        window = case["body_text"][: int(args.max_body_chars)]
        raw = client.extract_case(window, cid, paper_id=case["paper_id"])
        raw["case_id"] = cid
        predictions.append(raw)
        diag = client.last_diagnostics
        print(
            f"EXTRACT {cid} ok={diag.get('chat_ok')} json={diag.get('json_valid')} "
            f"ent={diag.get('entity_count')} rel={diag.get('relation_count')} "
            f"err={diag.get('error')}",
            flush=True,
        )
    duration = round(time.perf_counter() - t0, 2)

    pkg = evaluate_prediction_resolvability(
        cases=cases,
        predictions=predictions,
        target_rate=0.95,
        min_n=max(1, min(5, len(cases))),
        metric_mode="prediction_resolvability",
    )
    payload = pkg.to_dict()
    payload["model_id"] = args.model
    payload["context_mode"] = "raw_body" if args.raw_body else "structured_context"
    payload["duration_s"] = duration
    payload["last_extract_diagnostics"] = dict(client.last_diagnostics)
    payload["held_out_count"] = len(held_out)
    payload["cases_count"] = len(cases)

    out = args.output if args.output.is_absolute() else (repo / args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    line = (
        "canary-prediction-resolvability | "
        f"model: {args.model} | cases: {len(cases)} | "
        f"rate: {pkg.resolvability_rate} | page_bbox: {pkg.page_or_bbox_count} | "
        f"char_only: {pkg.char_only_count} | target_met: {str(pkg.target_met).lower()} | "
        f"duration_s: {duration} | import_eligible: false\n"
    )
    if args.json:
        sys.stdout.write(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    else:
        sys.stdout.write(line)
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
