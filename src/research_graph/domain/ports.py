"""Hexagonal Ports — the seams infrastructure Adapters implement (D086).

These ``typing.Protocol`` interfaces are the Core's view of the outside world.
They are deliberately narrow: each declares only the operations the typed
extraction pipeline actually needs. Adapters live in
:mod:`research_graph.infrastructure` (LadybugAdapter, the MiniMax LLM client,
the hybrid PDF parser) and implement these Protocols structurally.

Port rule (D086, Ponytail override in ``AGENTS.md``): a Port is added ONLY when
at least one holds — (1) two+ implementations exist, (2) a migration is
planned, or (3) mockability is required by the test contract. All three Ports
here meet that bar:

* :class:`LLMClientPort` — MiniMax (primary) + GLM (fallback); ADR-025.
* :class:`GraphDBPort` — LadybugDB now, FalkorDB migration planned (Phase 3);
  ADR-022/030.
* :class:`PDFParserPort` — Marker / GROBID / arxiv2md / OpenDataLoader; ADR-008.

This module is fail-closed infrastructure-agnostic: it imports only stdlib
types and the typed schema models. No LLM SDK, no graph driver, no parser.
"""

from __future__ import annotations

from typing import Any, Protocol, runtime_checkable

from research_graph.evaluation.schema import ExtractionPatch
from research_graph.papers.indexing.navigation import PageIndexDocument
from research_graph.papers.semantic_chunks import EvidencePath, SemanticChunk

#: Extraction kinds the LLM boundary distinguishes (matches the prototype's
#: ``extraction_kind`` snapshot hint and the forced-tool schemas). The Port is
#: intentionally stringly-typed so a future GLM adapter can reuse the same
#: contract without a shared enum import.
EXTRACTION_KIND_ENTITIES = "entities"
EXTRACTION_KIND_RELATIONS = "relations"


@runtime_checkable
class LLMClientPort(Protocol):
    """Structured extraction boundary over a chat LLM (MiniMax primary, GLM fallback).

    The canonical implementation calls MiniMax via the Anthropic-compatible API
    with forced tool calls + ``input_schema`` (per the ``minimax-safe-helper``
    skill: NOT prompt-only JSON). Adapters parse the tool output into a dict
    shaped like ``{"entities": [...]}`` / ``{"relations": [...]}``; callers
    then Adaptix-load it into the typed boundary models. Local schema
    validation backs every call (fail-closed: malformed output → empty dict).

    ``kind`` is one of :data:`EXTRACTION_KIND_ENTITIES` /
    :data:`EXTRACTION_KIND_RELATIONS`.
    """

    def extract(
        self, prompt: str, kind: str, *, context: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        """Return the LLM's structured extraction as a raw dict (fail-closed empty on error).

        The dict shape is ``{"entities": [...]}`` or ``{"relations": [...]}``
        depending on ``kind``. Never raises on provider error — returns ``{}``.
        """
        ...


@runtime_checkable
class GraphDBPort(Protocol):
    """Graph persistence boundary (LadybugDB now → FalkorDB Phase 3).

    Adapters own their connection internally (the Port methods carry no
    ``conn`` argument — that is an Adapter implementation detail). Every write
    stays fail-closed: ``upsert_scientific_kg`` persists a typed patch as
    *candidate evidence* and never sets import-eligibility flags (§6.3 #2, #4).
    """

    def init_schema(self) -> None:
        """Initialize the graph schema (idempotent)."""
        ...

    def upsert_scientific_kg(
        self,
        document: PageIndexDocument,
        chunks: list[SemanticChunk],
        evidence_paths: list[EvidencePath],
        patch: ExtractionPatch,
    ) -> None:
        """Persist one fixture scientific KG patch transactionally and idempotently.

        Equivalent to :func:`research_graph.graph.ladybug_client
        .upsert_scientific_kg` but with the connection owned by the Adapter.
        """
        ...


@runtime_checkable
class PDFParserPort(Protocol):
    """Hybrid PDF/markdown parser boundary (ADR-008/009).

    Adapters implement one parser backend (Marker / GROBID / arxiv2md /
    OpenDataLoader); a routing adapter selects among them by source quality.
    Parsing output is deterministic candidate evidence — parse warnings stay
    warnings, never silent successes.
    """

    def parse(self, source: Any) -> Any:
        """Parse ``source`` (fulltext ingestion result / markdown / PDF path) into a parsed article.

        Concrete return type is the domain's parsed-article structure; typed
        loosely here so the Port does not import the parser package.
        """
        ...


__all__ = [
    "EXTRACTION_KIND_ENTITIES",
    "EXTRACTION_KIND_RELATIONS",
    "GraphDBPort",
    "LLMClientPort",
    "PDFParserPort",
]
