"""Typed relation taxonomy for the ADR-028 knowledge schema.

Defines the 27 typed relation types in 5 groups, adapted from Agents-K1 with
quant-mind additions (CONSISTS_OF, SUBSET_OF from TreeKnowledge). These
constants are the canonical edge-type vocabulary for typed extraction
(TypedRelation.relation_type) and downstream typed FalkorDB edges.

This module is deterministic and dependency-free. It defines constants only —
no validation, no I/O, no LLM calls. Typed relation enforcement lives in
``schema.py`` (TypedRelation) and the extraction pipeline.

Groups (ADR-028 §2.2):

* Controlled   (6) — directed domain-control relations
* Causal       (5) — cause / effect relations
* Composition  (5) — structural / part-of relations
* Comparison   (7) — comparison and limitation relations
* Citation     (4) — citation-edge relations
"""

from __future__ import annotations

from types import MappingProxyType
from typing import Final

# ── Group 1: Controlled (6) ──────────────────────────────────────────────────
BUILDS_ON: Final[str] = "BUILDS_ON"
USES_COMPONENT: Final[str] = "USES_COMPONENT"
ALTERNATIVE_TO: Final[str] = "ALTERNATIVE_TO"
SOLVES: Final[str] = "SOLVES"
APPLIED_TO: Final[str] = "APPLIED_TO"
TARGETS: Final[str] = "TARGETS"

# ── Group 2: Causal (5) ──────────────────────────────────────────────────────
CAUSES: Final[str] = "CAUSES"
ENABLES: Final[str] = "ENABLES"
INHIBITS: Final[str] = "INHIBITS"
MODULATES: Final[str] = "MODULATES"
CORRELATED_WITH: Final[str] = "CORRELATED_WITH"

# ── Group 3: Composition (5) ─────────────────────────────────────────────────
USES_TECHNIQUE: Final[str] = "USES_TECHNIQUE"
CONSISTS_OF: Final[str] = "CONSISTS_OF"  # quant-mind TreeKnowledge addition
IMPLEMENTS: Final[str] = "IMPLEMENTS"
COMBINES: Final[str] = "COMBINES"
REQUIRES: Final[str] = "REQUIRES"

# ── Group 4: Comparison (7) ──────────────────────────────────────────────────
DERIVED_FROM: Final[str] = "DERIVED_FROM"
DIFFERS_FROM: Final[str] = "DIFFERS_FROM"
HAS_LIMITATION: Final[str] = "HAS_LIMITATION"
ADDRESSES_PROBLEM: Final[str] = "ADDRESSES_PROBLEM"
MOTIVATED_BY: Final[str] = "MOTIVATED_BY"
HAS_PROPERTY: Final[str] = "HAS_PROPERTY"
SUBSET_OF: Final[str] = "SUBSET_OF"  # quant-mind TreeKnowledge addition

# ── Group 5: Citation (4) ────────────────────────────────────────────────────
CITES: Final[str] = "CITES"
SUPPORTS: Final[str] = "SUPPORTS"
CONTRASTS: Final[str] = "CONTRASTS"
EXTENDS: Final[str] = "EXTENDS"


# Each group is an immutable frozenset for O(1) membership checks and to
# prevent accidental mutation of the canonical taxonomy.
CONTROLLED_RELATIONS: Final[frozenset[str]] = frozenset(
    {BUILDS_ON, USES_COMPONENT, ALTERNATIVE_TO, SOLVES, APPLIED_TO, TARGETS}
)
CAUSAL_RELATIONS: Final[frozenset[str]] = frozenset(
    {CAUSES, ENABLES, INHIBITS, MODULATES, CORRELATED_WITH}
)
COMPOSITION_RELATIONS: Final[frozenset[str]] = frozenset(
    {USES_TECHNIQUE, CONSISTS_OF, IMPLEMENTS, COMBINES, REQUIRES}
)
COMPARISON_RELATIONS: Final[frozenset[str]] = frozenset(
    {
        DERIVED_FROM,
        DIFFERS_FROM,
        HAS_LIMITATION,
        ADDRESSES_PROBLEM,
        MOTIVATED_BY,
        HAS_PROPERTY,
        SUBSET_OF,
    }
)
CITATION_RELATIONS: Final[frozenset[str]] = frozenset({CITES, SUPPORTS, CONTRASTS, EXTENDS})


def _build_all_typed_relations() -> frozenset[str]:
    """Union of all typed relation groups; sized at build time."""
    return frozenset(
        CONTROLLED_RELATIONS
        | CAUSAL_RELATIONS
        | COMPOSITION_RELATIONS
        | COMPARISON_RELATIONS
        | CITATION_RELATIONS
    )


# The complete typed relation vocabulary. 6 + 5 + 5 + 7 + 4 = 27.
ALL_TYPED_RELATIONS: Final[frozenset[str]] = _build_all_typed_relations()


# Group name → member set. MappingProxyType makes the registry read-only at
# runtime while still allowing keyed lookup (e.g. for diagnostics / reporting).
TYPED_RELATION_GROUPS: Final[MappingProxyType[str, frozenset[str]]] = MappingProxyType(
    {
        "controlled": CONTROLLED_RELATIONS,
        "causal": CAUSAL_RELATIONS,
        "composition": COMPOSITION_RELATIONS,
        "comparison": COMPARISON_RELATIONS,
        "citation": CITATION_RELATIONS,
    }
)


def is_typed_relation(value: str) -> bool:
    """Return True when ``value`` is one of the 27 canonical typed relations."""
    return value in ALL_TYPED_RELATIONS


def group_of(relation_type: str) -> str | None:
    """Return the group name for a typed relation, or None if unknown."""
    for name, members in TYPED_RELATION_GROUPS.items():
        if relation_type in members:
            return name
    return None


__all__ = [
    # Individual constants
    "BUILDS_ON",
    "USES_COMPONENT",
    "ALTERNATIVE_TO",
    "SOLVES",
    "APPLIED_TO",
    "TARGETS",
    "CAUSES",
    "ENABLES",
    "INHIBITS",
    "MODULATES",
    "CORRELATED_WITH",
    "USES_TECHNIQUE",
    "CONSISTS_OF",
    "IMPLEMENTS",
    "COMBINES",
    "REQUIRES",
    "DERIVED_FROM",
    "DIFFERS_FROM",
    "HAS_LIMITATION",
    "ADDRESSES_PROBLEM",
    "MOTIVATED_BY",
    "HAS_PROPERTY",
    "SUBSET_OF",
    "CITES",
    "SUPPORTS",
    "CONTRASTS",
    "EXTENDS",
    # Group sets
    "CONTROLLED_RELATIONS",
    "CAUSAL_RELATIONS",
    "COMPOSITION_RELATIONS",
    "COMPARISON_RELATIONS",
    "CITATION_RELATIONS",
    # Aggregate
    "ALL_TYPED_RELATIONS",
    "TYPED_RELATION_GROUPS",
    # Helpers
    "is_typed_relation",
    "group_of",
]
