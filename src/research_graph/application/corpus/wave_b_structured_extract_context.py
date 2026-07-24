"""Wave B structured extract context pack (not raw-MD-only).

Assembles deterministic preprocess signals already available in the repo:
  - outline headings / section windows
  - statistical keywords + term-dense evidence windows
  - grounded multiword candidates (header-first)
  - compact body head (title/abstract region)
  - section catalog for optional follow-up requests

Never pageindex graph write. Never import_eligible.
This is the handoff payload for LLM/select pilots so the model is not
re-fed only truncated raw markdown after we already computed structure.
"""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.article_preprocess import (
    build_article_preprocess_package,
)
from research_graph.application.corpus.keyword_spans import locate_keyword_spans
from research_graph.application.corpus.outline_signals import extract_outline_signals
from research_graph.application.corpus.term_dense_window import term_dense_window
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    build_body_candidates,
)
from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    truncate_body_for_pilot,
)
from research_graph.application.corpus.wave_b_hybrid_statistical_extraction import (
    build_hybrid_statistical_extraction,
)

SCHEMA_VERSION = "wave-b-structured-extract-context.v1"

_SECTION_SPLIT_RE = re.compile(r"(?m)^(#{1,6}\s+\S.*|\d+(?:\.\d+)*[.)]?\s+\S.+)$")


@dataclass(frozen=True, slots=True)
class StructuredExtractContext:
    """Assembled context for extract pilots. Fail-closed on import/writes."""

    schema_version: str
    paper_id: str
    case_id: str
    language: str
    language_confidence: float
    quality_status: str
    outline: tuple[dict[str, Any], ...]
    section_catalog: tuple[dict[str, Any], ...]
    sections: tuple[dict[str, Any], ...]
    keywords: tuple[str, ...]
    term_dense_windows: tuple[dict[str, Any], ...]
    candidates: tuple[dict[str, Any], ...]
    page_index_nodes: tuple[dict[str, Any], ...]
    body_head: str
    body_chars: int
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    dspy_optimizer_enabled: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("structured extract context cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("structured extract context cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "paper_id": self.paper_id,
            "case_id": self.case_id,
            "language": self.language,
            "language_confidence": self.language_confidence,
            "quality_status": self.quality_status,
            "outline": list(self.outline),
            "section_catalog": list(self.section_catalog),
            "sections": list(self.sections),
            "keywords": list(self.keywords),
            "term_dense_windows": list(self.term_dense_windows),
            "candidates": list(self.candidates),
            "page_index_nodes": list(self.page_index_nodes),
            "body_head": self.body_head,
            "body_chars": self.body_chars,
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "dspy_optimizer_enabled": False,
            "note": (
                "Structured extract handoff: outline + sections + candidates + "
                "term-dense windows. Not raw-md-only. Not import."
            ),
        }

    def section_by_id(self, section_id: str) -> dict[str, Any] | None:
        raw = str(section_id or "").strip()
        want = raw.casefold()
        if not want:
            return None
        # exact section_id
        for sec in self.sections:
            if str(sec.get("section_id")) == raw:
                return dict(sec)
        # exact title match
        for sec in self.sections:
            if str(sec.get("title") or "").casefold().strip() == want:
                return dict(sec)
        # fuzzy only against the request string (never title-in-own-sid)
        for sec in self.sections:
            sid = str(sec.get("section_id") or "").casefold()
            title = str(sec.get("title") or "").casefold().strip()
            tail = sid.rsplit(":", 1)[-1] if sid else ""
            if title and len(title) >= 4 and (title in want or want in title):
                return dict(sec)
            if tail and len(tail) >= 4 and (tail in want or want.endswith(tail)):
                return dict(sec)
            # bare token request like "method" vs sec:3:method
            if want and want == tail:
                return dict(sec)
        return None

    def resolve_followup_sections(
        self, requested: Sequence[str], *, max_sections: int = 4
    ) -> list[dict[str, Any]]:
        """Return section bodies for follow-up requests (id or title)."""
        out: list[dict[str, Any]] = []
        seen: set[str] = set()
        for raw in requested:
            if len(out) >= max_sections:
                break
            key = str(raw or "").strip()
            if not key:
                continue
            sec = self.section_by_id(key)
            if sec is None:
                continue
            sid = str(sec.get("section_id"))
            if sid in seen:
                continue
            seen.add(sid)
            out.append(sec)
        return out


