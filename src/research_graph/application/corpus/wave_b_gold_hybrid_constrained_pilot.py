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

_TOKEN_RE = re.compile(r"[A-Za-z\u0400-\u04FF][A-Za-z\u0400-\u04FF'\-]{2,}")
_CONNECTORS = frozenset(
    {"and", "of", "for", "with", "via", "the", "a", "an", "to", "in", "on", "vs"}
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
    return (noise_hits, punct, 1 if has_digit else 0, abs(words - 3), len(s))


def build_body_candidates(
    body_text: str,
    *,
    paper_id: str = "",
    top_k_keywords: int = 16,
    max_multiword: int = 32,
    max_total: int = 64,
) -> list[dict[str, Any]]:
    """Deterministic candidate inventory from hybrid body (no LLM, no gold)."""
    text = body_text or ""
    stats = build_hybrid_statistical_extraction(
        paper_id=paper_id or "unknown",
        body_text=text,
        body_path=None,
        top_k=top_k_keywords,
    )
    candidates: list[dict[str, Any]] = []
    seen: set[str] = set()

    def _add(surface: str, source: str) -> None:
        norm = _normalize_surface(surface)
        if not norm or norm in seen:
            return
        if not surface_in_body(surface, text):
            return
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

    # Prefer head-of-document phrases (titles/abstract); bibliography tails are noisy.
    head = text[:3500]
    tail = text[3500:]

    # Case-insensitive content n-grams (2–4 tokens) from document head first.
    # Recovers gold-style phrases in sentence/Title/ALLCAPS form.
    head_tokens = list(_TOKEN_RE.finditer(head))
    ngram_counts: dict[str, int] = {}
    for width in (4, 3, 2):
        for i in range(0, max(0, len(head_tokens) - width + 1)):
            parts = [head_tokens[i + k].group(0) for k in range(width)]
            if parts[0].casefold() in _CONNECTORS or parts[-1].casefold() in _CONNECTORS:
                continue
            conn = sum(1 for p in parts if p.casefold() in _CONNECTORS)
            if conn > 1:
                continue
            content = [p for p in parts if p.casefold() not in _CONNECTORS]
            if len(content) < 2:
                continue
            # Join tokens only (skip markdown/punct between matches).
            surface = " ".join(parts)
            if len(surface) < 6 or len(surface) > 80:
                continue
            # Must still be grounded as contiguous casefold substring in body.
            if not surface_in_body(surface, head):
                continue
            ngram_counts[surface] = ngram_counts.get(surface, 0) + 1
    ranked_ngrams = sorted(
        ngram_counts.items(),
        key=lambda kv: (_phrase_quality(kv[0]), -kv[1], kv[0].casefold()),
    )
    for surface, _count in ranked_ngrams[: max_multiword * 3]:
        _add(surface, "head_ngram")
        if len(candidates) >= max_total:
            return candidates

    head_phrases = sorted(
        _extract_titleish_phrases(head, max_phrases=max_multiword * 2),
        key=_phrase_quality,
    )
    for surface in head_phrases:
        _add(surface, "multiword_titleish_head")
        if len(candidates) >= max_total:
            return candidates
    for surface in _extract_titleish_phrases(tail, max_phrases=max(8, max_multiword // 3)):
        _add(surface, "multiword_titleish_tail")
        if len(candidates) >= max_total:
            return candidates

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
