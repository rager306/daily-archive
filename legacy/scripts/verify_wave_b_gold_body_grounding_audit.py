#!/usr/bin/env python3
"""Operator: audit reviewed gold labels vs hybrid body/candidates.

Usage::

    uv run python scripts/verify_wave_b_gold_body_grounding_audit.py
    uv run python scripts/verify_wave_b_gold_body_grounding_audit.py --json
    uv run python scripts/verify_wave_b_gold_body_grounding_audit.py \\
        --output artifacts/wave-b/gold-body-grounding-audit.json
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
from research_graph.application.corpus.wave_b_gold_body_grounding_audit import (
    audit_gold_body_grounding,
)
from research_graph.application.corpus.wave_b_gold_hybrid_join import (
    inventory_reviewed_gold_hybrid_join,
)
from research_graph.application.reviewed_extraction_fixtures import (
    load_reviewed_extraction_split,
)
from research_graph.workflows.composition.etl_body_coverage import DEFAULT_BODY_ROOTS

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Wave B gold↔body grounding audit")
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--no-stamp", action="store_true")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--max-body-chars", type=int, default=8000)
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
    train_g, _ = load_reviewed_extraction_split("train")
    val_g, _ = load_reviewed_extraction_split("validation")
    gold_all = train_g + val_g
    gold_by = {str(r.get("case_id")): r for r in gold_all if r.get("case_id")}
    join = inventory_reviewed_gold_hybrid_join(gold_records=gold_all, body_roots=roots)

    cases: list[dict] = []
    if gate.wave_b_gate_open and gate.human_go:
        for row in join.joined:
            case_id = str(row.get("case_id") or "")
            gold = gold_by.get(case_id)
            if gold is None:
                continue
            path = Path(str(row.get("body_path") or ""))
            try:
                body = path.read_text(encoding="utf-8") if path.is_file() else ""
            except OSError:
                body = ""
            cases.append(
                {
                    "case_id": case_id,
                    "paper_id": str(row.get("paper_id") or ""),
                    "gold": gold,
                    "body_text": body,
                }
            )

    if not cases:
        payload = {
            "schema_version": "wave-b-gold-body-grounding-audit.v1",
            "operator_status": "blocked_gate",
            "case_count": 0,
            "candidate_coverage_ratio": 0.0,
            "import_eligible": False,
            "wave_b_gate_open": gate.wave_b_gate_open,
            "human_go": gate.human_go,
            "joined_count": join.joined_count,
        }
    else:
        audit = audit_gold_body_grounding(
            cases=cases, max_body_chars=int(args.max_body_chars)
        )
        payload = audit.to_dict()
        payload["operator_status"] = (
            "pass" if audit.candidate_coverage_ratio >= 1.0 else "debt"
        )
        payload["wave_b_gate_open"] = gate.wave_b_gate_open
        payload["human_go"] = gate.human_go
        payload["joined_count"] = join.joined_count

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
    if args.json:
        sys.stdout.write(text)
    else:
        ungrounded_raw = payload.get("ungrounded")
        ungrounded_n = len(ungrounded_raw) if isinstance(ungrounded_raw, list) else 0
        sys.stdout.write(
            "wave-b-gold-body-grounding-audit | "
            f"status: {payload.get('operator_status')} | "
            f"cases: {payload.get('case_count')} | "
            f"body_ratio: {payload.get('body_coverage_ratio')} | "
            f"cand_ratio: {payload.get('candidate_coverage_ratio')} | "
            f"ungrounded: {ungrounded_n} | "
            "import_eligible: false\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
