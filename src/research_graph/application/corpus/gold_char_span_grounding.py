"""Attach justified char spans from hybrid body to gold labels (M281).

When a gold entity/relation surface appears in body text, emit SourceSpan-like
mappings with artifact_hash=body_sha256 and char_start/end. Page/bbox remain
null until layout IR is available — char-only is the justified fallback.

Never invents free labels. Never import.
"""

from __future__ import annotations

import hashlib
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    surface_in_body,
)

SCHEMA_VERSION = "gold-char-span-grounding.v1"


def body_artifact_hash(body_text: str) -> str:
    return hashlib.sha256((body_text or "").encode("utf-8")).hexdigest()


def locate_surface_char_span(
    surface: str,
    body_text: str,
) -> tuple[int, int] | None:
    """Return (start, end) offsets in original body, or None.

    Strategies (first hit wins):
    1. casefold exact substring on original
    2. token-flexible whitespace regex (case-insensitive)
    """
    surf = (surface or "").strip()
    body = body_text or ""
    if not surf or not body:
        return None
    # 1) direct casefold scan
    body_cf = body.casefold()
    surf_cf = surf.casefold()
    idx = body_cf.find(surf_cf)
    if idx >= 0:
        return idx, idx + len(surf)

    # 2) whitespace-flexible token match
    tokens = [t for t in re.split(r"\s+", surf) if t]
    if len(tokens) < 1:
        return None
    pattern = r"\s+".join(re.escape(t) for t in tokens)
    match = re.search(pattern, body, flags=re.IGNORECASE)
    if match is None:
        return None
    return match.start(), match.end()


def span_dict_for_surface(
    surface: str,
    body_text: str,
    *,
    artifact_hash: str | None = None,
) -> dict[str, Any] | None:
    """Build one justified char-only span if surface is in body."""
    if not surface_in_body(surface, body_text):
        # still try locate in case normalize differs slightly
        loc = locate_surface_char_span(surface, body_text)
        if loc is None:
            return None
    else:
        loc = locate_surface_char_span(surface, body_text)
        if loc is None:
            return None
    start, end = loc
    if not (0 <= start < end <= len(body_text or "")):
        return None
    h = artifact_hash or body_artifact_hash(body_text)
    return {
        "artifact_role": "hybrid_body_markdown",
        "artifact_hash": h,
        "page": None,
        "bbox": None,
        "char_start": start,
        "char_end": end,
        "surface": surface,
        "justified_char_only": True,
    }


def _entity_surface(entity: Mapping[str, Any]) -> str:
    for key in ("label", "text", "surface", "name"):
        v = entity.get(key)
        if isinstance(v, str) and v.strip():
            return v.strip()
    return ""


def _relation_surface(rel: Mapping[str, Any]) -> str:
    for key in ("text", "label", "surface", "evidence_text"):
        v = rel.get(key)
        if isinstance(v, str) and v.strip():
            return v.strip()
    # compose from endpoints if present
    parts = [
        str(rel.get(k) or "").strip()
        for k in ("source_label", "target_label", "type")
        if rel.get(k)
    ]
    return " ".join(parts).strip()


@dataclass(frozen=True, slots=True)
class GoldCharSpanGroundingResult:
    schema_version: str
    case_id: str
    paper_id: str
    body_sha256: str
    entity_total: int
    entity_grounded: int
    relation_total: int
    relation_grounded: int
    gold: dict[str, Any]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "case_id": self.case_id,
            "paper_id": self.paper_id,
            "body_sha256": self.body_sha256,
            "entity_total": self.entity_total,
            "entity_grounded": self.entity_grounded,
            "relation_total": self.relation_total,
            "relation_grounded": self.relation_grounded,
            "gold": self.gold,
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Char-span grounding from hybrid body. Page/bbox null until layout IR. "
                "Justified char-only fallback. Never import."
            ),
        }


