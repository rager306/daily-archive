"""Tests for gold↔body grounding audit."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_gold_body_grounding_audit import (
    GoldBodyGroundingAuditPackage,
    audit_gold_body_grounding,
)


def test_audit_reports_full_coverage_when_grounded() -> None:
    gold = {
        "case_id": "case:x",
        "paper_id": "x",
        "entities": [
            {"id": "e1", "type": "Field", "label": "Language and Perception"},
            {"id": "e2", "type": "Task", "label": "Grounded Attribute Learning"},
        ],
        "relations": [],
    }
    body = (
        "# A Joint Model of Language and Perception for Grounded Attribute Learning\n\n"
        "We study Language and Perception for Grounded Attribute Learning.\n"
    )
    pkg = audit_gold_body_grounding(
        cases=[{"case_id": "case:x", "paper_id": "x", "gold": gold, "body_text": body}]
    )
    assert pkg.candidate_coverage_ratio == 1.0
    assert pkg.body_coverage_ratio == 1.0
    assert pkg.ungrounded == ()
    assert pkg.import_eligible is False


def test_audit_flags_missing_gold() -> None:
    gold = {
        "case_id": "case:y",
        "entities": [
            {"id": "e1", "type": "Method", "label": "Completely Absent Phrase"},
        ],
        "relations": [],
    }
    pkg = audit_gold_body_grounding(
        cases=[
            {
                "case_id": "case:y",
                "paper_id": "y",
                "gold": gold,
                "body_text": "Unrelated paper about cats and dogs.",
            }
        ]
    )
    assert pkg.candidate_coverage_ratio == 0.0
    assert len(pkg.ungrounded) == 1
    assert pkg.ungrounded[0]["label"] == "Completely Absent Phrase"


def test_rejects_import() -> None:
    with pytest.raises(ValueError, match="import"):
        GoldBodyGroundingAuditPackage(
            schema_version="x",
            case_count=0,
            gold_entity_total=0,
            grounded_in_body=0,
            grounded_in_candidates=0,
            body_coverage_ratio=0.0,
            candidate_coverage_ratio=0.0,
            ungrounded=(),
            per_case=(),
            diagnostics=(),
            import_eligible=True,
        )