def _split_sections(text: str, *, max_sections: int = 24) -> list[dict[str, Any]]:
    """Split body into heading-bounded sections with stable ids."""
    lines = (text or "").splitlines()
    if not lines:
        return []
    # find heading line indices
    heads: list[tuple[int, str]] = []
    for i, line in enumerate(lines):
        if _SECTION_SPLIT_RE.match(line.strip()):
            heads.append((i, line.strip()))
    sections: list[dict[str, Any]] = []
    if not heads:
        body = "\n".join(lines).strip()
        if body:
            sections.append(
                {
                    "section_id": "sec:0:body",
                    "title": "body",
                    "level": 1,
                    "start_line": 0,
                    "end_line": len(lines),
                    "char_count": len(body),
                    "text": body,
                }
            )
        return sections

    # preamble before first heading
    first_i = heads[0][0]
    if first_i > 0:
        pre = "\n".join(lines[:first_i]).strip()
        if pre:
            sections.append(
                {
                    "section_id": "sec:0:preamble",
                    "title": "preamble",
                    "level": 1,
                    "start_line": 0,
                    "end_line": first_i,
                    "char_count": len(pre),
                    "text": pre,
                }
            )

    for idx, (start, title) in enumerate(heads):
        end = heads[idx + 1][0] if idx + 1 < len(heads) else len(lines)
        chunk = "\n".join(lines[start:end]).strip()
        level = 1
        if title.startswith("#"):
            level = min(6, len(title) - len(title.lstrip("#")))
        slug = re.sub(r"[^a-z0-9]+", "-", title.casefold())[:48].strip("-") or "sec"
        sections.append(
            {
                "section_id": f"sec:{idx + 1}:{slug}",
                "title": title.lstrip("#").strip(),
                "level": level,
                "start_line": start,
                "end_line": end,
                "char_count": len(chunk),
                "text": chunk,
            }
        )
        if len(sections) >= max_sections:
            break
    return sections


