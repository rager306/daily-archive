"""Measurability axis: measured/mandatory ratio, unmeasured names, negative cases.

The axis exists because "green" and "measured" are not the same thing. Every
gate in the acceptance package carries evidence, so evidence alone cannot tell a
measured gate from a dispositioned one; only the contract's ``measured_states``
vocabulary separates them. These tests pin the predicate — evidence bound to a
manifest and a pointer, and a state the contract counts as measured — with inline
fixtures, so they never depend on the live package's current membership, and one
asserts the regression direction directly: removing a measured gate changes the
ratio.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
import yaml

from governor.adapters.measurability_axis import (
    build_measurability_axis,
    observe_measurability_axis,
)

_PACKAGE_PATH = "artifacts/parse_compat/grobid_retirement_acceptance_package.json"
_CONTRACT_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"

_MANIFEST = "artifacts/parse_compat/native_corpus_acceptance.json"
_VOCABULARY = [
    "met",
    "unmet",
    "not_comparable",
    "threshold_pending",
    "not_assessable",
    "not_observed",
    "typed_refusal",
    "pending",
    "accepted_residual",
    "not_applicable",
]


# ── fixtures ─────────────────────────────────────────────────────────────────


def _write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _gate(
    gate_id: str,
    state: str,
    *,
    required: bool = True,
    blocks_s09: bool = False,
    kind: str = "threshold",
    evidence: bool = True,
) -> dict:
    gate = {
        "gate_id": gate_id,
        "kind": kind,
        "state": state,
        "required": required,
        "blocks_s09": blocks_s09,
    }
    if evidence:
        gate["evidence"] = {
            "manifest": _MANIFEST,
            "pointer": "documents",
            "revision_sha256": "sha256:" + "0" * 64,
        }
    return gate


def _package(root: Path, gates: list[dict]) -> None:
    _write(root, _PACKAGE_PATH, json.dumps({"mandatory_gates": gates}, indent=2))


def _contract(
    root: Path,
    measured_states=("met", "accepted_residual"),
    vocabulary=_VOCABULARY,
) -> None:
    expected: dict = {}
    if measured_states is not None:
        expected["measured_states"] = list(measured_states)
    if vocabulary is not None:
        expected["gate_state_vocabulary"] = list(vocabulary)
    _write(
        root,
        _CONTRACT_PATH,
        yaml.safe_dump({"acceptance_package": {"expected": expected}}),
    )


def _axis(root: Path):
    return build_measurability_axis(root)


def _ids(gates) -> set[str]:
    return {g.gate_id for g in gates}


# ── the measured predicate ───────────────────────────────────────────────────


def test_gate_with_evidence_and_a_measured_state_counts(tmp_path: Path):
    _package(tmp_path, [_gate("threshold.a", "met")])
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.mandatory_total == 1
    assert axis.measured_total == 1
    assert axis.ratio == 1.0
    assert axis.unmeasured == ()
    assert axis.gates[0].has_evidence is True
    assert "measured state" in axis.gates[0].detail


def test_accepted_residual_is_measured(tmp_path: Path):
    """`accepted_residual` is in the contract's measured set, so it counts."""
    _package(tmp_path, [_gate("h21", "accepted_residual")])
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.measured_total == 1
    assert axis.ratio == 1.0


def test_gate_without_evidence_is_unmeasured_even_in_a_measured_state(tmp_path: Path):
    """The M515-era lesson: a state alone is not a measurement."""
    _package(tmp_path, [_gate("threshold.a", "met", evidence=False)])
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.measured_total == 0
    assert axis.ratio == 0.0
    assert _ids(axis.unmeasured) == {"threshold.a"}
    assert "no evidence is bound" in axis.gates[0].detail


def test_gate_with_partial_evidence_is_unmeasured(tmp_path: Path):
    """A manifest without a pointer (or the reverse) is not a bound carrier."""
    gate = _gate("threshold.a", "met")
    gate["evidence"] = {"manifest": "", "pointer": "documents"}
    _package(tmp_path, [gate])
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.measured_total == 0
    assert axis.gates[0].has_evidence is False


def test_state_outside_measured_states_is_unmeasured(tmp_path: Path):
    """A state the contract does not declare as measured is not a measurement."""
    _package(tmp_path, [_gate("body", "not_assessable", blocks_s09=True)])
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.measured_total == 0
    assert _ids(axis.unmeasured) == {"body"}
    assert _ids(axis.unresolved) == {"body"}
    assert axis.dispositioned == ()


# ── the two halves of the unmeasured remainder ───────────────────────────────


