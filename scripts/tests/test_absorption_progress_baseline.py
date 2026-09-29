"""M514-1lss2s/S01/T05 — the committed absorption-progress baseline, byte-bound.

T04 proved the *measurement system* honest with synthetic fixtures. This file
proves the *delivered baseline* is honest on the real tree:

* ``test_committed_baseline_is_clean`` — the governed scorecard at
  ``artifacts/parse_compat/absorption_progress_baseline.json`` passes the composed
  T01+T04 oracle with the evidence hash recomputed from the real probe bytes. This
  is the positive control the Governor check runs on every ``preflight``.
* ``test_baseline_numbers_are_recomputed_from_the_probe`` — every number the
  baseline publishes is recomputed from the frozen probe and compared exactly. A
  hand-edited numerator, a rounded stratum or a re-cut census fails here, so the
  baseline is *derived*, not asserted.
* ``test_baseline_declares_full_denominator_and_unmet_axes`` — the full assigned
  population (610) stays in the denominator with all 79 typed exclusions, and the
  axes that have no product measurement (java independence, resources, reliability)
  are ``not_measured`` rather than zero. This is the "report real unmet outcomes"
  half: a green-looking scorecard that claimed those axes would fail.
* ``test_mutation_matrix`` — each probe mutates the *real* baseline once and
  requires the labelled violation; a surviving mutation is a test failure.
* ``test_historical_guard_pins_match_bytes`` — the M513 carriers keep the bytes the
  baseline ledger pins.

Boundary (E3): the pins and the evidence digest are recorded in the same
repository as the artifacts they bind, so this is a consistency check, not a
tamper-evident attestation. A writer able to rewrite the artifact, the ledger and
this test can satisfy it; its value is that drift, substitution and silent
re-labelling stop passing.
"""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path

import pytest
import yaml

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
_REPO_ROOT = _SCRIPTS_DIR.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from governor.adapters.absorption_progress_reader import (  # noqa: E402
    absorption_progress_violations,
    effective_progress_contract,
    missing_progress_vocabulary,
)

SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
BASELINE_PATH = "artifacts/parse_compat/absorption_progress_baseline.json"
REPORT_PATH = "doc/review/grobid-absorption-progress-baseline.md"
PROBE_PATH = "artifacts/parse_compat/external_holdout_release_probe.json"
DOMAIN_MODULE_PATH = "scripts/governor/domain/absorption_progress.py"

# D492: the closed vocabulary is owned by the T01 domain; the T04 rule YAML
# layers only its adapter-level labels on top. Mutation labels from the domain
# therefore appear in the validator source, not in the rule's `violations` list.
# Naming the split here means a label claimed for the wrong layer fails.
_DOMAIN_LAYER_LABELS = frozenset(
    {
        "exclusion_reason_untyped",
        "unnamed_exclusions",
        "skipped_as_delivered",
        "preservation_pooled_with_extraction",
        "threshold_invented_before_freeze",
        "authorization_used_as_product_evidence",
    }
)

# The frozen census the baseline is checked against (T01 schema
# `denominator.initial_census`). Written as literals so a DATA re-cut is a diff
# here rather than a silently accepted new denominator.
CENSUS = {
    "census_id": "holdout-partition-610-531-79",
    "declared": 610,
    "measurable": 531,
    "excluded": 79,
    "reason_counts": {
        "header_date_absent": 72,
        "header_title_absent": 5,
        "header_author_absent": 2,
    },
}


def _load_json(rel: str) -> dict:
    return json.loads((_REPO_ROOT / rel).read_text(encoding="utf-8"))


def _load_yaml(rel: str) -> dict:
    return yaml.safe_load((_REPO_ROOT / rel).read_text(encoding="utf-8"))


def _contract() -> dict:
    return effective_progress_contract(_load_yaml(SCHEMA_PATH), _load_yaml(RULE_PATH))


def _baseline() -> dict:
    return _load_json(BASELINE_PATH)


def _probe() -> dict:
    return _load_json(PROBE_PATH)


def _sha256(rel: str) -> str:
    return "sha256:" + hashlib.sha256((_REPO_ROOT / rel).read_bytes()).hexdigest()


def _observations(scorecard: dict, extra: dict | None = None) -> dict:
    """Recompute the content hash of every artifact the scorecard binds."""
    hashes: dict[str, str | None] = {}
    for row in scorecard.get("measurements", []):
        evidence = row.get("evidence") if isinstance(row, dict) else None
        if not isinstance(evidence, dict):
            continue
        rel = evidence.get("path")
        if isinstance(rel, str) and rel and rel not in hashes:
            path = _REPO_ROOT / rel
            hashes[rel] = _sha256(rel) if path.is_file() else None
    if extra:
        hashes.update(extra)
    return {"evidence_sha256": hashes}


