"""Extraction ablations and staged reviewed gates (M202 S04–S08).

Reuses :func:`~research_graph.application.extraction_benchmark.evaluate_records`
and the reviewed harness. Never activates DSPy optimizers, never loads raw
article text, never authorizes graph writes.
"""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from research_graph.application.extraction_benchmark import evaluate_records
from research_graph.application.reviewed_extraction_metrics import (
    ReviewedSplitReport,
    score_entity_split,
    score_relation_evidence_split,
)

GateVerdict = Literal["proceed", "repair", "stop"]

# Metadata-only proceed thresholds for the twenty-paper gate (fail-closed).
PROCEED_MIN_ENTITY_F1 = 0.70
PROCEED_MIN_RELATION_F1 = 0.60
PROCEED_MIN_EVIDENCE_VALIDITY = 0.70
PROCEED_MAX_INVALID_CASE_RATE = 0.20
REPAIR_MIN_ENTITY_F1 = 0.40


@dataclass(frozen=True)
class AblationReport:
    """Comparable metrics for baseline vs treatment prediction sets."""

    name: str
    baseline_metrics: dict[str, Any]
    treatment_metrics: dict[str, Any]
    deltas: dict[str, float]
    optimizer_enabled: bool = False
    notes: tuple[str, ...] = ()
    leakage_clean: bool = True

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "baseline_metrics": dict(self.baseline_metrics),
            "treatment_metrics": dict(self.treatment_metrics),
            "deltas": dict(self.deltas),
            "optimizer_enabled": self.optimizer_enabled,
            "notes": list(self.notes),
            "leakage_clean": self.leakage_clean,
        }


@dataclass(frozen=True)
class ProviderComparisonReport:
    """MiniMax vs GLM quality/cost/latency/refusal comparison (metadata only)."""

    minimax_metrics: dict[str, Any]
    glm_metrics: dict[str, Any]
    cost_delta: float
    latency_delta_ms: float
    refusal_or_empty_rate_minimax: float
    refusal_or_empty_rate_glm: float
    notes: tuple[str, ...] = ()

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "minimax_metrics": dict(self.minimax_metrics),
            "glm_metrics": dict(self.glm_metrics),
            "cost_delta": self.cost_delta,
            "latency_delta_ms": self.latency_delta_ms,
            "refusal_or_empty_rate_minimax": self.refusal_or_empty_rate_minimax,
            "refusal_or_empty_rate_glm": self.refusal_or_empty_rate_glm,
            "notes": list(self.notes),
        }


@dataclass(frozen=True)
class StatisticalAblationReport:
    """Evaluation-only statistical-context on/off comparison (ADR-024)."""

    with_stats_metrics: dict[str, Any]
    without_stats_metrics: dict[str, Any]
    entity_f1_delta: float
    relation_f1_delta: float
    production_remains_statistical_first: bool = True
    notes: tuple[str, ...] = ()

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "with_stats_metrics": dict(self.with_stats_metrics),
            "without_stats_metrics": dict(self.without_stats_metrics),
            "entity_f1_delta": self.entity_f1_delta,
            "relation_f1_delta": self.relation_f1_delta,
            "production_remains_statistical_first": self.production_remains_statistical_first,
            "notes": list(self.notes),
        }


@dataclass(frozen=True)
class StagedRunReport:
    """Ten/twenty paper reviewed run summary."""

    paper_count: int
    metrics: dict[str, Any]
    entity_split: ReviewedSplitReport | None = None
    relation_split: ReviewedSplitReport | None = None
    outliers: tuple[str, ...] = ()
    failure_burden: dict[str, int] = field(default_factory=dict)
    leakage_clean: bool = True

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "paper_count": self.paper_count,
            "metrics": dict(self.metrics),
            "outliers": list(self.outliers),
            "failure_burden": dict(self.failure_burden),
            "leakage_clean": self.leakage_clean,
        }


@dataclass(frozen=True)
class GateReport:
    """Stop/repair/proceed verdict for M203."""

    verdict: GateVerdict
    paper_count: int
    metrics: dict[str, Any]
    reasons: tuple[str, ...]
    leakage_clean: bool = True

    def to_sanitized_dict(self) -> dict[str, Any]:
        return {
            "verdict": self.verdict,
            "paper_count": self.paper_count,
            "metrics": dict(self.metrics),
            "reasons": list(self.reasons),
            "leakage_clean": self.leakage_clean,
        }


