"""Wave B gold-hybrid constrained extraction pilot.

Candidate-first mode (2025–2026 practice):
  1. Deterministic candidates from hybrid body (keywords + multiword surfaces)
  2. Injectible selector assigns closed types / links among candidate ids
  3. Grounding: surface must appear in body (casefold)
  4. Score via evaluate_records + decide_gate_verdict; compare to lexical floor

Never DSPy. Never import. LLM is optional injectible boundary only.
"""

from __future__ import annotations

import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    ALLOWED_ENTITY_TYPES,
    ALLOWED_RELATION_TYPES,
    truncate_body_for_pilot,
)
from research_graph.application.corpus.wave_b_hybrid_statistical_extraction import (
    build_hybrid_statistical_extraction,
)
from research_graph.application.extraction_ablations import decide_gate_verdict
from research_graph.application.extraction_benchmark import evaluate_records

SCHEMA_VERSION = "wave-b-reviewed-gold-hybrid-constrained-pilot.v1"

# (body_text, case_id, candidates) -> {"entities":[...], "relations":[...], "json_valid": bool}
ConstrainedSelectFn = Callable[
    [str, str, Sequence[Mapping[str, Any]]], Mapping[str, Any]
]

_TOKEN_RE = re.compile(r"[A-Za-z\u0400-\u04FF][A-Za-z0-9\u0400-\u04FF'\-]{1,}")
_CONNECTORS = frozenset(
    {"and", "of", "for", "with", "via", "the", "a", "an", "to", "in", "on", "vs"}
)
# Closed-class / boilerplate tokens that make weak multiword candidates.
_STOPWORDS = _CONNECTORS | frozenset(
    {
        "this",
        "that",
        "these",
        "those",
        "they",
        "them",
        "their",
        "there",
        "then",
        "than",
        "when",
        "where",
        "which",
        "what",
        "who",
        "whom",
        "whose",
        "how",
        "why",
        "are",
        "was",
        "were",
        "been",
        "being",
        "have",
        "has",
        "had",
        "having",
        "do",
        "does",
        "did",
        "done",
        "can",
        "could",
        "should",
        "would",
        "may",
        "might",
        "must",
        "will",
        "shall",
        "not",
        "nor",
        "but",
        "or",
        "if",
        "from",
        "into",
        "onto",
        "over",
        "under",
        "about",
        "after",
        "before",
        "between",
        "through",
        "during",
        "without",
        "within",
        "also",
        "such",
        "only",
        "just",
        "very",
        "more",
        "most",
        "other",
        "some",
        "any",
        "each",
        "every",
        "all",
        "both",
        "few",
        "many",
        "much",
        "our",
        "your",
        "its",
        "his",
        "her",
        "out",
        "up",
        "down",
        "off",
        "again",
        "further",
        "once",
        "here",
        "using",
        "used",
        "use",
        "based",
        "paper",
        "we",
        "present",
        "show",
        "propose",
        "proposed",
        "method",
        "methods",
        "model",
        "models",
        "approach",
        "results",
        "figure",
        "table",
        "section",
        "introduction",
        "conclusion",
        "abstract",
        "et",
        "al",
        "edu",
        "com",
        "org",
        "https",
        "http",
        "www",
    }
)
_HEADER_LINE_RE = re.compile(r"^(#{1,6}|\*\*)\s*(.+?)(\*\*)?\s*$", re.MULTILINE)
_ALLCAPS_TITLE_RE = re.compile(
    r"(?m)^(?:#{1,6}\s*)?([A-Z][A-Z0-9][A-Z0-9 ,:;'/\-]{8,120})$"
)


def _normalize_surface(value: str) -> str:
    return " ".join(value.casefold().strip().split())


def _slug(value: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "_", value.casefold().strip())
    return s.strip("_")[:80] or "x"


def surface_in_body(surface: str, body_text: str) -> bool:
    """True when normalized surface is a substring of normalized body."""
    surf = _normalize_surface(surface)
    if not surf:
        return False
    return surf in _normalize_surface(body_text or "")


