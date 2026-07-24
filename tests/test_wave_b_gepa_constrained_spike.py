"""Unit tests for Wave B GEPA-shaped constrained spike (no network, no gepa)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.wave_b_gepa_constrained_spike import (
    COMPONENT_ENTITY,
    COMPONENT_RELATION,
    DEFAULT_CANDIDATE,
    WaveBConstrainedGEPAAdapter,
    WaveBGEPASpikePackage,
    instruction_rule_select,
    offline_reflective_spike,
    parse_entity_type_hints,
    propose_instruction_from_reflection,
    try_gepa_optimize,
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


def _case() -> dict:
    gold = _gold()
    body = (
        "We study Language and Perception methods for Grounded Attribute Learning "
        "using probabilistic models."
    )
    return {
        "case_id": gold["case_id"],
        "paper_id": "1206.6423",
        "gold": gold,
        "body_text": body,
    }


def test_parse_type_hints() -> None:
    text = (
        "Select entities\n"
        "TYPE_HINT: Language and Perception -> Field\n"
        "TYPE_HINT: Grounded Attribute Learning -> Task\n"
    )
    hints = parse_entity_type_hints(text)
    assert hints["language and perception"] == "Field"
    assert hints["grounded attribute learning"] == "Task"


def test_instruction_rule_select_uses_hints() -> None:
    body = (
        "Language and Perception for Grounded Attribute Learning works."
    )
    from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
        build_body_candidates,
    )

    cands = build_body_candidates(body)
    sel = instruction_rule_select(
        body,
        "case:x",
        cands,
        entity_instruction=(
            "TYPE_HINT: Language and Perception -> Field\n"
            "TYPE_HINT: Grounded Attribute Learning -> Task\n"
            "SELECT_MAX: 4\n"
        ),
        relation_instruction="RELATION_HINT: Field APPLIED_TO Task\n",
    )
    assert len(sel["entities"]) == 2
    assert len(sel["relations"]) == 1
    assert sel["relations"][0]["type"] == "APPLIED_TO"


def test_adapter_evaluate_and_reflective_dataset() -> None:
    adapter = WaveBConstrainedGEPAAdapter([_case()])
    cov = adapter.coverage_summary()
    assert cov["gold_labels_in_candidates"] == 2
    assert cov["coverage_ratio"] == 1.0

    seed_batch = adapter.evaluate(None, DEFAULT_CANDIDATE, capture_traces=True)
    assert len(seed_batch.scores) == 1
    assert seed_batch.scores[0] == 0.0  # seed has no TYPE_HINT
    assert seed_batch.trajectories is not None
    assert seed_batch.trajectories[0]["missed_but_in_candidates"]

    reflective = adapter.make_reflective_dataset(
        DEFAULT_CANDIDATE,
        seed_batch,
        [COMPONENT_ENTITY, COMPONENT_RELATION],
    )
    assert COMPONENT_ENTITY in reflective
    assert "missed_available" in reflective[COMPONENT_ENTITY][0]["Feedback"]

    proposed = propose_instruction_from_reflection(DEFAULT_CANDIDATE, reflective)
    assert "TYPE_HINT" in proposed[COMPONENT_ENTITY]
    assert "Language and Perception" in proposed[COMPONENT_ENTITY]

    improved = adapter.evaluate(None, proposed, capture_traces=True)
    assert improved.scores[0] == 1.0
    assert improved.objective_scores is not None
    assert improved.objective_scores[0]["relation_f1"] == 1.0


def test_offline_reflective_spike_improves_when_coverage_ok() -> None:
    pkg = offline_reflective_spike(
        cases=[_case()],
        max_iterations=3,
        floor_metrics={"entity_f1": 0.5, "relation_f1": 0.5},
    )
    assert isinstance(pkg, WaveBGEPASpikePackage)
    assert pkg.import_eligible is False
    assert pkg.dspy_optimizer_enabled is False
    assert pkg.llm_used is False
    assert pkg.gepa_ran is False
    assert pkg.seed_metrics["entity_f1"] == 0.0
    assert pkg.best_metrics["entity_f1"] == 1.0
    assert pkg.oracle_ceiling_metrics is not None
    assert pkg.oracle_ceiling_metrics["entity_f1"] == 1.0
    assert any(it.get("accepted") for it in pkg.iterations)
    assert "TYPE_HINT" in pkg.best_candidate[COMPONENT_ENTITY]


def test_package_rejects_import() -> None:
    with pytest.raises(ValueError, match="import"):
        WaveBGEPASpikePackage(
            schema_version="x",
            mode="x",
            case_count=0,
            train_count=0,
            val_count=0,
            seed_metrics={},
            best_metrics={},
            floor_metrics=None,
            seed_candidate={},
            best_candidate={},
            iterations=(),
            reflective_samples=(),
            diagnostics=(),
            import_eligible=True,
        )


def test_try_gepa_optimize_without_package_or_lm() -> None:
    status = try_gepa_optimize(cases=[_case()], reflection_lm=None)
    assert status["ran"] is False
    assert status["import_eligible"] is False
    assert status["reason"] in {
        "gepa_package_not_installed",
        "reflection_lm_not_provided",
    }
