"""ACAP admission policy DATA shape contract (S07/T01).

The admission semantics the runtime executes and the governor validates must be
*one* declaration, not two: the ordered ``fsm.states`` list plus the
``admission`` block in ``scripts/governor/checks/article_context_assurance.yaml``.
T02/T03/T04 read that declaration (pure ``da-domain`` policy value, ``da-adapters``
loader, governor rule); this module pins the DATA shape those readers depend on,
so a weakened or reordered declaration fails here by name instead of silently
producing a different admission decision on each side.

It also pins the shared oracle: ``admission_parity.vectors.json`` is consumed by a
Rust test (T03) and a Python test (T04); the ten required cases and their expected
``admitted``/``code`` are fixed here so the two languages cannot drift.

This is a pure DATA shape test -- it does not import the governor and never runs
admission logic.
"""

from __future__ import annotations

import json
from pathlib import Path

import yaml

_REPO_ROOT = Path(__file__).resolve().parents[2]
_FIXTURES = "governor/fixtures/acap_runtime_control_plane"
_POLICY_REL = "scripts/governor/checks/article_context_assurance.yaml"
_PARITY_REL = f"{_FIXTURES}/admission_parity.vectors.json"
_POSITIVE_REL = f"{_FIXTURES}/admission_legal.positive.json"
_NEGATIVE_REL = f"{_FIXTURES}/admission_illegal.negative.json"

# The runtime legality semantics the declaration must carry (T01 Do step 1).
_EXPECTED_ADMISSION = {
    "semantics_version": "acap-admission-v1",
    "entry_states": ["source_registered", "source_verified", "parsed"],
    "forward_rule": "monotone_subset",
    "same_layer_repeat": "allowed",
    "failure_sink": "failed_visible",
    "evidence_required": True,
    "skip_requires_typed_reason": True,
}

# The ten required parity cases and the exact decision both sides must reach.
# ``None`` code means "admitted with no typed error code".
_EXPECTED_PARITY = {
    "entry_parsed": (True, None),
    "entry_mid_fsm": (False, "illegal_transition"),
    "entry_no_evidence": (False, "missing_entry_state"),
    "forward_skip": (True, None),
    "forward_skip_no_reason": (False, "missing_skip_reason"),
    "same_layer_repeat": (True, None),
    "failure_sink": (True, None),
    "regression": (False, "illegal_transition"),
    "missing_evidence": (False, "missing_evidence"),
    "terminal_sink": (False, "terminal_sink"),
}

_REQUIRED_PARITY_KEYS = {
    "case",
    "prior",
    "target",
    "evidence_ref",
    "skip_reason",
    "admitted",
    "code",
}


def _policy() -> dict:
    return yaml.safe_load((_REPO_ROOT / _POLICY_REL).read_text(encoding="utf-8")) or {}


def _records(rel: str) -> list:
    return json.loads((_REPO_ROOT / rel).read_text(encoding="utf-8"))


def _rank(states: list[str], state: str) -> int:
    return states.index(state)


# ── the single declaration ──────────────────────────────────────────────────


def test_admission_block_declares_the_runtime_semantics() -> None:
    policy = _policy()
    admission = policy.get("admission")
    assert isinstance(admission, dict), "article_context_assurance.yaml must declare an `admission` block"
    for key, expected in _EXPECTED_ADMISSION.items():
        assert admission.get(key) == expected, f"admission.{key} must be {expected!r}"


def test_admission_block_is_the_only_monotone_declaration() -> None:
    policy = _policy()
    # The rule lives in exactly one place: the `admission` block. It must not be
    # restated by the fsm block (a second copy is how the two sides diverged).
    fsm = policy["fsm"]
    assert fsm.get("skip_requires_typed_reason") is None, (
        "fsm.skip_requires_typed_reason is superseded by admission.skip_requires_typed_reason; "
        "one truth only"
    )
    # The layer order/rank is the ordered fsm.states list; admission must not
    # duplicate it.
    assert "states" not in policy["admission"]
    assert "order" not in policy["admission"]


def test_admission_references_are_declared_fsm_states() -> None:
    fsm = _policy()["fsm"]
    states = [str(state) for state in fsm["states"]]
    admission = _policy()["admission"]

    for entry in admission["entry_states"]:
        assert entry in states, f"admission.entry_states member {entry!r} is not an fsm state"
    assert admission["failure_sink"] in states, "admission.failure_sink must be a declared fsm state"
    # The failure sink is the same sink the fsm block declares, so the runtime
    # and the governor cannot disagree about where failure lands.
    assert admission["failure_sink"] == fsm["failure_transition_to"]


