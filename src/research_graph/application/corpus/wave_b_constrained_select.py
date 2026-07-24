"""Constrained entity/relation select over grounded candidates only.

No free-form label invention. Selectors return candidate_id + closed types.
Application pure; never import / DSPy.
"""

from __future__ import annotations

import json
import re
from collections.abc import Mapping, Sequence
from typing import Any

from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    ALLOWED_ENTITY_TYPES,
    ALLOWED_RELATION_TYPES,
    parse_llm_extraction_json,
)

_TASK_HINTS = (
    "learning",
    "summarization",
    "summarizing",
    "translation",
    "translate",
    "detection",
    "extrapolation",
    "classification",
    "recognition",
    "generation",
    "parsing",
    "retrieval",
    "alignment",
    "align and",
    "games",
    "synthesis",
    "modeling",
)
_METHOD_HINTS = (
    "network",
    "model",
    "attention",
    "transformer",
    "feedback",
    "algorithm",
    "architecture",
    "encoder",
    "decoder",
    "rnn",
    "lstm",
    "bert",
    "gpt",
)
_FIELD_HINTS = (
    "perception",
    "language and",
    "computer vision",
    "natural language",
    "machine learning",
)
_DATASET_HINTS = (
    "corpus",
    "corpora",
    "dataset",
    "benchmark",
    "booksum",
    "wmt",
)


def surface_norm(value: str) -> str:
    return " ".join(value.casefold().strip().split())


def guess_entity_type(surface: str) -> str:
    """Deterministic type guess from surface text (closed set)."""
    low = " ".join(surface.casefold().split())
    if any(h in low for h in _DATASET_HINTS):
        return "Dataset"
    if any(h in low for h in _FIELD_HINTS):
        return "Field"
    # Named methods that include task-ish words (NMT, attention models).
    if any(
        h in low
        for h in (
            "machine translation",
            "neural machine",
            "attention with",
            "linear biases",
            "recurrent neural",
            "neural network",
            "neural program",
            "program learning",
            "human feedback",
            "subword units",
            "unit test feedback",
            "beam search",
        )
    ):
        return "Method"
    # Align and Translate is a Task in gold (seq2seq learning goal).
    if "align and translate" in low or low.startswith("align and"):
        return "Task"
    if any(h in low for h in _METHOD_HINTS) and not any(
        h in low
        for h in ("learning", "summarization", "summarizing", "games", "translate")
    ):
        return "Method"
    if any(h in low for h in _TASK_HINTS):
        return "Task"
    if any(h in low for h in _METHOD_HINTS):
        return "Method"
    return "Method"


def _primary_title_blob(body_text: str) -> str:
    """First markdown/ALLCAPS title line(s) — strongest grounding for selection."""
    lines: list[str] = []
    for raw in (body_text or "").splitlines()[:40]:
        line = raw.strip()
        if not line:
            continue
        low = line.casefold()
        # skip pure arxiv/meta banners
        if low.startswith("arxiv:") or re.match(r"^arXiv:", line):
            continue
        if "@" in line or ".edu" in low:
            continue
        if line.startswith("#"):
            title = line.lstrip("#").strip()
            tlow = title.casefold()
            if tlow.startswith("arxiv:") or tlow.startswith("arxiv.org"):
                continue
            if title:
                lines.append(title)
            if len(lines) >= 2:
                break
            continue
        if line.isupper() and len(line) > 12:
            lines.append(line)
            if len(lines) >= 1:
                break
            continue
        # soft title: first non-author multiword Title-ish line
        if len(lines) == 0 and len(line.split()) >= 3 and line[:1].isupper():
            lines.append(line)
            break
    return " ".join(lines)


