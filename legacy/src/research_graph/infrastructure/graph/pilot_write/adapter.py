"""Falkor-compatible GraphDBPort adapter for M205 pilot write path only."""

from __future__ import annotations

from typing import Any

from research_graph.application.graph.pilot_write_authorization import PilotWriteAuthorization
from research_graph.domain.navigation import PageIndexDocument
from research_graph.domain.schema import ExtractionPatch
from research_graph.domain.semantic_chunks import EvidencePath, SemanticChunk
from research_graph.infrastructure.graph.pilot_write.driver import (
    DisposablePilotGraphStore,
    PilotEdge,
    PilotNode,
)


class UnauthorizedPilotWriteError(PermissionError):
    """Raised when pilot write is attempted without valid authorization."""


class FalkorPilotGraphDBAdapter:
    """GraphDBPort adapter backed by DisposablePilotGraphStore.

    Requires :class:`PilotWriteAuthorization` for every mutating call.
    Production SafetyFlags are not modified.
    """

    def __init__(
        self,
        store: DisposablePilotGraphStore,
        *,
        authorization: PilotWriteAuthorization | None = None,
    ) -> None:
        self._store = store
        self._authorization = authorization
        self.schema_initialized = False
        self.write_count = 0
        self.last_receipt_meta: dict[str, Any] = {}

    @property
    def store(self) -> DisposablePilotGraphStore:
        return self._store

    def set_authorization(self, authorization: PilotWriteAuthorization | None) -> None:
        if authorization is not None:
            authorization.assert_production_flags_closed()
        self._authorization = authorization

    def init_schema(self) -> None:
        self._require_auth("init_schema")
        self.schema_initialized = True
        # marker node for schema
        self._store.begin()
        try:
            self._store.upsert_node(
                PilotNode(
                    node_id="schema:pilot.v1",
                    labels=("SchemaMarker",),
                    props={"schema_version": "pilot-falkor.v1"},
                )
            )
            self._store.commit()
        except Exception:
            self._store.rollback()
            raise

    def upsert_scientific_kg(
        self,
        document: PageIndexDocument,
        chunks: list[SemanticChunk],
        evidence_paths: list[EvidencePath],
        patch: ExtractionPatch,
    ) -> None:
        self._require_auth("upsert_scientific_kg")
        paper_id = str(document.paper_id)
        self._store.begin()
        try:
            self._store.upsert_node(
                PilotNode(
                    node_id=f"paper:{paper_id}",
                    labels=("Paper",),
                    props={
                        "paper_id": paper_id,
                        "packet_hash": self._authorization.packet_hash if self._authorization else "",
                        "candidate_evidence": True,
                        "import_eligible": False,
                    },
                )
            )
            for chunk in chunks:
                chunk_id = str(getattr(chunk, "chunk_id", "") or getattr(chunk, "id", ""))
                if not chunk_id:
                    continue
                self._store.upsert_node(
                    PilotNode(
                        node_id=f"chunk:{chunk_id}",
                        labels=("SemanticChunk",),
                        props={"chunk_id": chunk_id, "paper_id": paper_id},
                    )
                )
                self._store.upsert_edge(
                    PilotEdge(
                        edge_id=f"edge:paper-chunk:{paper_id}:{chunk_id}",
                        edge_type="HAS_CHUNK",
                        source_id=f"paper:{paper_id}",
                        target_id=f"chunk:{chunk_id}",
                        props={},
                    )
                )
            for ep in evidence_paths:
                ep_id = str(getattr(ep, "evidence_path_id", "") or getattr(ep, "id", ""))
                if not ep_id:
                    continue
                self._store.upsert_node(
                    PilotNode(
                        node_id=f"evidence:{ep_id}",
                        labels=("EvidencePath",),
                        props={"evidence_path_id": ep_id, "paper_id": paper_id},
                    )
                )
            # entities / relations from patch as candidate evidence only
            entities = list(getattr(patch, "entities", []) or [])
            for ent in entities:
                ent_id = str(getattr(ent, "entity_id", "") or getattr(ent, "id", ""))
                if not ent_id:
                    continue
                self._store.upsert_node(
                    PilotNode(
                        node_id=f"entity:{ent_id}",
                        labels=("Entity",),
                        props={
                            "entity_id": ent_id,
                            "paper_id": paper_id,
                            "import_eligible": False,
                        },
                    )
                )
            relations = list(getattr(patch, "relations", []) or [])
            for rel in relations:
                rel_id = str(getattr(rel, "relation_id", "") or getattr(rel, "id", ""))
                src = str(getattr(rel, "source_id", "") or getattr(rel, "source", ""))
                tgt = str(getattr(rel, "target_id", "") or getattr(rel, "target", ""))
                if not rel_id or not src or not tgt:
                    continue
                # ensure endpoints
                if self._store.get_node(f"entity:{src}") is None and f"entity:{src}" not in {
                    n.node_id for n in self._store.list_nodes()
                }:
                    # during txn use upsert
                    self._store.upsert_node(
                        PilotNode(node_id=f"entity:{src}", labels=("Entity",), props={"entity_id": src})
                    )
                if self._store.get_node(f"entity:{tgt}") is None:
                    self._store.upsert_node(
                        PilotNode(node_id=f"entity:{tgt}", labels=("Entity",), props={"entity_id": tgt})
                    )
                # endpoints may only exist in txn buffer
                self._store.upsert_node(
                    PilotNode(node_id=f"entity:{src}", labels=("Entity",), props={"entity_id": src})
                )
                self._store.upsert_node(
                    PilotNode(node_id=f"entity:{tgt}", labels=("Entity",), props={"entity_id": tgt})
                )
                self._store.upsert_edge(
                    PilotEdge(
                        edge_id=f"rel:{rel_id}",
                        edge_type=str(getattr(rel, "relation_type", "") or getattr(rel, "type", "RELATED")),
                        source_id=f"entity:{src}",
                        target_id=f"entity:{tgt}",
                        props={"relation_id": rel_id, "import_eligible": False},
                    )
                )
            self._store.commit()
            self.write_count += 1
            self.last_receipt_meta = {
                "paper_id": paper_id,
                "chunk_count": len(chunks),
                "evidence_count": len(evidence_paths),
                "entity_count": len(entities),
                "relation_count": len(relations),
            }
        except Exception:
            self._store.rollback()
            raise

    def read_back_paper(self, paper_id: str) -> dict[str, Any]:
        paper = self._store.get_node(f"paper:{paper_id}")
        if paper is None:
            return {"found": False, "paper_id": paper_id}
        nodes = [n for n in self._store.list_nodes() if n.props.get("paper_id") == paper_id or n.node_id == f"paper:{paper_id}"]
        edges = [
            e
            for e in self._store.list_edges()
            if e.source_id.startswith("paper:")
            or e.target_id.startswith("chunk:")
            or e.edge_id.startswith("rel:")
        ]
        evidence_ids = [n.props.get("evidence_path_id") for n in nodes if "EvidencePath" in n.labels]
        return {
            "found": True,
            "paper_id": paper_id,
            "packet_hash": paper.props.get("packet_hash"),
            "import_eligible": paper.props.get("import_eligible", False),
            "node_count": len(nodes),
            "edge_count": len(edges),
            "evidence_path_ids": [e for e in evidence_ids if e],
            "schema_marker": self._store.get_node("schema:pilot.v1") is not None,
        }

    def _require_auth(self, operation: str) -> None:
        auth = self._authorization
        if auth is None or not auth.authorized or auth.status != "authorized":
            raise UnauthorizedPilotWriteError(
                f"pilot write denied for {operation}: missing or invalid authorization"
            )
        auth.assert_production_flags_closed()


__all__ = ["FalkorPilotGraphDBAdapter", "UnauthorizedPilotWriteError"]