def build_structured_extract_context(
    *,
    body_text: str,
    paper_id: str = "",
    case_id: str = "",
    max_body_chars: int = 12000,
    max_sections: int = 16,
    max_section_chars: int = 1800,
    max_candidates: int = 48,
    max_keywords: int = 16,
    max_term_windows: int = 6,
    include_full_section_text: bool = True,
) -> StructuredExtractContext:
    """Build structured handoff from hybrid body (deterministic, no LLM)."""
    text = body_text or ""
    window = truncate_body_for_pilot(text, max_chars=max_body_chars)
    preprocess = build_article_preprocess_package(
        source_id=paper_id or case_id or "unknown",
        text=window,
        source_class="hybrid_body",
        profile="scholarly",
        is_html=False,
    )
    cleaned = preprocess.cleaned_text or window
    outline = extract_outline_signals(cleaned)
    stats = build_hybrid_statistical_extraction(
        paper_id=paper_id or "unknown",
        body_text=cleaned,
        body_path=None,
        top_k=max_keywords,
    )
    keywords = [
        str(k.get("keyword") or k.get("token") or k.get("term") or "").strip()
        for k in stats.keywords
        if isinstance(k, Mapping)
    ]
    keywords = [k for k in keywords if k][:max_keywords]

    spans = locate_keyword_spans(cleaned, keywords, max_per_keyword=4)
    term_windows: list[dict[str, Any]] = []
    # one global dense window + per top keyword
    if spans.spans:
        global_win = term_dense_window(cleaned, spans=spans.spans, max_chars=480)
        term_windows.append(
            {
                "kind": "global",
                "keyword": "*",
                "start": global_win.start,
                "end": global_win.end,
                "hit_count": global_win.hit_count,
                "snippet": global_win.snippet,
            }
        )
    for kw in keywords[:max_term_windows]:
        kw_spans = locate_keyword_spans(cleaned, [kw], max_per_keyword=4)
        if not kw_spans.spans:
            continue
        win = term_dense_window(cleaned, spans=kw_spans.spans, max_chars=320)
        term_windows.append(
            {
                "kind": "keyword",
                "keyword": kw,
                "start": win.start,
                "end": win.end,
                "hit_count": win.hit_count,
                "snippet": win.snippet,
            }
        )

    candidates = build_body_candidates(
        cleaned,
        paper_id=paper_id,
        max_total=max_candidates,
    )
    cand_rows = [
        {
            "candidate_id": c.get("candidate_id"),
            "surface": c.get("surface"),
            "source": c.get("source"),
        }
        for c in candidates
    ]

    sections_raw = _split_sections(cleaned, max_sections=max_sections)
    sections: list[dict[str, Any]] = []
    catalog: list[dict[str, Any]] = []
    page_index_nodes: list[dict[str, Any]] = []
    parent_stack: list[tuple[int, str]] = []  # (level, node_id)
    for order, sec in enumerate(sections_raw):
        text_sec = str(sec.get("text") or "")
        if len(text_sec) > max_section_chars:
            text_sec = text_sec[: max_section_chars - 20] + "\n[...section truncated...]"
        section_id = str(sec["section_id"])
        level = int(sec["level"] or 1)
        # PageIndex-shaped navigation node (deterministic, body-derived; not graph write).
        while parent_stack and parent_stack[-1][0] >= level:
            parent_stack.pop()
        parent_id = parent_stack[-1][1] if parent_stack else None
        node_id = f"page_index:{paper_id or case_id or 'paper'}:{section_id}"
        path_titles = [t for _, t in parent_stack] + [str(sec["title"])]
        page_index_nodes.append(
            {
                "page_index_node_id": node_id,
                "section_id": section_id,
                "title": sec["title"],
                "level": level,
                "order": order,
                "parent_id": parent_id,
                "path": path_titles,
                "char_count": sec["char_count"],
                "start_line": sec["start_line"],
                "end_line": sec["end_line"],
            }
        )
        parent_stack.append((level, node_id))
        entry = {
            "section_id": section_id,
            "page_index_node_id": node_id,
            "title": sec["title"],
            "level": level,
            "start_line": sec["start_line"],
            "end_line": sec["end_line"],
            "char_count": sec["char_count"],
        }
        catalog.append(dict(entry))
        if include_full_section_text:
            entry = {**entry, "text": text_sec}
        sections.append(entry)

    outline_rows = [
        {
            "text": h.text,
            "level": h.level,
            "source": h.source,
            "line_index": h.line_index,
        }
        for h in outline.headings[:40]
    ]

    # Compact head for title/abstract (still structured pack, not sole payload)
    body_head = cleaned[:2200]

    diagnostics = (
        f"paper_id:{paper_id}",
        f"case_id:{case_id}",
        f"body_chars:{len(text)}",
        f"window_chars:{len(window)}",
        f"outline:{len(outline_rows)}",
        f"sections:{len(sections)}",
        f"keywords:{len(keywords)}",
        f"candidates:{len(cand_rows)}",
        f"term_windows:{len(term_windows)}",
        f"page_index_nodes:{len(page_index_nodes)}",
        f"language:{preprocess.language}",
        f"quality:{preprocess.quality_status}",
        "structured_not_raw_only",
        "page_index_bridge_body_derived",
        "import_write_fail_closed",
        "dspy:false",
    )
    return StructuredExtractContext(
        schema_version=SCHEMA_VERSION,
        paper_id=paper_id,
        case_id=case_id,
        language=preprocess.language,
        language_confidence=float(preprocess.language_confidence),
        quality_status=preprocess.quality_status,
        outline=tuple(outline_rows),
        section_catalog=tuple(catalog),
        sections=tuple(sections),
        keywords=tuple(keywords),
        term_dense_windows=tuple(term_windows),
        candidates=tuple(cand_rows),
        page_index_nodes=tuple(page_index_nodes),
        body_head=body_head,
        body_chars=len(text),
        diagnostics=diagnostics,
        import_eligible=False,
        graph_writes_allowed=False,
        dspy_optimizer_enabled=False,
    )


