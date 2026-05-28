"""Evaluation metrics for redacted article artifact detection manifests.

The metric functions in this module compare detection output against redacted
benchmark gold files. They intentionally operate on IDs, coordinates, hashes,
review states, safety flags, and diagnostic codes only; no raw article text or
model payload is needed or returned.
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from arxiv_archive.article_artifacts import (
    EXCLUDED_USES,
    FORBIDDEN_PAYLOAD_KEYS,
    TRUSTED_IMPORT_USE,
    default_safety_flags,
)

ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION = "m023-article-artifact-metrics.v1"


@dataclass(frozen=True)
class ArtifactMetricCounts:
    """Shared true/false positive/negative counts for a metric family."""

    true_positive: int
    false_positive: int
    false_negative: int

    @property
    def precision(self) -> float:
        return _ratio(self.true_positive, self.true_positive + self.false_positive)

    @property
    def recall(self) -> float:
        return _ratio(self.true_positive, self.true_positive + self.false_negative)

    def to_dict(self) -> dict[str, Any]:
        return {
            "true_positive": self.true_positive,
            "false_positive": self.false_positive,
            "false_negative": self.false_negative,
            "precision": self.precision,
            "recall": self.recall,
        }


def calculate_article_artifact_metrics(
    predicted_manifest: Mapping[str, Any],
    gold_manifest: Mapping[str, Any],
    *,
    case_id: str | None = None,
) -> dict[str, Any]:
    """Compare one predicted redacted manifest to one redacted gold manifest.

    Stable output keys are suitable for JSON snapshots and benchmark reports.
    Artifact matching uses ``(artifact_id, artifact_type)`` so a reused ID with
    the wrong vocabulary is counted as both a false positive and false negative.
    """

    predicted_artifacts = _artifact_records(predicted_manifest)
    gold_artifacts = _artifact_records(gold_manifest)
    artifact_counts = _set_counts(
        (_artifact_key(artifact) for artifact in predicted_artifacts),
        (_artifact_key(artifact) for artifact in gold_artifacts),
    )
    link_counts = _set_counts(
        (_candidate_link_key(link) for link in _candidate_links(predicted_artifacts)),
        (_candidate_link_key(link) for link in _candidate_links(gold_artifacts)),
    )
    span_counts = _set_counts(
        (_span_coverage_key(artifact) for artifact in predicted_artifacts if _has_source_span(artifact)),
        (_span_coverage_key(artifact) for artifact in gold_artifacts if _has_source_span(artifact)),
    )
    lineage_counts = _set_counts(
        (_lineage_key(artifact) for artifact in predicted_artifacts if _has_lineage(artifact)),
        (_lineage_key(artifact) for artifact in gold_artifacts if _has_lineage(artifact)),
    )
    raw_leak_count = count_raw_leakage(predicted_manifest)
    unsafe_authorization_count = count_unsafe_authorizations(predicted_manifest)
    review_burden = calculate_review_burden(predicted_manifest)

    return {
        "schema_version": ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION,
        "case_id": case_id,
        "paper_id": predicted_manifest.get("paper_id"),
        "gold_paper_id": gold_manifest.get("paper_id"),
        "artifact_precision": artifact_counts.precision,
        "artifact_recall": artifact_counts.recall,
        "artifact_counts": artifact_counts.to_dict(),
        "candidate_link_correctness": link_counts.precision,
        "candidate_link_recall": link_counts.recall,
        "candidate_link_counts": link_counts.to_dict(),
        "source_span_coverage": span_counts.recall,
        "source_span_counts": span_counts.to_dict(),
        "section_lineage_correctness": lineage_counts.precision,
        "section_lineage_recall": lineage_counts.recall,
        "section_lineage_counts": lineage_counts.to_dict(),
        "raw_leakage_count": raw_leak_count,
        "raw_leakage_rate": _ratio(raw_leak_count, max(1, len(predicted_artifacts))),
        "unsafe_authorization_count": unsafe_authorization_count,
        "review_burden": review_burden,
        "predicted_artifact_count": len(predicted_artifacts),
        "gold_artifact_count": len(gold_artifacts),
    }


def calculate_benchmark_metrics(
    benchmark_cases: Mapping[str, Any] | Sequence[Mapping[str, Any]],
    benchmark_gold: Mapping[str, Any] | Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Evaluate every case in a benchmark fixture and return stable JSON metrics."""

    cases = _cases_from_fixture(benchmark_cases)
    gold_by_id = _gold_by_case_id(benchmark_gold)
    case_metrics = []
    for case in cases:
        case_id = str(case.get("case_id"))
        gold = gold_by_id.get(case_id)
        if gold is None:
            raise ValueError(f"missing gold manifest for benchmark case: {case_id}")
        manifest = _manifest_from_case(case)
        case_metrics.append(calculate_article_artifact_metrics(manifest, gold, case_id=case_id))

    return {
        "schema_version": ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION,
        "case_count": len(case_metrics),
        "case_metrics": case_metrics,
        "macro": _macro_average(case_metrics),
        "totals": {
            "raw_leakage_count": sum(metric["raw_leakage_count"] for metric in case_metrics),
            "unsafe_authorization_count": sum(metric["unsafe_authorization_count"] for metric in case_metrics),
            "review_burden": sum(metric["review_burden"] for metric in case_metrics),
            "predicted_artifact_count": sum(metric["predicted_artifact_count"] for metric in case_metrics),
            "gold_artifact_count": sum(metric["gold_artifact_count"] for metric in case_metrics),
        },
    }


