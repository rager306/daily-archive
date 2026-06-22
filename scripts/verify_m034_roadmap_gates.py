#!/usr/bin/env python3
"""Verify M034 roadmap gates and conflict-resolution artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

GATES = [
    "Universal KB Scope Gate",
    "GraphDB Evaluation Gate",
    "State Model Gate",
    "Queue Semantics Gate",
    "Artifact Dependency Graph Gate",
    "Failure Taxonomy Gate",
    "Sidecar Lifecycle Gate",
    "Review Boundary Gate",
    "Graph-readiness Handoff Gate",
    "Agent Boundary Gate",
]
SAFETY = [
    "graph_import_allowed=false",
    "graphdb_written=false",
    "ladybugdb_written=false",
    "production_import_attempted=false",
    "import_eligible=false",
]
FILES = [
    "ROADMAP-GATES.md",
    "NEXT-MILESTONE-HANDOFF.md",
    "CONFLICT-RESOLUTION-PLAN.md",
    "OPEN-QUESTIONS.md",
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-dir", required=True, type=Path)
    args = parser.parse_args()
    pkg: Path = args.package_dir
    failures: list[str] = []
    texts: dict[str, str] = {}
    for name in FILES:
        path = pkg / name
        if not path.exists():
            failures.append(f"missing file: {path}")
            texts[name] = ""
        else:
            texts[name] = path.read_text(encoding="utf-8")

    roadmap = texts["ROADMAP-GATES.md"]
    for gate in GATES:
        if gate not in roadmap:
            failures.append(f"ROADMAP-GATES.md missing {gate}")
    for marker in ["Question", "Options", "Decision Criteria", "Required Artifact Before Coding"]:
        if marker not in roadmap:
            failures.append(f"ROADMAP-GATES.md missing table marker {marker}")

    handoff = texts["NEXT-MILESTONE-HANDOFF.md"]
    for marker in ["Recommended Prototype Slices", "Must Not Implement Yet", "Ready Inputs"]:
        if marker not in handoff:
            failures.append(f"NEXT-MILESTONE-HANDOFF.md missing {marker}")
    for marker in SAFETY:
        if marker not in handoff:
            failures.append(f"NEXT-MILESTONE-HANDOFF.md missing safety marker {marker}")

    routes_path = pkg / "correction-routes.json"
    if routes_path.exists():
        routes = json.loads(routes_path.read_text(encoding="utf-8")).get("routes", [])
        conflict_plan = texts["CONFLICT-RESOLUTION-PLAN.md"]
        missing_routes = [r.get("id") for r in routes if r.get("id") not in conflict_plan]
        if missing_routes:
            failures.append(f"conflict plan missing route IDs: {missing_routes}")
        if len(routes) != 15:
            failures.append(f"expected 15 correction routes, got {len(routes)}")
    else:
        failures.append(f"missing correction routes file: {routes_path}")

    open_questions = texts["OPEN-QUESTIONS.md"]
    for marker in [
        "Which GraphDB",
        "Should durable state",
        "When may LLM/agent helpers enter",
        "Open questions are not accepted decisions",
    ]:
        if marker not in open_questions:
            failures.append(f"OPEN-QUESTIONS.md missing {marker}")

    if failures:
        sys.stderr.write("M034 roadmap gates verification failed:\n")
        for failure in failures:
            sys.stderr.write(f"- {failure}\n")
        return 1

    sys.stdout.write("M034 roadmap gates verification passed\n")
    sys.stdout.write(f"gates={len(GATES)} routes=15 files={len(FILES)}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
