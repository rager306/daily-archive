#!/usr/bin/env python3
"""Generate the M122 recurring pipeline script inventory.

This script is a thin CLI wrapper around application-owned inventory builder logic.
"""

from __future__ import annotations

import argparse
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = REPO_ROOT / "src"
if str(SRC_ROOT) not in sys.path:
    sys.path.insert(0, str(SRC_ROOT))

from research_graph.application.pipeline_script_audit_inventory import (  # noqa: E402
    build_inventory,
    write_inventory,
)
from research_graph.application.pipeline_script_inventory import (  # noqa: E402
    ScriptInventory,
    ValidationIssue,
)

__all__ = ["build_inventory", "print_summary", "write_inventory"]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--write",
        type=Path,
        default=REPO_ROOT / "data" / "pipeline-script-architecture" / "script-inventory.json",
    )
    return parser.parse_args()


def print_summary(inventory: ScriptInventory, issues: list[ValidationIssue]) -> None:
    category_counts = Counter(item.category.value for item in inventory.items)
    classification_counts = Counter(item.classification.value for item in inventory.items)
    print(f"script_count={len(inventory.items)}")
    for category in sorted(category_counts):
        print(f"{category}={category_counts[category]}")
    for classification in sorted(classification_counts):
        print(f"classification.{classification}={classification_counts[classification]}")
    print(f"validation_issues={len(issues)}")
    for issue in issues:
        suffix = f" path={issue.path}" if issue.path else ""
        print(f"issue script_id={issue.script_id} field={issue.field} message={issue.message}{suffix}")


def main() -> int:
    args = parse_args()
    inventory = build_inventory(args.repo_root)
    issues = write_inventory(inventory, args.write)
    print_summary(inventory, issues)
    return 1 if issues else 0


if __name__ == "__main__":
    raise SystemExit(main())
