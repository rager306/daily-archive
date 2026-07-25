"""Application-layer corpus coverage aggregation.

The coverage use case consumes catalog/parser/probe summary contracts that are
already loaded by adapters. It owns denominator definitions and source artifact
references without importing filesystem, Markdown, or script-specific modules.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Protocol, runtime_checkable


@dataclass(frozen=True)
class CoverageSourceArtifact:
    """Artifact reference used as coverage evidence."""

    path: str
    artifact_type: str
    schema_version: str | None = None


@dataclass(frozen=True)
class CoverageDenominator:
    """Explicit denominator definition for reproducible coverage percentages."""

    name: str
    definition: str
    total: int
    included: int
    excluded: int = 0
    errors: int = 0


@dataclass(frozen=True)
class CoverageDiagnostic:
    """Reason-coded coverage diagnostic without source payload values."""

    code: str
    count: int
    source_artifact: str | None = None
    notes: str = ""


@dataclass(frozen=True)
class CatalogCoverageInput:
    """Catalog coverage facts loaded from ingest/index artifacts."""

    total_records: int
    index_entries: int | None
    ingested_count: int = 0
    skipped_count: int = 0
    failed_count: int = 0
    source_artifact: CoverageSourceArtifact | None = None


@dataclass(frozen=True)
class ParserCoverageInput:
    """Parser replay coverage facts loaded from parser summary artifacts."""

    total: int
    completed: int
    skipped: int
    errors: int
    chunk_count_total: int
    source_kind_counts: Mapping[str, int] = field(default_factory=dict)
    skip_reason_counts: Mapping[str, int] = field(default_factory=dict)
    skipped_article_refs: tuple[str, ...] = ()
    source_artifact: CoverageSourceArtifact | None = None


@dataclass(frozen=True)
class GraphProbeCoverageInput:
    """Optional graph probe facts used by R024/M121 coverage reports."""

    corpus_size: int
    skipped_metadata_only: int
    chunk_count_total: int
    n_nodes: int
    n_edges: int
    citation_relations_count: int = 0
    peak_memory_mb: float | None = None
    source_artifact: CoverageSourceArtifact | None = None


@dataclass(frozen=True)
class CorpusCoverageRequest:
    """Coverage aggregation request for one corpus report."""

    corpus_id: str
    catalog: CatalogCoverageInput
    parser: ParserCoverageInput
    graph_probe: GraphProbeCoverageInput | None = None
    source_artifacts: list[CoverageSourceArtifact] = field(default_factory=list)


@dataclass(frozen=True)
class CorpusCoverageResult:
    """Aggregated corpus coverage facts for report writers."""

    corpus_id: str
    catalog_records: int
    cumulative_corpus_records: int
    parser_total: int
    parser_completed: int
    parser_skipped: int
    parser_errors: int
    chunk_count_total: int
    source_kind_counts: dict[str, int]
    skip_reason_counts: dict[str, int]
    skipped_article_refs: tuple[str, ...]
    denominators: list[CoverageDenominator]
    diagnostics: list[CoverageDiagnostic]
    source_artifacts: list[CoverageSourceArtifact]
    graph_nodes: int | None = None
    graph_edges: int | None = None
    citation_relations: int | None = None
    graph_peak_memory_mb: float | None = None

    @property
    def source_backed_records(self) -> int:
        """Completed parser records with source artifacts."""

        return self.parser_completed

    @property
    def metadata_only_records(self) -> int:
        """Parser skipped records considered out of source-backed denominator."""

        return self.parser_skipped

    @property
    def succeeded(self) -> bool:
        """True when parser replay has no errors."""

        return self.parser_errors == 0


@runtime_checkable
class CoverageSummarySourcePort(Protocol):
    """Boundary for loading already-produced coverage summary contracts."""

    def coverage_request(self) -> CorpusCoverageRequest:
        """Return a coverage aggregation request."""
        ...


class CorpusCoverageUseCase:
    """Aggregate corpus coverage from catalog/parser/probe summaries."""

    def run(self, request: CorpusCoverageRequest) -> CorpusCoverageResult:
        """Return reproducible coverage totals and denominator definitions."""

        catalog_records = request.catalog.index_entries or request.catalog.total_records
        source_artifacts = self._source_artifacts(request)
        denominators = [
            CoverageDenominator(
                name="catalog_articles",
                definition="All article records in the canonical article catalog index.",
                total=catalog_records,
                included=catalog_records,
            ),
            CoverageDenominator(
                name="parser_replay_articles",
                definition="All catalog articles considered by parser/chunking replay.",
                total=request.parser.total,
                included=request.parser.completed,
                excluded=request.parser.skipped,
                errors=request.parser.errors,
            ),
            CoverageDenominator(
                name="source_backed_articles",
                definition="Parser replay records with local source artifacts and completed chunking.",
                total=request.parser.completed + request.parser.skipped,
                included=request.parser.completed,
                excluded=request.parser.skipped,
                errors=request.parser.errors,
            ),
        ]
        diagnostics = self._diagnostics(request)
        graph = request.graph_probe
        return CorpusCoverageResult(
            corpus_id=request.corpus_id,
            catalog_records=catalog_records,
            cumulative_corpus_records=request.catalog.total_records,
            parser_total=request.parser.total,
            parser_completed=request.parser.completed,
            parser_skipped=request.parser.skipped,
            parser_errors=request.parser.errors,
            chunk_count_total=request.parser.chunk_count_total,
            source_kind_counts=dict(sorted(request.parser.source_kind_counts.items())),
            skip_reason_counts=dict(sorted(request.parser.skip_reason_counts.items())),
            skipped_article_refs=tuple(sorted(request.parser.skipped_article_refs)),
            denominators=denominators,
            diagnostics=diagnostics,
            source_artifacts=source_artifacts,
            graph_nodes=graph.n_nodes if graph else None,
            graph_edges=graph.n_edges if graph else None,
            citation_relations=graph.citation_relations_count if graph else None,
            graph_peak_memory_mb=graph.peak_memory_mb if graph else None,
        )

    def _source_artifacts(self, request: CorpusCoverageRequest) -> list[CoverageSourceArtifact]:
        artifacts = list(request.source_artifacts)
        for candidate in (
            request.catalog.source_artifact,
            request.parser.source_artifact,
            request.graph_probe.source_artifact if request.graph_probe else None,
        ):
            if candidate is not None and candidate not in artifacts:
                artifacts.append(candidate)
        return artifacts

    def _diagnostics(self, request: CorpusCoverageRequest) -> list[CoverageDiagnostic]:
        diagnostics: list[CoverageDiagnostic] = []
        for reason, count in sorted(request.parser.skip_reason_counts.items()):
            diagnostics.append(
                CoverageDiagnostic(
                    code=reason,
                    count=int(count),
                    source_artifact=(
                        request.parser.source_artifact.path
                        if request.parser.source_artifact is not None
                        else None
                    ),
                    notes="parser replay skipped records",
                )
            )
        if request.parser.errors:
            diagnostics.append(
                CoverageDiagnostic(
                    code="parser_errors",
                    count=request.parser.errors,
                    source_artifact=(
                        request.parser.source_artifact.path
                        if request.parser.source_artifact is not None
                        else None
                    ),
                    notes="parser replay errors must be remediated before coverage is clean",
                )
            )
        if request.catalog.failed_count:
            diagnostics.append(
                CoverageDiagnostic(
                    code="catalog_ingest_failures",
                    count=request.catalog.failed_count,
                    source_artifact=(
                        request.catalog.source_artifact.path
                        if request.catalog.source_artifact is not None
                        else None
                    ),
                    notes="catalog ingest failures affect coverage denominator",
                )
            )
        return diagnostics


__all__ = [
    "CatalogCoverageInput",
    "CorpusCoverageRequest",
    "CorpusCoverageResult",
    "CorpusCoverageUseCase",
    "CoverageDenominator",
    "CoverageDiagnostic",
    "CoverageSourceArtifact",
    "CoverageSummarySourcePort",
    "GraphProbeCoverageInput",
    "ParserCoverageInput",
]
