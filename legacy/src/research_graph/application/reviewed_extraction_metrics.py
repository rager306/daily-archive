"""Reviewed extraction metrics harness (M202).

Thin application wrapper over
:func:`~research_graph.application.extraction_benchmark.evaluate_records` for
immutable independently-reviewed fixtures (M072/M073 style). Adds disagreement
evidence for operators without duplicating scorers or loading raw article text.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

from research_graph.application.extraction_benchmark import evaluate_records

# Keys that must never appear in harness outputs (leakage control).
FORBIDDEN_REPORT_KEYS = frozenset(
    {
        "body",
        "prompt",
        "prompts",
        "raw_text",
        "raw_pdf_text",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "secret",
        "api_key",
        "model_payload",
        "completion",
    }
)


@dataclass(frozen=True)
class DisagreementEvidence:
    """Case-level disagreement between gold and prediction (metadata only)."""

    case_id: str
    kind: str  # missing_prediction | extra_prediction | invalid_schema | metric_delta
    detail: str


@dataclass(frozen=True)
class ReviewedCaseReport:
    """Score report for one reviewed gold/prediction pair."""

    case_id: str
    metrics: dict[str, Any]
    disagreements: tuple[DisagreementEvidence, ...] = ()
    leakage_clean: bool = True

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "case_id": self.case_id,
            "metrics": dict(self.metrics),
            "disagreements": [
                {"case_id": d.case_id, "kind": d.kind, "detail": d.detail}
                for d in self.disagreements
            ],
            "leakage_clean": self.leakage_clean,
        }


@dataclass(frozen=True)
class ReviewedSplitReport:
    """Aggregate metrics for a reviewed split (train/validation/held-out)."""

    split_name: str
    metrics: dict[str, Any]
    disagreements: tuple[DisagreementEvidence, ...] = ()
    case_count: int = 0
    leakage_clean: bool = True
    confidence_intervals: dict[str, tuple[float, float]] = field(default_factory=dict)

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "split_name": self.split_name,
            "case_count": self.case_count,
            "metrics": dict(self.metrics),
            "disagreements": [
                {"case_id": d.case_id, "kind": d.kind, "detail": d.detail}
                for d in self.disagreements
            ],
            "confidence_intervals": {
                k: [v[0], v[1]] for k, v in self.confidence_intervals.items()
            },
            "leakage_clean": self.leakage_clean,
        }


def _assert_no_leakage(payload: Mapping[str, Any], *, path: str = "$") -> list[str]:
    """Return diagnostic paths for forbidden keys (does not raise)."""
    hits: list[str] = []
    for key, value in payload.items():
        key_l = str(key).lower()
        if key_l in FORBIDDEN_REPORT_KEYS or any(f in key_l for f in FORBIDDEN_REPORT_KEYS):
            hits.append(f"{path}.{key}")
        if isinstance(value, dict):
            hits.extend(_assert_no_leakage(value, path=f"{path}.{key}"))
        elif isinstance(value, list):
            for i, item in enumerate(value):
                if isinstance(item, dict):
                    hits.extend(_assert_no_leakage(item, path=f"{path}.{key}[{i}]"))
    return hits


def _wilson_interval(successes: int, total: int, *, z: float = 1.96) -> tuple[float, float]:
    """Wilson score interval for a proportion (metadata-only CI)."""
    if total <= 0:
        return (0.0, 0.0)
    p = successes / total
    denom = 1.0 + z * z / total
    centre = p + z * z / (2 * total)
    margin = z * ((p * (1 - p) + z * z / (4 * total)) / total) ** 0.5
    low = max(0.0, (centre - margin) / denom)
    high = min(1.0, (centre + margin) / denom)
    return (low, high)


def score_reviewed_case(
    gold: Mapping[str, Any],
    prediction: Mapping[str, Any],
) -> ReviewedCaseReport:
    """Score one frozen reviewed case through existing evaluate_records."""
    gold_rec = dict(gold)
    pred_rec = dict(prediction)
    case_id = str(gold_rec.get("case_id") or pred_rec.get("case_id") or "unknown")
    metrics = evaluate_records([gold_rec], [pred_rec])
    disagreements: list[DisagreementEvidence] = []

    for missing in metrics.get("missing_predictions", []):
        disagreements.append(
            DisagreementEvidence(case_id=str(missing), kind="missing_prediction", detail="no prediction row")
        )
    for extra in metrics.get("extra_predictions", []):
        disagreements.append(
            DisagreementEvidence(case_id=str(extra), kind="extra_prediction", detail="prediction without gold")
        )
    for invalid in metrics.get("invalid_cases", []):
        disagreements.append(
            DisagreementEvidence(case_id=str(invalid), kind="invalid_schema", detail="prediction schema invalid")
        )

    # Metric-level disagreement when not perfect on shared case
    if case_id not in metrics.get("missing_predictions", []) and case_id not in metrics.get(
        "extra_predictions", []
    ):
        if metrics.get("entity_f1", 1.0) < 1.0:
            disagreements.append(
                DisagreementEvidence(
                    case_id=case_id,
                    kind="metric_delta",
                    detail=f"entity_f1={metrics.get('entity_f1')}",
                )
            )
        if metrics.get("relation_f1", 1.0) < 1.0:
            disagreements.append(
                DisagreementEvidence(
                    case_id=case_id,
                    kind="metric_delta",
                    detail=f"relation_f1={metrics.get('relation_f1')}",
                )
            )

    leakage = _assert_no_leakage(metrics) + _assert_no_leakage(
        {"disagreements": [d.detail for d in disagreements]}
    )
    return ReviewedCaseReport(
        case_id=case_id,
        metrics=metrics,
        disagreements=tuple(disagreements),
        leakage_clean=not leakage,
    )


def score_reviewed_split(
    gold_records: Sequence[Mapping[str, Any]],
    prediction_records: Sequence[Mapping[str, Any]],
    *,
    split_name: str = "reviewed",
) -> ReviewedSplitReport:
    """Score a full reviewed split; reuses evaluate_records; adds CIs + disagreements."""
    golds = [dict(r) for r in gold_records]
    preds = [dict(r) for r in prediction_records]
    metrics = evaluate_records(golds, preds)

    disagreements: list[DisagreementEvidence] = []
    for missing in metrics.get("missing_predictions", []):
        disagreements.append(
            DisagreementEvidence(case_id=str(missing), kind="missing_prediction", detail="no prediction row")
        )
    for extra in metrics.get("extra_predictions", []):
        disagreements.append(
            DisagreementEvidence(case_id=str(extra), kind="extra_prediction", detail="prediction without gold")
        )
    for invalid in metrics.get("invalid_cases", []):
        disagreements.append(
            DisagreementEvidence(case_id=str(invalid), kind="invalid_schema", detail="prediction schema invalid")
        )

    # Wilson CIs on entity precision/recall using TP/predicted/gold counts
    cis: dict[str, tuple[float, float]] = {}
    tp = int(metrics.get("entity_true_positive", 0))
    pred_n = int(metrics.get("entity_predicted", 0))
    gold_n = int(metrics.get("entity_gold", 0))
    cis["entity_precision"] = _wilson_interval(tp, pred_n)
    cis["entity_recall"] = _wilson_interval(tp, gold_n)
    rtp = int(metrics.get("relation_true_positive", 0))
    rpred = int(metrics.get("relation_predicted", 0))
    rgold = int(metrics.get("relation_gold", 0))
    cis["relation_precision"] = _wilson_interval(rtp, rpred)
    cis["relation_recall"] = _wilson_interval(rtp, rgold)

    leakage = _assert_no_leakage(metrics)
    return ReviewedSplitReport(
        split_name=split_name,
        metrics=metrics,
        disagreements=tuple(disagreements),
        case_count=int(metrics.get("case_count", len(golds))),
        leakage_clean=not leakage,
        confidence_intervals=cis,
    )


def score_entity_split(
    gold_records: Sequence[Mapping[str, Any]],
    prediction_records: Sequence[Mapping[str, Any]],
    *,
    split_name: str = "entity",
) -> ReviewedSplitReport:
    """Entity-focused view of a reviewed split (precision/recall/types/CIs)."""
    report = score_reviewed_split(gold_records, prediction_records, split_name=split_name)
    entity_metrics = {
        k: v
        for k, v in report.metrics.items()
        if k.startswith("entity_")
        or k
        in {
            "case_count",
            "prediction_count",
            "missing_predictions",
            "extra_predictions",
            "invalid_cases",
            "schema_validity",
            "json_validity",
        }
    }
    # Type/canonicalization disagreement counts via re-key
    type_mismatches = 0
    gold_by = {r["case_id"]: r for r in gold_records if "case_id" in r}
    pred_by = {r["case_id"]: r for r in prediction_records if "case_id" in r}
    for case_id in set(gold_by) & set(pred_by):
        gold_labels = {
            (e.get("type"), str(e.get("label", "")).lower())
            for e in gold_by[case_id].get("entities", [])
            if isinstance(e, dict)
        }
        pred_labels = {
            (e.get("type"), str(e.get("label", "")).lower())
            for e in pred_by[case_id].get("entities", [])
            if isinstance(e, dict)
        }
        # same label different type
        gold_by_label = {lab: typ for typ, lab in gold_labels}
        for typ, lab in pred_labels:
            if lab in gold_by_label and gold_by_label[lab] != typ:
                type_mismatches += 1
    entity_metrics["entity_type_mismatches"] = type_mismatches
    entity_metrics["entity_canonicalization_agreement"] = (
        1.0
        if type_mismatches == 0
        else max(0.0, 1.0 - type_mismatches / max(int(report.metrics.get("entity_gold", 1)), 1))
    )
    return ReviewedSplitReport(
        split_name=split_name,
        metrics=entity_metrics,
        disagreements=report.disagreements,
        case_count=report.case_count,
        leakage_clean=report.leakage_clean,
        confidence_intervals={
            k: v for k, v in report.confidence_intervals.items() if k.startswith("entity_")
        },
    )


def score_relation_evidence_split(
    gold_records: Sequence[Mapping[str, Any]],
    prediction_records: Sequence[Mapping[str, Any]],
    *,
    split_name: str = "relation_evidence",
) -> ReviewedSplitReport:
    """Relation + EvidencePath metrics with endpoint/type/anchor separation."""
    report = score_reviewed_split(gold_records, prediction_records, split_name=split_name)
    metrics = {
        k: v
        for k, v in report.metrics.items()
        if k.startswith("relation_")
        or k
        in {
            "case_count",
            "prediction_count",
            "missing_predictions",
            "extra_predictions",
            "invalid_cases",
            "evidence_path_validity",
            "schema_validity",
            "json_validity",
        }
    }
    # Endpoint correctness: relation endpoints present in entity id set
    endpoint_ok = 0
    endpoint_total = 0
    anchor_ok = 0
    anchor_total = 0
    type_ok = 0
    type_total = 0
    gold_by = {r["case_id"]: r for r in gold_records if "case_id" in r}
    pred_by = {r["case_id"]: r for r in prediction_records if "case_id" in r}
    for case_id in set(gold_by) & set(pred_by):
        gold_rel = {
            (
                r.get("type"),
                r.get("source"),
                r.get("target"),
            )
            for r in gold_by[case_id].get("relations", [])
            if isinstance(r, dict)
        }
        pred_entities = {
            e.get("id") for e in pred_by[case_id].get("entities", []) if isinstance(e, dict)
        }
        for rel in pred_by[case_id].get("relations", []):
            if not isinstance(rel, dict):
                continue
            type_total += 1
            endpoint_total += 1
            src, tgt = rel.get("source"), rel.get("target")
            if src in pred_entities and tgt in pred_entities:
                endpoint_ok += 1
            gold_types = {g[0] for g in gold_rel}
            if rel.get("type") in gold_types:
                type_ok += 1
            refs = rel.get("evidence_refs") or []
            anchor_total += 1
            if isinstance(refs, list) and refs:
                anchor_ok += 1
            # also count entity evidence anchors
        for ent in pred_by[case_id].get("entities", []):
            if not isinstance(ent, dict):
                continue
            refs = ent.get("evidence_refs") or []
            anchor_total += 1
            if isinstance(refs, list) and refs:
                anchor_ok += 1

    metrics["relation_endpoint_correctness"] = (
        endpoint_ok / endpoint_total if endpoint_total else 1.0
    )
    metrics["relation_type_correctness"] = type_ok / type_total if type_total else 1.0
    metrics["evidence_anchor_correctness"] = anchor_ok / anchor_total if anchor_total else 1.0

    return ReviewedSplitReport(
        split_name=split_name,
        metrics=metrics,
        disagreements=report.disagreements,
        case_count=report.case_count,
        leakage_clean=report.leakage_clean,
        confidence_intervals={
            k: v for k, v in report.confidence_intervals.items() if k.startswith("relation_")
        },
    )


__all__ = [
    "FORBIDDEN_REPORT_KEYS",
    "DisagreementEvidence",
    "ReviewedCaseReport",
    "ReviewedSplitReport",
    "score_entity_split",
    "score_relation_evidence_split",
    "score_reviewed_case",
    "score_reviewed_split",
]
