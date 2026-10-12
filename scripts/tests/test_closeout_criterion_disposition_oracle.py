"""Tests for the closeout-criterion disposition oracle (M516-th0oby/S13/T02).

The registry (S13/T01) is DATA; this oracle is the pure, independently
testable reader that turns (registry, live high flags) into findings. These
tests pin its fail-closed contract:

  - a missing or malformed registry RAISES instead of reporting a vacuous
    "all disposed" green criterion;
  - a live high class with no record is `undisposed`; a live class whose
    record says `measured` is a `contradiction` (a stale clear); a
    well-formed `downgraded` record is silent;
  - a record that cannot be trusted (not a mapping, missing/empty required
    field, unknown disposition, unknown reason) is `malformed`;
  - keying is by flag CLASS (the id before any `@` suffix) and only
    `required_for_severities` flags are evaluated;
  - the result is deterministic (sorted by class), concatenates in a stable
    order via `findings`, and never mutates its inputs.

Run with: PYTHONPATH=scripts .venv/bin/python -m pytest -q scripts/tests/test_closeout_criterion_disposition_oracle.py
"""

import sys
from pathlib import Path

import pytest
import yaml

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from governor.adapters.closeout_criterion_disposition import (
    CLOSEOUT_CRITERION_UNDISPOSED,
    DISPOSITION_VOCABULARY,
    build_closeout_criterion_disposition,
    observe_closeout_criterion_disposition,
)
from governor.domain.trajectory import DriftFlag

_REPO_ROOT = _SCRIPTS_DIR.parent
_REGISTRY_RELATIVE = "scripts/governor/checks/closeout_criterion_dispositions.yaml"

_REASON_VOCABULARY = [
    "pre_existing_structural",
    "pre_existing_environment",
    "severity_environment_dependent",
    "governor_process_finding",
]


def _document(records: dict) -> dict:
    """A minimal well-formed registry document, shaped like the live DATA."""
    return {
        "schema_version": "closeout-criterion-dispositions.v1",
        "disposition_vocabulary": ["measured", "downgraded"],
        "reason_vocabulary": list(_REASON_VOCABULARY),
        "required_record_fields": [
            "disposition",
            "owner",
            "reason",
            "decision_ref",
            "rationale",
        ],
        "required_for_severities": ["high"],
        "records": dict(records),
    }


def _downgraded(**overrides) -> dict:
    record = {
        "disposition": "downgraded",
        "owner": "owner",
        "reason": "pre_existing_structural",
        "decision_ref": "D572",
        "rationale": "a recorded downgrade with an owner and a reason",
    }
    record.update(overrides)
    return record


def _measured(**overrides) -> dict:
    record = {
        "disposition": "measured",
        "owner": "owner",
        "reason": "pre_existing_structural",
        "decision_ref": "D572",
        "rationale": "the class is absent from the live report",
    }
    record.update(overrides)
    return record


def _write_registry(root: Path, document) -> None:
    path = Path(root) / _REGISTRY_RELATIVE
    path.parent.mkdir(parents=True, exist_ok=True)
    text = (
        document
        if isinstance(document, str)
        else yaml.safe_dump(document, sort_keys=False)
    )
    path.write_text(text, encoding="utf-8")


def _build(root: Path, flags):
    return build_closeout_criterion_disposition(root, flags)


# ── the module contract ──────────────────────────────────────────────────────

def test_exported_constants_pin_the_finding_class_and_vocabulary() -> None:
    assert CLOSEOUT_CRITERION_UNDISPOSED == "closeout_criterion_undisposed"
    assert DISPOSITION_VOCABULARY == frozenset({"measured", "downgraded"})


# ── fail-closed: the registry is a REQUIRED input ────────────────────────────

def test_missing_registry_raises_file_not_found_error(tmp_path: Path) -> None:
    """A deleted registry must raise, never read as a vacuous all-disposed zero."""
    assert not (tmp_path / _REGISTRY_RELATIVE).exists()
    with pytest.raises(FileNotFoundError) as excinfo:
        _build(tmp_path, [("orphan_class", "high")])
    assert _REGISTRY_RELATIVE in str(excinfo.value)


def test_non_mapping_yaml_raises_value_error(tmp_path: Path) -> None:
    _write_registry(tmp_path, "- just\n- a list\n")
    with pytest.raises(ValueError):
        _build(tmp_path, [("orphan_class", "high")])


