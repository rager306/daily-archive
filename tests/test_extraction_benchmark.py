from __future__ import annotations

import json
from pathlib import Path

import pytest

from research_graph.evaluation.extraction_benchmark import (
    evaluate_files,
    evaluate_records,
    load_jsonl,
)
from research_graph.workflows.universal_kb.queue import UniversalKBQueue

FIXTURE_DIR = Path("artifacts/m071-extraction-benchmark/fixtures")
M072_FIXTURE_DIR = Path("artifacts/m072-reviewed-extraction-benchmark/fixtures")


class FixedClock:
    def __init__(self, value: str = "2026-06-16T00:00:00Z") -> None:
        self.value = value

    def __call__(self) -> str:
        return self.value


def test_smoke_fixture_metrics_match_expected() -> None:
    metrics = evaluate_files(
        FIXTURE_DIR / "smoke-gold.jsonl",
        FIXTURE_DIR / "smoke-predictions.jsonl",
    )
    expected = json.loads((FIXTURE_DIR / "smoke-expected-metrics.json").read_text())

    for key, expected_value in expected.items():
        assert metrics[key] == pytest.approx(expected_value)

    assert metrics["missing_predictions"] == []
    assert metrics["extra_predictions"] == []


def test_m072_reviewed_fixture_metrics_match_expected() -> None:
    expected = json.loads((M072_FIXTURE_DIR / "expected-metrics.json").read_text())

    for split in ("train", "validation"):
        metrics = evaluate_files(
            M072_FIXTURE_DIR / f"{split}-gold.jsonl",
            M072_FIXTURE_DIR / f"{split}-baseline-predictions.jsonl",
        )
        for key, expected_value in expected[split].items():
            if isinstance(expected_value, float):
                assert metrics[key] == pytest.approx(expected_value)
            else:
                assert metrics[key] == expected_value


def test_perfect_records_score_one() -> None:
    gold = [
        {
            "case_id": "case:one",
            "paper_id": "arxiv:0000.00001v1",
            "source_artifact_refs": ["artifact:paper-one"],
            "entities": [
                {
                    "id": "entity:method:a",
                    "type": "Method",
                    "label": "Method A",
                    "evidence_refs": ["evidence:one:method"],
                },
                {
                    "id": "entity:task:b",
                    "type": "Task",
                    "label": "Task B",
                    "evidence_refs": ["evidence:one:task"],
                },
            ],
            "relations": [
                {
                    "id": "relation:a:b",
                    "type": "APPLIED_TO",
                    "source": "entity:method:a",
                    "target": "entity:task:b",
                    "evidence_refs": ["evidence:one:relation"],
                }
            ],
            "schema_valid": True,
            "json_valid": True,
            "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
        }
    ]
    prediction = [
        {
            "case_id": "case:one",
            "paper_id": "arxiv:0000.00001v1",
            "source_artifact_refs": ["artifact:paper-one"],
            "entities": [
                {
                    "id": "pred:method:a",
                    "type": "Method",
                    "label": "method a",
                    "evidence_refs": ["evidence:one:method"],
                },
                {
                    "id": "pred:task:b",
                    "type": "Task",
                    "label": "Task B",
                    "evidence_refs": ["evidence:one:task"],
                },
            ],
            "relations": [
                {
                    "id": "predrel:a:b",
                    "type": "APPLIED_TO",
                    "source": "pred:method:a",
                    "target": "pred:task:b",
                    "evidence_refs": ["evidence:one:relation"],
                }
            ],
            "schema_valid": True,
            "json_valid": True,
            "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
        }
    ]

    metrics = evaluate_records(gold, prediction)

    assert metrics["entity_f1"] == 1.0
    assert metrics["relation_f1"] == 1.0
    assert metrics["evidence_path_validity"] == 1.0
    assert metrics["schema_validity"] == 1.0


def test_invalid_prediction_schema_reduces_validity_without_crashing() -> None:
    gold = load_jsonl(FIXTURE_DIR / "smoke-gold.jsonl")[:1]
    prediction = [
        {
            "case_id": gold[0]["case_id"],
            "paper_id": gold[0]["paper_id"],
            "source_artifact_refs": ["artifact:paper-2606.13669v1"],
            "entities": [
                {
                    "id": "pred:method:agents_k1",
                    "type": "Method",
                    "label": "Agents-K1",
                    "evidence_refs": [],
                }
            ],
            "relations": [
                {
                    "id": "bad:relation",
                    "type": "USES_COMPONENT",
                    "source": "missing",
                    "target": "pred:method:agents_k1",
                    "evidence_refs": [],
                }
            ],
            "schema_valid": True,
            "json_valid": True,
            "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
        }
    ]

    metrics = evaluate_records(gold, prediction)

    assert metrics["schema_validity"] == 0.0
    assert metrics["evidence_path_validity"] == 0.0
    assert metrics["invalid_cases"] == [gold[0]["case_id"]]


def test_gold_fixture_validation_is_strict() -> None:
    with pytest.raises(ValueError, match="gold fixture is invalid"):
        evaluate_records(
            [
                {
                    "case_id": "case:bad-gold",
                    "paper_id": "arxiv:0000.00001v1",
                    "source_artifact_refs": ["not a metadata ref"],
                    "entities": [],
                    "relations": [],
                    "schema_valid": True,
                    "json_valid": True,
                    "operational": {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
                }
            ],
            [],
        )


def test_benchmark_metrics_can_be_stored_in_queue_payload_metadata(tmp_path: Path) -> None:
    metrics = evaluate_files(
        FIXTURE_DIR / "smoke-gold.jsonl",
        FIXTURE_DIR / "smoke-predictions.jsonl",
    )
    queue = UniversalKBQueue(tmp_path / "queue.sqlite", clock=FixedClock()).initialize()
    queue.enqueue(
        job_id="job-benchmark-smoke",
        stage="benchmark",
        input_refs=("artifact:m071-smoke-fixtures",),
        input_hash="sha256:fixtures",
        tool_version="tool:m071_benchmark",
        contract_version="contract:m071_v1",
        payload_metadata={
            "schema_version": "schema:m071_v1",
            "metric_bundle_id": "metric_bundle:m071_v1",
            "extractor_version": "extractor:fixture_baseline_v1",
            "source_artifact_refs": ["artifact:m071-smoke-fixtures"],
        },
    )

    updated = queue.update_payload_diagnostics(
        "job-benchmark-smoke",
        diagnostics={
            "entity_f1": metrics["entity_f1"],
            "relation_f1": metrics["relation_f1"],
            "evidence_path_validity": metrics["evidence_path_validity"],
            "schema_validity": metrics["schema_validity"],
            "json_validity": metrics["json_validity"],
        },
        cost_estimate=metrics["mean_cost_estimate"],
        latency_ms=int(metrics["mean_latency_ms"]),
        retry_count=metrics["total_retry_count"],
        evidence_path_refs=("evidence:m071:smoke",),
    )

    assert updated["payload_metadata"]["diagnostics"]["entity_f1"] == pytest.approx(0.8)
    assert updated["payload_metadata"]["diagnostics"]["relation_f1"] == pytest.approx(0.5)
    assert updated["payload_metadata"]["write_eligibility"] is False
    assert updated["payload_metadata"]["promotion_eligibility"] is False
