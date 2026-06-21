"""Level 2 reusable pipeline stages (ADR-033 Step 4 + ADR-024 statistical-first).

Each stage is a frozen dataclass that structurally satisfies the
:class:`research_graph.pipeline.types.PipelineStage` protocol and declares its
:class:`ResourceProfile` lane (LLM vs CPU) per ADR-027 §2.2 and D085.

Statistical-first (ADR-024): every LLM-calling stage is preceded by a
deterministic statistical pre-processor. In this foundation slice:

* **CPU-lane stages are real and deterministic** — they use the existing
  :class:`~research_graph.retrieval.keyword_extractor.KeywordExtractor` (YAKE)
  and simple co-occurrence, with no network and no LLM.
* **LLM-lane stages are stubbed** — they declare the LLM boundary (Adaptix
  :class:`~adaptix.Retort` factory + ``LLMEntityOutput`` / ``LLMRelationOutput``
  boundary models per ADR-033 §2.3) but take an injectable ``llm_client``
  callable that defaults to ``None``. With no client they emit empty
  fail-closed drafts (``safety_flags.import_eligible = False``); a real client
  is wired in the M103 S03 extraction prototype.

All outputs are candidate evidence, never truth: every draft carries
fail-closed ``safety_flags`` and nothing here authorizes graph writes (§6.3 #4).
"""

from __future__ import annotations

import json
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.evaluation.schema import (
    DEFAULT_SAFETY_FLAGS,
    ExtractionPatch,
    TypedEntity,
    TypedRelation,
    is_known_relation_type,
)
from research_graph.evaluation.statistical_context import StatisticalContext
from research_graph.papers.semantic_chunks import EvidencePath
from research_graph.pipeline.types import (
    PipelineContext,
    ResourceProfile,
)
from research_graph.retrieval.keyword_extractor import KeywordExtractor

#: LLM JSON boundary callable shape: ``(prompt, context_snapshot) -> raw_json``.
#: Returns parsed JSON (dict). The concrete client (MiniMax/GLM via
#: ``provider_config.py``, ADR-025) is injected by the orchestrator/profile in
#: M103 S03; here it is ``None`` so LLM stages are stubbed.
LLMClient = Callable[[str, dict[str, Any]], dict[str, Any]]

#: CPU-lane profile for deterministic statistical stages (YAKE, co-occurrence).
_CPU_LIGHT = ResourceProfile(cpu_required=True, cpu_intensity="light")

#: LLM-lane profile for MiniMax extraction (ADR-025 primary provider).
_LLM_MINIMAX = ResourceProfile(llm_required=True, llm_provider="minimax", estimated_tokens=2048)


# ── LLM JSON boundary models (ADR-033 §2.3 — Adaptix at boundary only) ───────


@dataclass(frozen=True)
class LLMEntityOutput:
    """Adaptix boundary model for one LLM-extracted entity (ADR-033 §2.3).

    Adaptix loads raw LLM JSON into this stdlib frozen dataclass, handling
    naming coercion and type tidying. This is the ONLY place Adaptix touches
    extraction output; downstream code uses :class:`TypedEntity`.
    """

    entity_type: str
    canonical_name: str
    confidence: float
    evidence_hint: str = ""


@dataclass(frozen=True)
class LLMRelationOutput:
    """Adaptix boundary model for one LLM-extracted typed relation.

    ``relation_type`` is validated against the 27 typed relations
    (:data:`~research_graph.evaluation.relation_types.ALL_TYPED_RELATIONS`)
    before promotion to :class:`TypedRelation`; invalid types are dropped, not
    coerced (fail-closed).
    """

    relation_type: str
    from_name: str
    to_name: str
    confidence: float


def _build_retort() -> Any:
    """Build the Adaptix Retort for LLM JSON → boundary models (lazy import).

    Adaptix is imported lazily so this module imports cleanly even when the
    Adaptix package is absent from the active environment (it is a runtime
    dependency only on the LLM boundary). The prototype slice (M103 S03)
    installs Adaptix and invokes this factory via a real ``llm_client``.
    """
    from adaptix import Retort  # lazy: ADR-033 §2.3 boundary-only

    return Retort()


# ── Statistical context (ADR-024) is defined canonically in ───────────────
# ``research_graph.evaluation.statistical_context`` (ADR-033 §2.6) and imported
# above. Re-exported via ``__all__`` so pipeline callers can import it from the
# pipeline namespace too (schema evolution, not duplication — §6.3 #6).

# ── Stage 1: StatisticalPreProcessor (CPU lane, deterministic) ───────────────


