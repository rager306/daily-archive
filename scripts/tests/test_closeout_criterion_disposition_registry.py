"""Tests for the closeout-criterion disposition registry (M516-th0oby/S13/T01).

Milestone success criterion SC7 clause 1 ("governor preflight reports no high
severity flag at closeout") is only machine-checkable once every high-capable
flag CLASS carries a recorded disposition. These tests pin the shape of the DATA
registry `scripts/governor/checks/closeout_criterion_dispositions.yaml`:

  - the closed `disposition_vocabulary` and `reason_vocabulary`;
  - the exact `required_record_fields` and `required_for_severities`;
  - every record: all five required fields non-empty, disposition in the
    vocabulary, reason in the vocabulary, decision_ref a D-number;
  - both audit sentinel classes declared (so neither fail-closed audit
    self-triggers);
  - no `dimension`/`severity` key anywhere (it is a disposition registry, not a
    check rule -- D492).

The shape rules live in `_shape_errors`, which is exercised both on the live
registry (must be clean) and on inline malformed fixtures (must be rejected) --
so the positive assertions cannot pass vacuously.

The registry is DATA; the S13 oracle/consumer (T02/T03) read it and the live-tree
test asserts the criterion. This file only pins the declared shape.

Run with: PYTHONPATH=scripts .venv/bin/python -m pytest -q scripts/tests/test_closeout_criterion_disposition_registry.py
"""

import re
import sys
from pathlib import Path

import yaml

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

_REPO_ROOT = _SCRIPTS_DIR.parent
_REGISTRY_RELATIVE = "scripts/governor/checks/closeout_criterion_dispositions.yaml"

_EXPECTED_SCHEMA_VERSION = "closeout-criterion-dispositions.v1"
_EXPECTED_DISPOSITION_VOCABULARY = ["measured", "downgraded"]
_EXPECTED_REASON_VOCABULARY = [
    "pre_existing_structural",
    "pre_existing_environment",
    "severity_environment_dependent",
    "governor_process_finding",
]
_EXPECTED_REQUIRED_RECORD_FIELDS = [
    "disposition",
    "owner",
    "reason",
    "decision_ref",
    "rationale",
]
_EXPECTED_REQUIRED_FOR_SEVERITIES = ["high"]

# The two fail-closed audits seed themselves with their own finding class; each
# must be declared or the audit self-triggers on every report.
_SENTINEL_CLASSES = ("drift_flag_owner_missing", "closeout_criterion_undisposed")

_DECISION_REF_PATTERN = re.compile(r"^D\d+$")


def _document() -> dict:
    return yaml.safe_load(
        (_REPO_ROOT / _REGISTRY_RELATIVE).read_text(encoding="utf-8")
    )


def _recursive_keys(node):
    """Every mapping key anywhere in a YAML document, for the D492 scan."""
    if isinstance(node, dict):
        for key, value in node.items():
            yield key
            yield from _recursive_keys(value)
    elif isinstance(node, list):
        for item in node:
            yield from _recursive_keys(item)


def _shape_errors(document) -> list:
    """Every way a registry document violates the declared shape.

    Returns a list of human-readable violations; an empty list means the
    document satisfies the whole contract. Exercised on the live registry
    (empty) and on inline malformed fixtures (non-empty), so the rules cannot
    pass vacuously.
    """
    errors: list = []
    if not isinstance(document, dict):
        return ["the registry must be a YAML mapping"]

    if document.get("schema_version") != _EXPECTED_SCHEMA_VERSION:
        errors.append("schema_version mismatch")
    if document.get("disposition_vocabulary") != _EXPECTED_DISPOSITION_VOCABULARY:
        errors.append("disposition_vocabulary mismatch")
    if document.get("reason_vocabulary") != _EXPECTED_REASON_VOCABULARY:
        errors.append("reason_vocabulary mismatch")
    if document.get("required_record_fields") != _EXPECTED_REQUIRED_RECORD_FIELDS:
        errors.append("required_record_fields mismatch")
    if document.get("required_for_severities") != _EXPECTED_REQUIRED_FOR_SEVERITIES:
        errors.append("required_for_severities mismatch")

    records = document.get("records")
    if not isinstance(records, dict) or not records:
        errors.append("records must be a non-empty mapping")
        return errors

    dispositions = set(document.get("disposition_vocabulary") or [])
    reasons = set(document.get("reason_vocabulary") or [])
    for flag_class, record in records.items():
        if not isinstance(record, dict):
            errors.append(f"{flag_class}: record is not a mapping")
            continue
        for field in _EXPECTED_REQUIRED_RECORD_FIELDS:
            value = record.get(field)
            if not (isinstance(value, str) and value.strip()):
                errors.append(f"{flag_class}: missing or empty `{field}`")
        if record.get("disposition") not in dispositions:
            errors.append(f"{flag_class}: disposition outside the vocabulary")
        if record.get("reason") not in reasons:
            errors.append(f"{flag_class}: reason outside the vocabulary")
        ref = record.get("decision_ref")
        if not (isinstance(ref, str) and _DECISION_REF_PATTERN.match(ref)):
            errors.append(f"{flag_class}: decision_ref is not a D-number")
        if record.get("disposition") == "measured":
            errors.append(f"{flag_class}: unexpected `measured` record")

    for sentinel in _SENTINEL_CLASSES:
        if sentinel not in records:
            errors.append(f"sentinel class {sentinel} is not declared")

    keys = list(_recursive_keys(document))
    if "dimension" in keys:
        errors.append("document carries a `dimension` key")
    if "severity" in keys:
        errors.append("document carries a `severity` key")

    return errors


