from __future__ import annotations

import json
from pathlib import Path

from arxiv_archive.article_artifact_metrics import (
    ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION,
    calculate_article_artifact_metrics,
    calculate_benchmark_metrics,
    calculate_review_burden,
    count_raw_leakage,
    count_unsafe_authorizations,
)
from arxiv_archive.article_artifacts import validate_article_artifact_manifest

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_artifacts"


def _load(name: str) -> dict:
    return json.loads((FIXTURES_DIR / name).read_text(encoding="utf-8"))


def test_benchmark_fixtures_are_redacted_and_cover_required_artifact_types() -> None:
    cases = _load("benchmark_cases.json")
    gold = _load("benchmark_gold.json")

    assert cases["schema_version"] == "m023-article-artifact-benchmark-cases.v1"
    assert gold["schema_version"] == "m023-article-artifact-benchmark-gold.v1"
    assert {case["case_id"] for case in cases["cases"]} == {record["case_id"] for record in gold["gold"]}

    gold_types = {
        artifact["artifact_type"]
        for record in gold["gold"]
        for artifact in record["manifest"]["artifacts"]
    }
    assert {
        "figure",
        "table",
        "equation",
        "dataset",
        "method",
        "metric",
        "experiment",
        "claim",
        "reference",
    }.issubset(gold_types)

    serialized_safe_cases = json.dumps(
        [case for case in cases["cases"] if case["case_id"] != "raw-leakage-sentinel"]
    )
    for forbidden in (
        '"text":',
        '"raw_text":',
        '"caption_text":',
        '"raw_model_output":',
        '"embedding":',
        '"vector":',
        '"secret":',
        '"source_of_truth":',
    ):
        assert forbidden not in serialized_safe_cases

    for record in gold["gold"]:
        assert validate_article_artifact_manifest(record["manifest"]) == []


def test_metrics_report_artifact_link_span_lineage_and_review_burden() -> None:
    cases = _load("benchmark_cases.json")
    gold = _load("benchmark_gold.json")
    case = next(case for case in cases["cases"] if case["case_id"] == "positive-core-coverage")
    expected = next(record for record in gold["gold"] if record["case_id"] == case["case_id"])

    metrics = calculate_article_artifact_metrics(case["manifest"], expected["manifest"], case_id=case["case_id"])

    assert metrics["schema_version"] == ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION
    assert metrics["artifact_precision"] == 1.0
    assert metrics["artifact_recall"] == 1.0
    assert metrics["candidate_link_correctness"] == 1.0
    assert metrics["candidate_link_recall"] == 1.0
    assert metrics["source_span_coverage"] == 1.0
    assert metrics["section_lineage_correctness"] == 1.0
    assert metrics["raw_leakage_rate"] == 0.0
    assert metrics["unsafe_authorization_count"] == 0
    assert metrics["review_burden"] > metrics["predicted_artifact_count"]


def test_metrics_penalize_missing_spans_ambiguous_lineage_and_extra_links() -> None:
    cases = _load("benchmark_cases.json")
    gold = _load("benchmark_gold.json")
    case = next(case for case in cases["cases"] if case["case_id"] == "ambiguous-and-missing-span")
    expected = next(record for record in gold["gold"] if record["case_id"] == case["case_id"])

    metrics = calculate_article_artifact_metrics(case["manifest"], expected["manifest"], case_id=case["case_id"])

    assert metrics["artifact_counts"] == {
        "true_positive": 1,
        "false_positive": 0,
        "false_negative": 1,
        "precision": 1.0,
        "recall": 0.5,
    }
    assert metrics["candidate_link_counts"]["false_positive"] == 1
    assert metrics["source_span_coverage"] == 0.0
    assert metrics["section_lineage_correctness"] == 0.0
    assert metrics["review_burden"] >= 3


def test_raw_leakage_and_unsafe_authorizations_are_counted_without_values() -> None:
    cases = _load("benchmark_cases.json")
    case = next(case for case in cases["cases"] if case["case_id"] == "raw-leakage-sentinel")
    manifest = case["manifest"]

    assert count_raw_leakage(manifest) == 2
    assert count_unsafe_authorizations(manifest) >= 4

    gold = _load("benchmark_gold.json")
    expected = next(record for record in gold["gold"] if record["case_id"] == case["case_id"])
    metrics = calculate_article_artifact_metrics(manifest, expected["manifest"], case_id=case["case_id"])

    assert metrics["raw_leakage_count"] == 2
    assert metrics["raw_leakage_rate"] == 1.0
    assert metrics["unsafe_authorization_count"] >= 4
    assert "do not expose" not in json.dumps(metrics)


def test_benchmark_metrics_macro_and_totals_are_stable() -> None:
    metrics = calculate_benchmark_metrics(_load("benchmark_cases.json"), _load("benchmark_gold.json"))

    assert metrics["schema_version"] == ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION
    assert metrics["case_count"] == 3
    assert [case["case_id"] for case in metrics["case_metrics"]] == [
        "positive-core-coverage",
        "ambiguous-and-missing-span",
        "raw-leakage-sentinel",
    ]
    assert set(metrics["macro"]) == {
        "artifact_precision",
        "artifact_recall",
        "candidate_link_correctness",
        "candidate_link_recall",
        "source_span_coverage",
        "section_lineage_correctness",
        "section_lineage_recall",
        "raw_leakage_rate",
    }
    assert metrics["totals"]["raw_leakage_count"] == 2
    assert metrics["totals"]["unsafe_authorization_count"] >= 4


def test_review_burden_counts_artifacts_links_and_blocking_diagnostics() -> None:
    case = next(case for case in _load("benchmark_cases.json")["cases"] if case["case_id"] == "ambiguous-and-missing-span")

    assert calculate_review_burden(case["manifest"]) == 3
