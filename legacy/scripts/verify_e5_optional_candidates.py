#!/usr/bin/env python3
"""E5 optional candidates operator (M284 S03).

Runs header-priority offline generator on canary held-out bodies, documents
GLiNER availability, Docling fallback gate, and optional blind second judge
vs prior LLM predictions when available.

Never invent gold. Never import. Never DSPy.

Usage::

    uv run python scripts/verify_e5_optional_candidates.py
    uv run python scripts/verify_e5_optional_candidates.py --limit 5
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.canary_gold_hybrid_join import (
    index_hybrid_bodies,
)
from research_graph.application.corpus.e5_optional_candidates import (
    build_e5_optional_candidates_package,
    docling_available,
    gliner_available,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RUNS = Path("artifacts/etl/m283-hybrid-layout/runs")
DEFAULT_HELD_OUT = Path("artifacts/etl/canary-held-out-split.v1.json")
DEFAULT_PRED = Path("artifacts/etl/canary-prediction-resolvability.v1.json")
DEFAULT_OUTPUT = Path("artifacts/etl/e5-optional-candidates.v1.json")


def _load_held_out(repo: Path, path: Path) -> list[str]:
    p = path if path.is_absolute() else (repo / path)
    if not p.is_file():
        return []
    d = json.loads(p.read_text(encoding="utf-8"))
    return [str(x) for x in (d.get("held_out_ids") or []) if x]


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="E5 optional candidates verify")
    p.add_argument("--repo-root", type=Path, default=ROOT)
    p.add_argument("--runs", type=Path, default=DEFAULT_RUNS)
    p.add_argument("--held-out", type=Path, default=DEFAULT_HELD_OUT)
    p.add_argument("--predictions", type=Path, default=DEFAULT_PRED)
    p.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    p.add_argument("--limit", type=int, default=0)
    p.add_argument("--json", action="store_true")
    args = p.parse_args(argv)
    repo = Path(args.repo_root)
    runs = args.runs if args.runs.is_absolute() else (repo / args.runs)
    held_out = _load_held_out(repo, args.held_out)
    if args.limit and args.limit > 0:
        held_out = held_out[: int(args.limit)]

    index = index_hybrid_bodies([runs])
    gen_rows: list[dict] = []
    header_entities_total = 0
    for pid in held_out:
        body_path = index.get(pid)
        if not body_path:
            continue
        body_path = Path(body_path)
        if not body_path.is_file():
            continue
        body = body_path.read_text(encoding="utf-8")
        pkg = build_e5_optional_candidates_package(
            body_text=body,
            paper_id=pid,
            case_id=pid,
            hybrid_failed=False,
        )
        gen = next(
            (g for g in pkg.generators if g.get("generator") == "header_priority"),
            {},
        )
        header_entities_total += int(gen.get("entity_count") or 0)
        gen_rows.append(
            {
                "paper_id": pid,
                "header_entity_count": gen.get("entity_count"),
                "header_relation_count": gen.get("relation_count"),
                "candidate_pool_size": gen.get("candidate_pool_size"),
                "entities": gen.get("entities"),
                "relations": gen.get("relations"),
            }
        )

    # Second judge: header-priority vs LLM predictions when available
    pred_path = (
        args.predictions
        if args.predictions.is_absolute()
        else (repo / args.predictions)
    )
    second_judge = None
    primary: list[dict] = []
    secondary: list[dict] = []
    if pred_path.is_file():
        pred_doc = json.loads(pred_path.read_text(encoding="utf-8"))
        # per_paper has entity_count but not full predictions; rebuild header as primary
        for row in gen_rows:
            primary.append(
                {
                    "case_id": row["paper_id"],
                    "entities": row.get("entities") or [],
                    "relations": row.get("relations") or [],
                }
            )
        # LLM predictions not stored full in resolvability artifact — judge only if shape present
        if isinstance(pred_doc.get("predictions"), list):
            secondary = list(pred_doc["predictions"])
        if primary and secondary:
            from research_graph.application.corpus.e5_optional_candidates import (
                blind_second_judge,
            )

            second_judge = blind_second_judge(
                primary=primary, secondary=secondary
            ).to_dict()

    sample = gen_rows[0] if gen_rows else {}
    sample_body = ""
    if gen_rows:
        bp = index.get(gen_rows[0]["paper_id"])
        if bp and Path(bp).is_file():
            sample_body = Path(bp).read_text(encoding="utf-8")[:4000]

    pkg = build_e5_optional_candidates_package(
        body_text=sample_body,
        paper_id=str(sample.get("paper_id") or ""),
        case_id=str(sample.get("paper_id") or ""),
        hybrid_failed=False,
    )
    payload = pkg.to_dict()
    payload["held_out_count"] = len(held_out)
    payload["papers_with_body"] = len(gen_rows)
    payload["header_entities_total"] = header_entities_total
    payload["per_paper"] = gen_rows
    payload["gliner_available"] = gliner_available()
    payload["docling_available"] = docling_available()
    if second_judge:
        payload["second_judge"] = second_judge
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False

    out = args.output if args.output.is_absolute() else (repo / args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    line = (
        "e5-optional-candidates | "
        f"papers: {len(gen_rows)} | header_entities_total: {header_entities_total} | "
        f"gliner: {str(gliner_available()).lower()} | "
        f"docling: {str(docling_available()).lower()} | "
        f"second_judge: {second_judge is not None} | "
        "import_eligible: false\n"
    )
    if args.json:
        sys.stdout.write(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    else:
        sys.stdout.write(line)
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
