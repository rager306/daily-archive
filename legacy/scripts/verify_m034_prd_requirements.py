#!/usr/bin/env python3
"""Verify M034 PRD and requirements artifacts."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

SAFETY_MARKERS = [
    "graph_import_allowed=false",
    "graphdb_written=false",
    "ladybugdb_written=false",
    "production_import_attempted=false",
    "import_eligible=false",
]
ADR_MARKERS = ["ADR-000", "ADR-002", "ADR-003", "ADR-004", "ADR-005", "ADR-006", "ADR-007"]
PRD_MARKERS = [
    "## Product Summary",
    "## Goals",
    "## Non-goals",
    "## Users and Workflows",
    "## Generic vs Paper-specific Scope",
    "## Acceptance Criteria",
]
FUNCTIONAL_MARKERS = [
    "## Generic Universal-KB Requirements",
    "## Scientific-paper First-domain Requirements",
    "## Safety Requirements",
    "FR-001",
    "PFR-001",
    "SFR-004",
]
NFR_MARKERS = ["NFR-001", "NFR-010", "GraphDB portability", "Resumability", "Observability"]


def read(path: Path, failures: list[str]) -> str:
    if not path.exists():
        failures.append(f"missing file: {path}")
        return ""
    return path.read_text(encoding="utf-8")


def require_all(text: str, markers: list[str], label: str, failures: list[str]) -> None:
    for marker in markers:
        if marker not in text:
            failures.append(f"{label} missing marker: {marker}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-dir", required=True, type=Path)
    args = parser.parse_args()
    pkg: Path = args.package_dir
    failures: list[str] = []

    prd = read(pkg / "PRD.md", failures)
    fr = read(pkg / "FUNCTIONAL-REQUIREMENTS.md", failures)
    nfr = read(pkg / "NON-FUNCTIONAL-REQUIREMENTS.md", failures)

    require_all(prd, PRD_MARKERS, "PRD", failures)
    require_all(prd, ADR_MARKERS, "PRD ADR references", failures)
    require_all(prd + fr + nfr, SAFETY_MARKERS, "PRD/requirements safety", failures)
    require_all(fr, FUNCTIONAL_MARKERS, "FUNCTIONAL-REQUIREMENTS", failures)
    require_all(nfr, NFR_MARKERS, "NON-FUNCTIONAL-REQUIREMENTS", failures)

    fr_ids = set(re.findall(r"\| (FR-\d{3}|PFR-\d{3}|SFR-\d{3}) \|", fr))
    nfr_ids = set(re.findall(r"\| (NFR-\d{3}) \|", nfr))
    if len(fr_ids) < 15:
        failures.append(
            f"expected at least 15 functional/safety requirement IDs, got {len(fr_ids)}"
        )
    if len(nfr_ids) < 10:
        failures.append(f"expected at least 10 non-functional requirement IDs, got {len(nfr_ids)}")
    if "Acceptance Criteria" not in fr or "Acceptance Criteria" not in nfr:
        failures.append("requirements docs must include acceptance criteria columns/summary")

    if failures:
        sys.stderr.write("M034 PRD/requirements verification failed:\n")
        for failure in failures:
            sys.stderr.write(f"- {failure}\n")
        return 1

    sys.stdout.write("M034 PRD/requirements verification passed\n")
    sys.stdout.write(f"functional_ids={len(fr_ids)} non_functional_ids={len(nfr_ids)}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
