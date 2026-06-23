from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.corpus.coverage import (
    CatalogCoverageInput,
    CorpusCoverageRequest,
    CorpusCoverageUseCase,
    CoverageSourceArtifact,
    GraphProbeCoverageInput,
    ParserCoverageInput,
)
from research_graph.infrastructure.corpus.reporting.coverage_report import (
    COVERAGE_REPORT_SCHEMA_VERSION,
    FilesystemCoverageReportWriter,
)


def _coverage_result():
    request = CorpusCoverageRequest(
        corpus_id="r024-218-document-corpus-v1",
        catalog=CatalogCoverageInput(
            total_records=166,
            index_entries=221,
            source_artifact=CoverageSourceArtifact(
                "data/r024-218-document-corpus-v1/ingest-summary.json",
                "json-summary",
                "r024-218-ingest-summary.v00.01",
            ),
        ),
        parser=ParserCoverageInput(
            total=221,
            completed=219,
            skipped=2,
            errors=0,
            chunk_count_total=2576,
            source_kind_counts={"html_native": 21, "pdf_converted": 198},
            skip_reason_counts={"metadata_only_no_local_source_artifact": 2},
            skipped_article_refs=(
                "arxiv/mixed-source/2605.29548",
                "stanford/cs224n/gradient-notes",
            ),
            source_artifact=CoverageSourceArtifact(
                "data/r024-218-document-corpus-v1/parser-chunking/summary.json",
                "json-summary",
            ),
        ),
        graph_probe=GraphProbeCoverageInput(
            corpus_size=219,
            skipped_metadata_only=2,
            chunk_count_total=2576,
            n_nodes=3891,
            n_edges=10102,
            citation_relations_count=6212,
            peak_memory_mb=13.81,
            source_artifact=CoverageSourceArtifact(
                "data/r024-218-document-corpus-v1/networkx-probe/summary.json",
                "json-summary",
            ),
        ),
    )
    return CorpusCoverageUseCase().run(request)


def test_coverage_report_writer_emits_markdown_and_json(tmp_path: Path) -> None:
    writer = FilesystemCoverageReportWriter(
        markdown_path=tmp_path / "R024-COVERAGE.md",
        json_path=tmp_path / "coverage-summary.json",
        generated_at="2026-06-23T00:00:00+00:00",
        milestone="M121-kd3kzr",
    )

    emitted = writer.write(_coverage_result())

    assert emitted.schema_version == COVERAGE_REPORT_SCHEMA_VERSION
    assert Path(emitted.markdown_path).exists()
    assert Path(emitted.json_path).exists()


def test_coverage_report_markdown_preserves_required_sections_and_counts(tmp_path: Path) -> None:
    report = tmp_path / "R024-COVERAGE.md"
    FilesystemCoverageReportWriter(
        markdown_path=report,
        json_path=tmp_path / "coverage-summary.json",
        generated_at="2026-06-23T00:00:00+00:00",
    ).write(_coverage_result())

    text = report.read_text()
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
    assert "221 article records" in text
    assert "166 M056" in text
    assert "219 source-backed" in text
    assert "2 metadata-only" in text
    assert "3891 nodes" in text
    assert "10102 edges" in text
    assert "13.81 MB" in text
    assert "metadata_only_no_local_source_artifact" in text
    assert "arxiv/mixed-source/2605.29548" in text
    assert "stanford/cs224n/gradient-notes" in text


def test_coverage_report_markdown_has_no_trailing_whitespace(tmp_path: Path) -> None:
    report = tmp_path / "R024-COVERAGE.md"
    FilesystemCoverageReportWriter(
        markdown_path=report,
        json_path=tmp_path / "coverage-summary.json",
        generated_at="2026-06-23T00:00:00+00:00",
    ).write(_coverage_result())

    for line in report.read_text().splitlines():
        assert line == line.rstrip()


def test_coverage_report_preserves_fail_closed_language(tmp_path: Path) -> None:
    report = tmp_path / "R024-COVERAGE.md"
    FilesystemCoverageReportWriter(
        markdown_path=report,
        json_path=tmp_path / "coverage-summary.json",
    ).write(_coverage_result())

    text = report.read_text()
    assert "NO network" in text
    assert "NO LadybugDB" in text
    assert "NO FalkorDB" in text
    assert "NO Neo4j" in text
    assert "NO production graph import" in text
    assert "does **not** claim production graph readiness" in text


def test_coverage_report_json_summary_is_schema_stable(tmp_path: Path) -> None:
    json_path = tmp_path / "coverage-summary.json"
    FilesystemCoverageReportWriter(
        markdown_path=tmp_path / "R024-COVERAGE.md",
        json_path=json_path,
        generated_at="2026-06-23T00:00:00+00:00",
    ).write(_coverage_result())

    summary = json.loads(json_path.read_text())
    assert summary["schema_version"] == COVERAGE_REPORT_SCHEMA_VERSION
    assert summary["catalog_records"] == 221
    assert summary["m056_records"] == 166
    assert summary["source_backed_records"] == 219
    assert summary["metadata_only_records"] == 2
    assert summary["parser_errors"] == 0
    assert summary["chunk_count_total"] == 2576
    assert summary["source_kind_counts"] == {"html_native": 21, "pdf_converted": 198}
    assert summary["skip_reason_counts"] == {"metadata_only_no_local_source_artifact": 2}
    assert summary["skipped_article_refs"] == [
        "arxiv/mixed-source/2605.29548",
        "stanford/cs224n/gradient-notes",
    ]
    assert summary["graph_nodes"] == 3891
    assert summary["graph_edges"] == 10102
    assert summary["citation_relations"] == 6212
    assert summary["network_fetch_attempted"] is False
    assert summary["ladybugdb_written"] is False
    assert [item["name"] for item in summary["denominators"]] == [
        "catalog_articles",
        "parser_replay_articles",
        "source_backed_articles",
    ]


def test_coverage_report_json_records_source_artifacts(tmp_path: Path) -> None:
    json_path = tmp_path / "coverage-summary.json"
    FilesystemCoverageReportWriter(
        markdown_path=tmp_path / "R024-COVERAGE.md",
        json_path=json_path,
    ).write(_coverage_result())

    summary = json.loads(json_path.read_text())
    assert [artifact["path"] for artifact in summary["source_artifacts"]] == [
        "data/r024-218-document-corpus-v1/ingest-summary.json",
        "data/r024-218-document-corpus-v1/parser-chunking/summary.json",
        "data/r024-218-document-corpus-v1/networkx-probe/summary.json",
    ]
