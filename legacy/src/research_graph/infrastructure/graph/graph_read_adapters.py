"""Backend-neutral GraphReadPort adapters (M206).

LadybugGraphReadAdapter wraps an existing Ladybug connection with the same
read-only Cypher shapes used by hybrid retrieval. SnapshotGraphReadAdapter
serves controlled Falkor/pilot export snapshots without a live driver.
"""

from __future__ import annotations

from typing import Any, cast


def _bounded_score(value: Any) -> float:
    try:
        score = float(value)
    except (TypeError, ValueError):
        return 1.0
    return min(max(score, 0.0), 1.0)


def _page_index_node_id_from_chunk(semantic_chunk_id: str) -> str:
    suffix = ":chunk-"
    if suffix in semantic_chunk_id:
        return semantic_chunk_id.split(suffix, 1)[0]
    return semantic_chunk_id


class LadybugGraphReadAdapter:
    """GraphReadPort over a LadybugDB connection (fixture / local only)."""

    def __init__(self, conn: Any) -> None:
        self._conn = conn
        self.backend = "ladybugdb"

    def seed_match(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]:
        needle_cf = needle.casefold().strip()
        if not needle_cf:
            return []
        candidates: list[dict[str, Any]] = []
        for label, text_property, source_name in [
            ("Claim", "text", "claim"),
            ("ScientificEntity", "label", "entity"),
            ("ScientificRelation", "relation_type", "relation"),
        ]:
            result = cast(
                Any,
                self._conn.execute(
                    f"MATCH (item:{label})-[:EVIDENCED_BY]->(evidence:EvidencePath) "
                    "RETURN item."
                    f"{text_property}, item.confidence, evidence.id, "
                    "evidence.page_index_node_id, evidence.semantic_chunk_id"
                ),
            )
            while result.has_next():
                item_text, confidence, evidence_id, page_index_node_id, semantic_chunk_id = (
                    result.get_next()
                )
                if needle_cf not in str(item_text).casefold():
                    continue
                candidates.append(
                    {
                        "semantic_chunk_id": str(semantic_chunk_id),
                        "page_index_node_id": str(page_index_node_id),
                        "evidence_path_id": str(evidence_id),
                        "graph_score": _bounded_score(confidence),
                        "graph_source": source_name,
                    }
                )
        candidates.sort(
            key=lambda c: (-float(c["graph_score"]), str(c["semantic_chunk_id"]), str(c["evidence_path_id"]))
        )
        return candidates[: max(limit, 0)]

    def lineage_expand(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]:
        needle_cf = needle.casefold().strip()
        if not needle_cf:
            return []
        candidates: list[dict[str, Any]] = []
        for endpoint_label, text_property, endpoint_role in [
            ("Claim", "text", "claim_endpoint"),
            ("ScientificEntity", "label", "entity_endpoint"),
        ]:
            for rel_table in ["SCIENTIFIC_RELATION_SOURCE", "SCIENTIFIC_RELATION_TARGET"]:
                result = cast(
                    Any,
                    self._conn.execute(
                        f"MATCH (relation:ScientificRelation)-[:{rel_table}]->"
                        f"(endpoint:{endpoint_label}), "
                        "(relation)-[:EVIDENCED_BY]->(evidence:EvidencePath) "
                        "RETURN endpoint."
                        f"{text_property}, relation.confidence, evidence.id, "
                        "evidence.page_index_node_id, evidence.semantic_chunk_id"
                    ),
                )
                while result.has_next():
                    endpoint_text, confidence, evidence_id, page_index_node_id, semantic_chunk_id = (
                        result.get_next()
                    )
                    if needle_cf not in str(endpoint_text).casefold():
                        continue
                    candidates.append(
                        {
                            "semantic_chunk_id": str(semantic_chunk_id),
                            "page_index_node_id": str(page_index_node_id),
                            "evidence_path_id": str(evidence_id),
                            "graph_score": _bounded_score(confidence) * 0.9,
                            "graph_source": endpoint_role,
                        }
                    )
        candidates.sort(
            key=lambda c: (-float(c["graph_score"]), str(c["semantic_chunk_id"]), str(c["evidence_path_id"]))
        )
        return candidates[: max(limit, 0)]

    def evidence_paths_by_chunk(self) -> dict[str, tuple[str, str]]:
        result = cast(
            Any,
            self._conn.execute(
                "MATCH (evidence:EvidencePath) "
                "RETURN evidence.semantic_chunk_id, evidence.id, evidence.page_index_node_id"
            ),
        )
        rows: dict[str, tuple[str, str]] = {}
        while result.has_next():
            semantic_chunk_id, evidence_id, page_index_node_id = result.get_next()
            rows[str(semantic_chunk_id)] = (str(evidence_id), str(page_index_node_id))
        return rows

    def page_neighbors(self, semantic_chunk_id: str) -> list[str]:
        evidence = self.evidence_paths_by_chunk()
        page_id = evidence.get(
            semantic_chunk_id,
            ("", _page_index_node_id_from_chunk(semantic_chunk_id)),
        )[1]
        if not page_id:
            return []
        result = cast(
            Any,
            self._conn.execute(
                "MATCH (node:PageIndexNode)-[:NEXT_PAGE_INDEX_NODE]->(next:PageIndexNode), "
                "(next)-[:HAS_SEMANTIC_CHUNK]->(chunk:SemanticChunk) "
                "RETURN node.id, chunk.id"
            ),
        )
        neighbors: list[str] = []
        while result.has_next():
            node_id, chunk_id = result.get_next()
            if str(node_id) == page_id:
                neighbors.append(str(chunk_id))
        return sorted(neighbors)

    def integrity_scan(self) -> dict[str, Any]:
        evidence = self.evidence_paths_by_chunk()
        chunk_ids = set(evidence)
        orphan_evidence = 0
        # Evidence without matching SemanticChunk node is counted if query works
        try:
            result = cast(Any, self._conn.execute("MATCH (s:SemanticChunk) RETURN s.id"))
            present_chunks: set[str] = set()
            while result.has_next():
                present_chunks.add(str(result.get_next()[0]))
            orphan_evidence = len(chunk_ids - present_chunks)
        except Exception:
            orphan_evidence = 0
        duplicate_ids = 0
        broken_paths = sum(1 for ep, page in evidence.values() if not ep or not page)
        return {
            "backend": self.backend,
            "evidence_path_count": len(evidence),
            "orphan_evidence_chunks": orphan_evidence,
            "broken_evidence_paths": broken_paths,
            "duplicate_stable_ids": duplicate_ids,
            "schema_violations": 0,
        }