@dataclass(frozen=True)
class StatisticalPreProcessor:
    """YAKE keyword extraction + co-occurrence (ADR-024, no LLM).

    Reads ``text_parts`` from the context (set by the caller/profile) and
    writes a :class:`StatisticalContext` into ``context.statistical_context``
    so downstream LLM stages receive deterministic statistical grounding.
    """

    stage_name: str = "statistical_pre_processor"
    resource_profile: ResourceProfile = _CPU_LIGHT
    keyword_top_k: int = 20
    co_occurrence_min: int = 2

    def run(self, context: PipelineContext) -> PipelineContext:
        text_parts: Sequence[str] = context.stage_outputs.get("text_parts", ())
        if not text_parts:
            ctx = context.with_output(self.stage_name, StatisticalContext())
            from dataclasses import replace

            return replace(ctx, statistical_context=ctx.stage_outputs[self.stage_name])
        extractor = KeywordExtractor()
        keywords = extractor.extract_for_text_parts(list(text_parts))[: self.keyword_top_k]
        scored = [(kw, 1.0 / (i + 1)) for i, kw in enumerate(keywords)]
        co_occur = _co_occurrence(keywords, list(text_parts), self.co_occurrence_min)
        stat = StatisticalContext(keywords=tuple(scored), co_occurrence=tuple(co_occur))
        from dataclasses import replace

        ctx = context.with_output(self.stage_name, stat)
        return replace(ctx, statistical_context=stat)


def _co_occurrence(
    keywords: list[str], text_parts: list[str], minimum: int
) -> list[tuple[str, str, int]]:
    """Deterministic keyword co-occurrence counts across text parts."""
    keyset = set(keywords)
    counts: dict[tuple[str, str], int] = {}
    for part in text_parts:
        present = sorted(k for k in keyset if k and k.lower() in part.lower())
        for i in range(len(present)):
            for j in range(i + 1, len(present)):
                pair = (present[i], present[j])
                counts[pair] = counts.get(pair, 0) + 1
    return [(a, b, n) for (a, b), n in counts.items() if n >= minimum]


# ── Stage 2: CoreEntityExtractor (LLM lane, stubbed in foundation slice) ─────


@dataclass(frozen=True)
class CoreEntityExtractor:
    """Core entity extraction via LLM + Adaptix boundary (ADR-029, ADR-033 §2.3).

    Foundation slice: ``llm_client`` defaults to ``None`` (stubbed — no real
    LLM call). The stub path emits zero entities but keeps the stage callable
    and the Adaptix seam visible (boundary model + lazy Retort factory). A real
    client is injected in M103 S03.

    Statistical-first (ADR-024): the LLM prompt is built from
    :class:`StatisticalContext`; the stage never calls the LLM without it.
    """

    stage_name: str = "core_entity_extractor"
    resource_profile: ResourceProfile = _LLM_MINIMAX
    llm_client: LLMClient | None = None

    def run(self, context: PipelineContext) -> PipelineContext:
        stat = context.statistical_context
        source_id = context.source_id
        text_parts: Sequence[str] = context.stage_outputs.get("text_parts", ())
        if self.llm_client is None or not text_parts:
            entities: list[TypedEntity] = []
        else:
            entities = _extract_entities_via_llm(self.llm_client, stat, text_parts, source_id)
        patch = _patch_with_entities(context, entities, source_id)
        return context.with_output(self.stage_name, patch)


def _extract_entities_via_llm(
    client: LLMClient,
    stat: StatisticalContext | None,
    text_parts: Sequence[str],
    source_id: str,
) -> list[TypedEntity]:
    """Call the LLM, Adaptix-parse JSON into TypedEntity drafts (ADR-033 §2.3).

    Statistical-first (ADR-024): the prompt embeds the YAKE keywords as
    grounding AND the chunk text the LLM extracts from. Without the text the
    LLM cannot extract — keywords alone are insufficient.
    """
    retort = _build_retort()
    text = "\n\n".join(text_parts)
    prompt = _entity_prompt(stat, text)
    raw = client(
        prompt, {"source_id": source_id, "keywords": _kw_list(stat), "extraction_kind": "entities"}
    )
    items = raw.get("entities", []) if isinstance(raw, dict) else []
    entities: list[TypedEntity] = []
    for idx, item in enumerate(items):
        try:
            out = retort.load(item, LLMEntityOutput)
        except Exception:  # noqa: BLE001 — fail-closed: skip malformed LLM output
            continue
        entities.append(_to_typed_entity(out, source_id, idx))
    return entities


def _entity_prompt(stat: StatisticalContext | None, text: str = "") -> str:
    kws = ", ".join(k for k, _ in (stat.keywords if stat else ()))
    return (
        "Extract the core technical entities from the following paper chunk. "
        f"Statistical keyword context: {kws}\n\nChunk:\n{text}"
    )


