from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

import pytest

from arxiv_archive.rlm_workflow import (
    REDUCER_SCHEMA_VERSION,
    WorkflowTrajectory,
    WorkflowTrajectoryStep,
    run_document_workflow,
)

_FIXTURE_DIR = Path(__file__).parents[1] / "tests" / "fixtures" / "article_artifacts"
_FIXTURE_STRUCTURE = json.loads(
    (_FIXTURE_DIR / "basic_article_structure.json").read_text(encoding="utf-8")
)
_SAFETY_KEYS = {
    "graph_import_allowed",
    "graphdb_written",
    "ladybugdb_written",
    "production_import_attempted",
    "import_eligible",
}


def _structure(paper_id: str = "m052-fixture-paper") -> dict[str, Any]:
    structure = copy.deepcopy(_FIXTURE_STRUCTURE)
    structure["paper_id"] = paper_id
    return structure


def _minimal_structure(paper_id: str = "m052-minimal-paper") -> dict[str, Any]:
    structure = _structure(paper_id)
    root = structure["sections"][0]
    structure["sections"] = [root]
    structure["artifact_placeholders"] = []
    structure["safe_spans"] = [span for span in structure["safe_spans"] if span["span_id"] == root["span_id"]]
    return structure


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


def _assert_safety_block_all_false(block: dict[str, Any]) -> None:
    assert set(block) == _SAFETY_KEYS
    assert all(value is False for value in block.values())


def test_workflow_trajectory_step_construction() -> None:
    step = WorkflowTrajectoryStep(
        step_type="section_navigate",
        section_id="paper:section:intro",
        diagnostics={"ordinal_path": [1]},
        run_id="run-step-construction",
        step_index=2,
    )

    assert step.step_type == "section_navigate"
    assert step.section_id == "paper:section:intro"
    assert step.span_id is None
    assert step.work_id is None
    assert len(step.step_id) == 16
    assert step.started_at == "2000-01-01T00:00:02+00:00"
    assert step.completed_at == "2000-01-01T00:00:02.001000+00:00"
    _assert_safety_block_all_false(step.safety_defaults)


def test_workflow_trajectory_step_sanitized_dict() -> None:
    step = WorkflowTrajectoryStep(
        step_type="span_visit",
        span_id="paper:span:0001",
        diagnostics={"source": "paragraph"},
        run_id="run-step-sanitized",
    )

    sanitized = step.to_sanitized_dict()

    assert sanitized["step_type"] == "span_visit"
    assert sanitized["span_id"] == "paper:span:0001"
    assert sanitized["section_id"] is None
    assert sanitized["work_id"] is None
    assert sanitized["diagnostics"] == {"source": "paragraph"}
    _assert_safety_block_all_false(sanitized["safety_defaults"])


def test_workflow_trajectory_aggregate_safety_defaults() -> None:
    step = WorkflowTrajectoryStep(step_type="helper_invoke", work_id="wid-1", run_id="run-trajectory")
    trajectory = WorkflowTrajectory(run_id="run-trajectory", work_ids=("wid-1",), steps=(step,))

    assert trajectory.schema_version == REDUCER_SCHEMA_VERSION
    _assert_safety_block_all_false(trajectory.aggregate_safety_defaults)
    sanitized = trajectory.to_sanitized_dict()
    _assert_safety_block_all_false(sanitized["aggregate_safety_defaults"])


def test_run_document_workflow_minimal_structure() -> None:
    result = run_document_workflow(
        _minimal_structure(),
        page_index={},
        chunks=[],
        evidence_paths=[],
        run_id="run-minimal",
    )

    assert result.trajectory.schema_version == REDUCER_SCHEMA_VERSION
    assert [step.step_type for step in result.trajectory.steps] == ["section_navigate", "span_visit"]
    assert result.trajectory.work_ids == ()
    assert result.aggregate_summary["total_unique_work_ids"] == 0
    assert result.safety_audit["import_authority"] == "import is not authorized"


