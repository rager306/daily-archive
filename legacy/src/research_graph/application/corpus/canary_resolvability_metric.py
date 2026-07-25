"""Canary resolvability rate metric (M280 evidence-ready tracking).

Measures share of gold-like entity/relation rows that resolve to SourceSpan
with page or bbox (or justified char-only fallback). Never import.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.evidence_resolvability import (
    evaluate_assertion_resolvability,
    resolvability_rate,
)

SCHEMA_VERSION = "canary-resolvability-metric.v2"
DEFAULT_TARGET_RATE = 0.95
DEFAULT_MIN_N = 10
REAL_MODES = frozenset(
    {
        "real_gold_hybrid_join",
        "char_span_from_hybrid_body",
        "layout_page_bbox",
        "prediction_resolvability",
        "real",
    }
)
DEMO_MODES = frozenset({"demo", "demo_placeholder", "placeholder"})


@dataclass(frozen=True, slots=True)
class CanaryResolvabilityPackage:
    schema_version: str
    total_rows: int
    resolvable_count: int
    char_only_count: int
    resolvability_rate: float
    target_rate: float
    target_met: bool
    sample_diagnostics: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    # M282 hygiene
    metric_mode: str = "unspecified"
    demo_metric: bool = False
    min_n: int = DEFAULT_MIN_N
    page_or_bbox_count: int = 0
    relation_rows: int = 0
    relation_resolvable: int = 0
    relation_grounded_ratio: float = 0.0
    alerts: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("canary resolvability metric cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "metric_mode": self.metric_mode,
            "demo_metric": self.demo_metric,
            "min_n": self.min_n,
            "total_rows": self.total_rows,
            "resolvable_count": self.resolvable_count,
            "char_only_count": self.char_only_count,
            "page_or_bbox_count": self.page_or_bbox_count,
            "resolvability_rate": self.resolvability_rate,
            "target_rate": self.target_rate,
            "target_met": self.target_met,
            "relation_rows": self.relation_rows,
            "relation_resolvable": self.relation_resolvable,
            "relation_grounded_ratio": self.relation_grounded_ratio,
            "alerts": list(self.alerts),
            "sample_diagnostics": list(self.sample_diagnostics),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Evidence-ready metric: share of rows with resolvable SourceSpan "
                "(page/bbox or justified char-only). target_met requires real mode, "
                f"n>=min_n, and rate>=target. Demo metrics never target_met. Never import."
            ),
        }


def _spans_from_row(row: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Extract span-like mappings from gold/assertion-like row."""
    for key in ("spans", "grounded_in", "source_spans"):
        raw = row.get(key)
        if isinstance(raw, list) and raw:
            return [dict(s) for s in raw if isinstance(s, Mapping)]
    for key in ("entities", "relations"):
        items = row.get(key)
        if not isinstance(items, list):
            continue
        out: list[dict[str, Any]] = []
        for item in items:
            if not isinstance(item, Mapping):
                continue
            for sk in ("spans", "grounded_in", "source_span"):
                sp = item.get(sk)
                if isinstance(sp, Mapping):
                    out.append(dict(sp))
                elif isinstance(sp, list):
                    out.extend(dict(s) for s in sp if isinstance(s, Mapping))
        if out:
            return out
    if any(k in row for k in ("page", "bbox", "artifact_hash", "char_start")):
        return [dict(row)]
    return []