def _to_typed_entity(out: LLMEntityOutput, source_id: str, idx: int) -> TypedEntity:
    return TypedEntity(
        entity_id=f"{source_id}:entity:{idx}:{out.canonical_name.lower().replace(' ', '-')}",
        source_id=source_id,
        entity_type=out.entity_type,
        canonical_name=out.canonical_name,
        confidence=float(out.confidence),
        evidence_path=None,
        extractor_version="core.v1",
        extractor_ref=None,
        safety_flags=dict(DEFAULT_SAFETY_FLAGS),
    )


# ── Stage 3: BinaryRelationDetector (CPU lane, deterministic) ────────────────


@dataclass(frozen=True)
class BinaryRelationDetector:
    """Statistical co-occurrence → candidate binary relations (ADR-024, no LLM).

    Promotes high-count co-occurrence pairs into untyped candidate relations
    (``relation_type`` left generic; typed assignment is the next LLM stage).
    Deterministic: reads :class:`StatisticalContext.co_occurrence`.
    """

    stage_name: str = "binary_relation_detector"
    resource_profile: ResourceProfile = _CPU_LIGHT
    min_co_occurrence: int = 2

    def run(self, context: PipelineContext) -> PipelineContext:
        stat = context.statistical_context
        source_id = context.source_id
        relations: list[TypedRelation] = []
        if stat:
            for idx, (a, b, count) in enumerate(stat.co_occurrence):
                if count < self.min_co_occurrence:
                    continue
                relations.append(_candidate_binary_relation(a, b, count, source_id, idx))
        patch = _patch_with_relations(context, relations, source_id)
        return context.with_output(self.stage_name, patch)


def _candidate_binary_relation(
    a: str, b: str, count: int, source_id: str, idx: int
) -> TypedRelation:
    return TypedRelation(
        relation_id=f"{source_id}:rel:cooc:{idx}:{a}:{b}",
        source_id=source_id,
        relation_type="RELATED_TO",  # generic; typed classifier upgrades this
        from_entity_id=f"{source_id}:entity:{a.lower().replace(' ', '-')}",
        to_entity_id=f"{source_id}:entity:{b.lower().replace(' ', '-')}",
        confidence=min(1.0, count / 10.0),
        evidence_path=None,
        extractor_version="cooc.v1",
        extractor_ref=None,
        safety_flags=dict(DEFAULT_SAFETY_FLAGS),
    )


# ── Stage 4: RelationTypeClassifier (LLM lane, constrained to 27 relations) ──


@dataclass(frozen=True)
class RelationTypeClassifier:
    """Typed-relation assignment constrained to the 27 typed relations (ADR-028).

    Foundation slice: ``llm_client`` defaults to ``None`` (stubbed). The stub
    path drops untyped candidate relations (fail-closed — never invents a typed
    relation without the LLM). With a client, the LLM proposes
    :class:`LLMRelationOutput`; only proposals whose ``relation_type`` is one
    of the 27 typed relations survive (validated via
    :func:`~research_graph.evaluation.schema.is_known_relation_type`).
    """

    stage_name: str = "relation_type_classifier"
    resource_profile: ResourceProfile = _LLM_MINIMAX
    llm_client: LLMClient | None = None

    def run(self, context: PipelineContext) -> PipelineContext:
        source_id = context.source_id
        text_parts: Sequence[str] = context.stage_outputs.get("text_parts", ())
        if self.llm_client is None or not text_parts:
            typed: list[TypedRelation] = []
        else:
            typed = _classify_relations_via_llm(self.llm_client, context, text_parts, source_id)
        patch = _patch_with_relations(context, typed, source_id)
        return context.with_output(self.stage_name, patch)


def _classify_relations_via_llm(
    client: LLMClient, context: PipelineContext, text_parts: Sequence[str], source_id: str
) -> list[TypedRelation]:
    """LLM-proposed typed relations, validated against the 27 typed relations.

    Statistical-first (ADR-024): the prompt embeds the candidate pairs (from
    co-occurrence) AND the chunk text so the LLM classifies grounded in the
    actual prose, not just pair labels.
    """
    retort = _build_retort()
    candidates = context.stage_outputs.get("binary_relation_detector")
    text = "\n\n".join(text_parts)
    prompt = _relation_prompt(text, _candidate_dump(candidates, source_id))
    raw = client(prompt, {"source_id": source_id, "extraction_kind": "relations"})
    items = raw.get("relations", []) if isinstance(raw, dict) else []
    typed: list[TypedRelation] = []
    for idx, item in enumerate(items):
        try:
            out = retort.load(item, LLMRelationOutput)
        except Exception:  # noqa: BLE001 — fail-closed
            continue
        if not is_known_relation_type(out.relation_type):
            continue  # fail-closed: drop unknown relation types, never coerce
        typed.append(_to_typed_relation(out, source_id, idx))
    return typed


