"""M284 S02: canary prediction resolvability pure module."""

from __future__ import annotations

from research_graph.application.corpus.canary_prediction_resolvability import (
    evaluate_prediction_resolvability,
    ground_prediction_to_spans,
)


def _layout() -> dict:
    return {
        "kids": [
            {
                "type": "heading",
                "heading level": 1,
                "page number": 1,
                "bounding box": [10.0, 20.0, 100.0, 40.0],
                "content": "Seq2Seq Models for Knowledge Graph Link Prediction",
                "id": 1,
            },
            {
                "type": "paragraph",
                "page number": 1,
                "bounding box": [10.0, 50.0, 200.0, 80.0],
                "content": "We study Seq2Seq Models on link prediction tasks.",
                "id": 2,
            },
        ]
    }


def _body() -> str:
    return "Seq2Seq Models for Knowledge Graph Link Prediction\nWe study Seq2Seq Models on link prediction tasks."


def test_ground_prediction_upgrades_to_page_bbox() -> None:
    pred = {
        "case_id": "c1",
        "entities": [
            {"id": "e1", "label": "Seq2Seq Models", "type": "Method"},
        ],
        "relations": [],
    }
    gold_u, stats = ground_prediction_to_spans(
        prediction=pred,
        body_text=_body(),
        case_id="c1",
        paper_id="p1",
        layout_json=_layout(),
    )
    assert stats["spans_total"] >= 1
    assert stats["spans_upgraded"] >= 1
    ent = gold_u["entities"][0]
    span = ent["spans"][0]
    assert span["page"] == 1
    assert span["bbox"] is not None
    assert span["justified_char_only"] is False


def test_prediction_resolvability_metric() -> None:
    cases = [
        {
            "case_id": "c1",
            "paper_id": "p1",
            "body_text": _body(),
            "layout_json": _layout(),
        },
        {
            "case_id": "c2",
            "paper_id": "p2",
            "body_text": "Transformer architectures improve translation quality.",
            "layout_json": {
                "kids": [
                    {
                        "type": "paragraph",
                        "page number": 2,
                        "bounding box": [1.0, 2.0, 3.0, 4.0],
                        "content": "Transformer architectures improve translation quality.",
                        "id": 1,
                    }
                ]
            },
        },
    ]
    predictions = [
        {
            "case_id": "c1",
            "entities": [{"id": "e1", "label": "Seq2Seq Models", "type": "Method"}],
            "relations": [],
        },
        {
            "case_id": "c2",
            "entities": [
                {"id": "e2", "label": "Transformer", "type": "Model"},
            ],
            "relations": [],
        },
    ]
    pkg = evaluate_prediction_resolvability(
        cases=cases, predictions=predictions, target_rate=0.5, min_n=1
    )
    assert pkg.import_eligible is False
    assert pkg.llm_used is True
    assert pkg.gt_isolation == "canary_held_out_only"
    assert pkg.page_or_bbox_count >= 2
    assert pkg.resolvability_rate == 1.0
    assert pkg.target_met is True
    assert len(pkg.per_paper) == 2


def test_prediction_resolvability_char_only_when_no_layout() -> None:
    cases = [
        {"case_id": "c1", "paper_id": "p1", "body_text": _body(), "layout_json": None},
    ]
    predictions = [
        {
            "case_id": "c1",
            "entities": [{"id": "e1", "label": "Seq2Seq Models", "type": "Method"}],
            "relations": [],
        },
    ]
    pkg = evaluate_prediction_resolvability(
        cases=cases, predictions=predictions, target_rate=0.5, min_n=1
    )
    assert pkg.page_or_bbox_count == 0
    assert pkg.char_only_count >= 1
    assert any("no_layout_json" in a for a in pkg.alerts)
