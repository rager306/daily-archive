"""Declaration synchronisation gap axis: gap count, named gaps, negative cases.

The axis exists because the DATA and the code can disagree silently: a policy
declares a hard metric no code computes, a stage lists an invariant with no
disposition, a prompt declaration binds a call form that neither its ``tasks[]``
nor the unified prompt registry can resolve, or a declared prompt lineage field
has no disposition record. These tests pin the four predicates with inline
fixtures, so they never depend on the real tree's current membership, and two
assert the regression direction directly: adding a declared hard metric or a
declared lineage field without a record adds exactly one gap.

Class A and class B are both disposition-driven: a hard metric is covered when
``scripts/governor/checks/article_context_assurance.yaml`` carries a
well-formed ``metric_dispositions`` record whose symbol resolves in
``crates/*/src``, and an invariant is covered when
``data/ontology/pipeline_stage_contracts.yaml`` carries a well-formed
``stage_invariant_dispositions`` record for it -- never by the incidental
appearance of a name in some other file. Class C resolves a bound call form
against the union of ``prompt_tasks.yaml`` ``tasks[]`` and ``prompt_registry.yaml``
``entries[].prompt_id``. Class D is disposition-driven too: a declared
``prompt_contract.required_lineage_fields`` entry is covered when
``prompt_lineage_dispositions.records`` carries a ``recorded`` record whose
``field`` occurs as a whole token in
``crates/da-domain/src/prompt_optimization.rs``, or a ``downgraded`` record
with an owner and a reason.
"""

from __future__ import annotations

from pathlib import Path

import pytest
import yaml

from governor.adapters.declaration_sync_gap import (
    HARD_METRIC_WITHOUT_COMPUTATION,
    PROMPT_FORM_WITHOUT_REGISTRY_ENTRY,
    PROMPT_LINEAGE_FIELD_WITHOUT_RECORD,
    STAGE_INVARIANT_WITHOUT_CHECK,
    build_declaration_sync_gap,
    observe_declaration_sync_gap,
)

_METRIC_PATH = "scripts/governor/checks/article_context_assurance.yaml"
_STAGE_PATH = "data/ontology/pipeline_stage_contracts.yaml"
_PROMPT_PATH = "data/ontology/prompt_tasks.yaml"
_PROMPT_ENTRY_PATH = "data/ontology/prompt_registry.yaml"
_LINEAGE_SURFACE = "crates/da-domain/src/prompt_optimization.rs"

#: The nine fields ``article_context_assurance.yaml`` declares and T01 recorded
#: on ``PromptExecutionLineage`` -- the surface token scan the default fixture
#: must satisfy.
_LINEAGE_FIELDS = [
    "prompt_id",
    "prompt_version",
    "prompt_sha256",
    "context_bundle_sha256",
    "provider",
    "model",
    "inference_config_sha256",
    "output_schema_sha256",
    "output_sha256",
]


# ── fixtures ─────────────────────────────────────────────────────────────────


def _write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _write_metrics(
    root: Path,
    body: str,
    dispositions: dict | None = None,
    *,
    include_dispositions: bool = True,
    lineage_fields: list[str] | None = None,
    lineage_records: dict | None = None,
    include_lineage: bool = True,
) -> None:
    """Write the metric declaration with its hard-metric and lineage registries.

    ``dispositions=None`` writes an empty ``metric_dispositions.records`` mapping
    so a caller that only cares about the declared metrics still produces a
    loadable fixture; pass a mapping to cover metrics, or
    ``include_dispositions=False`` to exercise the fail-closed path for a missing
    block. ``lineage_fields`` / ``lineage_records`` mirror that for the prompt
    lineage declaration, which lives in the same file (class D).
    """
    declaration = yaml.safe_load(f"metric_definitions:\n{body}")
    if not isinstance(declaration, dict):
        declaration = {}
    if include_dispositions:
        declaration["metric_dispositions"] = {
            "schema_version": "metric-dispositions.v1",
            "reason_vocabulary": {
                "no_production_denominator": (
                    "The metric has no typed production evidence that yields a "
                    "real denominator."
                ),
            },
            "records": dispositions or {},
        }
    if include_lineage:
        declaration["prompt_contract"] = {
            "required_lineage_fields": list(lineage_fields or []),
        }
        declaration["prompt_lineage_dispositions"] = {
            "schema_version": "prompt-lineage-dispositions.v1",
            "reason_vocabulary": {
                "not_recorded_on_lineage": (
                    "The runtime lineage does not carry the field."
                ),
            },
            "records": lineage_records or {},
        }
    _write(root, _METRIC_PATH, yaml.safe_dump(declaration, sort_keys=False))


