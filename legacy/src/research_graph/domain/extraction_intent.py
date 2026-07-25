"""ExtractionIntentManifest.v1 + negative constraints (M278 E3.1).

Domain-pure pre-commitment for what may be extracted and what is forbidden.
Never authorizes import or graph writes. Principles inspired by ARS Material
Passport / Claim Intent — own schemas only (no vendored prompts).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Final, Literal

SCHEMA_VERSION: Final[str] = "extraction-intent.v1"
CONSTRAINTS_SCHEMA_VERSION: Final[str] = "negative-constraints.v1"

RiskTier = Literal["high", "medium", "low"]
ConstraintScope = Literal["global", "relation", "entity", "span", "process"]


@dataclass(frozen=True, slots=True)
class NegativeConstraint:
    """A fail-closed rule that blocks or flags extraction/promotion."""

    constraint_id: str
    description: str
    scope: ConstraintScope
    # If set, applies only to these relation/entity types (empty = all)
    applies_to: tuple[str, ...] = ()
    severity: Literal["block", "flag"] = "block"
    code: str = ""

    def to_dict(self) -> dict[str, Any]:
        return {
            "constraint_id": self.constraint_id,
            "description": self.description,
            "scope": self.scope,
            "applies_to": list(self.applies_to),
            "severity": self.severity,
            "code": self.code or self.constraint_id,
        }


@dataclass(frozen=True, slots=True)
class ExtractionIntentManifest:
    """Pre-commitment: what we intend to extract from a doc/section."""

    schema_version: str
    paper_id: str
    intent_id: str
    target_entity_types: tuple[str, ...]
    target_relation_types: tuple[str, ...]
    allowed_sections: tuple[str, ...]  # empty = all
    forbid_free_invent: bool
    require_source_span: bool
    negative_constraint_ids: tuple[str, ...]
    notes: str = ""
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("ExtractionIntentManifest cannot authorize import/writes")
        if self.schema_version != SCHEMA_VERSION:
            raise ValueError(f"unsupported intent schema: {self.schema_version}")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "paper_id": self.paper_id,
            "intent_id": self.intent_id,
            "target_entity_types": list(self.target_entity_types),
            "target_relation_types": list(self.target_relation_types),
            "allowed_sections": list(self.allowed_sections),
            "forbid_free_invent": self.forbid_free_invent,
            "require_source_span": self.require_source_span,
            "negative_constraint_ids": list(self.negative_constraint_ids),
            "notes": self.notes,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def default_negative_constraints() -> tuple[NegativeConstraint, ...]:
    """Canonical fail-closed constraint set (own code, not ARS prompts)."""
    return (
        NegativeConstraint(
            constraint_id="no_free_invent_relation",
            description="Relation type must be in closed typed vocabulary",
            scope="relation",
            severity="block",
            code="free_invent_relation",
        ),
        NegativeConstraint(
            constraint_id="no_import_without_user_go",
            description="import_eligible and graph writes remain false until explicit user go",
            scope="process",
            severity="block",
            code="import_locked",
        ),
        NegativeConstraint(
            constraint_id="no_gold_in_llm_gepa_context",
            description="Held-out gold / canary labels must not appear in GEPA or LLM prompts",
            scope="process",
            severity="block",
            code="gold_leakage",
        ),
        NegativeConstraint(
            constraint_id="high_impact_requires_faithfulness",
            description="High-impact relations require claim-faithfulness to span before promote",
            scope="relation",
            applies_to=(),  # filled by risk strata at check time
            severity="block",
            code="high_impact_no_faithfulness",
        ),
        NegativeConstraint(
            constraint_id="no_spanless_claim",
            description="Claims/relations requiring span must carry resolvable SourceSpan",
            scope="span",
            severity="block",
            code="spanless_claim",
        ),
        NegativeConstraint(
            constraint_id="citation_existence_not_faithfulness",
            description="Bibliographic existence must not be treated as claim support",
            scope="relation",
            applies_to=("CITES", "SUPPORTS", "CONTRASTS", "EXTENDS"),
            severity="flag",
            code="existence_vs_faithfulness",
        ),
    )


@dataclass(frozen=True, slots=True)
class NegativeConstraintRegistry:
    schema_version: str
    constraints: tuple[NegativeConstraint, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("constraint registry cannot authorize import/writes")

    def by_id(self) -> dict[str, NegativeConstraint]:
        return {c.constraint_id: c for c in self.constraints}

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "constraints": [c.to_dict() for c in self.constraints],
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def build_default_constraint_registry() -> NegativeConstraintRegistry:
    return NegativeConstraintRegistry(
        schema_version=CONSTRAINTS_SCHEMA_VERSION,
        constraints=default_negative_constraints(),
    )


def build_extraction_intent(
    *,
    paper_id: str,
    intent_id: str,
    target_entity_types: tuple[str, ...] | list[str],
    target_relation_types: tuple[str, ...] | list[str],
    allowed_sections: tuple[str, ...] | list[str] = (),
    forbid_free_invent: bool = True,
    require_source_span: bool = True,
    negative_constraint_ids: tuple[str, ...] | list[str] | None = None,
    notes: str = "",
) -> ExtractionIntentManifest:
    """Factory with fail-closed defaults."""
    if negative_constraint_ids is None:
        negative_constraint_ids = tuple(
            c.constraint_id for c in default_negative_constraints()
        )
    return ExtractionIntentManifest(
        schema_version=SCHEMA_VERSION,
        paper_id=paper_id,
        intent_id=intent_id,
        target_entity_types=tuple(target_entity_types),
        target_relation_types=tuple(target_relation_types),
        allowed_sections=tuple(allowed_sections),
        forbid_free_invent=bool(forbid_free_invent),
        require_source_span=bool(require_source_span),
        negative_constraint_ids=tuple(negative_constraint_ids),
        notes=notes,
    )


__all__ = [
    "SCHEMA_VERSION",
    "CONSTRAINTS_SCHEMA_VERSION",
    "RiskTier",
    "ConstraintScope",
    "NegativeConstraint",
    "ExtractionIntentManifest",
    "NegativeConstraintRegistry",
    "default_negative_constraints",
    "build_default_constraint_registry",
    "build_extraction_intent",
]
