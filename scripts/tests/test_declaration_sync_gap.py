"""Declaration synchronisation gap axis: gap count, named gaps, negative cases.

The axis exists because the DATA and the code can disagree silently: a policy
declares a hard metric no code computes, a stage lists an invariant nothing
checks, or a prompt registry binds a task with no task entry. These tests pin
the three predicates with inline fixtures, so they never depend on the real
tree's current membership, and one asserts the regression direction directly:
adding a declared hard metric without an implementation adds exactly one gap.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from governor.adapters.declaration_sync_gap import (
    HARD_METRIC_WITHOUT_COMPUTATION,
    PROMPT_FORM_WITHOUT_REGISTRY_ENTRY,
    STAGE_INVARIANT_WITHOUT_CHECK,
    build_declaration_sync_gap,
    observe_declaration_sync_gap,
)

_METRIC_PATH = "scripts/governor/checks/article_context_assurance.yaml"
_STAGE_PATH = "data/ontology/pipeline_stage_contracts.yaml"
_PROMPT_PATH = "data/ontology/prompt_tasks.yaml"


# ── fixtures ─────────────────────────────────────────────────────────────────


def _write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _write_metrics(root: Path, body: str) -> None:
    _write(root, _METRIC_PATH, f"metric_definitions:\n{body}")


def _write_stages(root: Path, invariants: list[str]) -> None:
    items = "\n".join(f"    - {name}" for name in invariants)
    _write(
        root,
        _STAGE_PATH,
        "schema_version: pipeline-stage-contracts.v1\n"
        "stages:\n"
        "  - id: materialize\n"
        "    hard_invariants:\n"
        f"{items}\n",
    )


def _write_prompt_registry(
    root: Path, task_ids: list[str], binding_keys: list[str]
) -> None:
    tasks = "\n".join(f"  - task_id: {task_id}" for task_id in task_ids) or "  []"
    bindings = "\n".join(f"    {key}: [PR-001]" for key in binding_keys) or "    {}"
    _write(
        root,
        _PROMPT_PATH,
        "schema_version: prompt-task-registry.v1\n"
        "prompt_contract:\n"
        "  task_rule_bindings:\n"
        f"{bindings}\n"
        "tasks:\n"
        f"{tasks}\n",
    )


def _base(root: Path, metrics: str = "  a_hard: {kind: hard}\n") -> None:
    """A minimal valid declaration tree with an (empty) crates/ directory."""
    (root / "crates").mkdir(parents=True, exist_ok=True)
    _write_metrics(root, metrics)
    _write_stages(root, [])
    _write_prompt_registry(root, [], [])


def _gap(root: Path):
    return build_declaration_sync_gap(root)


def _names(axis, gap_class: str) -> set[str]:
    return {g.name for g in axis.gaps_for(gap_class)}


# ── class A: hard metric without a production computation ────────────────────


def test_hard_metric_computed_in_production_is_not_a_gap(tmp_path: Path):
    _base(tmp_path, metrics="  retained_required_fields: {kind: hard}\n")
    _write(
        tmp_path,
        "crates/da-application/src/report.rs",
        "fn compute() { let _ = \"retained_required_fields\"; }\n",
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION) == ()
    assert axis.declared_for(HARD_METRIC_WITHOUT_COMPUTATION) == 1


def test_hard_metric_without_production_computation_is_named(tmp_path: Path):
    _base(tmp_path, metrics="  checksum_lineage_coverage: {kind: hard}\n")
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION)
    assert len(gaps) == 1
    assert gaps[0].name == "checksum_lineage_coverage"
    assert gaps[0].declared_in == _METRIC_PATH
    assert "no code computes it" in gaps[0].detail


def test_diagnostic_metric_is_outside_the_axis(tmp_path: Path):
    _base(tmp_path, metrics="  coverage: {kind: diagnostic}\n")
    axis = _gap(tmp_path)
    assert axis.declared_for(HARD_METRIC_WITHOUT_COMPUTATION) == 0
    assert axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION) == ()


def test_adding_a_hard_metric_without_implementation_increases_count_by_one(
    tmp_path: Path,
):
    """The regression direction: a new declared hard metric is exactly +1 gap."""
    _base(tmp_path, metrics="  first_metric: {kind: hard}\n")
    before = _gap(tmp_path).count
    _base(
        tmp_path,
        metrics="  first_metric: {kind: hard}\n  second_metric: {kind: hard}\n",
    )
    after = _gap(tmp_path)
    assert after.count == before + 1
    assert _names(after, HARD_METRIC_WITHOUT_COMPUTATION) == {
        "first_metric",
        "second_metric",
    }


# ── class B: stage hard invariant without a check ────────────────────────────


def test_invariant_referenced_by_a_governor_check_is_not_a_gap(tmp_path: Path):
    _base(tmp_path)
    _write_stages(tmp_path, ["replay_idempotent"])
    _write(
        tmp_path,
        "scripts/governor/checks/acap_runtime_control_plane.yaml",
        "evidence_obligation: replay_idempotent\n",
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK) == ()
    assert axis.declared_for(STAGE_INVARIANT_WITHOUT_CHECK) == 1


def test_invariant_referenced_only_by_a_test_is_a_mapping(tmp_path: Path):
    _base(tmp_path)
    _write_stages(tmp_path, ["no_nan"])
    _write(
        tmp_path,
        "crates/da-adapters/tests/embedding.rs",
        "// invariant: no_nan\nfn rejects_non_finite_input() {}\n",
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK) == ()


def test_invariant_without_any_check_is_named(tmp_path: Path):
    _base(tmp_path)
    _write_stages(tmp_path, ["candidate_not_graph_truth"])
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK)
    assert [g.name for g in gaps] == ["candidate_not_graph_truth"]
    assert gaps[0].declared_in == _STAGE_PATH


def test_substring_is_not_a_mapping(tmp_path: Path):
    """`exact_lineage` inside `exact_lineage_and_policy_locks` is not a check.

    A substring scan would hide the decorative name this axis exists to expose.
    """
    _base(tmp_path)
    _write_stages(tmp_path, ["exact_lineage"])
    _write(
        tmp_path,
        "crates/da-parse/tests/callout_matching_oracle.rs",
        "fn exact_lineage_and_policy_locks() {}\n",
    )
    axis = _gap(tmp_path)
    assert _names(axis, STAGE_INVARIANT_WITHOUT_CHECK) == {"exact_lineage"}


def test_production_only_mention_is_not_a_check(tmp_path: Path):
    """A name in production src is not a mapping to a check."""
    _base(tmp_path)
    _write_stages(tmp_path, ["derived_not_authoritative"])
    _write(
        tmp_path,
        "crates/da-application/src/readiness.rs",
        "let derived_not_authoritative = true;\n",
    )
    axis = _gap(tmp_path)
    assert _names(axis, STAGE_INVARIANT_WITHOUT_CHECK) == {"derived_not_authoritative"}


def test_distinct_invariants_are_deduplicated(tmp_path: Path):
    _base(tmp_path)
    _write_stages(tmp_path, ["replay_idempotent"])
    # Same name declared by a second stage must not double-count.
    _write(
        tmp_path,
        _STAGE_PATH,
        "stages:\n"
        "  - id: a\n    hard_invariants: [replay_idempotent]\n"
        "  - id: b\n    hard_invariants: [replay_idempotent]\n",
    )
    axis = _gap(tmp_path)
    assert axis.declared_for(STAGE_INVARIANT_WITHOUT_CHECK) == 1
    assert len(axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK)) == 1


# ── class C: prompt call form without a registry entry ───────────────────────


def test_prompt_binding_key_without_a_task_is_named(tmp_path: Path):
    _base(tmp_path)
    _write_prompt_registry(tmp_path, task_ids=["article_summary"], binding_keys=["critic"])
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(PROMPT_FORM_WITHOUT_REGISTRY_ENTRY)
    assert [g.name for g in gaps] == ["critic"]
    assert gaps[0].declared_in == _PROMPT_PATH
    assert "no tasks[] entry" in gaps[0].detail


def test_prompt_binding_key_with_a_task_resolves(tmp_path: Path):
    _base(tmp_path)
    _write_prompt_registry(
        tmp_path, task_ids=["article_summary"], binding_keys=["article_summary"]
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(PROMPT_FORM_WITHOUT_REGISTRY_ENTRY) == ()
    assert axis.declared_for(PROMPT_FORM_WITHOUT_REGISTRY_ENTRY) == 1


# ── aggregate ────────────────────────────────────────────────────────────────


def test_aggregate_count_sums_the_classes_and_names_each_gap(tmp_path: Path):
    _base(tmp_path, metrics="  hard_unbuilt: {kind: hard}\n")
    _write_stages(tmp_path, ["decorative_a", "decorative_b"])
    _write_prompt_registry(tmp_path, task_ids=[], binding_keys=["orphan"])
    axis = _gap(tmp_path)
    assert axis.count == 4
    assert len(axis.gaps) == 4
    data = axis.to_dict()
    assert data["axis"] == "declaration_sync_gap"
    assert data["gap_count"] == 4
    assert data["gaps_by_class"][HARD_METRIC_WITHOUT_COMPUTATION]["gaps"] == 1
    assert data["gaps_by_class"][STAGE_INVARIANT_WITHOUT_CHECK]["gaps"] == 2
    assert data["gaps_by_class"][PROMPT_FORM_WITHOUT_REGISTRY_ENTRY]["gaps"] == 1
    assert {g["name"] for g in data["gaps"]} == {
        "hard_unbuilt",
        "decorative_a",
        "decorative_b",
        "orphan",
    }


def test_render_lines_state_the_count_and_the_classes(tmp_path: Path):
    _base(tmp_path, metrics="  hard_unbuilt: {kind: hard}\n")
    axis = _gap(tmp_path)
    rendered = "\n".join(axis.render_lines())
    assert "declaration synchronisation: 1 gaps" in rendered
    assert "hard metrics without a production computation: 1/1 unmapped" in rendered
    assert "hard_unbuilt" in rendered


# ── fail closed ──────────────────────────────────────────────────────────────


def test_missing_crates_dir_fails_loudly(tmp_path: Path):
    _write_metrics(tmp_path, "  a: {kind: hard}\n")
    with pytest.raises(FileNotFoundError):
        _gap(tmp_path)


def test_missing_declaration_file_fails_loudly(tmp_path: Path):
    (tmp_path / "crates").mkdir()
    _write_stages(tmp_path, [])
    _write_prompt_registry(tmp_path, [], [])
    with pytest.raises(FileNotFoundError):
        _gap(tmp_path)


def test_malformed_metric_declaration_fails_loudly(tmp_path: Path):
    (tmp_path / "crates").mkdir()
    _write(tmp_path, _METRIC_PATH, "metric_definitions: {not: a, mapping: [}\n")
    with pytest.raises(ValueError):
        _gap(tmp_path)


# ── real tree ────────────────────────────────────────────────────────────────


def test_real_tree_reports_a_nonzero_count_and_names_the_known_gaps():
    """On the live tree the axis is non-zero and names reader-checkable gaps.

    `checksum_lineage_coverage` is one of CONTEXT G2's unimplemented hard
    metrics; `critic` is CONTEXT G5's binding with no registered task.
    """
    axis = observe_declaration_sync_gap()
    assert axis.count > 0
    metric_names = _names(axis, HARD_METRIC_WITHOUT_COMPUTATION)
    assert "checksum_lineage_coverage" in metric_names
    assert "semantic_task_pass_rate" in metric_names
    assert "critic" in _names(axis, PROMPT_FORM_WITHOUT_REGISTRY_ENTRY)
    # The count is exactly the sum of the per-class gaps.
    assert axis.count == sum(
        len(axis.gaps_for(cls))
        for cls in (
            HARD_METRIC_WITHOUT_COMPUTATION,
            STAGE_INVARIANT_WITHOUT_CHECK,
            PROMPT_FORM_WITHOUT_REGISTRY_ENTRY,
        )
    )
    rendered = axis.render_lines()
    assert rendered and "declaration synchronisation:" in rendered[0]


def test_real_tree_metric_gaps_are_a_subset_of_the_declared_hard_metrics():
    """Every named metric gap is genuinely declared kind: hard."""
    import yaml

    axis = observe_declaration_sync_gap()
    root = Path(__file__).resolve().parent.parent.parent
    declarations = yaml.safe_load((root / _METRIC_PATH).read_text(encoding="utf-8"))
    hard_names = {
        name
        for name, spec in declarations["metric_definitions"].items()
        if isinstance(spec, dict) and spec.get("kind") == "hard"
    }
    assert _names(axis, HARD_METRIC_WITHOUT_COMPUTATION) <= hard_names
