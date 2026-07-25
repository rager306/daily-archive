"""Wave E5 optional candidates (M284 S03).

Roadmap E5 after evidence-ready (E1–E3):
  * offline candidate generator (header-priority always; GLiNER if installed)
  * Docling/CPU page fallback **only** for canary-failed pages
  * blind second judge for high-impact disagreements

Never free-invent gold. Never train on canary held-out. Never import.
GLiNER is optional — when not installed the generator reports unavailable
without inventing entities.
"""

from __future__ import annotations

import importlib.util
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any, Protocol

from research_graph.application.corpus.wave_b_constrained_select import (
    header_priority_select,
)
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    build_body_candidates,
)

SCHEMA_VERSION = "e5-optional-candidates.v1"
HIGH_IMPACT_RELATION_TYPES = frozenset(
    {"APPLIED_TO", "OUTPERFORMS", "EVALUATED_ON", "USES_COMPONENT"}
)


def gliner_available() -> bool:
    return importlib.util.find_spec("gliner") is not None


def docling_available() -> bool:
    return importlib.util.find_spec("docling") is not None


class OfflineCandidateGenerator(Protocol):
    """Pluggable offline entity/relation candidate source."""

    @property
    def name(self) -> str: ...

    def generate(
        self, *, body_text: str, paper_id: str = "", case_id: str = ""
    ) -> Mapping[str, Any]:
        """Return {entities, relations, available, diagnostics}."""
        ...


@dataclass(frozen=True, slots=True)
class HeaderPriorityCandidateGenerator:
    """Always-available offline generator: body candidates + header_priority_select."""

    name: str = "header_priority"

    def generate(
        self, *, body_text: str, paper_id: str = "", case_id: str = ""
    ) -> Mapping[str, Any]:
        text = body_text or ""
        candidates = build_body_candidates(text, paper_id=paper_id or "unknown")
        by_id = {
            str(c.get("candidate_id")): c
            for c in candidates
            if isinstance(c, Mapping) and c.get("candidate_id")
        }
        sel = header_priority_select(text, case_id or paper_id or "case:x", candidates)
        entities_raw = list(sel.get("entities") or []) if isinstance(sel, Mapping) else []
        relations_raw = list(sel.get("relations") or []) if isinstance(sel, Mapping) else []
        entities: list[dict[str, Any]] = []
        for e in entities_raw:
            if not isinstance(e, Mapping):
                continue
            cid = str(e.get("candidate_id") or "")
            surf = str((by_id.get(cid) or {}).get("surface") or "").strip()
            entities.append(
                {
                    "id": cid or f"e:{len(entities)}",
                    "label": surf,
                    "type": str(e.get("type") or "Method"),
                    "candidate_id": cid,
                }
            )
        id_to_label = {str(e["id"]): str(e["label"]) for e in entities if e.get("label")}
        relations: list[dict[str, Any]] = []
        for r in relations_raw:
            if not isinstance(r, Mapping):
                continue
            sid = str(r.get("source_id") or "")
            tid = str(r.get("target_id") or "")
            relations.append(
                {
                    "type": str(r.get("type") or ""),
                    "source": sid,
                    "target": tid,
                    "source_label": id_to_label.get(sid, ""),
                    "target_label": id_to_label.get(tid, ""),
                }
            )
        return {
            "generator": self.name,
            "available": True,
            "entities": entities,
            "relations": relations,
            "entity_count": len(entities),
            "relation_count": len(relations),
            "candidate_pool_size": len(candidates),
            "paper_id": paper_id,
            "case_id": case_id,
            "diagnostics": ("header_priority_offline", "import_write_fail_closed"),
        }


@dataclass(frozen=True, slots=True)
class OptionalGlinerCandidateGenerator:
    """GLiNER-Relex offline generator — unavailable when gliner not installed."""

    name: str = "gliner_relex"
    model_id: str = "urchade/gliner_medium-v2.1"

    def generate(
        self, *, body_text: str, paper_id: str = "", case_id: str = ""
    ) -> Mapping[str, Any]:
        if not gliner_available():
            return {
                "generator": self.name,
                "available": False,
                "entities": [],
                "relations": [],
                "entity_count": 0,
                "relation_count": 0,
                "paper_id": paper_id,
                "case_id": case_id,
                "diagnostics": (
                    "gliner_not_installed",
                    "optional_e5_skip",
                    "import_write_fail_closed",
                ),
                "blocked_reason": "gliner_not_installed",
            }
        # Installed path: do not auto-download models in pure unit path.
        # Callers that want live GLiNER must inject a concrete extract_fn.
        return {
            "generator": self.name,
            "available": True,
            "entities": [],
            "relations": [],
            "entity_count": 0,
            "relation_count": 0,
            "paper_id": paper_id,
            "case_id": case_id,
            "diagnostics": (
                "gliner_installed_but_no_live_extract_in_pure_path",
                "inject_extract_fn_for_live",
                "import_write_fail_closed",
            ),
            "blocked_reason": "live_extract_not_invoked_in_pure_path",
        }


