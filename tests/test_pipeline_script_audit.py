from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from research_graph.application.pipeline_script_inventory import (
    INVENTORY_SCHEMA_VERSION,
    ScriptCategory,
    ScriptClassification,
    ScriptInventory,
    validate_inventory,
)
from scripts import audit_pipeline_scripts

REPO_ROOT = Path(__file__).resolve().parents[1]
AUDIT_SCRIPT_PATH = REPO_ROOT / "scripts" / "audit_pipeline_scripts.py"


def _load_audit_module():
    return audit_pipeline_scripts


def test_build_inventory_captures_recurring_pipeline_categories() -> None:
    module = _load_audit_module()

    inventory = module.build_inventory(REPO_ROOT)

    assert isinstance(inventory, ScriptInventory)
    assert validate_inventory(inventory) == []
    assert inventory.counts_by_category() == {
        "catalog-ingest": 3,
        "parser-replay": 4,
        "graph-probe": 5,
        "quality-metrics": 4,
    }


def test_quality_metrics_scripts_are_first_class_production_candidates() -> None:
    module = _load_audit_module()

    inventory = module.build_inventory(REPO_ROOT)
    quality_items = inventory.by_category()[ScriptCategory.QUALITY_METRICS]

    assert {item.path for item in quality_items} == {
        "scripts/extract_r024_quality_metrics.py",
        "scripts/extract_r024_20_document_quality_metrics.py",
        "scripts/extract_r024_53_document_quality_metrics.py",
        "scripts/extract_r024_entity_quality_metrics.py",
    }
    assert all(
        item.classification is ScriptClassification.PRODUCTION_CANDIDATE
        for item in quality_items
    )
    assert {item.migration_slice for item in quality_items} == {"S04"}
    assert all(item.contract.expected_outputs for item in quality_items)


def test_inventory_marks_m061_legacy_ingest_as_compatibility_wrapper() -> None:
    module = _load_audit_module()

    inventory = module.build_inventory(REPO_ROOT)
    items_by_path = {item.path: item for item in inventory.items}

    legacy = items_by_path["scripts/m061_ingest_to_canonical_catalog.py"]
    assert legacy.category is ScriptCategory.CATALOG_INGEST
    assert legacy.classification is ScriptClassification.COMPATIBILITY_WRAPPER
    assert legacy.migration_slice == "S02"
    assert "--no-network" in legacy.contract.required_flags


def test_cli_write_outputs_schema_and_summary(tmp_path: Path) -> None:
    output = tmp_path / "script-inventory.json"

    result = subprocess.run(
        [sys.executable, str(AUDIT_SCRIPT_PATH), "--repo-root", str(REPO_ROOT), "--write", str(output)],
        check=True,
        text=True,
        capture_output=True,
    )

    payload = json.loads(output.read_text())
    inventory = ScriptInventory.from_dict(payload)
    assert payload["schema_version"] == INVENTORY_SCHEMA_VERSION
    assert validate_inventory(inventory) == []
    assert "script_count=16" in result.stdout
    assert "quality-metrics=4" in result.stdout
    assert "validation_issues=0" in result.stdout