def _valid_record(**overrides) -> dict:
    record = {
        "disposition": "downgraded",
        "owner": "owner",
        "reason": "pre_existing_environment",
        "decision_ref": "D572",
        "rationale": "a one-sentence rationale",
    }
    record.update(overrides)
    return record


def _valid_document() -> dict:
    return {
        "schema_version": _EXPECTED_SCHEMA_VERSION,
        "disposition_vocabulary": list(_EXPECTED_DISPOSITION_VOCABULARY),
        "reason_vocabulary": list(_EXPECTED_REASON_VOCABULARY),
        "required_record_fields": list(_EXPECTED_REQUIRED_RECORD_FIELDS),
        "required_for_severities": list(_EXPECTED_REQUIRED_FOR_SEVERITIES),
        "records": {
            _SENTINEL_CLASSES[0]: _valid_record(owner="governor-ownership",
                                                reason="governor_process_finding"),
            _SENTINEL_CLASSES[1]: _valid_record(owner="governor-criterion",
                                                reason="governor_process_finding"),
        },
    }


# ── the registry header / closed vocabularies ────────────────────────────────

def test_registry_parses_and_declares_the_schema_version() -> None:
    document = _document()
    assert isinstance(document, dict), "the registry must parse as a YAML mapping"
    assert document["schema_version"] == _EXPECTED_SCHEMA_VERSION


def test_disposition_vocabulary_is_the_closed_two_element_set() -> None:
    assert _document()["disposition_vocabulary"] == _EXPECTED_DISPOSITION_VOCABULARY


def test_reason_vocabulary_is_the_closed_four_element_set() -> None:
    assert _document()["reason_vocabulary"] == _EXPECTED_REASON_VOCABULARY


def test_required_record_fields_is_the_exact_five_field_list() -> None:
    assert _document()["required_record_fields"] == _EXPECTED_REQUIRED_RECORD_FIELDS


def test_required_for_severities_is_high_only() -> None:
    assert _document()["required_for_severities"] == _EXPECTED_REQUIRED_FOR_SEVERITIES


# ── the records block ────────────────────────────────────────────────────────

def test_live_registry_satisfies_the_whole_shape_contract() -> None:
    """The live registry must have no shape violation at all."""
    assert _shape_errors(_document()) == []


def test_every_record_carries_all_five_required_fields_non_empty() -> None:
    document = _document()
    records = document["records"]
    assert isinstance(records, dict) and records, "the registry must declare records"
    for flag_class, record in records.items():
        assert isinstance(record, dict), f"{flag_class} must be a mapping"
        for field in _EXPECTED_REQUIRED_RECORD_FIELDS:
            assert field in record, f"{flag_class} is missing required field {field}"
            value = record[field]
            assert isinstance(value, str) and value.strip(), (
                f"{flag_class}.{field} must be a non-empty string"
            )


def test_every_record_disposition_is_in_the_vocabulary() -> None:
    document = _document()
    vocabulary = set(document["disposition_vocabulary"])
    for flag_class, record in document["records"].items():
        assert record["disposition"] in vocabulary, (
            f"{flag_class}.disposition {record['disposition']!r} is outside the "
            f"closed vocabulary {sorted(vocabulary)}"
        )


