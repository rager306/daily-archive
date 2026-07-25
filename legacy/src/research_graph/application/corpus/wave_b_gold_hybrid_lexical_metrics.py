"""Wave B gold-linked hybrid lexical recovery metrics.

Deterministic floor baseline: recover gold entity labels that appear as
substrings in hybrid body text (casefold-normalized), keep relations only
when both endpoints recovered, score via evaluate_records + decide_gate_verdict.

No LLM, no DSPy, never import. This is a metrics floor, not production quality.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.extraction_ablations import decide_gate_verdict
from research_graph.application.extraction_benchmark import evaluate_records

SCHEMA_VERSION = "wave-b-reviewed-gold-hybrid-lexical-metrics.v1"


def _normalize_label(value: str) -> str:
    return " ".join(value.casefold().strip().split())


def build_lexical_recovery_prediction(
    gold: Mapping[str, Any],
    body_text: str,
) -> dict[str, Any]:
    """Build a prediction record by lexical presence of gold entity labels in body."""
    body_norm = _normalize_label(body_text or "")
    case_id = str(gold.get("case_id") or "unknown")
    paper_id = str(gold.get("paper_id") or "")
    recovered: list[dict[str, Any]] = []
    recovered_ids: set[str] = set()

    for entity in gold.get("entities") or []:
        if not isinstance(entity, Mapping):
            continue
        label = str(entity.get("label") or "")
        entity_id = str(entity.get("id") or "")
        entity_type = str(entity.get("type") or "")
        if not label or not entity_id or not entity_type:
            continue
        if _normalize_label(label) and _normalize_label(label) in body_norm:
            recovered_ids.add(entity_id)
            recovered.append(
                {
                    "id": f"pred:lexical:{entity_id}",
                    "type": entity_type,
                    "label": label,
                    "evidence_refs": [f"evidence:lexical:{case_id}:{entity_id}"],
                }
            )

    # map gold entity id -> pred entity id for relation rewrite
    id_map = {
        str(e.get("id")): f"pred:lexical:{e.get('id')}"
        for e in (gold.get("entities") or [])
        if isinstance(e, Mapping) and str(e.get("id") or "") in recovered_ids
    }
    relations: list[dict[str, Any]] = []
    for rel in gold.get("relations") or []:
        if not isinstance(rel, Mapping):
            continue
        src = str(rel.get("source") or "")
        tgt = str(rel.get("target") or "")
        rel_type = str(rel.get("type") or "")
        rel_id = str(rel.get("id") or "")
        if src in recovered_ids and tgt in recovered_ids and rel_type:
            relations.append(
                {
                    "id": f"pred:lexical:{rel_id or f'{src}:{tgt}'}",
                    "type": rel_type,
                    "source": id_map[src],
                    "target": id_map[tgt],
                    "evidence_refs": [f"evidence:lexical:{case_id}:relation:{rel_id}"],
                }
            )

    source_refs = gold.get("source_artifact_refs")
    if not isinstance(source_refs, list) or not source_refs:
        source_refs = [f"artifact:hybrid-body:{paper_id or case_id}"]

    return {
        "case_id": case_id,
        "paper_id": paper_id,
        "source_artifact_refs": list(source_refs),
        "entities": recovered,
        "relations": relations,
        "schema_valid": True,
        "json_valid": True,
        "operational": {
            "cost_estimate": 0.0,
            "latency_ms": 0,
            "retry_count": 0,
        },
    }


@dataclass(frozen=True, slots=True)
class GoldHybridLexicalMetricsPackage:
    schema_version: str
    case_count: int
    metrics: dict[str, Any]
    gate_verdict: str
    gate_reasons: tuple[str, ...]
    per_case: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    llm_used: bool = False
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gold hybrid lexical metrics cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("gold hybrid lexical metrics cannot enable DSPy")
        if self.llm_used:
            raise ValueError("gold hybrid lexical metrics must not claim LLM use")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "case_count": self.case_count,
            "metrics": dict(self.metrics),
            "gate_verdict": self.gate_verdict,
            "gate_reasons": list(self.gate_reasons),
            "per_case": list(self.per_case),
            "diagnostics": list(self.diagnostics),
            "llm_used": False,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Deterministic lexical gold-recovery floor on hybrid bodies; "
                "not production quality; not LLM; not DSPy; not import"
            ),
        }


def score_gold_hybrid_lexical_recovery(
    *,
    cases: Sequence[Mapping[str, Any]],
) -> GoldHybridLexicalMetricsPackage:
    """Score joined gold+body cases with lexical recovery predictions.

    Each case mapping requires: case_id, paper_id, gold (full gold record), body_text.
    """
    golds: list[dict[str, Any]] = []
    preds: list[dict[str, Any]] = []
    per_case: list[dict[str, Any]] = []

    for case in cases:
        gold = dict(case.get("gold") or {})
        body_text = str(case.get("body_text") or "")
        if not gold.get("case_id"):
            gold["case_id"] = str(case.get("case_id") or "unknown")
        if not gold.get("paper_id"):
            gold["paper_id"] = str(case.get("paper_id") or "")
        # ensure required gold operational fields if minimal fixture
        gold.setdefault("source_artifact_refs", ["artifact:catalog-unknown"])
        gold.setdefault("schema_valid", True)
        gold.setdefault("json_valid", True)
        gold.setdefault(
            "operational",
            {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
        )
        gold.setdefault("entities", [])
        gold.setdefault("relations", [])

        pred = build_lexical_recovery_prediction(gold, body_text)
        golds.append(gold)
        preds.append(pred)
        per_case.append(
            {
                "case_id": gold["case_id"],
                "paper_id": str(case.get("paper_id") or gold.get("paper_id") or ""),
                "entity_predicted": len(pred["entities"]),
                "entity_gold": len(gold.get("entities") or []),
                "relation_predicted": len(pred["relations"]),
                "relation_gold": len(gold.get("relations") or []),
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

    diagnostics = (
        f"case_count:{len(golds)}",
        f"entity_f1:{metrics.get('entity_f1')}",
        f"relation_f1:{metrics.get('relation_f1')}",
        f"entity_recall:{metrics.get('entity_recall')}",
        f"gate_verdict:{gate.verdict}",
        "method:lexical_gold_recovery",
        "llm:false",
        "dspy:false",
        "import_write_fail_closed",
        "floor_baseline_not_production",
    )
    return GoldHybridLexicalMetricsPackage(
        schema_version=SCHEMA_VERSION,
        case_count=len(golds),
        metrics=dict(metrics),
        gate_verdict=str(gate.verdict),
        gate_reasons=tuple(gate.reasons),
        per_case=tuple(per_case),
        diagnostics=diagnostics,
        llm_used=False,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
    )


__all__ = [
    "SCHEMA_VERSION",
    "GoldHybridLexicalMetricsPackage",
    "build_lexical_recovery_prediction",
    "score_gold_hybrid_lexical_recovery",
]