def _violations(scorecard: dict, extra: dict | None = None) -> set[str]:
    return absorption_progress_violations(
        scorecard, _contract(), _observations(scorecard, extra)
    )


# ── the contract is complete before anything is measured ───────────────────


def test_progress_vocabulary_is_complete_on_the_real_tree() -> None:
    contract = _contract()
    assert missing_progress_vocabulary(contract) == []


# ── positive control: the committed baseline is clean ──────────────────────


def test_committed_baseline_is_clean() -> None:
    scorecard = _baseline()
    assert _violations(scorecard) == set()


def test_real_tree_has_no_absorption_progress_flag() -> None:
    """The service path (not just the oracle) sees a clean delivered scorecard."""
    from governor.application.trajectory_service import TrajectoryService

    service = TrajectoryService.__new__(TrajectoryService)
    service.root = _REPO_ROOT
    assert service._check_absorption_progress() == []


# ── the numbers are derived from the frozen probe, not asserted ────────────


def test_baseline_numbers_are_recomputed_from_the_probe() -> None:
    probe = _probe()
    scorecard = _baseline()
    aggregate = probe["evaluated_population"]["aggregate"]
    parity = probe["evaluated_population"]["class_parity"]["aggregate"]
    reference = probe["reference_population"]

    # Denominator: the probed reference population plus its typed exceptions.
    exceptions = reference["exceptions"]
    assert len(exceptions) == CENSUS["excluded"]
    reason_counts: dict[str, int] = {}
    for exception in exceptions:
        reason_counts[exception["reason"]] = reason_counts.get(exception["reason"], 0) + 1
    assert reason_counts == CENSUS["reason_counts"]

    denominator = scorecard["denominator"]
    assert denominator["assigned"] == reference["documents"] == CENSUS["declared"]
    assert denominator["executed"] == aggregate["documents"] == CENSUS["measurable"]
    assert denominator["excluded"] == CENSUS["excluded"]
    assert denominator["refused"] == 0
    assert denominator["exclusion_reasons"] == CENSUS["reason_counts"]
    assert (
        denominator["assigned"]
        == denominator["executed"] + denominator["refused"] + denominator["excluded"]
    )

    by_metric = {row["metric_id"]: row for row in scorecard["measurements"]}

    preservation = by_metric["preservation.occurrence_conservation"]
    assert preservation["score"] == {
        "numerator": aggregate["admitted"],
        "denominator": aggregate["source_population"],
    }
    # The weakest document stratum is the minimum conserved/source_population over
    # the measured documents. Compare as an exact integer pair, not a float: a
    # rounded stratum would otherwise pass and misreport the band's weakest point.
    weakest_exact = min(
        ((o["conserved"], o["source_population"], o["document_id"])
         for o in probe["outcomes"]
         if o.get("source_population")),
        key=lambda item: item[0] / item[1],
    )
    assert preservation["strata"]["per_document"] == {
        "numerator": weakest_exact[0],
        "denominator": weakest_exact[1],
    }
    assert preservation["weakest_document_id"] == weakest_exact[2]

    extraction = by_metric["extraction.native_capability_recall"]
    assert extraction["score"] == {
        "numerator": parity["native_capability_recall_micros"],
        "denominator": 1000000,
    }
    per_family: dict[str, list[int]] = {}
    for profile in probe["class_profiles"]:
        for entry in profile["classes"]:
            bucket = per_family.setdefault(entry["class"], [0, 0])
            bucket[0] += entry.get("conserved", 0)
            bucket[1] += entry.get("source_population", 0)
    weakest_family = min(
        ((c, s, name) for name, (c, s) in per_family.items() if s > 0),
        key=lambda item: item[0] / item[1],
    )
    assert extraction["strata"]["per_family"] == {
        "numerator": weakest_family[0],
        "denominator": weakest_family[1],
    }
    assert extraction["weakest_family_id"] == weakest_family[2]