def attach_char_spans_to_gold_case(
    *,
    gold: Mapping[str, Any],
    body_text: str,
    case_id: str | None = None,
    paper_id: str | None = None,
) -> GoldCharSpanGroundingResult:
    """Return gold copy with spans attached to entities/relations when grounded."""
    body = body_text or ""
    h = body_artifact_hash(body)
    cid = str(case_id or gold.get("case_id") or "unknown")
    pid = str(paper_id or gold.get("paper_id") or "").replace("arxiv:", "")

    entities_in = list(gold.get("entities") or [])
    relations_in = list(gold.get("relations") or [])
    entities_out: list[dict[str, Any]] = []
    relations_out: list[dict[str, Any]] = []
    eg = rg = 0

    for ent in entities_in:
        if not isinstance(ent, Mapping):
            continue
        item = dict(ent)
        surface = _entity_surface(item)
        span = span_dict_for_surface(surface, body, artifact_hash=h) if surface else None
        if span is not None:
            item["spans"] = [span]
            eg += 1
        else:
            item["spans"] = list(item.get("spans") or []) if isinstance(item.get("spans"), list) else []
            # do not invent spans
            if not item["spans"]:
                item["spans"] = []
        entities_out.append(item)

    for rel in relations_in:
        if not isinstance(rel, Mapping):
            continue
        item = dict(rel)
        # Prefer explicit evidence text; else ground on endpoint surfaces present in body.
        candidates = [
            _relation_surface(item),
            str(item.get("source_label") or "").strip(),
            str(item.get("target_label") or "").strip(),
            str(item.get("subject") or "").strip(),
            str(item.get("object") or "").strip(),
        ]
        span = None
        for surface in candidates:
            if not surface:
                continue
            span = span_dict_for_surface(surface, body, artifact_hash=h)
            if span is not None:
                break
        if span is not None:
            item["spans"] = [span]
            rg += 1
        else:
            item["spans"] = (
                list(item.get("spans") or [])
                if isinstance(item.get("spans"), list)
                else []
            )
            if not item["spans"]:
                item["spans"] = []
        relations_out.append(item)

    gold_out = {
        **dict(gold),
        "entities": entities_out,
        "relations": relations_out,
        "case_id": cid,
        "paper_id": pid or gold.get("paper_id"),
        "body_sha256": h,
        "char_span_grounding": SCHEMA_VERSION,
    }
    et, rt = len(entities_out), len(relations_out)
    diagnostics = (
        f"entity_total:{et}",
        f"entity_grounded:{eg}",
        f"relation_total:{rt}",
        f"relation_grounded:{rg}",
        f"body_chars:{len(body)}",
        f"body_sha256:{h[:12]}",
        "mode:justified_char_only",
        "import_write_fail_closed",
    )
    return GoldCharSpanGroundingResult(
        schema_version=SCHEMA_VERSION,
        case_id=cid,
        paper_id=pid,
        body_sha256=h,
        entity_total=et,
        entity_grounded=eg,
        relation_total=rt,
        relation_grounded=rg,
        gold=gold_out,
        diagnostics=diagnostics,
    )


def attach_char_spans_batch(
    cases: Sequence[Mapping[str, Any]],
) -> list[GoldCharSpanGroundingResult]:
    """Batch: each case needs gold + body_text (or gold nested)."""
    out: list[GoldCharSpanGroundingResult] = []
    for case in cases:
        if not isinstance(case, Mapping):
            continue
        gold = case.get("gold") if isinstance(case.get("gold"), Mapping) else case
        if not isinstance(gold, Mapping):
            continue
        body = str(case.get("body_text") or case.get("body") or "")
        out.append(
            attach_char_spans_to_gold_case(
                gold=gold,
                body_text=body,
                case_id=str(case.get("case_id") or gold.get("case_id") or ""),
                paper_id=str(case.get("paper_id") or gold.get("paper_id") or ""),
            )
        )
    return out


__all__ = [
    "SCHEMA_VERSION",
    "body_artifact_hash",
    "locate_surface_char_span",
    "span_dict_for_surface",
    "GoldCharSpanGroundingResult",
    "attach_char_spans_to_gold_case",
    "attach_char_spans_batch",
]
