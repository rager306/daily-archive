"""Disposable pilot graph store for M205 controlled write path.

In-memory transactional store that simulates a Falkor-compatible write driver
without importing falkordb/redis SDKs. Lives only under
``infrastructure.graph.pilot_write`` — never imported by no-write projection,
readiness, or staging modules.
"""

from __future__ import annotations

import copy
import socket
import threading
import uuid
from dataclasses import dataclass, field
from typing import Any


@dataclass
class PilotNode:
    node_id: str
    labels: tuple[str, ...]
    props: dict[str, Any] = field(default_factory=dict)


@dataclass
class PilotEdge:
    edge_id: str
    edge_type: str
    source_id: str
    target_id: str
    props: dict[str, Any] = field(default_factory=dict)


class DisposablePilotGraphStore:
    """Isolated in-memory graph with begin/commit/rollback and cleanup."""

    def __init__(self, *, store_id: str | None = None) -> None:
        self.store_id = store_id or f"pilot-{uuid.uuid4().hex[:12]}"
        self._nodes: dict[str, PilotNode] = {}
        self._edges: dict[str, PilotEdge] = {}
        self._txn_open = False
        self._txn_nodes: dict[str, PilotNode] | None = None
        self._txn_edges: dict[str, PilotEdge] | None = None
        self._closed = False
        self._lock = threading.RLock()
        self._health_ok = True

    def health(self) -> dict[str, Any]:
        with self._lock:
            return {
                "store_id": self.store_id,
                "healthy": self._health_ok and not self._closed,
                "closed": self._closed,
                "node_count": len(self._nodes),
                "edge_count": len(self._edges),
                "txn_open": self._txn_open,
                "driver": "disposable_in_memory_falkor_compatible",
                "sdk_imported": False,
            }

    def connect_probe(self, host: str, port: int, *, timeout_s: float = 0.25) -> dict[str, Any]:
        """Optional TCP probe for a disposable local service (no protocol)."""
        try:
            with socket.create_connection((host, port), timeout=timeout_s):
                status = "reachable"
        except OSError as exc:
            status = f"unreachable:{type(exc).__name__}"
        return {"host": host, "port": port, "status": status, "store_id": self.store_id}

    def begin(self) -> None:
        with self._lock:
            self._ensure_open()
            if self._txn_open:
                raise RuntimeError("transaction already open")
            self._txn_nodes = copy.deepcopy(self._nodes)
            self._txn_edges = copy.deepcopy(self._edges)
            self._txn_open = True

    def commit(self) -> None:
        with self._lock:
            self._ensure_open()
            if not self._txn_open or self._txn_nodes is None or self._txn_edges is None:
                raise RuntimeError("no open transaction")
            self._nodes = self._txn_nodes
            self._edges = self._txn_edges
            self._txn_nodes = None
            self._txn_edges = None
            self._txn_open = False

    def rollback(self) -> None:
        with self._lock:
            self._ensure_open()
            self._txn_nodes = None
            self._txn_edges = None
            self._txn_open = False

    def upsert_node(self, node: PilotNode) -> None:
        with self._lock:
            self._ensure_open()
            target = self._active_nodes()
            target[node.node_id] = PilotNode(
                node_id=node.node_id,
                labels=tuple(node.labels),
                props=dict(node.props),
            )

    def upsert_edge(self, edge: PilotEdge) -> None:
        with self._lock:
            self._ensure_open()
            nodes = self._active_nodes()
            if edge.source_id not in nodes or edge.target_id not in nodes:
                raise ValueError("edge endpoints must exist")
            edges = self._active_edges()
            edges[edge.edge_id] = PilotEdge(
                edge_id=edge.edge_id,
                edge_type=edge.edge_type,
                source_id=edge.source_id,
                target_id=edge.target_id,
                props=dict(edge.props),
            )

    def get_node(self, node_id: str) -> PilotNode | None:
        with self._lock:
            return self._nodes.get(node_id)

    def get_edge(self, edge_id: str) -> PilotEdge | None:
        with self._lock:
            return self._edges.get(edge_id)

    def list_nodes(self) -> tuple[PilotNode, ...]:
        with self._lock:
            return tuple(self._nodes.values())

    def list_edges(self) -> tuple[PilotEdge, ...]:
        with self._lock:
            return tuple(self._edges.values())

    def export_snapshot(self) -> dict[str, Any]:
        with self._lock:
            return {
                "store_id": self.store_id,
                "nodes": [
                    {"node_id": n.node_id, "labels": list(n.labels), "props": dict(n.props)}
                    for n in self._nodes.values()
                ],
                "edges": [
                    {
                        "edge_id": e.edge_id,
                        "edge_type": e.edge_type,
                        "source_id": e.source_id,
                        "target_id": e.target_id,
                        "props": dict(e.props),
                    }
                    for e in self._edges.values()
                ],
            }

    def restore_snapshot(self, snapshot: dict[str, Any]) -> None:
        with self._lock:
            self._ensure_open()
            if self._txn_open:
                raise RuntimeError("cannot restore during transaction")
            nodes: dict[str, PilotNode] = {}
            edges: dict[str, PilotEdge] = {}
            for raw in snapshot.get("nodes", []):
                node = PilotNode(
                    node_id=str(raw["node_id"]),
                    labels=tuple(raw.get("labels") or ()),
                    props=dict(raw.get("props") or {}),
                )
                nodes[node.node_id] = node
            for raw in snapshot.get("edges", []):
                edge = PilotEdge(
                    edge_id=str(raw["edge_id"]),
                    edge_type=str(raw["edge_type"]),
                    source_id=str(raw["source_id"]),
                    target_id=str(raw["target_id"]),
                    props=dict(raw.get("props") or {}),
                )
                edges[edge.edge_id] = edge
            self._nodes = nodes
            self._edges = edges

    def cleanup(self) -> dict[str, Any]:
        with self._lock:
            self._nodes.clear()
            self._edges.clear()
            self._txn_nodes = None
            self._txn_edges = None
            self._txn_open = False
            self._closed = True
            self._health_ok = False
            return {
                "store_id": self.store_id,
                "cleaned": True,
                "closed": True,
                "node_count": 0,
                "edge_count": 0,
            }

    def _active_nodes(self) -> dict[str, PilotNode]:
        if self._txn_open and self._txn_nodes is not None:
            return self._txn_nodes
        return self._nodes

    def _active_edges(self) -> dict[str, PilotEdge]:
        if self._txn_open and self._txn_edges is not None:
            return self._txn_edges
        return self._edges

    def _ensure_open(self) -> None:
        if self._closed:
            raise RuntimeError("store is closed")


__all__ = [
    "DisposablePilotGraphStore",
    "PilotEdge",
    "PilotNode",
]
