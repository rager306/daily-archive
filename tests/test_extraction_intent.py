"""M278 E3.1: ExtractionIntent + negative constraints."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.extraction_intent_validate import (
    candidate_relation_allowed,
    validate_extraction_intent,
)
from research_graph.domain.extraction_intent import (
    SCHEMA_VERSION,
    build_default_constraint_registry,
    build_extraction_intent,
)
from research_graph.domain.relation_types import CAUSES, CITES


def test_default_registry_has_core_constraints() -> None:
    reg = build_default_constraint_registry()
    assert reg.import_eligible is False
    ids = {c.constraint_id for c in reg.constraints}
    assert "no_free_invent_relation" in ids
    assert "no_gold_in_llm_gepa_context" in ids
    assert "no_import_without_user_go" in ids


def test_valid_intent() -> None:
    intent = build_extraction_intent(
        paper_id="p1",
        intent_id="i1",
        target_entity_types=("method", "dataset"),
        target_relation_types=(CAUSES, CITES),
    )
    assert intent.schema_version == SCHEMA_VERSION
    v = validate_extraction_intent(intent)
    assert v.ok is True
    assert v.import_eligible is False
    assert len(v.intent_hash) == 64
    assert len(v.constraints_hash) == 64


def test_free_invent_relation_blocked() -> None:
    intent = build_extraction_intent(
        paper_id="p1",
        intent_id="i2",
        target_entity_types=("method",),
        target_relation_types=("OUTPERFORMS", "CAUSES"),  # OUTPERFORMS not in 27
    )
    v = validate_extraction_intent(intent)
    assert v.ok is False
    assert any("free_invent_relation:OUTPERFORMS" in x for x in v.violations)


def test_candidate_outside_intent() -> None:
    intent = build_extraction_intent(
        paper_id="p",
        intent_id="i",
        target_entity_types=(),
        target_relation_types=(CITES,),
    )
    ok, reason = candidate_relation_allowed(CAUSES, intent)
    assert ok is False
    assert reason == "outside_intent_targets"
    ok2, _ = candidate_relation_allowed(CITES, intent)
    assert ok2 is True


def test_intent_rejects_import_flag() -> None:
    with pytest.raises(ValueError, match="import"):
        build_extraction_intent(
            paper_id="p",
            intent_id="i",
            target_entity_types=(),
            target_relation_types=(CITES,),
        ).__class__(
            schema_version=SCHEMA_VERSION,
            paper_id="p",
            intent_id="i",
            target_entity_types=(),
            target_relation_types=(CITES,),
            allowed_sections=(),
            forbid_free_invent=True,
            require_source_span=True,
            negative_constraint_ids=(),
            import_eligible=True,
        )
