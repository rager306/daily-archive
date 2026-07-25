"""DSPy-like extraction signatures (ADR-029, ADR-033 Step 7).

Five typed signatures that frame the Core-then-Modes extraction pipeline. Like
:mod:`research_graph.infrastructure.evaluation.dspy_extraction`, this module is **DSPy-like,
not DSPy-dependent**: each signature is a frozen dataclass describing the
inputs/outputs of one extraction stage. No DSPy runtime import, no optimizer,
no LLM call — just typed contracts that a future DSPy wiring (ADR-029 §3) or
the M103 S03 prototype can bind to.

The five signatures map to ADR-029 Core-then-Modes + Upgrade modes:

1. :class:`CoreEntitySignature` — Core: extract entities (1 LLM).
2. :class:`BinaryRelationSignature` — Core: binary relations (statistical → 1 LLM).
3. :class:`RelationTypeSignature` — Upgrade: classify into the 27 typed relations.
4. :class:`AbstractEntitySignature` — Upgrade: Module C abstract entities (problem/gap/...).
5. :class:`CitationSignature` — Upgrade: citation/support/contrast (ADR-029 §2.2).

Every output field carries ``schema_version`` alignment via the typed schema
(:mod:`research_graph.domain.schema`) and stays fail-closed
(``import_eligible = False``).
"""

from __future__ import annotations

from dataclasses import dataclass, field

#: Signature family version (ADR-029 extraction boundary, DSPy-like contract).
SIGNATURE_VERSION: str = "extraction.signatures.v1"

#: Default fail-closed safety flags echoed by every signature output contract.
_DEFAULT_SAFETY_FLAGS: dict[str, bool] = field(default_factory=lambda: {"import_eligible": False})


@dataclass(frozen=True)
class _SignatureBase:
    """Common fields for every extraction signature (statistical-first)."""

    signature_version: str = SIGNATURE_VERSION
    #: Statistical-first (ADR-024): every LLM-bound signature declares that it
    #: REQUIRES deterministic statistical grounding before the LLM is invoked.
    requires_statistical_context: bool = True


@dataclass(frozen=True)
class CoreEntitySignature(_SignatureBase):
    """Core: extract typed entities from a chunk given statistical context.

    LLM call #1 of Core-then-Modes (ADR-029). Input: chunk text + YAKE keywords.
    Output: a list of :class:`~research_graph.domain.schema.TypedEntity`
    drafts (entity_type constrained to the closed vocabulary, fail-closed).
    """

    name: str = "core_entity_extraction"
    output_entity_types: tuple[str, ...] = ()  # constrained at call time
    estimated_llm_calls: int = 1


@dataclass(frozen=True)
class BinaryRelationSignature(_SignatureBase):
    """Core: detect binary relations between extracted entities.

    Statistical co-occurrence pre-filters candidates (0 LLM); the LLM confirms
    a small set. Output: candidate (from, to) pairs with a generic relation
    type, upgraded later by :class:`RelationTypeSignature`.
    """

    name: str = "binary_relation_detection"
    estimated_llm_calls: int = 1
    requires_statistical_context: bool = True


@dataclass(frozen=True)
class RelationTypeSignature(_SignatureBase):
    """Upgrade: classify binary relations into the 27 typed relations (ADR-028).

    Only relation types in
    :data:`~research_graph.domain.relation_types.ALL_TYPED_RELATIONS` are
    accepted; unknown proposals are dropped (fail-closed, never coerced).
    """

    name: str = "relation_type_classification"
    allowed_relation_types: tuple[str, ...] = ()  # the 27, injected at bind time
    estimated_llm_calls: int = 1


@dataclass(frozen=True)
class AbstractEntitySignature(_SignatureBase):
    """Upgrade: Module C abstract entities (ADR-028 §2.1).

    Extracts implicit/abstracted concepts (problem, motivation, gap,
    contribution, hypothesis, finding, mechanism, limitation, future_work) that
    distinguish the typed schema from flat scientific extraction.
    """

    name: str = "abstract_entity_extraction"
    abstract_types: tuple[str, ...] = (
        "problem",
        "motivation",
        "gap",
        "contribution",
        "hypothesis",
        "finding",
        "mechanism",
        "limitation",
        "future_work",
    )
    estimated_llm_calls: int = 1


@dataclass(frozen=True)
class CitationSignature(_SignatureBase):
    """Upgrade: citation/support/contrast relations (ADR-029 §2.2).

    ~0.5 LLM per chunk: citations are cheaper because GROBID already provides
    structured reference anchors; the LLM only confirms the citation relation
    type (CITES / SUPPORTS / CONTRASTS / EXTENDS).
    """

    name: str = "citation_relation_extraction"
    citation_relation_types: tuple[str, ...] = ("CITES", "SUPPORTS", "CONTRASTS", "EXTENDS")
    estimated_llm_calls: int = 0  # fractional; rounded to 1 at chunk granularity


#: The five Core-then-Modes signatures in execution order (ADR-029).
ALL_EXTRACTION_SIGNATURES: tuple[type, ...] = (
    CoreEntitySignature,
    BinaryRelationSignature,
    RelationTypeSignature,
    AbstractEntitySignature,
    CitationSignature,
)


__all__ = [
    "SIGNATURE_VERSION",
    "ALL_EXTRACTION_SIGNATURES",
    "AbstractEntitySignature",
    "BinaryRelationSignature",
    "CitationSignature",
    "CoreEntitySignature",
    "RelationTypeSignature",
]