@dataclass(frozen=True, slots=True)
class DoclingPageFallbackResult:
    schema_version: str
    attempted: bool
    available: bool
    used: bool
    page_count: int | None
    text_chars: int
    reason: str
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "attempted": self.attempted,
            "available": self.available,
            "used": self.used,
            "page_count": self.page_count,
            "text_chars": self.text_chars,
            "reason": self.reason,
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Docling/CPU fallback only for canary-failed pages. "
                "Never primary path. Never import."
            ),
        }


def docling_page_fallback(
    *,
    pdf_path: str | None,
    hybrid_failed: bool,
    force: bool = False,
    convert_fn: Callable[[str], Mapping[str, Any]] | None = None,
) -> DoclingPageFallbackResult:
    """Docling fallback **only** when hybrid/canary path failed (or force for tests).

    ``convert_fn`` is injectible for TDD. Live Docling is not invoked unless
    convert_fn is provided or hybrid_failed and docling is available.
    """
    if not hybrid_failed and not force:
        return DoclingPageFallbackResult(
            schema_version="e5-docling-page-fallback.v1",
            attempted=False,
            available=docling_available(),
            used=False,
            page_count=None,
            text_chars=0,
            reason="hybrid_not_failed_skip",
            diagnostics=("docling_fallback_gated", "import_write_fail_closed"),
        )
    if not pdf_path:
        return DoclingPageFallbackResult(
            schema_version="e5-docling-page-fallback.v1",
            attempted=False,
            available=docling_available(),
            used=False,
            page_count=None,
            text_chars=0,
            reason="pdf_path_missing",
            diagnostics=("docling_fallback_no_pdf", "import_write_fail_closed"),
        )
    if not docling_available() and convert_fn is None:
        return DoclingPageFallbackResult(
            schema_version="e5-docling-page-fallback.v1",
            attempted=True,
            available=False,
            used=False,
            page_count=None,
            text_chars=0,
            reason="docling_not_installed",
            diagnostics=("docling_unavailable", "import_write_fail_closed"),
        )
    if convert_fn is None:
        # Live convert is opt-in via injectible — pure path documents readiness only.
        return DoclingPageFallbackResult(
            schema_version="e5-docling-page-fallback.v1",
            attempted=True,
            available=True,
            used=False,
            page_count=None,
            text_chars=0,
            reason="docling_ready_inject_convert_fn_for_live",
            diagnostics=(
                "docling_available",
                "live_convert_not_invoked_in_pure_path",
                "import_write_fail_closed",
            ),
        )
    try:
        out = dict(convert_fn(pdf_path))
        text = str(out.get("text") or "")
        pages = out.get("page_count")
        try:
            page_count = int(pages) if pages is not None else None
        except (TypeError, ValueError):
            page_count = None
        return DoclingPageFallbackResult(
            schema_version="e5-docling-page-fallback.v1",
            attempted=True,
            available=True,
            used=bool(text),
            page_count=page_count,
            text_chars=len(text),
            reason="docling_fallback_used" if text else "docling_empty_text",
            diagnostics=("docling_fallback_invoked", "import_write_fail_closed"),
        )
    except Exception as exc:  # noqa: BLE001 - fail-closed fallback
        return DoclingPageFallbackResult(
            schema_version="e5-docling-page-fallback.v1",
            attempted=True,
            available=True,
            used=False,
            page_count=None,
            text_chars=0,
            reason=f"docling_failed:{type(exc).__name__}",
            diagnostics=("docling_fallback_error", "import_write_fail_closed"),
        )


