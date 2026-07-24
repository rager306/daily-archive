"""TDD: constrained selectors over body candidates (no free invent)."""

from __future__ import annotations

from research_graph.application.corpus.wave_b_constrained_select import (
    guess_entity_type,
    header_priority_select,
    parse_constrained_llm_selection,
    render_constrained_select_prompt,
)
from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    build_body_candidates,
    score_gold_hybrid_constrained_pilot,
)


def _body_and_gold() -> tuple[str, dict]:
    body = (
        "# A Joint Model of Language and Perception for Grounded Attribute Learning\n\n"
        "We study Language and Perception for Grounded Attribute Learning.\n"
    )
    gold = {
        "case_id": "case:train:1206.6423",
        "paper_id": "arxiv:1206.6423",
        "source_artifact_refs": ["artifact:x"],
        "entities": [
            {
                "id": "e:field",
                "type": "Field",
                "label": "Language and Perception",
                "evidence_refs": ["e1"],
            },
            {
                "id": "e:task",
                "type": "Task",
                "label": "Grounded Attribute Learning",
                "evidence_refs": ["e2"],
            },
        ],
        "relations": [
            {
                "id": "r1",
                "type": "APPLIED_TO",
                "source": "e:field",
                "target": "e:task",
                "evidence_refs": ["r1"],
            }
        ],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }
    return body, gold


def test_guess_entity_type_heuristics() -> None:
    assert guess_entity_type("Language and Perception") == "Field"
    assert guess_entity_type("Grounded Attribute Learning") == "Task"
    assert guess_entity_type("Neural Machine Translation") == "Method"
    assert guess_entity_type("Human Feedback") == "Method"
    assert guess_entity_type("Extractive Summarization") == "Task"
    assert guess_entity_type("Recursively Summarizing Books") == "Task"
    assert guess_entity_type("Recurrent Neural Network") == "Method"


def test_header_priority_select_recovers_title_gold() -> None:
    body, gold = _body_and_gold()
    cands = build_body_candidates(body, paper_id="1206.6423")
    sel = header_priority_select(body, gold["case_id"], cands)
    assert sel["json_valid"] is True
    assert len(sel["entities"]) >= 2
    # map surfaces
    by_id = {c["candidate_id"]: c for c in cands}
    labels = {by_id[e["candidate_id"]]["surface"] for e in sel["entities"]}
    assert "Language and Perception" in labels
    assert "Grounded Attribute Learning" in labels
    assert len(sel["relations"]) >= 1


def test_header_priority_scores_perfect_on_fixture() -> None:
    body, gold = _body_and_gold()
    pkg = score_gold_hybrid_constrained_pilot(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": body,
            },
        ),
        select_fn=header_priority_select,
        llm_used=False,
    )
    assert pkg.metrics["entity_f1"] == 1.0
    assert pkg.metrics["relation_f1"] == 1.0
    assert pkg.import_eligible is False


def test_parse_constrained_llm_selection_drops_unknown_ids() -> None:
    cands = [
        {
            "candidate_id": "c:a:0",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        },
        {
            "candidate_id": "c:b:1",
            "surface": "Grounded Attribute Learning",
            "surface_norm": "grounded attribute learning",
            "source": "header_title",
        },
    ]
    raw = {
        "entities": [
            {"candidate_id": "c:a:0", "type": "Field"},
            {"candidate_id": "c:invented", "type": "Method"},
            {"candidate_id": "c:b:1", "type": "Task"},
        ],
        "relations": [
            {
                "type": "APPLIED_TO",
                "source_id": "c:a:0",
                "target_id": "c:b:1",
            },
            {
                "type": "APPLIED_TO",
                "source_id": "c:a:0",
                "target_id": "c:missing",
            },
        ],
        "json_valid": True,
    }
    sel = parse_constrained_llm_selection(raw, cands)
    assert len(sel["entities"]) == 2
    assert len(sel["relations"]) == 1
    assert all(e["candidate_id"] in {"c:a:0", "c:b:1"} for e in sel["entities"])


def test_render_constrained_prompt_lists_candidates_only() -> None:
    cands = [
        {
            "candidate_id": "c:a:0",
            "surface": "Language and Perception",
            "surface_norm": "language and perception",
            "source": "header_title",
        }
    ]
    prompt = render_constrained_select_prompt(
        case_id="case:x",
        paper_id="x",
        candidates=cands,
        outline_titles=["Abstract", "Method"],
    )
    assert "c:a:0" in prompt
    assert "Language and Perception" in prompt
    assert "candidate_id" in prompt
    assert "Do NOT invent new labels" in prompt or "do not invent" in prompt.casefold()
