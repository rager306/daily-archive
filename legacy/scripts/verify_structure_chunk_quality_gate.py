#!/usr/bin/env python3
"""Continuous hybrid-body chunk quality gate (M273).

Usage::

    uv run python scripts/verify_structure_chunk_quality_gate.py
    uv run python scripts/verify_structure_chunk_quality_gate.py --json \\
        --output artifacts/etl/structure-chunk-quality-gate.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.structure_chunk_quality_gate import (
    evaluate_structure_chunk_quality_gate,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/etl/structure-chunk-quality-gate.json")


def _r(repo: Path, p: Path) -> Path:
    return p if p.is_absolute() else (repo / p).resolve()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Continuous sample gate on hybrid body quality + structure signals. "
            "Import always false."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--sample-limit", type=int, default=40)
    parser.add_argument("--min-sample", type=int, default=10)
    parser.add_argument("--min-pass-rate", type=float, default=0.55)
    parser.add_argument(
        "--body-root",
        action="append",
        default=None,
        help="Body root (repeatable); default DEFAULT_BODY_ROOTS",
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    repo = Path(args.repo_root)
    raw = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    roots = [_r(repo, Path(p)) for p in raw]
    pkg = evaluate_structure_chunk_quality_gate(
        roots,
        sample_limit=int(args.sample_limit),
        min_sample=int(args.min_sample),
        min_pass_rate=float(args.min_pass_rate),
    )
    payload = pkg.to_dict()
    payload["import_eligible"] = False
    out = _r(repo, Path(args.output))
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")
    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "structure-chunk-quality-gate | "
            f"signal: {payload['gate_signal']} | "
            f"sampled: {payload['sampled']} | "
            f"passed: {payload['passed']} | "
            f"soft: {payload['soft_signal']} | "
            f"low: {payload['low_quality']} | "
            f"pass_rate: {payload['pass_rate']} | "
            f"gap_cleared: {str(payload['continuity_gap_cleared']).lower()} | "
            "import_eligible: false\n"
        )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
