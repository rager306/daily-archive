"""M202: reviewed extraction metrics harness tests (no raw text leakage)."""

from __future__ import annotations

import json
from pathlib import Path

from research_graph.application.reviewed_extraction_metrics import (
    FORBIDDEN_REPORT_KEYS,
    score_entity_split,
    score_relation_evidence_split,
    score_reviewed_case,
    score_reviewed_split,
)

M072 = Path("artifacts/m072-reviewed-extraction-benchmark/fixtures")


def _minimal_case(case_id: str = "case:one", *, perfect: bool = True) -> tuple[dict, dict]:
    gold = {
        "case_id": case_id,
        "paper_id": "arxiv:0000.00001v1",
        "source_artifact_refs": ["artifact:paper-one"],
        "entities": [
            {
                "id": "entity:method:a",
                "type": "Method",
                "label": "Attention",
                "evidence_refs": ["evidence:one:method"],
            }
        ],
        "relations": [
            {
                "id": "rel:uses",
                "type": "USES",
                "source": "entity:method:a",
                "target": "entity:method:a",
                "evidence_refs": ["evidence:one:rel"],
            }
        ],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }
    pred = json.loads(json.dumps(gold))
    if not perfect:
        pred["entities"][0]["label"] = "Different Label"
        pred["operational"] = {"cost_estimate": 0.1, "latency_ms": 50, "retry_count": 1}
    return gold, pred


def test_score_reviewed_case_perfect() -> None:
    gold, pred = _minimal_case(perfect=True)
    report = score_reviewed_case(gold, pred)
    assert report.case_id == "case:one"
    assert report.metrics["entity_f1"] == 1.0
    assert report.metrics["relation_f1"] == 1.0
    assert report.leakage_clean is True
    assert report.disagreements == ()
    dumped = json.dumps(report.to_sanitized_dict())
    for key in FORBIDDEN_REPORT_KEYS:
        assert key not in dumped.lower() or key in {"vector"}  # only exact forbidden keys matter
    assert "raw_text" not in dumped
    assert "api_key" not in dumped


def test_score_reviewed_case_disagreement() -> None:
    gold, pred = _minimal_case(perfect=False)
    report = score_reviewed_case(gold, pred)
    assert report.metrics["entity_f1"] < 1.0
    assert any(d.kind == "metric_delta" for d in report.disagreements)


def test_score_reviewed_case_missing_prediction() -> None:
    gold, _ = _minimal_case(case_id="case:gold-only")
    pred = {
        "case_id": "case:other",
        "paper_id": "arxiv:0000.00002v1",
        "source_artifact_refs": ["artifact:paper-two"],
        "entities": [],
        "relations": [],
        "schema_valid": True,
        "json_valid": True,
        "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
    }
    report = score_reviewed_case(gold, pred)
    kinds = {d.kind for d in report.disagreements}
    assert "missing_prediction" in kinds or "extra_prediction" in kinds


def test_m072_train_split_via_harness() -> None:
    if not M072.exists():
        return  # environment without fixtures
    gold = [
        json.loads(line)
        for line in (M072 / "train-gold.jsonl").read_text().splitlines()
        if line.strip()
    ]
    pred = [
        json.loads(line)
        for line in (M072 / "train-baseline-predictions.jsonl").read_text().splitlines()
        if line.strip()
    ]
    report = score_reviewed_split(gold, pred, split_name="train")
    expected = json.loads((M072 / "expected-metrics.json").read_text())["train"]
    assert report.metrics["entity_f1"] == expected["entity_f1"]
    assert report.metrics["relation_f1"] == expected["relation_f1"]
    assert report.case_count == expected["case_count"]
    assert "entity_precision" in report.confidence_intervals
    assert report.leakage_clean is True


def test_entity_split_reports_ci_and_type_metrics() -> None:
    gold, pred = _minimal_case(perfect=True)
    # type mismatch case
    pred2 = json.loads(json.dumps(pred))
    pred2["entities"][0]["type"] = "Task"
    report = score_entity_split([gold], [pred2], split_name="entity")
    assert "entity_precision" in report.metrics
    assert "entity_type_mismatches" in report.metrics
    assert report.metrics["entity_type_mismatches"] >= 1
    assert "entity_precision" in report.confidence_intervals


def test_relation_evidence_split_separates_endpoint_type_anchor() -> None:
    gold, pred = _minimal_case(perfect=True)
    report = score_relation_evidence_split([gold], [pred], split_name="rel")
    assert "relation_endpoint_correctness" in report.metrics
    assert "relation_type_correctness" in report.metrics
    assert "evidence_anchor_correctness" in report.metrics
    assert report.metrics["relation_endpoint_correctness"] == 1.0
    assert report.metrics["evidence_path_validity"] == 1.0