def count_raw_leakage(value: Any) -> int:
    """Count forbidden raw-payload key occurrences without returning values."""

    count = 0
    if isinstance(value, Mapping):
        for key, child in value.items():
            if str(key) in FORBIDDEN_PAYLOAD_KEYS:
                count += 1
            count += count_raw_leakage(child)
    elif isinstance(value, list):
        for child in value:
            count += count_raw_leakage(child)
    return count


def count_unsafe_authorizations(manifest: Mapping[str, Any]) -> int:
    """Count flags or uses that would make a pre-KG manifest unsafe to trust."""

    count = 0
    count += _unsafe_flags(manifest)
    if manifest.get("production_import_attempted") is not False:
        count += 1
    if manifest.get("ladybugdb_written") is not False:
        count += 1
    if int(manifest.get("promoted_to_fact_count") or 0) != 0:
        count += 1
    if int(manifest.get("import_eligible_count") or 0) != 0:
        count += 1
    count += _unsafe_uses(manifest)
    for artifact in _artifact_records(manifest):
        count += _unsafe_flags(artifact)
        if artifact.get("promoted_to_fact") is not False:
            count += 1
        if artifact.get("import_eligible") is not False:
            count += 1
        count += _unsafe_uses(artifact)
        for link in _candidate_links([artifact]):
            if link.get("promoted_to_fact") is not False:
                count += 1
            if link.get("import_eligible") is not False:
                count += 1
            count += _unsafe_uses(link)
    return count


def calculate_review_burden(manifest: Mapping[str, Any]) -> int:
    """Return the count of records requiring human or repair review."""

    burden_states = {"detected_unreviewed", "review_required", "ambiguous", "repair_required"}
    burden = 0
    for artifact in _artifact_records(manifest):
        if artifact.get("review_state") in burden_states:
            burden += 1
        for link in _candidate_links([artifact]):
            if link.get("review_state") in burden_states:
                burden += 1
    for diagnostic in _dicts(manifest.get("diagnostics")):
        if diagnostic.get("blocks_import") is True:
            burden += 1
    return burden


def _macro_average(case_metrics: Sequence[Mapping[str, Any]]) -> dict[str, float]:
    keys = (
        "artifact_precision",
        "artifact_recall",
        "candidate_link_correctness",
        "candidate_link_recall",
        "source_span_coverage",
        "section_lineage_correctness",
        "section_lineage_recall",
        "raw_leakage_rate",
    )
    if not case_metrics:
        return {key: 0.0 for key in keys}
    return {key: round(sum(float(metric[key]) for metric in case_metrics) / len(case_metrics), 6) for key in keys}


