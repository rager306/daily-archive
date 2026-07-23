#!/usr/bin/env python3
"""Wave B gate operator (M251 / D123).

Reports whether Wave B extraction-quality work is authorized.
Default: blocked (human_go=false). Optional --human-go is a dry-run
simulation for the current process only — it does not persist authorization.

Never authorizes import or graph writes.

Usage::

    uv run python scripts/verify_wave_b_gate.py
    uv run python scripts/verify_wave_b_gate.py --json
    uv run python scripts/verify_wave_b_gate.py --human-go   # dry-run open
    uv run python scripts/verify_wave_b_gate.py --with-closeout
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from research_graph.application.corpus.wave_b_gate import evaluate_wave_b_gate

ROOT = Path(__file__).resolve().parents[1]


def _maybe_closeout_context(repo: Path) -> tuple[bool | None, str | None]:
    """Best-effort Wave A closeout context (never authorizes Wave B)."""
    try:
        from research_graph.application.corpus.composition_import_hold_inventory import (
            default_import_hold_roots,
            inventory_import_hold_trees,
        )
        from research_graph.application.corpus.etl_continuity_readiness import (
            build_continuity_readiness,
        )
        from research_graph.application.corpus.wave_a_closeout import (
            evaluate_wave_a_closeout,
        )
        from research_graph.workflows.composition.etl_body_coverage import (
            DEFAULT_BODY_ROOTS,
            DEFAULT_CATALOG_INDEX,
            DEFAULT_CATALOG_ROOT,
        )
    except Exception:  # noqa: BLE001 - gate must still report without context
        return None, None

    def _resolve(p: Path) -> Path:
        return p if p.is_absolute() else (repo / p).resolve()

    try:
        continuity = build_continuity_readiness(
            catalog_index_path=_resolve(DEFAULT_CATALOG_INDEX),
            catalog_root=_resolve(DEFAULT_CATALOG_ROOT),
            body_roots=tuple(_resolve(r) for r in DEFAULT_BODY_ROOTS),
            sample_limit=8,
        )
        hold = inventory_import_hold_trees(default_import_hold_roots())
        hits = int(hold.get("enablement_hit_count") or 0)
        closeout = evaluate_wave_a_closeout(
            hybrid_found=continuity.coverage.hybrid_body_found,
            readiness_signal=continuity.readiness_signal,
            import_hold_hits=hits,
            preprocess_errors=continuity.preprocess.error_count,
            preprocess_body_count=continuity.preprocess.body_count,
            article_count=continuity.coverage.article_count,
        )
        return closeout.closeout_pass, closeout.closeout_signal
    except Exception:  # noqa: BLE001
        return None, None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Wave B gate status. Default blocked. "
            "--human-go is dry-run only (not persisted). Import always false."
        )
    )
    parser.add_argument(
        "--human-go",
        action="store_true",
        help="Dry-run: simulate human go for this invocation only (not persisted)",
    )
    parser.add_argument(
        "--with-closeout",
        action="store_true",
        help="Compose live Wave A closeout as context (never authorizes B)",
    )
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)

    repo = Path(args.repo_root)
    closeout_pass: bool | None = None
    closeout_signal: str | None = None
    if args.with_closeout:
        closeout_pass, closeout_signal = _maybe_closeout_context(repo)

    package = evaluate_wave_b_gate(
        human_go=bool(args.human_go),
        wave_a_closeout_pass=closeout_pass,
        wave_a_closeout_signal=closeout_signal,
    )
    payload = package.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["human_go_persisted"] = False
    payload["human_go_is_dry_run"] = bool(args.human_go)
    payload["note_operator"] = (
        "Authorization is not persisted by this script. "
        "Record human go in PROJECT/decision before Wave B milestones."
    )

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        sys.stdout.write(
            "wave-b-gate | "
            f"signal: {package.gate_signal} | "
            f"open: {str(package.wave_b_gate_open).lower()} | "
            f"human_go: {str(package.human_go).lower()} "
            f"({'dry-run' if args.human_go else 'default'}) | "
            f"closeout_pass: {package.wave_a_closeout_pass} | "
            f"closeout_signal: {package.wave_a_closeout_signal} | "
            "import_eligible: false\n"
        )
        if not package.wave_b_gate_open:
            sys.stdout.write(
                "  note: Wave B blocked until explicit human go "
                "(D123); closeout_pass alone insufficient\n"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
