"""Adapter: measurability axis.

The first axis of the M516 scorecard answers the milestone's own question with a
number: *how many of the mandatory acceptance gates carry a measured
mechanism?* The gate ledger declares 21 mandatory gates; a gate carries a
measurement only when two things hold at once:

* it carries evidence bound to a **manifest** and a **pointer**, and
* its **state** is one the contract declares in ``measured_states``.

The second clause is what makes the axis honest rather than merely green. After
M516-th0oby/S05 every gate in the package carries evidence, so evidence alone
cannot separate a measured gate from a dispositioned one; and S05 recorded the
twelve ``required_scope.*`` families as ``required_family_downgrade`` /
``not_applicable``, a *disposition* that is non-blocking but explicitly "never a
measurement". The contract therefore keeps two vocabularies side by side:

.. code-block:: yaml

    non_blocking_states: [met, accepted_residual, not_applicable]
    measured_states: [met, accepted_residual]   # the strict subset

Reading ``measured_states`` from the contract — never hardcoding ``met`` and
``accepted_residual`` — is what lets the downgrade decision move the blocking
set without silently moving the measurement ratio.

What this adapter reports
-------------------------
For every ``required`` gate in the acceptance package's ``mandatory_gates``
list it reports whether the gate is measured, and it names each unmeasured gate
individually, partitioned into:

* **unresolved** — the gate still blocks S09 (``blocks_s09`` true); on the live
  tree this set is exactly the package's own ``blocking_gates`` list, so the
  axis states precisely what remains, and
* **dispositioned** — the gate is non-blocking yet not a measurement (e.g. a
  recorded ``not_applicable`` downgrade); counted as unmeasured, named
  separately so the ratio cannot be inflated by a disposition.

The axis value is ``measured / mandatory`` — a regression is a decrease, and the
named remainder makes the number actionable. The gate list is read from
``artifacts/parse_compat/grobid_retirement_acceptance_package.json`` and the
state vocabulary from
``scripts/governor/checks/grobid_absorption_progress.yaml``; neither is restated
here, so removing a gate from the package changes the ratio instead of hiding
behind a duplicated list.

Fail-closed: a missing package, a contract without ``measured_states``, an empty
gate list, or a malformed gate (no id, no state, a state outside the declared
vocabulary) raises rather than reporting a vacuous green axis.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

import yaml

__all__ = [
    "MandatoryGate",
    "MeasurabilityAxis",
    "build_measurability_axis",
    "observe_measurability_axis",
]

#: The acceptance package whose ``mandatory_gates`` list is the axis denominator.
_PACKAGE_REL = "artifacts/parse_compat/grobid_retirement_acceptance_package.json"
#: The DATA contract that declares which gate states count as a measurement.
_CONTRACT_REL = "scripts/governor/checks/grobid_absorption_progress.yaml"

#: Where the contract declares which gate states count as a measurement.
_MEASURED_STATES_KEY: tuple[str, ...] = ("acceptance_package", "expected", "measured_states")
#: The closed gate-state vocabulary, when the contract declares one.
_STATE_VOCABULARY_KEY: tuple[str, ...] = (
    "acceptance_package",
    "expected",
    "gate_state_vocabulary",
)
#: The mandatory gate list, read from the package rather than restated.
_GATES_KEY = "mandatory_gates"


# ── input loading ────────────────────────────────────────────────────────────


def _require_file(root: Path, rel: str) -> Path:
    path = root / rel
    if not path.is_file():
        raise FileNotFoundError(
            f"{rel} is missing under {root}; refusing to report a vacuous "
            "measurability green"
        )
    return path


def _load_json(root: Path, rel: str) -> dict:
    path = _require_file(root, rel)
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - malformed fixture
        raise ValueError(f"{rel} is not valid JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise ValueError(f"{rel} must parse to an object, got {type(data).__name__}")
    return data


def _load_yaml(root: Path, rel: str) -> dict:
    path = _require_file(root, rel)
    try:
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as exc:  # pragma: no cover - malformed fixture
        raise ValueError(f"{rel} is not valid YAML: {exc}") from exc
    if not isinstance(data, dict):
        raise ValueError(f"{rel} must parse to a mapping, got {type(data).__name__}")
    return data


def _dig(data: dict, keys: Sequence[str], rel: str):
    node = data
    for key in keys:
        if not isinstance(node, dict) or key not in node:
            raise ValueError(f"{rel} has no {'.'.join(keys)}")
        node = node[key]
    return node


def _text_list(node, keys: Sequence[str], rel: str) -> tuple[str, ...]:
    """A non-empty list of non-empty strings, or a loud failure."""
    label = ".".join(keys)
    if not isinstance(node, list) or not node:
        raise ValueError(f"{label} in {rel} must be a non-empty list of strings")
    out: list[str] = []
    for item in node:
        if not isinstance(item, str) or not item.strip():
            raise ValueError(f"{label} in {rel} must contain only non-empty strings")
        out.append(item)
    return tuple(out)


# ── the axis ─────────────────────────────────────────────────────────────────


@dataclass(frozen=True)
class MandatoryGate:
    """One required acceptance gate, measured or not."""

    gate_id: str
    kind: str
    state: str
    required: bool
    blocks_s09: bool
    evidence_manifest: str
    evidence_pointer: str
    has_evidence: bool
    measured: bool
    detail: str

    @property
    def name(self) -> str:
        return self.gate_id

    def to_dict(self) -> dict:
        return {
            "gate_id": self.gate_id,
            "kind": self.kind,
            "state": self.state,
            "required": self.required,
            "blocks_s09": self.blocks_s09,
            "evidence_manifest": self.evidence_manifest,
            "evidence_pointer": self.evidence_pointer,
            "has_evidence": self.has_evidence,
            "measured": self.measured,
            "unmeasured": not self.measured,
            "detail": self.detail,
        }


def _evidence_binding(raw: dict) -> tuple[str, str]:
    """The (manifest, pointer) a gate's evidence binds, if any."""
    evidence = raw.get("evidence")
    if not isinstance(evidence, dict):
        return "", ""
    manifest = evidence.get("manifest")
    pointer = evidence.get("pointer")
    return (
        manifest if isinstance(manifest, str) else "",
        pointer if isinstance(pointer, str) else "",
    )


