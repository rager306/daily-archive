from __future__ import annotations

from typing import Any

import pytest

from research_graph.application.pipeline_script_inventory import (
    INVENTORY_SCHEMA_VERSION,
    ScriptCategory,
    ScriptClassification,
    ScriptContract,
    ScriptInventory,
    ScriptInventoryItem,
    ValidationIssue,
    validate_inventory,
)


def _valid_item(**overrides: object) -> ScriptInventoryItem:
    data: dict[str, Any] = {
        "script_id": "extract-r024-quality-metrics",
        "path": "scripts/extract_r024_quality_metrics.py",
        "category": ScriptCategory.QUALITY_METRICS,
        "classification": ScriptClassification.PRODUCTION_CANDIDATE,
        "migration_slice": "S04",
        "contract": ScriptContract(
            inputs=["data/r024-10-document-corpus-v1/events.jsonl"],
            expected_outputs=[
                "data/r024-10-document-corpus-v1/quality-metrics.json",
                "data/r024-10-document-corpus-v1/quality-comparison-5-vs-10.md",
            ],
            verification=["uv run pytest tests/test_r024_quality_metrics.py"],
            required_flags=[],
            summary_fields=["article_count", "chunk_count", "comparison"],
        ),
        "notes": "Quality metrics are recurring report scripts and must not be lost.",
    }
    data.update(overrides)
    return ScriptInventoryItem(**data)


def test_quality_metrics_is_first_class_inventory_category() -> None:
    assert ScriptCategory.QUALITY_METRICS.value == "quality-metrics"
    assert "quality-metrics" in {category.value for category in ScriptCategory}

    inventory = ScriptInventory(items=[_valid_item()])

    assert inventory.by_category()[ScriptCategory.QUALITY_METRICS] == [_valid_item()]
    assert inventory.counts_by_category() == {"quality-metrics": 1}


def test_valid_inventory_has_schema_version_and_no_issues() -> None:
    inventory = ScriptInventory(items=[_valid_item()])

    assert inventory.schema_version == INVENTORY_SCHEMA_VERSION
    assert validate_inventory(inventory) == []
    assert inventory.to_dict()["items"][0]["category"] == "quality-metrics"


def test_validation_reports_missing_required_fields_with_script_id() -> None:
    item = _valid_item(script_id="", path="")
    inventory = ScriptInventory(items=[item])

    issues = validate_inventory(inventory)

    assert ValidationIssue(
        script_id="<unknown>",
        field="script_id",
        message="script_id is required",
    ) in issues
    assert ValidationIssue(
        script_id="<unknown>",
        field="path",
        message="path is required",
    ) in issues


def test_validation_rejects_descriptive_expected_outputs() -> None:
    item = _valid_item(
        contract=ScriptContract(
            inputs=["data/r024-10-document-corpus-v1/events.jsonl"],
            expected_outputs=["writes a quality metrics report"],
            verification=["uv run pytest tests/test_r024_quality_metrics.py"],
            required_flags=[],
            summary_fields=["article_count"],
        )
    )
    inventory = ScriptInventory(items=[item])

    issues = validate_inventory(inventory)

    assert issues == [
        ValidationIssue(
            script_id="extract-r024-quality-metrics",
            field="contract.expected_outputs[0]",
            message="expected output must be a file path",
            path="writes a quality metrics report",
        )
    ]


def test_inventory_rejects_duplicate_script_ids() -> None:
    inventory = ScriptInventory(items=[_valid_item(), _valid_item()])

    issues = validate_inventory(inventory)

    assert ValidationIssue(
        script_id="extract-r024-quality-metrics",
        field="script_id",
        message="duplicate script_id",
    ) in issues


def test_inventory_requires_quality_metrics_category_when_quality_scripts_are_present() -> None:
    item = _valid_item(category=ScriptCategory.COVERAGE_REPORT)
    inventory = ScriptInventory(items=[item])

    issues = validate_inventory(inventory)

    assert ValidationIssue(
        script_id="extract-r024-quality-metrics",
        field="category",
        message="quality metrics scripts must use category quality-metrics",
        path="scripts/extract_r024_quality_metrics.py",
    ) in issues


@pytest.mark.parametrize(
    "category",
    [
        ScriptCategory.CATALOG_INGEST,
        ScriptCategory.PARSER_REPLAY,
        ScriptCategory.GRAPH_PROBE,
        ScriptCategory.QUALITY_METRICS,
        ScriptCategory.COVERAGE_REPORT,
        ScriptCategory.HISTORICAL,
    ],
)
def test_supported_categories_are_serializable(category: ScriptCategory) -> None:
    item = _valid_item(category=category)
    inventory = ScriptInventory(items=[item])

    assert inventory.to_dict()["items"][0]["category"] == category.value
