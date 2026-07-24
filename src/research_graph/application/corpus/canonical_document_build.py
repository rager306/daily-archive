"""Build CanonicalDocument from ODL layout JSON / markdown (M275).

Application pure. Prefer layout JSON when present; markdown is a projection
fallback. Never authorizes import.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from research_graph.domain.canonical_document import (
    SCHEMA_VERSION,
    CanonicalBlock,
    CanonicalDocument,
    CanonicalSection,
    SourceSpanRef,
)


def _as_bbox(value: Any) -> tuple[float, float, float, float] | None:
    if not isinstance(value, (list, tuple)) or len(value) != 4:
        return None
    try:
        return (float(value[0]), float(value[1]), float(value[2]), float(value[3]))
    except (TypeError, ValueError):
        return None


def _block_kind(node: Mapping[str, Any]) -> str:
    raw = str(
        node.get("type")
        or node.get("category")
        or node.get("element_type")
        or node.get("role")
        or "other"
    ).casefold()
    if any(k in raw for k in ("heading", "title", "header")):
        return "heading"
    if "section" in raw:
        return "section"
    if "table" in raw:
        return "table"
    if "figure" in raw or "image" in raw:
        return "figure"
    if "equation" in raw or "formula" in raw:
        return "equation"
    if "caption" in raw:
        return "caption"
    if "list" in raw:
        return "list_item"
    if "ref" in raw or "bibl" in raw:
        return "reference"
    if "para" in raw or "text" in raw or "p" == raw:
        return "paragraph"
    return "other"


def _text_of(node: Mapping[str, Any]) -> str:
    for key in ("text", "content", "value", "title", "markdown"):
        v = node.get(key)
        if isinstance(v, str) and v.strip():
            return v.strip()
    return ""


def _walk_layout_blocks(
    node: Any,
    *,
    layout_hash: str | None,
    out: list[CanonicalBlock],
    counter: list[int],
) -> None:
    if isinstance(node, Mapping):
        text = _text_of(node)
        kind = _block_kind(node)
        bbox = _as_bbox(
            node.get("bbox")
            or node.get("bounding_box")
            or node.get("boundingBox")
            or node.get("box")
        )
        page = node.get("page") or node.get("page_number") or node.get("pageIndex")
        try:
            page_i = int(page) if page is not None else None
        except (TypeError, ValueError):
            page_i = None
        element_id = node.get("id") or node.get("element_id") or node.get("uid")
        if text or bbox is not None or kind not in {"other", "paragraph"}:
            counter[0] += 1
            span = SourceSpanRef(
                artifact_role="odl_layout",
                artifact_hash=layout_hash,
                page=page_i,
                bbox=bbox,
                element_id=str(element_id) if element_id is not None else None,
            )
            out.append(
                CanonicalBlock(
                    block_id=f"b{counter[0]}",
                    kind=kind,  # type: ignore[arg-type]
                    text=text,
                    level=int(node.get("level") or 0),
                    spans=(span,),
                    meta={
                        k: node.get(k)
                        for k in ("type", "category", "page", "page_number")
                        if k in node
                    },
                )
            )
        for v in node.values():
            _walk_layout_blocks(v, layout_hash=layout_hash, out=out, counter=counter)
    elif isinstance(node, list):
        for item in node:
            _walk_layout_blocks(item, layout_hash=layout_hash, out=out, counter=counter)


def _sections_from_blocks(blocks: Sequence[CanonicalBlock]) -> list[CanonicalSection]:
    """Simple linear sectioning: heading opens a section; following blocks attach."""
    sections: list[CanonicalSection] = []
    current_blocks: list[CanonicalBlock] = []
    current_title = "Document"
    current_level = 0
    sec_i = 0

    def flush() -> None:
        nonlocal sec_i, current_blocks
        if not current_blocks and not sections:
            return
        sec_i += 1
        sections.append(
            CanonicalSection(
                section_id=f"s{sec_i}",
                title=current_title,
                level=current_level,
                blocks=tuple(current_blocks),
            )
        )
        current_blocks = []

    for b in blocks:
        if b.kind in {"heading", "section"} and b.text:
            flush()
            current_title = b.text
            current_level = max(1, b.level or 1)
            current_blocks = [b]
        else:
            current_blocks.append(b)
    flush()
    if not sections:
        sections.append(
            CanonicalSection(
                section_id="s1",
                title="Document",
                level=0,
                blocks=tuple(blocks),
            )
        )
    return sections


def build_canonical_document_from_odl(
    *,
    paper_id: str,
    layout_json: Mapping[str, Any] | list[Any] | None = None,
    layout_json_sha256: str | None = None,
    markdown: str | None = None,
    title: str | None = None,
    parser_runs: Sequence[Mapping[str, Any]] | None = None,
    source_hashes: Mapping[str, str] | None = None,
) -> CanonicalDocument:
    """Build IR from ODL layout JSON; fall back to markdown paragraphs."""
    blocks: list[CanonicalBlock] = []
    diagnostics: list[str] = []
    counter = [0]

    if layout_json is not None:
        _walk_layout_blocks(
            layout_json,
            layout_hash=layout_json_sha256,
            out=blocks,
            counter=counter,
        )
        diagnostics.append(f"layout_blocks:{len(blocks)}")
    elif markdown and markdown.strip():
        paras = [p.strip() for p in markdown.split("\n\n") if p.strip()]
        for i, para in enumerate(paras, start=1):
            kind = "heading" if para.startswith("#") else "paragraph"
            text = para.lstrip("#").strip() if kind == "heading" else para
            blocks.append(
                CanonicalBlock(
                    block_id=f"m{i}",
                    kind=kind,  # type: ignore[arg-type]
                    text=text,
                    level=para.count("#") if kind == "heading" else 0,
                    spans=(
                        SourceSpanRef(
                            artifact_role="markdown",
                            artifact_hash=(
                                dict(source_hashes).get("markdown")
                                if source_hashes
                                else None
                            ),
                        ),
                    ),
                )
            )
        diagnostics.append(f"markdown_blocks:{len(blocks)}")
    else:
        diagnostics.append("empty_inputs")

    sections = _sections_from_blocks(blocks)
    grounded = sum(1 for b in blocks if any(s.bbox is not None or s.page is not None for s in b.spans))
    diagnostics.append(f"blocks_with_page_or_bbox:{grounded}")

    return CanonicalDocument(
        schema_version=SCHEMA_VERSION,
        paper_id=paper_id,
        title=title,
        sections=tuple(sections),
        blocks=tuple(blocks),
        parser_runs=tuple(dict(p) for p in (parser_runs or ())),
        source_hashes=dict(source_hashes or {}),
        diagnostics=tuple(diagnostics),
    )


__all__ = [
    "build_canonical_document_from_odl",
]