def render_structured_extract_prompt(
    ctx: StructuredExtractContext,
    *,
    allowed_entity_types: Sequence[str],
    allowed_relation_types: Sequence[str],
    followup_sections: Sequence[Mapping[str, Any]] | None = None,
) -> str:
    """Render model-facing prompt from structured context (not raw-md dump)."""
    entity_types = ", ".join(sorted(allowed_entity_types))
    rel_types = ", ".join(sorted(allowed_relation_types))

    outline_lines = []
    for h in ctx.outline[:30]:
        indent = "  " * max(0, int(h.get("level") or 1) - 1)
        outline_lines.append(f"{indent}- {h.get('text')}")

    catalog_lines = [
        f"- {s.get('section_id')} | page_index={s.get('page_index_node_id')}: "
        f"{s.get('title')} ({s.get('char_count')} chars)"
        for s in ctx.section_catalog[:24]
    ]
    page_index_lines = [
        f"- {n.get('page_index_node_id')} path={' > '.join(n.get('path') or [])}"
        for n in ctx.page_index_nodes[:24]
    ]
    cand_lines = [
        f"- {c.get('candidate_id')}: {c.get('surface')} [{c.get('source')}]"
        for c in ctx.candidates[:40]
    ]
    kw_line = ", ".join(ctx.keywords[:20]) or "(none)"

    section_blocks: list[str] = []
    for sec in ctx.sections[:12]:
        body = str(sec.get("text") or "")
        if not body:
            continue
        section_blocks.append(
            f"### {sec.get('section_id')} | {sec.get('title')}\n{body}"
        )

    dense_blocks: list[str] = []
    for w in ctx.term_dense_windows[:8]:
        dense_blocks.append(
            f"[{w.get('kind')}:{w.get('keyword')}] hits={w.get('hit_count')}\n"
            f"{w.get('snippet')}"
        )

    follow_blocks: list[str] = []
    for sec in followup_sections or []:
        follow_blocks.append(
            f"### FOLLOWUP {sec.get('section_id')} | {sec.get('title')}\n"
            f"{sec.get('text')}"
        )

    return (
        f"case_id={ctx.case_id}\n"
        f"paper_id={ctx.paper_id}\n"
        f"language={ctx.language} conf={ctx.language_confidence:.2f}\n"
        f"quality_status={ctx.quality_status}\n"
        f"body_chars={ctx.body_chars}\n"
        f"Allowed entity types: {entity_types}\n"
        f"Allowed relation types: {rel_types}\n"
        "You receive STRUCTURED paper context (outline, sections, candidates, "
        "term-dense windows). Prefer candidates and section evidence over invention.\n"
        "Extract ONLY the 2-4 most central contributions.\n"
        "Labels must be multi-word technical phrases grounded in candidates/sections "
        "when possible.\n"
        "If evidence is insufficient, you MAY request more sections instead of guessing:\n"
        '  {"need_sections": ["sec:3:..."], "entities": [], "relations": []}\n'
        "Otherwise return final JSON only.\n"
        "JSON schema:\n"
        "{\n"
        '  "entities": [{"type": "Method|Task|Field|Dataset|Model|Metric", '
        '"label": "...", "evidence_section_id": "sec:..."}],\n'
        '  "relations": [{"type": "APPLIED_TO|USES_COMPONENT|EVALUATED_ON|OUTPERFORMS",'
        ' "source_label": "...", "target_label": "..."}],\n'
        '  "need_sections": []\n'
        "}\n"
        "--- OUTLINE ---\n"
        + ("\n".join(outline_lines) if outline_lines else "(no outline)")
        + "\n--- SECTION CATALOG (requestable) ---\n"
        + ("\n".join(catalog_lines) if catalog_lines else "(no sections)")
        + "\n--- PAGEINDEX NODES (body-derived navigation) ---\n"
        + ("\n".join(page_index_lines) if page_index_lines else "(none)")
        + "\n--- KEYWORDS ---\n"
        + kw_line
        + "\n--- GROUNDED CANDIDATES ---\n"
        + ("\n".join(cand_lines) if cand_lines else "(no candidates)")
        + "\n--- TERM-DENSE WINDOWS ---\n"
        + ("\n\n".join(dense_blocks) if dense_blocks else "(none)")
        + "\n--- BODY HEAD (title/abstract region) ---\n"
        + (ctx.body_head or "(empty)")
        + "\n--- SECTIONS ---\n"
        + ("\n\n".join(section_blocks) if section_blocks else "(none)")
        + (
            "\n--- FOLLOWUP SECTIONS ---\n" + "\n\n".join(follow_blocks)
            if follow_blocks
            else ""
        )
        + "\n--- END STRUCTURED CONTEXT ---\n"
        "JSON:"
    )


def parse_need_sections(payload: Mapping[str, Any]) -> list[str]:
    raw = payload.get("need_sections")
    if not isinstance(raw, list):
        return []
    out: list[str] = []
    for item in raw:
        s = str(item or "").strip()
        if s:
            out.append(s)
    return out


__all__ = [
    "SCHEMA_VERSION",
    "StructuredExtractContext",
    "build_structured_extract_context",
    "parse_need_sections",
    "render_structured_extract_prompt",
]