def test_fsm_declares_fifteen_unique_states() -> None:
    states = [str(state) for state in _policy()["fsm"]["states"]]
    assert len(states) == 15, f"expected 15 ACAP states, got {len(states)}: {states}"
    assert len(set(states)) == len(states), "ACAP fsm.states must be unique"


# ── the shared parity oracle ────────────────────────────────────────────────


def test_parity_vectors_cover_every_required_case() -> None:
    cases = [str(record.get("case")) for record in _records(_PARITY_REL)]
    assert len(cases) == len(set(cases)), "parity vector cases must be unique"
    missing = sorted(set(_EXPECTED_PARITY) - set(cases))
    assert not missing, f"parity vectors missing required cases: {missing}"


def test_parity_vector_records_have_the_shared_shape() -> None:
    for record in _records(_PARITY_REL):
        assert set(record) == _REQUIRED_PARITY_KEYS, (
            f"parity case {record.get('case')!r} must carry exactly {sorted(_REQUIRED_PARITY_KEYS)}"
        )
        assert isinstance(record["admitted"], bool)
        assert isinstance(record["evidence_ref"], str)
        assert record["code"] is None or isinstance(record["code"], str)
        assert record["skip_reason"] is None or isinstance(record["skip_reason"], str)


def test_parity_vectors_reference_declared_states() -> None:
    states = {str(state) for state in _policy()["fsm"]["states"]}
    for record in _records(_PARITY_REL):
        assert record["target"] in states, (
            f"parity case {record['case']!r} target {record['target']!r} is not a declared state"
        )
        prior = record["prior"]
        if prior is not None:
            assert prior in states, (
                f"parity case {record['case']!r} prior {prior!r} is not a declared state"
            )


def test_parity_vectors_pin_the_expected_decision() -> None:
    vectors = {str(record["case"]): record for record in _records(_PARITY_REL)}
    for case, (admitted, code) in _EXPECTED_PARITY.items():
        record = vectors[case]
        assert (record["admitted"], record["code"]) == (admitted, code), (
            f"parity case {case!r} must be admitted={admitted} code={code!r}; "
            f"got admitted={record['admitted']} code={record['code']!r}"
        )
    # A forward layer skip is the divergence G1: it is admitted, and it must be
    # admitted with a typed skip reason available to the recorder.
    forward = vectors["forward_skip"]
    assert forward["prior"] == "parsed" and forward["target"] == "summarized"
    assert forward["skip_reason"], "forward_skip must carry a non-empty skip_reason"


# ── the governor fixtures that exercise the decision ────────────────────────


def test_negative_fixture_illegal_transition_is_a_regression() -> None:
    states = [str(state) for state in _policy()["fsm"]["states"]]
    records = {str(record.get("violation")): record["record"] for record in _records(_NEGATIVE_REL)}
    illegal = records["illegal_transition"]
    # source_registered -> summarized is now legal (monotone forward skip), so the
    # illegal_transition fixture must be a genuine FSM regression to stay negative.
    assert _rank(states, illegal["to_state"]) < _rank(states, illegal["from_state"]), (
        "illegal_transition fixture must regress the FSM (rank(to) < rank(from))"
    )
    assert illegal["admitted"] is False
    assert illegal["evidence_ref"], "the regression record must carry durable evidence"
    assert illegal["error_class"] == "admission:illegal_transition"


def test_negative_fixture_declares_missing_skip_reason() -> None:
    records = {str(record.get("violation")): record["record"] for record in _records(_NEGATIVE_REL)}
    assert "missing_skip_reason" in records, (
        "the negative fixture must carry a missing_skip_reason record so T04 can require it"
    )
    record = records["missing_skip_reason"]
    assert record["from_state"] == "parsed" and record["to_state"] == "summarized"
    assert record["admitted"] is True
    assert record["evidence_ref"], "the skip record must carry durable evidence"
    assert "skip_reason" not in record, "the record must lack a typed skip reason to trigger the code"


def test_positive_fixture_admits_forward_skip_repeat_and_sink() -> None:
    records = _records(_POSITIVE_REL)
    by_shape = {(record["from_state"], record["to_state"]): record for record in records}
    assert records and all(record["admitted"] is True for record in records), (
        "every admission_legal positive record must be admitted"
    )
    forward = by_shape[("parsed", "summarized")]
    assert forward.get("skip_reason"), "the forward-skip positive record must carry a skip_reason"
    assert ("parsed", "parsed") in by_shape, "the positive fixture must cover a same-layer repeat"
    assert ("parsed", "failed_visible") in by_shape, "the positive fixture must cover the failure sink"