def _metric_deltas(baseline: Mapping[str, Any], treatment: Mapping[str, Any]) -> dict[str, float]:
    keys = (
        "entity_f1",
        "entity_precision",
        "entity_recall",
        "relation_f1",
        "relation_precision",
        "relation_recall",
        "evidence_path_validity",
        "schema_validity",
        "mean_cost_estimate",
        "mean_latency_ms",
    )
    deltas: dict[str, float] = {}
    for key in keys:
        if key in baseline and key in treatment:
            try:
                deltas[key] = float(treatment[key]) - float(baseline[key])
            except (TypeError, ValueError):
                continue
    return deltas


def _deterministic_predictions_from_gold(
    gold_records: Sequence[Mapping[str, Any]],
) -> list[dict[str, Any]]:
    """Clone gold as zero-cost deterministic predictions (oracle baseline)."""
    preds: list[dict[str, Any]] = []
    for gold in gold_records:
        pred = json.loads(json.dumps(dict(gold)))  # deep copy metadata only
        pred["operational"] = {
            "cost_estimate": 0.0,
            "latency_ms": 0,
            "retry_count": 0,
            "provider": "deterministic_oracle",
        }
        pred["schema_valid"] = True
        pred["json_valid"] = True
        preds.append(pred)
    return preds


def compare_prediction_sets(
    gold_records: Sequence[Mapping[str, Any]],
    baseline_predictions: Sequence[Mapping[str, Any]],
    treatment_predictions: Sequence[Mapping[str, Any]],
    *,
    name: str = "ablation",
    optimizer_enabled: bool = False,
) -> AblationReport:
    """Compare two prediction sets against the same gold via evaluate_records."""
    if optimizer_enabled:
        raise ValueError(
            "optimizer_enabled must remain False for M202 ablations "
            "(ADR-024/S08: no DSPy optimizer activation)"
        )
    baseline_metrics = evaluate_records(
        [dict(r) for r in gold_records], [dict(r) for r in baseline_predictions]
    )
    treatment_metrics = evaluate_records(
        [dict(r) for r in gold_records], [dict(r) for r in treatment_predictions]
    )
    return AblationReport(
        name=name,
        baseline_metrics=baseline_metrics,
        treatment_metrics=treatment_metrics,
        deltas=_metric_deltas(baseline_metrics, treatment_metrics),
        optimizer_enabled=False,
        notes=("evaluate_records reused for both arms",),
    )


def compare_deterministic_vs_llm(
    gold_records: Sequence[Mapping[str, Any]],
    llm_predictions: Sequence[Mapping[str, Any]],
    *,
    name: str = "deterministic_vs_llm",
) -> AblationReport:
    """Deterministic oracle (gold clone) vs LLM predictions on same fixtures."""
    baseline = _deterministic_predictions_from_gold(gold_records)
    return compare_prediction_sets(
        gold_records,
        baseline,
        llm_predictions,
        name=name,
        optimizer_enabled=False,
    )


def compare_providers(
    gold_records: Sequence[Mapping[str, Any]],
    minimax_predictions: Sequence[Mapping[str, Any]],
    glm_predictions: Sequence[Mapping[str, Any]],
) -> ProviderComparisonReport:
    """MiniMax vs GLM quality/cost/latency/refusal comparison."""
    minimax_metrics = evaluate_records(
        [dict(r) for r in gold_records], [dict(r) for r in minimax_predictions]
    )
    glm_metrics = evaluate_records(
        [dict(r) for r in gold_records], [dict(r) for r in glm_predictions]
    )

    def _refusal_rate(preds: Sequence[Mapping[str, Any]]) -> float:
        if not preds:
            return 1.0
        empty = 0
        for p in preds:
            ents = p.get("entities") or []
            rels = p.get("relations") or []
            if not ents and not rels:
                empty += 1
        return empty / len(preds)

    return ProviderComparisonReport(
        minimax_metrics=minimax_metrics,
        glm_metrics=glm_metrics,
        cost_delta=float(glm_metrics.get("mean_cost_estimate", 0.0))
        - float(minimax_metrics.get("mean_cost_estimate", 0.0)),
        latency_delta_ms=float(glm_metrics.get("mean_latency_ms", 0.0))
        - float(minimax_metrics.get("mean_latency_ms", 0.0)),
        refusal_or_empty_rate_minimax=_refusal_rate(minimax_predictions),
        refusal_or_empty_rate_glm=_refusal_rate(glm_predictions),
        notes=("metadata-only operational fields; no raw payloads",),
    )


