"""NetworkX graph probe adapter for bounded corpus validation."""

from __future__ import annotations

import tracemalloc
from collections import defaultdict
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from research_graph.application.graph.probe import (
    GraphProbeArtifactRef,
    GraphProbeDiagnostic,
    GraphProbeExecutionResult,
    GraphProbeMemoryProfile,
    GraphProbeMetrics,
    GraphProbeRequest,
)
from research_graph.domain.ports import (
    ProjectionDiagnostic,
    ProjectionEdgeRef,
    ProjectionNodeRef,
    ProjectionRequest,
    ProjectionResult,
)

DEFAULT_ENTITY_TYPES = (
    "metadata",
    "table_context",
    "figure_caption_context",
    "citation_context",
    "retrieval_context",
)


class NetworkXProjectionAdapter:
    """Project candidate-packet metadata refs into an in-memory NetworkX graph."""

    def __init__(self, *, graph_library: Any | None = None) -> None:
        self._graph_library = graph_library

    def project(self, request: ProjectionRequest) -> ProjectionResult:
        try:
            nx = self._networkx()
            graph = nx.DiGraph(name=request.candidate_packet.candidate_id)
            node_refs = tuple(
                ProjectionNodeRef(ref=ref, node_type=_projection_ref_kind(ref, default="node"))
                for ref in request.candidate_packet.graph_node_refs
            )
            edge_refs = tuple(
                _projection_edge_ref(ref) for ref in request.candidate_packet.graph_edge_refs
            )
            for node in node_refs:
                graph.add_node(node.ref, node_type=node.node_type)
            for edge in edge_refs:
                graph.add_edge(edge.source_ref, edge.target_ref, edge_type=edge.edge_type, ref=edge.ref)
            return ProjectionResult(
                schema_version=request.schema_version,
                backend="networkx",
                node_refs=node_refs,
                edge_refs=edge_refs,
                evidence_refs=request.candidate_packet.evidence_refs,
                provenance_refs=request.candidate_packet.provenance_refs,
                diagnostics=(
                    ProjectionDiagnostic(code="networkx_projection_completed", phase="graph_projection"),
                ),
            )
        except Exception:
            return ProjectionResult(
                schema_version=request.schema_version,
                backend="networkx",
                diagnostics=(
                    ProjectionDiagnostic(code="networkx_projection_failed", phase="graph_projection"),
                ),
            )

    def _networkx(self) -> Any:
        if self._graph_library is not None:
            return self._graph_library
        import networkx as nx  # type: ignore[import-unresolved]

        return nx


