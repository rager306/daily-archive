"""Compatibility shim for article artifact metrics.

The canonical import path is now ``arxiv_archive.artifacts.metrics``.
This module remains to preserve imports created before M081.
"""

from arxiv_archive.artifacts.metrics import (
    ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION,
    ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION,
    DSPY_READINESS_THRESHOLDS,
    ArtifactMetricCounts,
    calculate_article_artifact_metrics,
    calculate_benchmark_metrics,
    build_article_artifact_benchmark_report,
    render_article_artifact_benchmark_markdown,
    write_article_artifact_benchmark_report,
    count_raw_leakage,
    count_unsafe_authorizations,
    calculate_review_burden,
)

__all__ = [
    'ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION',
    'ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION',
    'DSPY_READINESS_THRESHOLDS',
    'ArtifactMetricCounts',
    'calculate_article_artifact_metrics',
    'calculate_benchmark_metrics',
    'build_article_artifact_benchmark_report',
    'render_article_artifact_benchmark_markdown',
    'write_article_artifact_benchmark_report',
    'count_raw_leakage',
    'count_unsafe_authorizations',
    'calculate_review_burden',
]
