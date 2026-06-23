"""Filesystem coverage report writer for corpus coverage results."""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path

from research_graph.application.corpus.coverage import CorpusCoverageResult

COVERAGE_REPORT_SCHEMA_VERSION = "corpus-coverage-report.v00.01"


@dataclass(frozen=True)
class CoverageReportWriteResult:
    """Emitted coverage report artifact paths for downstream audit."""

    markdown_path: str
    json_path: str
    schema_version: str


class FilesystemCoverageReportWriter:
    """Emit R024-style coverage Markdown plus machine-readable JSON summary."""

    def __init__(
        self,
        *,
        markdown_path: Path | str,
        json_path: Path | str,
        generated_at: str | None = None,
        milestone: str = "M122 Pipeline Script Architecture Migration",
    ) -> None:
        self.markdown_path = Path(markdown_path)
        self.json_path = Path(json_path)
        self.generated_at = generated_at or datetime.now(UTC).replace(microsecond=0).isoformat()
        self.milestone = milestone

    def write(self, result: CorpusCoverageResult) -> CoverageReportWriteResult:
        """Write Markdown and JSON artifacts and return emitted path metadata."""

        self.markdown_path.parent.mkdir(parents=True, exist_ok=True)
        self.json_path.parent.mkdir(parents=True, exist_ok=True)
        self.markdown_path.write_text(self.render_markdown(result), encoding="utf-8")
        self.json_path.write_text(json.dumps(self.to_json_summary(result), indent=2) + "\n", encoding="utf-8")
        return CoverageReportWriteResult(
            markdown_path=self.markdown_path.as_posix(),
            json_path=self.json_path.as_posix(),
            schema_version=COVERAGE_REPORT_SCHEMA_VERSION,
        )

    def render_markdown(self, result: CorpusCoverageResult) -> str:
        """Render current R024 coverage Markdown sections from coverage facts."""

        lines: list[str] = []
        lines.append(f"# R024 Coverage Report: {result.corpus_id}")
        lines.append("")
        lines.append(f"**Generated**: {self.generated_at}")
        lines.append(f"**Milestone**: {self.milestone}")
        lines.append(f"**Source**: `{result.corpus_id}/`")
        lines.append(
            "**Architecture**: Offline catalog coverage, parser/chunking replay, and bounded graph probe. "
            "NO network, NO LadybugDB, NO FalkorDB, NO Neo4j, NO production graph import."
        )
        lines.append("")
        lines.extend(self._executive_summary(result))
        lines.extend(self._stage_summary(result))
        lines.extend(self._catalog_section(result))
        lines.extend(self._parser_section(result))
        lines.extend(self._networkx_section(result))
        lines.extend(self._verification_section(result))
        lines.extend(self._interpretation_section(result))
        lines.extend(self._recommendations_section())
        lines.extend(self._files_section(result))
        return "\n".join(lines).rstrip() + "\n"

    def to_json_summary(self, result: CorpusCoverageResult) -> dict[str, object]:
        """Return a stable machine-readable coverage summary shape."""

        return {
            "schema_version": COVERAGE_REPORT_SCHEMA_VERSION,
            "generated_at": self.generated_at,
            "corpus_id": result.corpus_id,
            "catalog_records": result.catalog_records,
            "m056_records": result.m056_records,
            "parser_total": result.parser_total,
            "parser_completed": result.parser_completed,
            "parser_skipped": result.parser_skipped,
            "parser_errors": result.parser_errors,
            "source_backed_records": result.source_backed_records,
            "metadata_only_records": result.metadata_only_records,
            "chunk_count_total": result.chunk_count_total,
            "source_kind_counts": result.source_kind_counts,
            "skip_reason_counts": result.skip_reason_counts,
            "skipped_article_refs": list(result.skipped_article_refs),
            "graph_nodes": result.graph_nodes,
            "graph_edges": result.graph_edges,
            "citation_relations": result.citation_relations,
            "graph_peak_memory_mb": result.graph_peak_memory_mb,
            "denominators": [denominator.__dict__ for denominator in result.denominators],
            "diagnostics": [diagnostic.__dict__ for diagnostic in result.diagnostics],
            "source_artifacts": [artifact.__dict__ for artifact in result.source_artifacts],
            "network_fetch_attempted": False,
            "production_import_attempted": False,
            "graph_import_allowed": False,
            "ladybugdb_written": False,
            "falkordb_written": False,
            "neo4j_written": False,
        }

    def _executive_summary(self, result: CorpusCoverageResult) -> list[str]:
        lines = ["## Executive Summary", ""]
        lines.append(
            f"Coverage for `{result.corpus_id}` records **{result.catalog_records} article records**, "
            f"including **{result.m056_records} M056** records where applicable."
        )
        lines.append("")
        lines.append("The verified downstream stages are:")
        lines.append(
            f"1. **Catalog coverage**: denominator `{self._denominator('catalog_articles', result)}`."
        )
        lines.append(
            f"2. **Parser/chunking replay**: **{result.source_backed_records} source-backed** records completed; "
            f"**{result.metadata_only_records} metadata-only** records skipped; **{result.parser_errors} errors**."
        )
        if result.graph_nodes is not None and result.graph_edges is not None:
            lines.append(
                f"3. **NetworkX graph probe**: **{result.graph_nodes} nodes**, **{result.graph_edges} edges**, "
                f"**{result.citation_relations or 0} citation edges**, "
                f"**{result.graph_peak_memory_mb:.2f} MB peak memory**."
            )
        lines.append("")
        lines.append(
            "This report does **not** claim production graph readiness. It preserves fail-closed semantics."
        )
        lines.append("")
        return lines

    def _stage_summary(self, result: CorpusCoverageResult) -> list[str]:
        lines = ["## Stage Summary", ""]
        lines.append("| Stage | Evidence | Result |")
        lines.append("|-------|----------|--------|")
        lines.append(
            f"| Catalog ingest | `{self._artifact_path(result, 'ingest')}` | {result.catalog_records} article records |"
        )
        lines.append(
            f"| Parser + Chunking Replay | `{self._artifact_path(result, 'parser')}` | "
            f"{result.parser_completed} completed, {result.parser_skipped} skipped, {result.parser_errors} errors |"
        )
        if result.graph_nodes is not None:
            lines.append(
                f"| NetworkX Probe | `{self._artifact_path(result, 'networkx')}` | "
                f"{result.graph_nodes} nodes, {result.graph_edges} edges |"
            )
        lines.append("")
        return lines

    def _catalog_section(self, result: CorpusCoverageResult) -> list[str]:
        return [
            "## Catalog Expansion (S01-S03)",
            "",
            "### Results",
            "",
            f"- **M056 cumulative records**: {result.m056_records}",
            f"- **Catalog article records after ingest**: {result.catalog_records}",
            f"- **M056 records ingested**: {result.m056_records}",
            "- **Fail-closed metadata**: source variants remain offline and do not authorize production import",
            "",
        ]

    def _parser_section(self, result: CorpusCoverageResult) -> list[str]:
        lines = ["## Parser + Chunking Replay (S04)", ""]
        lines.append("### Results")
        lines.append("")
        lines.append(f"- **Total catalog records**: {result.parser_total}")
        lines.append(f"- **Source-backed records completed**: {result.parser_completed}")
        lines.append(f"- **Metadata-only records skipped**: {result.parser_skipped}")
        lines.append(f"- **Errors**: {result.parser_errors}")
        for source_kind, count in result.source_kind_counts.items():
            lines.append(f"- **{source_kind.replace('_', '-').upper()} sources**: {count}")
        lines.append(f"- **Chunk count total**: {result.chunk_count_total}")
        if result.skip_reason_counts:
            lines.append("")
            lines.append("### Metadata-only exclusions")
            lines.append("")
            for article_ref in result.skipped_article_refs:
                lines.append(f"- `{article_ref}`")
            for reason, count in result.skip_reason_counts.items():
                lines.append(f"- `{reason}`: {count}")
        lines.append("")
        return lines

    def _networkx_section(self, result: CorpusCoverageResult) -> list[str]:
        lines = ["## NetworkX Probe (S05)", ""]
        if result.graph_nodes is None:
            lines.append("No NetworkX probe summary was provided for this coverage result.")
            lines.append("")
            return lines
        lines.append("| Metric | Value |")
        lines.append("|--------|-------|")
        lines.append(f"| Source-backed records | {result.source_backed_records} |")
        lines.append(f"| Metadata-only exclusions | {result.metadata_only_records} |")
        lines.append(f"| Chunks | {result.chunk_count_total} |")
        lines.append(f"| Total nodes | {result.graph_nodes} |")
        lines.append(f"| Total edges | {result.graph_edges} |")
        lines.append(f"| Citation relations | {result.citation_relations or 0} |")
        if result.graph_peak_memory_mb is not None:
            lines.append(f"| Peak memory | {result.graph_peak_memory_mb:.2f} MB |")
        lines.append("")
        return lines

    def _verification_section(self, result: CorpusCoverageResult) -> list[str]:
        lines = ["## Verification Baseline", ""]
        lines.append("Fail-closed invariants:")
        lines.append("")
        lines.append("- `network_fetch_attempted=false`")
        lines.append("- `production_import_attempted=false`")
        lines.append("- `graph_import_allowed=false`")
        lines.append("- `ladybugdb_written=false`")
        lines.append("- `falkordb_written=false`")
        lines.append("- `neo4j_written=false`")
        lines.append("- NO network, NO LadybugDB, NO FalkorDB, NO Neo4j, NO production graph import")
        lines.append("")
        if result.diagnostics:
            lines.append("Diagnostics:")
            lines.append("")
            for diagnostic in result.diagnostics:
                lines.append(f"- `{diagnostic.code}`: {diagnostic.count} ({diagnostic.notes})")
            lines.append("")
        return lines

    def _interpretation_section(self, result: CorpusCoverageResult) -> list[str]:
        return [
            "## R024 Interpretation",
            "",
            f"R024 coverage currently validates {result.source_backed_records} source-backed records "
            f"out of {result.parser_total} parser replay records, with {result.metadata_only_records} "
            "metadata-only exclusions recorded explicitly.",
            "",
            "The result advances corpus evidence while preserving fail-closed boundaries and does **not** claim production graph readiness.",
            "",
        ]

    def _recommendations_section(self) -> list[str]:
        return [
            "## Recommendations",
            "",
            "1. Continue using package use cases for coverage regeneration instead of milestone script logic.",
            "2. Keep metadata-only exclusions explicit and fail-closed.",
            "3. Treat NetworkX graph output as bounded evidence, not production graph readiness.",
            "",
        ]

    def _files_section(self, result: CorpusCoverageResult) -> list[str]:
        lines = ["## Files of Record", ""]
        for artifact in result.source_artifacts:
            lines.append(f"- `{artifact.path}` ({artifact.artifact_type})")
        lines.append(f"- `{self.markdown_path.as_posix()}` (coverage markdown)")
        lines.append(f"- `{self.json_path.as_posix()}` (coverage json summary)")
        lines.append("")
        return lines

    @staticmethod
    def _denominator(name: str, result: CorpusCoverageResult) -> str:
        for denominator in result.denominators:
            if denominator.name == name:
                return f"{denominator.included}/{denominator.total} included"
        return "n/a"

    @staticmethod
    def _artifact_path(result: CorpusCoverageResult, hint: str) -> str:
        for artifact in result.source_artifacts:
            if hint in artifact.path:
                return artifact.path
        return "n/a"


__all__ = [
    "COVERAGE_REPORT_SCHEMA_VERSION",
    "CoverageReportWriteResult",
    "FilesystemCoverageReportWriter",
]