class NetworkXGraphProbeAdapter:
    """Build an in-memory NetworkX probe graph and report bounded metrics."""

    def __init__(
        self,
        *,
        graphml_path: Path | None = None,
        include_citation_relations: bool = True,
        graph_library: Any | None = None,
        trace_module: Any = tracemalloc,
    ) -> None:
        self._graphml_path = graphml_path
        self._include_citation_relations = include_citation_relations
        self._graph_library = graph_library
        self._trace_module = trace_module

    def execute(self, request: GraphProbeRequest) -> GraphProbeExecutionResult:
        diagnostics: list[GraphProbeDiagnostic] = []
        nx = self._networkx(diagnostics)
        if nx is None:
            return GraphProbeExecutionResult(diagnostics=tuple(diagnostics))

        memory_started = self._start_memory_sampling(diagnostics)
        snap_before = self._take_snapshot(diagnostics, phase="memory_sampling_start")

        try:
            graph = self._build_graph(nx, request)
        except Exception as exc:
            self._stop_memory_sampling(memory_started)
            return GraphProbeExecutionResult(
                diagnostics=(
                    GraphProbeDiagnostic(
                        code="graph_construction_failed",
                        phase="graph_construction",
                        exception_class=exc.__class__.__name__,
                        notes="networkx graph construction failed",
                    ),
                )
            )

        output_artifacts: list[GraphProbeArtifactRef] = []
        if self._graphml_path is not None:
            try:
                self._graphml_path.parent.mkdir(parents=True, exist_ok=True)
                nx.write_graphml(graph, str(self._graphml_path))
                output_artifacts.append(
                    GraphProbeArtifactRef(
                        path=self._graphml_path.as_posix(),
                        artifact_type="graphml",
                    )
                )
            except Exception as exc:
                diagnostics.append(
                    GraphProbeDiagnostic(
                        code="graphml_write_failed",
                        phase="artifact_write",
                        exception_class=exc.__class__.__name__,
                        notes="networkx graphml write failed",
                    )
                )

        try:
            metrics = GraphProbeMetrics(
                n_nodes=graph.number_of_nodes(),
                n_edges=graph.number_of_edges(),
                node_types=_count_node_types(graph),
                edge_types=_count_edge_types(graph),
                citation_relations_count=int(
                    _count_edge_types(graph).get("article_cites_article", 0)
                ),
            )
        except Exception as exc:
            self._stop_memory_sampling(memory_started)
            return GraphProbeExecutionResult(
                diagnostics=(
                    *diagnostics,
                    GraphProbeDiagnostic(
                        code="metric_extraction_failed",
                        phase="metric_extraction",
                        exception_class=exc.__class__.__name__,
                        notes="networkx metric extraction failed",
                    ),
                )
            )

        memory_profile = self._memory_profile(
            metrics=metrics,
            memory_started=memory_started,
            snap_before=snap_before,
            diagnostics=diagnostics,
        )
        self._stop_memory_sampling(memory_started)

        return GraphProbeExecutionResult(
            metrics=metrics,
            memory_profile=memory_profile,
            implementation={
                "library": "networkx",
                "graph_type": "DiGraph",
                "in_memory_only": True,
                "no_db_connection": True,
                "no_network_io": True,
            },
            output_artifacts=tuple(output_artifacts),
            diagnostics=tuple(diagnostics),
        )

    def _networkx(self, diagnostics: list[GraphProbeDiagnostic]) -> Any | None:
        if self._graph_library is not None:
            return self._graph_library
        try:
            import networkx as nx  # type: ignore[import-unresolved]
        except ImportError as exc:
            diagnostics.append(
                GraphProbeDiagnostic(
                    code="networkx_unavailable",
                    phase="adapter_import",
                    exception_class=exc.__class__.__name__,
                    notes="networkx dependency is unavailable",
                )
            )
            return None
        return nx

    def _build_graph(self, nx: Any, request: GraphProbeRequest) -> Any:
        graph = nx.DiGraph(name=f"{request.corpus_id}-source-backed-probe")
        graph.add_node(
            f"corpus:{request.corpus_id}",
            node_type="corpus",
            label=f"{request.corpus_id} source-backed corpus probe",
            scale=len(request.completed_articles),
            skipped_metadata_only=len(request.excluded_records),
        )
        entity_types = tuple(request.entity_types)

        for article in request.completed_articles:
            article_node_id = f"article:{article.article_ref}"
            graph.add_node(
                article_node_id,
                node_type="article",
                article_ref=article.article_ref,
                article_key=article.article_key,
                source_kind=article.source_kind,
                text_source=article.text_source,
                chunk_count=article.chunk_count,
            )
            graph.add_edge(
                f"corpus:{request.corpus_id}",
                article_node_id,
                edge_type="corpus_contains_article",
            )

            for index in range(article.chunk_count):
                chunk_id = f"chunk:{article.article_ref}:{index + 1:04d}"
                graph.add_node(
                    chunk_id,
                    node_type="chunk",
                    article_ref=article.article_ref,
                    chunk_index=index + 1,
                )
                graph.add_edge(
                    article_node_id,
                    chunk_id,
                    edge_type="article_contains_chunk",
                )

            for entity_type in entity_types:
                entity_id = f"entity:{article.article_ref}:{entity_type}"
                graph.add_node(
                    entity_id,
                    node_type="entity",
                    entity_type=entity_type,
                    article_ref=article.article_ref,
                    source="m025_chunk_types",
                )
                graph.add_edge(article_node_id, entity_id, edge_type="article_has_entity")

        if self._include_citation_relations:
            for source_ref, target_ref, category in find_citation_relations(
                [article.article_ref for article in request.completed_articles]
            ):
                graph.add_edge(
                    f"article:{source_ref}",
                    f"article:{target_ref}",
                    edge_type="article_cites_article",
                    source=f"coarse_category:{category}",
                )
        return graph

    def _start_memory_sampling(self, diagnostics: list[GraphProbeDiagnostic]) -> bool:
        try:
            self._trace_module.start()
        except Exception as exc:
            diagnostics.append(
                GraphProbeDiagnostic(
                    code="memory_sampling_start_failed",
                    phase="memory_sampling",
                    severity="warning",
                    exception_class=exc.__class__.__name__,
                    notes="memory sampling could not start",
                )
            )
            return False
        return True

    def _take_snapshot(
        self,
        diagnostics: list[GraphProbeDiagnostic],
        *,
        phase: str,
    ) -> Any | None:
        try:
            return self._trace_module.take_snapshot()
        except Exception as exc:
            diagnostics.append(
                GraphProbeDiagnostic(
                    code="memory_snapshot_failed",
                    phase=phase,
                    severity="warning",
                    exception_class=exc.__class__.__name__,
                    notes="memory snapshot could not be captured",
                )
            )
            return None

    def _memory_profile(
        self,
        *,
        metrics: GraphProbeMetrics,
        memory_started: bool,
        snap_before: Any | None,
        diagnostics: list[GraphProbeDiagnostic],
    ) -> GraphProbeMemoryProfile | None:
        if not memory_started:
            return None
        try:
            current, peak = self._trace_module.get_traced_memory()
            snap_after = self._trace_module.take_snapshot()
        except Exception as exc:
            diagnostics.append(
                GraphProbeDiagnostic(
                    code="memory_sampling_failed",
                    phase="memory_sampling",
                    severity="warning",
                    exception_class=exc.__class__.__name__,
                    notes="memory metrics could not be captured",
                )
            )
            return None

        top_allocations: tuple[dict[str, object], ...] = ()
        if snap_before is not None:
            try:
                top_allocations = tuple(
                    {
                        "file": str(stat.traceback),
                        "size_diff_bytes": stat.size_diff,
                        "size_bytes": stat.size,
                    }
                    for stat in snap_after.compare_to(snap_before, "lineno")[:5]
                )
            except Exception as exc:
                diagnostics.append(
                    GraphProbeDiagnostic(
                        code="memory_top_allocations_failed",
                        phase="memory_sampling",
                        severity="warning",
                        exception_class=exc.__class__.__name__,
                        notes="memory top allocations could not be computed",
                    )
                )
        return GraphProbeMemoryProfile(
            method="tracemalloc",
            peak_mb=round(peak / (1024 * 1024), 4),
            current_mb=round(current / (1024 * 1024), 4),
            tracemalloc_current_bytes=current,
            tracemalloc_peak_bytes=peak,
            approx_bytes_per_node=current // max(1, metrics.n_nodes),
            n_nodes=metrics.n_nodes,
            n_edges=metrics.n_edges,
            top_allocations=top_allocations,
        )

    def _stop_memory_sampling(self, memory_started: bool) -> None:
        if not memory_started:
            return
        try:
            self._trace_module.stop()
        except Exception:
            return


