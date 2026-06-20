# Formerly: src/arxiv_archive/evaluation.py

"""Pure evaluation contracts for deterministic scientific KG benchmarks.

This module is intentionally local-only: it validates typed extraction patches and
computes fixture-scale metrics over IDs. Diagnostics are text-safe and avoid
persisting paper body text, chunk text, embeddings, secrets, or credentials.
"""

from __future__ import annotations

from collections import Counter
from collections.abc import Iterable, Mapping
from dataclasses import dataclass, field
from typing import Any, cast

import ladybug

from research_graph.evaluation.scientific_extraction import (
    ExtractionPatch,
    validate_extraction_patch,
)
from research_graph.graph.ladybug_client import evidence_path_id
from research_graph.retrieval.hybrid import (
    HybridRetrievalMode,
    HybridRetrievalQuery,
    InMemoryVectorCandidateIndex,
    retrieve_hybrid,
)


@dataclass(frozen=True)
class BenchmarkRetrievalQuestion:
    """Expected retrieval behavior for one deterministic benchmark question."""

    name: str
    query: str
    query_vector: tuple[float, ...] | None = None
    expected_result_ids: set[str] = field(default_factory=set)
    expected_evidence_path_ids: set[str] = field(default_factory=set)
    retrieval_mode: str | None = None


@dataclass(frozen=True)
class RetrievalAblationResult:
    """Per-mode retrieval benchmark metrics and text-safe diagnostics."""

    question_id: str
    mode: HybridRetrievalMode
    top_k: int
    returned_semantic_chunk_ids: list[str]
    returned_evidence_path_ids: list[str]
    evidence_path_hit_rate: float
    retrieval_recall: float
    missing_expected_evidence_path_ids: list[str]
    missing_expected_result_ids: list[str]
    s06_diagnostics: dict[str, Any]


@dataclass(frozen=True)
class ExtractionBenchmarkFixture:
    """Expected extraction behavior for one deterministic fixture patch."""

    name: str
    patch: ExtractionPatch
    expected_claim_ids: list[str] = field(default_factory=list)
    expected_entity_ids: list[str] = field(default_factory=list)
    expected_relation_ids: list[str] = field(default_factory=list)
    expected_evidence_path_ids: set[str] = field(default_factory=set)


@dataclass(frozen=True)
class SchemaValidityResult:
    """Schema validation diagnostics for an extraction patch or fixture."""

    valid: bool
    diagnostics: list[str]
    diagnostic_count: int
    claim_count: int
    entity_count: int
    relation_count: int


@dataclass(frozen=True)
class GroundednessProxyResult:
    """ID-only proxy diagnostics for evidence-backed extraction drafts."""

    claim_count: int
    entity_count: int
    relation_count: int
    evidence_backed_claim_count: int
    evidence_backed_entity_count: int
    evidence_backed_relation_count: int
    derived_evidence_path_ids: list[str]
    missing_expected_evidence_path_ids: list[str]
    unexpected_evidence_path_ids: list[str]
    missing_evidence_path_draft_ids: list[str]


@dataclass(frozen=True)
class EvidencePathHitRateResult:
    """Evidence-path hit-rate metric and ID-only diagnostics."""

    hit_rate: float
    result_count: int
    none_evidence_path_count: int
    returned_evidence_path_ids: list[str]
    hit_evidence_path_ids: list[str]
    missing_expected_evidence_path_ids: list[str]
    unexpected_evidence_path_ids: list[str]
    duplicate_evidence_path_ids: list[str]


@dataclass(frozen=True)
class RetrievalRecallResult:
    """Retrieval recall metric and ID-only diagnostics."""

    recall: float
    result_count: int
    none_result_id_count: int
    returned_result_ids: list[str]
    matched_expected_result_ids: list[str]
    missing_expected_result_ids: list[str]
    unexpected_result_ids: list[str]
    duplicate_result_ids: list[str]


def evaluate_schema_validity(
    patch: ExtractionPatch | ExtractionBenchmarkFixture,
) -> SchemaValidityResult:
    """Validate a patch or fixture using the S04 extraction schema validator."""
    extraction_patch = _coerce_patch(patch)
    diagnostics = validate_extraction_patch(extraction_patch)
    return SchemaValidityResult(
        valid=not diagnostics,
        diagnostics=diagnostics,
        diagnostic_count=len(diagnostics),
        claim_count=len(extraction_patch.claims),
        entity_count=len(extraction_patch.entities),
        relation_count=len(extraction_patch.relations),
    )