def test_dispositioned_gate_is_unmeasured_but_not_blocking(tmp_path: Path):
    """`not_applicable` is non-blocking yet explicitly never a measurement."""
    _package(
        tmp_path,
        [_gate("required_scope.rest.isalive", "not_applicable", kind="required_family_downgrade")],
    )
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.measured_total == 0
    assert _ids(axis.unmeasured) == {"required_scope.rest.isalive"}
    assert axis.unresolved == ()
    assert _ids(axis.dispositioned) == {"required_scope.rest.isalive"}
    assert "dispositioned" in axis.gates[0].detail


def test_unresolved_and_dispositioned_partition_the_unmeasured_set(tmp_path: Path):
    _package(
        tmp_path,
        [
            _gate("threshold.met", "met"),
            _gate("h21", "accepted_residual"),
            _gate("required_scope.downgraded", "not_applicable", kind="required_family_downgrade"),
            _gate("h20", "pending", blocks_s09=True, kind="human_release"),
        ],
    )
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.mandatory_total == 4
    assert axis.measured_total == 2
    assert axis.ratio == 0.5
    assert _ids(axis.unmeasured) == {"required_scope.downgraded", "h20"}
    assert _ids(axis.unresolved) == {"h20"}
    assert _ids(axis.dispositioned) == {"required_scope.downgraded"}
    assert _ids(axis.unresolved) | _ids(axis.dispositioned) == _ids(axis.unmeasured)


# ── the gate list and the state vocabulary are read, not restated ─────────────


def test_measured_states_is_read_from_the_contract_not_hardcoded(tmp_path: Path):
    """A contract that measures a novel state moves the axis without a code change."""
    _package(tmp_path, [_gate("threshold.a", "sampled", kind="threshold")])
    _contract(tmp_path, measured_states=("sampled",), vocabulary=["sampled", "met"])
    axis = _axis(tmp_path)
    assert axis.measured_states == ("sampled",)
    assert axis.measured_total == 1
    # `met` is now NOT a measured state in this contract.
    _package(tmp_path, [_gate("threshold.a", "met")])
    assert _axis(tmp_path).measured_total == 0


def test_gate_list_is_read_from_the_package_not_a_duplicated_list(tmp_path: Path):
    """A synthetic two-gate package yields a two-gate axis, not the live 21."""
    _package(
        tmp_path,
        [_gate("threshold.a", "met"), _gate("threshold.b", "pending", blocks_s09=True)],
    )
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.mandatory_total == 2
    assert {g.gate_id for g in axis.gates} == {"threshold.a", "threshold.b"}


def test_removing_a_measured_gate_changes_the_ratio(tmp_path: Path):
    """The regression direction: losing a measured gate lowers the ratio."""
    _package(
        tmp_path,
        [
            _gate("threshold.a", "met"),
            _gate("threshold.b", "met"),
            _gate("required_scope.c", "not_applicable", kind="required_family_downgrade"),
        ],
    )
    _contract(tmp_path)
    before = _axis(tmp_path)
    assert before.ratio == 2 / 3

    _package(
        tmp_path,
        [
            _gate("threshold.a", "met"),
            _gate("required_scope.c", "not_applicable", kind="required_family_downgrade"),
        ],
    )
    after = _axis(tmp_path)
    assert after.measured_total == 1
    assert after.mandatory_total == 2
    assert after.ratio == 0.5
    assert after.ratio < before.ratio


def test_non_required_gate_is_outside_the_axis(tmp_path: Path):
    """The denominator is the mandatory set, so a non-required gate is excluded."""
    _package(
        tmp_path,
        [_gate("threshold.a", "met"), _gate("optional.b", "pending", required=False)],
    )
    _contract(tmp_path)
    axis = _axis(tmp_path)
    assert axis.mandatory_total == 1
    assert {g.gate_id for g in axis.gates} == {"threshold.a"}


# ── fail closed ──────────────────────────────────────────────────────────────


def test_missing_package_fails_loudly(tmp_path: Path):
    _contract(tmp_path)
    with pytest.raises(FileNotFoundError):
        _axis(tmp_path)


def test_missing_contract_fails_loudly(tmp_path: Path):
    _package(tmp_path, [_gate("threshold.a", "met")])
    with pytest.raises(FileNotFoundError):
        _axis(tmp_path)


def test_missing_measured_states_fails_loudly(tmp_path: Path):
    _package(tmp_path, [_gate("threshold.a", "met")])
    _contract(tmp_path, measured_states=None)
    with pytest.raises(ValueError):
        _axis(tmp_path)


def test_empty_gate_list_fails_loudly(tmp_path: Path):
    _package(tmp_path, [])
    _contract(tmp_path)
    with pytest.raises(ValueError):
        _axis(tmp_path)


def test_malformed_gate_fails_loudly(tmp_path: Path):
    _package(tmp_path, [{"state": "met", "required": True, "blocks_s09": False}])
    _contract(tmp_path)
    with pytest.raises(ValueError):
        _axis(tmp_path)