@dataclass(frozen=True, slots=True)
class BlindSecondJudgePackage:
    schema_version: str
    compared_cases: int
    high_impact_disagreements: int
    entity_disagreements: int
    relation_disagreements: int
    agreement_rate: float | None
    disagreements: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "compared_cases": self.compared_cases,
            "high_impact_disagreements": self.high_impact_disagreements,
            "entity_disagreements": self.entity_disagreements,
            "relation_disagreements": self.relation_disagreements,
            "agreement_rate": self.agreement_rate,
            "disagreements": list(self.disagreements),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Blind second judge: compares two prediction sets without gold. "
                "Flags high-impact relation disagreements for human review. "
                "Never resolves by inventing. Never import."
            ),
        }


def _entity_labels(pred: Mapping[str, Any]) -> set[str]:
    out: set[str] = set()
    for e in pred.get("entities") or []:
        if not isinstance(e, Mapping):
            continue
        for k in ("label", "text", "surface", "name"):
            v = e.get(k)
            if isinstance(v, str) and v.strip():
                out.add(v.strip().casefold())
                break
    return out


def _relation_keys(pred: Mapping[str, Any]) -> set[tuple[str, str, str]]:
    out: set[tuple[str, str, str]] = set()
    for r in pred.get("relations") or []:
        if not isinstance(r, Mapping):
            continue
        rtype = str(r.get("type") or r.get("relation_type") or "").strip().upper()
        src = str(
            r.get("source_label") or r.get("source") or r.get("head") or ""
        ).strip().casefold()
        tgt = str(
            r.get("target_label") or r.get("target") or r.get("tail") or ""
        ).strip().casefold()
        if rtype or src or tgt:
            out.add((src, rtype, tgt))
    return out


def blind_second_judge(
    *,
    primary: Sequence[Mapping[str, Any]],
    secondary: Sequence[Mapping[str, Any]],
    high_impact_types: frozenset[str] = HIGH_IMPACT_RELATION_TYPES,
) -> BlindSecondJudgePackage:
    """Compare two prediction sets; flag high-impact disagreements (no gold)."""
    by_case_a: dict[str, Mapping[str, Any]] = {
        str(p.get("case_id") or ""): p for p in primary if p.get("case_id")
    }
    by_case_b: dict[str, Mapping[str, Any]] = {
        str(p.get("case_id") or ""): p for p in secondary if p.get("case_id")
    }
    common = sorted(set(by_case_a) & set(by_case_b) - {""})
    disagreements: list[dict[str, Any]] = []
    ent_d = 0
    rel_d = 0
    high_d = 0
    agree = 0
    for cid in common:
        a = by_case_a[cid]
        b = by_case_b[cid]
        ea, eb = _entity_labels(a), _entity_labels(b)
        ra, rb = _relation_keys(a), _relation_keys(b)
        only_a_e = sorted(ea - eb)
        only_b_e = sorted(eb - ea)
        only_a_r = sorted(ra - rb)
        only_b_r = sorted(rb - ra)
        if not only_a_e and not only_b_e and not only_a_r and not only_b_r:
            agree += 1
            continue
        if only_a_e or only_b_e:
            ent_d += 1
        if only_a_r or only_b_r:
            rel_d += 1
        high_types = {
            t
            for (_s, t, _t) in (only_a_r + only_b_r)
            if t in high_impact_types
        }
        if high_types:
            high_d += 1
        disagreements.append(
            {
                "case_id": cid,
                "entity_only_primary": only_a_e[:12],
                "entity_only_secondary": only_b_e[:12],
                "relation_only_primary": [
                    {"source": s, "type": t, "target": tg} for s, t, tg in only_a_r[:8]
                ],
                "relation_only_secondary": [
                    {"source": s, "type": t, "target": tg} for s, t, tg in only_b_r[:8]
                ],
                "high_impact_types": sorted(high_types),
                "high_impact": bool(high_types),
            }
        )
    n = len(common)
    rate = (agree / n) if n else None
    return BlindSecondJudgePackage(
        schema_version="e5-blind-second-judge.v1",
        compared_cases=n,
        high_impact_disagreements=high_d,
        entity_disagreements=ent_d,
        relation_disagreements=rel_d,
        agreement_rate=rate,
        disagreements=tuple(disagreements[:50]),
        diagnostics=(
            f"compared_cases:{n}",
            f"agreement_rate:{rate}",
            f"high_impact_disagreements:{high_d}",
            "blind_second_judge",
            "import_write_fail_closed",
        ),
    )


