"""M276 E1.6: SourceSpan resolvability fail-closed."""

from __future__ import annotations

from research_graph.application.corpus.evidence_resolvability import (
    evaluate_assertion_resolvability,
    evaluate_source_span_resolvability,
    resolvability_rate,
)


def test_page_bbox_resolvable() -> None:
    v = evaluate_source_span_resolvability(
        {
            "artifact_hash": "abc",
            "page": 1,
            "bbox": [0.0, 1.0, 2.0, 3.0],
        }
    )
    assert v.resolvable is True
    assert v.reason == "page_or_bbox"
    assert v.import_eligible is False


def test_missing_hash_fails() -> None:
    v = evaluate_source_span_resolvability({"page": 1, "bbox": [0, 0, 1, 1]})
    assert v.resolvable is False
    assert v.reason == "missing_artifact_hash"
    assert v.import_eligible is False


def test_char_only_fallback() -> None:
    v = evaluate_source_span_resolvability(
        {"artifact_hash": "h", "char_start": 10, "char_end": 20}
    )
    assert v.resolvable is True
    assert v.justified_char_only is True
    assert v.import_eligible is False


def test_char_only_forbidden() -> None:
    v = evaluate_source_span_resolvability(
        {"artifact_hash": "h", "char_start": 10, "char_end": 20},
        allow_char_only_fallback=False,
    )
    assert v.resolvable is False
    assert v.reason == "char_only_forbidden"


def test_assertion_any_span() -> None:
    v = evaluate_assertion_resolvability(
        [
            {"artifact_hash": "x"},  # no coords
            {"artifact_hash": "y", "page": 3},
        ]
    )
    assert v.resolvable is True
    assert v.import_eligible is False


def test_empty_spans_fail() -> None:
    v = evaluate_assertion_resolvability([])
    assert v.resolvable is False
    assert v.reason == "no_spans"


def test_resolvability_rate_batch() -> None:
    rows = [
        {"spans": [{"artifact_hash": "a", "page": 1}]},
        {"spans": [{"artifact_hash": "b", "char_start": 0, "char_end": 5}]},
        {"spans": [{"page": 1}]},  # no hash
    ]
    stats = resolvability_rate(rows)
    assert stats["total_assertions"] == 3
    assert stats["resolvable_count"] == 2
    assert stats["char_only_count"] == 1
    assert abs(stats["resolvability_rate"] - 2 / 3) < 1e-9
    assert stats["import_eligible"] is False