def test_state_outside_the_declared_vocabulary_fails_loudly(tmp_path: Path):
    _package(tmp_path, [_gate("threshold.a", "invented")])
    _contract(tmp_path)
    with pytest.raises(ValueError):
        _axis(tmp_path)


def test_a_contract_without_a_vocabulary_is_accepted(tmp_path: Path):
    """The closed vocabulary is an optional guard; measured_states is required."""
    _package(tmp_path, [_gate("threshold.a", "met")])
    _contract(tmp_path, vocabulary=None)
    axis = _axis(tmp_path)
    assert axis.measured_total == 1


# ── the machine-readable payload ─────────────────────────────────────────────


def test_to_dict_names_each_unmeasured_gate_and_partitions_it(tmp_path: Path):
    _package(
        tmp_path,
        [
            _gate("threshold.a", "met"),
            _gate("h20", "pending", blocks_s09=True, kind="human_release"),
            _gate("required_scope.c", "not_applicable", kind="required_family_downgrade"),
        ],
    )
    _contract(tmp_path)
    data = _axis(tmp_path).to_dict()
    assert data["axis"] == "measurability"
    assert data["measured_states"] == ["met", "accepted_residual"]
    assert data["mandatory_gates"] == 3
    assert data["measured_gates"] == 1
    assert data["unmeasured_count"] == 2
    assert set(data["unmeasured_gate_ids"]) == {"h20", "required_scope.c"}
    assert data["unresolved_gate_ids"] == ["h20"]
    assert data["dispositioned_gate_ids"] == ["required_scope.c"]
    assert {g["gate_id"] for g in data["gates"]} == {"threshold.a", "h20", "required_scope.c"}


def test_render_lines_state_the_ratio_and_name_the_remainder(tmp_path: Path):
    _package(
        tmp_path,
        [
            _gate("threshold.a", "met"),
            _gate("h20", "pending", blocks_s09=True, kind="human_release"),
        ],
    )
    _contract(tmp_path)
    rendered = "\n".join(_axis(tmp_path).render_lines())
    assert "measurability: 1/2" in rendered
    assert "measured states (from the contract): met, accepted_residual" in rendered
    assert "h20" in rendered
    assert "unresolved/blocking (1): h20" in rendered


# ── real tree ────────────────────────────────────────────────────────────────


def _real_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


def test_real_tree_reports_the_scorecard_ratio_and_names_the_remainder():
    """The live axis is 5/21; the unmeasured set is 16, its unresolved half is 4.

    The remaining-4 assertion is the hand-checkable one: the unresolved names
    must equal the package's own ``blocking_gates`` list, so the axis states
    exactly what the package still blocks on rather than a prose summary.
    """
    axis = observe_measurability_axis()
    assert axis.mandatory_total == 21
    assert axis.measured_total == 5
    assert axis.ratio == pytest.approx(5 / 21)
    assert axis.measured_states == ("met", "accepted_residual")

    package = json.loads((_real_root() / _PACKAGE_PATH).read_text(encoding="utf-8"))
    blocking_ids = {gate["gate_id"] for gate in package["blocking_gates"]}
    dispositioned_ids = {
        gate["gate_id"]
        for gate in package["mandatory_gates"]
        if gate["state"] == "not_applicable"
    }

    unmeasured_ids = _ids(axis.unmeasured)
    assert len(unmeasured_ids) == 16
    assert _ids(axis.unresolved) == blocking_ids
    assert len(blocking_ids) == 4
    assert _ids(axis.dispositioned) == dispositioned_ids
    assert unmeasured_ids == blocking_ids | dispositioned_ids

    assert _ids(axis.measured) <= {
        gate["gate_id"] for gate in package["mandatory_gates"]
    }
    assert all(g.state in axis.measured_states for g in axis.measured)
    assert all(g.has_evidence for g in axis.measured)
    assert all(g.state not in axis.measured_states for g in axis.unmeasured)

    rendered = axis.render_lines()
    assert rendered and rendered[0].startswith("measurability: 5/21")


def test_real_tree_measured_gates_are_the_met_and_residual_gates():
    """Every live measured gate is genuinely `met` or `accepted_residual`."""
    axis = observe_measurability_axis()
    package = json.loads((_real_root() / _PACKAGE_PATH).read_text(encoding="utf-8"))
    by_id = {gate["gate_id"]: gate for gate in package["mandatory_gates"]}
    for gate in axis.measured:
        assert by_id[gate.gate_id]["state"] in ("met", "accepted_residual")
        assert by_id[gate.gate_id]["evidence"]["manifest"] == gate.evidence_manifest