def _is_content_cap(tok: str) -> bool:
    """Title-case or ALLCAPS content token (not pure connector)."""
    if not tok or tok.casefold() in _CONNECTORS:
        return False
    return tok[:1].isupper()


def _add_phrase_and_subspans(counts: dict[str, int], parts: list[str]) -> None:
    """Count full phrase and contiguous 2..n content-word subspans."""
    if len(parts) < 2:
        return
    full = " ".join(parts)
    counts[full] = counts.get(full, 0) + 1
    # contiguous windows of length 2..min(4, n)
    n = len(parts)
    for width in range(2, min(4, n) + 1):
        for start in range(0, n - width + 1):
            window = parts[start : start + width]
            # skip windows that start/end with connector
            if window[0].casefold() in _CONNECTORS or window[-1].casefold() in _CONNECTORS:
                continue
            # at most one connector inside
            conn = sum(1 for w in window if w.casefold() in _CONNECTORS)
            if conn > 1:
                continue
            phrase = " ".join(window)
            counts[phrase] = counts.get(phrase, 0) + 1


def _extract_titleish_phrases(text: str, *, max_phrases: int = 48) -> list[str]:
    """Extract multiword technical phrases (Title/ALLCAPS + at most one connector).

    Emits subspans so gold-style "Neural Machine Translation" is recovered even
    when embedded in header noise like "BACKGROUND NEURAL MACHINE TRANSLATION".
    """
    tokens = list(_TOKEN_RE.finditer(text))
    if not tokens:
        return []
    counts: dict[str, int] = {}
    n = len(tokens)
    i = 0
    while i < n:
        tok = tokens[i].group(0)
        if not _is_content_cap(tok):
            i += 1
            continue
        end = i
        j = i + 1
        connectors_used = 0
        content_words = 1
        while j < n and content_words < 5:
            nxt = tokens[j].group(0)
            if _is_content_cap(nxt):
                end = j
                content_words += 1
                j += 1
                continue
            if (
                connectors_used == 0
                and nxt.casefold() in _CONNECTORS
                and j + 1 < n
                and _is_content_cap(tokens[j + 1].group(0))
                and content_words < 5
            ):
                end = j + 1
                connectors_used = 1
                content_words += 1
                j += 2
                continue
            break
        if end > i:
            surface = text[tokens[i].start() : tokens[end].end()].strip()
            parts = surface.split()
            while parts and parts[0].casefold() in _CONNECTORS:
                parts = parts[1:]
            while parts and parts[-1].casefold() in _CONNECTORS:
                parts = parts[:-1]
            _add_phrase_and_subspans(counts, parts)
            i = end + 1
        else:
            i += 1
    ranked = sorted(
        counts.items(), key=lambda kv: (-kv[1], -len(kv[0]), kv[0].casefold())
    )
    return [s for s, _ in ranked[:max_phrases]]


def _phrase_quality(surface: str) -> tuple:
    """Sort key: lower is better; demote bibliography/noise phrases."""
    s = surface.strip()
    low = s.casefold()
    noise_hits = sum(
        1
        for n in (
            "et al",
            "proc.",
            "proceedings",
            "university",
            "doi",
            "arxiv",
            "http",
            "figure",
            "table",
            "pp.",
            "vol.",
            "ieee",
            "acm",
            "##",
            "[cs.",
        )
        if n in low
    )
    # punctuation-heavy spans from markdown headers / citations
    punct = sum(1 for ch in s if ch in ":;#[](){}")
    has_digit = any(ch.isdigit() for ch in s)
    words = len(s.split())
    stop_hits = sum(1 for w in low.split() if w in _STOPWORDS)
    return (noise_hits, stop_hits, punct, 1 if has_digit else 0, abs(words - 3), len(s))


def _is_tech_token(tok: str) -> bool:
    """Alnum tech (Seq2Seq) or short ALLCAPS/acronym (GEPA, BERT)."""
    if not tok:
        return False
    if any(ch.isdigit() for ch in tok):
        return True
    if tok.isupper() and tok.isalpha() and 3 <= len(tok) <= 8:
        return True
    # CamelCase / mixed alnum without being pure Title-case word
    if any(ch.isupper() for ch in tok[1:]) and any(ch.islower() for ch in tok):
        return True
    return False


