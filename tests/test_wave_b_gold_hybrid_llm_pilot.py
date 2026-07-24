"""Wave B gold-hybrid LLM extract pilot (unit, no live network)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    GoldHybridLlmPilotPackage,
    build_llm_prediction_record,
    parse_llm_extraction_json,
    score_gold_hybrid_llm_pilot,
    truncate_body_for_pilot,
)


def _gold(
    *,
    case_id: str = "case:train:1206.6423",
    paper_id: str = "arxiv:1206.6423",
) -> dict:
    return {
        "case_id": case_id,
        "paper_id": paper_id,
        "source_artifact_refs": ["artifact:catalog-arxiv-cs-cl-1206.6423"],
        "entities": [
            {
                "id": "e:1206.6423:field:language_perception",
                "type": "Field",
                "label": "Language and Perception",
                "evidence_refs": ["evidence:fixture:field"],
            },
            {
                "id": "e:1206.6423:task:grounded_attribute_learning",
                "type": "Task",
                "label": "Grounded Attribute Learning",
                "evidence_refs": ["evidence:fixture:task"],
            },
        ],
        "relations": [
            {
                "id": "r:1206.6423:applied_to",
                "type": "APPLIED_TO",
                "source": "e:1206.6423:field:language_perception",
                "target": "e:1206.6423:task:grounded_attribute_learning",
                "evidence_refs": ["evidence:fixture:relation"],
            }
        ],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }


def test_parse_llm_json_plain_and_fenced() -> None:
    plain = parse_llm_extraction_json(
        '{"entities":[{"type":"Method","label":"A"}],"relations":[]}'
    )
    assert plain["json_valid"] is True
    assert plain["entities"][0]["label"] == "A"

    fenced = parse_llm_extraction_json(
        'Here:\n```json\n{"entities":[{"type":"Task","label":"B"}],"relations":[]}\n```\n'
    )
    assert fenced["json_valid"] is True
    assert fenced["entities"][0]["label"] == "B"

    bad = parse_llm_extraction_json("not json at all")
    assert bad["json_valid"] is False
    assert bad["entities"] == []


def test_build_prediction_maps_labels_to_relation_endpoints() -> None:
    pred = build_llm_prediction_record(
        case_id="case:train:x",
        paper_id="x",
        llm_payload={
            "json_valid": True,
            "entities": [
                {"type": "Field", "label": "Language and Perception"},
                {"type": "Task", "label": "Grounded Attribute Learning"},
            ],
            "relations": [
                {
                    "type": "APPLIED_TO",
                    "source_label": "Language and Perception",
                    "target_label": "Grounded Attribute Learning",
                }
            ],
        },
    )
    assert pred["schema_valid"] is True
    assert len(pred["entities"]) == 2
    assert len(pred["relations"]) == 1
    assert pred["relations"][0]["type"] == "APPLIED_TO"
    entity_ids = {e["id"] for e in pred["entities"]}
    assert pred["relations"][0]["source"] in entity_ids
    assert pred["relations"][0]["target"] in entity_ids


def test_score_with_perfect_extract_fn() -> None:
    gold = _gold()

    def extract(_body: str, _case_id: str) -> dict:
        return {
            "json_valid": True,
            "entities": [
                {"type": "Field", "label": "Language and Perception"},
                {"type": "Task", "label": "Grounded Attribute Learning"},
            ],
            "relations": [
                {
                    "type": "APPLIED_TO",
                    "source_label": "Language and Perception",
                    "target_label": "Grounded Attribute Learning",
                }
            ],
        }

    pkg = score_gold_hybrid_llm_pilot(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": "Language and Perception / Grounded Attribute Learning",
            },
        ),
        extract_fn=extract,
        floor_metrics={"entity_f1": 0.5, "relation_f1": 0.5},
        model_id="fake",
    )
    assert isinstance(pkg, GoldHybridLlmPilotPackage)
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.llm_used is True
    assert pkg.metrics["entity_f1"] == 1.0
    assert pkg.metrics["relation_f1"] == 1.0
    assert "beats_lexical_floor:True" in pkg.diagnostics


def test_score_requires_extract_or_predictions() -> None:
    with pytest.raises(ValueError, match="extract_fn or predictions"):
        score_gold_hybrid_llm_pilot(cases=())


def test_package_rejects_import() -> None:
    with pytest.raises(ValueError, match="import"):
        GoldHybridLlmPilotPackage(
            schema_version="x",
            case_count=0,
            metrics={},
            floor_metrics=None,
            gate_verdict="hold",
            gate_reasons=(),
            per_case=(),
            diagnostics=(),
            import_eligible=True,
        )


def test_truncate_body() -> None:
    text = "a" * 10000
    out = truncate_body_for_pilot(text, max_chars=100)
    assert len(out) < 200
    assert "truncated" in out


def test_ninerouter_json_extract_client_parse_path() -> None:
    from research_graph.infrastructure.llm.ninerouter_json_extract import (
        NineRouterJsonExtractClient,
    )
    from research_graph.infrastructure.llm.ninerouter_client import NineRouterChatClient

    def fake_post(method, url, headers, body):
        return {
            "choices": [
                {
                    "message": {
                        "content": (
                            '{"entities":[{"type":"Method","label":"NMT"}],'
                            '"relations":[]}'
                        )
                    }
                }
            ],
            "usage": {"total_tokens": 10},
        }

    client = NineRouterJsonExtractClient(
        chat_client=NineRouterChatClient(http_post_json=fake_post),
        model="test-model",
    )
    out = client.extract_case("Neural machine translation text", "case:x")
    assert out["json_valid"] is True
    assert out["entities"][0]["label"] == "NMT"
    assert client.last_diagnostics["chat_ok"] is True