def test_every_record_reason_is_in_the_vocabulary() -> None:
    document = _document()
    vocabulary = set(document["reason_vocabulary"])
    for flag_class, record in document["records"].items():
        assert record["reason"] in vocabulary, (
            f"{flag_class}.reason {record['reason']!r} is outside the closed "
            f"vocabulary {sorted(vocabulary)}"
        )


def test_every_decision_ref_matches_a_d_number() -> None:
    document = _document()
    for flag_class, record in document["records"].items():
        assert _DECISION_REF_PATTERN.match(record["decision_ref"]), (
            f"{flag_class}.decision_ref {record['decision_ref']!r} is not a D-number"
        )


def test_both_audit_sentinel_classes_are_declared() -> None:
    """Each fail-closed audit seeds itself with its own class; declare both."""
    records = _document()["records"]
    for flag_class in _SENTINEL_CLASSES:
        assert flag_class in records, f"the sentinel class {flag_class} must be declared"


def test_no_measured_record_on_this_tree() -> None:
    """Every high-capable class is live or severity-fluctuating: all downgraded.

    A `measured` record for a class that is live would be a stale-clear the S13
    check then flags as a contradiction, so the registry declares none.
    """
    document = _document()
    measured = [
        flag_class
        for flag_class, record in document["records"].items()
        if record["disposition"] == "measured"
    ]
    assert measured == [], f"unexpected `measured` records: {measured}"


# ── the D492 scan: a disposition registry is not a check rule ────────────────

def test_registry_carries_no_dimension_or_severity_key() -> None:
    """An ownership/disposition registry must not masquerade as a quality gate."""
    keys = list(_recursive_keys(_document()))
    assert "dimension" not in keys, "the registry must not carry a `dimension` key"
    assert "severity" not in keys, "the registry must not carry a `severity` key"


# ── negative: the shape rules reject malformed documents (not vacuous) ───────

def test_shape_accepts_the_minimal_valid_document() -> None:
    assert _shape_errors(_valid_document()) == []


def test_shape_rejects_a_record_missing_a_required_field() -> None:
    document = _valid_document()
    del document["records"][_SENTINEL_CLASSES[0]]["decision_ref"]
    errors = _shape_errors(document)
    assert any("decision_ref" in error for error in errors), errors


def test_shape_rejects_an_empty_required_field() -> None:
    document = _valid_document()
    document["records"][_SENTINEL_CLASSES[0]]["owner"] = "   "
    errors = _shape_errors(document)
    assert any("owner" in error for error in errors), errors


def test_shape_rejects_an_unknown_disposition() -> None:
    document = _valid_document()
    document["records"][_SENTINEL_CLASSES[0]]["disposition"] = "cleared"
    errors = _shape_errors(document)
    assert any("disposition" in error for error in errors), errors


def test_shape_rejects_an_unknown_reason() -> None:
    document = _valid_document()
    document["records"][_SENTINEL_CLASSES[0]]["reason"] = "because"
    errors = _shape_errors(document)
    assert any("reason" in error for error in errors), errors


def test_shape_rejects_a_non_d_decision_ref() -> None:
    document = _valid_document()
    document["records"][_SENTINEL_CLASSES[0]]["decision_ref"] = "572"
    errors = _shape_errors(document)
    assert any("decision_ref" in error for error in errors), errors


def test_shape_rejects_a_missing_sentinel_class() -> None:
    document = _valid_document()
    del document["records"][_SENTINEL_CLASSES[1]]
    errors = _shape_errors(document)
    assert any("sentinel" in error for error in errors), errors


def test_shape_rejects_a_measured_record() -> None:
    document = _valid_document()
    document["records"][_SENTINEL_CLASSES[0]]["disposition"] = "measured"
    errors = _shape_errors(document)
    assert any("measured" in error for error in errors), errors


def test_shape_rejects_a_dimension_or_severity_key() -> None:
    document = _valid_document()
    document["records"][_SENTINEL_CLASSES[0]]["severity"] = "high"
    errors = _shape_errors(document)
    assert any("severity" in error for error in errors), errors


def test_shape_rejects_a_non_mapping_records_block() -> None:
    document = _valid_document()
    document["records"] = ["not", "a", "mapping"]
    errors = _shape_errors(document)
    assert any("records" in error for error in errors), errors
