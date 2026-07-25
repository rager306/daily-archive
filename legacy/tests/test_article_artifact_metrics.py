from __future__ import annotations

import json
from pathlib import Path

from research_graph.infrastructure.papers.artifacts.metrics import (
    ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION,
    ARTICLE_ARTIFACT_METRICS_SCHEMA_VERSION,
    build_article_artifact_benchmark_report,
    calculate_article_artifact_metrics,
    calculate_benchmark_metrics,
    calculate_review_burden,
    count_raw_leakage,
    count_unsafe_authorizations,
    render_article_artifact_benchmark_markdown,
    write_article_artifact_benchmark_report,
)
from research_graph.infrastructure.papers.artifacts.models import validate_article_artifact_manifest

FIXTURES_DIR = Path(__file__).parent / "fixtures" / "article_artifacts"


def _load(name: str) -> dict:
    return json.loads((FIXTURES_DIR / name).read_text(encoding="utf-8"))


def test_benchmark_fixtures_are_redacted_and_cover_required_artifact_types() -> None:
    cases = _load("benchmark_cases.json")
    gold = _load("benchmark_gold.json")

    assert cases["schema_version"] == "m023-article-artifact-benchmark-cases.v1"
    assert gold["schema_version"] == "m023-article-artifact-benchmark-gold.v1"
    assert {case["case_id"] for case in cases["cases"]} == {
        record["case_id"] for record in gold["gold"]
    }

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

    metrics = calculate_article_artifact_metrics(
        case["manifest"], expected["manifest"], case_id=case["case_id"]
    )

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

    metrics = calculate_article_artifact_metrics(
        case["manifest"], expected["manifest"], case_id=case["case_id"]
    )

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
    metrics = calculate_article_artifact_metrics(
        manifest, expected["manifest"], case_id=case["case_id"]
    )

    assert metrics["raw_leakage_count"] == 2
    assert metrics["raw_leakage_rate"] == 1.0
    assert metrics["unsafe_authorization_count"] >= 4
    assert "do not expose" not in json.dumps(metrics)


def test_benchmark_metrics_macro_and_totals_are_stable() -> None:
    metrics = calculate_benchmark_metrics(
        _load("benchmark_cases.json"), _load("benchmark_gold.json")
    )

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


def test_benchmark_report_compares_deterministic_and_minimax_mock_without_running_dspy() -> None:
    cases = _load("benchmark_cases.json")
    gold = _load("benchmark_gold.json")

    report = build_article_artifact_benchmark_report(cases, gold, minimax_cases=gold["gold"])

    assert report["schema_version"] == ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION
    assert report["dspy_optimization_ran"] is False
    assert report["dspy_status"] == "blocked"
    assert set(report["runs"]) == {"deterministic", "minimax_mock"}
    assert report["runs"]["deterministic"]["totals"]["raw_leakage_count"] == 2
    assert report["runs"]["minimax_mock"]["macro"]["artifact_precision"] == 1.0
    assert report["helper_delta"]["macro_artifact_recall"] > 0
    assert report["helper_delta"]["total_raw_leakage_count"] == -2
    assert report["observability"]["baseline_quality_recorded"] is True
    assert report["observability"]["helper_deltas_recorded"] is True
    assert report["observability"]["blocked_dspy_status_recorded"] is True
    assert report["dspy_precheck"]["selected_run"] == "minimax_mock"
    assert "all_runs_raw_leakage_count" in report["dspy_precheck"]["blockers"]
    assert "all_runs_unsafe_authorization_count" in report["dspy_precheck"]["blockers"]

    markdown = render_article_artifact_benchmark_markdown(report)
    assert "# Article Artifact Benchmark Report" in markdown
    assert "| deterministic | 3 |" in markdown
    assert "| minimax_mock | 3 |" in markdown
    assert "DSPy was not imported or executed" in markdown
    assert "do not expose" not in json.dumps(report)
    assert "do not expose" not in markdown


def test_benchmark_report_writer_emits_json_and_markdown(tmp_path: Path) -> None:
    written = write_article_artifact_benchmark_report(
        tmp_path,
        _load("benchmark_cases.json"),
        _load("benchmark_gold.json"),
        minimax_cases=_load("benchmark_gold.json")["gold"],
    )

    json_path = Path(written["json_path"])
    markdown_path = Path(written["markdown_path"])
    assert json_path.exists()
    assert markdown_path.exists()
    assert (
        json.loads(json_path.read_text(encoding="utf-8"))["schema_version"]
        == ARTICLE_ARTIFACT_BENCHMARK_REPORT_SCHEMA_VERSION
    )
    assert markdown_path.read_text(encoding="utf-8").startswith(
        "# Article Artifact Benchmark Report"
    )


def test_benchmark_metrics_reject_missing_gold_and_malformed_cases() -> None:
    cases = _load("benchmark_cases.json")
    gold = _load("benchmark_gold.json")

    truncated_gold = {"gold": gold["gold"][:1]}
    try:
        calculate_benchmark_metrics(cases, truncated_gold)
    except ValueError as exc:
        assert "missing gold manifest" in str(exc)
    else:  # pragma: no cover - documents the negative contract if exception behavior regresses
        raise AssertionError("expected missing gold manifest error")

    malformed_cases = {"cases": [{"case_id": gold["gold"][0]["case_id"]}]}
    try:
        calculate_benchmark_metrics(malformed_cases, gold)
    except ValueError as exc:
        assert "lacks manifest" in str(exc)
    else:  # pragma: no cover
        raise AssertionError("expected malformed case error")


def test_review_burden_counts_artifacts_links_and_blocking_diagnostics() -> None:
    case = next(
        case
        for case in _load("benchmark_cases.json")["cases"]
        if case["case_id"] == "ambiguous-and-missing-span"
    )

    assert calculate_review_burden(case["manifest"]) == 3


def test_missing_false_safety_flags_are_treated_as_safe_defaults_for_old_fixtures() -> None:
    manifest = {
        "safety_flags": {
            "raw_text_included": False,
            "raw_binary_included": False,
            "base64_included": False,
            "chunk_text_included": False,
            "model_outputs_included": False,
            "embeddings_included": False,
            "vectors_included": False,
            "optimizer_traces_included": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "trusted_kg_import_allowed": False,
        },
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "promoted_to_fact_count": 0,
        "import_eligible_count": 0,
        "allowed_uses": [],
        "excluded_uses": [
            "trusted_kg_import",
            "production_ladybugdb_write",
            "embedding_generation",
            "source_of_truth_claim",
        ],
        "artifacts": [],
    }

    assert count_unsafe_authorizations(manifest) == 0


def test_article_artifact_metrics_old_module_is_archived_with_canonical_breadcrumb() -> None:
    top_level_archive_path = Path(
        "archive/package-layout-shims/wave-01/src/arxiv_archive/article_artifact_metrics.py"
    )
    package_archive_path = Path(
        "archive/package-rename-waves/wave-01/src/arxiv_archive/artifacts/metrics.py"
    )
    canonical_path = Path("src/research_graph/papers/artifacts/metrics.py")

    assert top_level_archive_path.exists()
    assert package_archive_path.exists()
    assert not Path("src/arxiv_archive/article_artifact_metrics.py").exists()
    assert not Path("src/arxiv_archive/artifacts/metrics.py").exists()
    assert "Formerly: src/arxiv_archive/artifacts/metrics.py" in canonical_path.read_text(
        encoding="utf-8"
    )
