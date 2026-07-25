"""Canary prediction resolvability (M284 S02).

Validate evidence-ready at scale WITHOUT gold labels: run LLM extraction on
canary held-out bodies, ground predicted entity/relation surfaces to char
spans in the hybrid body, then upgrade with layout page/bbox. Measure what
fraction of predicted surfaces resolve to a real page/bbox span.

This is *prediction resolvability*, not gold F1. It answers: "do the facts an
LLM extracts actually trace back to a located span in the source?" It does not
measure correctness (no gold), only evidence traceability.

GT isolation: canary held-out only; never train on these. Never import.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

from research_graph.application.corpus.canary_resolvability_metric import (
    CanaryResolvabilityPackage,
    evaluate_canary_resolvability,
)
from research_graph.application.corpus.gold_char_span_grounding import (
    attach_char_spans_to_gold_case,
)
from research_graph.application.corpus.layout_span_upgrade import (
    upgrade_grounded_gold_with_layout,
)

SCHEMA_VERSION = "canary-prediction-resolvability.v1"


@dataclass(frozen=True, slots=True)
class PredictionResolvabilityPackage:
    schema_version: str
    metric_mode: str
    demo_metric: bool
    resolvability_rate: float | None
    target_rate: float
    target_met: bool
    page_or_bbox_count: int
    char_only_count: int
    total_rows: int
    relation_grounded_ratio: float | None
    alerts: tuple[str, ...]
    diagnostics: tuple[str, ...]
    per_paper: tuple[dict[str, Any], ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    llm_used: bool = True
    gt_isolation: str = "canary_held_out_only"

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("prediction resolvability cannot authorize import")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "metric_mode": self.metric_mode,
            "demo_metric": self.demo_metric,
            "resolvability_rate": self.resolvability_rate,
            "target_rate": self.target_rate,
            "target_met": self.target_met,
            "page_or_bbox_count": self.page_or_bbox_count,
            "char_only_count": self.char_only_count,
            "total_rows": self.total_rows,
            "relation_grounded_ratio": self.relation_grounded_ratio,
            "alerts": list(self.alerts),
            "diagnostics": list(self.diagnostics),
            "per_paper": list(self.per_paper),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "llm_used": self.llm_used,
            "gt_isolation": self.gt_isolation,
            "note": (
                "Prediction resolvability: LLM-extracted surfaces resolve to "
                "page/bbox spans. Not gold F1 (no correctness). Held-out only; "
                "never train; never import."
            ),
        }


def ground_prediction_to_spans(
    *,
    prediction: Mapping[str, Any],
    body_text: str,
    case_id: str,
    paper_id: str,
    layout_json: Mapping[str, Any] | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Ground one LLM prediction's surfaces to char spans, then layout page/bbox.

    Returns (grounded_prediction, stats).
    """
    # Reuse the gold grounding path: it extracts surfaces from entity.label /
    # relation text and locates char spans in the body. A prediction has the
    # same shape (entities with label, relations with source/target/type).
    pred = dict(prediction)
    pred.setdefault("case_id", case_id)
    pred.setdefault("paper_id", paper_id)
    pred.setdefault("entities", [])
    pred.setdefault("relations", [])
    pred.setdefault("schema_valid", True)
    pred.setdefault("json_valid", True)

    grounded = attach_char_spans_to_gold_case(
        gold=pred,
        body_text=body_text,
        case_id=case_id,
        paper_id=paper_id,
    )
    gold_u, stats = upgrade_grounded_gold_with_layout(grounded.gold, layout_json)
    return gold_u, stats


def evaluate_prediction_resolvability(
    *,
    cases: Sequence[Mapping[str, Any]],
    predictions: Sequence[Mapping[str, Any]],
    target_rate: float = 0.95,
    min_n: int = 5,
    metric_mode: str = "prediction_resolvability",
) -> PredictionResolvabilityPackage:
    """Compute prediction resolvability across cases.

    Each case: {case_id, paper_id, body_text, layout_json (optional)}.
    Each prediction: {case_id, entities:[{label,...}], relations:[...]}.
    Predictions are matched to cases by case_id.
    """
    pred_by_case: dict[str, Mapping[str, Any]] = {}
    for p in predictions:
        cid = str(p.get("case_id") or "")
        if cid:
            pred_by_case[cid] = p

    grounded_predictions: list[dict[str, Any]] = []
    per_paper: list[dict[str, Any]] = []
    spans_total = 0
    spans_upgraded = 0
    layout_hits = 0
    for case in cases:
        case_id = str(case.get("case_id") or "")
        paper_id = str(case.get("paper_id") or case_id)
        body_text = str(case.get("body_text") or "")
        layout = case.get("layout_json")
        if layout is not None:
            layout_hits += 1
        pred = pred_by_case.get(case_id)
        if pred is None:
            continue
        gold_u, stats = ground_prediction_to_spans(
            prediction=pred,
            body_text=body_text,
            case_id=case_id,
            paper_id=paper_id,
            layout_json=layout if isinstance(layout, Mapping) else None,
        )
        grounded_predictions.append(gold_u)
        spans_total += int(stats.get("spans_total") or 0)
        spans_upgraded += int(stats.get("spans_upgraded") or 0)
        ent_n = len(gold_u.get("entities") or [])
        rel_n = len(gold_u.get("relations") or [])
        per_paper.append(
            {
                "case_id": case_id,
                "paper_id": paper_id,
                "entity_count": ent_n,
                "relation_count": rel_n,
                "spans_total": int(stats.get("spans_total") or 0),
                "spans_upgraded": int(stats.get("spans_upgraded") or 0),
                "layout_present": layout is not None,
            }
        )

    metric: CanaryResolvabilityPackage = evaluate_canary_resolvability(
        grounded_predictions,
        target_rate=target_rate,
        expand_gold=True,
        metric_mode=metric_mode,
        demo_metric=False,
        min_n=min_n,
    )

    alerts: list[str] = list(metric.alerts)
    if layout_hits == 0:
        alerts.append("no_layout_json:prediction_resolvability_char_only")
    if metric.total_rows < min_n:
        alerts.append(f"below_min_n:{metric.total_rows}<{min_n}")

    diagnostics = (
        f"cases:{len(cases)}",
        f"predictions:{len(grounded_predictions)}",
        f"layout_hits:{layout_hits}",
        f"spans_total:{spans_total}",
        f"spans_upgraded:{spans_upgraded}",
        f"resolvability_rate:{metric.resolvability_rate}",
        f"page_or_bbox_count:{metric.page_or_bbox_count}",
        f"char_only_count:{metric.char_only_count}",
        f"relation_grounded_ratio:{metric.relation_grounded_ratio}",
        f"target_met:{str(metric.target_met).lower()}",
        "gt_isolation:canary_held_out_only",
        "import_write_fail_closed",
    )
    return PredictionResolvabilityPackage(
        schema_version=SCHEMA_VERSION,
        metric_mode=metric_mode,
        demo_metric=False,
        resolvability_rate=metric.resolvability_rate,
        target_rate=target_rate,
        target_met=metric.target_met,
        page_or_bbox_count=metric.page_or_bbox_count,
        char_only_count=metric.char_only_count,
        total_rows=metric.total_rows,
        relation_grounded_ratio=metric.relation_grounded_ratio,
        alerts=tuple(alerts),
        diagnostics=tuple(diagnostics),
        per_paper=tuple(per_paper),
    )


__all__ = [
    "PredictionResolvabilityPackage",
    "ground_prediction_to_spans",
    "evaluate_prediction_resolvability",
]
