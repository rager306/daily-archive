"""Application-layer graph probe orchestration contracts.

The graph probe use case owns corpus/probe accounting and diagnostic shape while
leaving graph construction and metric extraction to an infrastructure adapter.
It deliberately has no NetworkX, filesystem, database, or script imports.
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Protocol, runtime_checkable

GRAPH_PROBE_SCHEMA_VERSION = "graph-probe-result.v00.01"

FAIL_CLOSED_INVARIANTS: dict[str, bool] = {
    "network_fetch_attempted": False,
    "production_import_attempted": False,
    "graph_import_allowed": False,
    "ladybugdb_written": False,
    "trusted_kg_import_allowed": False,
    "graph_readiness_claim": False,
    "falkordb_written": False,
    "neo4j_written": False,
    "ladybugdb_connection_attempted": False,
}


@dataclass(frozen=True)
class GraphProbeArtifactRef:
    """Path-like reference to a graph probe input or output artifact."""

    path: str
    artifact_type: str
    schema_version: str | None = None


@dataclass(frozen=True)
class GraphProbeArticleEvidence:
    """Source-backed article/chunk evidence consumed by a graph probe adapter."""

    article_ref: str
    article_key: str
    chunk_count: int
    source_kind: str = "unknown"
    text_source: str = ""


@dataclass(frozen=True)
class GraphProbeExcludedRecord:
    """Catalog record intentionally excluded from source-backed graph probing."""

    article_ref: str
    article_key: str
    skip_reason: str


@dataclass(frozen=True)
class GraphProbeMetrics:
    """Graph metrics reported by the infrastructure graph probe adapter."""

    n_nodes: int
    n_edges: int
    node_types: Mapping[str, int] = field(default_factory=dict)
    edge_types: Mapping[str, int] = field(default_factory=dict)
    citation_relations_count: int = 0


@dataclass(frozen=True)
class GraphProbeMemoryProfile:
    """Bounded memory metrics reported by the graph probe adapter."""

    method: str
    peak_mb: float
    current_mb: float | None = None
    tracemalloc_current_bytes: int | None = None
    tracemalloc_peak_bytes: int | None = None
    approx_bytes_per_node: int | None = None
    n_nodes: int | None = None
    n_edges: int | None = None
    top_allocations: tuple[Mapping[str, object], ...] = ()


@dataclass(frozen=True)
class GraphProbeDiagnostic:
    """Reason-coded diagnostic that does not include corpus body text."""

    code: str
    phase: str
    severity: str = "error"
    count: int = 1
    artifact_ref: str | None = None
    exception_class: str | None = None
    notes: str = ""


@dataclass(frozen=True)
class GraphProbeRequest:
    """Application request for one bounded graph probe run."""

    corpus_id: str
    completed_articles: Sequence[GraphProbeArticleEvidence]
    excluded_records: Sequence[GraphProbeExcludedRecord] = ()
    input_artifacts: Sequence[GraphProbeArtifactRef] = ()
    entity_types: Sequence[str] = ()


@dataclass(frozen=True)
class GraphProbeExecutionResult:
    """Infrastructure adapter result before application-level accounting."""

    metrics: GraphProbeMetrics | None = None
    memory_profile: GraphProbeMemoryProfile | None = None
    implementation: Mapping[str, object] = field(default_factory=dict)
    output_artifacts: Sequence[GraphProbeArtifactRef] = ()
    diagnostics: Sequence[GraphProbeDiagnostic] = ()

    @property
    def succeeded(self) -> bool:
        """True when the adapter produced metrics without error diagnostics."""

        return self.metrics is not None and not any(
            diagnostic.severity == "error" for diagnostic in self.diagnostics
        )


@dataclass(frozen=True)
class GraphProbeResult:
    """Application graph probe result for writers and script wrappers."""

    schema_version: str
    corpus_id: str
    total_catalog_records_seen: int
    corpus_size: int
    skipped_metadata_only: int
    completed_articles: tuple[GraphProbeArticleEvidence, ...]
    excluded_records: tuple[GraphProbeExcludedRecord, ...]
    chunk_count_total: int
    source_kind_counts: dict[str, int]
    metrics: GraphProbeMetrics | None
    memory_profile: GraphProbeMemoryProfile | None
    input_artifacts: tuple[GraphProbeArtifactRef, ...]
    output_artifacts: tuple[GraphProbeArtifactRef, ...]
    implementation: dict[str, object]
    fail_closed_invariants: dict[str, bool]
    diagnostics: tuple[GraphProbeDiagnostic, ...] = ()

    @property
    def succeeded(self) -> bool:
        """True when metrics exist and no error diagnostics were recorded."""

        return self.metrics is not None and not any(
            diagnostic.severity == "error" for diagnostic in self.diagnostics
        )

    @property
    def failure_phase(self) -> str | None:
        """First error phase for compact wrapper reporting."""

        for diagnostic in self.diagnostics:
            if diagnostic.severity == "error":
                return diagnostic.phase
        return None

    @property
    def first_failure_code(self) -> str | None:
        """First error code for compact wrapper reporting."""

        for diagnostic in self.diagnostics:
            if diagnostic.severity == "error":
                return diagnostic.code
        return None


@runtime_checkable
class GraphProbeExecutionPort(Protocol):
    """Infrastructure boundary for graph construction and metric extraction."""

    def execute(self, request: GraphProbeRequest) -> GraphProbeExecutionResult:
        """Build the probe graph and return graph/memory metrics."""
        ...


class GraphProbeUseCase:
    """Run bounded graph probe orchestration through an execution port."""

    def run(
        self,
        request: GraphProbeRequest,
        executor: GraphProbeExecutionPort,
    ) -> GraphProbeResult:
        """Validate input, call the adapter, and return stable probe accounting."""

        validation_diagnostics = self._validate(request)
        if validation_diagnostics:
            return self._result(request, diagnostics=validation_diagnostics)

        try:
            execution = executor.execute(request)
        except Exception as exc:  # pragma: no cover - exact adapter failures are tested via fakes.
            diagnostic = GraphProbeDiagnostic(
                code="adapter_exception",
                phase="adapter_execution",
                exception_class=exc.__class__.__name__,
                notes="graph probe adapter raised before producing metrics",
            )
            return self._result(request, diagnostics=(diagnostic,))

        return self._result(
            request,
            metrics=execution.metrics,
            memory_profile=execution.memory_profile,
            implementation=dict(execution.implementation),
            output_artifacts=tuple(execution.output_artifacts),
            diagnostics=tuple(execution.diagnostics),
        )

    def _validate(self, request: GraphProbeRequest) -> tuple[GraphProbeDiagnostic, ...]:
        diagnostics: list[GraphProbeDiagnostic] = []
        if not request.corpus_id:
            diagnostics.append(
                GraphProbeDiagnostic(
                    code="missing_corpus_id",
                    phase="input_validation",
                    notes="corpus_id is required",
                )
            )
        bad_chunk_counts = sum(
            1 for article in request.completed_articles if article.chunk_count < 0
        )
        if bad_chunk_counts:
            diagnostics.append(
                GraphProbeDiagnostic(
                    code="invalid_chunk_count",
                    phase="input_validation",
                    count=bad_chunk_counts,
                    notes="completed article chunk counts must be non-negative",
                )
            )
        return tuple(diagnostics)

    def _result(
        self,
        request: GraphProbeRequest,
        *,
        metrics: GraphProbeMetrics | None = None,
        memory_profile: GraphProbeMemoryProfile | None = None,
        implementation: Mapping[str, object] | None = None,
        output_artifacts: Sequence[GraphProbeArtifactRef] = (),
        diagnostics: Sequence[GraphProbeDiagnostic] = (),
    ) -> GraphProbeResult:
        source_kind_counts = Counter(
            article.source_kind for article in request.completed_articles
        )
        return GraphProbeResult(
            schema_version=GRAPH_PROBE_SCHEMA_VERSION,
            corpus_id=request.corpus_id,
            total_catalog_records_seen=len(request.completed_articles)
            + len(request.excluded_records),
            corpus_size=len(request.completed_articles),
            skipped_metadata_only=len(request.excluded_records),
            completed_articles=tuple(request.completed_articles),
            excluded_records=tuple(request.excluded_records),
            chunk_count_total=sum(article.chunk_count for article in request.completed_articles),
            source_kind_counts=dict(sorted(source_kind_counts.items())),
            metrics=metrics,
            memory_profile=memory_profile,
            input_artifacts=tuple(request.input_artifacts),
            output_artifacts=tuple(output_artifacts),
            implementation=dict(implementation or {}),
            fail_closed_invariants=dict(FAIL_CLOSED_INVARIANTS),
            diagnostics=tuple(diagnostics),
        )
