"""Tests for M121 S06 R024 coverage report."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

REPORT = Path("data/r024-218-document-corpus-v1/R024-COVERAGE.md")
INGEST_SUMMARY = Path("data/r024-218-document-corpus-v1/ingest-summary.json")
PARSER_SUMMARY = Path("data/r024-218-document-corpus-v1/parser-chunking/summary.json")
NETWORKX_SUMMARY = Path("data/r024-218-document-corpus-v1/networkx-probe/summary.json")
MEMORY_PROFILE = Path("data/r024-218-document-corpus-v1/networkx-probe/memory-profile.json")


def _text() -> str:
    return REPORT.read_text()


def _load(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text())
    return data if isinstance(data, dict) else {}


def test_coverage_report_exists() -> None:
    assert REPORT.exists()


def test_report_has_required_sections() -> None:
    text = _text()
    for heading in (
        "## Executive Summary",
        "## Stage Summary",
        "## Catalog Expansion (S01-S03)",
        "## Parser + Chunking Replay (S04)",
        "## NetworkX Probe (S05)",
        "## Verification Baseline",
        "## R024 Interpretation",
        "## Recommendations",
        "## Files of Record",
    ):
        assert heading in text


def test_report_mentions_core_counts() -> None:
    text = _text()
    assert "221 article records" in text
    assert "166 M056" in text
    assert "219 source-backed" in text
    assert "2 metadata-only" in text
    assert "3891 nodes" in text
    assert "10102 edges" in text
    assert "13.81 MB" in text


def test_report_preserves_fail_closed_language() -> None:
    text = _text()
    assert "NO network" in text
    assert "NO LadybugDB" in text
    assert "NO FalkorDB" in text
    assert "NO Neo4j" in text
    assert "NO production graph import" in text
    assert "does **not** claim production graph readiness" in text


def test_report_names_metadata_only_exclusions() -> None:
    text = _text()
    assert "arxiv/mixed-source/2605.29548" in text
    assert "stanford/cs224n/gradient-notes" in text
    assert "metadata_only_no_local_source_artifact" in text


def test_report_matches_ingest_summary() -> None:
    ingest = _load(INGEST_SUMMARY)
    text = _text()
    assert ingest["total_records"] == 166
    assert ingest["index_entries"] == 221
    assert "Catalog article records after ingest**: 221" in text
    assert "M056 records ingested**: 166" in text


def test_report_matches_parser_summary() -> None:
    parser = _load(PARSER_SUMMARY)
    assert parser["total"] == 221
    assert parser["ok"] == 219
    assert parser["skipped"] == 2
    assert parser["errors"] == 0
    assert parser["chunk_count_total"] == 2576
    assert parser["source_kind_counts"] == {"html_native": 21, "pdf_converted": 198}


def test_report_matches_networkx_summary() -> None:
    summary = _load(NETWORKX_SUMMARY)
    assert summary["corpus_size"] == 219
    assert summary["skipped_metadata_only"] == 2
    assert summary["chunk_count_total"] == 2576
    assert summary["n_nodes"] == 3891
    assert summary["n_edges"] == 10102
    assert summary["citation_relations_count"] == 6212


def test_memory_profile_matches_report_bound() -> None:
    profile = _load(MEMORY_PROFILE)
    assert float(profile["peak_mb"]) < 50
    assert round(float(profile["peak_mb"]), 2) == 13.81


def test_report_does_not_claim_production_readiness() -> None:
    text = _text().lower()
    forbidden_phrases = (
        "production graph readiness is complete",
        "graph import allowed=true",
        "ladybugdb_written=true",
        "trusted_kg_import_allowed=true",
    )
    for phrase in forbidden_phrases:
        assert phrase not in text
