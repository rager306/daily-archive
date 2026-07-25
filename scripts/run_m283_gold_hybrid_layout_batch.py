#!/usr/bin/env python3
"""Live hybrid re-run for m072 gold PDFs with ODL JSON+markdown layout (M283).

Writes under artifacts/etl/m283-hybrid-layout/runs/<paper_id>/body/:
  *.hybrid.body.md, *.odl.layout.json, *.canonical.json, *.grobid.tei.xml, ...

Never import. Never DSPy.

Usage::

    uv run python scripts/run_m283_gold_hybrid_layout_batch.py
    uv run python scripts/run_m283_gold_hybrid_layout_batch.py --limit 3
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import traceback
from pathlib import Path

from research_graph.application.corpus.canary_gold_hybrid_join import load_gold_jsonl
from research_graph.workflows.composition.hybrid_live_ports import resolve_live_hybrid_ports
from research_graph.workflows.composition.parser_body_resolve import (
    ArticleBodyRequest,
    resolve_article_body,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = Path("artifacts/etl/m283-hybrid-layout")
DEFAULT_GOLD = [
    Path("artifacts/m072-reviewed-extraction-benchmark/fixtures/train-gold.jsonl"),
    Path("artifacts/m072-reviewed-extraction-benchmark/fixtures/validation-gold.jsonl"),
]


def _norm_pid(raw: str) -> str:
    return str(raw or "").replace("arxiv:", "").strip()


def _find_pdf(repo: Path, paper_id: str) -> Path | None:
    # Prefer canonical catalog
    hits = list((repo / "data/article_catalog").rglob(f"{paper_id}.pdf")) if (
        repo / "data/article_catalog"
    ).exists() else []
    if hits:
        return hits[0]
    hits = list((repo / "artifacts/etl/gold-pdf-cache").rglob(f"{paper_id}.pdf")) if (
        repo / "artifacts/etl/gold-pdf-cache"
    ).exists() else []
    return hits[0] if hits else None


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="M283 gold hybrid layout batch")
    p.add_argument("--repo-root", type=Path, default=ROOT)
    p.add_argument("--output-root", type=Path, default=DEFAULT_OUT)
    p.add_argument("--limit", type=int, default=0, help="0 = all gold papers")
    p.add_argument("--json", action="store_true")
    p.add_argument("--ensure-containers", action="store_true", default=False)
    args = p.parse_args(argv)
    repo = Path(args.repo_root)
    out_root = args.output_root if args.output_root.is_absolute() else (repo / args.output_root)
    runs = out_root / "runs"
    runs.mkdir(parents=True, exist_ok=True)

    gold_rows: list[dict] = []
    for g in DEFAULT_GOLD:
        gp = repo / g
        if gp.is_file():
            gold_rows.extend(load_gold_jsonl(gp))
    paper_ids: list[str] = []
    seen: set[str] = set()
    for row in gold_rows:
        pid = _norm_pid(str(row.get("paper_id") or ""))
        if pid and pid not in seen:
            seen.add(pid)
            paper_ids.append(pid)
    if args.limit and args.limit > 0:
        paper_ids = paper_ids[: int(args.limit)]

    ports = resolve_live_hybrid_ports(
        enable=True, ensure_containers=bool(args.ensure_containers)
    )
    rows: list[dict] = []
    t0 = time.perf_counter()
    for pid in paper_ids:
        pdf = _find_pdf(repo, pid)
        row: dict = {
            "paper_id": pid,
            "pdf": str(pdf) if pdf else None,
            "ok": False,
            "import_eligible": False,
        }
        if pdf is None:
            row["error"] = "pdf_missing"
            rows.append(row)
            print(f"MISS {pid}", flush=True)
            continue
        work = runs / pid
        work.mkdir(parents=True, exist_ok=True)
        try:
            res = resolve_article_body(
                ArticleBodyRequest(
                    source=str(pdf),
                    work_dir=work,
                    preference="hybrid",
                    paper_id=pid,
                    allow_network=False,
                ),
                grobid=ports.grobid,
                opendataloader=ports.opendataloader,
                hybrid_pdf_path=pdf,
            )
            body_dir = work / "body"
            layout = body_dir / f"{pid}.odl.layout.json"
            tei = body_dir / f"{pid}.grobid.tei.xml"
            canon = body_dir / f"{pid}.canonical.json"
            body = body_dir / f"{pid}.hybrid.body.md"
            row.update(
                {
                    "ok": res.route == "hybrid" and body.is_file(),
                    "route": res.route,
                    "body_chars": res.body_chars,
                    "layout_json": layout.is_file(),
                    "layout_bytes": layout.stat().st_size if layout.is_file() else 0,
                    "tei": tei.is_file(),
                    "canonical": canon.is_file(),
                    "body_md": body.is_file(),
                    "work_dir": str(work),
                    "diagnostics_sample": [
                        d
                        for d in res.diagnostics
                        if any(
                            x in d
                            for x in (
                                "odl_bbox",
                                "odl_layout",
                                "canonical_grounded",
                                "bbox_source",
                                "tei_sha",
                            )
                        )
                    ][:12],
                }
            )
            print(
                f"{'OK' if row['ok'] else 'FAIL'} {pid} route={res.route} "
                f"layout={row['layout_json']} tei={row['tei']} chars={res.body_chars}",
                flush=True,
            )
        except Exception as exc:  # noqa: BLE001 - batch continue
            row["error"] = f"{type(exc).__name__}:{exc}"
            row["traceback"] = traceback.format_exc(limit=4)
            print(f"ERR {pid} {row['error']}", flush=True)
        rows.append(row)

    layout_n = sum(1 for r in rows if r.get("layout_json"))
    tei_n = sum(1 for r in rows if r.get("tei"))
    ok_n = sum(1 for r in rows if r.get("ok"))
    report = {
        "schema_version": "m283-gold-hybrid-layout-batch.v1",
        "paper_count": len(paper_ids),
        "ok_count": ok_n,
        "layout_json_count": layout_n,
        "tei_count": tei_n,
        "duration_s": round(time.perf_counter() - t0, 2),
        "live_ports": ports.to_dict(),
        "rows": rows,
        "import_eligible": False,
        "graph_writes_allowed": False,
        "demo_metric": False,
        "metric_mode": "live_hybrid_layout_batch",
        "note": (
            "Live hybrid re-run for gold PDFs with ODL json+markdown. "
            "Never import. Use with layout upgrade + resolvability recompute."
        ),
    }
    report_path = out_root / "batch-report.json"
    report_path.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    line = (
        "m283-gold-hybrid-layout-batch | "
        f"papers: {len(paper_ids)} | ok: {ok_n} | layout: {layout_n} | "
        f"tei: {tei_n} | duration_s: {report['duration_s']} | "
        "import_eligible: false\n"
    )
    if args.json:
        sys.stdout.write(json.dumps(report, indent=2, sort_keys=True) + "\n")
    else:
        sys.stdout.write(line)
        sys.stdout.write(f"  report: {report_path}\n")

    # Governor: after hybrid batch, recompute REAL gold↔hybrid resolvability.
    try:
        from research_graph.application.corpus.canary_gold_hybrid_join import (
            evaluate_joined_canary_resolvability,
        )

        join_pkg = evaluate_joined_canary_resolvability(
            gold_rows=gold_rows,
            body_roots=[runs],
            target_rate=0.95,
        )
        join_path = out_root / "post-batch-resolvability.json"
        join_payload = join_pkg.to_dict()
        join_payload["metric_mode"] = "real_gold_hybrid_join"
        join_payload["demo_metric"] = False
        join_payload["import_eligible"] = False
        join_path.write_text(
            json.dumps(join_payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        rate = (join_payload.get("resolvability") or {}).get("resolvability_rate")
        sys.stdout.write(
            f"post-batch-resolvability | mode: real_gold_hybrid_join | "
            f"joined: {join_payload.get('joined_count')} | rate: {rate} | "
            f"report: {join_path}\n"
        )
    except Exception as exc:  # noqa: BLE001 - non-fatal operator follow-up
        sys.stdout.write(f"post-batch-resolvability | skipped: {type(exc).__name__}:{exc}\n")

    return 0 if ok_n == len(paper_ids) else 1


if __name__ == "__main__":
    raise SystemExit(main())
