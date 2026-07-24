#!/usr/bin/env python3
"""Wave A data-readiness closeout operator (M250).

Composes continuity readiness + import-hold inventory into evaluate_wave_a_closeout.
Never authorizes import or auto-opens Wave B.

Exit 0 after report generation. Use --strict to exit 1 when closeout_pass is false.

Usage::

    uv run python scripts/verify_wave_a_closeout.py
    uv run python scripts/verify_wave_a_closeout.py --json
    uv run python scripts/verify_wave_a_closeout.py --strict
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
from research_graph.application.corpus.etl_continuity_readiness import (
    build_continuity_readiness,
)
from research_graph.application.corpus.wave_a_closeout import (
    DEFAULT_MIN_HYBRID_FOUND,
    evaluate_wave_a_closeout,
)
from research_graph.workflows.composition.etl_body_coverage import (
    DEFAULT_BODY_ROOTS,
    DEFAULT_CATALOG_INDEX,
    DEFAULT_CATALOG_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave A closeout: hybrid_found threshold + continuity + import-hold. "
            "Import always false. Wave B never auto-open. Exit 0 after report."
        )
    )
    parser.add_argument("--catalog-index", type=Path, default=DEFAULT_CATALOG_INDEX)
    parser.add_argument("--catalog-root", type=Path, default=DEFAULT_CATALOG_ROOT)
    parser.add_argument(
        "--body-root",
        action="append",
        type=Path,
        default=None,
        help="Hybrid body root (repeatable)",
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument(
        "--min-hybrid-found",
        type=int,
        default=DEFAULT_MIN_HYBRID_FOUND,
        help="Minimum hybrid_found for wave_a_closed",
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional path to write full closeout JSON",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit 1 when closeout_pass is false",
    )
    parser.add_argument("--sample-limit", type=int, default=12)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _resolve(p: Path) -> Path:
        path = Path(p)
        return path if path.is_absolute() else (repo / path).resolve()

    catalog_index = _resolve(args.catalog_index)
    catalog_root = _resolve(args.catalog_root)
    raw_roots = tuple(args.body_root) if args.body_root else DEFAULT_BODY_ROOTS
    body_roots = tuple(_resolve(r) for r in raw_roots)

    continuity = build_continuity_readiness(
        catalog_index_path=catalog_index,
        catalog_root=catalog_root,
        body_roots=body_roots,
        sample_limit=args.sample_limit,
    )
    hold = inventory_import_hold_trees(default_import_hold_roots())
    enablement_hits = int(hold.get("enablement_hit_count") or 0)

    closeout = evaluate_wave_a_closeout(
        hybrid_found=continuity.coverage.hybrid_body_found,
        readiness_signal=continuity.readiness_signal,
        import_hold_hits=enablement_hits,
        preprocess_errors=continuity.preprocess.error_count,
        preprocess_body_count=continuity.preprocess.body_count,
        article_count=continuity.coverage.article_count,
        min_hybrid_found=args.min_hybrid_found,
        hybrid_fraction=float(continuity.coverage.hybrid_body_fraction),
    )

    payload = closeout.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["wave_b_gate_open"] = False
    payload["continuity_readiness_signal"] = continuity.readiness_signal
    payload["hybrid_fraction"] = continuity.coverage.hybrid_body_fraction
    payload["quality_status_counts"] = dict(
        continuity.preprocess.quality_status_counts
    )
    payload["import_hold_verdict"] = (
        "pass" if enablement_hits == 0 else "fail"
    )

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = _resolve(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        q = (
            ",".join(
                f"{k}={v}"
                for k, v in continuity.preprocess.quality_status_counts.items()
            )
            or "none"
        )
        sys.stdout.write(
            "wave-a-closeout | "
            f"signal: {closeout.closeout_signal} | "
            f"pass: {str(closeout.closeout_pass).lower()} | "
            f"hybrid_found: {closeout.hybrid_found} "
            f"(min {closeout.min_hybrid_found}) | "
            f"readiness: {closeout.readiness_signal} | "
            f"preprocess_bodies: {closeout.preprocess_body_count} | "
            f"preprocess_errors: {closeout.preprocess_errors} | "
            f"quality: {q} | "
            f"import_hold_hits: {closeout.import_hold_hits} | "
            "wave_b_gate_open: false | "
            "import_eligible: false\n"
        )
        if not closeout.closeout_pass:
            blocks = [d for d in closeout.diagnostics if d.startswith("block:")]
            if blocks:
                sys.stdout.write("  blocks: " + ", ".join(blocks) + "\n")
        sys.stdout.write("  runbook:\n")
        for cmd in closeout.operator_commands:
            sys.stdout.write(f"    {cmd}\n")

    if args.strict and not closeout.closeout_pass:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
