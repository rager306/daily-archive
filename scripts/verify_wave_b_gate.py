#!/usr/bin/env python3
"""Wave B gate operator (M251 / D123 / M254 stamp-aware).

Reports whether Wave B extraction-quality work is authorized.

Modes:
  * Default: read durable stamp at artifacts/wave-b/human_go.json
    (or --stamp PATH). Opens only when stamp.human_go is true (D124).
  * --no-stamp: ignore stamp; blocked unless --human-go dry-run.
  * --human-go: dry-run open for this process only (not persisted),
    used with --no-stamp or when stamp is missing.

Never authorizes import or graph writes.

Usage::

    uv run python scripts/verify_wave_b_gate.py
    uv run python scripts/verify_wave_b_gate.py --json
    uv run python scripts/verify_wave_b_gate.py --no-stamp
    uv run python scripts/verify_wave_b_gate.py --no-stamp --human-go
    uv run python scripts/verify_wave_b_gate.py --stamp path/to/human_go.json
    uv run python scripts/verify_wave_b_gate.py --with-closeout
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
            "Wave B gate status. Default reads durable human_go stamp. "
            "--no-stamp ignores stamp; --human-go is dry-run only. "
            "Import always false."
        )
    )
    parser.add_argument(
        "--human-go",
        action="store_true",
        help="Dry-run: simulate human go for this invocation only (not persisted)",
    )
    parser.add_argument(
        "--stamp",
        type=Path,
        default=None,
        help=(
            "Path to durable human_go stamp JSON "
            f"(default: {DEFAULT_HUMAN_GO_STAMP})"
        ),
    )
    parser.add_argument(
        "--no-stamp",
        action="store_true",
        help="Ignore durable stamp; blocked unless --human-go dry-run",
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

    stamp_path: Path | None = None
    stamp_present = False
    human_go_source = "none"
    human_go_persisted = False
    human_go_is_dry_run = False

    if args.no_stamp:
        package = evaluate_wave_b_gate(
            human_go=bool(args.human_go),
            wave_a_closeout_pass=closeout_pass,
            wave_a_closeout_signal=closeout_signal,
        )
        if args.human_go:
            human_go_source = "flag"
            human_go_is_dry_run = True
        stamp_path = None
        stamp_present = False
    else:
        raw_stamp = args.stamp if args.stamp is not None else DEFAULT_HUMAN_GO_STAMP
        stamp_path = (
            raw_stamp if raw_stamp.is_absolute() else (repo / raw_stamp).resolve()
        )
        package = evaluate_wave_b_gate_from_stamp(
            stamp_path,
            wave_a_closeout_pass=closeout_pass,
            wave_a_closeout_signal=closeout_signal,
        )
        stamp_present = stamp_path.is_file()
        if package.human_go and stamp_present:
            human_go_source = "stamp"
            human_go_persisted = True
        elif args.human_go and not package.human_go:
            # stamp missing/invalid: allow dry-run flag as override for ops
            package = evaluate_wave_b_gate(
                human_go=True,
                wave_a_closeout_pass=closeout_pass,
                wave_a_closeout_signal=closeout_signal,
            )
            human_go_source = "flag"
            human_go_is_dry_run = True
            human_go_persisted = False

    payload = package.to_dict()
    payload["import_eligible"] = False
    payload["graph_writes_allowed"] = False
    payload["human_go_persisted"] = human_go_persisted
    payload["human_go_is_dry_run"] = human_go_is_dry_run
    payload["human_go_source"] = human_go_source
    payload["stamp_path"] = str(stamp_path) if stamp_path is not None else None
    payload["stamp_present"] = stamp_present
    payload["note_operator"] = (
        "Default reads durable stamp (D124). "
        "--no-stamp ignores stamp; --human-go is dry-run only. "
        "Stamp never authorizes import."
    )

    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        out = args.output if args.output.is_absolute() else (repo / args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")

    if args.json:
        sys.stdout.write(text)
    else:
        source_note = human_go_source
        if human_go_is_dry_run:
            source_note = f"{human_go_source}/dry-run"
        sys.stdout.write(
            "wave-b-gate | "
            f"signal: {package.gate_signal} | "
            f"open: {str(package.wave_b_gate_open).lower()} | "
            f"human_go: {str(package.human_go).lower()} "
            f"(source={source_note}) | "
            f"stamp_present: {str(stamp_present).lower()} | "
            f"closeout_pass: {package.wave_a_closeout_pass} | "
            f"closeout_signal: {package.wave_a_closeout_signal} | "
            "import_eligible: false\n"
        )
        if not package.wave_b_gate_open:
            sys.stdout.write(
                "  note: Wave B blocked until durable human_go stamp "
                "(D124) or --human-go dry-run; closeout_pass alone "
                "insufficient; import never authorized\n"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
