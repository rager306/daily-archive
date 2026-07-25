"""Upgrade char-only spans with page/bbox from ODL layout JSON (M282).

When layout element text matches a span surface (or body slice), attach page
and bbox. Never invents coordinates. Never import.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

SCHEMA_VERSION = "layout-span-upgrade.v1"


def _norm(text: str) -> str:
    return " ".join((text or "").casefold().split())


def _as_bbox(value: Any) -> list[float] | None:
    if not isinstance(value, (list, tuple)) or len(value) != 4:
        return None
    try:
        return [float(value[0]), float(value[1]), float(value[2]), float(value[3])]
    except (TypeError, ValueError):
        return None


def _text_of(node: Mapping[str, Any]) -> str:
    for key in ("text", "content", "value", "title", "markdown"):
        v = node.get(key)
        if isinstance(v, str) and v.strip():
            return v.strip()
    return ""


def iter_layout_elements(layout: Any) -> list[dict[str, Any]]:
    """Flatten layout tree into element dicts with optional page/bbox/text."""
    out: list[dict[str, Any]] = []

    def walk(node: Any) -> None:
        if isinstance(node, Mapping):
            text = _text_of(node)
            bbox = _as_bbox(
                node.get("bbox")
                or node.get("bounding_box")
                or node.get("boundingBox")
                or node.get("box")
            )
            page = node.get("page")
            if page is None:
                page = node.get("page_number") or node.get("pageIndex")
            try:
                page_i = int(page) if page is not None else None
            except (TypeError, ValueError):
                page_i = None
            if text or bbox is not None or page_i is not None:
                out.append(
                    {
                        "text": text,
                        "bbox": bbox,
                        "page": page_i,
                        "id": node.get("id") or node.get("element_id"),
                    }
                )
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for item in node:
                walk(item)

    walk(layout)
    return out


def match_layout_for_surface(
    surface: str,
    layout_elements: Sequence[Mapping[str, Any]],
) -> dict[str, Any] | None:
    """Best layout element for surface: exact norm match, else containment."""
    surf = _norm(surface)
    if not surf:
        return None
    exact: dict[str, Any] | None = None
    contains: dict[str, Any] | None = None
    for el in layout_elements:
        if not isinstance(el, Mapping):
            continue
        et = _norm(str(el.get("text") or ""))
        if not et:
            continue
        if et == surf and (el.get("page") is not None or el.get("bbox") is not None):
            exact = dict(el)
            break
        if surf in et and (el.get("page") is not None or el.get("bbox") is not None):
            if contains is None or len(et) < len(_norm(str(contains.get("text") or ""))):
                contains = dict(el)
    return exact or contains


def upgrade_span_with_layout(
    span: Mapping[str, Any],
    layout_elements: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Return span copy with page/bbox filled when layout matches surface."""
    out = dict(span)
    surface = str(span.get("surface") or "")
    if not surface and span.get("char_start") is not None:
        # cannot match without surface or external body slice
        return out
    hit = match_layout_for_surface(surface, layout_elements)
    if hit is None:
        out.setdefault("layout_upgrade", "no_match")
        return out
    if out.get("page") is None and hit.get("page") is not None:
        out["page"] = hit["page"]
    if out.get("bbox") is None and hit.get("bbox") is not None:
        out["bbox"] = hit["bbox"]
    if hit.get("id") is not None:
        out["element_id"] = str(hit["id"])
    out["layout_upgrade"] = "matched"
    if out.get("page") is not None or out.get("bbox") is not None:
        out["justified_char_only"] = False
    return out


def upgrade_spans_with_layout_json(
    spans: Sequence[Mapping[str, Any]],
    layout_json: Mapping[str, Any] | list[Any] | None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    """Upgrade a list of spans; return (new_spans, stats)."""
    if layout_json is None:
        return [dict(s) for s in spans if isinstance(s, Mapping)], {
            "layout_present": False,
            "upgraded": 0,
            "total": 0,
            "import_eligible": False,
        }
    elements = iter_layout_elements(layout_json)
    out: list[dict[str, Any]] = []
    upgraded = 0
    for span in spans:
        if not isinstance(span, Mapping):
            continue
        new = upgrade_span_with_layout(span, elements)
        if new.get("layout_upgrade") == "matched" and (
            new.get("page") is not None or new.get("bbox") is not None
        ):
            upgraded += 1
        out.append(new)
    stats = {
        "layout_present": True,
        "layout_elements": len(elements),
        "total": len(out),
        "upgraded": upgraded,
        "schema_version": SCHEMA_VERSION,
        "import_eligible": False,
        "graph_writes_allowed": False,
    }
    return out, stats


def upgrade_grounded_gold_with_layout(
    grounded_gold: Mapping[str, Any],
    layout_json: Mapping[str, Any] | list[Any] | None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Upgrade entity/relation spans inside a grounded gold dict."""
    gold = dict(grounded_gold)
    total_up = 0
    total_sp = 0
    for kind in ("entities", "relations"):
        items = gold.get(kind)
        if not isinstance(items, list):
            continue
        new_items: list[dict[str, Any]] = []
        for item in items:
            if not isinstance(item, Mapping):
                continue
            row = dict(item)
            spans = row.get("spans")
            if isinstance(spans, list) and spans:
                new_spans, st = upgrade_spans_with_layout_json(spans, layout_json)
                row["spans"] = new_spans
                total_up += int(st.get("upgraded") or 0)
                total_sp += int(st.get("total") or 0)
            new_items.append(row)
        gold[kind] = new_items
    stats = {
        "layout_present": layout_json is not None,
        "spans_total": total_sp,
        "spans_upgraded": total_up,
        "schema_version": SCHEMA_VERSION,
        "import_eligible": False,
        "graph_writes_allowed": False,
    }
    return gold, stats


__all__ = [
    "SCHEMA_VERSION",
    "iter_layout_elements",
    "match_layout_for_surface",
    "upgrade_span_with_layout",
    "upgrade_spans_with_layout_json",
    "upgrade_grounded_gold_with_layout",
]