def _computed(computation: str) -> dict:
    return {"disposition": "computed", "computation": computation}


def _write_computation_surface(root: Path, symbol: str) -> None:
    """A production-looking Rust file carrying ``symbol`` as a whole token."""
    _write(
        root,
        "crates/da-domain/src/hard_metrics.rs",
        f"pub fn {symbol}() {{ /* production computation */ }}\n",
    )


def _write_lineage_surface(root: Path, fields: list[str] | None = None) -> None:
    """The Rust type that must carry every declared lineage field as a token."""
    body = "\n".join(f"    pub {field}: String," for field in (fields or _LINEAGE_FIELDS))
    _write(
        root,
        _LINEAGE_SURFACE,
        f"pub struct PromptExecutionLineage {{\n{body}\n}}\n",
    )


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
    root: Path,
    task_ids: list[str],
    binding_keys: list[str],
    registry_ids: list[str] | None = None,
) -> None:
    """Write the prompt-task declaration and the unified entry registry.

    ``registry_ids`` is the meta-prompt entry surface
    (``data/ontology/prompt_registry.yaml``); it defaults to ``task_ids`` so a
    form declared as both a task and a registry entry stays resolvable.
    """
    if registry_ids is None:
        registry_ids = list(task_ids)
    declaration = {
        "schema_version": "prompt-task-registry.v1",
        "prompt_contract": {
            "task_rule_bindings": {key: ["PR-001"] for key in binding_keys},
        },
        "tasks": [{"task_id": task_id} for task_id in task_ids],
    }
    _write(root, _PROMPT_PATH, yaml.safe_dump(declaration, sort_keys=False))
    registry = {
        "schema_version": "prompt-registry.v1",
        "registry_id": "test-registry",
        "entries": [
            {
                "prompt_id": prompt_id,
                "prompt_version": "1",
                "source_ref": f"data/prompts/{prompt_id}.md",
            }
            for prompt_id in registry_ids
        ],
    }
    _write(root, _PROMPT_ENTRY_PATH, yaml.safe_dump(registry, sort_keys=False))


def _base(
    root: Path,
    metrics: str = "  a_hard: {kind: hard}\n",
    dispositions: dict | None = None,
) -> None:
    """A minimal valid declaration tree with an (empty) crates/ directory."""
    (root / "crates").mkdir(parents=True, exist_ok=True)
    _write_metrics(root, metrics, dispositions)
    _write_stages(root, [])
    _write_prompt_registry(root, [], [])
    _write_lineage_surface(root)


def _gap(root: Path):
    return build_declaration_sync_gap(root)


def _names(axis, gap_class: str) -> set[str]:
    return {g.name for g in axis.gaps_for(gap_class)}


# ── class A: hard metric without a disposition record ────────────────────────


def test_computed_metric_whose_symbol_resolves_is_not_a_gap(tmp_path: Path):
    """A `computed` record is covered only when its symbol is in crates/*/src."""
    _base(
        tmp_path,
        metrics="  required_field_retention: {kind: hard}\n",
        dispositions={
            "required_field_retention": _computed(
                "da-domain::assurance::required_field_retention"
            )
        },
    )
    _write_computation_surface(tmp_path, "required_field_retention")
    axis = _gap(tmp_path)
    assert axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION) == ()
    assert axis.declared_for(HARD_METRIC_WITHOUT_COMPUTATION) == 1


def test_computed_metric_whose_symbol_does_not_resolve_is_named(tmp_path: Path):
    _base(
        tmp_path,
        metrics="  checksum_lineage_coverage: {kind: hard}\n",
        dispositions={
            "checksum_lineage_coverage": _computed(
                "da-domain::hard_metrics::no_such_symbol_anywhere"
            )
        },
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION)
    assert len(gaps) == 1
    assert gaps[0].name == "checksum_lineage_coverage"
    assert gaps[0].declared_in == _METRIC_PATH
    assert "no_such_symbol_anywhere" in gaps[0].detail


def test_downgraded_metric_with_owner_and_reason_is_covered(tmp_path: Path):
    _base(
        tmp_path,
        metrics="  needs_real_denominator: {kind: hard}\n",
        dispositions={
            "needs_real_denominator": {
                "disposition": "downgraded",
                "owner": "M516-th0oby/S08",
                "reason": "no_production_denominator",
                "decision_ref": "D556",
            }
        },
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION) == ()


