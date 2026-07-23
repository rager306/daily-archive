#!/usr/bin/env python3
"""Wave B extraction quality baseline operator (M252 / D124).

Scores M072 reviewed fixtures via M202 harness (no DSPy optimizer, no import).
Writes durable human_go stamp for Wave B (not import authorization).

Usage::

    uv run python scripts/verify_wave_b_extraction_baseline.py
    uv run python scripts/verify_wave_b_extraction_baseline.py --json
    uv run python scripts/verify_wave_b_extraction_baseline.py --stamp-only
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_extraction_baseline import (
    DEFAULT_HUMAN_GO_STAMP,
    build_wave_b_extraction_baseline,
    read_human_go_stamp,
    write_human_go_stamp,
)

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B extraction baseline on M072 fixtures. "
            "Writes human_go stamp (D124). Never import. Never DSPy optimizer."
        )
    )
    parser.add_argument(
        "--stamp-path",
        type=Path,
        default=DEFAULT_HUMAN_GO_STAMP,
        help="Path for durable Wave B human_go stamp",
    )
    parser.add_argument(
        "--decision-ref",
        type=str,
        default="D124",
        help="Decision id for human go stamp",
    )
    parser.add_argument(
        "--authorized-by",
        type=str,
        default="user",
        help="Who authorized Wave B",
    )
    parser.add_argument(
        "--stamp-only",
        action="store_true",
        help="Only write/read stamp; skip fixture scoring",
    )
    parser.add_argument(
        "--skip-stamp",
        action="store_true",
        help="Do not write stamp (score only)",
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    stamp_path = Path(args.stamp_path)
    if not stamp_path.is_absolute():
        stamp_path = (repo / stamp_path).resolve()

    stamp_payload: dict | None = None
    if not args.skip_stamp:
        stamp_payload = write_human_go_stamp(
            stamp_path,
            authorized_by=args.authorized_by,
            decision_ref=args.decision_ref,
            note="Wave B extraction quality authorized (user go; D124)",
        )
    else:
        stamp_payload = read_human_go_stamp(stamp_path)

    if args.stamp_only:
        payload = {
            "schema_version": "m252-wave-b-extraction-baseline-report.v1",
            "stamp": stamp_payload,
            "stamp_path": str(stamp_path),
            "baseline": None,
            "import_eligible": False,
            "dspy_optimizer_enabled": False,
            "wave": "B",
        }
    else:
        # load_m072_split uses cwd-relative fixtures path
        package = build_wave_b_extraction_baseline(human_go=True)
        payload = {
            "schema_version": "m252-wave-b-extraction-baseline-report.v1",
            "stamp": stamp_payload,
            "stamp_path": str(stamp_path),
            "baseline": package.to_dict(),
            "import_eligible": False,
            "dspy_optimizer_enabled": False,
            "wave": "B",
        }

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "wave-b-extraction-baseline | "
            f"stamp: {stamp_path.name if stamp_payload else 'none'} | "
            f"human_go: {bool(stamp_payload and stamp_payload.get('human_go'))} | "
        )
        baseline = payload.get("baseline")
        reasons: list[object] | None = None
        if isinstance(baseline, dict):
            tm_raw = baseline.get("train_metrics")
            vm_raw = baseline.get("validation_metrics")
            tm = tm_raw if isinstance(tm_raw, dict) else {}
            vm = vm_raw if isinstance(vm_raw, dict) else {}
            sys.stdout.write(
                f"train_cases: {baseline.get('train_case_count')} | "
                f"val_cases: {baseline.get('validation_case_count')} | "
                f"train_entity_f1: {tm.get('entity_f1')} | "
                f"train_relation_f1: {tm.get('relation_f1')} | "
                f"val_entity_f1: {vm.get('entity_f1')} | "
                f"gate: {baseline.get('gate_verdict')} | "
                f"leakage_clean: {baseline.get('leakage_clean')} | "
            )
            gr = baseline.get("gate_reasons")
            if isinstance(gr, list):
                reasons = gr
        sys.stdout.write("dspy: false | import_eligible: false\n")
        if reasons:
            sys.stdout.write(
                "  gate_reasons: "
                + "; ".join(str(r) for r in reasons[:4])
                + "\n"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