def _is_weak_ngram(parts: Sequence[str]) -> bool:
    """Reject stopword-heavy / function-word n-grams that crowd out titles.

    Technical method names like "Seq2Seq Models" end with a common noun that is
    also in the soft stop list — keep them when a tech/acronym token is present.
    """
    if not parts:
        return True
    lows = [p.casefold() for p in parts]
    tech = any(_is_tech_token(p) for p in parts)
    if not tech:
        if lows[0] in _STOPWORDS or lows[-1] in _STOPWORDS:
            return True
    else:
        # still reject pure function edges, not domain nouns (models/network)
        _edge_func = _CONNECTORS | {
            "the", "a", "an", "and", "or", "of", "for", "with", "to", "in", "on",
            "this", "that", "these", "those", "we", "our", "using", "via",
        }
        if lows[0] in _edge_func or lows[-1] in _edge_func:
            return True
    content = [p for p in lows if p not in _STOPWORDS and p not in _CONNECTORS]
    if tech:
        # tech token counts as content even if sibling is soft-stop domain noun
        content = [p for p in lows if p not in _CONNECTORS]
    if len(content) < (1 if tech else 2):
        return True
    stop_ratio = sum(1 for p in lows if p in _STOPWORDS) / max(1, len(lows))
    # allow one soft-stop domain noun beside a tech token
    if tech and stop_ratio <= 0.5:
        return False
    return stop_ratio > 0.34


def _title_case_surface(parts: Sequence[str]) -> str:
    """Canonical display form: Title Case content, keep connectors lower.

    Preserve ALLCAPS acronyms and alnum tech tokens (GEPA, Seq2Seq).
    """
    out: list[str] = []
    for p in parts:
        if p.casefold() in _CONNECTORS:
            out.append(p.casefold())
        elif _is_tech_token(p):
            out.append(p)  # keep Seq2Seq / GEPA / BERT intact
        elif p.isupper() and len(p) > 1:
            out.append(p[:1].upper() + p[1:].lower())
        else:
            out.append(p)
    return " ".join(out)