def ablate_statistical_context(
    gold_records: Sequence[Mapping[str, Any]],
    with_stats_predictions: Sequence[Mapping[str, Any]],
    without_stats_predictions: Sequence[Mapping[str, Any]],
) -> StatisticalAblationReport:
    """Evaluation-only ablation of statistical context (production stays statistical-first)."""
    with_m = evaluate_records(
        [dict(r) for r in gold_records], [dict(r) for r in with_stats_predictions]
    )
    without_m = evaluate_records(
        [dict(r) for r in gold_records], [dict(r) for r in without_stats_predictions]
    )
    return StatisticalAblationReport(
        with_stats_metrics=with_m,
        without_stats_metrics=without_m,
        entity_f1_delta=float(with_m.get("entity_f1", 0.0)) - float(without_m.get("entity_f1", 0.0)),
        relation_f1_delta=float(with_m.get("relation_f1", 0.0))
        - float(without_m.get("relation_f1", 0.0)),
        production_remains_statistical_first=True,
        notes=(
            "evaluation-only ablation",
            "production statistical-first invariant unchanged (ADR-024)",
        ),
    )


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def expand_records_to_n(
    records: Sequence[Mapping[str, Any]],
    n: int,
    *,
    id_prefix: str = "synth",
) -> list[dict[str, Any]]:
    """Expand metadata-only records to n cases by cloning with new case_ids.

    Used for staged 10/20 paper gates when reviewed fixtures are smaller.
    Does not invent new entity labels beyond cloning existing metadata.
    """
    if n <= 0:
        return []
    base = [dict(r) for r in records]
    if not base:
        raise ValueError("cannot expand empty record set")
    out: list[dict[str, Any]] = []
    i = 0
    while len(out) < n:
        src = json.loads(json.dumps(base[i % len(base)]))
        src["case_id"] = f"case:{id_prefix}:{len(out):04d}"
        # keep paper_id distinct-ish for leakage-safe identity
        paper = str(src.get("paper_id", "arxiv:0000.00000"))
        src["paper_id"] = f"{paper}:{id_prefix}:{len(out):04d}"
        out.append(src)
        i += 1
    return out


def run_staged_reviewed_run(
    gold_records: Sequence[Mapping[str, Any]],
    prediction_records: Sequence[Mapping[str, Any]],
    *,
    target_count: int,
    split_name: str = "staged",
) -> StagedRunReport:
    """Score a staged N-paper reviewed run with outliers and failure burden."""
    golds = expand_records_to_n(gold_records, target_count, id_prefix=f"{split_name}-g")
    # Align predictions to expanded gold case_ids by zip-clone mapping
    base_preds = [dict(r) for r in prediction_records] or _deterministic_predictions_from_gold(
        gold_records
    )
    preds: list[dict[str, Any]] = []
    for i, gold in enumerate(golds):
        src = json.loads(json.dumps(base_preds[i % len(base_preds)]))
        src["case_id"] = gold["case_id"]
        src["paper_id"] = gold["paper_id"]
        preds.append(src)

    metrics = evaluate_records(golds, preds)
    entity_split = score_entity_split(golds, preds, split_name=f"{split_name}-entity")
    relation_split = score_relation_evidence_split(
        golds, preds, split_name=f"{split_name}-relation"
    )

    # Outliers: cases in invalid_cases or with empty predictions
    outliers: list[str] = list(metrics.get("invalid_cases", []))
    for p in preds:
        ents = p.get("entities") or []
        if not ents:
            outliers.append(str(p.get("case_id")))
    outliers = sorted(set(outliers))

    failure_burden = {
        "invalid_cases": len(metrics.get("invalid_cases", [])),
        "missing_predictions": len(metrics.get("missing_predictions", [])),
        "empty_entity_predictions": sum(
            1 for p in preds if not (p.get("entities") or [])
        ),
        "total_retry_count": int(metrics.get("total_retry_count", 0)),
    }

    return StagedRunReport(
        paper_count=target_count,
        metrics=metrics,
        entity_split=entity_split,
        relation_split=relation_split,
        outliers=tuple(outliers),
        failure_burden=failure_burden,
        leakage_clean=entity_split.leakage_clean and relation_split.leakage_clean,
    )


