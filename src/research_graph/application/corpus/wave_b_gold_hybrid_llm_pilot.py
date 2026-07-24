"""Wave B gold-linked hybrid LLM extraction pilot.

Bounded pilot: run injected structured extract over joined gold↔hybrid cases,
score with evaluate_records + decide_gate_verdict, compare to lexical floor.

LLM is optional at the application boundary (injectible callable).
Never DSPy. Never import. import_eligible always false.
"""

from __future__ import annotations

import json
import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from research_graph.application.extraction_ablations import decide_gate_verdict
from research_graph.application.extraction_benchmark import evaluate_records

SCHEMA_VERSION = "wave-b-reviewed-gold-hybrid-llm-pilot.v1"

# Injectible: (body_text, case_id) -> raw {"entities":[...], "relations":[...]}
LlmCaseExtractFn = Callable[[str, str], Mapping[str, Any]]

_FENCE_RE = re.compile(r"```(?:json)?\s*([\s\S]*?)\s*```", re.IGNORECASE)
_THINK_RE = re.compile(r"<think>[\s\S]*?</think>", re.IGNORECASE)

ALLOWED_ENTITY_TYPES = frozenset({"Field", "Task", "Method", "Dataset", "Model", "Metric"})
ALLOWED_RELATION_TYPES = frozenset({"APPLIED_TO", "USES_COMPONENT", "EVALUATED_ON", "OUTPERFORMS"})


def parse_llm_extraction_json(text: str) -> dict[str, Any]:
    """Parse LLM chat text into extraction dict; fail-closed to empty lists."""
    raw = _THINK_RE.sub("", text or "").strip()
    if not raw:
        return {"entities": [], "relations": [], "json_valid": False}

    candidates: list[str] = [raw]
    m = _FENCE_RE.search(raw)
    if m:
        candidates.insert(0, m.group(1).strip())
    # try substring from first { to last }
    if "{" in raw and "}" in raw:
        candidates.append(raw[raw.find("{") : raw.rfind("}") + 1])

    for cand in candidates:
        try:
            data = json.loads(cand)
        except json.JSONDecodeError:
            continue
        if not isinstance(data, dict):
            continue
        entities = data.get("entities")
        relations = data.get("relations")
        if not isinstance(entities, list):
            entities = []
        if not isinstance(relations, list):
            relations = []
        return {
            "entities": [e for e in entities if isinstance(e, Mapping)],
            "relations": [r for r in relations if isinstance(r, Mapping)],
            "json_valid": True,
        }
    return {"entities": [], "relations": [], "json_valid": False}


def _slug(value: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "_", value.casefold().strip())
    return s.strip("_")[:80] or "x"


def build_llm_prediction_record(
    *,
    case_id: str,
    paper_id: str,
    llm_payload: Mapping[str, Any],
    source_artifact_refs: Sequence[str] | None = None,
    latency_ms: int = 0,
    retry_count: int = 0,
) -> dict[str, Any]:
    """Map raw LLM payload to evaluate_records prediction shape."""
    json_valid = bool(llm_payload.get("json_valid", True))
    entities_out: list[dict[str, Any]] = []
    label_to_id: dict[str, str] = {}

    for idx, entity in enumerate(llm_payload.get("entities") or []):
        if not isinstance(entity, Mapping):
            continue
        label = str(entity.get("label") or entity.get("canonical_name") or "").strip()
        etype = str(entity.get("type") or entity.get("entity_type") or "").strip()
        if not label or not etype:
            continue
        # normalize type casing to Title-ish gold style when all-lower
        if etype.islower():
            etype = etype[:1].upper() + etype[1:]
        if etype not in ALLOWED_ENTITY_TYPES:
            # keep but title-case common synonyms
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
        eid = f"pred:llm:{case_id}:{etype}:{_slug(label)}:{idx}"
        label_key = _normalize_label(label)
        label_to_id[label_key] = eid
        entities_out.append(
            {
                "id": eid,
                "type": etype,
                "label": label,
                "evidence_refs": [f"evidence:llm:{case_id}:{_slug(label)}"],
            }
        )

    relations_out: list[dict[str, Any]] = []
    for idx, rel in enumerate(llm_payload.get("relations") or []):
        if not isinstance(rel, Mapping):
            continue
        rtype = str(rel.get("type") or rel.get("relation_type") or "").strip()
        if rtype.islower():
            rtype = rtype.upper()
        if rtype not in ALLOWED_RELATION_TYPES:
            continue
        # accept source/target ids or labels
        src = str(rel.get("source") or rel.get("from_name") or rel.get("source_label") or "").strip()
        tgt = str(rel.get("target") or rel.get("to_name") or rel.get("target_label") or "").strip()
        if not src or not tgt:
            continue
        src_id = src if src in {e["id"] for e in entities_out} else label_to_id.get(_normalize_label(src))
        tgt_id = tgt if tgt in {e["id"] for e in entities_out} else label_to_id.get(_normalize_label(tgt))
        if not src_id or not tgt_id:
            continue
        relations_out.append(
            {
                "id": f"pred:llm:rel:{case_id}:{idx}",
                "type": rtype,
                "source": src_id,
                "target": tgt_id,
                "evidence_refs": [f"evidence:llm:{case_id}:relation:{idx}"],
            }
        )

    refs = list(source_artifact_refs or [])
    if not refs:
        refs = [f"artifact:hybrid-body:{paper_id or case_id}"]

    schema_valid = json_valid and bool(entities_out or not llm_payload.get("entities"))
    # schema_valid true when structure is well-formed for evaluator
    schema_valid = json_valid

    return {
        "case_id": case_id,
        "paper_id": paper_id,
        "source_artifact_refs": refs,
        "entities": entities_out,
        "relations": relations_out,
        "schema_valid": schema_valid,
        "json_valid": json_valid,
        "operational": {
            "cost_estimate": 0.0,
            "latency_ms": int(latency_ms),
            "retry_count": int(retry_count),
        },
    }


