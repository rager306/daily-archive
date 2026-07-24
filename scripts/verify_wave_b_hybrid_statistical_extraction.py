#!/usr/bin/env python3
"""Wave B hybrid statistical extraction operator (M255 S02).

Samples hybrid bodies, runs deterministic statistical-first extraction
(token-frequency + co-occurrence). Stamp-aware gate. No LLM, no DSPy, no import.

Usage::

    uv run python scripts/verify_wave_b_hybrid_statistical_extraction.py
    uv run python scripts/verify_wave_b_hybrid_statistical_extraction.py --json
    uv run python scripts/verify_wave_b_hybrid_statistical_extraction.py --no-stamp
    uv run python scripts/verify_wave_b_hybrid_statistical_extraction.py --sample-limit 5
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
from research_graph.application.corpus.wave_b_hybrid_extraction_inventory import (
    inventory_hybrid_extraction_candidates,
)
from research_graph.application.corpus.wave_b_hybrid_statistical_extraction import (
    build_hybrid_statistical_extraction,
    build_hybrid_statistical_fleet,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B hybrid statistical extraction. "
            "Stamp-aware; never LLM/DSPy/import."
        )
    )
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--sample-limit", type=int, default=10)
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
    inv = inventory_hybrid_extraction_candidates(
        body_roots=roots,
        sample_limit=max(args.sample_limit, 200),
    )
    limit = max(0, int(args.sample_limit))
    selected = inv.candidates[:limit]
    packages = []
    for cand in selected:
        try:
            text = Path(cand.path).read_text(encoding="utf-8")
        except OSError:
            text = ""
        packages.append(
            build_hybrid_statistical_extraction(
                paper_id=cand.paper_id,
                body_text=text,
                body_path=cand.path,
            )
        )
    fleet = build_hybrid_statistical_fleet(
        packages=packages,
        wave_b_gate_open=gate.wave_b_gate_open,
        human_go=gate.human_go,
    )
    payload = fleet.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["dspy_optimizer_enabled"] = False
    payload["llm_used"] = False
    payload["inventory_candidate_count"] = inv.candidate_count
    payload["sample_limit"] = limit

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "wave-b-hybrid-statistical-extraction | "
            f"status: {fleet.fleet_status} | "
            f"papers: {fleet.paper_count} | "
            f"ready: {fleet.statistical_ready_count} | "
            f"empty: {fleet.empty_count} | "
            f"keywords: {fleet.total_keywords} | "
            f"relations: {fleet.total_candidate_relations} | "
            f"words: {fleet.total_words} | "
            f"gate_open: {str(fleet.wave_b_gate_open).lower()} | "
            f"human_go: {str(fleet.human_go).lower()} | "
            "llm: false | dspy: false | import_eligible: false\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
