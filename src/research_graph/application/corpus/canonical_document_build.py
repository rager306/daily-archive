"""Build CanonicalDocument from ODL layout JSON / markdown (M275).

Application pure. Prefer layout JSON when present; markdown is a projection
fallback. Never authorizes import.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
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


def write_canonical_document_artifact(
    path: Path,
    document: CanonicalDocument,
) -> str:
    """Write CanonicalDocument JSON; return sha256 hex. Never import-eligible."""
    import json

    from research_graph.application.corpus.parser_run_artifacts import write_text_artifact

    payload = json.dumps(document.to_dict(), indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    return write_text_artifact(path, payload)


def persist_canonical_from_odl_metrics(
    *,
    body_dir: Path,
    paper_id: str,
    odl_metrics: Mapping[str, Any] | None,
    grobid_metrics: Mapping[str, Any] | None = None,
    title: str | None = None,
) -> tuple[CanonicalDocument | None, list[str]]:
    """Build+write CanonicalDocument from hybrid ODL metrics.

    Returns (document_or_none, diagnostics). Import remains false.
    """
    diag: list[str] = []
    body_dir = Path(body_dir)
    if not isinstance(odl_metrics, Mapping):
        return None, ["canonical_skipped_no_odl_metrics"]

    layout = odl_metrics.get("layout_json")
    markdown = odl_metrics.get("markdown")
    if layout is None and not (isinstance(markdown, str) and markdown.strip()):
        return None, ["canonical_skipped_no_layout_or_markdown"]

    layout_hash = odl_metrics.get("layout_json_sha256")
    if layout is not None and not layout_hash:
        from research_graph.application.corpus.parser_run_artifacts import sha256_text
        import json as _json

        try:
            layout_hash = sha256_text(
                _json.dumps(layout, sort_keys=True, ensure_ascii=False)
            )
        except (TypeError, ValueError):
            layout_hash = None

    md_hash = None
    if isinstance(markdown, str) and markdown:
        from research_graph.application.corpus.parser_run_artifacts import sha256_text

        md_hash = sha256_text(markdown)

    source_hashes: dict[str, str] = {}
    if layout_hash:
        source_hashes["odl_layout"] = str(layout_hash)
    if md_hash:
        source_hashes["markdown"] = md_hash
    if isinstance(grobid_metrics, Mapping):
        tei_sha = grobid_metrics.get("tei_sha256")
        if tei_sha:
            source_hashes["tei"] = str(tei_sha)

    parser_runs: list[dict[str, Any]] = []
    if isinstance(odl_metrics, Mapping):
        parser_runs.append(
            {
                "parser": "opendataloader",
                "format": odl_metrics.get("format"),
                "bbox_source": odl_metrics.get("bbox_source"),
                "layout_element_count": odl_metrics.get("layout_element_count"),
                "bounding_box_count": odl_metrics.get("bounding_box_count"),
            }
        )
    if isinstance(grobid_metrics, Mapping):
        parser_runs.append(
            {
                "parser": "grobid",
                "status": grobid_metrics.get("status"),
                "tei_sha256": grobid_metrics.get("tei_sha256"),
                "structured_parse_ok": grobid_metrics.get("structured_parse_ok"),
            }
        )

    doc = build_canonical_document_from_odl(
        paper_id=paper_id,
        layout_json=layout if isinstance(layout, (dict, list)) else None,
        layout_json_sha256=str(layout_hash) if layout_hash else None,
        markdown=markdown if isinstance(markdown, str) else None,
        title=title,
        parser_runs=parser_runs,
        source_hashes=source_hashes,
    )
    out_path = body_dir / f"{paper_id}.canonical.json"
    digest = write_canonical_document_artifact(out_path, doc)
    diag.append(f"canonical_document_path:{out_path.name}")
    diag.append(f"canonical_document_sha256:{digest}")
    diag.extend(list(doc.diagnostics))
    grounded = sum(
        1
        for b in doc.blocks
        if any(s.page is not None or s.bbox is not None for s in b.spans)
    )
    diag.append(f"canonical_blocks:{len(doc.blocks)}")
    diag.append(f"canonical_grounded_blocks:{grounded}")
    return doc, diag
