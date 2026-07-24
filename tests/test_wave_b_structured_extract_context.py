"""Tests for Wave B structured extract context (no network)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_structured_extract_context import (
    StructuredExtractContext,
    build_structured_extract_context,
    parse_need_sections,
    render_structured_extract_prompt,
)


def _sample_body() -> str:
    return (
        "# A Joint Model of Language and Perception for Grounded Attribute Learning\n\n"
        "## Abstract\n"
        "We present a joint model of Language and Perception for Grounded Attribute "
        "Learning using probabilistic methods.\n\n"
        "## Introduction\n"
        "Grounded Attribute Learning connects language to visual attributes.\n\n"
        "## Method\n"
        "Our approach combines perception features with language supervision.\n\n"
        "## Experiments\n"
        "We evaluate on attribute recognition benchmarks.\n"
    )


def test_build_structured_context_has_outline_sections_candidates() -> None:
    ctx = build_structured_extract_context(
        body_text=_sample_body(),
        paper_id="1206.6423",
        case_id="case:train:1206.6423",
    )
    assert isinstance(ctx, StructuredExtractContext)
    assert ctx.import_eligible is False
    assert ctx.dspy_optimizer_enabled is False
    assert ctx.outline
    assert ctx.section_catalog
    assert ctx.sections
    assert ctx.candidates
    surfaces = {str(c.get("surface") or "").casefold() for c in ctx.candidates}
    assert any("language and perception" in s for s in surfaces)
    assert any("grounded attribute learning" in s for s in surfaces)
    # catalog requestable
    sid = str(ctx.section_catalog[0]["section_id"])
    got = ctx.section_by_id(sid)
    assert got is not None
    assert "text" in got


def test_render_prompt_is_structured_not_raw_only() -> None:
    ctx = build_structured_extract_context(
        body_text=_sample_body(),
        paper_id="x",
        case_id="case:x",
    )
    prompt = render_structured_extract_prompt(
        ctx,
        allowed_entity_types=["Field", "Task", "Method"],
        allowed_relation_types=["APPLIED_TO"],
    )
    assert "--- OUTLINE ---" in prompt
    assert "--- SECTION CATALOG" in prompt
    assert "--- GROUNDED CANDIDATES ---" in prompt
    assert "--- PAPER TEXT ---" not in prompt
    assert "need_sections" in prompt


def test_followup_section_resolution() -> None:
    ctx = build_structured_extract_context(
        body_text=_sample_body(),
        paper_id="x",
        case_id="case:x",
    )
    # request by title fragment present in catalog
    titles = [str(s.get("title") or "") for s in ctx.section_catalog]
    assert titles
    # request first section id
    sid = str(ctx.section_catalog[0]["section_id"])
    resolved = ctx.resolve_followup_sections([sid, "missing-sec"])
    assert len(resolved) == 1
    assert resolved[0]["section_id"] == sid
    assert parse_need_sections({"need_sections": [sid, ""]}) == [sid]


def test_package_rejects_import() -> None:
    with pytest.raises(ValueError, match="import"):
        StructuredExtractContext(
            schema_version="x",
            paper_id="p",
            case_id="c",
            language="en",
            language_confidence=1.0,
            quality_status="ok",
            outline=(),
            section_catalog=(),
            sections=(),
            keywords=(),
            term_dense_windows=(),
            candidates=(),
            body_head="",
            body_chars=0,
            diagnostics=(),
            import_eligible=True,
        )
