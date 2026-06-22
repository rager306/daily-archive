# Formerly: src/arxiv_archive/scientific_extraction.py

"""Typed scientific extraction draft contracts (evolved).

This module is deterministic and local-only. It defines the draft objects that
future extraction, LadybugDB persistence, retrieval, DSPy, and RLM layers must
validate against. It does not call LLMs, create embeddings, or write storage.

ADR-028 + ADR-033 §2.2 — schema evolution, not duplication
---------------------------------------------------------

The flat ``ScientificEntity`` / ``ScientificRelation`` / ``ExtractionPatch``
drafts previously defined inline here now **evolve** into their typed forms
defined canonically in :mod:`research_graph.domain.schema`. This module:

* re-exports the typed types under backward-compatible names
  (``ScientificEntity = TypedEntity``, ``ScientificRelation = TypedRelation``);
* keeps the legacy ID-builder helpers (``claim_id``, ``entity_id``,
  ``relation_id``) and the validation surface (``validate_*``) so existing
  callers import unchanged;
* widens ``SUPPORTED_RELATION_TYPES`` to the 27 typed relations
  (:data:`research_graph.domain.relation_types.ALL_TYPED_RELATIONS`).

The types ARE the new typed types — there is no adapter / converter layer.
``schema_version`` advances to ``typed.v1``.
"""

from __future__ import annotations

from research_graph.corpus.parsing.normalization import slugify
from research_graph.domain.relation_types import ALL_TYPED_RELATIONS, is_typed_relation
from research_graph.domain.schema import (
    SCHEMA_VERSION,
    AbstractEntity,
    Claim,
    ExtractionPatch,
    ExtractionRef,
    KnowledgeCard,
    TypedEntity,
    TypedRelation,
)

# Backward-compatible type aliases. The names ``ScientificEntity`` and
# ``ScientificRelation`` remain importable; they now resolve to the typed
# evolved dataclasses (ADR-033 §2.2: "the types ARE the new types").
ScientificEntity = TypedEntity
ScientificRelation = TypedRelation

# Widened to the 27 typed relations. Legacy lowercase forms ("supports" etc.)
# are no longer the canonical vocabulary — typed relations are UPPERCASE.
SUPPORTED_RELATION_TYPES = ALL_TYPED_RELATIONS


def claim_id(source_id: str, semantic_chunk_id: str, label: str) -> str:
    """Build a deterministic claim ID from source, evidence chunk, and label."""
    return f"claim:{source_id}:{_stable_slug(semantic_chunk_id)}:{_stable_slug(label)}"


def entity_id(source_id: str, label: str) -> str:
    """Build a deterministic entity ID from source and normalized label."""
    return f"entity:{source_id}:{_stable_slug(label)}"


def relation_id(
    source_id: str, from_entity_id: str, to_entity_id: str, relation_type: str
) -> str:
    """Build a deterministic relation ID from endpoints and relation type."""
    return (
        f"relation:{source_id}:{_stable_slug(from_entity_id)}:"
        f"{_stable_slug(to_entity_id)}:{_stable_slug(relation_type)}"
    )


def validate_claim(claim: Claim) -> list[str]:
    """Return diagnostics for a claim draft."""
    return _validate_traceable_draft("Claim", claim, claim.claim_id)


def validate_entity(entity: ScientificEntity) -> list[str]:
    """Return diagnostics for an entity draft."""
    return _validate_traceable_draft("ScientificEntity", entity, entity.entity_id)


def validate_relation(relation: ScientificRelation) -> list[str]:
    """Return diagnostics for a relation draft, excluding patch endpoint checks."""
    diagnostics = _validate_traceable_draft("Relation", relation, relation.relation_id)
    if not relation.from_entity_id:
        diagnostics.append(f"Relation {relation.relation_id} is missing from_entity_id")
    if not relation.to_entity_id:
        diagnostics.append(f"Relation {relation.relation_id} is missing to_entity_id")
    if not is_typed_relation(relation.relation_type):
        diagnostics.append(
            f"Relation {relation.relation_id} relation_type {relation.relation_type} is unsupported"
        )
    return diagnostics


