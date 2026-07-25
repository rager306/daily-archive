"""Application-level contracts for recurring pipeline script inventory.

This module intentionally contains no filesystem scanning or script execution. It
only defines the inventory shape and validation rules that S01 downstream audit
commands can use before migrating script business logic into hexagonal/onion
package seams.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any, Self

INVENTORY_SCHEMA_VERSION = "pipeline-script-inventory/v1"


class ScriptCategory(StrEnum):
    """Recurring pipeline script categories tracked by the migration."""

    CATALOG_INGEST = "catalog-ingest"
    PARSER_REPLAY = "parser-replay"
    GRAPH_PROBE = "graph-probe"
    QUALITY_METRICS = "quality-metrics"
    COVERAGE_REPORT = "coverage-report"
    HISTORICAL = "historical"
    OTHER = "other"


class ScriptClassification(StrEnum):
    """How the migration should treat a script."""

    PRODUCTION_CANDIDATE = "production-candidate"
    COMPATIBILITY_WRAPPER = "compatibility-wrapper"
    HISTORICAL_ONLY = "historical-only"
    OUT_OF_SCOPE = "out-of-scope"


@dataclass(frozen=True)
class ValidationIssue:
    """A secret-free validation issue for one inventory field."""

    script_id: str
    field: str
    message: str
    path: str | None = None


@dataclass(frozen=True)
class ScriptContract:
    """CLI and artifact contract that a script wrapper must preserve."""

    inputs: list[str] = field(default_factory=list)
    expected_outputs: list[str] = field(default_factory=list)
    verification: list[str] = field(default_factory=list)
    required_flags: list[str] = field(default_factory=list)
    summary_fields: list[str] = field(default_factory=list)

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> Self:
        return cls(
            inputs=list(payload.get("inputs", [])),
            expected_outputs=list(payload.get("expected_outputs", [])),
            verification=list(payload.get("verification", [])),
            required_flags=list(payload.get("required_flags", [])),
            summary_fields=list(payload.get("summary_fields", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "inputs": list(self.inputs),
            "expected_outputs": list(self.expected_outputs),
            "verification": list(self.verification),
            "required_flags": list(self.required_flags),
            "summary_fields": list(self.summary_fields),
        }


@dataclass(frozen=True)
class ScriptInventoryItem:
    """One script and its migration contract."""

    script_id: str
    path: str
    category: ScriptCategory
    classification: ScriptClassification
    contract: ScriptContract
    migration_slice: str | None = None
    notes: str = ""

    def __post_init__(self) -> None:
        object.__setattr__(self, "category", ScriptCategory(self.category))
        object.__setattr__(self, "classification", ScriptClassification(self.classification))
        if isinstance(self.contract, dict):
            object.__setattr__(self, "contract", ScriptContract.from_dict(self.contract))

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> Self:
        return cls(
            script_id=str(payload.get("script_id", "")),
            path=str(payload.get("path", "")),
            category=ScriptCategory(payload.get("category", ScriptCategory.OTHER.value)),
            classification=ScriptClassification(
                payload.get("classification", ScriptClassification.OUT_OF_SCOPE.value)
            ),
            migration_slice=payload.get("migration_slice"),
            contract=ScriptContract.from_dict(payload.get("contract", {})),
            notes=str(payload.get("notes", "")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "script_id": self.script_id,
            "path": self.path,
            "category": self.category.value,
            "classification": self.classification.value,
            "migration_slice": self.migration_slice,
            "contract": self.contract.to_dict(),
            "notes": self.notes,
        }


@dataclass(frozen=True)
class ScriptInventory:
    """Versioned collection of script migration contracts."""

    items: list[ScriptInventoryItem]
    schema_version: str = INVENTORY_SCHEMA_VERSION

    @classmethod
    def from_dict(cls, payload: dict[str, Any]) -> Self:
        return cls(
            schema_version=str(payload.get("schema_version", INVENTORY_SCHEMA_VERSION)),
            items=[ScriptInventoryItem.from_dict(item) for item in payload.get("items", [])],
        )

    def by_category(self) -> dict[ScriptCategory, list[ScriptInventoryItem]]:
        grouped: dict[ScriptCategory, list[ScriptInventoryItem]] = {}
        for item in self.items:
            grouped.setdefault(item.category, []).append(item)
        return grouped

    def counts_by_category(self) -> dict[str, int]:
        return {category.value: len(items) for category, items in self.by_category().items()}

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "items": [item.to_dict() for item in self.items],
        }


_PATH_SUFFIXES = (
    ".json",
    ".jsonl",
    ".md",
    ".txt",
    ".csv",
    ".tsv",
    ".pdf",
    ".html",
    ".parquet",
)


def validate_inventory(inventory: ScriptInventory) -> list[ValidationIssue]:
    """Validate inventory shape and wrapper contract path fields."""

    issues: list[ValidationIssue] = []
    seen_script_ids: set[str] = set()

    if inventory.schema_version != INVENTORY_SCHEMA_VERSION:
        issues.append(
            ValidationIssue(
                script_id="<inventory>",
                field="schema_version",
                message=f"schema_version must be {INVENTORY_SCHEMA_VERSION}",
            )
        )

    for item in inventory.items:
        script_id = item.script_id or "<unknown>"
        if not item.script_id:
            issues.append(
                ValidationIssue(
                    script_id=script_id,
                    field="script_id",
                    message="script_id is required",
                )
            )
        elif item.script_id in seen_script_ids:
            issues.append(
                ValidationIssue(
                    script_id=item.script_id,
                    field="script_id",
                    message="duplicate script_id",
                )
            )
        seen_script_ids.add(item.script_id)

        if not item.path:
            issues.append(
                ValidationIssue(
                    script_id=script_id,
                    field="path",
                    message="path is required",
                )
            )
        elif _looks_like_quality_metrics_script(item.path) and item.category is not ScriptCategory.QUALITY_METRICS:
            issues.append(
                ValidationIssue(
                    script_id=script_id,
                    field="category",
                    message="quality metrics scripts must use category quality-metrics",
                    path=item.path,
                )
            )

        issues.extend(_validate_contract_paths(script_id, item.contract))

    return issues


def _validate_contract_paths(script_id: str, contract: ScriptContract) -> list[ValidationIssue]:
    issues: list[ValidationIssue] = []
    for field_name, paths in (
        ("contract.inputs", contract.inputs),
        ("contract.expected_outputs", contract.expected_outputs),
    ):
        for index, path in enumerate(paths):
            if not _looks_like_file_path(path):
                label = "input" if field_name.endswith("inputs") else "expected output"
                issues.append(
                    ValidationIssue(
                        script_id=script_id,
                        field=f"{field_name}[{index}]",
                        message=f"{label} must be a file path",
                        path=path,
                    )
                )
    return issues


def _looks_like_file_path(value: str) -> bool:
    if not value or " " in value:
        return False
    if value.endswith(_PATH_SUFFIXES):
        return True
    if "/" in value and "." in value.rsplit("/", 1)[-1]:
        return True
    return False


def _looks_like_quality_metrics_script(path: str) -> bool:
    name = path.rsplit("/", 1)[-1]
    return name.startswith("extract_r024") and name.endswith("quality_metrics.py")