def _normalize_label(value: str) -> str:
    return " ".join(value.casefold().strip().split())


def truncate_body_for_pilot(body_text: str, *, max_chars: int = 8000) -> str:
    """Deterministic body window for pilot cost control."""
    text = body_text or ""
    if len(text) <= max_chars:
        return text
    head = max_chars * 3 // 4
    tail = max_chars - head
    return text[:head] + "\n\n[...truncated...]\n\n" + text[-tail:]


@dataclass(frozen=True, slots=True)
class GoldHybridLlmPilotPackage:
    schema_version: str
    case_count: int
    metrics: dict[str, Any]
    floor_metrics: dict[str, Any] | None
    gate_verdict: str
    gate_reasons: tuple[str, ...]
    per_case: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    llm_used: bool = True
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    model_id: str = ""

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("llm pilot cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("llm pilot cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "case_count": self.case_count,
            "metrics": dict(self.metrics),
            "floor_metrics": dict(self.floor_metrics) if self.floor_metrics else None,
            "gate_verdict": self.gate_verdict,
            "gate_reasons": list(self.gate_reasons),
            "per_case": list(self.per_case),
            "diagnostics": list(self.diagnostics),
            "llm_used": True,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "model_id": self.model_id,
            "note": (
                "Bounded LLM extract pilot on gold-hybrid join; "
                "compare to lexical floor; not DSPy; not import"
            ),
        }


def score_gold_hybrid_llm_pilot(
    *,
    cases: Sequence[Mapping[str, Any]],
    extract_fn: LlmCaseExtractFn | None = None,
    predictions: Sequence[Mapping[str, Any]] | None = None,
    floor_metrics: Mapping[str, Any] | None = None,
    model_id: str = "",
    max_body_chars: int = 8000,
) -> GoldHybridLlmPilotPackage:
    """Score joined gold+body cases with LLM predictions.

    Provide either ``predictions`` (precomputed evaluate_records rows) or
    ``extract_fn`` called per case. Each case needs case_id, paper_id, gold, body_text.
    """
    if extract_fn is None and predictions is None:
        raise ValueError("score_gold_hybrid_llm_pilot requires extract_fn or predictions")

    golds: list[dict[str, Any]] = []
    preds: list[dict[str, Any]] = []
    per_case: list[dict[str, Any]] = []
    pred_by_case: dict[str, Mapping[str, Any]] = {}
    if predictions is not None:
        for p in predictions:
            pred_by_case[str(p.get("case_id") or "")] = p

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

        if case_id in pred_by_case:
            pred = dict(pred_by_case[case_id])
        else:
            assert extract_fn is not None
            window = truncate_body_for_pilot(body_text, max_chars=max_body_chars)
            raw = dict(extract_fn(window, case_id))
            pred = build_llm_prediction_record(
                case_id=case_id,
                paper_id=paper_id,
                llm_payload=raw,
                source_artifact_refs=gold.get("source_artifact_refs"),
            )

        golds.append(gold)
        preds.append(pred)
        per_case.append(
            {
                "case_id": case_id,
                "paper_id": paper_id,
                "entity_predicted": len(pred.get("entities") or []),
                "entity_gold": len(gold.get("entities") or []),
                "relation_predicted": len(pred.get("relations") or []),
                "relation_gold": len(gold.get("relations") or []),
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
        ) and float(metrics.get("relation_f1") or 0.0) >= float(floor.get("relation_f1") or 0.0)

    diagnostics = (
        f"case_count:{len(golds)}",
        f"entity_f1:{metrics.get('entity_f1')}",
        f"relation_f1:{metrics.get('relation_f1')}",
        f"entity_recall:{metrics.get('entity_recall')}",
        f"gate_verdict:{gate.verdict}",
        f"model:{model_id or 'unspecified'}",
        f"beats_lexical_floor:{beats_floor}",
        "method:llm_json_extract_pilot",
        "llm:true",
        "dspy:false",
        "import_write_fail_closed",
        "pilot_not_production",
    )
    return GoldHybridLlmPilotPackage(
        schema_version=SCHEMA_VERSION,
        case_count=len(golds),
        metrics=dict(metrics),
        floor_metrics=floor,
        gate_verdict=str(gate.verdict),
        gate_reasons=tuple(gate.reasons),
        per_case=tuple(per_case),
        diagnostics=diagnostics,
        llm_used=True,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
        model_id=model_id,
    )


__all__ = [
    "ALLOWED_ENTITY_TYPES",
    "ALLOWED_RELATION_TYPES",
    "SCHEMA_VERSION",
    "GoldHybridLlmPilotPackage",
    "LlmCaseExtractFn",
    "build_llm_prediction_record",
    "parse_llm_extraction_json",
    "score_gold_hybrid_llm_pilot",
    "truncate_body_for_pilot",
]
