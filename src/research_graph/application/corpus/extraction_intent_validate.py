"""Validate ExtractionIntentManifest against closed types + constraints (M278).

Application pure. Never sets import_eligible true.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.domain.extraction_intent import (
    ExtractionIntentManifest,
    NegativeConstraintRegistry,
    build_default_constraint_registry,
)
from research_graph.domain.relation_types import ALL_TYPED_RELATIONS


@dataclass(frozen=True, slots=True)
class IntentValidationVerdict:
    ok: bool
    violations: tuple[str, ...]
    intent_hash: str
    constraints_hash: str
    import_eligible: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "ok": self.ok,
            "violations": list(self.violations),
            "intent_hash": self.intent_hash,
            "constraints_hash": self.constraints_hash,
            "import_eligible": False,
            "graph_writes_allowed": False,
        }


def hash_stable_payload(payload: Mapping[str, Any]) -> str:
    raw = json.dumps(payload, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def intent_content_hash(intent: ExtractionIntentManifest) -> str:
    return hash_stable_payload(intent.to_dict())


def constraints_content_hash(registry: NegativeConstraintRegistry) -> str:
    return hash_stable_payload(registry.to_dict())


def validate_extraction_intent(
    intent: ExtractionIntentManifest,
    *,
    registry: NegativeConstraintRegistry | None = None,
    known_entity_types: Sequence[str] | None = None,
) -> IntentValidationVerdict:
    """Validate intent pre-commitment.

    Violations (block):
    - empty target relation types when forbid_free_invent
    - relation types outside closed vocabulary when forbid_free_invent
    - unknown constraint ids
    - import flags (already rejected by domain ctor)
    """
    reg = registry or build_default_constraint_registry()
    violations: list[str] = []
    known_rel = set(ALL_TYPED_RELATIONS)
    by_id = reg.by_id()

    for cid in intent.negative_constraint_ids:
        if cid not in by_id:
            violations.append(f"unknown_constraint_id:{cid}")

    if intent.forbid_free_invent:
        if not intent.target_relation_types:
            violations.append("empty_target_relation_types")
        for rt in intent.target_relation_types:
            if rt not in known_rel:
                violations.append(f"free_invent_relation:{rt}")

    if known_entity_types is not None:
        allowed_e = set(known_entity_types)
        for et in intent.target_entity_types:
            if et not in allowed_e:
                violations.append(f"unknown_entity_type:{et}")

    if not intent.require_source_span and "no_spanless_claim" in intent.negative_constraint_ids:
        violations.append("require_source_span_false_with_no_spanless_constraint")

    ok = not violations
    return IntentValidationVerdict(
        ok=ok,
        violations=tuple(violations),
        intent_hash=intent_content_hash(intent),
        constraints_hash=constraints_content_hash(reg),
    )


def candidate_relation_allowed(
    relation_type: str,
    intent: ExtractionIntentManifest,
) -> tuple[bool, str]:
    """Check a single candidate relation type against intent."""
    if intent.forbid_free_invent and relation_type not in ALL_TYPED_RELATIONS:
        return False, "free_invent_relation"
    if intent.target_relation_types and relation_type not in intent.target_relation_types:
        return False, "outside_intent_targets"
    return True, "ok"


__all__ = [
    "IntentValidationVerdict",
    "hash_stable_payload",
    "intent_content_hash",
    "constraints_content_hash",
    "validate_extraction_intent",
    "candidate_relation_allowed",
]