def test_disposition_vocabulary_that_is_not_a_list_raises_value_error(
    tmp_path: Path,
) -> None:
    document = _document({})
    document["disposition_vocabulary"] = "measured"
    _write_registry(tmp_path, document)
    with pytest.raises(ValueError):
        _build(tmp_path, [("orphan_class", "high")])


def test_records_block_that_is_not_a_mapping_raises_value_error(
    tmp_path: Path,
) -> None:
    document = _document({})
    document["records"] = ["not", "a", "mapping"]
    _write_registry(tmp_path, document)
    with pytest.raises(ValueError):
        _build(tmp_path, [("orphan_class", "high")])


# ── the three findings ───────────────────────────────────────────────────────

def test_live_high_class_without_record_yields_one_undisposed_finding(
    tmp_path: Path,
) -> None:
    _write_registry(tmp_path, _document({"owned_class": _downgraded()}))
    result = _build(tmp_path, [("orphan_class", "high")])
    assert len(result.undisposed) == 1
    finding = result.undisposed[0]
    assert finding.flag_class == "orphan_class"
    assert finding.kind == "undisposed"
    assert "orphan_class" in finding.detail
    assert not result.is_silent


def test_live_high_class_with_downgraded_record_is_silent(tmp_path: Path) -> None:
    _write_registry(tmp_path, _document({"owned_class": _downgraded()}))
    result = _build(tmp_path, [("owned_class", "high")])
    assert result.findings == ()
    assert result.is_silent


def test_live_high_class_with_measured_record_yields_one_contradiction(
    tmp_path: Path,
) -> None:
    """A `measured` record for a class that is live is a stale clear."""
    _write_registry(tmp_path, _document({"live_class": _measured()}))
    result = _build(tmp_path, [("live_class", "high")])
    assert len(result.contradictions) == 1
    finding = result.contradictions[0]
    assert finding.flag_class == "live_class"
    assert finding.kind == "contradiction"
    assert "measured" in finding.detail


# ── malformed records: cannot be trusted either way ──────────────────────────

def test_record_missing_a_required_field_is_malformed(tmp_path: Path) -> None:
    record = _downgraded()
    del record["decision_ref"]
    _write_registry(tmp_path, _document({"sloppy_class": record}))
    result = _build(tmp_path, [("sloppy_class", "high")])
    assert len(result.malformed) == 1
    finding = result.malformed[0]
    assert finding.kind == "malformed"
    assert "decision_ref" in finding.detail


def test_record_with_empty_owner_is_malformed(tmp_path: Path) -> None:
    _write_registry(
        tmp_path, _document({"sloppy_class": _downgraded(owner="   ")})
    )
    result = _build(tmp_path, [("sloppy_class", "high")])
    assert len(result.malformed) == 1
    assert "owner" in result.malformed[0].detail


def test_record_with_unknown_reason_is_malformed(tmp_path: Path) -> None:
    _write_registry(
        tmp_path,
        _document({"sloppy_class": _downgraded(reason="because_i_said_so")}),
    )
    result = _build(tmp_path, [("sloppy_class", "high")])
    assert len(result.malformed) == 1
    assert "because_i_said_so" in result.malformed[0].detail


def test_record_with_unknown_disposition_is_malformed(tmp_path: Path) -> None:
    _write_registry(
        tmp_path, _document({"sloppy_class": _downgraded(disposition="cleared")})
    )
    result = _build(tmp_path, [("sloppy_class", "high")])
    assert len(result.malformed) == 1
    assert "cleared" in result.malformed[0].detail


def test_record_that_is_not_a_mapping_is_malformed(tmp_path: Path) -> None:
    """A null record is present but not a mapping: malformed, not undisposed."""
    _write_registry(tmp_path, _document({"null_class": None}))
    result = _build(tmp_path, [("null_class", "high")])
    assert len(result.malformed) == 1
    assert result.undisposed == ()
    assert "not a mapping" in result.malformed[0].detail


def test_many_causes_collapse_into_one_malformed_finding_per_class(
    tmp_path: Path,
) -> None:
    """One finding per class, with every cause named in its detail."""
    _write_registry(tmp_path, _document({"sloppy_class": {"disposition": "cleared"}}))
    result = _build(tmp_path, [("sloppy_class", "high")])
    assert len(result.malformed) == 1
    detail = result.malformed[0].detail
    assert "cleared" in detail and "owner" in detail and "rationale" in detail


