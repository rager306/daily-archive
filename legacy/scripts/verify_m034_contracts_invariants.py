#!/usr/bin/env python3
"""Verify M034 contracts and invariants artifacts."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

FILES = [
    "CONTRACTS.md",
    "SAFETY-INVARIANTS.md",
    "STATUS-MATRIX.md",
    "FAILURE-TAXONOMY.md",
    "ARTIFACT-DEPENDENCY-MODEL.md",
]
CONTRACT_MARKERS = [
    "KnowledgeSourceRecord",
    "DomainAdapterRecord",
    "EvidenceArtifactRecord",
    "ProcessingJob",
    "DependencyRecord",
    "FailureRecord",
    "CandidatePacket",
    "ReviewPacket",
    "GraphReadinessHandoff",
    "KnowledgeSubstratePort",
    "SafetyFlags",
    "ArticleRecord",
    "GROBIDSidecarArtifact",
    "OpenDataLoaderSidecarArtifact",
    "AdaptixMappingArtifact",
]
SAFETY_MARKERS = [
    "graph_import_allowed=false",
    "graphdb_written=false",
    "ladybugdb_written=false",
    "production_import_attempted=false",
    "import_eligible=false",
]
STATUS_MARKERS = [
    "pending",
    "ready",
    "running",
    "succeeded",
    "failed_retryable",
    "failed_terminal",
    "blocked",
    "stale",
    "needs_review",
    "skipped",
]
FAILURE_MARKERS = [
    "retryable",
    "terminal",
    "blocked",
    "stale",
    "needs_review",
    "missing_local_source",
    "backend_unhealthy",
    "model_cache_missing_no_network",
    "adaptix_mapping_failed",
    "graph_readiness_postcheck_failed",
]
DEPENDENCY_MARKERS = [
    "GROBID stale does not automatically make OpenDataLoader stale",
    "Source hash stale makes all dependent sidecars stale",
    "GraphReadinessHandoff",
    "No-write boundary",
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

    all_text = "\n".join(texts.values())
    for marker in CONTRACT_MARKERS:
        if marker not in texts["CONTRACTS.md"]:
            failures.append(f"CONTRACTS.md missing {marker}")
    for marker in SAFETY_MARKERS:
        if marker not in all_text:
            failures.append(f"package missing safety marker {marker}")
    for marker in STATUS_MARKERS:
        if marker not in texts["STATUS-MATRIX.md"]:
            failures.append(f"STATUS-MATRIX.md missing {marker}")
    for marker in FAILURE_MARKERS:
        if marker not in texts["FAILURE-TAXONOMY.md"]:
            failures.append(f"FAILURE-TAXONOMY.md missing {marker}")
    for marker in DEPENDENCY_MARKERS:
        if marker not in texts["ARTIFACT-DEPENDENCY-MODEL.md"]:
            failures.append(f"ARTIFACT-DEPENDENCY-MODEL.md missing {marker}")
    for name, text in texts.items():
        mermaid_count = text.count("```mermaid")
        if mermaid_count > 3:
            failures.append(f"{name} has too many Mermaid diagrams: {mermaid_count}")

    if failures:
        sys.stderr.write("M034 contracts/invariants verification failed:\n")
        for failure in failures:
            sys.stderr.write(f"- {failure}\n")
        return 1

    sys.stdout.write("M034 contracts/invariants verification passed\n")
    sys.stdout.write(
        f"files={len(FILES)} contracts={len(CONTRACT_MARKERS)} statuses={len(STATUS_MARKERS)}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