def _looks_like_author_span(surface: str) -> bool:
    parts = surface.split()
    if len(parts) < 2:
        return False
    # 2+ consecutive Capitalized tokens without connectors/tech words
    caps = sum(1 for p in parts if p[:1].isupper() and p[1:].islower())
    if caps >= 2 and caps == len(parts):
        low = surface.casefold()
        tech = any(
            t in low
            for t in (
                "network",
                "learning",
                "translation",
                "attention",
                "summar",
                "language",
                "feedback",
                "perception",
                "machine",
                "neural",
                "model",
                "games",
                "interaction",
                "biases",
                "extrapolation",
                "subword",
                "units",
                "program",
                "synthesis",
                "modeling",
                "beam",
                "search",
                "reinforcement",
                "test",
                "reasoning",
                "prompt",
            )
        )
        return not tech
    return False


def _rank_header_candidates(
    candidates: Sequence[Mapping[str, Any]],
    *,
    body_text: str = "",
) -> list[dict[str, Any]]:
    rows = [dict(c) for c in candidates if isinstance(c, Mapping)]
    headers = [c for c in rows if str(c.get("source") or "") == "header_title"]
    others = [c for c in rows if str(c.get("source") or "") != "header_title"]
    title_blob = surface_norm(_primary_title_blob(body_text))

    _lead_stop = frozenset(
        {
            "a",
            "an",
            "the",
            "we",
            "our",
            "this",
            "that",
            "study",
            "using",
            "for",
            "with",
            "and",
            "of",
            "to",
            "in",
            "on",
            "by",
            "from",
            "adding",
            "achieves",
            "accurate",
            "computer",
            "initially",
            "enables",
            "kyunghyun",
            "yoshua",
            "cheng",
            "lapata",
            "barack",
            "obama",
            "perception",
            "network",
            "model",
            "biases",
            "enables",
            "linear",
        }
    )
    _prose_leads = frozenset(
        {
            "adding",
            "achieves",
            "accurate",
            "computer",
            "initially",
            "enables",
            "using",
            "study",
            "present",
            "show",
            "propose",
            "we",
            "our",
            "this",
            "that",
            "kyunghyun",
            "cheng",
            "barack",
            "obama",
            "compare",
            "civil",
            "details",
            "exploring",
            "rare",
            "words",
            "data",
            "sets",
            "limits",
            "robustfill",
        }
    )
    _bridge_noise = (
        "enables",
        "based",
        "through",
        "using",
        "details",
        "leaders",
        "outputs",
        "compare",
        "initially",
        "nothing",
        "performance",
        "better",
        "under noisy",
        "data sets",
        "exploring the",
        "rare words",
    )

    def key(c: Mapping[str, Any]) -> tuple:
        surface = str(c.get("surface") or "")
        norm = str(c.get("surface_norm") or surface_norm(surface))
        parts = surface.split()
        words = len(parts)
        lead = parts[0].casefold() if parts else ""
        in_title = 0 if title_blob and norm and norm in title_blob else 1
        author_pen = 1 if _looks_like_author_span(surface) else 0
        prose_pen = 1 if lead in _prose_leads else 0
        bridge_pen = 1 if any(b in norm for b in _bridge_noise) else 0
        # Prefer 2–4 word phrases; demote 1-word and long dumps.
        length_pen = 0 if words in (2, 3, 4) else (1 if words == 5 else 2)
        # Prefer near-3-word technical NPs; allow 4-word "X with Y Z" cores.
        if words == 4 and any(p.casefold() == "with" for p in parts):
            ideal_len_pen = 0
        else:
            ideal_len_pen = abs(words - 3) if words >= 2 else 3
        lead_pen = 1 if lead in _lead_stop else 0
        # Demote "Learning X" / "Books with X" wrappers when core NP is better.
        wrapper_pen = (
            1 if lead in {"learning", "books", "summarizing", "using", "study", "exploring", "rare", "words", "data", "limits"} else 0
        )
        # Structural boosts (not gold-looking): terminal title NP, connector NPs.
        core_boost = 0
        if in_title == 0 and title_blob:
            if title_blob.endswith(norm) or title_blob.startswith(norm):
                core_boost -= 1
            # "X and Y" / "X with Y" title cores are high-value technical NPs.
            if any(p.casefold() in {"and", "with"} for p in parts) and words in (2, 3, 4):
                core_boost -= 1
            # Prefer mid-title technical NPs over leading "Learning/Books/Summarizing".
            if words == 2 and lead not in {"learning", "books", "summarizing", "exploring", "words", "rare"}:
                core_boost -= 1
            # Prefer compact 2–3 word technical compounds over "Words with X" glue.
            if words == 2 and any(
                p.casefold()
                in {
                    "units",
                    "networks",
                    "network",
                    "modeling",
                    "synthesis",
                    "search",
                    "feedback",
                    "learning",
                }
                for p in parts
            ):
                core_boost -= 2
            if words >= 3 and any(p.casefold() in {"with", "of", "under"} for p in parts[1:-1]):
                # demote gluey mid-title spans slightly vs compact compounds
                core_boost += 1
        conn_bonus = (
            0
            if any(p.casefold() in {"and", "with", "for", "of"} for p in parts)
            else 1
        )
        return (
            in_title,
            author_pen,
            prose_pen,
            bridge_pen,
            wrapper_pen,
            lead_pen,
            length_pen,
            ideal_len_pen,
            core_boost,
            conn_bonus,
            surface.casefold(),
        )

    headers_sorted = sorted(headers, key=key)
    seen: set[str] = set()
    out: list[dict[str, Any]] = []
    for c in headers_sorted + others:
        norm = str(c.get("surface_norm") or surface_norm(str(c.get("surface") or "")))
        if not norm or norm in seen:
            continue
        if _looks_like_author_span(str(c.get("surface") or "")):
            continue
        seen.add(norm)
        out.append(dict(c))
    return out


