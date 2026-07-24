#!/usr/bin/env python3
"""Wave C structure readiness operator (M262).

Bridges M209 pipeline continuity structure layer with ETL continuity pack
hybrid/closeout context. Optional citation review if batch root provided.
Never import.

Usage::

    uv run python scripts/verify_structure_readiness_package.py
    uv run python scripts/verify_structure_readiness_package.py --json
    uv run python scripts/verify_structure_readiness_package.py \\
        --output artifacts/etl/structure-readiness.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from research_graph.application.corpus.etl_continuity_pack import (
    compose_live_continuity_pack,
)
from research_graph.application.corpus.structure_readiness_package import (
    build_structure_readiness_package,
    extract_structure_layer,
)
from research_graph.application.pipeline_continuity import build_continuity_audit

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path("artifacts/etl/structure-readiness.json")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave C structure readiness: M209 structure layer + ETL pack context. "
            "Import always false."
        )
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--citation-batch-root",
        type=Path,
        default=None,
        help="Optional hybrid batch root for citation review policy (skip if absent)",
    )
    parser.add_argument(
        "--skip-etl-pack",
        action="store_true",
        help="Skip live continuity pack (structure layer only)",
    )
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)

    def _r(p: Path) -> Path:
        return p if p.is_absolute() else (repo / p).resolve()

    audit = build_continuity_audit(repo_root=repo)
    structure_layer = extract_structure_layer(audit)

    hybrid_found = None
    hybrid_fraction = None
    closeout_signal = None
    dashboard: dict[str, Any] = {}
    if not args.skip_etl_pack:
        try:
            pack = compose_live_continuity_pack(repo_root=repo)
            dashboard = dict(pack.dashboard)
            hybrid_found = int(dashboard.get("hybrid_found") or 0)
            hybrid_fraction = float(dashboard.get("hybrid_fraction") or 0.0)
            closeout_signal = str(dashboard.get("closeout_signal") or "")
        except Exception as exc:  # noqa: BLE001
            dashboard = {"etl_pack_error": f"{type(exc).__name__}:{exc}"}

    citation_verdict = None
    if args.citation_batch_root is not None:
        body_root = _r(args.citation_batch_root)
        if body_root.is_dir():
            try:
                from research_graph.workflows.composition.citation_review_policy import (
                    CitationReviewPolicyRequest,
                    run_citation_review_policy,
                )

                result = run_citation_review_policy(
                    CitationReviewPolicyRequest(
                        body_root=body_root,
                        repo_root=repo,
                    )
                )
                policy = result.policy
                citation_verdict = str(getattr(policy, "verdict", None) or "") or None
            except Exception as exc:  # noqa: BLE001
                citation_verdict = f"error:{type(exc).__name__}"

    pkg = build_structure_readiness_package(
        structure_layer=structure_layer,
        pipeline_overall=str(audit.overall),
        hybrid_found=hybrid_found,
        hybrid_fraction=hybrid_fraction,
        closeout_signal=closeout_signal,
        citation_verdict=citation_verdict,
        etl_dashboard=dashboard,
    )
    payload = pkg.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["falkor_touched"] = False
    payload["pipeline_continuity_overall"] = audit.overall
    payload["etl_dashboard_subset"] = {
        k: dashboard.get(k)
        for k in (
            "hybrid_found",
            "hybrid_fraction",
            "closeout_signal",
            "closeout_pass",
            "multi_root_divergent_content_count",
            "expand_ready_frac",
        )
        if k in dashboard
    }

    out = _r(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "structure-readiness | "
            f"signal: {pkg.structure_signal} | "
            f"structure_health: {pkg.structure_layer_health} | "
            f"seams: {len(pkg.structure_present_seams)}/"
            f"{len(pkg.structure_present_seams) + len(pkg.structure_missing_seams)} | "
            f"hybrid_found: {pkg.hybrid_found} | "
            f"closeout: {pkg.closeout_signal} | "
            f"citation: {pkg.citation_verdict} | "
            f"alerts: {len(pkg.alerts)} | "
            "import_eligible: false\n"
        )
        if pkg.alerts:
            sys.stdout.write("  alerts: " + ", ".join(pkg.alerts) + "\n")
        if pkg.structure_missing_seams:
            sys.stdout.write(
                "  missing: " + ", ".join(pkg.structure_missing_seams[:5]) + "\n"
            )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