def _gate_from_raw(
    raw,
    measured_states: frozenset[str],
    vocabulary: frozenset[str] | None,
    rel: str,
) -> MandatoryGate:
    if not isinstance(raw, dict):
        raise ValueError(f"a gate in {rel} is not an object")
    gate_id = raw.get("gate_id")
    if not isinstance(gate_id, str) or not gate_id.strip():
        raise ValueError(f"a gate in {rel} has no gate_id")
    state = raw.get("state")
    if not isinstance(state, str) or not state.strip():
        raise ValueError(f"gate {gate_id} in {rel} has no state")
    if vocabulary is not None and state not in vocabulary:
        raise ValueError(
            f"gate {gate_id} state {state!r} is not in the contract's declared "
            "gate_state_vocabulary"
        )
    required = raw.get("required")
    if not isinstance(required, bool):
        raise ValueError(f"gate {gate_id} in {rel} has no boolean required")
    blocks = raw.get("blocks_s09")
    if not isinstance(blocks, bool):
        raise ValueError(f"gate {gate_id} in {rel} has no boolean blocks_s09")
    kind = raw.get("kind")
    kind = kind if isinstance(kind, str) else ""

    manifest, pointer = _evidence_binding(raw)
    has_evidence = bool(manifest.strip()) and bool(pointer.strip())
    measured = required and has_evidence and state in measured_states

    if measured:
        detail = (
            f"state {state!r} is a measured state and evidence is bound to "
            f"{manifest}#{pointer}"
        )
    elif not has_evidence:
        detail = (
            f"state {state!r} "
            f"{'is' if state in measured_states else 'is not'} a measured state, but no "
            "evidence is bound to a manifest and a pointer"
        )
    elif blocks:
        detail = (
            f"state {state!r} is not a measured state and the gate still blocks S09 "
            "(unresolved)"
        )
    else:
        detail = (
            f"state {state!r} is not a measured state; the gate is dispositioned "
            "(non-blocking) rather than measured"
        )

    return MandatoryGate(
        gate_id=gate_id,
        kind=kind,
        state=state,
        required=required,
        blocks_s09=blocks,
        evidence_manifest=manifest,
        evidence_pointer=pointer,
        has_evidence=has_evidence,
        measured=measured,
        detail=detail,
    )


