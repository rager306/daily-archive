"""Schema-version gate for no-write graph projection rehearsal.

The gate reports migration requirements as metadata-only placeholders. It does
not migrate data, authorize imports, or write graph state.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any

from research_graph.domain.ports import PROJECTION_SCHEMA_VERSION, ProjectionRequest
from research_graph.domain.universal_kb.contracts import SafetyFlags

CURRENT_CANDIDATE_SCHEMA_VERSION = "universal-kb-candidate.v1"
CURRENT_PROJECTION_SCHEMA_VERSION = PROJECTION_SCHEMA_VERSION


@dataclass(frozen=True, slots=True)
class SchemaMigrationPlan:
    """Metadata-only placeholder for a future schema migration path."""

    from_schema_version: str
    to_schema_version: str
    status: str = "placeholder_only"

    def __post_init__(self) -> None:
        for field_name in ("from_schema_version", "to_schema_version", "status"):
            if not str(getattr(self, field_name)).strip():
                raise ValueError(f"{field_name} must be non-empty")


@dataclass(frozen=True, slots=True)
class SchemaGateResult:
    """Fail-closed schema gate output for projection rehearsal."""

    candidate_schema_version: str
    projection_schema_version: str
    accepted: bool
    migration_required: bool
    diagnostics: tuple[str, ...]
    migration_plan: SchemaMigrationPlan | None = None
    safety_flags: SafetyFlags = SafetyFlags()

    def __post_init__(self) -> None:
        for field_name in ("candidate_schema_version", "projection_schema_version"):
            if not str(getattr(self, field_name)).strip():
                raise ValueError(f"{field_name} must be non-empty")
        object.__setattr__(self, "diagnostics", tuple(str(item) for item in self.diagnostics))
        if self.accepted and self.migration_required:
            raise ValueError("accepted schema gate cannot require migration")
        if self.migration_required and self.migration_plan is None:
            raise ValueError("migration_required needs a migration_plan")
        self.assert_no_write()

    def assert_no_write(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


class GraphProjectionSchemaGate:
    """Validate candidate/projection schema versions before no-write projection."""

    def validate(self, request: ProjectionRequest) -> SchemaGateResult:
        candidate_schema = request.candidate_packet.schema_version
        projection_schema = request.schema_version
        request.candidate_packet.assert_no_write()

        if candidate_schema != CURRENT_CANDIDATE_SCHEMA_VERSION:
            return SchemaGateResult(
                candidate_schema_version=candidate_schema,
                projection_schema_version=projection_schema,
                accepted=False,
                migration_required=True,
                diagnostics=("schema_migration_required",),
                migration_plan=SchemaMigrationPlan(
                    from_schema_version=candidate_schema,
                    to_schema_version=CURRENT_CANDIDATE_SCHEMA_VERSION,
                ),
            )
        if projection_schema != CURRENT_PROJECTION_SCHEMA_VERSION:
            return SchemaGateResult(
                candidate_schema_version=candidate_schema,
                projection_schema_version=projection_schema,
                accepted=False,
                migration_required=True,
                diagnostics=("schema_migration_required",),
                migration_plan=SchemaMigrationPlan(
                    from_schema_version=projection_schema,
                    to_schema_version=CURRENT_PROJECTION_SCHEMA_VERSION,
                ),
            )
        return SchemaGateResult(
            candidate_schema_version=candidate_schema,
            projection_schema_version=projection_schema,
            accepted=True,
            migration_required=False,
            diagnostics=("schema_versions_current",),
        )


__all__ = [
    "CURRENT_CANDIDATE_SCHEMA_VERSION",
    "CURRENT_PROJECTION_SCHEMA_VERSION",
    "GraphProjectionSchemaGate",
    "SchemaGateResult",
    "SchemaMigrationPlan",
]
