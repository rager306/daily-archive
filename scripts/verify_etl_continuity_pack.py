#!/usr/bin/env python3
"""Wave A operator: ETL continuity pack dashboard.

Composes body coverage + multi-root metrics + hybrid-missing PDF readiness +
Wave A closeout into one report. Never import. Never starts hybrid batch.

Usage::

    uv run python scripts/verify_etl_continuity_pack.py
    uv run python scripts/verify_etl_continuity_pack.py --json
    uv run python scripts/verify_etl_continuity_pack.py \\
        --output artifacts/etl/continuity-pack.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.composition_import_hold_inventory import (
    default_import_hold_roots,
    inventory_import_hold_trees,
)
from research_graph.application.corpus.etl_continuity_pack import build_etl_continuity_pack
from research_graph.application.corpus.etl_continuity_readiness import (
    build_continuity_readiness,
)
from research_graph.application.corpus.etl_hybrid_missing_pdf_readiness import (
    audit_hybrid_missing_pdf_readiness,
)
from research_graph.application.corpus.wave_a_closeout import evaluate_wave_a_closeout
from research_graph.workflows.composition.etl_body_coverage import (
    DEFAULT_BODY_ROOTS,
    DEFAULT_CATALOG_INDEX,
    DEFAULT_CATALOG_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave A continuity pack dashboard: coverage + multi_root + PDF readiness "
            "+ closeout. Import always false."
        )
    )
    parser.add_argument("--catalog-index", type=Path, default=DEFAULT_CATALOG_INDEX)
    parser.add_argument("--catalog-root", type=Path, default=DEFAULT_CATALOG_ROOT)
    parser.add_argument("--body-root", action="append", type=Path, default=None)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _r(p: Path) -> Path:
        return p if p.is_absolute() else (repo / p)

    catalog_index = _r(args.catalog_index)
    catalog_root = _r(args.catalog_root)
    if args.body_root:
        body_roots = tuple(_r(p) for p in args.body_root)
    else:
        body_roots = tuple(_r(Path(p)) for p in DEFAULT_BODY_ROOTS)

    continuity = build_continuity_readiness(
        catalog_index_path=catalog_index,
        body_roots=body_roots,
        catalog_root=catalog_root,
    )
    coverage = continuity.coverage
    preprocess = continuity.preprocess
    pdf = audit_hybrid_missing_pdf_readiness(
        catalog_index_path=catalog_index,
        catalog_root=catalog_root,
        body_roots=body_roots,
    )
    hold = inventory_import_hold_trees(default_import_hold_roots())
    closeout = evaluate_wave_a_closeout(
        hybrid_found=coverage.hybrid_body_found,
        readiness_signal=continuity.readiness_signal,
        import_hold_hits=int(hold.get("enablement_hit_count") or 0),
        preprocess_errors=int(preprocess.error_count),
        preprocess_body_count=int(preprocess.body_count),
        article_count=int(coverage.article_count),
    )
    pack = build_etl_continuity_pack(
        coverage=coverage,
        pdf_readiness=pdf,
        closeout=closeout,
    )
    payload = pack.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["continuity_readiness_signal"] = continuity.readiness_signal
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = _r(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
    if args.json:
        sys.stdout.write(text)
    else:
        d = pack.dashboard
        alerts = list(pack.alerts)
        sys.stdout.write(
            "etl-continuity-pack | "
            f"hybrid_found: {d.get('hybrid_found')} | "
            f"hybrid_fraction: {d.get('hybrid_fraction')} | "
            f"expand_ready_frac: {d.get('expand_ready_frac')} | "
            f"multi_root_divergent: {d.get('multi_root_divergent_content_count')} | "
            f"closeout: {d.get('closeout_signal')} | "
            f"alerts: {len(alerts)} | "
            "import_eligible: false\n"
        )
        if alerts:
            sys.stdout.write("  alerts: " + ", ".join(alerts) + "\n")
        sys.stdout.write(
            "  multi_root: "
            f"ids={d.get('multi_root_paper_id_count')} "
            f"identical={d.get('multi_root_identical_content_count')} "
            f"divergent={d.get('multi_root_divergent_content_count')}\n"
        )
        sys.stdout.write(
            "  pdf_queue: "
            f"missing_with_pdf={d.get('missing_with_local_pdf_count')} "
            f"missing_without_pdf={d.get('missing_without_local_pdf_count')}\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
