"""PageIndex document navigation over local full-text ingestion results.

S02 keeps PageIndex construction deterministic and local-only. It consumes the
S01 `FullTextIngestionResult` boundary and exposes an ordered node tree with
parent/child links, NEXT traversal, stable paths, and validation diagnostics.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path

from arxiv_archive.full_text import FullTextIngestionResult

_HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
_SLUG_RE = re.compile(r"[^a-z0-9]+")


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


@dataclass
class PageIndexDocument:
    """PageIndex tree plus navigation helpers for one paper."""

    paper_id: str
    source_path: Path
    root: PageIndexNode
    nodes: list[PageIndexNode]
    validation_warnings: list[str]
    provenance: dict[str, str]

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


def build_page_index(ingestion: FullTextIngestionResult) -> PageIndexDocument:
    """Build a deterministic PageIndex tree from local full-text ingestion output."""
    headings = _parse_heading_sections(ingestion.text)
    if not headings:
        return _fallback_document(ingestion)

    nodes: list[PageIndexNode] = []
    stack: list[PageIndexNode] = []
    used_ids: set[str] = set()

    for order, section in enumerate(headings):
        parent = _nearest_parent(stack, section["level"])
        node_id = _stable_node_id(ingestion.paper_id, section["title"], used_ids, root=order == 0)
        path = [node_id] if parent is None else [*parent.path, node_id]
        node = PageIndexNode(
            id=node_id,
            paper_id=ingestion.paper_id,
            title=section["title"],
            level=section["level"],
            order=order,
            text=section["text"],
            source_path=ingestion.source_path,
            parent_id=parent.id if parent is not None else None,
            path=path,
            provenance={
                "paper_id": ingestion.paper_id,
                "source_path": str(ingestion.source_path),
                "heading_level": str(section["level"]),
                "parser": "markdown_headings_v1",
            },
        )
        if parent is not None:
            parent.children_ids.append(node.id)
        nodes.append(node)
        stack = [candidate for candidate in stack if candidate.level < node.level]
        stack.append(node)

    for current, next_node in zip(nodes, nodes[1:], strict=False):
        current.next_id = next_node.id

    provenance = dict(ingestion.provenance)
    provenance["parser"] = "markdown_headings_v1"
    return PageIndexDocument(
        paper_id=ingestion.paper_id,
        source_path=ingestion.source_path,
        root=nodes[0],
        nodes=nodes,
        validation_warnings=[],
        provenance=provenance,
    )


def _parse_heading_sections(text: str) -> list[dict[str, object]]:
    body = _strip_yaml_frontmatter(text.strip())
    lines = body.splitlines()
    heading_positions: list[tuple[int, int, str]] = []
    for index, line in enumerate(lines):
        match = _HEADING_RE.match(line)
        if match:
            heading_positions.append((index, len(match.group(1)), match.group(2).strip()))

    sections: list[dict[str, object]] = []
    for position, (line_index, level, title) in enumerate(heading_positions):
        next_line_index = (
            heading_positions[position + 1][0] if position + 1 < len(heading_positions) else len(lines)
        )
        section_text = "\n".join(lines[line_index + 1 : next_line_index]).strip()
        sections.append({"level": level, "title": title, "text": section_text})
    return sections


def _strip_yaml_frontmatter(text: str) -> str:
    if not text.startswith("---"):
        return text
    lines = text.splitlines()
    for index, line in enumerate(lines[1:], start=1):
        if line.strip() == "---":
            return "\n".join(lines[index + 1 :]).strip()
    return text


def _nearest_parent(stack: list[PageIndexNode], level: int) -> PageIndexNode | None:
    candidates = [node for node in stack if node.level < level]
    return candidates[-1] if candidates else None


def _stable_node_id(paper_id: str, title: str, used_ids: set[str], *, root: bool) -> str:
    if root:
        node_id = f"{paper_id}:root"
    else:
        slug = _slug(title)
        node_id = f"{paper_id}:{slug}"
    if node_id not in used_ids:
        used_ids.add(node_id)
        return node_id

    suffix = 2
    while f"{node_id}-{suffix}" in used_ids:
        suffix += 1
    deduped = f"{node_id}-{suffix}"
    used_ids.add(deduped)
    return deduped


def _slug(title: str) -> str:
    slug = _SLUG_RE.sub("-", title.casefold()).strip("-")
    return slug or "section"


def _fallback_document(ingestion: FullTextIngestionResult) -> PageIndexDocument:
    root_id = f"{ingestion.paper_id}:root"
    fallback_id = f"{ingestion.paper_id}:full-text"
    root = PageIndexNode(
        id=root_id,
        paper_id=ingestion.paper_id,
        title="Document",
        level=1,
        order=0,
        text="",
        source_path=ingestion.source_path,
        parent_id=None,
        children_ids=[fallback_id],
        next_id=fallback_id,
        path=[root_id],
        provenance={
            "paper_id": ingestion.paper_id,
            "source_path": str(ingestion.source_path),
            "parser": "markdown_headings_v1",
            "fallback_reason": "no_headings",
        },
    )
    fallback = PageIndexNode(
        id=fallback_id,
        paper_id=ingestion.paper_id,
        title="Full Text",
        level=2,
        order=1,
        text=ingestion.text,
        source_path=ingestion.source_path,
        parent_id=root_id,
        path=[root_id, fallback_id],
        provenance={
            "paper_id": ingestion.paper_id,
            "source_path": str(ingestion.source_path),
            "parser": "markdown_headings_v1",
            "fallback_reason": "no_headings",
        },
    )
    provenance = dict(ingestion.provenance)
    provenance["parser"] = "markdown_headings_v1"
    provenance["fallback_reason"] = "no_headings"
    return PageIndexDocument(
        paper_id=ingestion.paper_id,
        source_path=ingestion.source_path,
        root=root,
        nodes=[root, fallback],
        validation_warnings=["no markdown headings found; created fallback full-text section"],
        provenance=provenance,
    )


__all__ = ["PageIndexDocument", "PageIndexNode", "build_page_index"]