def test_baseline_declares_full_denominator_and_unmet_axes() -> None:
    scorecard = _baseline()
    by_axis = {row["axis"]: row for row in scorecard["measurements"]}

    # Full denominator: the exclusions stay inside it and are typed.
    assert scorecard["denominator"]["excluded"] == CENSUS["excluded"]
    assert set(scorecard["denominator"]["exclusion_reasons"]) == set(
        CENSUS["reason_counts"]
    )

    # Real unmet outcomes: no product measurement for these axes, and never zero.
    for axis in ("java_independence", "resources", "reliability"):
        row = by_axis[axis]
        assert row["state"] == "not_measured", axis
        assert "score" not in row, axis

    # The two axes the census measured are measured, and each carries its band.
    for axis in ("preservation", "extraction"):
        row = by_axis[axis]
        assert row["state"] == "measured", axis
        assert row["population_basis"]["measurable_documents"] == CENSUS["measurable"]
        assert row["population_basis"]["assigned_documents"] == CENSUS["declared"]
        assert row["population_basis"]["document_coverage"] == {
            "numerator": CENSUS["measurable"],
            "denominator": CENSUS["declared"],
        }

    # Authorization is separate and grants no coverage.
    assert scorecard["authorization"]["state"] == "owner_directive_research"
    assert scorecard["authorization"]["product_coverage_effect"] == "none"


# ── mutation matrix on the real baseline ───────────────────────────────────

_MUTATIONS: list[tuple[str, str, object]] = [
    (
        "evidence_hash_substituted",
        "substitute the declared evidence digest",
        lambda s: s["measurements"][0]["evidence"].__setitem__(
            "sha256", "sha256:" + "b" * 64
        ),
    ),
    (
        "evidence_path_not_relative",
        "make the evidence path absolute",
        lambda s: s["measurements"][0]["evidence"].__setitem__("path", "/etc/passwd"),
    ),
    (
        "evidence_artifact_missing",
        "bind a path that does not resolve",
        lambda s: s["measurements"][0]["evidence"].__setitem__(
            "path", "artifacts/parse_compat/absent_probe.json"
        ),
    ),
    (
        "oracle_changed",
        "change the oracle of one measured row",
        lambda s: s["measurements"][1]["comparability"].__setitem__(
            "oracle", "other_oracle"
        ),
    ),
    (
        "scope_not_accepted",
        "move one measured row out of the accepted corpus",
        lambda s: s["measurements"][1]["comparability"].__setitem__(
            "corpus_id", "some-other-corpus"
        ),
    ),
    (
        "denominator_shrink",
        "shrink the executed population below the pinned census",
        lambda s: s["denominator"].__setitem__("executed", 400)
        or s["denominator"].__setitem__("assigned", 479),
    ),
    (
        "unnamed_exclusions",
        "drop one typed exclusion reason",
        lambda s: s["denominator"]["exclusion_reasons"].pop("header_author_absent"),
    ),
    (
        "exclusion_reason_untyped",
        "rename an exclusion reason outside the closed vocabulary",
        lambda s: s["denominator"]["exclusion_reasons"].__setitem__(
            "unknown_reason", s["denominator"]["exclusion_reasons"].pop(
                "header_author_absent"
            )
        ),
    ),
    (
        "skipped_as_delivered",
        "offer a terminal GSD state as delivered product",
        lambda s: s["measurements"][0].__setitem__("delivered_by", "complete"),
    ),
    (
        "preservation_pooled_with_extraction",
        "pool preservation into the extraction numerator",
        lambda s: s["measurements"][1].__setitem__("includes_preservation", True),
    ),
    (
        "threshold_invented_before_freeze",
        "invent a numeric threshold before S02 freezes one",
        lambda s: s["measurements"][0].__setitem__("threshold", 0.9),
    ),
    (
        "coverage_regression",
        "silence an axis the pinned census measured",
        lambda s: s["measurements"][1].__setitem__("state", "not_measured"),
    ),
    (
        "savings_without_paired_cpu",
        "claim a savings without its paired CPU measurement",
        lambda s: s["measurements"][3].__setitem__("savings_claimed", True),
    ),
    (
        "authorization_used_as_product_evidence",
        "let the research authorization raise product coverage",
        lambda s: s["authorization"].__setitem__("product_coverage_effect", "full"),
    ),
]


@pytest.mark.parametrize(
    "label,description,mutate",
    _MUTATIONS,
    ids=[label for label, _, _ in _MUTATIONS],
)
def test_mutation_matrix(label: str, description: str, mutate) -> None:
    scorecard = copy.deepcopy(_baseline())
    mutate(scorecard)
    extra = None
    if label == "evidence_artifact_missing":
        # A path the reader resolved but could not read is what the label means.
        extra = {"artifacts/parse_compat/absent_probe.json": None}
    violations = _violations(scorecard, extra)
    assert label in violations, f"{description}: got {sorted(violations)}"