def _extract_markdown_header_phrases(text: str, *, max_phrases: int = 48) -> list[str]:
    """Pull multiword + strong single tokens from markdown / ALLCAPS title lines.

    Priority source for gold-style labels that live in paper titles/abstract heads.
    Emits subspans and Title-Case variants so ALLCAPS headers match gold casing.
    """
    counts: dict[str, int] = {}
    header_blobs: list[str] = []

    for match in _HEADER_LINE_RE.finditer(text or ""):
        raw = (match.group(2) or "").strip()
        if raw:
            header_blobs.append(raw)
    for match in _ALLCAPS_TITLE_RE.finditer(text or ""):
        raw = (match.group(1) or "").strip()
        if raw:
            header_blobs.append(raw)

    # Also treat first non-empty lines as soft headers (hybrid bodies vary).
    line_budget = 0
    for line in (text or "").splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("#") or stripped.isupper() or stripped[:1].isupper():
            header_blobs.append(stripped.lstrip("#").strip())
            line_budget += 1
            if line_budget >= 12:
                break

    for blob in header_blobs:
        # Drop author/email-ish lines early.
        low = blob.casefold()
        if any(x in low for x in ("@", ".edu", "arxiv:", "http", "university")):
            continue
        # Leading ALLCAPS acronym before colon/dash (e.g. "GEPA: Reflective...").
        lead = re.match(r"^\s*([A-Z]{3,8})\s*[:\-—–]", blob)
        if lead:
            acr = lead.group(1)
            counts[acr] = counts.get(acr, 0) + 8
        tokens = list(_TOKEN_RE.finditer(blob))
        if not tokens:
            continue
        parts = [t.group(0) for t in tokens]
        # Full header phrase + subspans (2..min(6, n))
        n = len(parts)
        if n >= 2:
            for width in range(2, min(6, n) + 1):
                for start in range(0, n - width + 1):
                    window = parts[start : start + width]
                    if window[0].casefold() in _CONNECTORS or window[-1].casefold() in _CONNECTORS:
                        continue
                    conn = sum(1 for w in window if w.casefold() in _CONNECTORS)
                    if conn > 1:
                        continue
                    if _is_weak_ngram(window):
                        continue
                    surface_raw = " ".join(window)
                    surface_tc = _title_case_surface(window)
                    counts[surface_raw] = counts.get(surface_raw, 0) + 2
                    counts[surface_tc] = counts.get(surface_tc, 0) + 3
        # Strong single-token titles (e.g. gold label "Interaction").
        # Weight short title-like headers higher so singles are not crowded out
        # by long abstract soft-header n-grams.
        title_like = n <= 8 and not any(
            w.casefold() in {"abstract", "introduction", "conclusion"} for w in parts
        )
        if title_like:
            for idx, p in enumerate(parts):
                if p.casefold() in _STOPWORDS or p.casefold() in _CONNECTORS:
                    continue
                # ALLCAPS/acronyms (GEPA, BERT) may be 3-6 chars; else require length >= 5.
                is_acronym = p.isupper() and p.isalpha() and 3 <= len(p) <= 8
                if not is_acronym and len(p) < 5:
                    continue
                if not (p[:1].isupper() or p.isupper()):
                    continue
                surface_tc = _title_case_surface([p])
                # Terminal content token of a short title is especially valuable.
                boost = 5 if is_acronym else (4 if idx == n - 1 else 2)
                if is_acronym:
                    boost += 2 if idx == 0 else 0
                counts[surface_tc] = counts.get(surface_tc, 0) + boost
                counts[p] = counts.get(p, 0) + boost
                # Prefer exact acronym surface as its own candidate key
                if is_acronym:
                    counts[p.upper()] = counts.get(p.upper(), 0) + boost + 1

    ranked = sorted(
        counts.items(), key=lambda kv: (-kv[1], _phrase_quality(kv[0]), kv[0].casefold())
    )
    return [s for s, _ in ranked[:max_phrases]]


