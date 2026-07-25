#!/usr/bin/env python3
"""Import evidence chain operator (M284 S04).

Assembles evidence_dashboard + prediction resolvability + structure +
import_hold + E5 into one fail-closed chain. Never sets import_eligible.
Never graph write. D127: user_go alone never flips import_eligible.

Usage::

    uv run python scripts/verify_import_evidence_chain.py
    uv run python scripts/verify_import_evidence_chain.py --user-go
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from research_graph.application.corpus.composition_import_hold_inventory import (
    default_import_hold_roots,
    inventory_import_hold_trees,
)
from research_graph.application.corpus.import_evidence_chain import (
    build_import_evidence_chain,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DASH = Path("artifacts/etl/evidence-dashboard.v1.json")
DEFAULT_PRED = Path("artifacts/etl/canary-prediction-resolvability.v1.json")
DEFAULT_STRUCT = Path("artifacts/etl/structure-readiness-m283.json")
DEFAULT_E5 = Path("artifacts/etl/e5-optional-candidates.v1.json")
DEFAULT_OUTPUT = Path("artifacts/etl/import-evidence-chain.v1.json")


def _r(repo: Path, p: Path) -> Path:
    return p if p.is_absolute() else (repo / p).resolve()


def _load(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Import evidence chain (fail-closed)")
    p.add_argument("--repo-root", type=Path, default=ROOT)
    p.add_argument("--evidence-dashboard", type=Path, default=DEFAULT_DASH)
    p.add_argument("--prediction-resolvability", type=Path, default=DEFAULT_PRED)
    p.add_argument("--structure-readiness", type=Path, default=DEFAULT_STRUCT)
    p.add_argument("--e5", type=Path, default=DEFAULT_E5)
    p.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    p.add_argument(
        "--user-go",
        action="store_true",
        help="Record user_go=true (does NOT flip import_eligible; D127)",
    )
    p.add_argument(
        "--prediction-target-rate",
        type=float,
        default=0.70,
        help="Floor for prediction resolvability (default 0.70)",
    )
    p.add_argument("--json", action="store_true")
    args = p.parse_args(argv)
    repo = Path(args.repo_root)

    dash = _load(_r(repo, Path(args.evidence_dashboard)))
    pred = _load(_r(repo, Path(args.prediction_resolvability)))
    struct = _load(_r(repo, Path(args.structure_readiness)))
    e5 = _load(_r(repo, Path(args.e5)))

    try:
        roots = default_import_hold_roots()
        hold_inv = inventory_import_hold_trees(roots)
        hold = dict(hold_inv) if isinstance(hold_inv, dict) else {
            "verdict": "pass",
            "enablement_hits": 0,
            "import_eligible": False,
        }
        hits = hold.get("enablement_hits")
        if hits is None and isinstance(hold.get("hits"), list):
            hits = len(hold["hits"])
        if isinstance(hits, list):
            hits = len(hits)
        hold["enablement_hits"] = int(hits or 0)
        hold["verdict"] = "pass" if int(hold["enablement_hits"]) == 0 else "fail"
        hold["import_eligible"] = False
    except Exception as exc:  # noqa: BLE001
        hold = {
            "verdict": "error",
            "error": f"{type(exc).__name__}:{exc}",
            "enablement_hits": 0,
            "import_eligible": False,
        }

    pkg = build_import_evidence_chain(
        evidence_dashboard=dash or None,
        prediction_resolvability=pred or None,
        structure_readiness=struct or None,
        import_hold=hold,
        e5_optional=e5 or None,
        user_go=bool(args.user_go),
        prediction_target_rate=float(args.prediction_target_rate),
    )
    payload = pkg.to_dict()
    payload["sources"] = {
        "evidence_dashboard": str(args.evidence_dashboard),
        "prediction_resolvability": str(args.prediction_resolvability),
        "structure_readiness": str(args.structure_readiness),
        "e5": str(args.e5),
        "import_hold": "live_inventory",
    }
    payload["graph_write_status"] = "blocked_no_write"
    payload["next_step"] = (
        "If chain_green and you want graph write: explicit separate user yes "
        "is still required; this package never authorizes write (D127)."
        if pkg.chain_green
        else "Resolve blockers before any import discussion."
    )

    out = _r(repo, Path(args.output))
    out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "import-evidence-chain | "
            f"chain_green: {str(pkg.chain_green).lower()} | "
            f"evidence_ready: {str(pkg.evidence_ready_ok).lower()} | "
            f"verification_ready: {str(pkg.verification_ready_ok).lower()} | "
            f"pred_rate: {pkg.prediction_resolvability_rate} | "
            f"page_bbox: {pkg.page_or_bbox_count} | "
            f"user_go: {str(pkg.user_go).lower()} | "
            f"blockers: {len(pkg.blockers)} | "
            "import_eligible: false | graph_write: blocked\n"
        )
        if pkg.blockers:
            sys.stdout.write(
                "  blockers: " + "; ".join(pkg.blockers[:8]) + "\n"
            )
        sys.stdout.write(f"  report: {out}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