def _projection_edge_ref(ref: str) -> ProjectionEdgeRef:
    body = ref.removeprefix("edge:")
    if "->" not in body:
        return ProjectionEdgeRef(
            ref=ref,
            edge_type="candidate_edge",
            source_ref="node:unknown:source",
            target_ref="node:unknown:target",
        )
    source, target = body.split("->", 1)
    source_ref = source if source.startswith("node:") else f"node:{source}"
    target_ref = target if target.startswith("node:") else f"node:{target}"
    return ProjectionEdgeRef(
        ref=ref,
        edge_type=f"{_projection_ref_kind(source_ref, default='source')}_to_{_projection_ref_kind(target_ref, default='target')}",
        source_ref=source_ref,
        target_ref=target_ref,
    )


def _projection_ref_kind(ref: str, *, default: str) -> str:
    parts = ref.split(":")
    if len(parts) >= 2 and parts[1]:
        return parts[1]
    return default


def find_citation_relations(article_refs: Sequence[str]) -> list[tuple[str, str, str]]:
    """Find coarse article_cites_article relations grouped by source/category."""

    relations: list[tuple[str, str, str]] = []
    by_category: dict[str, list[str]] = defaultdict(list)
    for article_ref in article_refs:
        by_category[_coarse_category(article_ref)].append(article_ref)

    seen: set[tuple[str, str]] = set()
    for category, refs in sorted(by_category.items()):
        if len(refs) < 2:
            continue
        for index, source_ref in enumerate(sorted(refs)):
            for target_ref in sorted(refs)[index + 1 :]:
                pair = (
                    (source_ref, target_ref)
                    if source_ref < target_ref
                    else (target_ref, source_ref)
                )
                if pair in seen:
                    continue
                seen.add(pair)
                relations.append((source_ref, target_ref, category))
    return relations


def _coarse_category(article_ref: str) -> str:
    parts = article_ref.split("/")
    if len(parts) >= 2 and parts[0] in {"arxiv", "company_blog", "nature"}:
        return parts[1]
    if len(parts) >= 2:
        return parts[0]
    return "other"


def _count_node_types(graph: Any) -> dict[str, int]:
    node_types: dict[str, int] = {}
    for _, data in graph.nodes(data=True):
        node_type = str(data.get("node_type", "unknown")) if isinstance(data, dict) else "unknown"
        node_types[node_type] = node_types.get(node_type, 0) + 1
    return node_types


def _count_edge_types(graph: Any) -> dict[str, int]:
    edge_types: dict[str, int] = {}
    for _, _, data in graph.edges(data=True):
        edge_type = str(data.get("edge_type", "unknown")) if isinstance(data, dict) else "unknown"
        edge_types[edge_type] = edge_types.get(edge_type, 0) + 1
    return edge_types
