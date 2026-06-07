#!/usr/bin/env python3
"""Verify M034 ADR template, index, and north-star ADR artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REQUIRED_TEMPLATE_MARKERS = [
    "## 0. One-line Decision",
    "## 1. Context",
    "## 2. Decision",
    "## 3. Applies To",
    "## 4. Requirements and Decisions Impacted",
    "## 5. Options Considered",
    "## 6. Trade-off Analysis",
    "## 7. Consequences",
    "## 8. Safety and Non-Authorization",
    "## 9. Contract Impact",
    "## 10. Validation / Evidence Required",
    "## 11. Open Questions",
    "## 12. Follow-up Actions",
    "## 13. Supersedes / Superseded By",
    "## 14. LLM Reading Notes",
    "GraphDB ADR",
    "Queue / Status ADR",
    "Evidence-chain ADR",
    "Universal KB vs Paper Domain ADR",
    "prose and tables are authoritative",
    "Mermaid diagrams are optional",
]

REQUIRED_ADR000_MARKERS = [
    "# ADR-000: Universal KB North Star",
    "**Status:** Accepted",
    "**Binding Level:** binding",
    "local-first universal knowledge base",
    "scientific articles as the primary first domain",
    "GraphDB selection",
    "KnowledgeSubstratePort",
    "graph_import_allowed=false",
    "graphdb_written=false",
    "ladybugdb_written=false",
    "production_import_attempted=false",
    "import_eligible=false",
    "## 14. LLM Reading Notes",
]

REQUIRED_RD_REFERENCES = [
    "R024",
    "R027",
    "R029",
    "R040",
    "R050",
    "R054",
    "R055",
    "R056",
    "R057",
    "R058",
    "R059",
    "R060",
    "R061",
    "D065",
    "D066",
    "D067",
]


def load_text(path: Path, failures: list[str]) -> str:
    if not path.exists():
        failures.append(f"missing required file: {path}")
        return ""
    return path.read_text(encoding="utf-8")


def require_marker(text: str, marker: str, label: str, failures: list[str]) -> None:
    if marker not in text:
        failures.append(f"{label} missing marker: {marker}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-dir", required=True, type=Path)
    args = parser.parse_args()

    package_dir: Path = args.package_dir
    failures: list[str] = []

    template_path = package_dir / "ADR-TEMPLATE.md"
    index_path = package_dir / "ADR-INDEX.md"
    adr_path = package_dir / "ADR-000-universal-kb-north-star.md"
    audit_path = package_dir / "r-d-consistency-audit.json"

    template = load_text(template_path, failures)
    index = load_text(index_path, failures)
    adr = load_text(adr_path, failures)

    for marker in REQUIRED_TEMPLATE_MARKERS:
        require_marker(template, marker, "ADR-TEMPLATE", failures)

    for marker in ["ADR-TEMPLATE.md", "ADR-000", "Defer Final GraphDB Selection", "Non-Authorization Reminder"]:
        require_marker(index, marker, "ADR-INDEX", failures)

    for marker in REQUIRED_ADR000_MARKERS:
        require_marker(adr, marker, "ADR-000", failures)
    for marker in REQUIRED_RD_REFERENCES:
        require_marker(adr, marker, "ADR-000 R/D references", failures)

    mermaid_count = adr.count("```mermaid")
    if not 2 <= mermaid_count <= 5:
        failures.append(f"ADR-000 Mermaid diagram count must be between 2 and 5, got {mermaid_count}")

    if audit_path.exists():
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
        counts = audit.get("classification_counts", {})
        if counts.get("needs-clarification", 0) <= 0:
            failures.append("S01 audit did not expose needs-clarification records for ADR consumption")
        if "needs-clarification" not in index and "Needs clarification" not in adr and "needs-clarification" not in adr:
            failures.append("ADR package does not visibly consume S01 needs-clarification findings")
    else:
        failures.append(f"missing S01 audit artifact: {audit_path}")

    if failures:
        sys.stderr.write("M034 ADR template/north-star verification failed:\n")
        for failure in failures:
            sys.stderr.write(f"- {failure}\n")
        return 1

    sys.stdout.write("M034 ADR template/north-star verification passed\n")
    sys.stdout.write(f"template_markers={len(REQUIRED_TEMPLATE_MARKERS)}\n")
    sys.stdout.write(f"adr000_mermaid_count={mermaid_count}\n")
    sys.stdout.write(f"rd_references={len(REQUIRED_RD_REFERENCES)}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