def evaluate_groundedness_proxy(
    patch: ExtractionPatch | ExtractionBenchmarkFixture,
    expected_evidence_path_ids: Iterable[str] | None = None,
) -> GroundednessProxyResult:
    """Compare draft evidence paths with expected evidence IDs without text payloads."""
    extraction_patch = _coerce_patch(patch)
    expected_ids = _expected_evidence_ids(patch, expected_evidence_path_ids)

    claim_evidence_ids, missing_claim_ids = _draft_evidence_ids(extraction_patch.claims)
    entity_evidence_ids, missing_entity_ids = _draft_evidence_ids(extraction_patch.entities)
    relation_evidence_ids, missing_relation_ids = _draft_evidence_ids(extraction_patch.relations)

    derived_ids = set(claim_evidence_ids) | set(entity_evidence_ids) | set(relation_evidence_ids)
    missing_draft_ids = [*missing_claim_ids, *missing_entity_ids, *missing_relation_ids]

    return GroundednessProxyResult(
        claim_count=len(extraction_patch.claims),
        entity_count=len(extraction_patch.entities),
        relation_count=len(extraction_patch.relations),
        evidence_backed_claim_count=len(claim_evidence_ids),
        evidence_backed_entity_count=len(entity_evidence_ids),
        evidence_backed_relation_count=len(relation_evidence_ids),
        derived_evidence_path_ids=sorted(derived_ids),
        missing_expected_evidence_path_ids=sorted(expected_ids - derived_ids),
        unexpected_evidence_path_ids=sorted(derived_ids - expected_ids),
        missing_evidence_path_draft_ids=missing_draft_ids,
    )


def calculate_evidence_path_hit_rate(
    results: Iterable[Mapping[str, Any] | object],
    expected_evidence_path_ids: Iterable[str],
) -> EvidencePathHitRateResult:
    """Compute unique evidence ID hit rate over result rows.

    Empty expected sets are vacuously satisfied only when no non-null evidence
    IDs are returned. Missing or ``None`` evidence IDs are ignored for scoring
    and reported separately.
    """
    rows = list(results)
    expected_ids = set(expected_evidence_path_ids)
    ids, none_count = _extract_non_null_ids(rows, "evidence_path_id")
    id_counts = Counter(ids)
    returned_ids = set(ids)
    hit_ids = returned_ids & expected_ids

    if not expected_ids:
        hit_rate = 1.0 if not returned_ids else 0.0
    else:
        hit_rate = len(hit_ids) / len(expected_ids)

    return EvidencePathHitRateResult(
        hit_rate=hit_rate,
        result_count=len(rows),
        none_evidence_path_count=none_count,
        returned_evidence_path_ids=sorted(returned_ids),
        hit_evidence_path_ids=sorted(hit_ids),
        missing_expected_evidence_path_ids=sorted(expected_ids - returned_ids),
        unexpected_evidence_path_ids=sorted(returned_ids - expected_ids),
        duplicate_evidence_path_ids=sorted(id_ for id_, count in id_counts.items() if count > 1),
    )


def evaluate_evidence_path_hit_rate(
    results: Iterable[Mapping[str, Any] | object],
    expected_evidence_path_ids: Iterable[str],
) -> EvidencePathHitRateResult:
    """Alias for the evidence-path hit-rate public contract."""
    return calculate_evidence_path_hit_rate(results, expected_evidence_path_ids)


def calculate_retrieval_recall(
    results: Iterable[Mapping[str, Any] | object],
    expected_result_ids: Iterable[str],
    *,
    result_id_field: str = "semantic_chunk_id",
) -> RetrievalRecallResult:
    """Compute unique-ID retrieval recall over mapping or object result rows."""
    rows = list(results)
    expected_ids = set(expected_result_ids)
    ids, none_count = _extract_non_null_ids(rows, result_id_field)
    id_counts = Counter(ids)
    returned_ids = set(ids)
    matched_ids = returned_ids & expected_ids

    if not expected_ids:
        recall = 1.0 if not returned_ids else 0.0
    else:
        recall = len(matched_ids) / len(expected_ids)

    return RetrievalRecallResult(
        recall=recall,
        result_count=len(rows),
        none_result_id_count=none_count,
        returned_result_ids=sorted(returned_ids),
        matched_expected_result_ids=sorted(matched_ids),
        missing_expected_result_ids=sorted(expected_ids - returned_ids),
        unexpected_result_ids=sorted(returned_ids - expected_ids),
        duplicate_result_ids=sorted(id_ for id_, count in id_counts.items() if count > 1),
    )


