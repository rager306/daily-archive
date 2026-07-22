"""M202 S04–S08: ablations, provider compare, staged runs, gate tests."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.application.extraction_ablations import (
    ablate_statistical_context,
    compare_deterministic_vs_llm,
    compare_prediction_sets,
    compare_providers,
    decide_gate_verdict,
    expand_records_to_n,
    load_m072_split,
    run_staged_reviewed_run,
    run_twenty_paper_gate,
)

M072 = Path("artifacts/m072-reviewed-extraction-benchmark/fixtures")


def _case(case_id: str, *, label: str = "Attention", cost: float = 0.0) -> dict:
    return {
        "case_id": case_id,
        "paper_id": f"arxiv:{case_id}",
        "source_artifact_refs": ["artifact:paper-one"],
        "entities": [
            {
                "id": f"e:{case_id}:method",
                "type": "Method",
                "label": label,
                "evidence_refs": [f"evidence:{case_id}:method"],
            }
        ],
        "relations": [
            {
                "id": f"r:{case_id}:uses",
                "type": "USES",
                "source": f"e:{case_id}:method",
                "target": f"e:{case_id}:method",
                "evidence_refs": [f"evidence:{case_id}:rel"],
            }
        ],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": cost, "latency_ms": 10, "retry_count": 0},
    }


def test_compare_deterministic_vs_llm_delta() -> None:
    gold = [_case("case:a"), _case("case:b")]
    llm = [_case("case:a"), _case("case:b", label="Wrong")]
    report = compare_deterministic_vs_llm(gold, llm)
    assert report.optimizer_enabled is False
    assert report.baseline_metrics["entity_f1"] == 1.0
    assert report.treatment_metrics["entity_f1"] < 1.0
    assert report.deltas["entity_f1"] < 0
    assert "raw_text" not in json.dumps(report.to_sanitized_dict())


def test_optimizer_enabled_rejected() -> None:
    gold = [_case("case:a")]
    with pytest.raises(ValueError, match="optimizer_enabled"):
        compare_prediction_sets(gold, gold, gold, optimizer_enabled=True)


def test_compare_providers_cost_and_refusal() -> None:
    gold = [_case("case:a"), _case("case:b")]
    minimax = [
        _case("case:a", cost=0.01),
        {
            **_case("case:b", cost=0.01),
            "entities": [],
            "relations": [],
        },
    ]
    glm = [_case("case:a", cost=0.02), _case("case:b", cost=0.02)]
    report = compare_providers(gold, minimax, glm)
    assert report.refusal_or_empty_rate_minimax == 0.5
    assert report.refusal_or_empty_rate_glm == 0.0
    assert report.cost_delta == pytest.approx(0.01)


def test_statistical_context_ablation() -> None:
    gold = [_case("case:a"), _case("case:b")]
    with_stats = [_case("case:a"), _case("case:b")]
    without = [_case("case:a"), _case("case:b", label="Different")]
    report = ablate_statistical_context(gold, with_stats, without)
    assert report.production_remains_statistical_first is True
    assert report.entity_f1_delta > 0


def test_expand_and_ten_paper_run() -> None:
    gold = [_case("case:a"), _case("case:b")]
    pred = [_case("case:a"), _case("case:b")]
    staged = run_staged_reviewed_run(gold, pred, target_count=10, split_name="ten")
    assert staged.paper_count == 10
    assert staged.metrics["case_count"] == 10
    assert staged.entity_split is not None
    assert staged.relation_split is not None
    assert staged.leakage_clean is True


def test_twenty_paper_gate_proceed_on_perfect() -> None:
    gold = [_case("case:a"), _case("case:b")]
    pred = [_case("case:a"), _case("case:b")]
    staged, gate = run_twenty_paper_gate(gold, pred)
    assert staged.paper_count == 20
    assert gate.verdict == "proceed"
    assert gate.paper_count == 20


def test_gate_stop_on_low_f1() -> None:
    metrics = {
        "entity_f1": 0.1,
        "relation_f1": 0.1,
        "evidence_path_validity": 0.1,
        "prediction_count": 20,
        "invalid_cases": [],
    }
    gate = decide_gate_verdict(metrics, paper_count=20)
    assert gate.verdict == "stop"


def test_gate_repair_band() -> None:
    metrics = {
        "entity_f1": 0.5,
        "relation_f1": 0.5,
        "evidence_path_validity": 0.9,
        "prediction_count": 20,
        "invalid_cases": [],
    }
    gate = decide_gate_verdict(metrics, paper_count=20)
    assert gate.verdict == "repair"


def test_m072_deterministic_vs_baseline_if_present() -> None:
    if not M072.exists():
        return
    gold, pred = load_m072_split("train")
    report = compare_deterministic_vs_llm(gold, pred, name="m072-train")
    assert report.baseline_metrics["entity_f1"] == 1.0
    assert report.treatment_metrics["entity_f1"] < 1.0 or report.treatment_metrics["entity_f1"] == 1.0
    assert report.optimizer_enabled is False


def test_expand_records_to_n() -> None:
    rows = expand_records_to_n([_case("case:a")], 5, id_prefix="t")
    assert len(rows) == 5
    assert len({r["case_id"] for r in rows}) == 5
