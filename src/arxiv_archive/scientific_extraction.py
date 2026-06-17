"""Typed scientific extraction draft contracts.

This module is deterministic and local-only. It defines the draft objects that
future extraction, LadybugDB persistence, retrieval, DSPy, and RLM layers must
validate against. It does not call LLMs, create embeddings, or write storage.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from arxiv_archive.evidence import EvidencePath
from research_graph.corpus.parsing.normalization import slugify

SUPPORTED_RELATION_TYPES = frozenset({"supports", "contradicts", "mentions", "uses", "extends"})


@dataclass(frozen=True)
class Claim:
    """A traceable scientific claim draft before graph persistence."""

    id: str
    paper_id: str
    text: str
    claim_type: str
    confidence: float
    evidence_path: EvidencePath | None
    schema_version: str
    extractor_version: str
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ScientificEntity:
    """A traceable scientific entity draft before graph persistence."""

    id: str
    paper_id: str
    label: str
    entity_type: str
    confidence: float
    evidence_path: EvidencePath | None
    schema_version: str
    extractor_version: str
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ScientificRelation:
    """A typed relation draft connecting claim/entity draft IDs."""

    id: str
    paper_id: str
    relation_type: str
    source_id: str
    target_id: str
    confidence: float
    evidence_path: EvidencePath | None
    schema_version: str
    extractor_version: str
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class ExtractionPatch:
    """Storage-ready draft bundle for one paper."""

    paper_id: str
    claims: list[Claim]
    entities: list[ScientificEntity]
    relations: list[ScientificRelation]
    schema_version: str
    extractor_version: str
    validation_warnings: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


def claim_id(paper_id: str, semantic_chunk_id: str, label: str) -> str:
    """Build a deterministic claim ID from paper, evidence chunk, and label."""
    return f"claim:{paper_id}:{_stable_slug(semantic_chunk_id)}:{_stable_slug(label)}"


def entity_id(paper_id: str, label: str) -> str:
    """Build a deterministic entity ID from paper and normalized label."""
    return f"entity:{paper_id}:{_stable_slug(label)}"


def relation_id(paper_id: str, source_id: str, target_id: str, relation_type: str) -> str:
    """Build a deterministic relation ID from endpoints and relation type."""
    return f"relation:{paper_id}:{_stable_slug(source_id)}:{_stable_slug(target_id)}:{_stable_slug(relation_type)}"


def validate_claim(claim: Claim) -> list[str]:
    """Return diagnostics for a claim draft."""
    return _validate_traceable_draft("Claim", claim)


def validate_entity(entity: ScientificEntity) -> list[str]:
    """Return diagnostics for an entity draft."""
    return _validate_traceable_draft("ScientificEntity", entity)


def validate_relation(relation: ScientificRelation) -> list[str]:
    """Return diagnostics for a relation draft, excluding patch endpoint checks."""
    diagnostics = _validate_traceable_draft("Relation", relation)
    if not relation.source_id:
        diagnostics.append(f"Relation {relation.id} is missing source_id")
    if not relation.target_id:
        diagnostics.append(f"Relation {relation.id} is missing target_id")
    if relation.relation_type not in SUPPORTED_RELATION_TYPES:
        diagnostics.append(f"Relation {relation.id} relation_type {relation.relation_type} is unsupported")
    return diagnostics


def validate_extraction_patch(patch: ExtractionPatch) -> list[str]:
    """Return diagnostics for a storage-bound extraction patch."""
    diagnostics: list[str] = []

    if not patch.schema_version:
        diagnostics.append(f"ExtractionPatch {patch.paper_id} is missing schema_version")
    if not patch.extractor_version:
        diagnostics.append(f"ExtractionPatch {patch.paper_id} is missing extractor_version")
    if not patch.provenance:
        diagnostics.append(f"ExtractionPatch {patch.paper_id} is missing provenance")

    draft_ids: set[str] = set()
    for claim in patch.claims:
        diagnostics.extend(validate_claim(claim))
        diagnostics.extend(_validate_duplicate_draft_id(patch.paper_id, draft_ids, claim.id))
        diagnostics.extend(_validate_patch_membership("Claim", claim.id, claim.paper_id, patch.paper_id, claim.evidence_path))

    for entity in patch.entities:
        diagnostics.extend(validate_entity(entity))
        diagnostics.extend(_validate_duplicate_draft_id(patch.paper_id, draft_ids, entity.id))
        diagnostics.extend(
            _validate_patch_membership("ScientificEntity", entity.id, entity.paper_id, patch.paper_id, entity.evidence_path)
        )

    for relation in patch.relations:
        diagnostics.extend(validate_relation(relation))
        diagnostics.extend(_validate_duplicate_draft_id(patch.paper_id, draft_ids, relation.id))
        diagnostics.extend(
            _validate_patch_membership("Relation", relation.id, relation.paper_id, patch.paper_id, relation.evidence_path)
        )
        if relation.source_id not in draft_ids:
            diagnostics.append(
                f"Relation {relation.id} source_id {relation.source_id} does not reference a claim or entity in the patch"
            )
        if relation.target_id not in draft_ids:
            diagnostics.append(
                f"Relation {relation.id} target_id {relation.target_id} does not reference a claim or entity in the patch"
            )

    return diagnostics


def _validate_traceable_draft(kind: str, draft: Claim | ScientificEntity | ScientificRelation) -> list[str]:
    diagnostics: list[str] = []
    if draft.evidence_path is None:
        diagnostics.append(f"{kind} {draft.id} is missing evidence_path")
    elif draft.evidence_path.validation_warnings:
        diagnostics.append(
            f"{kind} {draft.id} evidence_path has validation warnings: "
            + "; ".join(draft.evidence_path.validation_warnings)
        )

    if not 0.0 <= draft.confidence <= 1.0:
        diagnostics.append(f"{kind} {draft.id} confidence {draft.confidence} is outside [0.0, 1.0]")
    if not draft.schema_version:
        diagnostics.append(f"{kind} {draft.id} is missing schema_version")
    if not draft.extractor_version:
        diagnostics.append(f"{kind} {draft.id} is missing extractor_version")
    if not draft.provenance:
        diagnostics.append(f"{kind} {draft.id} is missing provenance")
    if not _has_stable_id_prefix(kind, draft.id):
        diagnostics.append(f"{kind} {draft.id} has an unstable id prefix")
    return diagnostics


def _validate_duplicate_draft_id(patch_paper_id: str, draft_ids: set[str], draft_id: str) -> list[str]:
    if draft_id in draft_ids:
        return [f"ExtractionPatch {patch_paper_id} has duplicate draft id {draft_id}"]
    draft_ids.add(draft_id)
    return []


def _validate_patch_membership(
    kind: str,
    draft_id: str,
    draft_paper_id: str,
    patch_paper_id: str,
    evidence_path: EvidencePath | None,
) -> list[str]:
    diagnostics: list[str] = []
    if draft_paper_id != patch_paper_id:
        diagnostics.append(f"{kind} {draft_id} paper_id {draft_paper_id} does not match patch paper_id {patch_paper_id}")
    if evidence_path is not None and evidence_path.paper_id != patch_paper_id:
        diagnostics.append(
            f"{kind} {draft_id} evidence_path paper_id {evidence_path.paper_id} does not match patch paper_id {patch_paper_id}"
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
    "Claim",
    "ExtractionPatch",
    "ScientificEntity",
    "ScientificRelation",
    "SUPPORTED_RELATION_TYPES",
    "claim_id",
    "entity_id",
    "relation_id",
    "validate_claim",
    "validate_entity",
    "validate_extraction_patch",
    "validate_relation",
]
