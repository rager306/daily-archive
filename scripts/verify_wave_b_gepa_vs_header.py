#!/usr/bin/env python3
"""Same-n header vs offline GEPA instruction select (M268 S01).

Loads GEPA spike best_candidate, scores both header_priority_select and
GEPA instruction rule select on joined gold-hybrid cases.

Usage::

    uv run python scripts/verify_wave_b_gepa_vs_header.py
    uv run python scripts/verify_wave_b_gepa_vs_header.py --json \\
        --output artifacts/wave-b/gepa-vs-header-n23.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from research_graph.application.corpus.wave_b_extraction_baseline import (
    DEFAULT_HUMAN_GO_STAMP,
    read_human_go_stamp,
)
from research_graph.application.corpus.wave_b_gate import evaluate_wave_b_gate_from_stamp
from research_graph.application.corpus.wave_b_gepa_vs_header import (
    DEFAULT_MAX_VAL_GAP,
    candidate_from_gepa_artifact,
    compare_header_vs_gepa_instruction,
)
from research_graph.application.corpus.wave_b_gold_hybrid_join import (
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
DEFAULT_GEPA = Path("artifacts/wave-b/gepa-constrained-spike-n23-candfix2.json")
DEFAULT_OUTPUT = Path("artifacts/wave-b/gepa-vs-header-n23.json")


def _r(repo: Path, p: Path) -> Path:
    return p if p.is_absolute() else (repo / p).resolve()


def _load_cases(repo: Path) -> list[dict[str, Any]]:
    roots = tuple(_r(repo, Path(p)) for p in DEFAULT_BODY_ROOTS)
    train_g, _ = load_reviewed_extraction_split("train")
    val_g, _ = load_reviewed_extraction_split("validation")
    gold_all = train_g + val_g
    gold_by = {str(r.get("case_id")): r for r in gold_all if r.get("case_id")}
    join = inventory_reviewed_gold_hybrid_join(gold_records=gold_all, body_roots=roots)
    cases: list[dict[str, Any]] = []
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
                "paper_id": str(row.get("paper_id") or case_id),
                "gold": gold,
                "body_text": body,
            }
        )
    return cases


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Same-n header vs GEPA instruction select on gold-hybrid join. "
            "Import always false. Promote only dual F1 + val-gap (D128)."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--gepa-artifact", type=Path, default=DEFAULT_GEPA)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--stamp", type=Path, default=None)
    parser.add_argument("--max-val-gap", type=float, default=DEFAULT_MAX_VAL_GAP)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)
    repo = Path(args.repo_root)

    stamp_path = _r(
        repo,
        Path(args.stamp) if args.stamp is not None else Path(DEFAULT_HUMAN_GO_STAMP),
    )
    stamp = read_human_go_stamp(stamp_path)
    human_go = bool(stamp and stamp.get("human_go") is True)
    gate = evaluate_wave_b_gate_from_stamp(stamp_path)
    if not (human_go and gate.wave_b_gate_open):
        payload = {
            "schema_version": "wave-b-gepa-vs-header.v1",
            "operator_status": "blocked_gate",
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": "Stamp/gate closed; comparison not run",
        }
        text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
        if args.json:
            sys.stdout.write(text)
        else:
            sys.stdout.write(
                "wave-b-gepa-vs-header | status: blocked_gate | import_eligible: false\n"
            )
        return 0

    gepa_path = _r(repo, Path(args.gepa_artifact))
    if not gepa_path.is_file():
        sys.stderr.write(f"missing gepa artifact: {gepa_path}\n")
        return 2
    gepa_raw = json.loads(gepa_path.read_text(encoding="utf-8"))
    candidate = candidate_from_gepa_artifact(gepa_raw)
    # merge train/val from spike best_metrics for val-gap guard
    bm = gepa_raw.get("best_metrics") if isinstance(gepa_raw, dict) else None
    if isinstance(bm, dict):
        if bm.get("train_entity_f1") is not None:
            candidate["train_entity_f1"] = bm["train_entity_f1"]
        if bm.get("val_entity_f1") is not None:
            candidate["val_entity_f1"] = bm["val_entity_f1"]

    cases = _load_cases(repo)
    if not cases:
        sys.stderr.write("no joined gold-hybrid cases\n")
        return 2
    floor = score_gold_hybrid_lexical_recovery(cases=cases)
    pkg = compare_header_vs_gepa_instruction(
        cases=cases,
        gepa_candidate=candidate,
        floor_metrics=floor.metrics,
        max_val_gap=float(args.max_val_gap),
    )
    payload = pkg.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["gepa_artifact"] = str(gepa_path)
    payload["human_go"] = True
    payload["operator_status"] = "compared"

    out = _r(repo, Path(args.output))
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        h = payload["header"]
        g = payload["gepa"]
        d = payload["delta_vs_header"]
        sys.stdout.write(
            "wave-b-gepa-vs-header | "
            f"joined: {payload['joined_count']} | "
            f"header: e={h.get('entity_f1')} r={h.get('relation_f1')} | "
            f"gepa: e={g.get('entity_f1')} r={g.get('relation_f1')} | "
            f"delta: e={d.get('entity_f1')} r={d.get('relation_f1')} | "
            f"promote_ready: {str(payload['promote_ready']).lower()} | "
            f"blockers: {len(payload.get('promote_blockers') or [])} | "
            "import_eligible: false\n"
        )
        if payload.get("promote_blockers"):
            sys.stdout.write(
                "  blockers: " + ", ".join(payload["promote_blockers"]) + "\n"
            )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
