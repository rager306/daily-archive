"""Declaration synchronisation gap axis: gap count, named gaps, negative cases.

The axis exists because the DATA and the code can disagree silently: a policy
declares a hard metric no code computes, a stage lists an invariant with no
disposition, or a prompt registry binds a task with no task entry. These tests
pin the three predicates with inline fixtures, so they never depend on the real
tree's current membership, and one asserts the regression direction directly:
adding a declared hard metric without an implementation adds exactly one gap.

Class B is disposition-driven: an invariant is covered when
``data/ontology/pipeline_stage_contracts.yaml`` carries a well-formed
``stage_invariant_dispositions`` record for it, never by the incidental
appearance of its name in some other file.
"""

from __future__ import annotations

from pathlib import Path

import pytest
import yaml

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


def _write_stages(
    root: Path,
    invariants: list[str],
    dispositions: dict | None = None,
    *,
    include_dispositions: bool = True,
) -> None:
    """Write the stage declaration, optionally without the dispositions block.

    ``dispositions=None`` writes an empty ``records`` mapping so a caller that
    only cares about the declared names still produces a loadable fixture; pass
    a mapping to cover names, or ``include_dispositions=False`` to exercise the
    fail-closed path for a missing block.
    """
    declaration: dict = {
        "schema_version": "pipeline-stage-contracts.v1",
        "stages": [
            {"id": "materialize", "hard_invariants": list(invariants)},
        ],
    }
    if include_dispositions:
        declaration["stage_invariant_dispositions"] = {
            "schema_version": "stage-invariant-dispositions.v1",
            "records": dispositions or {},
        }
    _write(root, _STAGE_PATH, yaml.safe_dump(declaration, sort_keys=False))


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


# ── class B: stage hard invariant without a disposition record ───────────────


def test_checked_disposition_covers_an_invariant_with_no_token_anywhere(
    tmp_path: Path,
):
    """A well-formed `checked` record is the mapping, not a token elsewhere."""
    _base(tmp_path)
    _write_stages(
        tmp_path,
        ["replay_idempotent"],
        dispositions={
            "replay_idempotent": {
                "disposition": "checked",
                "check_id": "replay_idempotent",
            }
        },
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK) == ()
    assert axis.declared_for(STAGE_INVARIANT_WITHOUT_CHECK) == 1


def test_downgraded_disposition_with_owner_and_reason_is_covered(tmp_path: Path):
    _base(tmp_path)
    _write_stages(
        tmp_path,
        ["network_requires_explicit_authorization"],
        dispositions={
            "network_requires_explicit_authorization": {
                "disposition": "downgraded",
                "owner": "M516/S08",
                "reason": "review_only_commitment",
            }
        },
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK) == ()


def test_declared_invariant_without_a_disposition_record_is_named(tmp_path: Path):
    _base(tmp_path)
    _write_stages(tmp_path, ["candidate_not_graph_truth"], dispositions={})
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK)
    assert [g.name for g in gaps] == ["candidate_not_graph_truth"]
    assert gaps[0].declared_in == _STAGE_PATH
    assert "no stage_invariant_dispositions record" in gaps[0].detail


def test_checked_record_without_a_check_id_is_a_gap(tmp_path: Path):
    _base(tmp_path)
    _write_stages(
        tmp_path,
        ["no_nan"],
        dispositions={"no_nan": {"disposition": "checked"}},
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK)
    assert [g.name for g in gaps] == ["no_nan"]
    assert "check_id" in gaps[0].detail


def test_downgraded_record_without_owner_or_reason_is_a_gap(tmp_path: Path):
    _base(tmp_path)
    _write_stages(
        tmp_path,
        ["evidence_not_yet_emitted"],
        dispositions={"evidence_not_yet_emitted": {"disposition": "downgraded"}},
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK)
    assert [g.name for g in gaps] == ["evidence_not_yet_emitted"]
    assert "owner and reason" in gaps[0].detail


def test_unknown_disposition_is_a_gap(tmp_path: Path):
    _base(tmp_path)
    _write_stages(
        tmp_path,
        ["typed_missingness"],
        dispositions={"typed_missingness": {"disposition": "ignored"}},
    )
    axis = _gap(tmp_path)
    assert _names(axis, STAGE_INVARIANT_WITHOUT_CHECK) == {"typed_missingness"}


def test_adding_a_declared_invariant_without_a_record_adds_one_gap(tmp_path: Path):
    """The regression direction: a new declared invariant is exactly +1 gap."""
    _base(tmp_path)
    _write_stages(
        tmp_path,
        ["first_inv"],
        dispositions={"first_inv": {"disposition": "checked", "check_id": "first_inv"}},
    )
    before = _gap(tmp_path).count
    _write_stages(
        tmp_path,
        ["first_inv", "second_inv"],
        dispositions={"first_inv": {"disposition": "checked", "check_id": "first_inv"}},
    )
    after = _gap(tmp_path)
    assert after.count == before + 1
    assert _names(after, STAGE_INVARIANT_WITHOUT_CHECK) == {"second_inv"}


def test_distinct_invariants_are_deduplicated(tmp_path: Path):
    _base(tmp_path)
    # Same name declared twice must not double-count.
    _write_stages(
        tmp_path,
        ["replay_idempotent", "replay_idempotent"],
        dispositions={},
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


def test_missing_dispositions_block_fails_loudly(tmp_path: Path):
    """A stage file with no dispositions block must raise, not read as zero gaps."""
    _base(tmp_path)
    _write_stages(
        tmp_path,
        ["replay_idempotent"],
        include_dispositions=False,
    )
    with pytest.raises(ValueError):
        _gap(tmp_path)


def test_malformed_dispositions_records_fails_loudly(tmp_path: Path):
    """A non-mapping `records` block must raise rather than vacuously pass."""
    _base(tmp_path)
    _write(
        tmp_path,
        _STAGE_PATH,
        "schema_version: pipeline-stage-contracts.v1\n"
        "stages:\n"
        "  - id: materialize\n"
        "    hard_invariants: [replay_idempotent]\n"
        "stage_invariant_dispositions:\n"
        "  schema_version: stage-invariant-dispositions.v1\n"
        "  records: []\n",
    )
    with pytest.raises(ValueError):
        _gap(tmp_path)


# ── real tree ────────────────────────────────────────────────────────────────


def test_real_tree_reports_a_nonzero_count_and_names_the_known_gaps():
    """On the live tree class B reads zero and class C still names `critic`.

    Every one of the 52 declared stage hard invariants carries a disposition
    record, so class B has no gaps. Class A's membership is owned by T05, so
    this test does not pin it; class C is unchanged.
    """
    axis = observe_declaration_sync_gap()
    assert axis.count > 0
    assert axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK) == ()
    assert axis.declared_for(STAGE_INVARIANT_WITHOUT_CHECK) == 52
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
    axis = observe_declaration_sync_gap()
    root = Path(__file__).resolve().parent.parent.parent
    declarations = yaml.safe_load((root / _METRIC_PATH).read_text(encoding="utf-8"))
    hard_names = {
        name
        for name, spec in declarations["metric_definitions"].items()
        if isinstance(spec, dict) and spec.get("kind") == "hard"
    }
    assert _names(axis, HARD_METRIC_WITHOUT_COMPUTATION) <= hard_names