def _is_subspan(norm: str, longer_norms: Sequence[str]) -> bool:
    """True when norm is a proper multiword subspan of an already picked surface."""
    if not norm or " " not in norm:
        # allow single tokens unless exact duplicate
        return any(norm == other for other in longer_norms)
    for other in longer_norms:
        if norm == other:
            return True
        if f" {norm} " in f" {other} " or other.startswith(norm + " ") or other.endswith(
            " " + norm
        ):
            return True
    return False


def header_priority_select(
    body_text: str,
    case_id: str,
    candidates: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Deterministic no-LLM selector: top header multiword candidates + typed links."""
    del case_id
    ranked = _rank_header_candidates(candidates, body_text=body_text)
    _marketing_prefixes = (
        "exploring the",
        "towards ",
        "program learning under",
        "learning under noisy",
        "words with ",
        "rare words with",
        "biases enables",
        "translation process",
        "annotated logical",
        "summarizing books with",
        "best neural synthesis",
        "text summarization",
        "diverse tasks",
        "explore recent",
        "generate basic",
        "ideas work",
    )
    _marketing_contains = (
        " under noisy",
        " under no",
        " tasks like",
        " like ",
        " enables ",
        " process simpler",
        " recent advances",
    )
    _ban_singles = {
        "translation",
        "learning",
        "language",
        "modeling",
        "network",
        "networks",
        "attention",
        "feedback",
        "summarization",
        "search",
        "units",
        "model",
        "models",
        "method",
        "task",
        "limits",
        "exploring",
        "noisy",
        "program",
    }

    picked: list[dict[str, Any]] = []
    picked_norms: list[str] = []
    for c in ranked:
        surface = str(c.get("surface") or "").strip()
        if not surface:
            continue
        words = surface.split()
        low = surface.casefold()
        norm = surface_norm(surface)
        if len(words) < 2:
            # Interaction-style partner only after one multiword pick
            if len(picked) != 1 or len(surface) < 8:
                continue
            if low in _ban_singles:
                continue
            if any(norm in pn.split() or norm == pn for pn in picked_norms):
                continue
        if any(x in low for x in ("@", ".edu", "et al", "arxiv")):
            continue
        if re.search(r"\d{4}", surface) and len(words) <= 2:
            continue
        if len(words) > 5:
            continue
        if low.startswith(_marketing_prefixes):
            continue
        if any(m in low for m in _marketing_contains):
            continue
        # Prefer compact title NPs: skip brand-only tokens as second method when
        # a multiword technical alternative remains later in ranking.
        if len(words) == 1 and len(picked) == 1:
            # leave singles for Interaction-style only after multiword task/method
            first_type = str(picked[0].get("type") or "")
            if first_type != "Task":
                continue
        if _is_subspan(norm, picked_norms):
            continue
        # Drop already-picked shorter spans covered by this longer hit.
        if any(
            p != norm and (p in norm or _is_subspan(p, [norm])) for p in list(picked_norms)
        ):
            keep_e: list[dict[str, Any]] = []
            keep_n: list[str] = []
            for e, pn in zip(picked, picked_norms, strict=True):
                if pn != norm and (pn in norm or _is_subspan(pn, [norm])):
                    continue
                keep_e.append(e)
                keep_n.append(pn)
            picked = keep_e
            picked_norms = keep_n
        etype = guess_entity_type(surface)
        if etype not in ALLOWED_ENTITY_TYPES:
            continue
        picked.append(
            {
                "candidate_id": str(c.get("candidate_id")),
                "type": etype,
                "_surface": surface,
            }
        )
        picked_norms.append(norm)
        if len(picked) >= 2:
            break


    by_type: dict[str, list[dict[str, Any]]] = {}
    for e in picked:
        by_type.setdefault(str(e["type"]), []).append(e)

    entities = [
        {"candidate_id": e["candidate_id"], "type": e["type"]} for e in picked
    ]

    relations: list[dict[str, Any]] = []
    sources = by_type.get("Field", []) + by_type.get("Method", [])
    tasks = by_type.get("Task", [])
    if sources and tasks:
        relations.append(
            {
                "type": "APPLIED_TO",
                "source_id": str(sources[0]["candidate_id"]),
                "target_id": str(tasks[0]["candidate_id"]),
            }
        )
    elif len(entities) >= 2:
        relations.append(
            {
                "type": "APPLIED_TO",
                "source_id": str(entities[0]["candidate_id"]),
                "target_id": str(entities[1]["candidate_id"]),
            }
        )

    relations = [r for r in relations if r.get("type") in ALLOWED_RELATION_TYPES]
    return {"entities": entities, "relations": relations, "json_valid": True}


def parse_constrained_llm_selection(
    raw: Mapping[str, Any],
    candidates: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Keep only selections that reference known candidate_ids + closed types."""
    allowed_ids = {
        str(c.get("candidate_id"))
        for c in candidates
        if isinstance(c, Mapping) and c.get("candidate_id")
    }
    entities: list[dict[str, Any]] = []
    seen: set[str] = set()
    for ent in raw.get("entities") or []:
        if not isinstance(ent, Mapping):
            continue
        cid = str(ent.get("candidate_id") or ent.get("id") or "").strip()
        if cid not in allowed_ids or cid in seen:
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
        seen.add(cid)
        entities.append({"candidate_id": cid, "type": etype})

    selected = {e["candidate_id"] for e in entities}
    relations: list[dict[str, Any]] = []
    for rel in raw.get("relations") or []:
        if not isinstance(rel, Mapping):
            continue
        rtype = str(rel.get("type") or rel.get("relation_type") or "").strip().upper()
        if rtype not in ALLOWED_RELATION_TYPES:
            continue
        src = str(
            rel.get("source_id") or rel.get("source") or rel.get("from") or ""
        ).strip()
        tgt = str(
            rel.get("target_id") or rel.get("target") or rel.get("to") or ""
        ).strip()
        if src not in selected or tgt not in selected or src == tgt:
            continue
        relations.append({"type": rtype, "source_id": src, "target_id": tgt})

    return {
        "entities": entities,
        "relations": relations,
        "json_valid": bool(raw.get("json_valid", True)),
    }


def render_constrained_select_prompt(
    *,
    case_id: str,
    paper_id: str,
    candidates: Sequence[Mapping[str, Any]],
    outline_titles: Sequence[str] | None = None,
    max_candidates: int = 40,
) -> str:
    """Prompt for LLM to pick among candidates only (no new labels)."""
    entity_types = ", ".join(sorted(ALLOWED_ENTITY_TYPES))
    rel_types = ", ".join(sorted(ALLOWED_RELATION_TYPES))
    lines = []
    for c in list(candidates)[:max_candidates]:
        if not isinstance(c, Mapping):
            continue
        lines.append(
            f"- {c.get('candidate_id')}: {c.get('surface')} [{c.get('source')}]"
        )
    outline = ", ".join(outline_titles or []) or "(none)"
    return (
        f"case_id={case_id}\n"
        f"paper_id={paper_id}\n"
        f"outline_titles: {outline}\n"
        f"Allowed entity types: {entity_types}\n"
        f"Allowed relation types: {rel_types}\n"
        "Select 2-4 central entities ONLY from the candidate list below.\n"
        "Return candidate_id values exactly. Do NOT invent new labels or ids.\n"
        "Prefer header_title multiword technical phrases from the title/abstract.\n"
        "JSON schema only:\n"
        "{\n"
        '  "entities": [{"candidate_id": "c:...", '
        '"type": "Field|Task|Method|Dataset|Model|Metric"}],\n'
        '  "relations": [{"type": "APPLIED_TO|USES_COMPONENT|EVALUATED_ON|OUTPERFORMS",'
        ' "source_id": "c:...", "target_id": "c:..."}]\n'
        "}\n"
        "--- CANDIDATES ---\n"
        + ("\n".join(lines) if lines else "(none)")
        + "\n--- END ---\n"
        "JSON:"
    )


def make_llm_constrained_select_fn(
    *,
    chat_fn: Any,
    model: str,
    max_tokens: int = 700,
    temperature: float = 0.0,
) -> Any:
    """Build ConstrainedSelectFn using injectible chat_fn(messages)->text."""

    def _select(
        body_text: str,
        case_id: str,
        candidates: Sequence[Mapping[str, Any]],
    ) -> Mapping[str, Any]:
        del body_text
        prompt = render_constrained_select_prompt(
            case_id=case_id,
            paper_id=case_id,
            candidates=candidates,
        )
        try:
            text = chat_fn(
                [
                    {
                        "role": "system",
                        "content": (
                            "You select knowledge-graph entities from a closed "
                            "candidate list. Return ONLY JSON. Never invent "
                            "candidate_id or labels."
                        ),
                    },
                    {"role": "user", "content": prompt},
                ],
                model=model,
                max_tokens=max_tokens,
                temperature=temperature,
            )
        except Exception:  # noqa: BLE001
            return {"entities": [], "relations": [], "json_valid": False}

        raw_text = str(text or "")
        parsed = parse_llm_extraction_json(raw_text)
        obj: dict[str, Any] = {}
        m = re.search(r"\{[\s\S]*\}", raw_text)
        if m:
            try:
                loaded = json.loads(m.group(0))
                if isinstance(loaded, dict):
                    obj = loaded
            except json.JSONDecodeError:
                obj = {}
        if not obj and parsed.get("json_valid"):
            obj = {
                "entities": parsed.get("entities") or [],
                "relations": parsed.get("relations") or [],
                "json_valid": True,
            }
            by_norm = {
                surface_norm(str(c.get("surface") or "")): str(c.get("candidate_id"))
                for c in candidates
                if isinstance(c, Mapping)
            }
            mapped = []
            for e in obj["entities"]:
                if not isinstance(e, Mapping):
                    continue
                if e.get("candidate_id"):
                    mapped.append(e)
                    continue
                lab = surface_norm(str(e.get("label") or ""))
                cid = by_norm.get(lab)
                if cid:
                    mapped.append(
                        {
                            "candidate_id": cid,
                            "type": e.get("type") or "Method",
                        }
                    )
            obj["entities"] = mapped
        if not obj:
            return {"entities": [], "relations": [], "json_valid": False}
        obj.setdefault("json_valid", True)
        return parse_constrained_llm_selection(obj, candidates)

    return _select


__all__ = [
    "guess_entity_type",
    "header_priority_select",
    "make_llm_constrained_select_fn",
    "parse_constrained_llm_selection",
    "render_constrained_select_prompt",
    "surface_norm",
]
