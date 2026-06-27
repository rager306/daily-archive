#!/usr/bin/env python3
"""Inventory pytest files against the project test-layer taxonomy.

This script is a thin CLI wrapper around application-owned inventory logic.
"""

from __future__ import annotations

import argparse
import json

from research_graph.application.test_architecture_inventory import (
    BUCKET_ORDER,
    DEFAULT_OUTPUT_DIR,
    DEFAULT_TESTS_DIR,
    RECENT_PILOT_PREFIXES,
    SCHEMA_VERSION,
    TestFileAnalysis,
    analyze_test_file,
    build_inventory,
    choose_pilot_candidates,
    classify,
    render_markdown,
    suggested_layer_for,
    write_outputs,
)

__all__ = [
    "BUCKET_ORDER",
    "DEFAULT_OUTPUT_DIR",
    "DEFAULT_TESTS_DIR",
    "RECENT_PILOT_PREFIXES",
    "SCHEMA_VERSION",
    "TestFileAnalysis",
    "analyze_test_file",
    "build_inventory",
    "choose_pilot_candidates",
    "classify",
    "render_markdown",
    "suggested_layer_for",
    "write_outputs",
]


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audit pytest files by architecture layer.")
    parser.add_argument("--tests-dir", type=DEFAULT_TESTS_DIR.__class__, default=DEFAULT_TESTS_DIR)
    parser.add_argument("--output-dir", type=DEFAULT_OUTPUT_DIR.__class__, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--json", action="store_true", help="Print the inventory JSON to stdout.")
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    inventory = build_inventory(args.tests_dir)
    json_path, markdown_path, pilot_path = write_outputs(inventory, args.output_dir)
    if args.json:
        print(json.dumps(inventory, indent=2, sort_keys=True))
    else:
        summary = inventory["summary"]
        print(
            " | ".join(
                [
                    "test architecture inventory",
                    f"files: {summary['total_test_files']}",
                    f"json: {json_path}",
                    f"markdown: {markdown_path}",
                    f"pilot: {pilot_path}",
                ]
            )
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