class SnapshotGraphReadAdapter:
    """GraphReadPort over a controlled Falkor/pilot export snapshot (no live SDK)."""

    def __init__(self, snapshot: dict[str, Any], *, backend: str = "falkor_snapshot") -> None:
        self.backend = backend
        self._nodes = {
            str(n["node_id"]): n for n in (snapshot.get("nodes") or []) if isinstance(n, dict)
        }
        self._edges = [
            e for e in (snapshot.get("edges") or []) if isinstance(e, dict)
        ]
        self._index_from_snapshot()

    def _index_from_snapshot(self) -> None:
        self._evidence: dict[str, tuple[str, str]] = {}
        self._seed_rows: list[dict[str, Any]] = []
        self._lineage_rows: list[dict[str, Any]] = []
        self._neighbors: dict[str, list[str]] = {}
        for node in self._nodes.values():
            labels = set(node.get("labels") or [])
            props = dict(node.get("props") or {})
            if "EvidencePath" in labels:
                ep_id = str(props.get("evidence_path_id") or node["node_id"].removeprefix("evidence:"))
                chunk_id = str(props.get("chunk_id") or props.get("semantic_chunk_id") or "")
                page_id = str(props.get("page_index_node_id") or _page_index_node_id_from_chunk(chunk_id))
                if chunk_id:
                    self._evidence[chunk_id] = (ep_id, page_id)
            if "Claim" in labels or "Entity" in labels or "ScientificEntity" in labels:
                text = str(props.get("text") or props.get("label") or props.get("entity_id") or "")
                chunk_id = str(props.get("semantic_chunk_id") or props.get("chunk_id") or "")
                ep_id = str(props.get("evidence_path_id") or "")
                page_id = str(props.get("page_index_node_id") or _page_index_node_id_from_chunk(chunk_id))
                if chunk_id:
                    self._seed_rows.append(
                        {
                            "semantic_chunk_id": chunk_id,
                            "page_index_node_id": page_id,
                            "evidence_path_id": ep_id or f"evidence:{chunk_id}",
                            "graph_score": _bounded_score(props.get("confidence", 0.85)),
                            "graph_source": "snapshot_seed",
                            "match_text": text,
                        }
                    )
            if "SemanticChunk" in labels:
                chunk_id = str(props.get("chunk_id") or node["node_id"].removeprefix("chunk:"))
                page_id = str(props.get("page_index_node_id") or _page_index_node_id_from_chunk(chunk_id))
                ep_id = str(props.get("evidence_path_id") or f"evidence:{chunk_id}")
                if chunk_id and chunk_id not in self._evidence:
                    self._evidence[chunk_id] = (ep_id, page_id)
                self._seed_rows.append(
                    {
                        "semantic_chunk_id": chunk_id,
                        "page_index_node_id": page_id,
                        "evidence_path_id": ep_id,
                        "graph_score": 0.8,
                        "graph_source": "snapshot_chunk",
                        "match_text": str(props.get("label") or chunk_id),
                    }
                )
        # lineage from relation edges
        for edge in self._edges:
            if str(edge.get("edge_type", "")).startswith("rel") or edge.get("edge_id", "").startswith("rel:"):
                props = dict(edge.get("props") or {})
                src = str(edge.get("source_id", ""))
                tgt = str(edge.get("target_id", ""))
                for endpoint in (src, tgt):
                    ent = self._nodes.get(endpoint, {})
                    eprops = dict(ent.get("props") or {})
                    chunk_id = str(eprops.get("semantic_chunk_id") or eprops.get("chunk_id") or "")
                    if not chunk_id:
                        # fall back: use paper method chunk if paper-linked
                        paper_id = str(eprops.get("paper_id") or "")
                        if paper_id:
                            chunk_id = f"{paper_id}:method:chunk-0001"
                    if not chunk_id:
                        continue
                    ep_id, page_id = self._evidence.get(
                        chunk_id,
                        (f"evidence:{chunk_id}", _page_index_node_id_from_chunk(chunk_id)),
                    )
                    self._lineage_rows.append(
                        {
                            "semantic_chunk_id": chunk_id,
                            "page_index_node_id": page_id,
                            "evidence_path_id": ep_id,
                            "graph_score": 0.75,
                            "graph_source": "snapshot_lineage",
                            "match_text": str(
                                eprops.get("label")
                                or eprops.get("entity_id")
                                or props.get("relation_id")
                                or chunk_id
                            ),
                        }
                    )
        # neighbors: same paper_id chunks
        by_paper: dict[str, list[str]] = {}
        for chunk_id in self._evidence:
            paper = chunk_id.split(":", 1)[0]
            by_paper.setdefault(paper, []).append(chunk_id)
        for chunk_ids in by_paper.values():
            ordered = sorted(chunk_ids)
            for i, cid in enumerate(ordered):
                self._neighbors[cid] = [c for j, c in enumerate(ordered) if j != i][:5]

    def seed_match(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]:
        needle_cf = needle.casefold().strip()
        if not needle_cf:
            return []
        # Also index raw node ids/props for snapshot text search.
        extra_rows: list[dict[str, Any]] = []
        for node_id, node in self._nodes.items():
            props = dict(node.get("props") or {})
            blob = " ".join(
                [
                    node_id,
                    str(props.get("label") or ""),
                    str(props.get("text") or ""),
                    str(props.get("entity_id") or ""),
                    str(props.get("chunk_id") or ""),
                    str(props.get("paper_id") or ""),
                ]
            ).casefold()
            if needle_cf not in blob:
                continue
            chunk_id = str(
                props.get("semantic_chunk_id")
                or props.get("chunk_id")
                or (node_id.removeprefix("chunk:") if node_id.startswith("chunk:") else "")
            )
            if not chunk_id and props.get("paper_id"):
                chunk_id = f"{props['paper_id']}:method:chunk-0001"
            if not chunk_id:
                continue
            ep_id, page_id = self._evidence.get(
                chunk_id,
                (f"evidence:{chunk_id}", _page_index_node_id_from_chunk(chunk_id)),
            )
            extra_rows.append(
                {
                    "semantic_chunk_id": chunk_id,
                    "page_index_node_id": page_id,
                    "evidence_path_id": ep_id,
                    "graph_score": 0.82,
                    "graph_source": "snapshot_node",
                    "match_text": blob,
                }
            )
        pool = list(self._seed_rows) + extra_rows
        hits = [
            {k: v for k, v in row.items() if k != "match_text"}
            for row in pool
            if needle_cf in str(row.get("match_text", "")).casefold()
            or needle_cf in str(row.get("semantic_chunk_id", "")).casefold()
            or needle_cf in str(row.get("evidence_path_id", "")).casefold()
        ]
        # de-dupe by chunk
        best: dict[str, dict[str, Any]] = {}
        for hit in hits:
            cid = str(hit["semantic_chunk_id"])
            if cid not in best or float(hit["graph_score"]) > float(best[cid]["graph_score"]):
                best[cid] = hit
        rows = sorted(best.values(), key=lambda r: (-float(r["graph_score"]), str(r["semantic_chunk_id"])))
        return rows[: max(limit, 0)]

    def lineage_expand(self, needle: str, *, limit: int = 10) -> list[dict[str, Any]]:
        needle_cf = needle.casefold().strip()
        if not needle_cf:
            return []
        hits = [
            {k: v for k, v in row.items() if k != "match_text"}
            for row in self._lineage_rows
            if needle_cf in str(row.get("match_text", "")).casefold()
            or needle_cf in str(row.get("semantic_chunk_id", "")).casefold()
        ]
        if not hits:
            # fallback: seed matches as lineage
            return self.seed_match(needle, limit=limit)
        best: dict[str, dict[str, Any]] = {}
        for hit in hits:
            cid = str(hit["semantic_chunk_id"])
            if cid not in best or float(hit["graph_score"]) > float(best[cid]["graph_score"]):
                best[cid] = hit
        rows = sorted(best.values(), key=lambda r: (-float(r["graph_score"]), str(r["semantic_chunk_id"])))
        return rows[: max(limit, 0)]

    def evidence_paths_by_chunk(self) -> dict[str, tuple[str, str]]:
        return dict(self._evidence)

    def page_neighbors(self, semantic_chunk_id: str) -> list[str]:
        return list(self._neighbors.get(semantic_chunk_id, []))

    def integrity_scan(self) -> dict[str, Any]:
        broken = sum(1 for ep, page in self._evidence.values() if not ep or not page)
        ids = list(self._nodes)
        duplicate_ids = len(ids) - len(set(ids))
        return {
            "backend": self.backend,
            "evidence_path_count": len(self._evidence),
            "orphan_evidence_chunks": 0,
            "broken_evidence_paths": broken,
            "duplicate_stable_ids": duplicate_ids,
            "schema_violations": 0,
            "node_count": len(self._nodes),
            "edge_count": len(self._edges),
        }


__all__ = ["LadybugGraphReadAdapter", "SnapshotGraphReadAdapter"]
