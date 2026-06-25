from __future__ import annotations

from pathlib import Path

from scripts.inventory_write_paths import _classify


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