def _set_counts(predicted: Iterable[tuple[Any, ...]], gold: Iterable[tuple[Any, ...]]) -> ArtifactMetricCounts:
    predicted_set = set(predicted)
    gold_set = set(gold)
    return ArtifactMetricCounts(
        true_positive=len(predicted_set & gold_set),
        false_positive=len(predicted_set - gold_set),
        false_negative=len(gold_set - predicted_set),
    )


def _ratio(numerator: int, denominator: int) -> float:
    if denominator == 0:
        return 1.0
    return round(numerator / denominator, 6)


def _artifact_records(manifest: Mapping[str, Any]) -> list[dict[str, Any]]:
    return _dicts(manifest.get("artifacts"))


def _candidate_links(artifacts: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    return [link for artifact in artifacts for link in _dicts(artifact.get("candidate_links"))]


def _artifact_key(artifact: Mapping[str, Any]) -> tuple[Any, ...]:
    return (artifact.get("artifact_id"), artifact.get("artifact_type"))


def _candidate_link_key(link: Mapping[str, Any]) -> tuple[Any, ...]:
    return (link.get("source_artifact_id"), link.get("target_ref"), link.get("link_type"))


def _span_coverage_key(artifact: Mapping[str, Any]) -> tuple[Any, ...]:
    return (artifact.get("artifact_id"), artifact.get("artifact_type"))


def _lineage_key(artifact: Mapping[str, Any]) -> tuple[Any, ...]:
    lineage = artifact.get("section_lineage")
    if not isinstance(lineage, Mapping):
        return (artifact.get("artifact_id"), None, None, ())
    ordinal = lineage.get("ordinal_path")
    return (
        artifact.get("artifact_id"),
        lineage.get("section_id"),
        lineage.get("parent_section_id"),
        tuple(ordinal) if isinstance(ordinal, list) else (),
    )


def _has_source_span(artifact: Mapping[str, Any]) -> bool:
    return bool(_dicts(artifact.get("source_spans")))


def _has_lineage(artifact: Mapping[str, Any]) -> bool:
    return isinstance(artifact.get("section_lineage"), Mapping)


def _unsafe_flags(value: Mapping[str, Any]) -> int:
    flags = value.get("safety_flags")
    if not isinstance(flags, Mapping):
        return 1
    count = 0
    for key, expected in default_safety_flags().items():
        if flags.get(key) is not expected:
            count += 1
    return count


def _unsafe_uses(value: Mapping[str, Any]) -> int:
    count = 0
    allowed = value.get("allowed_uses")
    excluded = value.get("excluded_uses")
    if isinstance(allowed, list) and TRUSTED_IMPORT_USE in allowed:
        count += 1
    if isinstance(excluded, list):
        missing = [use for use in EXCLUDED_USES if use not in excluded]
        count += len(missing)
    else:
        count += len(EXCLUDED_USES)
    return count


def _cases_from_fixture(fixture: Mapping[str, Any] | Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    if isinstance(fixture, Mapping):
        return _dicts(fixture.get("cases"))
    return [dict(item) for item in fixture if isinstance(item, Mapping)]


def _gold_by_case_id(fixture: Mapping[str, Any] | Sequence[Mapping[str, Any]]) -> dict[str, dict[str, Any]]:
    if isinstance(fixture, Mapping):
        records = _dicts(fixture.get("gold"))
    else:
        records = [dict(item) for item in fixture if isinstance(item, Mapping)]
    result = {}
    for record in records:
        case_id = str(record.get("case_id"))
        result[case_id] = _manifest_from_case(record)
    return result


def _manifest_from_case(case: Mapping[str, Any]) -> dict[str, Any]:
    manifest = case.get("manifest")
    if isinstance(manifest, Mapping):
        return dict(manifest)
    raise ValueError(f"benchmark case lacks manifest: {case.get('case_id')}")


def _dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [dict(item) for item in value if isinstance(item, Mapping)]


__all__ = [
    "ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION",
    "ArtifactMetricCounts",
    "calculate_article_artifact_metrics",
    "calculate_benchmark_metrics",
    "calculate_review_burden",
    "count_raw_leakage",
    "count_unsafe_authorizations",
]