def test_run_document_workflow_with_real_fixture() -> None:
    result = run_document_workflow(
        _structure("m052-real-fixture"),
        page_index={"pages": []},
        chunks=[],
        evidence_paths=[],
        run_id="run-real-fixture",
    )

    step_types = [step.step_type for step in result.trajectory.steps]
    assert step_types.count("section_navigate") == 3
    assert step_types.count("span_visit") == 3
    assert step_types.count("helper_invoke") == 2
    assert result.aggregate_summary["total_unique_work_ids"] == 2
    assert result.aggregate_summary["work_ids"] == sorted(result.trajectory.work_ids)


def test_workflow_determinism_byte_identical() -> None:
    structure = _structure("m052-deterministic")
    first = run_document_workflow(structure, {}, [], [], run_id="run-deterministic")
    second = run_document_workflow(copy.deepcopy(structure), {}, [], [], run_id="run-deterministic")

    first_json = json.dumps(_scrub_generated(first.to_sanitized_dict()), sort_keys=True)
    second_json = json.dumps(_scrub_generated(second.to_sanitized_dict()), sort_keys=True)

    assert first_json == second_json


@pytest.mark.parametrize("max_steps", [1, 4, 7])
def test_workflow_max_steps_enforcement(max_steps: int) -> None:
    result = run_document_workflow(
        _structure(f"m052-max-steps-{max_steps}"),
        {},
        [],
        [],
        run_id=f"run-max-steps-{max_steps}",
        max_steps=max_steps,
    )

    assert len(result.trajectory.steps) <= max_steps


def test_workflow_helper_invoke_carries_m050_work_id() -> None:
    result = run_document_workflow(
        _structure("m052-helper-work-id"),
        {},
        [],
        [],
        run_id="run-helper-work-id",
    )

    helper_work_ids = tuple(step.work_id for step in result.trajectory.steps if step.step_type == "helper_invoke")
    assert helper_work_ids
    assert helper_work_ids == result.trajectory.work_ids
    assert set(helper_work_ids) == set(result.aggregate_summary["work_ids"])
    assert all(isinstance(work_id, str) and len(work_id) == 64 for work_id in helper_work_ids)


def test_workflow_safety_defaults_all_false() -> None:
    result = run_document_workflow(
        _structure("m052-safety"),
        {},
        [],
        [],
        run_id="run-safety",
    )

    _assert_safety_block_all_false(result.trajectory.aggregate_safety_defaults)
    assert result.safety_audit["all_step_safety_defaults_false"] is True
    assert result.safety_audit["all_reducer_safety_defaults_false"] is True
    for step in result.trajectory.steps:
        _assert_safety_block_all_false(step.safety_defaults)


def test_workflow_schema_version() -> None:
    result = run_document_workflow(
        _minimal_structure("m052-schema"),
        {},
        [],
        [],
        run_id="run-schema",
    )

    assert REDUCER_SCHEMA_VERSION == "m052-rlm-workflow.v1"
    assert result.trajectory.schema_version == REDUCER_SCHEMA_VERSION


def test_workflow_never_imports_urllib() -> None:
    source = Path("src/arxiv_archive/rlm_workflow.py").read_text(encoding="utf-8")

    assert "urllib" not in source


def test_workflow_step_types_in_order() -> None:
    result = run_document_workflow(
        _structure("m052-step-order"),
        {},
        [],
        [],
        run_id="run-step-order",
    )
    step_types = [step.step_type for step in result.trajectory.steps]

    first_span = step_types.index("span_visit")
    first_helper = step_types.index("helper_invoke")
    assert all(step_type == "section_navigate" for step_type in step_types[:first_span])
    assert all(step_type == "span_visit" for step_type in step_types[first_span:first_helper])
    assert all(step_type == "helper_invoke" for step_type in step_types[first_helper:])


def test_workflow_step_fields_are_mutually_exclusive() -> None:
    with pytest.raises(ValueError, match="exactly one"):
        WorkflowTrajectoryStep(
            step_type="section_navigate",
            section_id="paper:section:intro",
            span_id="paper:span:intro",
            run_id="run-invalid",
        )
