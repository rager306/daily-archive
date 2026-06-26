from __future__ import annotations

from pathlib import Path

from scripts.inventory_write_paths import _classify, render_delta_markdown


def test_reviewed_shared_state_records_get_precise_categories() -> None:
    assert _classify(
        Path("src/research_graph/application/validation/batch_state.py"),
        "write_text",
        "output_path",
        None,
    ) == ("run-owned-state", "workflow-owned batch state replacement")
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/ingestion/catalog_adapters.py"),
        "write_text",
        "summary_path",
        None,
    ) == (
        "legacy-evidence-regeneration",
        "reviewed legacy ingest summary regeneration",
    )
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py"),
        "write_text",
        "report_path",
        None,
    ) == (
        "legacy-evidence-regeneration",
        "reviewed legacy ingest report regeneration",
    )
    assert _classify(
        Path("src/research_graph/infrastructure/repair/chunk_baseline_measurement.py"),
        "write_text",
        "index_path",
        None,
    ) == ("caller-owned-index", "caller-provided paired review index output")


def test_graph_readiness_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/graph/readiness/export.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("graph-readiness-evidence", "reviewed graph-readiness evidence output")


def test_source_asset_and_article_artifact_outputs_get_precise_categories() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/papers/source_assets/registry.py"),
        "write_text",
        "output_dir / 'source-asset-summary.json'",
        None,
    ) == ("source-asset-package", "reviewed source asset package output")
    assert _classify(
        Path("src/research_graph/cli/commands/article_artifacts.py"),
        "write_text",
        "diagnostics_path",
        None,
    ) == ("article-artifact-package", "reviewed article artifact package output")


def test_m176_script_wave_one_outputs_get_precise_categories_without_generic_script_rules() -> None:
    assert _classify(
        Path("scripts/m061_anchor_pilot.py"),
        "write_text",
        "path",
        None,
    ) == ("m061-acquisition-pipeline-output", "reviewed M061 acquisition pipeline output")
    assert _classify(
        Path("scripts/m058_plotextractor_extract.py"),
        "write_text",
        "summary_path",
        None,
    ) == (
        "figure-extraction-benchmark-output",
        "reviewed figure extraction benchmark output",
    )
    assert _classify(
        Path("scripts/build_m028_pdf_acquisition_diagnostics.py"),
        "write_text",
        "out_dir / REPORT_FILENAME",
        None,
    ) == ("m028-acquisition-evidence-output", "reviewed M028 acquisition evidence output")
    assert _classify(
        Path("scripts/verify_m029_unified_source_acquisition.py"),
        "write_text",
        "path",
        None,
    ) == ("script-only", "write occurs in process-boundary script")


def test_daily_cli_outputs_get_precise_category_without_moving_temp_path() -> None:
    assert _classify(
        Path("src/research_graph/cli/__init__.py"),
        "write_text",
        "filepath",
        None,
    ) == ("daily-cli-output", "reviewed daily CLI output")
    assert _classify(
        Path("src/research_graph/cli/__init__.py"),
        "write_text",
        "day_dir / 'papers.json'",
        None,
    ) == ("daily-cli-output", "reviewed daily CLI output")
    assert _classify(
        Path("src/research_graph/cli/__init__.py"),
        "write_text",
        "temp_path",
        None,
    ) == ("temporary", "same-directory temporary write before final replacement")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "filepath",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")


def test_parser_replay_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/parsing/replay_adapters.py"),
        "write_text",
        "cache_path",
        None,
    ) == ("parser-replay-output", "reviewed parser replay output")


def test_source_scan_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/corpus/sources/thirty_paper_source_scan.py"),
        "write_text",
        "destination",
        None,
    ) == ("source-scan-output", "reviewed source scan output")


def test_graph_probe_outputs_get_precise_category() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/graph/r024_networkx_probe.py"),
        "write_text",
        "config.memory_profile_path",
        None,
    ) == ("graph-probe-output", "reviewed graph probe output")


def test_repair_benchmark_outputs_get_precise_category_without_moving_index() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/repair/chunking_benchmark.py"),
        "write_text",
        "output_dir / 'chunking-benchmark-summary.json'",
        None,
    ) == ("repair-benchmark-output", "reviewed repair benchmark output")
    assert _classify(
        Path("src/research_graph/infrastructure/repair/chunk_baseline_measurement.py"),
        "write_text",
        "index_path",
        None,
    ) == ("caller-owned-index", "caller-provided paired review index output")


def test_validation_batch_outputs_get_precise_category_without_generic_summary_rule() -> None:
    assert _classify(
        Path("src/research_graph/workflows/validation/batch_workflow.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("validation-batch-output", "reviewed validation batch output")
    assert _classify(
        Path("src/research_graph/workflows/validation/batch_workflow.py"),
        "write_text",
        "output_path",
        None,
    ) == ("validation-batch-output", "reviewed validation batch output")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")


def test_unreviewed_summary_artifact_cache_and_destination_paths_keep_broad_categories() -> None:
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "output_dir / 'source-asset-summary.json'",
        None,
    ) == ("run-scoped", "caller/output scoped artifact path")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "cache_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/example_writer.py"),
        "write_text",
        "destination",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/graph/example_probe.py"),
        "write_text",
        "summary_path",
        None,
    ) == ("caller-owned", "caller-provided or adapter-owned output path")
    assert _classify(
        Path("src/research_graph/infrastructure/repair/example_benchmark.py"),
        "write_text",
        "diagnostics_path",
        None,
    ) == ("append-log", "event or diagnostics log path")


def test_unreviewed_state_index_catalog_paths_remain_shared_state() -> None:
    for target in ("state_path", "index_path", "catalog_path"):
        category, reason = _classify(
            Path("src/research_graph/infrastructure/example_writer.py"),
            "write_text",
            target,
            None,
        )
        assert category == "shared-state"
        assert reason == "stable shared state or index path"


def test_render_delta_markdown_reports_totals_and_sorted_category_deltas() -> None:
    baseline = {
        "summary": {
            "total_records": 3,
            "by_category": {"caller-owned": 2, "run-scoped": 1},
        }
    }
    current = {
        "summary": {
            "total_records": 4,
            "by_category": {"caller-owned": 1, "daily-cli-output": 2, "run-scoped": 1},
        }
    }

    markdown = render_delta_markdown(baseline, current)

    assert "Baseline total records: `3`" in markdown
    assert "Current total records: `4`" in markdown
    assert "Total delta: `+1`" in markdown
    assert "| caller-owned | 2 | 1 | -1 |" in markdown
    assert "| daily-cli-output | 0 | 2 | +2 |" in markdown
    assert "| run-scoped | 1 | 1 | +0 |" in markdown
    assert markdown.index("caller-owned") < markdown.index("daily-cli-output") < markdown.index(
        "run-scoped"
    )
