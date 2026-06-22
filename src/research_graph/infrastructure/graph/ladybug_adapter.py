"""LadybugAdapter — wraps :mod:`research_graph.infrastructure.graph.ladybug_client` behind
:class:`~research_graph.domain.ports.GraphDBPort` (D086).

This is a THIN adapter (Ponytail): it owns the LadybugDB connection and
delegates every operation to the existing ``ladybug_client`` functions WITHOUT
changing their behaviour. The transactional fail-closed semantics
(BEGIN/COMMIT/rollback, 5 import flags false) are preserved because the adapter
calls the same code path — it only relocates the ``conn`` argument from the
caller into the adapter.

The concrete ``ladybug.Connection`` stays an Adapter implementation detail:
:class:`GraphDBPort` methods carry no ``conn`` parameter. When Phase 3 adds a
FalkorDB adapter (ADR-030), it implements the same Port and callers swap
adapters, not call sites.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

from research_graph.domain.navigation import PageIndexDocument
from research_graph.domain.schema import ExtractionPatch
from research_graph.domain.semantic_chunks import EvidencePath, SemanticChunk
from research_graph.infrastructure.graph import ladybug_client


class LadybugAdapter:
    """GraphDBPort adapter over the existing LadybugDB client (D086).

    Construct with an open connection (``conn=``) or a db path (``db_path=``,
    which calls :func:`ladybug_client.init_db` and initializes the schema).
    Structural typing: satisfies
    :class:`~research_graph.domain.ports.GraphDBPort` without inheritance.
    """

    def __init__(
        self,
        *,
        conn: Any | None = None,
        db_path: Path | str | None = None,
        init_schema_on_construct: bool = True,
    ) -> None:
        if conn is None and db_path is None:
            raise ValueError("LadybugAdapter requires conn= or db_path=")
        # Allow a test fake connection to be injected (FailingConn pattern).
        # pyrefly: ignore [bad-argument-type]
        self._conn: Any = conn if conn is not None else ladybug_client.init_db(db_path)  # ty:ignore[invalid-argument-type]
        if init_schema_on_construct and conn is None:
            # init_db already initializes the schema for a fresh connection; only
            # re-init when an external connection was passed without schema.
            self.init_schema()

    @property
    def conn(self) -> Any:
        """Expose the underlying connection (Adapter-private; for tests/audit)."""
        return self._conn

    def init_schema(self) -> None:
        """Initialize base + scientific-KG schema (idempotent)."""
        ladybug_client.init_base_schema(self._conn)
        ladybug_client.init_scientific_kg_schema(self._conn)

    def upsert_scientific_kg(
        self,
        document: PageIndexDocument,
        chunks: list[SemanticChunk],
        evidence_paths: list[EvidencePath],
        patch: ExtractionPatch,
    ) -> None:
        """Persist one fixture scientific KG patch transactionally (fail-closed).

        Delegates to :func:`ladybug_client.upsert_scientific_kg` with this
        adapter's connection. The transaction BEGIN/COMMIT/rollback and the
        5 fail-closed import flags are preserved unchanged.
        """
        ladybug_client.upsert_scientific_kg(self._conn, document, chunks, evidence_paths, patch)


__all__ = ["LadybugAdapter"]