# ── class keying and severity scoping ────────────────────────────────────────

def test_parameterized_id_resolves_through_class_key(tmp_path: Path) -> None:
    """`some_class@M516` must resolve through the record keyed `some_class`."""
    _write_registry(tmp_path, _document({"some_class": _downgraded()}))
    result = _build(tmp_path, [("some_class@M516", "high")])
    assert result.is_silent


def test_medium_flag_is_not_evaluated(tmp_path: Path) -> None:
    """Only `required_for_severities` fails closed; a medium flag may be free."""
    _write_registry(tmp_path, _document({}))
    result = _build(tmp_path, [("medium_orphan", "medium")])
    assert result.is_silent


def test_mixed_severities_evaluate_only_the_required_ones(tmp_path: Path) -> None:
    _write_registry(tmp_path, _document({}))
    result = _build(
        tmp_path, [("high_orphan", "high"), ("medium_orphan", "medium")]
    )
    assert len(result.undisposed) == 1
    assert result.undisposed[0].flag_class == "high_orphan"


def test_object_flags_with_flag_and_severity_attributes(tmp_path: Path) -> None:
    """The consumer passes DriftFlag objects, not just (id, severity) pairs."""
    _write_registry(tmp_path, _document({"owned_class": _downgraded()}))
    result = _build(
        tmp_path,
        [DriftFlag("owned_class@M516", "high"), DriftFlag("orphan", "high")],
    )
    assert len(result.undisposed) == 1
    assert result.undisposed[0].flag_class == "orphan"


def test_duplicate_classes_collapse_to_one_finding(tmp_path: Path) -> None:
    """Two parameterized ids of one class share one record: one finding."""
    _write_registry(tmp_path, _document({}))
    result = _build(tmp_path, [("x_class@M515", "high"), ("x_class@M516", "high")])
    assert len(result.undisposed) == 1
    assert result.undisposed[0].flag_class == "x_class"


# ── determinism, ordering, purity ────────────────────────────────────────────

def test_findings_property_concatenates_in_stable_order(tmp_path: Path) -> None:
    _write_registry(
        tmp_path,
        _document(
            {
                "m_class": _measured(),
                "bad_class": {"disposition": "cleared"},
                "d_class": _downgraded(),
            }
        ),
    )
    result = _build(
        tmp_path,
        [("m_class", "high"), ("bad_class", "high"), ("u_class", "high")],
    )
    assert [f.kind for f in result.findings] == [
        "undisposed",
        "contradiction",
        "malformed",
    ]


def test_deterministic_order_and_no_input_mutation(tmp_path: Path) -> None:
    flags = [("z_class", "high"), ("a_class", "high"), ("m_class", "medium")]
    snapshot = list(flags)
    _write_registry(tmp_path, _document({}))
    result = _build(tmp_path, flags)
    classes = [f.flag_class for f in result.undisposed]
    assert classes == sorted(classes) == ["a_class", "z_class"]
    assert flags == snapshot, "the oracle must never mutate its inputs"


# ── the live tree ────────────────────────────────────────────────────────────

def test_live_registry_loads_through_the_observer() -> None:
    """The committed registry must load fail-closed-clean on the real tree."""
    with_empty_flags = observe_closeout_criterion_disposition(
        _REPO_ROOT, live_high_flags=[]
    )
    assert with_empty_flags.is_silent
    with_default_root = observe_closeout_criterion_disposition(
        live_high_flags=[]
    )
    assert with_default_root.is_silent


def test_every_live_record_is_well_formed_and_covers_its_own_class() -> None:
    """Build the oracle against one synthetic high flag per live record class.

    `malformed` must stay empty (every live record is trustworthy) and
    `undisposed` must stay empty (every record covers the class it is keyed
    by). Contradictions are deliberately not asserted: a future honest
    `measured` record for a genuinely absent class is legal DATA.
    """
    document = yaml.safe_load(
        (_REPO_ROOT / _REGISTRY_RELATIVE).read_text(encoding="utf-8")
    )
    classes = sorted(str(key) for key in document["records"])
    assert classes, "the live registry must declare records"
    result = build_closeout_criterion_disposition(
        _REPO_ROOT, [(f"{flag_class}@PROBE", "high") for flag_class in classes]
    )
    assert result.malformed == ()
    assert result.undisposed == ()
