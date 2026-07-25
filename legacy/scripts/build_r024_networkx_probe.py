#!/usr/bin/env python3
"""R024 S04: bounded NetworkX graph probe wrapper for 10 articles."""

from __future__ import annotations

import sys
from pathlib import Path

from research_graph.application.graph.probe import GraphProbeUseCase
from research_graph.infrastructure.graph.networkx_probe import NetworkXGraphProbeAdapter
from research_graph.infrastructure.graph.r024_networkx_probe import (
    R024NetworkXProbeConfig,
    build_request,
    write_legacy_artifacts,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
R024_DIR = REPO_ROOT / "data" / "r024-10-document-corpus-v1"
PROBE_DIR = R024_DIR / "networkx-probe"
SELECTION = R024_DIR / "selection.json"
EVENTS_LOG = R024_DIR / "parser-chunking" / "events.jsonl"
GRAPHML = PROBE_DIR / "probe.graphml"
SUMMARY = PROBE_DIR / "summary.json"
PROBE_EVENTS = PROBE_DIR / "events.jsonl"


def main() -> int:
    config = R024NetworkXProbeConfig(
        corpus_id="r024-10-document-corpus-v1",
        corpus_dir=R024_DIR,
        selection_path=SELECTION,
        parser_events_path=EVENTS_LOG,
        probe_dir=PROBE_DIR,
        graphml_path=GRAPHML,
        summary_path=SUMMARY,
        events_path=PROBE_EVENTS,
        summary_schema_version="r024-networkx-probe-summary.v00.01",
        entity_types=(),
        include_citation_relations=False,
    )
    request = build_request(config)
    adapter = NetworkXGraphProbeAdapter(
        graphml_path=GRAPHML,
        include_citation_relations=config.include_citation_relations,
    )
    result = GraphProbeUseCase().run(request, adapter)
    write_legacy_artifacts(result, config)
    metrics = result.metrics
    print(
        "summary: "
        f"nodes={metrics.n_nodes if metrics else 0}, "
        f"edges={metrics.n_edges if metrics else 0}"
    )
    print(f"summary_path={SUMMARY}")
    if result.first_failure_code:
        print(f"first_failure_code={result.first_failure_code}")
    return 0 if result.succeeded else 1


if __name__ == "__main__":
    sys.exit(main())
