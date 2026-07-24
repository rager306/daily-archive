"""M256 S02: deterministic lexical gold recovery scorer."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_gold_hybrid_lexical_metrics import (
    GoldHybridLexicalMetricsPackage,
    build_lexical_recovery_prediction,
    score_gold_hybrid_lexical_recovery,
)


def _gold(
    *,
    case_id: str = "case:train:1206.6423",
    paper_id: str = "arxiv:1206.6423",
    entities: list[dict] | None = None,
    relations: list[dict] | None = None,
) -> dict:
    ents = entities or [
        {
            "id": "e:1206.6423:field:language_perception",
            "type": "Field",
            "label": "Language and Perception",
            "evidence_refs": ["evidence:m072:train:1206.6423:field"],
        },
        {
            "id": "e:1206.6423:task:grounded_attribute_learning",
            "type": "Task",
            "label": "Grounded Attribute Learning",
            "evidence_refs": ["evidence:m072:train:1206.6423:task"],
        },
    ]
    rels = relations or [
        {
            "id": "r:1206.6423:applied_to",
            "type": "APPLIED_TO",
            "source": "e:1206.6423:field:language_perception",
            "target": "e:1206.6423:task:grounded_attribute_learning",
            "evidence_refs": ["evidence:m072:train:1206.6423:relation"],
        }
    ]
    return {
        "case_id": case_id,
        "paper_id": paper_id,
        "source_artifact_refs": ["artifact:catalog-arxiv-cs-cl-1206.6423"],
        "entities": ents,
        "relations": rels,
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }


def test_prediction_recovers_labels_present_in_body() -> None:
    gold = _gold()
    body = (
        "We study language and perception for grounded attribute learning "
        "in joint models."
    )
    pred = build_lexical_recovery_prediction(gold, body)
    assert pred["case_id"] == gold["case_id"]
    assert pred["schema_valid"] is True
    assert pred["json_valid"] is True
    assert len(pred["entities"]) == 2
    labels = {e["label"] for e in pred["entities"]}
    assert "Language and Perception" in labels
    assert "Grounded Attribute Learning" in labels
    # both endpoints recovered → relation kept
    assert len(pred["relations"]) == 1
    assert pred["relations"][0]["type"] == "APPLIED_TO"
    assert all(r.startswith("evidence:lexical:") for e in pred["entities"] for r in e["evidence_refs"])


def test_prediction_omits_missing_labels() -> None:
    gold = _gold()
    body = "This text mentions language and perception only."
    pred = build_lexical_recovery_prediction(gold, body)
    assert len(pred["entities"]) == 1
    assert pred["entities"][0]["label"] == "Language and Perception"
    # relation dropped because target missing
    assert pred["relations"] == []


def test_score_perfect_body() -> None:
    gold = _gold()
    body = "Language and Perception methods for Grounded Attribute Learning work."
    pkg = score_gold_hybrid_lexical_recovery(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": body,
            },
        )
    )
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.llm_used is False
    assert pkg.case_count == 1
    assert pkg.metrics["entity_f1"] == 1.0
    assert pkg.metrics["relation_f1"] == 1.0
    assert pkg.gate_verdict == "proceed"
    d = pkg.to_dict()
    assert d["import_eligible"] is False
    assert d["llm_used"] is False


def test_score_empty_body_zero_recall() -> None:
    gold = _gold()
    pkg = score_gold_hybrid_lexical_recovery(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": "unrelated filler text without labels",
            },
        )
    )
    assert pkg.metrics["entity_f1"] == 0.0
    assert pkg.metrics["entity_recall"] == 0.0
    assert pkg.gate_verdict in {"repair", "stop"}


def test_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        GoldHybridLexicalMetricsPackage(
            schema_version="m256-wave-b-gold-hybrid-lexical-metrics.v1",
            case_count=0,
            metrics={},
            gate_verdict="stop",
            gate_reasons=(),
            per_case=(),
            diagnostics=(),
            llm_used=False,
            dspy_optimizer_enabled=False,
            import_eligible=True,
            graph_writes_allowed=False,
        )
