#!/usr/bin/env python3
"""Verify import-hold inventory over package roots (M239).

Scans default package roots (domain / application / composition / infrastructure)
for Python True enablements of ``import_eligible`` or ``graph_writes_allowed``.

Exit codes:
  0 — enablement_hit_count == 0 (import hold clean)
  1 — one or more True enablements found (fail-closed)

Never authorizes import. Report always has import_eligible=false.

Usage::

    uv run python scripts/verify_import_hold_inventory.py
    uv run python scripts/verify_import_hold_inventory.py --json
    uv run python scripts/verify_import_hold_inventory.py --root path/to/tree --root path/to/other
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


def build_report(roots: list[Path]) -> dict:
    """Run inventory and attach operator verdict fields (still import-blocked)."""
    report = inventory_import_hold_trees(roots)
    # Explicit operator fields; never flip import flags.
    report = {
        **report,
        "verdict": "pass" if report["enablement_hit_count"] == 0 else "fail",
        "import_eligible": False,
        "graph_writes_allowed": False,
    }
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Fail-closed import-hold inventory: exit 1 if any Python "
            "import_eligible/graph_writes_allowed = True enablement is found."
        )
    )
    parser.add_argument(
        "--root",
        action="append",
        type=Path,
        default=None,
        help=(
            "Optional tree root to scan (repeatable). "
            "Default: package default_import_hold_roots()."
        ),
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print full JSON report to stdout.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Optional path to write JSON report.",
    )
    args = parser.parse_args(argv)

    roots = list(args.root) if args.root else default_import_hold_roots()
    report = build_report(roots)

    text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")

    if args.json or args.output is None:
        # Always print a short summary; full JSON when --json.
        if args.json:
            sys.stdout.write(text)
        else:
            sys.stdout.write(
                "import-hold inventory | "
                f"verdict: {report['verdict']} | "
                f"trees: {report['tree_count']} | "
                f"scanned: {report['scanned_file_count']} | "
                f"modules: {report['module_count']} | "
                f"enablement_hits: {report['enablement_hit_count']} | "
                "import_eligible: false\n"
            )
            if report["enablement_hits"]:
                for hit in report["enablement_hits"][:20]:
                    sys.stdout.write(f"  hit: {hit}\n")

    return 0 if report["enablement_hit_count"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
