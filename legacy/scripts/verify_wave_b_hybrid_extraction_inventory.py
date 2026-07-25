#!/usr/bin/env python3
"""Wave B hybrid extraction inventory + disagreements operator (M253 / D124).

Composes:
  - durable human_go stamp → Wave B gate
  - unique hybrid bodies as extraction candidates (metadata only)
  - Reviewed disagreement rollup via extraction harness

No LLM fleet, no DSPy optimizer, no import.

Usage::

    uv run python scripts/verify_wave_b_hybrid_extraction_inventory.py
    uv run python scripts/verify_wave_b_hybrid_extraction_inventory.py --json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_disagreement_inventory import (
    inventory_reviewed_extraction_disagreements,
)
from research_graph.application.corpus.wave_b_extraction_baseline import (
    DEFAULT_HUMAN_GO_STAMP,
)
from research_graph.application.corpus.wave_b_gate import evaluate_wave_b_gate_from_stamp
from research_graph.application.corpus.wave_b_hybrid_extraction_inventory import (
    inventory_hybrid_extraction_candidates,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B: stamp gate + hybrid extraction candidates + reviewed disagreements. "
            "Import always false. No DSPy optimizer. No LLM run."
        )
    )
    parser.add_argument(
        "--stamp-path",
        type=Path,
        default=DEFAULT_HUMAN_GO_STAMP,
        help="Durable Wave B human_go stamp path",
    )
    parser.add_argument(
        "--body-root",
        action="append",
        type=Path,
        default=None,
        help="Hybrid body root (repeatable). Default: DEFAULT_BODY_ROOTS",
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--sample-limit", type=int, default=12)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _resolve(p: Path) -> Path:
        path = Path(p)
        return path if path.is_absolute() else (repo / path).resolve()

    stamp_path = _resolve(args.stamp_path)
    raw_roots = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    body_roots = tuple(_resolve(r) for r in raw_roots)

    gate = evaluate_wave_b_gate_from_stamp(stamp_path)
    hybrid = inventory_hybrid_extraction_candidates(
        body_roots=body_roots,
        sample_limit=args.sample_limit,
    )
    # reviewed fixture loader defaults to artifact root; run from repo root
    disagreements = inventory_reviewed_extraction_disagreements(sample_limit=args.sample_limit)

    payload = {
        "schema_version": "m253-wave-b-hybrid-extraction-inventory-report.v1",
        "wave": "B",
        "gate": gate.to_dict(),
        "hybrid_inventory": hybrid.to_dict(),
        "disagreement_inventory": disagreements.to_dict(),
        "import_eligible": False,
        "dspy_optimizer_enabled": False,
        "note": (
            "Wave B inventory only; stamp opens gate; no LLM extraction; "
            "not import; not DSPy optimizer"
        ),
    }

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = _resolve(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        kinds = disagreements.disagreement_kind_counts
        kind_s = (
            ",".join(f"{k}={v}" for k, v in sorted(kinds.items())) if kinds else "none"
        )
        sys.stdout.write(
            "wave-b-hybrid-extraction-inventory | "
            f"gate: {gate.gate_signal} | "
            f"open: {str(gate.wave_b_gate_open).lower()} | "
            f"human_go: {str(gate.human_go).lower()} | "
            f"hybrid_candidates: {hybrid.candidate_count} | "
            f"empty: {hybrid.empty_count} | "
            f"total_words: {hybrid.total_words} | "
            f"reviewed_train_disagreements: {disagreements.train_disagreement_count} | "
            f"reviewed_val_disagreements: {disagreements.validation_disagreement_count} | "
            f"kinds: {kind_s} | "
            f"train_entity_f1: {disagreements.train_entity_f1} | "
            "dspy: false | import_eligible: false\n"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