def expand_gold_rows_to_assertions(
    cases: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Flatten gold cases into per-entity/relation assertion-like rows with spans."""
    rows: list[dict[str, Any]] = []
    for case in cases:
        if not isinstance(case, Mapping):
            continue
        case_id = str(case.get("case_id") or case.get("paper_id") or "")
        expanded = False
        for kind in ("entities", "relations"):
            items = case.get(kind)
            if not isinstance(items, list):
                continue
            for item in items:
                if not isinstance(item, Mapping):
                    continue
                # Item-local spans only — do not inherit sibling entity/relation spans.
                spans = _spans_from_row(item)
                if not spans:
                    # Optional case-level top-level spans only (not nested entities).
                    top = case.get("spans") or case.get("grounded_in")
                    if isinstance(top, list):
                        spans = [dict(s) for s in top if isinstance(s, Mapping)]
                kind_label = "entity" if kind == "entities" else "relation"
                rows.append(
                    {
                        "case_id": case_id,
                        "kind": kind_label,
                        "id": item.get("id") or item.get("label") or item.get("type"),
                        "spans": spans,
                    }
                )
                expanded = True
        if not expanded and (
            "spans" in case or "predicate" in case or "subject" in case
        ):
            rows.append(
                {
                    "case_id": case_id,
                    "kind": "assertion",
                    "id": case.get("assertion_id") or case_id,
                    "spans": _spans_from_row(case),
                }
            )
    return rows


def evaluate_canary_resolvability(
    rows: Sequence[Mapping[str, Any]],
    *,
    target_rate: float = DEFAULT_TARGET_RATE,
    allow_char_only_fallback: bool = True,
    expand_gold: bool = True,
    metric_mode: str = "unspecified",
    min_n: int = DEFAULT_MIN_N,
    demo_metric: bool | None = None,
) -> CanaryResolvabilityPackage:
    """Compute resolvability rate with hygiene gates for target_met.

    target_met requires:
    - not demo_metric
    - metric_mode in REAL_MODES (or unspecified treated as real only if not demo)
    - total_rows >= min_n
    - rate >= target_rate
    """
    mode = str(metric_mode or "unspecified")
    is_demo = bool(demo_metric) if demo_metric is not None else mode in DEMO_MODES

    work: list[dict[str, Any]]
    if expand_gold and rows and any(
        isinstance(r, Mapping) and ("entities" in r or "relations" in r) for r in rows
    ):
        work = expand_gold_rows_to_assertions(rows)
    else:
        work = []
        for r in rows:
            if not isinstance(r, Mapping):
                continue
            spans = r.get("spans")
            if not isinstance(spans, list):
                spans = _spans_from_row(r)
            work.append({**dict(r), "spans": spans})

    stats = resolvability_rate(
        work,
        spans_key="spans",
        allow_char_only_fallback=allow_char_only_fallback,
    )
    total = int(stats["total_assertions"])
    resolvable = int(stats["resolvable_count"])
    char_only = int(stats["char_only_count"])
    rate = float(stats["resolvability_rate"])
    target = float(target_rate)
    min_rows = max(1, int(min_n))

    page_bbox = 0
    rel_rows = 0
    rel_ok = 0
    sample: list[str] = []
    for row in work:
        spans = list(row.get("spans") or [])
        kind = str(row.get("kind") or "")
        for sp in spans:
            if isinstance(sp, Mapping) and (
                sp.get("page") is not None or sp.get("bbox") is not None
            ):
                page_bbox += 1
                break
        v = evaluate_assertion_resolvability(
            spans,  # type: ignore[arg-type]
            allow_char_only_fallback=allow_char_only_fallback,
        )
        if kind == "relation":
            rel_rows += 1
            if v.resolvable:
                rel_ok += 1
        if len(sample) < 12:
            sample.append(
                f"{row.get('case_id')}:{kind}:{v.reason}:ok={v.resolvable}"
            )

    rel_ratio = (rel_ok / rel_rows) if rel_rows else 0.0
    alerts: list[str] = []
    if is_demo:
        alerts.append("demo_metric_not_for_target")
    if total < min_rows:
        alerts.append(f"below_min_n:{total}<{min_rows}")
    if mode not in REAL_MODES and mode != "unspecified":
        alerts.append(f"non_real_mode:{mode}")
    if rel_rows > 0 and rel_ratio < 0.5:
        alerts.append(f"low_relation_grounded_ratio:{round(rel_ratio, 3)}")
    if char_only == resolvable and resolvable > 0 and page_bbox == 0:
        alerts.append("char_only_no_page_bbox")

    # Hygiene: demo never meets target; require min_n and real-ish mode
    mode_ok = mode in REAL_MODES or (mode == "unspecified" and not is_demo)
    target_met = (
        (not is_demo)
        and mode_ok
        and total >= min_rows
        and rate + 1e-12 >= target
    )

    diagnostics = (
        f"total_rows:{total}",
        f"resolvable_count:{resolvable}",
        f"char_only_count:{char_only}",
        f"page_or_bbox_count:{page_bbox}",
        f"resolvability_rate:{round(rate, 4)}",
        f"target_rate:{target}",
        f"target_met:{str(target_met).lower()}",
        f"metric_mode:{mode}",
        f"demo_metric:{str(is_demo).lower()}",
        f"min_n:{min_rows}",
        f"relation_grounded_ratio:{round(rel_ratio, 4)}",
        f"alerts:{len(alerts)}",
        "import_write_fail_closed",
        "canary_resolvability_metric_only",
    )
    return CanaryResolvabilityPackage(
        schema_version=SCHEMA_VERSION,
        total_rows=total,
        resolvable_count=resolvable,
        char_only_count=char_only,
        resolvability_rate=round(rate, 6),
        target_rate=target,
        target_met=target_met,
        sample_diagnostics=tuple(sample),
        diagnostics=diagnostics,
        metric_mode=mode,
        demo_metric=is_demo,
        min_n=min_rows,
        page_or_bbox_count=page_bbox,
        relation_rows=rel_rows,
        relation_resolvable=rel_ok,
        relation_grounded_ratio=round(rel_ratio, 6),
        alerts=tuple(alerts),
    )


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_TARGET_RATE",
    "CanaryResolvabilityPackage",
    "expand_gold_rows_to_assertions",
    "evaluate_canary_resolvability",
]