def validate_extraction_patch(patch: ExtractionPatch) -> list[str]:
    """Return diagnostics for a storage-bound extraction patch."""
    diagnostics: list[str] = []

    if not patch.schema_version:
        diagnostics.append(f"ExtractionPatch {patch.source_id} is missing schema_version")
    if not patch.extractor_version:
        diagnostics.append(f"ExtractionPatch {patch.source_id} is missing extractor_version")
    if not patch.provenance:
        diagnostics.append(f"ExtractionPatch {patch.source_id} is missing provenance")

    draft_ids: set[str] = set()
    for claim in patch.claims:
        diagnostics.extend(validate_claim(claim))
        diagnostics.extend(_validate_duplicate_draft_id(patch.source_id, draft_ids, claim.claim_id))
        diagnostics.extend(
            _validate_patch_membership(
                "Claim", claim.claim_id, claim.source_id, patch.source_id, claim.evidence_path
            )
        )

    for entity in patch.entities:
        diagnostics.extend(validate_entity(entity))
        diagnostics.extend(_validate_duplicate_draft_id(patch.source_id, draft_ids, entity.entity_id))
        diagnostics.extend(
            _validate_patch_membership(
                "ScientificEntity",
                entity.entity_id,
                entity.source_id,
                patch.source_id,
                entity.evidence_path,
            )
        )

    for relation in patch.relations:
        diagnostics.extend(validate_relation(relation))
        diagnostics.extend(
            _validate_duplicate_draft_id(patch.source_id, draft_ids, relation.relation_id)
        )
        diagnostics.extend(
            _validate_patch_membership(
                "Relation",
                relation.relation_id,
                relation.source_id,
                patch.source_id,
                relation.evidence_path,
            )
        )
        if relation.from_entity_id not in draft_ids:
            diagnostics.append(
                f"Relation {relation.relation_id} from_entity_id {relation.from_entity_id} "
                "does not reference a claim or entity in the patch"
            )
        if relation.to_entity_id not in draft_ids:
            diagnostics.append(
                f"Relation {relation.relation_id} to_entity_id {relation.to_entity_id} "
                "does not reference a claim or entity in the patch"
            )

    return diagnostics


def _validate_traceable_draft(
    kind: str,
    draft: Claim | ScientificEntity | ScientificRelation,
    draft_id: str,
) -> list[str]:
    diagnostics: list[str] = []
    if draft.evidence_path is None:
        diagnostics.append(f"{kind} {draft_id} is missing evidence_path")
    elif draft.evidence_path.validation_warnings:
        diagnostics.append(
            f"{kind} {draft_id} evidence_path has validation warnings: "
            + "; ".join(draft.evidence_path.validation_warnings)
        )

    if not 0.0 <= draft.confidence <= 1.0:
        diagnostics.append(f"{kind} {draft_id} confidence {draft.confidence} is outside [0.0, 1.0]")
    if not draft.schema_version:
        diagnostics.append(f"{kind} {draft_id} is missing schema_version")
    if not draft.extractor_version:
        diagnostics.append(f"{kind} {draft_id} is missing extractor_version")
    if not draft.provenance:
        diagnostics.append(f"{kind} {draft_id} is missing provenance")
    if not _has_stable_id_prefix(kind, draft_id):
        diagnostics.append(f"{kind} {draft_id} has an unstable id prefix")
    return diagnostics


def _validate_duplicate_draft_id(
    patch_source_id: str, draft_ids: set[str], draft_id: str
) -> list[str]:
    if draft_id in draft_ids:
        return [f"ExtractionPatch {patch_source_id} has duplicate draft id {draft_id}"]
    draft_ids.add(draft_id)
    return []


def _validate_patch_membership(
    kind: str,
    draft_id: str,
    draft_source_id: str,
    patch_source_id: str,
    evidence_path,
) -> list[str]:
    diagnostics: list[str] = []
    if draft_source_id != patch_source_id:
        diagnostics.append(
            f"{kind} {draft_id} source_id {draft_source_id} does not match patch source_id {patch_source_id}"
        )
    if evidence_path is not None and evidence_path.paper_id != patch_source_id:
        diagnostics.append(
            f"{kind} {draft_id} evidence_path paper_id {evidence_path.paper_id} "
            f"does not match patch source_id {patch_source_id}"
        )
    return diagnostics


def _has_stable_id_prefix(kind: str, draft_id: str) -> bool:
    expected_prefix = {
        "Claim": "claim:",
        "ScientificEntity": "entity:",
        "Relation": "relation:",
    }.get(kind)
    return expected_prefix is None or draft_id.startswith(expected_prefix)


def _stable_slug(value: str) -> str:
    return slugify(value).replace("section", "unknown") if not value.strip() else slugify(value)


__all__ = [
    "AbstractEntity",
    "Claim",
    "ExtractionPatch",
    "ExtractionRef",
    "KnowledgeCard",
    "SCHEMA_VERSION",
    "SUPPORTED_RELATION_TYPES",
    "ScientificEntity",
    "ScientificRelation",
    "claim_id",
    "entity_id",
    "relation_id",
    "validate_claim",
    "validate_entity",
    "validate_extraction_patch",
    "validate_relation",
]