def test_declared_hard_metric_without_a_record_is_named(tmp_path: Path):
    _base(tmp_path, metrics="  checksum_lineage_coverage: {kind: hard}\n")
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION)
    assert [g.name for g in gaps] == ["checksum_lineage_coverage"]
    assert gaps[0].declared_in == _METRIC_PATH
    assert "no metric_dispositions record" in gaps[0].detail


def test_computed_record_without_a_computation_is_a_gap(tmp_path: Path):
    _base(
        tmp_path,
        metrics="  checksum_lineage_coverage: {kind: hard}\n",
        dispositions={"checksum_lineage_coverage": {"disposition": "computed"}},
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION)
    assert [g.name for g in gaps] == ["checksum_lineage_coverage"]
    assert "no non-empty computation" in gaps[0].detail


def test_downgraded_metric_without_owner_or_reason_is_a_gap(tmp_path: Path):
    _base(
        tmp_path,
        metrics="  checksum_lineage_coverage: {kind: hard}\n",
        dispositions={"checksum_lineage_coverage": {"disposition": "downgraded"}},
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION)
    assert [g.name for g in gaps] == ["checksum_lineage_coverage"]
    assert "owner and reason" in gaps[0].detail


def test_unknown_metric_disposition_is_a_gap(tmp_path: Path):
    _base(
        tmp_path,
        metrics="  checksum_lineage_coverage: {kind: hard}\n",
        dispositions={"checksum_lineage_coverage": {"disposition": "ignored"}},
    )
    axis = _gap(tmp_path)
    assert _names(axis, HARD_METRIC_WITHOUT_COMPUTATION) == {
        "checksum_lineage_coverage"
    }


def test_diagnostic_metric_is_outside_the_axis(tmp_path: Path):
    _base(tmp_path, metrics="  coverage: {kind: diagnostic}\n")
    axis = _gap(tmp_path)
    assert axis.declared_for(HARD_METRIC_WITHOUT_COMPUTATION) == 0
    assert axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION) == ()


def test_adding_a_hard_metric_without_a_record_adds_exactly_one_gap(
    tmp_path: Path,
):
    """The regression direction: a new declared hard metric is exactly +1 gap."""
    covered = {"first_metric": _computed("da-domain::hard_metrics::first_metric")}
    _base(tmp_path, metrics="  first_metric: {kind: hard}\n", dispositions=covered)
    _write_computation_surface(tmp_path, "first_metric")
    before = _gap(tmp_path).count
    _base(
        tmp_path,
        metrics="  first_metric: {kind: hard}\n  second_metric: {kind: hard}\n",
        dispositions=covered,
    )
    _write_computation_surface(tmp_path, "first_metric")
    after = _gap(tmp_path)
    assert after.count == before + 1
    assert _names(after, HARD_METRIC_WITHOUT_COMPUTATION) == {"second_metric"}


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


def test_prompt_binding_key_resolves_via_the_entry_registry(tmp_path: Path):
    """`critic` is a meta prompt: no ``tasks[]`` entry, but a registry entry."""
    _base(tmp_path)
    _write_prompt_registry(
        tmp_path,
        task_ids=[],
        binding_keys=["critic"],
        registry_ids=["critic"],
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(PROMPT_FORM_WITHOUT_REGISTRY_ENTRY) == ()
    assert axis.declared_for(PROMPT_FORM_WITHOUT_REGISTRY_ENTRY) == 1


def test_missing_prompt_entry_registry_fails_loudly(tmp_path: Path):
    """A missing unified registry is a required-input failure, not a free 0."""
    (tmp_path / "crates").mkdir(parents=True)
    _write_metrics(tmp_path, "  a_hard: {kind: hard}\n")
    _write_stages(tmp_path, [])
    _write_lineage_surface(tmp_path)
    _write(
        tmp_path,
        _PROMPT_PATH,
        "schema_version: prompt-task-registry.v1\n"
        "prompt_contract:\n"
        "  task_rule_bindings: {article_summary: [PR-001]}\n"
        "tasks:\n"
        "  - task_id: article_summary\n",
    )
    with pytest.raises(FileNotFoundError):
        _gap(tmp_path)


def test_malformed_prompt_entry_registry_fails_loudly(tmp_path: Path):
    _base(tmp_path)
    _write(
        tmp_path,
        _PROMPT_ENTRY_PATH,
        "schema_version: prompt-registry.v1\nentries: {not: a-list}\n",
    )
    with pytest.raises(ValueError):
        _gap(tmp_path)


# ── class D: declared prompt lineage field without a disposition record ───────


def test_recorded_lineage_field_resolving_in_the_surface_is_covered(tmp_path: Path):
    _base(tmp_path)
    _write_lineage_surface(tmp_path, ["prompt_id"])
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["prompt_id"],
        lineage_records={
            "prompt_id": {"disposition": "recorded", "field": "prompt_id"}
        },
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD) == ()
    assert axis.declared_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD) == 1


