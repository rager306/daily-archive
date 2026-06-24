from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from scripts.m052_rlm_e2e import SAFETY_KEYS, run_e2e


def _scrub_generated(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: _scrub_generated(child)
            for key, child in value.items()
            if key not in {"generated_at", "started_at", "completed_at"}
        }
    if isinstance(value, list):
        return [_scrub_generated(child) for child in value]
    return value


def test_e2e_pipeline_runs(tmp_path: Path) -> None:
    audit = run_e2e(tmp_path)

    assert (tmp_path / "audit.json").is_file()
    assert (tmp_path / "audit.md").is_file()
    assert audit["schema_version"] == "m052-s02-rlm-e2e-audit.v1"
    assert audit["trajectory"]["step_count"] == 8


def test_e2e_trajectory_has_navigation_helper_steps(tmp_path: Path) -> None:
    audit = run_e2e(tmp_path)

    assert audit["trajectory"]["step_types"] == [
        "section_navigate",
        "section_navigate",
        "section_navigate",
        "span_visit",
        "span_visit",
        "span_visit",
        "helper_invoke",
        "helper_invoke",
    ]
    assert len(audit["helper_candidate_set"]) == 2
    assert all(candidate["work_id"] for candidate in audit["helper_candidate_set"])


def test_e2e_comparison_result_valid(tmp_path: Path) -> None:
    audit = run_e2e(tmp_path)
    comparison = audit["comparison"]

    assert comparison["question_id"] == "m052-s02-e2e-pageindex"
    assert comparison["rlm_traversal"]["policy_label"] == "rlm_style_deterministic"
    assert comparison["rlm_traversal"]["stop_reason"] in {
        "target_recall_reached",
        "frontier_exhausted",
        "budget_exhausted",
    }
    assert comparison["baseline_count"] == 4
    assert comparison["baseline_labels"] == [
        "vector_only",
        "graph_one_hop",
        "hybrid",
        "heuristic_bfs",
    ]


def test_e2e_evaluation_metrics_present(tmp_path: Path) -> None:
    audit = run_e2e(tmp_path)
    metrics = audit["metrics"]

    assert set(metrics) == {"retrieval_recall", "evidence_path_hit_rate"}
    assert 0.0 <= metrics["retrieval_recall"]["recall"] <= 1.0
    assert 0.0 <= metrics["evidence_path_hit_rate"]["hit_rate"] <= 1.0
    assert metrics["retrieval_recall"]["result_count"] >= 1
    assert metrics["evidence_path_hit_rate"]["result_count"] >= 1


def test_e2e_5_safety_defaults_all_false_in_audit(tmp_path: Path) -> None:
    audit = run_e2e(tmp_path)
    safety = audit["safety_defaults"]

    assert tuple(safety["keys"]) == SAFETY_KEYS
    assert set(safety["values"]) == set(SAFETY_KEYS)
    assert all(value is False for value in safety["values"].values())
    assert safety["all_5_safety_defaults_false"] is True
    assert safety["persistent_graph_writes"] is False
    assert safety["network_endpoint"] == "127.0.0.1 disabled"


def test_e2e_determinism(tmp_path: Path) -> None:
    first = run_e2e(tmp_path / "first")
    second = run_e2e(tmp_path / "second")

    assert json.dumps(_scrub_generated(first), sort_keys=True) == json.dumps(
        _scrub_generated(second), sort_keys=True
    )


def test_e2e_artifacts_do_not_reference_loopback_hostname(tmp_path: Path) -> None:
    run_e2e(tmp_path)
    forbidden = "local" + "host"

    assert forbidden not in (tmp_path / "audit.json").read_text(encoding="utf-8")
    assert forbidden not in (tmp_path / "audit.md").read_text(encoding="utf-8")