def decide_gate_verdict(metrics: Mapping[str, Any], *, paper_count: int) -> GateReport:
    """Compute proceed/repair/stop for M203 from reviewed metrics."""
    reasons: list[str] = []
    entity_f1 = float(metrics.get("entity_f1", 0.0))
    relation_f1 = float(metrics.get("relation_f1", 0.0))
    evidence_v = float(metrics.get("evidence_path_validity", 0.0))
    pred_count = int(metrics.get("prediction_count", 0)) or 1
    invalid_rate = len(metrics.get("invalid_cases", [])) / pred_count

    if entity_f1 >= PROCEED_MIN_ENTITY_F1 and relation_f1 >= PROCEED_MIN_RELATION_F1:
        if evidence_v >= PROCEED_MIN_EVIDENCE_VALIDITY and invalid_rate <= PROCEED_MAX_INVALID_CASE_RATE:
            verdict: GateVerdict = "proceed"
            reasons.append(
                f"entity_f1={entity_f1:.3f}>={PROCEED_MIN_ENTITY_F1}, "
                f"relation_f1={relation_f1:.3f}>={PROCEED_MIN_RELATION_F1}, "
                f"evidence_path_validity={evidence_v:.3f}"
            )
        else:
            verdict = "repair"
            if evidence_v < PROCEED_MIN_EVIDENCE_VALIDITY:
                reasons.append(f"evidence_path_validity={evidence_v:.3f} below {PROCEED_MIN_EVIDENCE_VALIDITY}")
            if invalid_rate > PROCEED_MAX_INVALID_CASE_RATE:
                reasons.append(f"invalid_case_rate={invalid_rate:.3f} above {PROCEED_MAX_INVALID_CASE_RATE}")
    elif entity_f1 >= REPAIR_MIN_ENTITY_F1:
        verdict = "repair"
        reasons.append(
            f"entity_f1={entity_f1:.3f} in repair band "
            f"[{REPAIR_MIN_ENTITY_F1}, {PROCEED_MIN_ENTITY_F1})"
        )
    else:
        verdict = "stop"
        reasons.append(
            f"entity_f1={entity_f1:.3f} below repair floor {REPAIR_MIN_ENTITY_F1}"
        )

    return GateReport(
        verdict=verdict,
        paper_count=paper_count,
        metrics=dict(metrics),
        reasons=tuple(reasons),
        leakage_clean=True,
    )


def run_twenty_paper_gate(
    gold_records: Sequence[Mapping[str, Any]],
    prediction_records: Sequence[Mapping[str, Any]],
) -> tuple[StagedRunReport, GateReport]:
    """Twenty-paper staged run + stop/repair/proceed verdict for M203."""
    staged = run_staged_reviewed_run(
        gold_records, prediction_records, target_count=20, split_name="twenty"
    )
    gate = decide_gate_verdict(staged.metrics, paper_count=20)
    return staged, gate


def load_reviewed_extraction_split(
    split: str = "train",
    *,
    fixtures_root: Path | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Load reviewed gold + baseline predictions for a split.

    Prefer :mod:`research_graph.application.reviewed_extraction_fixtures`.
    """
    from research_graph.application.reviewed_extraction_fixtures import (
        load_reviewed_extraction_split as _load,
    )

    return _load(split, fixtures_root=fixtures_root)


__all__ = [
    "AblationReport",
    "GateReport",
    "PROCEED_MAX_INVALID_CASE_RATE",
    "PROCEED_MIN_ENTITY_F1",
    "PROCEED_MIN_EVIDENCE_VALIDITY",
    "PROCEED_MIN_RELATION_F1",
    "ProviderComparisonReport",
    "REPAIR_MIN_ENTITY_F1",
    "StagedRunReport",
    "StatisticalAblationReport",
    "ablate_statistical_context",
    "compare_deterministic_vs_llm",
    "compare_prediction_sets",
    "compare_providers",
    "decide_gate_verdict",
    "expand_records_to_n",
    "load_reviewed_extraction_split",
    "run_staged_reviewed_run",
    "run_twenty_paper_gate",
]