@dataclass(frozen=True)
class MeasurabilityAxis:
    """The measurability axis: measured mandatory gates over all mandatory gates."""

    root: str
    measured_states: tuple[str, ...]
    gates: tuple[MandatoryGate, ...]

    @property
    def mandatory_total(self) -> int:
        return len(self.gates)

    @property
    def measured_total(self) -> int:
        return sum(1 for g in self.gates if g.measured)

    @property
    def measured(self) -> tuple[MandatoryGate, ...]:
        return tuple(g for g in self.gates if g.measured)

    @property
    def unmeasured(self) -> tuple[MandatoryGate, ...]:
        return tuple(g for g in self.gates if not g.measured)

    @property
    def unresolved(self) -> tuple[MandatoryGate, ...]:
        """Unmeasured gates that still block S09 — the package's blocking set."""
        return tuple(g for g in self.unmeasured if g.blocks_s09)

    @property
    def dispositioned(self) -> tuple[MandatoryGate, ...]:
        """Unmeasured gates that are non-blocking but explicitly not a measurement."""
        return tuple(g for g in self.unmeasured if not g.blocks_s09)

    @property
    def ratio(self) -> float:
        if not self.gates:
            return 0.0
        return self.measured_total / self.mandatory_total

    def to_dict(self) -> dict:
        return {
            "axis": "measurability",
            "definition": (
                "mandatory acceptance gates that carry a measured mechanism over all "
                "mandatory gates. A gate is measured when it carries evidence bound to "
                "a manifest and a pointer and its state is one the contract declares in "
                "measured_states; a gate whose state is not a measured state is "
                "unmeasured, whether it is dispositioned (non-blocking but not a "
                "measurement) or still unresolved (blocking). The gate list is read "
                "from the acceptance package, never restated."
            ),
            "root": self.root,
            "measured_states": list(self.measured_states),
            "mandatory_gates": self.mandatory_total,
            "measured_gates": self.measured_total,
            "ratio": self.ratio,
            "unmeasured_count": len(self.unmeasured),
            "unmeasured_gate_ids": [g.gate_id for g in self.unmeasured],
            "unresolved_count": len(self.unresolved),
            "unresolved_gate_ids": [g.gate_id for g in self.unresolved],
            "dispositioned_count": len(self.dispositioned),
            "dispositioned_gate_ids": [g.gate_id for g in self.dispositioned],
            "gates": [g.to_dict() for g in self.gates],
        }

    def render_lines(self) -> list[str]:
        """The axis as the lines a session reads first."""
        pct = self.ratio * 100.0
        lines = [
            f"measurability: {self.measured_total}/{self.mandatory_total} "
            f"({pct:.1f}%) mandatory gates carry a measured mechanism",
            f"  measured states (from the contract): {', '.join(self.measured_states)}",
        ]
        unmeasured = self.unmeasured
        if not unmeasured:
            lines.append("  unmeasured: none")
            return lines
        lines.append(
            f"  unmeasured ({len(unmeasured)}): "
            + ", ".join(g.gate_id for g in unmeasured)
        )
        lines.append(
            f"  unresolved/blocking ({len(self.unresolved)}): "
            + (", ".join(g.gate_id for g in self.unresolved) or "none")
        )
        lines.append(
            f"  dispositioned/non-blocking but not measured ({len(self.dispositioned)}): "
            + (", ".join(g.gate_id for g in self.dispositioned) or "none")
        )
        return lines


def build_measurability_axis(root: Path) -> MeasurabilityAxis:
    """Compute the axis for ``root`` from the package and the DATA contract.

    The gate list comes from the package's ``mandatory_gates`` and the state
    vocabulary from the contract's ``acceptance_package.expected`` block. Raises
    when either source is missing, when the contract does not declare
    ``measured_states``, or when the package carries no required gate: an empty
    scan would otherwise report a vacuous ``0/0`` and hide a bad root.
    """
    root = Path(root)
    package = _load_json(root, _PACKAGE_REL)
    contract = _load_yaml(root, _CONTRACT_REL)

    measured_state_list = _text_list(
        _dig(contract, _MEASURED_STATES_KEY, _CONTRACT_REL), _MEASURED_STATES_KEY, _CONTRACT_REL
    )
    measured_states = frozenset(measured_state_list)
    try:
        vocabulary = frozenset(
            _text_list(
                _dig(contract, _STATE_VOCABULARY_KEY, _CONTRACT_REL),
                _STATE_VOCABULARY_KEY,
                _CONTRACT_REL,
            )
        )
    except ValueError:
        # The closed vocabulary is an optional guard; measured_states is required.
        vocabulary = None

    raw_gates = _dig(package, (_GATES_KEY,), _PACKAGE_REL)
    if not isinstance(raw_gates, list) or not raw_gates:
        raise ValueError(f"{_PACKAGE_REL} has no non-empty {_GATES_KEY} list")

    gates: list[MandatoryGate] = []
    for raw in raw_gates:
        gate = _gate_from_raw(raw, measured_states, vocabulary, _PACKAGE_REL)
        if gate.required:
            gates.append(gate)
    if not gates:
        raise ValueError(
            f"{_PACKAGE_REL} carries no required mandatory gate; refusing to report a "
            "vacuous measurability axis"
        )

    return MeasurabilityAxis(
        root=str(root),
        measured_states=measured_state_list,
        gates=tuple(gates),
    )


def observe_measurability_axis(root: Path | None = None) -> MeasurabilityAxis:
    """Observe the axis on the real tree (or a given root)."""
    if root is None:
        root = Path(__file__).resolve().parent.parent.parent.parent
    return build_measurability_axis(Path(root))
