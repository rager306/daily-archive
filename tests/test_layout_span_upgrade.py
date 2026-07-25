"""M282: upgrade char spans with layout page/bbox when ODL JSON present."""

from __future__ import annotations

from research_graph.application.corpus.layout_span_upgrade import (
    match_layout_for_surface,
    upgrade_grounded_gold_with_layout,
    upgrade_spans_with_layout_json,
)


def test_match_and_upgrade_page_bbox() -> None:
    layout = {
        "elements": [
            {
                "type": "paragraph",
                "text": "Seq2Seq Models for graphs",
                "page": 1,
                "bbox": [10.0, 20.0, 100.0, 40.0],
                "id": "e1",
            }
        ]
    }
    spans = [
        {
            "artifact_hash": "h",
            "char_start": 0,
            "char_end": 13,
            "surface": "Seq2Seq Models",
            "justified_char_only": True,
            "page": None,
            "bbox": None,
        }
    ]
    hit = match_layout_for_surface("Seq2Seq Models", layout["elements"])
    assert hit is not None
    new_spans, stats = upgrade_spans_with_layout_json(spans, layout)
    assert stats["upgraded"] == 1
    assert new_spans[0]["page"] == 1
    assert new_spans[0]["bbox"] == [10.0, 20.0, 100.0, 40.0]
    assert new_spans[0]["justified_char_only"] is False
    assert stats["import_eligible"] is False


def test_missing_layout_no_invent() -> None:
    spans = [{"artifact_hash": "h", "char_start": 0, "char_end": 3, "surface": "ABC"}]
    new_spans, stats = upgrade_spans_with_layout_json(spans, None)
    assert stats["layout_present"] is False
    assert new_spans[0].get("page") is None


def test_upgrade_grounded_gold() -> None:
    gold = {
        "entities": [
            {
                "id": "e1",
                "label": "Foo Bar",
                "spans": [
                    {
                        "surface": "Foo Bar",
                        "artifact_hash": "h",
                        "char_start": 0,
                        "char_end": 7,
                        "justified_char_only": True,
                    }
                ],
            }
        ],
        "relations": [],
    }
    layout = {
        "elements": [
            {"text": "Foo Bar method", "page": 2, "bbox": [0, 0, 1, 1]},
        ]
    }
    upgraded, stats = upgrade_grounded_gold_with_layout(gold, layout)
    assert stats["spans_upgraded"] == 1
    assert upgraded["entities"][0]["spans"][0]["page"] == 2