def build_body_candidates(
    body_text: str,
    *,
    paper_id: str = "",
    top_k_keywords: int = 16,
    max_multiword: int = 32,
    max_total: int = 96,
) -> list[dict[str, Any]]:
    """Deterministic candidate inventory from hybrid body (no LLM, no gold).

    Priority order (fills slots before lower-priority sources can crowd them out):
      1. markdown / ALLCAPS title-header phrases (+ Title-Case variants)
      2. titleish multiword from document head
      3. titleish multiword from tail (small budget)
      4. stopword-filtered head n-grams
      5. statistical keywords
    """
    text = body_text or ""
    stats = build_hybrid_statistical_extraction(
        paper_id=paper_id or "unknown",
        body_text=text,
        body_path=None,
        top_k=top_k_keywords,
    )
    candidates: list[dict[str, Any]] = []
    seen: set[str] = set()

    def _add(surface: str, source: str) -> bool:
        """Add grounded surface; return True if accepted."""
        if len(candidates) >= max_total:
            return False
        norm = _normalize_surface(surface)
        if not norm or norm in seen:
            return False
        if not surface_in_body(surface, text):
            return False
        seen.add(norm)
        cid = f"c:{_slug(surface)}:{len(candidates)}"
        candidates.append(
            {
                "candidate_id": cid,
                "surface": surface.strip(),
                "surface_norm": norm,
                "source": source,
            }
        )
        return True

    # Prefer head-of-document phrases (titles/abstract); bibliography tails are noisy.
    head = text[:4500]
    tail = text[4500:]

    # 1) Title / header phrases first (never short-circuit before this finishes).
    for surface in _extract_markdown_header_phrases(
        head, max_phrases=max_multiword * 3
    ):
        _add(surface, "header_title")

    # 2) Title-case / ALLCAPS multiword runs in head
    head_phrases = sorted(
        _extract_titleish_phrases(head, max_phrases=max_multiword * 2),
        key=_phrase_quality,
    )
    for surface in head_phrases:
        _add(surface, "multiword_titleish_head")
        # Title-Case variant for ALLCAPS extractions
        parts = surface.split()
        if parts and all(p.isupper() or p.casefold() in _CONNECTORS for p in parts):
            _add(_title_case_surface(parts), "multiword_titleish_head_tc")

    # 3) Small tail budget (methods sections), only if slots remain
    if len(candidates) < max_total:
        for surface in _extract_titleish_phrases(
            tail, max_phrases=max(8, max_multiword // 3)
        ):
            _add(surface, "multiword_titleish_tail")

    # 4) Stopword-filtered content n-grams from head (secondary)
    if len(candidates) < max_total:
        head_tokens = list(_TOKEN_RE.finditer(head))
        ngram_counts: dict[str, int] = {}
        for width in (4, 3, 2):
            for i in range(0, max(0, len(head_tokens) - width + 1)):
                parts = [head_tokens[i + k].group(0) for k in range(width)]
                if _is_weak_ngram(parts):
                    continue
                conn = sum(1 for p in parts if p.casefold() in _CONNECTORS)
                if conn > 1:
                    continue
                surface = " ".join(parts)
                if len(surface) < 6 or len(surface) > 80:
                    continue
                if not surface_in_body(surface, head):
                    continue
                ngram_counts[surface] = ngram_counts.get(surface, 0) + 1
                # Title-Case form helps match gold labels from ALLCAPS regions
                if any(p.isupper() and len(p) > 1 for p in parts):
                    tc = _title_case_surface(parts)
                    if surface_in_body(tc, head):
                        ngram_counts[tc] = ngram_counts.get(tc, 0) + 2
        ranked_ngrams = sorted(
            ngram_counts.items(),
            key=lambda kv: (_phrase_quality(kv[0]), -kv[1], kv[0].casefold()),
        )
        ngram_budget = max(8, max_multiword)
        added_ngrams = 0
        for surface, _count in ranked_ngrams:
            if added_ngrams >= ngram_budget or len(candidates) >= max_total:
                break
            if _add(surface, "head_ngram"):
                added_ngrams += 1

    # 5) Statistical keywords fill remaining slots
    if len(candidates) < max_total:
        for kw in stats.keywords:
            surface = str(
                kw.get("keyword") or kw.get("token") or kw.get("term") or ""
            ).strip()
            if surface:
                _add(surface, "statistical_keyword")
            if len(candidates) >= max_total:
                break

    return candidates


def build_constrained_prediction_record(
    *,
    case_id: str,
    paper_id: str,
    body_text: str,
    candidates: Sequence[Mapping[str, Any]],
    selection: Mapping[str, Any],
    source_artifact_refs: Sequence[str] | None = None,
    latency_ms: int = 0,
    retry_count: int = 0,
) -> dict[str, Any]:
    """Map constrained selection onto evaluate_records prediction shape.

    selection entities items: candidate_id + type (or entity_type)
    selection relations items: type + source_id/target_id (candidate ids)
      or source_label/target_label matching candidate surfaces
    """
    by_id = {
        str(c.get("candidate_id")): c
        for c in candidates
        if isinstance(c, Mapping) and c.get("candidate_id")
    }
    by_norm = {
        str(c.get("surface_norm") or _normalize_surface(str(c.get("surface") or ""))): c
        for c in candidates
        if isinstance(c, Mapping) and c.get("surface")
    }

    json_valid = bool(selection.get("json_valid", True))
    entities_out: list[dict[str, Any]] = []
    selected_ids: dict[str, str] = {}  # candidate_id -> pred entity id

    for idx, ent in enumerate(selection.get("entities") or []):
        if not isinstance(ent, Mapping):
            continue
        cid = str(ent.get("candidate_id") or ent.get("id") or "").strip()
        cand = by_id.get(cid)
        if cand is None:
            # allow surface match fallback
            label_hint = str(ent.get("label") or ent.get("surface") or "").strip()
            cand = by_norm.get(_normalize_surface(label_hint))
            if cand is not None:
                cid = str(cand.get("candidate_id"))
        if cand is None:
            continue
        surface = str(cand.get("surface") or "").strip()
        if not surface_in_body(surface, body_text):
            continue
        etype = str(ent.get("type") or ent.get("entity_type") or "").strip()
        if etype.islower():
            etype = etype[:1].upper() + etype[1:]
        aliases = {
            "field": "Field",
            "task": "Task",
            "method": "Method",
            "dataset": "Dataset",
            "model": "Model",
            "metric": "Metric",
        }
        etype = aliases.get(etype.casefold(), etype)
        if etype not in ALLOWED_ENTITY_TYPES:
            continue
        eid = f"pred:constrained:{case_id}:{etype}:{_slug(surface)}:{idx}"
        selected_ids[cid] = eid
        entities_out.append(
            {
                "id": eid,
                "type": etype,
                "label": surface,
                "evidence_refs": [f"evidence:constrained:{case_id}:{_slug(surface)}"],
            }
        )

    relations_out: list[dict[str, Any]] = []
    for idx, rel in enumerate(selection.get("relations") or []):
        if not isinstance(rel, Mapping):
            continue
        rtype = str(rel.get("type") or rel.get("relation_type") or "").strip()
        if rtype.islower():
            rtype = rtype.upper()
        if rtype not in ALLOWED_RELATION_TYPES:
            continue
        src_raw = str(
            rel.get("source_id")
            or rel.get("source")
            or rel.get("source_label")
            or rel.get("from_name")
            or ""
        ).strip()
        tgt_raw = str(
            rel.get("target_id")
            or rel.get("target")
            or rel.get("target_label")
            or rel.get("to_name")
            or ""
        ).strip()
        src_id = selected_ids.get(src_raw)
        tgt_id = selected_ids.get(tgt_raw)
        if src_id is None:
            # label / surface path
            cand = by_norm.get(_normalize_surface(src_raw))
            if cand is not None:
                src_id = selected_ids.get(str(cand.get("candidate_id")))
        if tgt_id is None:
            cand = by_norm.get(_normalize_surface(tgt_raw))
            if cand is not None:
                tgt_id = selected_ids.get(str(cand.get("candidate_id")))
        if not src_id or not tgt_id:
            continue
        relations_out.append(
            {
                "id": f"pred:constrained:rel:{case_id}:{idx}",
                "type": rtype,
                "source": src_id,
                "target": tgt_id,
                "evidence_refs": [f"evidence:constrained:{case_id}:relation:{idx}"],
            }
        )

    refs = list(source_artifact_refs or [])
    if not refs:
        refs = [f"artifact:hybrid-body:{paper_id or case_id}"]

    return {
        "case_id": case_id,
        "paper_id": paper_id,
        "source_artifact_refs": refs,
        "entities": entities_out,
        "relations": relations_out,
        "schema_valid": json_valid,
        "json_valid": json_valid,
        "operational": {
            "cost_estimate": 0.0,
            "latency_ms": int(latency_ms),
            "retry_count": int(retry_count),
        },
    }


def lexical_oracle_select(
    body_text: str,
    case_id: str,
    candidates: Sequence[Mapping[str, Any]],
    *,
    gold: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Diagnostic-only selector: pick candidates that match gold labels.

    NEVER use as production extract path — measures candidate-coverage ceiling.
    """
    del body_text, case_id  # candidates already grounded
    gold = gold or {}
    gold_ents = [e for e in (gold.get("entities") or []) if isinstance(e, Mapping)]
    gold_by_norm = {
        _normalize_surface(str(e.get("label") or "")): e
        for e in gold_ents
        if e.get("label")
    }
    entities: list[dict[str, Any]] = []
    for c in candidates:
        if not isinstance(c, Mapping):
            continue
        norm = str(c.get("surface_norm") or _normalize_surface(str(c.get("surface") or "")))
        g = gold_by_norm.get(norm)
        if g is None:
            continue
        entities.append(
            {
                "candidate_id": str(c.get("candidate_id")),
                "type": str(g.get("type") or "Method"),
            }
        )
    # relations if both endpoints selected
    selected_norms = {
        str(c.get("surface_norm"))
        for c in candidates
        if isinstance(c, Mapping)
        and any(
            e.get("candidate_id") == c.get("candidate_id") for e in entities
        )
    }
    # map gold id -> candidate_id via label
    gold_id_to_cand: dict[str, str] = {}
    for c in candidates:
        if not isinstance(c, Mapping):
            continue
        norm = str(c.get("surface_norm") or "")
        g = gold_by_norm.get(norm)
        if g and g.get("id"):
            gold_id_to_cand[str(g.get("id"))] = str(c.get("candidate_id"))

    relations: list[dict[str, Any]] = []
    for rel in gold.get("relations") or []:
        if not isinstance(rel, Mapping):
            continue
        src = gold_id_to_cand.get(str(rel.get("source") or ""))
        tgt = gold_id_to_cand.get(str(rel.get("target") or ""))
        rtype = str(rel.get("type") or "")
        if src and tgt and rtype:
            relations.append(
                {"type": rtype, "source_id": src, "target_id": tgt}
            )
    del selected_norms
    return {"entities": entities, "relations": relations, "json_valid": True}


@dataclass(frozen=True, slots=True)
class GoldHybridConstrainedPilotPackage:
    schema_version: str
    case_count: int
    metrics: dict[str, Any]
    floor_metrics: dict[str, Any] | None
    gate_verdict: str
    gate_reasons: tuple[str, ...]
    per_case: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    llm_used: bool = False
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    model_id: str = ""
    mode: str = "constrained_select"

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("constrained pilot cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("constrained pilot cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "mode": self.mode,
            "case_count": self.case_count,
            "metrics": dict(self.metrics),
            "floor_metrics": dict(self.floor_metrics) if self.floor_metrics else None,
            "gate_verdict": self.gate_verdict,
            "gate_reasons": list(self.gate_reasons),
            "per_case": list(self.per_case),
            "diagnostics": list(self.diagnostics),
            "llm_used": self.llm_used,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "model_id": self.model_id,
            "note": (
                "Constrained candidate-select pilot; grounding required; "
                "not free-form labels; not DSPy; not import"
            ),
        }


def score_gold_hybrid_constrained_pilot(
    *,
    cases: Sequence[Mapping[str, Any]],
    select_fn: ConstrainedSelectFn | None = None,
    floor_metrics: Mapping[str, Any] | None = None,
    model_id: str = "",
    max_body_chars: int = 8000,
    llm_used: bool = False,
    use_lexical_oracle: bool = False,
) -> GoldHybridConstrainedPilotPackage:
    """Score joined gold+body cases with constrained selection.

    Each case: case_id, paper_id, gold, body_text.
    Provide select_fn, or use_lexical_oracle=True for diagnostic ceiling.
    """
    if select_fn is None and not use_lexical_oracle:
        raise ValueError(
            "score_gold_hybrid_constrained_pilot requires select_fn "
            "or use_lexical_oracle=True"
        )

    golds: list[dict[str, Any]] = []
    preds: list[dict[str, Any]] = []
    per_case: list[dict[str, Any]] = []

    for case in cases:
        gold = dict(case.get("gold") or {})
        body_text = str(case.get("body_text") or "")
        case_id = str(case.get("case_id") or gold.get("case_id") or "unknown")
        paper_id = str(case.get("paper_id") or gold.get("paper_id") or "")
        if not gold.get("case_id"):
            gold["case_id"] = case_id
        if not gold.get("paper_id"):
            gold["paper_id"] = paper_id
        gold.setdefault("source_artifact_refs", ["artifact:catalog-unknown"])
        gold.setdefault("schema_valid", True)
        gold.setdefault("json_valid", True)
        gold.setdefault(
            "operational",
            {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
        )
        gold.setdefault("entities", [])
        gold.setdefault("relations", [])

        window = truncate_body_for_pilot(body_text, max_chars=max_body_chars)
        candidates = build_body_candidates(window, paper_id=paper_id)

        if use_lexical_oracle:
            selection = lexical_oracle_select(
                window, case_id, candidates, gold=gold
            )
        else:
            assert select_fn is not None
            selection = dict(select_fn(window, case_id, candidates))

        pred = build_constrained_prediction_record(
            case_id=case_id,
            paper_id=paper_id,
            body_text=window,
            candidates=candidates,
            selection=selection,
            source_artifact_refs=gold.get("source_artifact_refs"),
        )
        golds.append(gold)
        preds.append(pred)

        gold_norms = {
            _normalize_surface(str(e.get("label") or ""))
            for e in (gold.get("entities") or [])
            if isinstance(e, Mapping)
        }
        cand_norms = {str(c.get("surface_norm") or "") for c in candidates}
        coverage = len(gold_norms & cand_norms)
        per_case.append(
            {
                "case_id": case_id,
                "paper_id": paper_id,
                "candidate_count": len(candidates),
                "gold_label_coverage": coverage,
                "gold_entity_count": len(gold.get("entities") or []),
                "entity_predicted": len(pred.get("entities") or []),
                "relation_predicted": len(pred.get("relations") or []),
                "json_valid": bool(pred.get("json_valid")),
                "import_eligible": False,
                # Observability for empty-LLM → header fallback (optional keys).
                "fallback_used": bool(selection.get("fallback_used")),
                "fallback_reason": selection.get("fallback_reason"),
            }
        )

    if not golds:
        metrics: dict[str, Any] = {
            "entity_f1": 0.0,
            "relation_f1": 0.0,
            "entity_precision": 0.0,
            "entity_recall": 0.0,
            "relation_precision": 0.0,
            "relation_recall": 0.0,
            "evidence_path_validity": 1.0,
            "case_count": 0,
            "prediction_count": 0,
        }
        gate = decide_gate_verdict(metrics, paper_count=0)
    else:
        metrics = evaluate_records(golds, preds)
        gate = decide_gate_verdict(metrics, paper_count=len(golds))

    floor = dict(floor_metrics) if floor_metrics else None
    beats_floor = None
    if floor is not None:
        beats_floor = float(metrics.get("entity_f1") or 0.0) >= float(
            floor.get("entity_f1") or 0.0
        ) and float(metrics.get("relation_f1") or 0.0) >= float(
            floor.get("relation_f1") or 0.0
        )

    mode = "lexical_oracle_diagnostic" if use_lexical_oracle else "constrained_select"
    diagnostics = (
        f"case_count:{len(golds)}",
        f"entity_f1:{metrics.get('entity_f1')}",
        f"relation_f1:{metrics.get('relation_f1')}",
        f"entity_recall:{metrics.get('entity_recall')}",
        f"gate_verdict:{gate.verdict}",
        f"mode:{mode}",
        f"model:{model_id or 'none'}",
        f"beats_lexical_floor:{beats_floor}",
        f"llm:{str(llm_used).lower()}",
        "dspy:false",
        "import_write_fail_closed",
        "candidate_first_grounded",
    )
    return GoldHybridConstrainedPilotPackage(
        schema_version=SCHEMA_VERSION,
        case_count=len(golds),
        metrics=dict(metrics),
        floor_metrics=floor,
        gate_verdict=str(gate.verdict),
        gate_reasons=tuple(gate.reasons),
        per_case=tuple(per_case),
        diagnostics=diagnostics,
        llm_used=llm_used,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
        model_id=model_id,
        mode=mode,
    )


__all__ = [
    "SCHEMA_VERSION",
    "ConstrainedSelectFn",
    "GoldHybridConstrainedPilotPackage",
    "build_body_candidates",
    "build_constrained_prediction_record",
    "lexical_oracle_select",
    "score_gold_hybrid_constrained_pilot",
    "surface_in_body",
]
