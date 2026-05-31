"""Navigation contracts for PageIndex documents."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class PageIndexNode:
    """Navigable section node in a single-paper PageIndex tree."""

    id: str
    paper_id: str
    title: str
    level: int
    order: int
    text: str
    source_path: Path
    parent_id: str | None
    children_ids: list[str] = field(default_factory=list)
    next_id: str | None = None
    path: list[str] = field(default_factory=list)
    provenance: dict[str, str] = field(default_factory=dict)


@dataclass(frozen=True)
class NavigationAnchor:
    """Inspectable navigation summary for one PageIndex node."""

    node_id: str
    title: str
    path: list[str]
    parent_id: str | None
    children_ids: list[str]
    next_id: str | None


@dataclass
class PageIndexDocument:
    """PageIndex tree plus navigation helpers for one paper."""

    paper_id: str
    source_path: Path
    root: PageIndexNode
    nodes: list[PageIndexNode]
    validation_warnings: list[str]
    provenance: dict[str, str]
    navigation_anchors: list[NavigationAnchor] = field(default_factory=list)

    def __post_init__(self) -> None:
        if not self.navigation_anchors:
            self.navigation_anchors = build_navigation_anchors(self.nodes)

    def find_by_title(self, title: str) -> PageIndexNode | None:
        """Return the first node whose title matches case-insensitively."""
        normalized = title.casefold()
        return next((node for node in self.nodes if node.title.casefold() == normalized), None)

    def node_by_id(self, node_id: str) -> PageIndexNode | None:
        """Return a node by stable id for downstream chunk attachment."""
        return next((node for node in self.nodes if node.id == node_id), None)

    def children_of(self, node_id: str) -> list[PageIndexNode]:
        """Return direct children in stored child order."""
        node = self.node_by_id(node_id)
        if node is None:
            return []
        return [child for child_id in node.children_ids if (child := self.node_by_id(child_id))]

    def path_to(self, node_id: str) -> list[str]:
        """Return the stable Paper -> PageIndexNode path for a node id."""
        node = self.node_by_id(node_id)
        return list(node.path) if node is not None else []

    def walk_next(self) -> list[PageIndexNode]:
        """Walk nodes in their deterministic NEXT order."""
        if not self.nodes:
            return []
        by_id = {node.id: node for node in self.nodes}
        ordered = [self.nodes[0]]
        seen = {self.nodes[0].id}
        current = self.nodes[0]
        while current.next_id is not None and current.next_id not in seen:
            next_node = by_id.get(current.next_id)
            if next_node is None:
                break
            ordered.append(next_node)
            seen.add(next_node.id)
            current = next_node
        return ordered

    def validate_navigation(self) -> list[str]:
        """Return structural navigation diagnostics for parent/child/NEXT/path invariants."""
        diagnostics: list[str] = []
        by_id = {node.id: node for node in self.nodes}

        if self.root.id not in by_id:
            diagnostics.append(f"root node missing from node list: {self.root.id}")

        for expected_order, node in enumerate(self.nodes):
            if node.order != expected_order:
                diagnostics.append(
                    f"node {node.id} order {node.order} does not match position {expected_order}"
                )
            if not node.path or node.path[-1] != node.id:
                diagnostics.append(f"node {node.id} path does not end with its own id")
            if node is self.root and node.parent_id is not None:
                diagnostics.append(f"root node {node.id} unexpectedly has parent {node.parent_id}")
            if node is not self.root:
                if node.parent_id not in by_id:
                    diagnostics.append(f"node {node.id} references missing parent {node.parent_id}")
                elif node.id not in by_id[node.parent_id].children_ids:
                    diagnostics.append(
                        f"node {node.id} parent {node.parent_id} does not reference it as a child"
                    )
            for child_id in node.children_ids:
                child = by_id.get(child_id)
                if child is None:
                    diagnostics.append(f"node {node.id} references missing child {child_id}")
                elif child.parent_id != node.id:
                    diagnostics.append(
                        f"child {child_id} parent {child.parent_id} does not match {node.id}"
                    )

        for current, next_node in zip(self.nodes, self.nodes[1:], strict=False):
            if current.next_id != next_node.id:
                diagnostics.append(
                    f"node {current.id} next_id {current.next_id} does not match {next_node.id}"
                )
        if self.nodes and self.nodes[-1].next_id is not None:
            diagnostics.append(f"last node {self.nodes[-1].id} unexpectedly has next_id")

        return diagnostics

    def _node_by_id(self, node_id: str) -> PageIndexNode | None:
        return self.node_by_id(node_id)


def build_navigation_anchors(nodes: list[PageIndexNode]) -> list[NavigationAnchor]:
    """Return deterministic, inspectable navigation anchors for nodes."""
    return [
        NavigationAnchor(
            node_id=node.id,
            title=node.title,
            path=list(node.path),
            parent_id=node.parent_id,
            children_ids=list(node.children_ids),
            next_id=node.next_id,
        )
        for node in nodes
    ]


__all__ = [
    "NavigationAnchor",
    "PageIndexDocument",
    "PageIndexNode",
    "build_navigation_anchors",
]