def test_every_committed_label_is_registered_in_a_governed_vocabulary() -> None:
    """Each mutation label must live in one of the two governed layers (D492).

    The T04 rule YAML is the adapter-level closed vocabulary; the T01 domain
    carries the rest. A label registered in neither is an implementation bug, so
    the two-layer split is asserted rather than assumed.
    """
    registered = set(_load_yaml(RULE_PATH).get("violations") or [])
    declared = {label for label, _, _ in _MUTATIONS}
    # Adapter-level labels must be declared in the rule DATA.
    assert declared - _DOMAIN_LAYER_LABELS <= registered, sorted(
        (declared - _DOMAIN_LAYER_LABELS) - registered
    )
    # Domain-level labels must really exist as literals in the T01 validator, so
    # a docstring/source drift is a failure instead of a silent claim.
    domain_source = (_REPO_ROOT / DOMAIN_MODULE_PATH).read_text(encoding="utf-8")
    for label in sorted(_DOMAIN_LAYER_LABELS):
        assert f'"{label}"' in domain_source, label
    # The split must be exhaustive: every declared label is in exactly one layer.
    assert _DOMAIN_LAYER_LABELS <= declared


# ── historical guard bytes ─────────────────────────────────────────────────


def test_historical_guard_pins_match_bytes() -> None:
    pins = _baseline()["historical_guard_pins"]["pins"]
    assert pins, "the baseline must pin the M513 carriers it did not touch"
    for rel, expected in pins.items():
        path = _REPO_ROOT / rel
        assert path.is_file(), rel
        assert _sha256(rel) == expected, f"{rel} changed under a read-only pin"


def test_historical_carriers_are_declared_read_only() -> None:
    carriers = _load_yaml(SCHEMA_PATH)["historical_carriers"]
    assert carriers["mode"] == "read_only"
    assert carriers["rewriting_forbidden"] is True
    ids = {entry["id"] for entry in carriers["carriers"]}
    assert "m513-s03-uat-4" in ids


# ── the deliverable report describes the delivered bytes, honestly ──────────


def test_report_pins_the_baseline_artifact_hash() -> None:
    """The report's baseline pin must match the artifact bytes it describes.

    §1 already warns that the *schema* hash goes stale when S02 freezes
    thresholds; that staleness is expected. The baseline scorecard row is
    different: it describes the artifact this file governs, so a byte change
    without the report following is drift, not an expected staleness.
    """
    report = (_REPO_ROOT / REPORT_PATH).read_text(encoding="utf-8")
    assert _sha256(BASELINE_PATH).removeprefix("sha256:") in report


def test_gate_run_history_keeps_the_first_failure_unexplained() -> None:
    """Both gate runs are recorded, and the first failure keeps an unknown cause.

    A previous revision of the report asserted a proven bind/drop race and the
    absence of a regression. Neither was established: the first run's capture
    was discarded, so its failing test is unknown. The machine-readable record
    must stay unresolved, and a green repeat must not be offered as the reason.
    """
    history = _baseline()["gate_run_history"]
    first, second = history["runs"]
    assert first["exit_code"] == 1
    assert first["failed_step"] == "da-parse tests"
    assert first["failure_cause"] == "unresolved"
    assert first["root_cause_established"] is False
    assert first["detail_capture"] == "not_retained"
    assert first["failing_tests"] is None
    assert second["exit_code"] == 0
    assert second["root_cause_established"] is False
    hypothesis = history["hypothesis"]
    assert hypothesis["status"] == "unverified"
    assert hypothesis["disconfirming_evidence"]
    conclusions = " | ".join(hypothesis["cannot_conclude"]).lower()
    assert "no root cause is established" in conclusions
    assert "no claim that a flake was eliminated" in conclusions
    assert "no claim that no regression occurred" in conclusions
    # The diagnostics fix is named, so the record points at its own guard.
    guard = history["diagnostics_change"]["test"]
    assert guard.endswith("test_gate_runner_persists_failing_step_capture_and_names_the_test")


def test_report_records_the_gate_failure_as_unresolved() -> None:
    """§10 must separate observed facts from the unproven hypothesis."""
    report = (_REPO_ROOT / REPORT_PATH).read_text(encoding="utf-8")
    section = report.split("## 10.", 1)[1].split("## 11.", 1)[0]
    # Both runs survive: the failed run and the later green run.
    assert "18/19" in section and "19/19" in section
    assert "exit 1" in section and "exit 0" in section
    # The lost capture and the unknown cause are stated, not implied.
    assert "конкретный failing test прогона #1 неизвестен" in section
    assert "причина провала #1 **не установлена**" in section
    assert "root-cause proof" in section
    # The previous revision's unsupported claim must not return.
    assert "Причина — не регрессия кода" not in section
    assert "а флейк под параллельной нагрузкой" not in section