@dataclass(frozen=True, slots=True)
class E5OptionalCandidatesPackage:
    schema_version: str
    generators: tuple[dict[str, Any], ...]
    docling_fallback: dict[str, Any]
    second_judge: dict[str, Any] | None
    coverage_delta: dict[str, Any]
    alerts: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("e5 package cannot authorize import")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "generators": list(self.generators),
            "docling_fallback": dict(self.docling_fallback),
            "second_judge": dict(self.second_judge) if self.second_judge else None,
            "coverage_delta": dict(self.coverage_delta),
            "alerts": list(self.alerts),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "E5 optional candidates: offline generators + Docling page fallback "
                "+ blind second judge. GLiNER optional. Never invent gold. Never import."
            ),
        }


def build_e5_optional_candidates_package(
    *,
    body_text: str = "",
    paper_id: str = "",
    case_id: str = "",
    hybrid_failed: bool = False,
    pdf_path: str | None = None,
    primary_predictions: Sequence[Mapping[str, Any]] | None = None,
    secondary_predictions: Sequence[Mapping[str, Any]] | None = None,
    generators: Sequence[OfflineCandidateGenerator] | None = None,
    docling_convert_fn: Callable[[str], Mapping[str, Any]] | None = None,
) -> E5OptionalCandidatesPackage:
    """Compose E5 optional candidate surfaces (fail-closed)."""
    gens: list[OfflineCandidateGenerator] = list(
        generators
        if generators is not None
        else (HeaderPriorityCandidateGenerator(), OptionalGlinerCandidateGenerator())
    )
    gen_out: list[dict[str, Any]] = []
    header_ent = 0
    for g in gens:
        out = dict(
            g.generate(body_text=body_text, paper_id=paper_id, case_id=case_id)
        )
        gen_out.append(out)
        if out.get("generator") == "header_priority":
            header_ent = int(out.get("entity_count") or 0)

    docling = docling_page_fallback(
        pdf_path=pdf_path,
        hybrid_failed=hybrid_failed,
        convert_fn=docling_convert_fn,
    )
    judge = None
    if primary_predictions is not None and secondary_predictions is not None:
        judge = blind_second_judge(
            primary=primary_predictions, secondary=secondary_predictions
        )

    gliner_avail = any(
        g.get("generator") == "gliner_relex" and g.get("available") for g in gen_out
    )
    alerts: list[str] = []
    if not gliner_available():
        alerts.append("gliner_not_installed:optional_skip")
    if hybrid_failed and not docling.used and docling.reason != "docling_ready_inject_convert_fn_for_live":
        alerts.append(f"docling_fallback:{docling.reason}")
    if judge and judge.high_impact_disagreements:
        alerts.append(f"high_impact_disagreements:{judge.high_impact_disagreements}")

    coverage_delta = {
        "header_entity_count": header_ent,
        "gliner_available": gliner_available(),
        "gliner_used": gliner_avail,
        "docling_available": docling_available(),
        "docling_used": docling.used,
        "second_judge_compared": judge.compared_cases if judge else 0,
        "second_judge_high_impact": judge.high_impact_disagreements if judge else 0,
        "e5_coverage_delta": (
            "header_priority_baseline"
            if header_ent
            else "no_header_candidates"
        ),
    }
    diagnostics = (
        f"generators:{len(gen_out)}",
        f"header_entities:{header_ent}",
        f"gliner_available:{str(gliner_available()).lower()}",
        f"docling_available:{str(docling_available()).lower()}",
        f"docling_used:{str(docling.used).lower()}",
        f"second_judge:{judge is not None}",
        f"alerts:{len(alerts)}",
        "import_write_fail_closed",
        "e5_optional_only",
    )
    return E5OptionalCandidatesPackage(
        schema_version=SCHEMA_VERSION,
        generators=tuple(gen_out),
        docling_fallback=docling.to_dict(),
        second_judge=judge.to_dict() if judge else None,
        coverage_delta=coverage_delta,
        alerts=tuple(alerts),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "HIGH_IMPACT_RELATION_TYPES",
    "OfflineCandidateGenerator",
    "HeaderPriorityCandidateGenerator",
    "OptionalGlinerCandidateGenerator",
    "DoclingPageFallbackResult",
    "BlindSecondJudgePackage",
    "E5OptionalCandidatesPackage",
    "gliner_available",
    "docling_available",
    "docling_page_fallback",
    "blind_second_judge",
    "build_e5_optional_candidates_package",
]