def test_declared_lineage_field_without_a_record_is_named(tmp_path: Path):
    _base(tmp_path)
    _write_metrics(tmp_path, "  a_hard: {kind: hard}\n", lineage_fields=["prompt_id"])
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD)
    assert [g.name for g in gaps] == ["prompt_id"]
    assert gaps[0].declared_in == _METRIC_PATH
    assert "no prompt_lineage_dispositions record" in gaps[0].detail


def test_recorded_lineage_field_naming_an_absent_symbol_is_malformed(tmp_path: Path):
    _base(tmp_path)
    _write_lineage_surface(tmp_path, ["prompt_id"])
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["prompt_id"],
        lineage_records={
            "prompt_id": {"disposition": "recorded", "field": "no_such_field"}
        },
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD)
    assert [g.name for g in gaps] == ["prompt_id"]
    assert "no_such_field" in gaps[0].detail


def test_recorded_lineage_field_without_a_field_is_malformed(tmp_path: Path):
    _base(tmp_path)
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["prompt_id"],
        lineage_records={"prompt_id": {"disposition": "recorded"}},
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD)
    assert [g.name for g in gaps] == ["prompt_id"]
    assert "no non-empty field" in gaps[0].detail


def test_downgraded_lineage_field_with_owner_and_reason_is_covered(tmp_path: Path):
    _base(tmp_path)
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["some_future_field"],
        lineage_records={
            "some_future_field": {
                "disposition": "downgraded",
                "owner": "M516-th0oby/S09",
                "reason": "not_recorded_on_lineage",
            }
        },
    )
    axis = _gap(tmp_path)
    assert axis.gaps_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD) == ()


def test_downgraded_lineage_field_without_owner_or_reason_is_a_gap(tmp_path: Path):
    _base(tmp_path)
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["prompt_id"],
        lineage_records={"prompt_id": {"disposition": "downgraded"}},
    )
    axis = _gap(tmp_path)
    gaps = axis.gaps_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD)
    assert [g.name for g in gaps] == ["prompt_id"]
    assert "owner and reason" in gaps[0].detail


def test_unknown_lineage_disposition_is_a_gap(tmp_path: Path):
    _base(tmp_path)
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["prompt_id"],
        lineage_records={"prompt_id": {"disposition": "ignored"}},
    )
    axis = _gap(tmp_path)
    assert _names(axis, PROMPT_LINEAGE_FIELD_WITHOUT_RECORD) == {"prompt_id"}


def test_adding_a_declared_lineage_field_without_a_record_adds_one_gap(tmp_path: Path):
    """The regression direction: a new declared lineage field is exactly +1 gap."""
    _base(tmp_path)
    covered = {
        "prompt_id": {
            "disposition": "downgraded",
            "owner": "M516-th0oby/S09",
            "reason": "not_recorded_on_lineage",
        }
    }
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["prompt_id"],
        lineage_records=covered,
    )
    before = _gap(tmp_path).count
    _write_metrics(
        tmp_path,
        "  a_hard: {kind: hard}\n",
        lineage_fields=["prompt_id", "second_field"],
        lineage_records=covered,
    )
    after = _gap(tmp_path)
    assert after.count == before + 1
    assert _names(after, PROMPT_LINEAGE_FIELD_WITHOUT_RECORD) == {"second_field"}


def test_missing_prompt_lineage_dispositions_block_fails_loudly(tmp_path: Path):
    """A declared lineage field with no block must raise, not read as covered."""
    _base(tmp_path)
    _write(
        tmp_path,
        _METRIC_PATH,
        "metric_definitions:\n"
        "  a_hard: {kind: hard}\n"
        "metric_dispositions:\n"
        "  schema_version: metric-dispositions.v1\n"
        "  records: {}\n"
        "prompt_contract:\n"
        "  required_lineage_fields: [prompt_id]\n",
    )
    with pytest.raises(ValueError):
        _gap(tmp_path)


