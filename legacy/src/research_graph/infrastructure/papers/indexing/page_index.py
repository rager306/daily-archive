"""Metadata-only PageIndex navigation over redacted article structures.

This module builds a deterministic, JSON-native navigation skeleton from the
redacted article-structure contract produced before any trusted graph import.
It carries IDs, source references, source-span hashes, hierarchy, anchors, and
redacted diagnostics only; it never serializes article prose, captions,
equations, binary payloads, embeddings, vectors, model outputs, secrets, or
import-eligibility claims.


Formerly: src/arxiv_archive/article_page_index.py"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Literal

from research_graph.infrastructure.papers.artifacts.models import (
    REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION,
)

ARTICLE_PAGE_INDEX_SCHEMA_VERSION = "article-page-index.v1"
ARTICLE_PAGE_INDEX_DIAGNOSTICS_SCHEMA_VERSION = "article-page-index-diagnostics.v1"
ARTICLE_PAGE_INDEX_BUILDER = "redacted_article_structure_page_index_v1"

PageIndexDiagnosticSeverity = Literal["info", "warning", "repair_required", "error"]
PageIndexNodeType = Literal["section", "artifact", "fallback"]

ALLOWED_PAGE_INDEX_SECTION_TYPES = frozenset(
    {
        "root",
        "abstract",
        "introduction",
        "background",
        "methods",
        "results",
        "discussion",
        "conclusion",
        "appendix",
        "unknown",
    }
)
ALLOWED_PAGE_INDEX_ARTIFACT_TYPES = frozenset(
    {
        "figure",
        "table",
        "equation",
        "reference",
        "dataset",
        "code",
        "method",
        "metric",
        "claim",
        "scientific_term",
        "experiment",
    }
)
ALLOWED_PAGE_INDEX_COORDINATE_SPACES = frozenset(
    {"normalized_markdown_char", "semantic_chunk_char", "page_bbox", "artifact_record"}
)

FORBIDDEN_PAYLOAD_KEYS = frozenset(
    {
        "text",
        "raw_text",
        "chunk_text",
        "paper_text",
        "claim_text",
        "section_text",
        "caption_text",
        "table_text",
        "equation_text",
        "model_output",
        "raw_model_output",
        "raw_minimax_response",
        "base64",
        "binary",
        "bytes",
        "image_bytes",
        "payload",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "secrets",
        "token",
        "tokens",
        "api_key",
        "credentials",
        "optimizer_trace",
        "optimizer_traces",
    }
)
FORBIDDEN_SOURCE_OF_TRUTH_KEYS = frozenset(
    {
        "source_of_truth",
        "source_of_truth_claim",
        "truth_source",
        "canonical_source",
        "minimax_source_of_truth",
    }
)
UNSAFE_TRUE_FLAGS = frozenset(
    {
        "raw_text_included",
        "raw_binary_included",
        "base64_included",
        "model_outputs_included",
        "embeddings_included",
        "vectors_included",
        "secrets_included",
        "optimizer_traces_included",
        "trusted_kg_import_allowed",
        "ladybugdb_written",
        "production_import_attempted",
        "import_eligible",
        "promoted_to_fact",
    }
)

BRIDGE_SUBTREE_STATUS = {
    "status": "review_only_not_import_eligible",
    "source_slice": "M024-0xjwh9/S02",
    "graph_import_claim": False,
    "trusted_kg_import_allowed": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
}


@dataclass(frozen=True)
class ArticlePageIndexDiagnostic:
    """One stable, redacted diagnostic for PageIndex construction or validation."""

    code: str
    json_path: str
    severity: PageIndexDiagnosticSeverity = "repair_required"
    object_id: str | None = None
    message: str = (
        "Article PageIndex diagnostic; inspect stable code and JSON path, not source content."
    )
    blocks_import: bool = True

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "json_path": self.json_path,
            "severity": self.severity,
            "object_id": self.object_id,
            "message": self.message,
            "blocks_import": self.blocks_import,
        }


@dataclass(frozen=True)
class ArticlePageIndexSourceSpan:
    """Coordinate pointer into a source artifact without carrying content."""

    span_id: str
    source_id: str
    coordinate_space: str
    char_start: int | None = None
    char_end: int | None = None
    page_start: int | None = None
    page_end: int | None = None
    bbox: tuple[float, float, float, float] | None = None
    span_hash: str | None = None

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "span_id": self.span_id,
            "source_id": self.source_id,
            "coordinate_space": self.coordinate_space,
            "char_start": self.char_start,
            "char_end": self.char_end,
            "page_start": self.page_start,
            "page_end": self.page_end,
            "bbox": list(self.bbox) if self.bbox is not None else None,
            "span_hash": self.span_hash,
            "raw_text_embedded": False,
        }


@dataclass(frozen=True)
class ArticlePageIndexAnchor:
    """Navigable source anchor for one node and one safe span."""

    anchor_id: str
    node_id: str
    paper_id: str
    span_id: str
    source_id: str
    coordinate_space: str
    span_hash: str | None
    anchor_type: str

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "anchor_id": self.anchor_id,
            "node_id": self.node_id,
            "paper_id": self.paper_id,
            "span_id": self.span_id,
            "source_id": self.source_id,
            "coordinate_space": self.coordinate_space,
            "span_hash": self.span_hash,
            "anchor_type": self.anchor_type,
            "raw_text_embedded": False,
            "import_eligible": False,
            "promoted_to_fact": False,
        }


@dataclass(frozen=True)
class ArticlePageIndexNode:
    """One metadata-only PageIndex node."""

    node_id: str
    paper_id: str
    node_type: PageIndexNodeType
    source_id: str | None
    parent_id: str | None
    children_ids: tuple[str, ...]
    next_id: str | None
    path: tuple[str, ...]
    order: int
    summary: dict[str, Any]
    source_ref_ids: tuple[str, ...]
    source_span: ArticlePageIndexSourceSpan | None
    anchor_ids: tuple[str, ...]

    def to_redacted_dict(self) -> dict[str, Any]:
        return {
            "node_id": self.node_id,
            "paper_id": self.paper_id,
            "node_type": self.node_type,
            "source_id": self.source_id,
            "parent_id": self.parent_id,
            "children_ids": list(self.children_ids),
            "next_id": self.next_id,
            "path": list(self.path),
            "order": self.order,
            "summary": dict(self.summary),
            "source_ref_ids": list(self.source_ref_ids),
            "source_span": self.source_span.to_redacted_dict()
            if self.source_span is not None
            else None,
            "anchor_ids": list(self.anchor_ids),
            "import_eligible": False,
            "promoted_to_fact": False,
        }


def build_article_page_index_from_structure(structure: dict[str, Any]) -> dict[str, Any]:
    """Build a deterministic metadata-only PageIndex from a redacted structure dict."""
    paper_id = _paper_id(structure)
    diagnostics = _validate_structure_boundary(structure)
    source_refs = [
        _redacted_source_ref(source, paper_id)
        for source in _list_of_dicts(structure.get("source_refs"))
    ]
    source_ref_ids = tuple(
        source["source_id"] for source in source_refs if isinstance(source.get("source_id"), str)
    )
    spans = {
        str(span.get("span_id")): _span_from_structure(span)
        for span in _list_of_dicts(structure.get("safe_spans"))
        if isinstance(span.get("span_id"), str) and span.get("span_id")
    }
    sections = _list_of_dicts(structure.get("sections"))
    placeholders = _list_of_dicts(structure.get("artifact_placeholders"))

    if not sections:
        diagnostics.append(
            _diagnostic("no_sections_fallback", "/sections", severity="info", blocks_import=False)
        )
        return _manifest(
            paper_id=paper_id,
            source_refs=source_refs,
            nodes=[_fallback_node(paper_id)],
            anchors=[],
            diagnostics=diagnostics,
        )

    nodes: list[dict[str, Any]] = []
    anchors: list[dict[str, Any]] = []
    children_by_parent: dict[str | None, list[str]] = {}
    first_section_by_id: dict[str, dict[str, Any]] = {}
    section_node_id_by_section_id: dict[str, str] = {}

    for section in sections:
        section_id = _string_or_none(section.get("section_id"))
        if section_id and section_id not in first_section_by_id:
            first_section_by_id[section_id] = section
            section_node_id_by_section_id[section_id] = _section_node_id(paper_id, section_id)

    artifacts_by_section: dict[str, list[dict[str, Any]]] = {}
    for placeholder in placeholders:
        section_id = _string_or_none(placeholder.get("section_id"))
        if section_id:
            artifacts_by_section.setdefault(section_id, []).append(placeholder)
        if section_id and section_id not in first_section_by_id:
            diagnostics.append(
                _diagnostic(
                    "artifact_missing_section_parent",
                    _path_for_item(
                        placeholders, placeholder, "artifact_placeholders", "section_id"
                    ),
                    _string_or_none(placeholder.get("artifact_id")),
                )
            )
            span_id = _string_or_none(placeholder.get("span_id"))
            if span_id and span_id not in spans:
                diagnostics.append(
                    _diagnostic(
                        "missing_span",
                        _path_for_item(
                            placeholders, placeholder, "artifact_placeholders", "span_id"
                        ),
                        _string_or_none(placeholder.get("artifact_id")),
                        severity="warning",
                    )
                )

    for section_index, section in enumerate(sections):
        section_id = _string_or_none(section.get("section_id"))
        if not section_id:
            continue
        node_id = _section_node_id(paper_id, section_id)
        parent_section_id = _string_or_none(section.get("parent_section_id"))
        parent_id = (
            section_node_id_by_section_id.get(parent_section_id) if parent_section_id else None
        )
        if parent_section_id and parent_section_id not in first_section_by_id:
            diagnostics.append(
                _diagnostic(
                    "missing_parent", f"/sections[{section_index}]/parent_section_id", section_id
                )
            )
        span_id = _string_or_none(section.get("span_id"))
        source_span = spans.get(span_id) if span_id else None
        if span_id and source_span is None:
            diagnostics.append(
                _diagnostic(
                    "missing_span",
                    f"/sections[{section_index}]/span_id",
                    section_id,
                    severity="warning",
                )
            )
        section_type = _string_or_none(section.get("section_type")) or "unknown"
        if section_type not in ALLOWED_PAGE_INDEX_SECTION_TYPES:
            diagnostics.append(
                _diagnostic(
                    "unsupported_section_type",
                    f"/sections[{section_index}]/section_type",
                    section_id,
                )
            )
            section_type = "unknown"
        anchor_ids: list[str] = []
        if source_span is not None:
            anchor = ArticlePageIndexAnchor(
                anchor_id=f"{paper_id}:page-index-anchor:section-{_section_slug(section_id)}",
                node_id=node_id,
                paper_id=paper_id,
                span_id=source_span.span_id,
                source_id=source_span.source_id,
                coordinate_space=source_span.coordinate_space,
                span_hash=source_span.span_hash,
                anchor_type="section",
            ).to_redacted_dict()
            anchors.append(anchor)
            anchor_ids.append(anchor["anchor_id"])
        nodes.append(
            {
                "node_id": node_id,
                "paper_id": paper_id,
                "node_type": "section",
                "source_id": source_span.source_id if source_span is not None else None,
                "parent_id": parent_id,
                "children_ids": [],
                "next_id": None,
                "path": [],
                "order": 0,
                "summary": {
                    "section_id": section_id,
                    "section_type": section_type,
                    "ordinal_path": _int_list(section.get("ordinal_path")),
                },
                "source_ref_ids": list(source_ref_ids),
                "source_span": source_span.to_redacted_dict() if source_span is not None else None,
                "anchor_ids": anchor_ids,
                "import_eligible": False,
                "promoted_to_fact": False,
            }
        )
        children_by_parent.setdefault(parent_id, []).append(node_id)

        for placeholder in artifacts_by_section.get(section_id, []):
            artifact_node, artifact_anchors, artifact_diagnostics = _artifact_node(
                paper_id=paper_id,
                placeholder=placeholder,
                placeholders=placeholders,
                parent_id=node_id,
                source_ref_ids=source_ref_ids,
                spans=spans,
            )
            diagnostics.extend(artifact_diagnostics)
            nodes.append(artifact_node)
            anchors.extend(artifact_anchors)
            children_by_parent.setdefault(node_id, []).append(artifact_node["node_id"])

    _apply_navigation(nodes, children_by_parent)
    return _manifest(
        paper_id=paper_id,
        source_refs=source_refs,
        nodes=nodes,
        anchors=anchors,
        diagnostics=diagnostics,
    )


def validate_article_page_index(page_index: dict[str, Any]) -> list[dict[str, Any]]:
    """Return redacted diagnostics for PageIndex manifest invariants."""
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    if page_index.get("schema_version") != ARTICLE_PAGE_INDEX_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_schema_version", "/schema_version"))
    diagnostics.extend(_validate_forbidden_keys(page_index))
    diagnostics.extend(_validate_source_of_truth_markers(page_index))
    diagnostics.extend(_validate_top_level_import_flags(page_index))

    nodes = _list_of_dicts(page_index.get("nodes"))
    anchors = _list_of_dicts(page_index.get("anchors"))
    by_id = {node.get("node_id"): node for node in nodes if isinstance(node.get("node_id"), str)}
    anchor_by_id = {
        anchor.get("anchor_id"): anchor
        for anchor in anchors
        if isinstance(anchor.get("anchor_id"), str)
    }

    if len(by_id) != len(nodes):
        diagnostics.append(_diagnostic("duplicate_or_missing_node_id", "/nodes"))
    if len(anchor_by_id) != len(anchors):
        diagnostics.append(_diagnostic("duplicate_or_missing_anchor_id", "/anchors"))

    for index, node in enumerate(nodes):
        node_id = _string_or_none(node.get("node_id"))
        if node.get("order") != index:
            diagnostics.append(
                _diagnostic("node_order_mismatch", f"/nodes[{index}]/order", node_id)
            )
        parent_id = node.get("parent_id")
        if parent_id is not None and parent_id not in by_id:
            diagnostics.append(_diagnostic("missing_parent", f"/nodes[{index}]/parent_id", node_id))
        for child_id in _string_list(node.get("children_ids")):
            child = by_id.get(child_id)
            if child is None:
                diagnostics.append(
                    _diagnostic("missing_child", f"/nodes[{index}]/children_ids", node_id)
                )
            elif child.get("parent_id") != node_id:
                diagnostics.append(
                    _diagnostic("child_parent_mismatch", f"/nodes[{index}]/children_ids", node_id)
                )
        if node.get("next_id") is not None and node.get("next_id") not in by_id:
            diagnostics.append(_diagnostic("missing_next", f"/nodes[{index}]/next_id", node_id))
        if node.get("import_eligible") is not False:
            diagnostics.append(
                _diagnostic("node_import_eligible", f"/nodes[{index}]/import_eligible", node_id)
            )
        if node.get("promoted_to_fact") is not False:
            diagnostics.append(
                _diagnostic("node_promoted_to_fact", f"/nodes[{index}]/promoted_to_fact", node_id)
            )
        if isinstance(node.get("source_span"), dict):
            diagnostics.extend(
                _validate_span(node["source_span"], f"/nodes[{index}]/source_span", node_id)
            )
        for anchor_id in _string_list(node.get("anchor_ids")):
            if anchor_id not in anchor_by_id:
                diagnostics.append(
                    _diagnostic("missing_anchor", f"/nodes[{index}]/anchor_ids", node_id)
                )

    for index, (current, next_node) in enumerate(zip(nodes, nodes[1:], strict=False)):
        if current.get("next_id") != next_node.get("node_id"):
            diagnostics.append(
                _diagnostic(
                    "next_order_mismatch",
                    f"/nodes[{index}]/next_id",
                    _string_or_none(current.get("node_id")),
                )
            )
    if nodes and nodes[-1].get("next_id") is not None:
        diagnostics.append(
            _diagnostic(
                "last_next_not_null",
                f"/nodes[{len(nodes) - 1}]/next_id",
                _string_or_none(nodes[-1].get("node_id")),
            )
        )

    for index, anchor in enumerate(anchors):
        node_id = _string_or_none(anchor.get("node_id"))
        if node_id not in by_id:
            diagnostics.append(
                _diagnostic("anchor_missing_node", f"/anchors[{index}]/node_id", node_id)
            )
        if anchor.get("raw_text_embedded") is not False:
            diagnostics.append(
                _diagnostic(
                    "anchor_raw_text_embedded", f"/anchors[{index}]/raw_text_embedded", node_id
                )
            )
        if anchor.get("import_eligible") is not False:
            diagnostics.append(
                _diagnostic("anchor_import_eligible", f"/anchors[{index}]/import_eligible", node_id)
            )
        if anchor.get("promoted_to_fact") is not False:
            diagnostics.append(
                _diagnostic(
                    "anchor_promoted_to_fact", f"/anchors[{index}]/promoted_to_fact", node_id
                )
            )
    return [diagnostic.to_redacted_dict() for diagnostic in diagnostics]


def node_by_id(page_index: dict[str, Any], node_id: str) -> dict[str, Any] | None:
    """Return a node by stable ID."""
    return next(
        (
            node
            for node in _list_of_dicts(page_index.get("nodes"))
            if node.get("node_id") == node_id
        ),
        None,
    )


def children_of(page_index: dict[str, Any], node_id: str) -> list[dict[str, Any]]:
    """Return direct children in stored child order."""
    node = node_by_id(page_index, node_id)
    if node is None:
        return []
    by_id = {
        node["node_id"]: node
        for node in _list_of_dicts(page_index.get("nodes"))
        if isinstance(node.get("node_id"), str)
    }
    return [
        child
        for child_id in _string_list(node.get("children_ids"))
        if (child := by_id.get(child_id)) is not None
    ]


def path_to(page_index: dict[str, Any], node_id: str) -> list[str]:
    """Return the stable PageIndex path for a node ID."""
    node = node_by_id(page_index, node_id)
    return _string_list(node.get("path")) if node is not None else []


def walk_next(page_index: dict[str, Any]) -> list[dict[str, Any]]:
    """Walk nodes in deterministic NEXT order."""
    nodes = _list_of_dicts(page_index.get("nodes"))
    if not nodes:
        return []
    by_id = {node["node_id"]: node for node in nodes if isinstance(node.get("node_id"), str)}
    ordered = [nodes[0]]
    seen = {nodes[0].get("node_id")}
    current = nodes[0]
    while isinstance(current.get("next_id"), str) and current["next_id"] not in seen:
        next_node = by_id.get(current["next_id"])
        if next_node is None:
            break
        ordered.append(next_node)
        seen.add(next_node.get("node_id"))
        current = next_node
    return ordered


def to_json(value: dict[str, Any]) -> str:
    """Serialize a PageIndex contract dictionary deterministically."""
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def _manifest(
    *,
    paper_id: str,
    source_refs: list[dict[str, Any]],
    nodes: list[dict[str, Any]],
    anchors: list[dict[str, Any]],
    diagnostics: list[ArticlePageIndexDiagnostic],
) -> dict[str, Any]:
    diagnostic_dicts = [diagnostic.to_redacted_dict() for diagnostic in diagnostics]
    summary = _summary(nodes, anchors, diagnostic_dicts)
    return {
        "schema_version": ARTICLE_PAGE_INDEX_SCHEMA_VERSION,
        "diagnostics_schema_version": ARTICLE_PAGE_INDEX_DIAGNOSTICS_SCHEMA_VERSION,
        "builder": ARTICLE_PAGE_INDEX_BUILDER,
        "paper_id": paper_id,
        "source_refs": source_refs,
        "nodes": nodes,
        "anchors": anchors,
        "summary": summary,
        "diagnostics": diagnostic_dicts,
        "diagnostic_counts_by_code": _counts(
            diagnostic.get("code") for diagnostic in diagnostic_dicts
        ),
        "bridge_subtree": dict(BRIDGE_SUBTREE_STATUS),
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
    }


def _summary(
    nodes: list[dict[str, Any]], anchors: list[dict[str, Any]], diagnostics: list[dict[str, Any]]
) -> dict[str, int]:
    return {
        "node_count": len(nodes),
        "anchor_count": len(anchors),
        "missing_parent_count": sum(
            1
            for diagnostic in diagnostics
            if diagnostic.get("code") in {"missing_parent", "artifact_missing_section_parent"}
        ),
        "missing_span_count": sum(
            1 for diagnostic in diagnostics if diagnostic.get("code") == "missing_span"
        ),
        "fallback_count": sum(1 for node in nodes if node.get("node_type") == "fallback"),
        "blocker_count": sum(
            1 for diagnostic in diagnostics if diagnostic.get("blocks_import") is True
        ),
        "import_eligible_count": 0,
    }


def _fallback_node(paper_id: str) -> dict[str, Any]:
    fallback_id = f"{paper_id}:page-index:fallback:no-sections"
    return ArticlePageIndexNode(
        node_id=fallback_id,
        paper_id=paper_id,
        node_type="fallback",
        source_id=None,
        parent_id=None,
        children_ids=(),
        next_id=None,
        path=(fallback_id,),
        order=0,
        summary={"fallback_reason": "no_sections"},
        source_ref_ids=(),
        source_span=None,
        anchor_ids=(),
    ).to_redacted_dict()


def _artifact_node(
    *,
    paper_id: str,
    placeholder: dict[str, Any],
    placeholders: list[dict[str, Any]],
    parent_id: str,
    source_ref_ids: tuple[str, ...],
    spans: dict[str, ArticlePageIndexSourceSpan],
) -> tuple[dict[str, Any], list[dict[str, Any]], list[ArticlePageIndexDiagnostic]]:
    artifact_id = (
        _string_or_none(placeholder.get("artifact_id")) or f"{paper_id}:artifact:unknown:0000"
    )
    artifact_type = _string_or_none(placeholder.get("artifact_type")) or "unknown"
    node_id = _artifact_node_id(paper_id, artifact_id, artifact_type)
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    if artifact_type not in ALLOWED_PAGE_INDEX_ARTIFACT_TYPES:
        diagnostics.append(
            _diagnostic(
                "unsupported_artifact_type",
                _path_for_item(placeholders, placeholder, "artifact_placeholders", "artifact_type"),
                artifact_id,
            )
        )
    span_id = _string_or_none(placeholder.get("span_id"))
    source_span = spans.get(span_id) if span_id else None
    if span_id and source_span is None:
        diagnostics.append(
            _diagnostic(
                "missing_span",
                _path_for_item(placeholders, placeholder, "artifact_placeholders", "span_id"),
                artifact_id,
                severity="warning",
            )
        )

    anchor_ids: list[str] = []
    anchors: list[dict[str, Any]] = []
    if source_span is not None:
        anchor = ArticlePageIndexAnchor(
            anchor_id=_artifact_anchor_id(paper_id, artifact_id, artifact_type, primary=True),
            node_id=node_id,
            paper_id=paper_id,
            span_id=source_span.span_id,
            source_id=source_span.source_id,
            coordinate_space=source_span.coordinate_space,
            span_hash=source_span.span_hash,
            anchor_type=artifact_type,
        ).to_redacted_dict()
        anchors.append(anchor)
        anchor_ids.append(anchor["anchor_id"])
    caption_span_id = _string_or_none(placeholder.get("caption_span_id"))
    caption_span = spans.get(caption_span_id) if caption_span_id else None
    if caption_span_id and caption_span is None:
        diagnostics.append(
            _diagnostic(
                "missing_span",
                _path_for_item(
                    placeholders, placeholder, "artifact_placeholders", "caption_span_id"
                ),
                artifact_id,
                severity="warning",
            )
        )
    if caption_span is not None:
        anchor = ArticlePageIndexAnchor(
            anchor_id=_artifact_anchor_id(paper_id, artifact_id, artifact_type, primary=False),
            node_id=node_id,
            paper_id=paper_id,
            span_id=caption_span.span_id,
            source_id=caption_span.source_id,
            coordinate_space=caption_span.coordinate_space,
            span_hash=caption_span.span_hash,
            anchor_type="caption",
        ).to_redacted_dict()
        anchors.append(anchor)
        anchor_ids.append(anchor["anchor_id"])

    node = {
        "node_id": node_id,
        "paper_id": paper_id,
        "node_type": "artifact",
        "source_id": source_span.source_id if source_span is not None else None,
        "parent_id": parent_id,
        "children_ids": [],
        "next_id": None,
        "path": [],
        "order": 0,
        "summary": {
            "artifact_id": artifact_id,
            "artifact_type": artifact_type
            if artifact_type in ALLOWED_PAGE_INDEX_ARTIFACT_TYPES
            else "unknown",
            "section_id": _string_or_none(placeholder.get("section_id")),
            "has_caption_anchor": caption_span is not None,
            "has_candidate_link_targets": bool(
                _string_list(placeholder.get("candidate_link_targets"))
            ),
            "has_target_ref": isinstance(placeholder.get("target_ref"), str),
        },
        "source_ref_ids": list(source_ref_ids),
        "source_span": source_span.to_redacted_dict() if source_span is not None else None,
        "anchor_ids": anchor_ids,
        "import_eligible": False,
        "promoted_to_fact": False,
    }
    return node, anchors, diagnostics


def _apply_navigation(
    nodes: list[dict[str, Any]], children_by_parent: dict[str | None, list[str]]
) -> None:
    by_id = {node["node_id"]: node for node in nodes if isinstance(node.get("node_id"), str)}
    for order, node in enumerate(nodes):
        node["order"] = order
        node["next_id"] = nodes[order + 1]["node_id"] if order + 1 < len(nodes) else None
        node["children_ids"] = list(children_by_parent.get(node["node_id"], []))

    for node in nodes:
        path: list[str] = []
        current: dict[str, Any] | None = node
        seen: set[str] = set()
        while (
            current is not None
            and isinstance(current.get("node_id"), str)
            and current["node_id"] not in seen
        ):
            seen.add(current["node_id"])
            path.append(current["node_id"])
            parent_id = current.get("parent_id")
            current = by_id.get(parent_id) if isinstance(parent_id, str) else None
        node["path"] = list(reversed(path))


def _validate_structure_boundary(structure: dict[str, Any]) -> list[ArticlePageIndexDiagnostic]:
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    if structure.get("schema_version") != REDACTED_ARTICLE_STRUCTURE_SCHEMA_VERSION:
        diagnostics.append(_diagnostic("invalid_input_schema_version", "/schema_version"))
    if not isinstance(structure.get("paper_id"), str) or not structure.get("paper_id"):
        diagnostics.append(_diagnostic("missing_paper_id", "/paper_id"))
    diagnostics.extend(_validate_forbidden_keys(structure))
    diagnostics.extend(_validate_source_of_truth_markers(structure))
    diagnostics.extend(_validate_structure_safety_flags(structure.get("safety_flags")))
    diagnostics.extend(_validate_duplicate_sections(structure))
    return diagnostics


def _validate_duplicate_sections(structure: dict[str, Any]) -> list[ArticlePageIndexDiagnostic]:
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    seen: set[str] = set()
    for index, section in enumerate(_list_of_dicts(structure.get("sections"))):
        section_id = _string_or_none(section.get("section_id"))
        if not section_id:
            continue
        if section_id in seen:
            diagnostics.append(
                _diagnostic("duplicate_section_id", f"/sections[{index}]/section_id", section_id)
            )
        else:
            seen.add(section_id)
    return diagnostics


def _validate_structure_safety_flags(flags: Any) -> list[ArticlePageIndexDiagnostic]:
    if not isinstance(flags, dict):
        return [_diagnostic("missing_safety_flags", "/safety_flags")]
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    for key in sorted(UNSAFE_TRUE_FLAGS):
        if flags.get(key) is True:
            diagnostics.append(
                _diagnostic(f"unsafe_import_flag_true:{key}", f"/safety_flags/{key}")
            )
    return diagnostics


def _validate_top_level_import_flags(
    page_index: dict[str, Any],
) -> list[ArticlePageIndexDiagnostic]:
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    for key in ("production_import_attempted", "ladybugdb_written", "trusted_kg_import_allowed"):
        if page_index.get(key) is not False:
            diagnostics.append(_diagnostic(f"unsafe_import_flag_true:{key}", f"/{key}"))
    if page_index.get("import_eligible_count") != 0:
        diagnostics.append(_diagnostic("import_eligible_count_nonzero", "/import_eligible_count"))
    if page_index.get("promoted_to_fact_count") != 0:
        diagnostics.append(_diagnostic("promoted_to_fact_count_nonzero", "/promoted_to_fact_count"))
    bridge = page_index.get("bridge_subtree")
    if isinstance(bridge, dict):
        for key in (
            "trusted_kg_import_allowed",
            "ladybugdb_written",
            "production_import_attempted",
            "graph_import_claim",
        ):
            if bridge.get(key) is not False:
                diagnostics.append(
                    _diagnostic(f"unsafe_import_flag_true:{key}", f"/bridge_subtree/{key}")
                )
    return diagnostics


def _validate_span(
    span: dict[str, Any], path: str, object_id: str | None
) -> list[ArticlePageIndexDiagnostic]:
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    if span.get("coordinate_space") not in ALLOWED_PAGE_INDEX_COORDINATE_SPACES:
        diagnostics.append(
            _diagnostic("invalid_coordinate_space", f"{path}/coordinate_space", object_id)
        )
    if span.get("raw_text_embedded") is not False:
        diagnostics.append(
            _diagnostic("span_raw_text_embedded", f"{path}/raw_text_embedded", object_id)
        )
    coordinate_space = span.get("coordinate_space")
    has_chars = (
        isinstance(span.get("char_start"), int)
        and isinstance(span.get("char_end"), int)
        and span["char_end"] > span["char_start"] >= 0
    )
    has_page_bbox = (
        coordinate_space == "page_bbox"
        and isinstance(span.get("bbox"), list)
        and len(span["bbox"]) == 4
    )
    if coordinate_space != "artifact_record" and not has_chars and not has_page_bbox:
        diagnostics.append(_diagnostic("invalid_source_span_coordinates", path, object_id))
    return diagnostics


def _redacted_source_ref(source: dict[str, Any], paper_id: str) -> dict[str, Any]:
    return {
        "source_id": _string_or_none(source.get("source_id")),
        "paper_id": paper_id,
        "source_role": _string_or_none(source.get("source_role")),
        "source_path": _string_or_none(source.get("source_path")),
        "sha256": _string_or_none(source.get("sha256")),
        "media_type": _string_or_none(source.get("media_type")),
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
    }


def _span_from_structure(span: dict[str, Any]) -> ArticlePageIndexSourceSpan:
    bbox = span.get("bbox")
    return ArticlePageIndexSourceSpan(
        span_id=str(span.get("span_id")),
        source_id=str(span.get("source_id")),
        coordinate_space=str(span.get("coordinate_space")),
        char_start=span.get("char_start") if isinstance(span.get("char_start"), int) else None,
        char_end=span.get("char_end") if isinstance(span.get("char_end"), int) else None,
        page_start=span.get("page_start") if isinstance(span.get("page_start"), int) else None,
        page_end=span.get("page_end") if isinstance(span.get("page_end"), int) else None,
        # pyrefly: ignore [bad-argument-type]
        bbox=tuple(float(value) for value in bbox)
        if isinstance(bbox, list) and len(bbox) == 4
        else None,  # ty:ignore[invalid-argument-type]
        span_hash=_string_or_none(span.get("span_hash")),
    )


def _validate_forbidden_keys(value: Any, path: str = "") -> list[ArticlePageIndexDiagnostic]:
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in FORBIDDEN_PAYLOAD_KEYS:
                diagnostics.append(_diagnostic("forbidden_payload_key", child_path))
                continue
            diagnostics.extend(_validate_forbidden_keys(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_forbidden_keys(child, f"{path}[{index}]"))
    return diagnostics


def _validate_source_of_truth_markers(
    value: Any, path: str = ""
) -> list[ArticlePageIndexDiagnostic]:
    diagnostics: list[ArticlePageIndexDiagnostic] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key.lower() in FORBIDDEN_SOURCE_OF_TRUTH_KEYS:
                diagnostics.append(_diagnostic("source_of_truth_claim", child_path))
                continue
            diagnostics.extend(_validate_source_of_truth_markers(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            diagnostics.extend(_validate_source_of_truth_markers(child, f"{path}[{index}]"))
    return diagnostics


def _diagnostic(
    code: str,
    json_path: str,
    object_id: str | None = None,
    *,
    severity: PageIndexDiagnosticSeverity = "repair_required",
    blocks_import: bool = True,
) -> ArticlePageIndexDiagnostic:
    return ArticlePageIndexDiagnostic(
        code=code,
        json_path=json_path,
        object_id=object_id,
        severity=severity,
        blocks_import=blocks_import,
    )


def _paper_id(structure: dict[str, Any]) -> str:
    value = structure.get("paper_id")
    return value if isinstance(value, str) and value else "unknown-paper"


def _section_node_id(paper_id: str, section_id: str) -> str:
    return f"{paper_id}:page-index:section:{_section_slug(section_id)}"


def _artifact_node_id(paper_id: str, artifact_id: str, artifact_type: str) -> str:
    return f"{paper_id}:page-index:artifact:{artifact_type}:{_artifact_ordinal(artifact_id)}"


def _artifact_anchor_id(
    paper_id: str, artifact_id: str, artifact_type: str, *, primary: bool
) -> str:
    ordinal = _artifact_ordinal(artifact_id)
    if not primary:
        return f"{paper_id}:page-index-anchor:caption-{artifact_type}-{ordinal}"
    if artifact_type == "reference":
        return f"{paper_id}:page-index-anchor:citation-{ordinal}"
    return f"{paper_id}:page-index-anchor:{artifact_type}-{ordinal}"


def _section_slug(section_id: str) -> str:
    return section_id.rsplit(":", 1)[-1]


def _artifact_ordinal(artifact_id: str) -> str:
    return artifact_id.rsplit(":", 1)[-1]


def _path_for_item(
    items: list[dict[str, Any]], item: dict[str, Any], collection: str, field_name: str
) -> str:
    try:
        index = items.index(item)
    except ValueError:
        index = 0
    return f"/{collection}[{index}]/{field_name}"


def _list_of_dicts(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, dict)]


def _string_list(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [str(item) for item in value if item is not None]


def _int_list(value: Any) -> list[int]:
    if not isinstance(value, list):
        return []
    return [item for item in value if isinstance(item, int)]


def _string_or_none(value: Any) -> str | None:
    return str(value) if value is not None else None


def _counts(values: Any) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        if value is None:
            continue
        key = str(value)
        counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))


__all__ = [
    "ALLOWED_PAGE_INDEX_ARTIFACT_TYPES",
    "ALLOWED_PAGE_INDEX_COORDINATE_SPACES",
    "ALLOWED_PAGE_INDEX_SECTION_TYPES",
    "ARTICLE_PAGE_INDEX_BUILDER",
    "ARTICLE_PAGE_INDEX_DIAGNOSTICS_SCHEMA_VERSION",
    "ARTICLE_PAGE_INDEX_SCHEMA_VERSION",
    "ArticlePageIndexAnchor",
    "ArticlePageIndexDiagnostic",
    "ArticlePageIndexNode",
    "ArticlePageIndexSourceSpan",
    "build_article_page_index_from_structure",
    "children_of",
    "node_by_id",
    "path_to",
    "to_json",
    "validate_article_page_index",
    "walk_next",
]