def evaluate_retrieval_recall(
    results: Iterable[Mapping[str, Any] | object],
    expected_semantic_chunk_ids: Iterable[str],
) -> RetrievalRecallResult:
    """Alias for semantic-chunk retrieval recall."""
    return calculate_retrieval_recall(
        results,
        expected_result_ids=expected_semantic_chunk_ids,
        result_id_field="semantic_chunk_id",
    )


def run_retrieval_ablations(
    conn: ladybug.Connection,
    questions: Iterable[BenchmarkRetrievalQuestion],
    vector_index: InMemoryVectorCandidateIndex,
    modes: Iterable[HybridRetrievalMode] = (
        HybridRetrievalMode.VECTOR_ONLY,
        HybridRetrievalMode.GRAPH_ONLY,
        HybridRetrievalMode.HYBRID,
    ),
    top_k: int = 10,
) -> list[RetrievalAblationResult]:
    """Run deterministic S06 retrieval modes and score ID-only fixture metrics."""
    results: list[RetrievalAblationResult] = []
    for question in questions:
        for mode in modes:
            response = retrieve_hybrid(
                conn,
                HybridRetrievalQuery(
                    text=question.query,
                    vector=question.query_vector,
                    mode=mode,
                    limit=top_k,
                ),
                vector_index=vector_index,
            )
            hit_rate = calculate_evidence_path_hit_rate(
                response.results,
                question.expected_evidence_path_ids,
            )
            recall = calculate_retrieval_recall(
                response.results,
                question.expected_result_ids,
                result_id_field="semantic_chunk_id",
            )
            results.append(
                RetrievalAblationResult(
                    question_id=question.name,
                    mode=mode,
                    top_k=top_k,
                    returned_semantic_chunk_ids=recall.returned_result_ids,
                    returned_evidence_path_ids=hit_rate.returned_evidence_path_ids,
                    evidence_path_hit_rate=hit_rate.hit_rate,
                    retrieval_recall=recall.recall,
                    missing_expected_evidence_path_ids=hit_rate.missing_expected_evidence_path_ids,
                    missing_expected_result_ids=recall.missing_expected_result_ids,
                    s06_diagnostics=dict(response.diagnostics),
                )
            )
    return results


def _coerce_patch(value: ExtractionPatch | ExtractionBenchmarkFixture) -> ExtractionPatch:
    if isinstance(value, ExtractionBenchmarkFixture):
        return value.patch
    return value


def _expected_evidence_ids(
    value: ExtractionPatch | ExtractionBenchmarkFixture,
    explicit_expected_ids: Iterable[str] | None,
) -> set[str]:
    if explicit_expected_ids is not None:
        return set(explicit_expected_ids)
    if isinstance(value, ExtractionBenchmarkFixture):
        return set(value.expected_evidence_path_ids)
    return set()


def _draft_id(draft: Any) -> str:
    """Resolve the identifier of a typed draft (Claim/TypedEntity/TypedRelation)."""
    for attr in ("claim_id", "entity_id", "relation_id", "abstract_id", "id"):
        value = getattr(draft, attr, None)
        if value is not None:
            return str(value)
    return str(draft)


def _draft_evidence_ids(drafts: Iterable[Any]) -> tuple[list[str], list[str]]:
    evidence_ids: list[str] = []
    missing_draft_ids: list[str] = []
    for draft in drafts:
        path = draft.evidence_path
        if path is None:
            missing_draft_ids.append(_draft_id(draft))
            continue
        evidence_ids.append(evidence_path_id(path))
    return evidence_ids, missing_draft_ids


def _extract_non_null_ids(
    rows: Iterable[Mapping[str, Any] | object], field_name: str
) -> tuple[list[str], int]:
    ids: list[str] = []
    none_count = 0
    for row in rows:
        value = _row_value(row, field_name)
        if value is None:
            none_count += 1
            continue
        ids.append(str(value))
    return ids, none_count


def _row_value(row: Mapping[str, Any] | object, field_name: str) -> Any:
    if isinstance(row, Mapping):
        mapping_row = cast(Mapping[str, Any], row)
        return mapping_row.get(field_name)
    return getattr(row, field_name, None)


__all__ = [
    "BenchmarkRetrievalQuestion",
    "EvidencePathHitRateResult",
    "ExtractionBenchmarkFixture",
    "GroundednessProxyResult",
    "RetrievalAblationResult",
    "RetrievalRecallResult",
    "SchemaValidityResult",
    "calculate_evidence_path_hit_rate",
    "calculate_retrieval_recall",
    "evaluate_evidence_path_hit_rate",
    "evaluate_groundedness_proxy",
    "evaluate_retrieval_recall",
    "evaluate_schema_validity",
    "run_retrieval_ablations",
]
