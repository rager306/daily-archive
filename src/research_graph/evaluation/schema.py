"""Typed knowledge schema (ADR-028 + ADR-033 Steps 1-2).

This module is the canonical home for the typed knowledge schema. It defines
the evolved entity / relation / abstract / card types that replace the flat
``ScientificEntity`` / ``ScientificRelation`` / ``ExtractionPatch`` drafts
previously defined inline in ``scientific_extraction.py``.

Design rules (ADR-033):

* **Schema evolution, not duplication.** ``scientific_extraction.py`` imports
  from here and provides backward-compatible aliases. The types ARE the new
  types; there is no adapter / converter layer.
* **stdlib dataclasses** (``@dataclass(frozen=True)``). No Pydantic for
  pipeline types.
* **safety_flags fail-closed.** Every extracted draft carries
  ``safety_flags`` defaulting to ``{"import_eligible": False}``. Typed
  extraction is a specification, not authorization for graph writes.
* **Adaptix boundary** is not part of this module. Adaptix (LLM JSON -> typed
  dataclass) is wired in later extraction primitives (ADR-033 Step 4).

This module is deterministic and local-only: no LLMs, no embeddings, no
storage writes.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from types import MappingProxyType
from typing import Final

from research_graph.evaluation.relation_types import ALL_TYPED_RELATIONS
from research_graph.papers.semantic_chunks import EvidencePath

#: Schema version stamp carried by every evolved typed draft. ADR-033 §2.2
#: renamed the lineage from ``scientific-extraction.v1`` to ``typed.v1``.
SCHEMA_VERSION: Final[str] = "typed.v1"

#: Default safety-flags bundle for all extracted drafts. Fail-closed: typed
#: drafts are candidates, not import-authorized facts (ADR-028 §5).
DEFAULT_SAFETY_FLAGS: Final[MappingProxyType[str, bool]] = MappingProxyType(
    {"import_eligible": False}
)


def _default_safety_flags() -> dict[str, bool]:
    """Factory for per-instance mutable safety_flags copies (frozen dataclass-safe)."""
    return {"import_eligible": False}


# ── Module C abstract entity types (ADR-028 §2.1) ────────────────────────────
# These are the implicit/abstracted concepts that distinguish the typed schema
# from the flat scientific-extraction draft. Each becomes a TypedEntity.entity_type.
ABSTRACT_ENTITY_TYPES: Final[frozenset[str]] = frozenset(
    {
        "problem",
        "motivation",
        "gap",
        "contribution",
        "hypothesis",
        "finding",
        "mechanism",
        "limitation",
        "future_work",
    }
)

# Module B concrete entity types (a representative subset; not exhaustive here).
CONCRETE_ENTITY_TYPES: Final[frozenset[str]] = frozenset(
    {
        "method",
        "dataset",
        "metric",
        "task",
        "baseline",
        "implementation",
        "theorem",
        "definition",
        "figure",
        "table",
        "equation",
        "concept",
        "example",
        "exercise",
        "code_component",
        "api",
        "configuration",
    }
)

# Module A source/author/venue/resource kinds.
SOURCE_KINDS: Final[frozenset[str]] = frozenset({"paper", "textbook", "code_repo", "dataset", "tech_doc"})

#: All recognized entity_type values. TypedEntity validation constrains
#: entity_type to this set (closed vocabulary per ADR-028 §2.1).
CURRENT_SCHEMA_ENTITY_TYPES: Final[frozenset[str]] = frozenset(
    ABSTRACT_ENTITY_TYPES | CONCRETE_ENTITY_TYPES
)


@dataclass(frozen=True)
class ExtractionRef:
    """Typed provenance for one extraction act (quant-mind ExtractionRef).

    Captures the deterministic flow / model / prompt-hash triple that produced
    a draft, enabling audit without carrying article text or binary payloads.
    """

    flow: str
    model: str
    prompt_hash: str


@dataclass(frozen=True)
class TypedEntity:
    """Evolved typed entity draft (was ``ScientificEntity``).

    Field renames vs the legacy flat draft (ADR-033 §2.2):

    * ``id``          -> ``entity_id``
    * ``paper_id``    -> ``source_id`` (now universal across domains)
    * ``label``       -> ``canonical_name``
    * ``extractor_ref`` is NEW typed provenance
    * ``safety_flags`` is NEW fail-closed import eligibility

    ``entity_type`` is constrained to :data:`CURRENT_SCHEMA_ENTITY_TYPES`.
    """

    entity_id: str
    source_id: str
    entity_type: str
    canonical_name: str
    confidence: float
    evidence_path: EvidencePath | None
    extractor_version: str
    schema_version: str = SCHEMA_VERSION
    extractor_ref: ExtractionRef | None = None
    safety_flags: dict[str, bool] = field(default_factory=_default_safety_flags)
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class TypedRelation:
    """Evolved typed relation draft (was ``ScientificRelation``).

    Field renames vs the legacy flat draft (ADR-033 §2.2):

    * ``id``        -> ``relation_id``
    * ``paper_id``  -> ``source_id``
    * ``source_id`` -> ``from_entity_id``
    * ``target_id`` -> ``to_entity_id``

    ``relation_type`` is constrained to one of the 27 typed relations
    (:data:`research_graph.evaluation.relation_types.ALL_TYPED_RELATIONS`).
    """

    relation_id: str
    source_id: str
    relation_type: str
    from_entity_id: str
    to_entity_id: str
    confidence: float
    evidence_path: EvidencePath | None
    extractor_version: str
    schema_version: str = SCHEMA_VERSION
    extractor_ref: ExtractionRef | None = None
    safety_flags: dict[str, bool] = field(default_factory=_default_safety_flags)
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class AbstractEntity:
    """Module C abstracted concept (ADR-028 §2.1) — the key differentiator.

    Abstract entities capture implicit/abstracted concepts (problem,
    motivation, gap, contribution, hypothesis, finding, mechanism, limitation,
    future_work) that the flat scientific-extraction schema could not express.
    They are distinct from concrete TypedEntity instances to preserve the
    Module C semantics in typed queries and review gates.
    """

    abstract_id: str
    source_id: str
    abstract_type: str
    canonical_name: str
    statement: str
    confidence: float
    evidence_path: EvidencePath | None
    extractor_version: str
    schema_version: str = SCHEMA_VERSION
    extractor_ref: ExtractionRef | None = None
    safety_flags: dict[str, bool] = field(default_factory=_default_safety_flags)
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class KnowledgeCard:
    """Distilled paper knowledge card (quant-mind PaperKnowledgeCard pattern).

    Aggregates the structured summary of a single source: methodology,
    findings, and limitations. This is a distilled view, not a raw extraction;
    it is composed from typed entities / abstracts during post-extraction
    synthesis.
    """

    card_id: str
    source_id: str
    title: str
    methodology: list[str]
    findings: list[str]
    limitations: list[str]
    schema_version: str = SCHEMA_VERSION
    safety_flags: dict[str, bool] = field(default_factory=_default_safety_flags)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class Claim:
    """A traceable scientific claim draft before graph persistence.

    Evolved from the legacy flat Claim: carries typed schema_version and
    fail-closed safety_flags. Field names are preserved for the Claim lineage
    (no rename mandated by ADR-033 §2.2's entity/relation illustration).
    """

    claim_id: str
    source_id: str
    text: str
    claim_type: str
    confidence: float
    evidence_path: EvidencePath | None
    extractor_version: str
    schema_version: str = SCHEMA_VERSION
    extractor_ref: ExtractionRef | None = None
    safety_flags: dict[str, bool] = field(default_factory=_default_safety_flags)
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ExtractionPatch:
    """Storage-ready typed draft bundle for one source.

    Evolved from the legacy flat ExtractionPatch. The bundle is keyed by
    ``source_id`` (was ``paper_id``) and now carries typed claims / entities /
    relations plus the new abstract entities and knowledge cards.
    """

    source_id: str
    claims: list[Claim]
    entities: list[TypedEntity]
    relations: list[TypedRelation]
    abstracts: list[AbstractEntity] = field(default_factory=list)
    knowledge_cards: list[KnowledgeCard] = field(default_factory=list)
    schema_version: str = SCHEMA_VERSION
    extractor_version: str = ""
    safety_flags: dict[str, bool] = field(default_factory=_default_safety_flags)
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


# ── Schema validation helpers ────────────────────────────────────────────────
def is_known_entity_type(entity_type: str) -> bool:
    """Return True when ``entity_type`` is in the closed typed entity vocabulary."""
    return entity_type in CURRENT_SCHEMA_ENTITY_TYPES


def is_known_abstract_type(abstract_type: str) -> bool:
    """Return True when ``abstract_type`` is one of the Module C abstract types."""
    return abstract_type in ABSTRACT_ENTITY_TYPES


def is_known_relation_type(relation_type: str) -> bool:
    """Return True when ``relation_type`` is one of the 27 typed relations."""
    return relation_type in ALL_TYPED_RELATIONS


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_SAFETY_FLAGS",
    "ABSTRACT_ENTITY_TYPES",
    "CONCRETE_ENTITY_TYPES",
    "SOURCE_KINDS",
    "CURRENT_SCHEMA_ENTITY_TYPES",
    "ExtractionRef",
    "TypedEntity",
    "TypedRelation",
    "AbstractEntity",
    "KnowledgeCard",
    "Claim",
    "ExtractionPatch",
    "is_known_entity_type",
    "is_known_abstract_type",
    "is_known_relation_type",
]
