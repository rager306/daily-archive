"""Unit tests for Wave B constrained candidate-select pilot (no network)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    GoldHybridConstrainedPilotPackage,
    build_body_candidates,
    build_constrained_prediction_record,
    score_gold_hybrid_constrained_pilot,
    surface_in_body,
)


def _gold() -> dict:
    return {
        "case_id": "case:train:1206.6423",
        "paper_id": "arxiv:1206.6423",
        "source_artifact_refs": ["artifact:catalog-arxiv-cs-cl-1206.6423"],
        "entities": [
            {
                "id": "e:field",
                "type": "Field",
                "label": "Language and Perception",
                "evidence_refs": ["evidence:fixture:field"],
            },
            {
                "id": "e:task",
                "type": "Task",
                "label": "Grounded Attribute Learning",
                "evidence_refs": ["evidence:fixture:task"],
            },
        ],
        "relations": [
            {
                "id": "r1",
                "type": "APPLIED_TO",
                "source": "e:field",
                "target": "e:task",
                "evidence_refs": ["evidence:fixture:rel"],
            }
        ],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }


def test_surface_in_body_and_candidates_include_multiword() -> None:
    body = (
        "We study Language and Perception methods for Grounded Attribute Learning "
        "using probabilistic models."
    )
    assert surface_in_body("Language and Perception", body)
    assert not surface_in_body("Completely Missing Phrase", body)
    cands = build_body_candidates(body, paper_id="1206.6423")
    surfaces = {c["surface"] for c in cands}
    assert "Language and Perception" in surfaces
    assert "Grounded Attribute Learning" in surfaces
    assert all(c["candidate_id"].startswith("c:") for c in cands)


def test_header_allcaps_and_single_token_candidates() -> None:
    body = (
        "## TRAIN SHORT, TEST LONG: ATTENTION WITH LINEAR BIASES ENABLES "
        "INPUT LENGTH EXTRAPOLATION\n\n"
        "## Learning Language Games through Interaction\n\n"
        "Abstract body continues with methods and experiments."
    )
    cands = build_body_candidates(body, paper_id="header-demo")
    norms = {c["surface_norm"] for c in cands}
    assert "attention with linear biases" in norms
    assert "input length extrapolation" in norms
    assert "language games" in norms
    assert "interaction" in norms
    sources = {
        c["source"]
        for c in cands
        if c["surface_norm"]
        in {"attention with linear biases", "language games", "interaction"}
    }
    assert "header_title" in sources


def test_grounding_drops_ungrounded_and_maps_selection() -> None:
    body = "Language and Perception for Grounded Attribute Learning works."
    cands = build_body_candidates(body)
    # inject a fake ungrounded candidate
    cands.append(
        {
            "candidate_id": "c:fake:99",
            "surface": "Invented Hallucination",
            "surface_norm": "invented hallucination",
            "source": "test",
        }
    )
    field = next(c for c in cands if c["surface"] == "Language and Perception")
    task = next(c for c in cands if c["surface"] == "Grounded Attribute Learning")
    pred = build_constrained_prediction_record(
        case_id="case:x",
        paper_id="x",
        body_text=body,
        candidates=cands,
        selection={
            "json_valid": True,
            "entities": [
                {"candidate_id": field["candidate_id"], "type": "Field"},
                {"candidate_id": task["candidate_id"], "type": "Task"},
                {"candidate_id": "c:fake:99", "type": "Method"},
            ],
            "relations": [
                {
                    "type": "APPLIED_TO",
                    "source_id": field["candidate_id"],
                    "target_id": task["candidate_id"],
                }
            ],
        },
    )
    labels = {e["label"] for e in pred["entities"]}
    assert "Language and Perception" in labels
    assert "Grounded Attribute Learning" in labels
    assert "Invented Hallucination" not in labels
    assert len(pred["relations"]) == 1
    assert pred["relations"][0]["type"] == "APPLIED_TO"


def test_lexical_oracle_ceiling_near_perfect() -> None:
    gold = _gold()
    body = (
        "Language and Perception research enables Grounded Attribute Learning "
        "in joint models."
    )
    pkg = score_gold_hybrid_constrained_pilot(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": body,
            },
        ),
        use_lexical_oracle=True,
        floor_metrics={"entity_f1": 0.5, "relation_f1": 0.5},
        llm_used=False,
    )
    assert isinstance(pkg, GoldHybridConstrainedPilotPackage)
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.llm_used is False
    assert pkg.mode == "lexical_oracle_diagnostic"
    assert pkg.metrics["entity_f1"] == 1.0
    assert pkg.metrics["relation_f1"] == 1.0
    assert pkg.per_case[0]["gold_label_coverage"] == 2


def test_select_fn_path_scores() -> None:
    gold = _gold()
    body = "Language and Perception and Grounded Attribute Learning."

    def select(_body: str, _case_id: str, candidates):
        by = {c["surface"]: c for c in candidates}
        return {
            "json_valid": True,
            "entities": [
                {
                    "candidate_id": by["Language and Perception"]["candidate_id"],
                    "type": "Field",
                },
                {
                    "candidate_id": by["Grounded Attribute Learning"]["candidate_id"],
                    "type": "Task",
                },
            ],
            "relations": [
                {
                    "type": "APPLIED_TO",
                    "source_id": by["Language and Perception"]["candidate_id"],
                    "target_id": by["Grounded Attribute Learning"]["candidate_id"],
                }
            ],
        }

    pkg = score_gold_hybrid_constrained_pilot(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": body,
            },
        ),
        select_fn=select,
        llm_used=False,
    )
    assert pkg.metrics["entity_f1"] == 1.0
    assert pkg.metrics["relation_f1"] == 1.0


def test_requires_select_or_oracle() -> None:
    with pytest.raises(ValueError, match="select_fn"):
        score_gold_hybrid_constrained_pilot(cases=())


def test_package_rejects_import() -> None:
    with pytest.raises(ValueError, match="import"):
        GoldHybridConstrainedPilotPackage(
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


def test_per_case_records_fallback_used() -> None:
    """Selection fallback_used must surface in per_case observability."""
    from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
        score_gold_hybrid_constrained_pilot,
    )

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
        "relations": [],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }

    def empty_then_marked(body_text, case_id, candidates):
        return {"entities": [], "relations": [], "json_valid": False, "fallback_used": True}

    pkg = score_gold_hybrid_constrained_pilot(
        cases=(
            {
                "case_id": gold["case_id"],
                "paper_id": "1206.6423",
                "gold": gold,
                "body_text": body,
            },
        ),
        select_fn=empty_then_marked,
        llm_used=True,
    )
    assert pkg.per_case
    assert pkg.per_case[0].get("fallback_used") is True



def test_candidates_include_acronyms_and_alnum_tech_terms() -> None:
    """Gold labels like GEPA and Seq2Seq Models must appear as candidates (M268 debt)."""
    body = (
        "## GEPA: REFLECTIVE PROMPT EVOLUTION CAN OUTPERFORM REINFORCEMENT LEARNING\n\n"
        "# Leveraging Graph Structure in Seq2Seq Models for Knowledge Graph Link Prediction\n\n"
        "We introduce GEPA as a prompt optimizer and Seq2Seq Models for link prediction."
    )
    cands = build_body_candidates(body, paper_id="acronym-demo", max_total=96)
    norms = {c["surface_norm"] for c in cands}
    surfaces = {c["surface"] for c in cands}
    assert "gepa" in norms, norms
    assert any(s == "GEPA" or s.casefold() == "gepa" for s in surfaces)
    assert "seq2seq models" in norms, norms
