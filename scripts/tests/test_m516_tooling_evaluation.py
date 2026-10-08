"""Contract test for the M516/S06/T05 tooling-evaluation artifact.

The artifact is the deliverable of T05: a *measured* verdict for the hotpath-rs
and fearless-simd candidates. This test pins the artifact's stated contract so a
later edit cannot quietly turn a measured verdict into an endorsement, drop the
named hot path, or add a dependency.

Negative fixtures are inline dicts passed to :func:`validate_evaluation`; the
real artifact is asserted only through the same validator, so the positive and
negative paths exercise one definition.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

_REPO_ROOT = Path(__file__).resolve().parents[2]
ARTIFACT = _REPO_ROOT / "artifacts/observability/m516_tooling_evaluation.json"

VERDICTS = {"adopt", "defer", "reject"}


def _require(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def validate_evaluation(doc: dict[str, Any]) -> list[str]:
    """Return a list of contract violations (empty means the artifact is valid)."""
    errors: list[str] = []
    _require(
        isinstance(doc.get("schema_version"), str) and doc["schema_version"],
        "schema_version missing",
        errors,
    )

    candidates = doc.get("candidates")
    _require(isinstance(candidates, list) and candidates, "candidates missing/empty", errors)
    candidates = candidates if isinstance(candidates, list) else []
    ids = [c.get("id") for c in candidates if isinstance(c, dict)]
    _require(len(ids) == len(set(ids)), "candidate ids are not unique", errors)

    for candidate in candidates:
        if not isinstance(candidate, dict):
            errors.append("candidate is not an object")
            continue
        cid = candidate.get("id", "<missing-id>")
        verdict = candidate.get("verdict")
        if verdict not in VERDICTS:
            errors.append(f"{cid}: verdict {verdict!r} not in {sorted(VERDICTS)}")
            continue
        if verdict == "adopt":
            _require(
                bool(candidate.get("measured_justification")),
                f"{cid}: adopt requires measured_justification",
                errors,
            )
        if verdict == "defer":
            _require(
                bool(candidate.get("decision_changing_measurement")),
                f"{cid}: defer requires decision_changing_measurement",
                errors,
            )
        if verdict == "reject":
            _require(bool(candidate.get("reason")), f"{cid}: reject requires reason", errors)

    # At least one hot path must carry a real profile with an identified instrument.
    hot_paths = doc.get("hot_paths")
    _require(isinstance(hot_paths, list) and hot_paths, "hot_paths missing/empty", errors)
    hot_paths = hot_paths if isinstance(hot_paths, list) else []
    measured = [h for h in hot_paths if isinstance(h, dict) and h.get("measured") is True]
    _require(len(measured) >= 1, "no hot path was measured", errors)

    measurement = doc.get("measurement")
    _require(isinstance(measurement, dict), "measurement missing", errors)
    measurement = measurement if isinstance(measurement, dict) else {}

    instrument = measurement.get("instrument")
    _require(isinstance(instrument, dict), "measurement.instrument missing", errors)
    if isinstance(instrument, dict):
        _require(bool(instrument.get("name")), "measurement.instrument.name missing", errors)

    runs = measurement.get("runs")
    _require(isinstance(runs, list) and runs, "measurement.runs missing/empty", errors)
    runs = runs if isinstance(runs, list) else []
    run_ids = {r.get("id") for r in runs if isinstance(r, dict)}

    for hot in measured:
        _require(
            bool(hot.get("measurement_ref")),
            f"{hot.get('id')}: measured hot path has no measurement_ref",
            errors,
        )
        refs = [hot.get("measurement_ref")]
        extra = hot.get("measurement_refs")
        if isinstance(extra, list):
            refs.extend(extra)
        for ref in refs:
            _require(
                ref in run_ids,
                f"{hot.get('id')}: measurement_ref {ref!r} matches no run",
                errors,
            )

    delta = measurement.get("what_existing_instrumentation_did_not_measure")
    _require(
        isinstance(delta, list) and len(delta) >= 1,
        "what_existing_instrumentation_did_not_measure missing/empty",
        errors,
    )

    guardrails = doc.get("guardrails")
    _require(isinstance(guardrails, dict), "guardrails missing", errors)
    guardrails = guardrails if isinstance(guardrails, dict) else {}
    _require(guardrails.get("dependencies_added") == [], "dependencies_added is not empty", errors)
    _require(guardrails.get("manifests_modified") == [], "manifests_modified is not empty", errors)
    lockfile = guardrails.get("lockfile")
    _require(
        isinstance(lockfile, dict) and lockfile.get("untouched") is True,
        "guardrails.lockfile.untouched is not True",
        errors,
    )
    return errors


def _load() -> dict[str, Any]:
    return json.loads(ARTIFACT.read_text(encoding="utf-8"))


def test_artifact_exists_and_is_valid_json() -> None:
    assert ARTIFACT.is_file(), f"missing artifact {ARTIFACT}"
    _load()


def test_artifact_satisfies_contract() -> None:
    errors = validate_evaluation(_load())
    assert errors == [], f"artifact contract violations: {errors}"


def test_artifact_carries_one_verdict_per_named_candidate() -> None:
    doc = _load()
    verdicts = {c["id"]: c["verdict"] for c in doc["candidates"]}
    assert set(verdicts) == {"hotpath-rs", "fearless-simd"}, verdicts
    assert all(v in VERDICTS for v in verdicts.values()), verdicts


def test_artifact_records_named_hot_path_with_instrument() -> None:
    doc = _load()
    measured = [h for h in doc["hot_paths"] if h.get("measured") is True]
    assert measured, "no measured hot path"
    instrument = doc["measurement"]["instrument"]["name"]
    assert instrument, "instrument not identified"
    run_ids = {r["id"] for r in doc["measurement"]["runs"]}
    refs = [measured[0]["measurement_ref"], *measured[0].get("measurement_refs", [])]
    assert set(refs) <= run_ids, refs


# --- Negative fixtures: one variable broken per test -------------------------


def _valid_doc() -> dict[str, Any]:
    return _load()


def test_negative_unknown_verdict_is_rejected() -> None:
    doc = _valid_doc()
    doc["candidates"][0]["verdict"] = "maybe"
    assert any("not in" in e for e in validate_evaluation(doc))


def test_negative_defer_without_measurement_is_rejected() -> None:
    doc = _valid_doc()
    target = next(c for c in doc["candidates"] if c["verdict"] == "defer")
    target.pop("decision_changing_measurement", None)
    assert any("decision_changing_measurement" in e for e in validate_evaluation(doc))


def test_negative_reject_without_reason_is_rejected() -> None:
    doc = _valid_doc()
    target = next(c for c in doc["candidates"] if c["verdict"] == "reject")
    target.pop("reason", None)
    assert any("reject requires reason" in e for e in validate_evaluation(doc))


def test_negative_adopt_without_measured_justification_is_rejected() -> None:
    doc = _valid_doc()
    doc["candidates"][0]["verdict"] = "adopt"
    doc["candidates"][0].pop("measured_justification", None)
    assert any("measured_justification" in e for e in validate_evaluation(doc))


def test_negative_zero_measured_hot_paths_is_rejected() -> None:
    doc = _valid_doc()
    for hot in doc["hot_paths"]:
        hot["measured"] = False
    assert any("no hot path was measured" in e for e in validate_evaluation(doc))


def test_negative_dependency_added_is_rejected() -> None:
    doc = _valid_doc()
    doc["guardrails"]["dependencies_added"] = ["hotpath"]
    assert any("dependencies_added" in e for e in validate_evaluation(doc))


def test_negative_unknown_measurement_ref_is_rejected() -> None:
    doc = _valid_doc()
    doc["hot_paths"][0]["measurement_ref"] = "perf-stat-does-not-exist"
    assert any("matches no run" in e for e in validate_evaluation(doc))


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(pytest.main([__file__, "-q"]))
