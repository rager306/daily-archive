# Formerly: src/arxiv_archive/chunking_benchmark.py

"""Redacted chunking benchmark contract for M005/S06.

The benchmark compares chunking method diagnostics without serializing raw paper
text, chunk text, embeddings, vectors, optimizer traces, or production write
state. It is a dry-run artifact model only.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "m005-chunking-benchmark.v1"

FORBIDDEN_RAW_FIELDS = frozenset({"text", "raw_text", "chunk_text", "paper_text", "claim_text"})
FORBIDDEN_EMBEDDING_FIELDS = frozenset({"embedding", "embeddings"})
FORBIDDEN_VECTOR_FIELDS = frozenset({"vector", "vectors"})
FORBIDDEN_SECRET_FIELDS = frozenset(
    {"secret", "secrets", "token", "tokens", "api_key", "credentials"}
)
FORBIDDEN_OPTIMIZER_FIELDS = frozenset({"optimizer_trace", "optimizer_traces"})


def _safety_flags() -> dict[str, bool]:
    return {
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "secrets_included": False,
        "optimizer_traces_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


@dataclass(frozen=True)
class BenchmarkDiagnostic:
    """One redacted benchmark validation diagnostic."""

    reason: str
    object_id: str | None = None
    object_type: str | None = None
    blocks_recommendation: bool = True


@dataclass(frozen=True)
class BenchmarkValidationResult:
    """Validation result for a benchmark artifact."""

    valid_benchmark: bool
    diagnostics: tuple[BenchmarkDiagnostic, ...]

    @property
    def passed(self) -> bool:
        return self.valid_benchmark and not self.diagnostics

    @property
    def refusal_counts(self) -> dict[str, int]:
        counts: dict[str, int] = {}
        for diagnostic in self.diagnostics:
            counts[diagnostic.reason] = counts.get(diagnostic.reason, 0) + 1
        return dict(sorted(counts.items()))


@dataclass(frozen=True)
class MethodMetrics:
    """Aggregate metrics for one chunking method."""

    method_id: str
    paper_count: int
    chunk_count: int
    import_eligible_chunk_count: int = 0
    refused_chunk_count: int = 0
    counts_by_route: dict[str, int] = field(default_factory=dict)
    counts_by_chunk_type: dict[str, int] = field(default_factory=dict)
    counts_by_state: dict[str, int] = field(default_factory=dict)
    refusal_counts: dict[str, int] = field(default_factory=dict)
    source_span_coverage: float = 0.0
    parent_reference_resolution_rate: float = 0.0
    annotation_coverage_rate: float = 0.0
    asset_linkage_coverage_rate: float = 0.0
    missing_source_counts: dict[str, int] = field(default_factory=dict)
    caveats: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        return {
            "method_id": self.method_id,
            "paper_count": self.paper_count,
            "chunk_count": self.chunk_count,
            "import_eligible_chunk_count": self.import_eligible_chunk_count,
            "refused_chunk_count": self.refused_chunk_count,
            "counts_by_route": dict(sorted(self.counts_by_route.items())),
            "counts_by_chunk_type": dict(sorted(self.counts_by_chunk_type.items())),
            "counts_by_state": dict(sorted(self.counts_by_state.items())),
            "refusal_counts": dict(sorted(self.refusal_counts.items())),
            "source_span_coverage": self.source_span_coverage,
            "parent_reference_resolution_rate": self.parent_reference_resolution_rate,
            "annotation_coverage_rate": self.annotation_coverage_rate,
            "asset_linkage_coverage_rate": self.asset_linkage_coverage_rate,
            "missing_source_counts": dict(sorted(self.missing_source_counts.items())),
            "caveats": list(self.caveats),
            **_safety_flags(),
        }


@dataclass(frozen=True)
class PaperMethodMetrics:
    """Per-paper benchmark metrics for one method."""

    paper_id: str
    method_id: str
    chunk_count: int
    import_eligible_chunk_count: int = 0
    refused_chunk_count: int = 0
    asset_count: int = 0
    counts_by_route: dict[str, int] = field(default_factory=dict)
    counts_by_chunk_type: dict[str, int] = field(default_factory=dict)
    counts_by_state: dict[str, int] = field(default_factory=dict)
    refusal_counts: dict[str, int] = field(default_factory=dict)
    source_span_coverage: float = 0.0
    annotation_coverage_rate: float = 0.0
    asset_linkage_coverage_rate: float = 0.0
    caveats: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        return {
            "paper_id": self.paper_id,
            "method_id": self.method_id,
            "chunk_count": self.chunk_count,
            "import_eligible_chunk_count": self.import_eligible_chunk_count,
            "refused_chunk_count": self.refused_chunk_count,
            "asset_count": self.asset_count,
            "counts_by_route": dict(sorted(self.counts_by_route.items())),
            "counts_by_chunk_type": dict(sorted(self.counts_by_chunk_type.items())),
            "counts_by_state": dict(sorted(self.counts_by_state.items())),
            "refusal_counts": dict(sorted(self.refusal_counts.items())),
            "source_span_coverage": self.source_span_coverage,
            "annotation_coverage_rate": self.annotation_coverage_rate,
            "asset_linkage_coverage_rate": self.asset_linkage_coverage_rate,
            "caveats": list(self.caveats),
            **_safety_flags(),
        }


@dataclass(frozen=True)
class ChunkingBenchmark:
    """Redacted benchmark artifact comparing chunking methods."""

    input_corpus: str
    methods: tuple[MethodMetrics, ...]
    per_paper: tuple[PaperMethodMetrics, ...] = ()
    recommendation_status: str = "review_required"
    caveats: tuple[str, ...] = ()

    def to_contract(self) -> dict[str, Any]:
        method_records = [method.to_contract() for method in self.methods]
        paper_records = [paper.to_contract() for paper in self.per_paper]
        return {
            "schema_version": SCHEMA_VERSION,
            "input_corpus": self.input_corpus,
            "method_count": len(method_records),
            "methods": method_records,
            "per_paper": paper_records,
            "aggregate": aggregate_method_metrics(method_records),
            "recommendation_status": self.recommendation_status,
            "caveats": list(self.caveats),
            **_safety_flags(),
        }


def write_chunking_benchmark_run(benchmark: ChunkingBenchmark, output_dir: Path) -> dict[str, Any]:
    """Write redacted benchmark summary and method diagnostics."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    contract = benchmark.to_contract()
    summary = {key: value for key, value in contract.items() if key not in {"methods", "per_paper"}}
    (output_dir / "chunking-benchmark-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    records = [_method_diagnostic_record(method) for method in contract["methods"]]
    (output_dir / "chunking-benchmark-diagnostics.jsonl").write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    return summary


def _method_diagnostic_record(method: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": "m005-chunking-benchmark-method-diagnostic.v1",
        "method_id": method.get("method_id"),
        "paper_count": method.get("paper_count"),
        "chunk_count": method.get("chunk_count"),
        "import_eligible_chunk_count": method.get("import_eligible_chunk_count"),
        "refused_chunk_count": method.get("refused_chunk_count"),
        "counts_by_route": method.get("counts_by_route", {}),
        "counts_by_chunk_type": method.get("counts_by_chunk_type", {}),
        "counts_by_state": method.get("counts_by_state", {}),
        "refusal_counts": method.get("refusal_counts", {}),
        "source_span_coverage": method.get("source_span_coverage"),
        "parent_reference_resolution_rate": method.get("parent_reference_resolution_rate"),
        "annotation_coverage_rate": method.get("annotation_coverage_rate"),
        "asset_linkage_coverage_rate": method.get("asset_linkage_coverage_rate"),
        "missing_source_counts": method.get("missing_source_counts", {}),
        "caveats": method.get("caveats", []),
        **_safety_flags(),
    }


def method_from_baseline_summary(summary: dict[str, Any]) -> MethodMetrics:
    """Create benchmark metrics from the S02 baseline summary."""
    chunk_count = _int(summary.get("refused_chunk_count")) or _int(summary.get("chunk_count"))
    return MethodMetrics(
        method_id="baseline_pageindex_semanticchunk",
        paper_count=_int(summary.get("paper_count")),
        chunk_count=chunk_count,
        import_eligible_chunk_count=_int(summary.get("import_eligible_chunk_count")),
        refused_chunk_count=_int(summary.get("refused_chunk_count")),
        counts_by_route=_dict_ints(summary.get("counts_by_route")),
        counts_by_chunk_type=_dict_ints(summary.get("counts_by_chunk_type")),
        counts_by_state=_dict_ints(summary.get("counts_by_state")),
        refusal_counts=_dict_ints(summary.get("refusal_counts")),
        source_span_coverage=0.0,
        parent_reference_resolution_rate=0.0,
        annotation_coverage_rate=0.0,
        asset_linkage_coverage_rate=0.0,
        caveats=("baseline_retrieval_only_not_import_ready", "no_annotation_or_asset_linkage"),
    )


def method_from_structure_aware_summary(
    summary: dict[str, Any],
    *,
    annotation_summary: dict[str, Any] | None = None,
    source_asset_summary: dict[str, Any] | None = None,
) -> MethodMetrics:
    """Create benchmark metrics from S03/S04/S05 structure-aware evidence."""
    chunk_count = _int(summary.get("chunk_count"))
    annotation_count = _int((annotation_summary or {}).get("annotated_chunk_count"))
    asset_count = _int((source_asset_summary or {}).get("asset_count"))
    return MethodMetrics(
        method_id="structure_aware_control",
        paper_count=_int(summary.get("paper_count")),
        chunk_count=chunk_count,
        import_eligible_chunk_count=_int(summary.get("import_eligible_chunk_count")),
        refused_chunk_count=_int(summary.get("refused_chunk_count")),
        counts_by_route=_dict_ints(summary.get("counts_by_route")),
        counts_by_chunk_type=_dict_ints(summary.get("counts_by_chunk_type")),
        counts_by_state=_dict_ints(summary.get("counts_by_state")),
        refusal_counts=_dict_ints(summary.get("refusal_counts")),
        source_span_coverage=1.0 if chunk_count else 0.0,
        parent_reference_resolution_rate=1.0 if chunk_count else 0.0,
        annotation_coverage_rate=annotation_count / chunk_count if chunk_count else 0.0,
        asset_linkage_coverage_rate=asset_count / chunk_count if chunk_count else 0.0,
        missing_source_counts=_dict_ints((source_asset_summary or {}).get("missing_counts")),
        caveats=("control_chunker_not_final_algorithm", "all_chunks_remain_import_blocked"),
    )


def method_from_simple_section_window(source_asset_summary: dict[str, Any]) -> MethodMetrics:
    """Create a bounded deterministic candidate estimate from S05 source/asset manifests."""
    asset_counts = _dict_ints(source_asset_summary.get("asset_counts_by_type"))
    source_file_count = _int(source_asset_summary.get("source_file_count"))
    asset_count = sum(asset_counts.values())
    chunk_count = asset_count + source_file_count
    counts_by_route = {
        "retrieval_only": source_file_count,
        "table_extraction": asset_counts.get("table", 0),
        "citation_graph": asset_counts.get("reference", 0),
        "metadata_graph": asset_counts.get("metadata", 0),
    }
    figure_equation_count = asset_counts.get("figure", 0) + asset_counts.get("equation", 0)
    if figure_equation_count:
        counts_by_route["retrieval_only"] = (
            counts_by_route.get("retrieval_only", 0) + figure_equation_count
        )
    return MethodMetrics(
        method_id="simple_section_window_estimate",
        paper_count=_int(source_asset_summary.get("paper_count")),
        chunk_count=chunk_count,
        import_eligible_chunk_count=0,
        refused_chunk_count=chunk_count,
        counts_by_route={key: value for key, value in counts_by_route.items() if value},
        counts_by_chunk_type={
            "retrieval_context": source_file_count + figure_equation_count,
            "table_context": asset_counts.get("table", 0),
            "reference_entry": asset_counts.get("reference", 0),
            "metadata": asset_counts.get("metadata", 0),
        },
        counts_by_state={
            "repair_required": asset_count,
            "ok_for_retrieval_only": source_file_count,
        },
        refusal_counts={"estimated_candidate_requires_review": chunk_count},
        source_span_coverage=1.0 if chunk_count else 0.0,
        parent_reference_resolution_rate=0.0,
        annotation_coverage_rate=0.0,
        asset_linkage_coverage_rate=asset_count / chunk_count if chunk_count else 0.0,
        missing_source_counts=_dict_ints(source_asset_summary.get("missing_counts")),
        caveats=(
            "estimated_candidate_not_real_chunker",
            "uses_s05_asset_links_without_raw_text_serialization",
            "chonkie_llamaindex_langchain_not_executed",
        ),
    )


def build_benchmark_from_artifacts(
    *,
    input_corpus: str,
    baseline_summary_path: Path,
    structure_summary_path: Path,
    annotation_summary_path: Path,
    source_asset_summary_path: Path,
) -> ChunkingBenchmark:
    """Build a benchmark object from prior redacted run summaries."""
    baseline_summary = _load_json(baseline_summary_path)
    structure_summary = _load_json(structure_summary_path)
    annotation_summary = _load_json(annotation_summary_path)
    source_asset_summary = _load_json(source_asset_summary_path)
    return ChunkingBenchmark(
        input_corpus=input_corpus,
        methods=(
            method_from_baseline_summary(baseline_summary),
            method_from_structure_aware_summary(
                structure_summary,
                annotation_summary=annotation_summary,
                source_asset_summary=source_asset_summary,
            ),
            method_from_simple_section_window(source_asset_summary),
        ),
        recommendation_status="review_required",
        caveats=(
            "dry_run_only",
            "real_library_candidates_not_executed",
            "production_import_blocked",
        ),
    )


def aggregate_method_metrics(method_records: list[dict[str, Any]]) -> dict[str, Any]:
    """Aggregate method metrics for a run-level summary."""
    chunk_count = sum(_int(record.get("chunk_count")) for record in method_records)
    import_eligible = sum(
        _int(record.get("import_eligible_chunk_count")) for record in method_records
    )
    refused = sum(_int(record.get("refused_chunk_count")) for record in method_records)
    return {
        "total_chunk_count": chunk_count,
        "total_import_eligible_chunk_count": import_eligible,
        "total_refused_chunk_count": refused,
        "counts_by_route": _merge_counts(
            record.get("counts_by_route") for record in method_records
        ),
        "counts_by_chunk_type": _merge_counts(
            record.get("counts_by_chunk_type") for record in method_records
        ),
        "counts_by_state": _merge_counts(
            record.get("counts_by_state") for record in method_records
        ),
        "refusal_counts": _merge_counts(record.get("refusal_counts") for record in method_records),
        "missing_source_counts": _merge_counts(
            record.get("missing_source_counts") for record in method_records
        ),
        "method_ids": sorted(str(record.get("method_id")) for record in method_records),
    }


def validate_chunking_benchmark(benchmark: dict[str, Any]) -> BenchmarkValidationResult:
    """Validate benchmark artifact structure and redaction boundaries."""
    diagnostics: list[BenchmarkDiagnostic] = []
    diagnostics.extend(
        _required_fields(
            benchmark,
            fields=("schema_version", "input_corpus", "methods", "aggregate"),
            object_id=None,
            object_type="benchmark",
        )
    )
    diagnostics.extend(
        _validate_redaction(benchmark, object_id="benchmark", object_type="benchmark")
    )
    if benchmark.get("schema_version") != SCHEMA_VERSION:
        diagnostics.append(
            BenchmarkDiagnostic(reason="schema_version_mismatch", object_type="benchmark")
        )
    methods = _list_of_dicts(benchmark.get("methods"))
    if benchmark.get("method_count") != len(methods):
        diagnostics.append(
            BenchmarkDiagnostic(reason="method_count_mismatch", object_type="benchmark")
        )
    for method in methods:
        diagnostics.extend(_validate_method(method))
    for paper in _list_of_dicts(benchmark.get("per_paper")):
        diagnostics.extend(_validate_paper_method(paper))
    for field_name, expected in _safety_flags().items():
        if benchmark.get(field_name) is not expected:
            diagnostics.append(
                BenchmarkDiagnostic(
                    reason=f"unsafe_{field_name}", object_id="benchmark", object_type="benchmark"
                )
            )
    return BenchmarkValidationResult(
        valid_benchmark=not diagnostics, diagnostics=tuple(diagnostics)
    )


def _validate_method(method: dict[str, Any]) -> list[BenchmarkDiagnostic]:
    method_id = _string_or_none(method.get("method_id"))
    diagnostics = _required_fields(
        method,
        fields=(
            "method_id",
            "paper_count",
            "chunk_count",
            "import_eligible_chunk_count",
            "refused_chunk_count",
        ),
        object_id=method_id,
        object_type="method",
    )
    diagnostics.extend(_validate_metric_ranges(method, object_id=method_id, object_type="method"))
    diagnostics.extend(_validate_safety_flags(method, object_id=method_id, object_type="method"))
    diagnostics.extend(_validate_redaction(method, object_id=method_id, object_type="method"))
    return diagnostics


def _validate_paper_method(paper: dict[str, Any]) -> list[BenchmarkDiagnostic]:
    object_id = f"{paper.get('paper_id')}:{paper.get('method_id')}"
    diagnostics = _required_fields(
        paper,
        fields=(
            "paper_id",
            "method_id",
            "chunk_count",
            "import_eligible_chunk_count",
            "refused_chunk_count",
        ),
        object_id=object_id,
        object_type="paper_method",
    )
    diagnostics.extend(
        _validate_metric_ranges(paper, object_id=object_id, object_type="paper_method")
    )
    diagnostics.extend(
        _validate_safety_flags(paper, object_id=object_id, object_type="paper_method")
    )
    diagnostics.extend(_validate_redaction(paper, object_id=object_id, object_type="paper_method"))
    return diagnostics


def _validate_safety_flags(
    payload: dict[str, Any], *, object_id: str | None, object_type: str
) -> list[BenchmarkDiagnostic]:
    diagnostics: list[BenchmarkDiagnostic] = []
    for field_name, expected in _safety_flags().items():
        if field_name in payload and payload.get(field_name) is not expected:
            diagnostics.append(
                BenchmarkDiagnostic(
                    reason=f"unsafe_{field_name}", object_id=object_id, object_type=object_type
                )
            )
    return diagnostics


def _validate_metric_ranges(
    payload: dict[str, Any], *, object_id: str | None, object_type: str
) -> list[BenchmarkDiagnostic]:
    diagnostics: list[BenchmarkDiagnostic] = []
    for field_name in (
        "source_span_coverage",
        "parent_reference_resolution_rate",
        "annotation_coverage_rate",
        "asset_linkage_coverage_rate",
    ):
        if field_name not in payload:
            continue
        value = payload.get(field_name)
        if not isinstance(value, int | float) or value < 0 or value > 1:
            diagnostics.append(
                BenchmarkDiagnostic(
                    reason=f"invalid_{field_name}", object_id=object_id, object_type=object_type
                )
            )
    for field_name in ("chunk_count", "import_eligible_chunk_count", "refused_chunk_count"):
        if field_name in payload and _int(payload.get(field_name)) < 0:
            diagnostics.append(
                BenchmarkDiagnostic(
                    reason=f"invalid_{field_name}", object_id=object_id, object_type=object_type
                )
            )
    return diagnostics


def _validate_redaction(
    payload: Any, *, object_id: str | None, object_type: str
) -> list[BenchmarkDiagnostic]:
    return _validate_nested_redaction(
        payload, object_id=object_id, object_type=object_type, path=()
    )


def _validate_nested_redaction(
    value: Any, *, object_id: str | None, object_type: str, path: tuple[str, ...]
) -> list[BenchmarkDiagnostic]:
    diagnostics: list[BenchmarkDiagnostic] = []
    if isinstance(value, dict):
        forbidden = (
            FORBIDDEN_RAW_FIELDS
            | FORBIDDEN_EMBEDDING_FIELDS
            | FORBIDDEN_VECTOR_FIELDS
            | FORBIDDEN_SECRET_FIELDS
            | FORBIDDEN_OPTIMIZER_FIELDS
        ) & set(value)
        for field_name in sorted(forbidden):
            if field_name in FORBIDDEN_RAW_FIELDS:
                reason = "raw_text_leakage"
            elif field_name in FORBIDDEN_EMBEDDING_FIELDS:
                reason = "embedding_leakage"
            elif field_name in FORBIDDEN_VECTOR_FIELDS:
                reason = "vector_leakage"
            elif field_name in FORBIDDEN_SECRET_FIELDS:
                reason = "secret_leakage"
            else:
                reason = "optimizer_trace_leakage"
            diagnostics.append(
                BenchmarkDiagnostic(
                    reason=reason,
                    object_id=_redaction_path(
                        object_id=object_id, object_type=object_type, path=(*path, str(field_name))
                    ),
                    object_type=object_type,
                )
            )
        for key, nested_value in value.items():
            diagnostics.extend(
                _validate_nested_redaction(
                    nested_value,
                    object_id=object_id,
                    object_type=object_type,
                    path=(*path, str(key)),
                )
            )
    elif isinstance(value, list):
        for index, nested_value in enumerate(value):
            diagnostics.extend(
                _validate_nested_redaction(
                    nested_value,
                    object_id=object_id,
                    object_type=object_type,
                    path=(*path, str(index)),
                )
            )
    return diagnostics


def _required_fields(
    payload: dict[str, Any], *, fields: tuple[str, ...], object_id: str | None, object_type: str
) -> list[BenchmarkDiagnostic]:
    diagnostics: list[BenchmarkDiagnostic] = []
    for field_name in fields:
        if field_name not in payload or payload.get(field_name) is None:
            diagnostics.append(
                BenchmarkDiagnostic(
                    reason=f"missing_{field_name}", object_id=object_id, object_type=object_type
                )
            )
    return diagnostics


def _merge_counts(sources: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for source in sources:
        if not isinstance(source, dict):
            continue
        for key, value in source.items():
            counts[str(key)] = counts.get(str(key), 0) + _int(value)
    return dict(sorted(counts.items()))


def _dict_ints(value: Any) -> dict[str, int]:
    if not isinstance(value, dict):
        return {}
    return {str(key): _int(raw_value) for key, raw_value in sorted(value.items())}


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(Path(path).read_text(encoding="utf-8"))
    return value if isinstance(value, dict) else {}


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _redaction_path(*, object_id: str | None, object_type: str, path: tuple[str, ...]) -> str:
    prefix = object_id or object_type
    return f"{prefix}:{'.'.join(path)}"


def _string_or_none(value: Any) -> str | None:
    return str(value) if value is not None else None


def _int(value: Any) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0
