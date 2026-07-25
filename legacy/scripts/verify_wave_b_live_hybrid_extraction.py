#!/usr/bin/env python3
"""Wave B live hybrid extraction scaffold operator (M254 S05).

Builds a fail-closed package over hybrid extraction candidates.
Default: read durable human_go stamp. Never import / never DSPy / no LLM.

Usage::

    uv run python scripts/verify_wave_b_live_hybrid_extraction.py
    uv run python scripts/verify_wave_b_live_hybrid_extraction.py --json
    uv run python scripts/verify_wave_b_live_hybrid_extraction.py --no-stamp
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
from research_graph.application.corpus.wave_b_live_hybrid_extraction import (
    build_wave_b_live_hybrid_extraction,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B live hybrid extraction scaffold. "
            "Stamp-aware gate; never import; no DSPy; no LLM."
        )
    )
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--sample-limit", type=int, default=40)
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
    package = build_wave_b_live_hybrid_extraction(
        candidates=inv.candidates,
        wave_b_gate_open=gate.wave_b_gate_open,
        human_go=gate.human_go,
        sample_limit=args.sample_limit,
    )
    payload = package.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["dspy_optimizer_enabled"] = False
    payload["inventory_candidate_count"] = inv.candidate_count

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "wave-b-live-hybrid-extraction | "
            f"status: {package.extraction_status} | "
            f"candidates: {package.candidate_count} | "
            f"empty: {package.empty_count} | "
            f"sampled: {package.sampled_count} | "
            f"gate_open: {str(package.wave_b_gate_open).lower()} | "
            f"human_go: {str(package.human_go).lower()} | "
            "dspy: false | import_eligible: false\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
