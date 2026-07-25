#!/usr/bin/env python3
"""Verify the M034 formal ADR package."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ADR_FILES = {
    "ADR-000": "ADR-000-universal-kb-north-star.md",
    "ADR-002": "ADR-002-defer-final-graphdb-selection.md",
    "ADR-003": "ADR-003-durable-lazy-async-evidence-pipeline.md",
    "ADR-004": "ADR-004-sidecars-as-candidate-evidence-producers.md",
    "ADR-005": "ADR-005-no-direct-extractor-to-graphdb-path.md",
    "ADR-006": "ADR-006-agent-boundary.md",
    "ADR-007": "ADR-007-quantmind-pattern-source-not-runtime-dependency.md",
}
REQUIRED_SECTIONS = [
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
]
SAFETY_MARKERS = [
    "graph_import_allowed=false",
    "graphdb_written=false",
    "ladybugdb_written=false",
    "production_import_attempted=false",
    "import_eligible=false",
]
ADR_SPECIFIC_MARKERS = {
    "ADR-000": [
        "local-first universal knowledge base",
        "scientific articles as the primary first domain",
    ],
    "ADR-002": ["LadybugDB", "FalkorDB", "HelixDB", "KnowledgeSubstratePort"],
    "ADR-003": ["durable lazy", "retry", "stale"],
    "ADR-004": ["candidate evidence", "GROBID", "OpenDataLoader", "Adaptix"],
    "ADR-005": ["No Direct", "GraphDB", "direct writes"],
    "ADR-006": ["Agents", "optional future helpers", "not current core orchestrator"],
    "ADR-007": ["quant-mind", "pattern source", "runtime dependency"],
}


def require(condition: bool, message: str, failures: list[str]) -> None:
    if not condition:
        failures.append(message)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-dir", required=True, type=Path)
    args = parser.parse_args()
    package_dir: Path = args.package_dir
    failures: list[str] = []

    index_path = package_dir / "ADR-INDEX.md"
    index = index_path.read_text(encoding="utf-8") if index_path.exists() else ""
    require(bool(index), f"missing ADR index: {index_path}", failures)

    audit_path = package_dir / "r-d-consistency-audit.json"
    if audit_path.exists():
        audit = json.loads(audit_path.read_text(encoding="utf-8"))
        require(
            audit.get("classification_counts", {}).get("needs-clarification", 0) == 15,
            "unexpected S01 needs-clarification count",
            failures,
        )
    else:
        failures.append(f"missing S01 audit: {audit_path}")

    for adr_id, filename in ADR_FILES.items():
        path = package_dir / filename
        if not path.exists():
            failures.append(f"missing {adr_id}: {path}")
            continue
        text = path.read_text(encoding="utf-8")
        require(
            text.startswith(f"# {adr_id}:"),
            f"{adr_id} title does not start with expected heading",
            failures,
        )
        for section in REQUIRED_SECTIONS:
            require(section in text, f"{adr_id} missing section {section}", failures)
        for marker in SAFETY_MARKERS:
            require(marker in text, f"{adr_id} missing safety marker {marker}", failures)
        for marker in ADR_SPECIFIC_MARKERS.get(adr_id, []):
            require(marker in text, f"{adr_id} missing specific marker {marker}", failures)
        mermaid_count = text.count("```mermaid")
        require(
            1 <= mermaid_count <= 5,
            f"{adr_id} Mermaid count out of bounds: {mermaid_count}",
            failures,
        )
        require("LLM Reading Notes" in text, f"{adr_id} missing LLM Reading Notes", failures)
        require(adr_id in index, f"ADR index missing {adr_id}", failures)

    expected_statuses = {
        "ADR-000": "Accepted",
        "ADR-002": "Deferred",
        "ADR-003": "Accepted",
        "ADR-004": "Accepted",
        "ADR-005": "Accepted",
        "ADR-006": "Accepted",
        "ADR-007": "Accepted",
    }
    for adr_id, status in expected_statuses.items():
        matching_rows = [line for line in index.splitlines() if line.startswith(f"| {adr_id} |")]
        require(bool(matching_rows), f"ADR index missing row for {adr_id}", failures)
        if matching_rows:
            require(
                f"| {status} |" in matching_rows[0],
                f"ADR index row for {adr_id} missing status {status}",
                failures,
            )

    if failures:
        sys.stderr.write("M034 formal ADR package verification failed:\n")
        for failure in failures:
            sys.stderr.write(f"- {failure}\n")
        return 1

    sys.stdout.write("M034 formal ADR package verification passed\n")
    sys.stdout.write(f"adr_count={len(ADR_FILES)}\n")
    sys.stdout.write(
        "statuses=ADR-000 Accepted, ADR-002 Deferred, ADR-003/004/005/006/007 Accepted\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
