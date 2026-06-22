from __future__ import annotations

import json
import subprocess
from pathlib import Path

from scripts.verify_m023_artifact_scaffold_gate import (
    FINAL_GATE_SCHEMA_VERSION,
    build_artifact_scaffold_gate,
    render_artifact_scaffold_review_markdown,
    verify_artifact_scaffold_gate,
)


def test_builds_redacted_final_scaffold_gate_from_fixtures() -> None:
    gate = build_artifact_scaffold_gate()

    assert gate["schema_version"] == FINAL_GATE_SCHEMA_VERSION
    assert gate["artifact_schema_version"] == "m023-article-artifacts.v1"
    assert gate["manifest_summary"]["artifact_count"] == 5
    assert gate["manifest_summary"]["candidate_link_count"] == 6
    assert gate["manifest_summary"]["artifact_counts_by_type"] == {
        "equation": 1,
        "figure": 1,
        "reference": 1,
        "section": 2,
    }
    assert gate["manifest_summary"]["candidate_link_type_counts"] == {
        "cites": 1,
        "contains": 3,
        "located_in": 1,
        "supports": 1,
    }
    assert gate["minimax_helper_status"]["validation_status"] == "valid"
    assert gate["minimax_helper_status"]["raw_prompt_persisted"] is False
    assert gate["minimax_helper_status"]["raw_response_persisted"] is False
    assert gate["minimax_helper_status"]["minimax_source_of_truth"] is False
    assert gate["benchmark_status"]["run_count"] == 2
    assert set(gate["benchmark_status"]["runs"]) == {"deterministic", "minimax_mock"}
    assert gate["dspy_readiness"]["status"] == "blocked"
    assert gate["dspy_readiness"]["optimization_ran"] is False
    assert gate["blocked_operation_flags"]["production_import_attempted"] is False
    assert gate["blocked_operation_flags"]["ladybugdb_written"] is False
    assert gate["blocked_operation_flags"]["trusted_kg_import_allowed"] is False
    assert gate["strict_validation"]["passed"] is True
    assert verify_artifact_scaffold_gate(gate) == []

    serialized = json.dumps(gate)
    for forbidden in (
        "raw paper text",
        '"text":',
        '"caption_text":',
        '"raw_model_output":',
        '"raw_minimax_response":',
        '"embedding":',
        '"vector":',
        '"secret":',
        '"trusted_kg_import_allowed": true',
        '"ladybugdb_written": true',
        '"production_import_attempted": true',
        '"model_outputs_included": true',
    ):
        assert forbidden not in serialized


def test_gate_records_unsafe_counters_without_authorizing_imports() -> None:
    gate = build_artifact_scaffold_gate()
    counters = gate["unsafe_counters"]

    assert counters["manifest_raw_leakage_count"] == 0
    assert counters["manifest_unsafe_authorization_count"] == 0
    assert counters["helper_raw_response_persisted_count"] == 0
    assert counters["helper_source_of_truth_count"] == 0
    assert counters["benchmark_all_runs_raw_leakage_count"] == 2
    assert counters["benchmark_all_runs_unsafe_authorization_count"] >= 4
    assert counters["production_import_attempted_count"] == 0
    assert counters["ladybugdb_written_count"] == 0
    assert counters["trusted_kg_import_allowed_count"] == 0
    assert counters["import_eligible_count"] == 0
    assert counters["promoted_to_fact_count"] == 0
    assert "all_runs_raw_leakage_count" in gate["dspy_readiness"]["blockers"]
    assert "all_runs_unsafe_authorization_count" in gate["dspy_readiness"]["blockers"]


def test_review_markdown_explains_scaffold_boundaries() -> None:
    gate = build_artifact_scaffold_gate()
    markdown = render_artifact_scaffold_review_markdown(gate)

    assert markdown.startswith("# M023 Article Artifact Scaffold Gate Review")
    assert "## Artifact Types" in markdown
    assert "## Candidate Link Semantics" in markdown
    assert "## Review States" in markdown
    assert "## MiniMax Helper Status" in markdown
    assert "## Benchmark and DSPy Readiness" in markdown
    assert "## No-Import Boundaries" in markdown
    assert "## Recommended Next Milestone" in markdown
    assert "review-only candidates" in markdown
    assert "not trusted facts" in markdown
    assert "DSPy was not imported or executed" in markdown
    assert "ontology and KG-design milestone" in markdown
    assert "raw paper text" not in markdown.lower()


def test_cli_writes_gate_json_and_markdown(tmp_path: Path) -> None:
    output_json = tmp_path / "m023-artifact-scaffold-gate.json"
    output_markdown = tmp_path / "m023-artifact-scaffold-review.md"

    result = subprocess.run(
        [
            "uv",
            "run",
            "python",
            "scripts/verify_m023_artifact_scaffold_gate.py",
            "--output-json",
            str(output_json),
            "--output-markdown",
            str(output_markdown),
            "--strict",
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    response = json.loads(result.stdout)
    assert response["status"] == "passed"
    assert output_json.exists()
    assert output_markdown.exists()
    payload = json.loads(output_json.read_text(encoding="utf-8"))
    assert payload["schema_version"] == FINAL_GATE_SCHEMA_VERSION
    assert payload["strict_validation"]["passed"] is True
    assert output_markdown.read_text(encoding="utf-8").startswith(
        "# M023 Article Artifact Scaffold Gate Review"
    )
