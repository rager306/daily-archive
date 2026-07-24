"""M280: canary resolvability rate metric."""

from __future__ import annotations

from research_graph.application.corpus.canary_resolvability_metric import (
    evaluate_canary_resolvability,
    expand_gold_rows_to_assertions,
)


def test_rate_with_page_bbox_and_missing() -> None:
    rows = [
        {
            "case_id": "a",
            "spans": [{"artifact_hash": "h", "page": 1, "bbox": [0, 0, 1, 1]}],
        },
        {
            "case_id": "b",
            "spans": [{"artifact_hash": "h", "char_start": 0, "char_end": 10}],
        },
        {"case_id": "c", "spans": [{"page": 1}]},  # no hash
        {"case_id": "d", "spans": []},
    ]
    pkg = evaluate_canary_resolvability(rows, target_rate=0.5, expand_gold=False)
    assert pkg.total_rows == 4
    assert pkg.resolvable_count == 2
    assert pkg.char_only_count == 1
    assert abs(pkg.resolvability_rate - 0.5) < 1e-9
    assert pkg.target_met is True
    assert pkg.import_eligible is False


def test_expand_gold_entities_relations() -> None:
    cases = [
        {
            "case_id": "case:x",
            "paper_id": "x",
            "entities": [
                {
                    "id": "e1",
                    "label": "Method",
                    "spans": [{"artifact_hash": "a", "page": 2}],
                },
                {"id": "e2", "label": "Task"},  # no span
            ],
            "relations": [
                {
                    "type": "USES",
                    "spans": [
                        {
                            "artifact_hash": "a",
                            "char_start": 1,
                            "char_end": 5,
                        }
                    ],
                }
            ],
        }
    ]
    flat = expand_gold_rows_to_assertions(cases)
    assert len(flat) == 3
    # e2 has no spans and must not inherit sibling entity spans
    e2 = next(r for r in flat if r.get("id") == "e2")
    assert e2["spans"] == []
    pkg = evaluate_canary_resolvability(cases, target_rate=0.95)
    assert pkg.total_rows == 3
    assert pkg.resolvable_count == 2
    assert pkg.target_met is False
    assert pkg.import_eligible is False


def test_empty_rows() -> None:
    pkg = evaluate_canary_resolvability([], target_rate=0.95)
    assert pkg.total_rows == 0
    assert pkg.target_met is False