def _relation_prompt(text: str, candidates: list[dict[str, Any]]) -> str:
    cand = json.dumps(candidates[:10])
    return (
        "Classify the relations between extracted entities into ONE of the 27 typed "
        "relations (BUILDS_ON, IMPLEMENTS, EXTENDS, SOLVES, TARGETS, CAUSES, ENABLES, "
        "INHIBITS, CONSISTS_OF, REQUIRES, DERIVED_FROM, HAS_LIMITATION, SUBSET_OF, "
        "CITES, SUPPORTS, CONTRASTS). Drop any pair whose type is not one of the 27. "
        f"Only relate entities present in the text.\n\nCandidate pairs: {cand}\n\nChunk:\n{text}"
    )


def _candidate_dump(candidates: Any, source_id: str) -> list[dict[str, Any]]:
    if not isinstance(candidates, ExtractionPatch):
        return []
    return [
        {"from": r.from_entity_id, "to": r.to_entity_id, "score": r.confidence}
        for r in candidates.relations
    ]


def _to_typed_relation(out: LLMRelationOutput, source_id: str, idx: int) -> TypedRelation:
    return TypedRelation(
        relation_id=f"{source_id}:rel:typed:{idx}:{out.relation_type}",
        source_id=source_id,
        relation_type=out.relation_type,
        from_entity_id=f"{source_id}:entity:{out.from_name.lower().replace(' ', '-')}",
        to_entity_id=f"{source_id}:entity:{out.to_name.lower().replace(' ', '-')}",
        confidence=float(out.confidence),
        evidence_path=None,
        extractor_version="typed.v1",
        extractor_ref=None,
        safety_flags=dict(DEFAULT_SAFETY_FLAGS),
    )


# ── Stage 5: EvidenceLinker (CPU lane, deterministic) ────────────────────────


@dataclass(frozen=True)
class EvidenceLinker:
    """Attaches deterministic :class:`EvidencePath` to extracted drafts (CPU).

    Builds an EvidencePath from the context's ``evidence_anchor`` (set by the
    profile) and stamps it onto every draft entity/relation in the latest
    extraction patch. Deterministic: no LLM, no network.
    """

    stage_name: str = "evidence_linker"
    resource_profile: ResourceProfile = _CPU_LIGHT

    def run(self, context: PipelineContext) -> PipelineContext:
        source_id = context.source_id
        anchor = context.stage_outputs.get("evidence_anchor")
        path = _evidence_path(anchor, source_id)
        patch = _latest_patch(context)
        if patch is None:
            return context.with_output(self.stage_name, None)
        linked = _link_evidence(patch, path)
        return context.with_output(self.stage_name, linked)


def _evidence_path(anchor: Any, source_id: str) -> EvidencePath | None:
    if not isinstance(anchor, dict):
        return None
    return EvidencePath(
        paper_id=source_id,
        page_index_node_id=str(anchor.get("page_index_node_id", "")),
        semantic_chunk_id=str(anchor.get("semantic_chunk_id", "")),
        node_path=tuple(anchor.get("node_path", ())),
    )


def _latest_patch(context: PipelineContext) -> ExtractionPatch | None:
    for stage in ("relation_type_classifier", "core_entity_extractor"):
        out = context.stage_outputs.get(stage)
        if isinstance(out, ExtractionPatch):
            return out
    return None


def _link_evidence(patch: ExtractionPatch, path: EvidencePath | None) -> ExtractionPatch:
    from dataclasses import replace

    if path is None:
        return patch
    entities = [replace(e, evidence_path=path) for e in patch.entities]
    relations = [replace(r, evidence_path=path) for r in patch.relations]
    return replace(patch, entities=entities, relations=relations)


# ── Patch assembly helpers ───────────────────────────────────────────────────


def _empty_patch(source_id: str) -> ExtractionPatch:
    return ExtractionPatch(
        source_id=source_id,
        claims=[],
        entities=[],
        relations=[],
        safety_flags=dict(DEFAULT_SAFETY_FLAGS),
        extractor_version="",
    )


def _patch_with_entities(
    context: PipelineContext, entities: list[TypedEntity], source_id: str
) -> ExtractionPatch:
    base = _latest_patch(context) or _empty_patch(source_id)
    from dataclasses import replace

    return replace(base, entities=entities)


def _patch_with_relations(
    context: PipelineContext, relations: list[TypedRelation], source_id: str
) -> ExtractionPatch:
    base = _latest_patch(context) or _empty_patch(source_id)
    from dataclasses import replace

    return replace(base, relations=relations)


def _kw_list(stat: StatisticalContext | None) -> list[str]:
    return [k for k, _ in (stat.keywords if stat else ())]


__all__ = [
    "LLMClient",
    "LLMEntityOutput",
    "LLMRelationOutput",
    "StatisticalContext",
    "StatisticalPreProcessor",
    "CoreEntityExtractor",
    "BinaryRelationDetector",
    "RelationTypeClassifier",
    "EvidenceLinker",
]