def test_malformed_prompt_lineage_dispositions_records_fails_loudly(tmp_path: Path):
    """A non-mapping `records` block must raise rather than vacuously pass."""
    _base(tmp_path)
    _write(
        tmp_path,
        _METRIC_PATH,
        "metric_definitions:\n"
        "  a_hard: {kind: hard}\n"
        "metric_dispositions:\n"
        "  schema_version: metric-dispositions.v1\n"
        "  records: {}\n"
        "prompt_contract:\n"
        "  required_lineage_fields: [prompt_id]\n"
        "prompt_lineage_dispositions:\n"
        "  schema_version: prompt-lineage-dispositions.v1\n"
        "  records: []\n",
    )
    with pytest.raises(ValueError):
        _gap(tmp_path)


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
    assert data["gaps_by_class"][PROMPT_LINEAGE_FIELD_WITHOUT_RECORD]["gaps"] == 0
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
    assert "prompt lineage fields without a disposition record: 0/0 unmapped" in rendered
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


def test_missing_metric_dispositions_block_fails_loudly(tmp_path: Path):
    """A metric file with no dispositions block must raise, not read as zero."""
    (tmp_path / "crates").mkdir()
    _write(tmp_path, _METRIC_PATH, "metric_definitions:\n  a: {kind: hard}\n")
    _write_stages(tmp_path, [])
    _write_prompt_registry(tmp_path, [], [])
    with pytest.raises(ValueError):
        _gap(tmp_path)


def test_malformed_metric_disposition_records_fails_loudly(tmp_path: Path):
    """A non-mapping `records` block must raise rather than vacuously pass."""
    (tmp_path / "crates").mkdir()
    _write(
        tmp_path,
        _METRIC_PATH,
        "metric_definitions:\n  a: {kind: hard}\n"
        "metric_dispositions:\n"
        "  schema_version: metric-dispositions.v1\n"
        "  records: []\n",
    )
    _write_stages(tmp_path, [])
    _write_prompt_registry(tmp_path, [], [])
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


def test_real_tree_all_declaration_classes_read_zero_and_critic_resolves():
    """On the live tree every declaration is disposed and `critic` resolves.

    All eleven declared hard metrics carry a resolving `computed` record
    (class A 0/11); all 52 declared stage hard invariants carry a disposition
    (class B 0/52); each of the three `task_rule_bindings` keys resolves -- the
    meta-prompt `critic` through `prompt_registry.yaml`, where it has no
    `tasks[]` entry (class C 0/3); and all nine declared lineage fields carry a
    resolving `recorded` record (class D 0/9).
    """
    root = Path(__file__).resolve().parent.parent.parent
    prompt = yaml.safe_load((root / _PROMPT_PATH).read_text(encoding="utf-8"))
    assert "critic" in prompt["prompt_contract"]["task_rule_bindings"]

    axis = observe_declaration_sync_gap()
    assert axis.gaps_for(HARD_METRIC_WITHOUT_COMPUTATION) == ()
    assert axis.gaps_for(STAGE_INVARIANT_WITHOUT_CHECK) == ()
    assert axis.gaps_for(PROMPT_FORM_WITHOUT_REGISTRY_ENTRY) == ()
    assert axis.gaps_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD) == ()
    assert axis.count == 0
    assert axis.declared_for(HARD_METRIC_WITHOUT_COMPUTATION) == 11
    assert axis.declared_for(STAGE_INVARIANT_WITHOUT_CHECK) == 52
    assert axis.declared_for(PROMPT_FORM_WITHOUT_REGISTRY_ENTRY) == 3
    assert axis.declared_for(PROMPT_LINEAGE_FIELD_WITHOUT_RECORD) == 9
    rendered = axis.render_lines()
    assert rendered and "declaration synchronisation:" in rendered[0]


def test_real_tree_every_declared_hard_metric_carries_a_disposition_record():
    """The registry covers exactly the declared hard metrics, no more, no less."""
    root = Path(__file__).resolve().parent.parent.parent
    declarations = yaml.safe_load((root / _METRIC_PATH).read_text(encoding="utf-8"))
    hard_names = {
        name
        for name, spec in declarations["metric_definitions"].items()
        if isinstance(spec, dict) and spec.get("kind") == "hard"
    }
    records = declarations["metric_dispositions"]["records"]
    assert set(records) == hard_names


def test_real_tree_every_declared_lineage_field_carries_a_record():
    """The lineage registry covers exactly the declared fields, no more, no less."""
    root = Path(__file__).resolve().parent.parent.parent
    declarations = yaml.safe_load((root / _METRIC_PATH).read_text(encoding="utf-8"))
    declared = set(declarations["prompt_contract"]["required_lineage_fields"])
    records = declarations["prompt_lineage_dispositions"]["records"]
    assert set(records) == declared
