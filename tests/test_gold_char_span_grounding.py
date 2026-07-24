"""M281: gold surface → justified char spans from hybrid body."""

from __future__ import annotations

from research_graph.application.corpus.evidence_resolvability import (
    evaluate_assertion_resolvability,
)
from research_graph.application.corpus.gold_char_span_grounding import (
    attach_char_spans_to_gold_case,
    locate_surface_char_span,
    span_dict_for_surface,
)


def test_locate_and_span_resolvable() -> None:
    body = "We introduce Language Games as a Task for agents.\n"
    loc = locate_surface_char_span("Language Games", body)
    assert loc is not None
    start, end = loc
    assert body[start:end].casefold() == "language games"
    span = span_dict_for_surface("Language Games", body)
    assert span is not None
    assert span["artifact_hash"]
    assert span["justified_char_only"] is True
    v = evaluate_assertion_resolvability([span])
    assert v.resolvable is True
    assert v.justified_char_only is True
    assert v.import_eligible is False


def test_missing_surface_no_span() -> None:
    body = "Unrelated abstract without the label."
    assert span_dict_for_surface("Quantum Entanglement Soup", body) is None


def test_attach_entities_and_relations() -> None:
    body = (
        "Introduction. Language Games enable multi-agent learning. "
        "Our Method uses Dialogue Policy for the Task.\n"
    )
    gold = {
        "case_id": "case:x",
        "paper_id": "arxiv:1606.02447",
        "entities": [
            {"id": "e1", "label": "Language Games", "type": "Task"},
            {"id": "e2", "label": "Missing Label XYZ", "type": "Method"},
            {"id": "e3", "label": "Dialogue Policy", "type": "Method"},
        ],
        "relations": [
            {
                "id": "r1",
                "type": "USES",
                "source_label": "Language Games",
                "target_label": "Dialogue Policy",
            }
        ],
    }
    result = attach_char_spans_to_gold_case(
        gold=gold, body_text=body, case_id="case:x", paper_id="1606.02447"
    )
    assert result.import_eligible is False
    assert result.entity_total == 3
    assert result.entity_grounded == 2
    assert result.relation_grounded >= 1
    grounded = [e for e in result.gold["entities"] if e.get("spans")]
    assert len(grounded) == 2
    missing = next(e for e in result.gold["entities"] if e["id"] == "e2")
    assert missing["spans"] == []
    # resolvability on grounded entity
    v = evaluate_assertion_resolvability(grounded[0]["spans"])
    assert v.resolvable is True
