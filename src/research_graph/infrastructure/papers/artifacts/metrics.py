"""Evaluation metrics for redacted article artifact detection manifests.

The metric functions in this module compare detection output against redacted
benchmark gold files. They intentionally operate on IDs, coordinates, hashes,
review states, safety flags, and diagnostic codes only; no raw article text or
model payload is needed or returned.

Formerly: src/arxiv_archive/article_artifact_metrics.py

Formerly: src/arxiv_archive/artifacts/metrics.py
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.infrastructure.papers.artifacts.models import (
    EXCLUDED_USES,
    FORBIDDEN_PAYLOAD_KEYS,
    TRUSTED_IMPORT_USE,
    default_safety_flags,
)

ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION = "article-artifact-metrics.v1"
ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION = "article-artifact-benchmark-report.v1"

DSPY_READINESS_THRESHOLDS = {
    "min_case_count": 3,
    "artifact_precision": 0.8,
    "artifact_recall": 0.8,
    "source_span_coverage": 0.8,
    "candidate_link_correctness": 0.75,
    "section_lineage_correctness": 0.75,
    "max_raw_leakage_count": 0,
    "max_unsafe_authorization_count": 0,
}


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
        (
            _span_coverage_key(artifact)
            for artifact in predicted_artifacts
            if _has_source_span(artifact)
        ),
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
            "unsafe_authorization_count": sum(
                metric["unsafe_authorization_count"] for metric in case_metrics
            ),
            "review_burden": sum(metric["review_burden"] for metric in case_metrics),
            "predicted_artifact_count": sum(
                metric["predicted_artifact_count"] for metric in case_metrics
            ),
            "gold_artifact_count": sum(metric["gold_artifact_count"] for metric in case_metrics),
        },
    }


def build_article_artifact_benchmark_report(
    deterministic_cases: Mapping[str, Any] | Sequence[Mapping[str, Any]],
    benchmark_gold: Mapping[str, Any] | Sequence[Mapping[str, Any]],
    *,
    minimax_cases: Mapping[str, Any] | Sequence[Mapping[str, Any]] | None = None,
    thresholds: Mapping[str, float | int] | None = None,
) -> dict[str, Any]:
    """Build a stable benchmark report for deterministic and MiniMax-assisted runs.

    The report evaluates local redacted fixtures only. It records quality deltas,
    no-import safety counters, and an explicit DSPy readiness precheck without
    importing or invoking DSPy optimizers.
    """

    effective_thresholds = {**DSPY_READINESS_THRESHOLDS, **dict(thresholds or {})}
    deterministic_metrics = calculate_benchmark_metrics(deterministic_cases, benchmark_gold)
    runs: dict[str, Any] = {"deterministic": deterministic_metrics}
    if minimax_cases is not None:
        runs["minimax_mock"] = calculate_benchmark_metrics(minimax_cases, benchmark_gold)

    helper_delta = _helper_delta(runs.get("deterministic"), runs.get("minimax_mock"))
    dspy_precheck = _dspy_readiness_precheck(runs, effective_thresholds)

    return {
        "schema_version": ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION,
        "metrics_schema_version": ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION,
        "report_mode": "local_redacted_fixture_benchmark",
        "dspy_optimization_ran": False,
        "dspy_status": "ready" if dspy_precheck["ready"] else "blocked",
        "thresholds": effective_thresholds,
        "runs": runs,
        "helper_delta": helper_delta,
        "dspy_precheck": dspy_precheck,
        "observability": {
            "baseline_quality_recorded": True,
            "helper_deltas_recorded": minimax_cases is not None,
            "blocked_dspy_status_recorded": not dspy_precheck["ready"],
            "no_import_safety_counters": _no_import_safety_counters(runs),
        },
    }


def render_article_artifact_benchmark_markdown(report: Mapping[str, Any]) -> str:
    """Render a compact Markdown summary for a benchmark report."""

    lines = [
        "# Article Artifact Benchmark Report",
        "",
        f"- Schema: `{report.get('schema_version')}`",
        f"- Mode: `{report.get('report_mode')}`",
        f"- DSPy optimization ran: `{str(report.get('dspy_optimization_ran')).lower()}`",
        f"- DSPy status: **{report.get('dspy_status')}**",
        "",
        "## Run Metrics",
        "",
        "| Run | Cases | Artifact Precision | Artifact Recall | Span Coverage | Link Correctness | Lineage Correctness | Raw Leaks | Unsafe Auth | Review Burden |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for run_name, metrics in dict(report.get("runs", {})).items():
        macro = metrics.get("macro", {})
        totals = metrics.get("totals", {})
        lines.append(
            "| "
            + " | ".join(
                [
                    str(run_name),
                    str(metrics.get("case_count", 0)),
                    _format_metric(macro.get("artifact_precision")),
                    _format_metric(macro.get("artifact_recall")),
                    _format_metric(macro.get("source_span_coverage")),
                    _format_metric(macro.get("candidate_link_correctness")),
                    _format_metric(macro.get("section_lineage_correctness")),
                    str(totals.get("raw_leakage_count", 0)),
                    str(totals.get("unsafe_authorization_count", 0)),
                    str(totals.get("review_burden", 0)),
                ]
            )
            + " |"
        )

    lines.extend(["", "## Helper Delta", ""])
    delta = report.get("helper_delta")
    if isinstance(delta, Mapping) and delta:
        for key, value in delta.items():
            lines.append(f"- `{key}`: {_format_metric(value)}")
    else:
        lines.append("- No MiniMax mock run supplied.")

    lines.extend(["", "## DSPy Precheck", ""])
    precheck = report.get("dspy_precheck", {})
    lines.append(f"- Ready: `{str(precheck.get('ready')).lower()}`")
    lines.append(f"- Selected run: `{precheck.get('selected_run')}`")
    blockers = precheck.get("blockers", [])
    if blockers:
        lines.append("- Blockers:")
        for blocker in blockers:
            lines.append(f"  - {blocker}")
    else:
        lines.append("- Blockers: none")
    lines.append("- DSPy was not imported or executed; this is a readiness gate only.")
    lines.append("")
    return "\n".join(lines)


def write_article_artifact_benchmark_report(
    output_dir: Path | str,
    deterministic_cases: Mapping[str, Any] | Sequence[Mapping[str, Any]],
    benchmark_gold: Mapping[str, Any] | Sequence[Mapping[str, Any]],
    *,
    minimax_cases: Mapping[str, Any] | Sequence[Mapping[str, Any]] | None = None,
    thresholds: Mapping[str, float | int] | None = None,
) -> dict[str, Any]:
    """Write JSON and Markdown benchmark report artifacts and return paths."""

    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    report = build_article_artifact_benchmark_report(
        deterministic_cases,
        benchmark_gold,
        minimax_cases=minimax_cases,
        thresholds=thresholds,
    )
    json_path = output_path / "article-artifact-benchmark-report.json"
    markdown_path = output_path / "article-artifact-benchmark-report.md"
    json_path.write_text(_json_dumps(report), encoding="utf-8")
    markdown_path.write_text(render_article_artifact_benchmark_markdown(report), encoding="utf-8")
    return {
        "report": report,
        "json_path": str(json_path),
        "markdown_path": str(markdown_path),
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


def _helper_delta(
    deterministic: Mapping[str, Any] | None, minimax: Mapping[str, Any] | None
) -> dict[str, float | int]:
    if deterministic is None or minimax is None:
        return {}
    macro_keys = (
        "artifact_precision",
        "artifact_recall",
        "candidate_link_correctness",
        "candidate_link_recall",
        "source_span_coverage",
        "section_lineage_correctness",
        "section_lineage_recall",
        "raw_leakage_rate",
    )
    delta: dict[str, float | int] = {}
    for key in macro_keys:
        delta[f"macro_{key}"] = round(
            float(minimax.get("macro", {}).get(key, 0.0))
            - float(deterministic.get("macro", {}).get(key, 0.0)),
            6,
        )
    for key in (
        "raw_leakage_count",
        "unsafe_authorization_count",
        "review_burden",
        "predicted_artifact_count",
    ):
        delta[f"total_{key}"] = int(minimax.get("totals", {}).get(key, 0)) - int(
            deterministic.get("totals", {}).get(key, 0)
        )
    return delta


def _dspy_readiness_precheck(
    runs: Mapping[str, Mapping[str, Any]], thresholds: Mapping[str, float | int]
) -> dict[str, Any]:
    selected_run = "minimax_mock" if "minimax_mock" in runs else "deterministic"
    metrics = runs[selected_run]
    macro = metrics.get("macro", {})
    totals = metrics.get("totals", {})
    all_raw_leaks = sum(
        int(run.get("totals", {}).get("raw_leakage_count", 0)) for run in runs.values()
    )
    all_unsafe_authorizations = sum(
        int(run.get("totals", {}).get("unsafe_authorization_count", 0)) for run in runs.values()
    )
    checks = {
        "case_count": int(metrics.get("case_count", 0)) >= int(thresholds["min_case_count"]),
        "artifact_precision": float(macro.get("artifact_precision", 0.0))
        >= float(thresholds["artifact_precision"]),
        "artifact_recall": float(macro.get("artifact_recall", 0.0))
        >= float(thresholds["artifact_recall"]),
        "source_span_coverage": float(macro.get("source_span_coverage", 0.0))
        >= float(thresholds["source_span_coverage"]),
        "candidate_link_correctness": float(macro.get("candidate_link_correctness", 0.0))
        >= float(thresholds["candidate_link_correctness"]),
        "section_lineage_correctness": float(macro.get("section_lineage_correctness", 0.0))
        >= float(thresholds["section_lineage_correctness"]),
        "raw_leakage_count": int(totals.get("raw_leakage_count", 0))
        <= int(thresholds["max_raw_leakage_count"]),
        "unsafe_authorization_count": int(totals.get("unsafe_authorization_count", 0))
        <= int(thresholds["max_unsafe_authorization_count"]),
        "all_runs_raw_leakage_count": all_raw_leaks <= int(thresholds["max_raw_leakage_count"]),
        "all_runs_unsafe_authorization_count": all_unsafe_authorizations
        <= int(thresholds["max_unsafe_authorization_count"]),
        "dspy_not_run": True,
    }
    blockers = [name for name, passed in checks.items() if not passed]
    return {
        "ready": not blockers,
        "selected_run": selected_run,
        "checks": checks,
        "blockers": blockers,
        "optimization_ran": False,
    }


def _no_import_safety_counters(runs: Mapping[str, Mapping[str, Any]]) -> dict[str, dict[str, int]]:
    return {
        name: {
            "raw_leakage_count": int(metrics.get("totals", {}).get("raw_leakage_count", 0)),
            "unsafe_authorization_count": int(
                metrics.get("totals", {}).get("unsafe_authorization_count", 0)
            ),
        }
        for name, metrics in runs.items()
    }


def _format_metric(value: Any) -> str:
    if isinstance(value, float):
        return f"{value:.6f}"
    if isinstance(value, int):
        return str(value)
    return "0.000000"


def _json_dumps(value: Mapping[str, Any]) -> str:
    import json

    return json.dumps(value, indent=2, sort_keys=True) + "\n"


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
        return dict.fromkeys(keys, 0.0)
    return {
        key: round(sum(float(metric[key]) for metric in case_metrics) / len(case_metrics), 6)
        for key in keys
    }


def _set_counts(
    predicted: Iterable[tuple[Any, ...]], gold: Iterable[tuple[Any, ...]]
) -> ArtifactMetricCounts:
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
        # Older redacted fixtures may omit newly introduced false-valued safety
        # flags. Treat those as the safe default while still counting explicit
        # unsafe values, and still count missing true-valued defaults as unsafe.
        actual = flags.get(key, expected if expected is False else None)
        if actual is not expected:
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


def _cases_from_fixture(
    fixture: Mapping[str, Any] | Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    if isinstance(fixture, Mapping):
        return _dicts(fixture.get("cases"))
    return [dict(item) for item in fixture if isinstance(item, Mapping)]


def _gold_by_case_id(
    fixture: Mapping[str, Any] | Sequence[Mapping[str, Any]],
) -> dict[str, dict[str, Any]]:
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
    "ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION",
    "ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION",
    "ArtifactMetricCounts",
    "calculate_article_artifact_metrics",
    "build_article_artifact_benchmark_report",
    "calculate_benchmark_metrics",
    "calculate_review_burden",
    "render_article_artifact_benchmark_markdown",
    "count_raw_leakage",
    "count_unsafe_authorizations",
    "write_article_artifact_benchmark_report",
]
