"""GSD control-plane health oracle — pure decision over injected observations.

The Governor already reads `.gsd/exec` evidence (`GsdExecReader`) and resource
pressure (`resource_safety`). Neither saw the failure class the 2026-09-20..27 log
review surfaced: projection-claim contention, liveness wedges, lease conflicts,
Technical-Verdict blocks, an always-red doctor with no baseline, and unbounded
runtime stores. Those are control-plane facts, and they were only discoverable by
reading the journal by hand.

This module decides over *observations* supplied by
`governor.adapters.gsd_control_plane_reader.GsdControlPlaneReader`, and takes every
threshold from `gsd_control_plane.yaml`. It performs no filesystem access, so the
decision can be tested against committed fixtures instead of the live machine
(ADR-051: state-based fakes beat choreography).

Fail-closed contract: a contract that is absent, malformed, or missing any
required threshold returns the single `contract_invalid` signal rather than an
empty finding list. An empty list must mean "measured and clean", never "could not
measure".
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Any

_REQUIRED_THRESHOLDS = (
    "projection_contention_max",
    "liveness_wedge_max",
    "lease_conflict_max",
    "tech_verdict_block_max",
    "unretained_exec_captures_max",
    "unretained_quarantine_runs_max",
    "tmp_scratch_dirs_max",
    "commit_hook_failures_max",
)

# The retention policy is DATA the observer and the hygiene command both use; the
# oracle only checks that it is present and integral, so a contract that drops it
# cannot silently disable the very comparison it defines.
_REQUIRED_RETENTION_KEYS = ("days_exec", "days_quarantine", "days_activity", "min_age_hours")

_REQUIRED_JOURNAL_MARKERS = (
    "projection_contention",
    "liveness_wedge",
    "lease_conflict",
    "tech_verdict_block",
)

_REQUIRED_OBSERVATIONS = (
    "journal_counts",
    "unretained_exec_captures",
    "unretained_quarantine_runs",
    "tmp_scratch_dirs",
    "orphan_projection_dir",
    "doctor_latest",
    "doctor_baseline_entries",
    "doctor_baseline_present",
    "commit_failures",
    "commit_failure_hooks",
    "rust_verification",
)

# Doctor messages name a milestone in two shapes: `M465-sqt7w0/S01/T01` and a
# phase path such as `phases/500-qk6nr1-native-family.../500-02-RESEARCH.md`. Both
# must be recognised, otherwise a divergence that is scoped to an old milestone
# reads as `unscoped` and can never be baselined without suppressing it everywhere.
_MILESTONE_RE = re.compile(r"\bM(\d{3})(?:-[a-z0-9]+)?\b|\bphases/(\d{3})-")

_VALIDATION_SIGNAL = "contract_invalid"
_INCOMPLETE_SIGNAL = "observations_incomplete"

# Longest evidence string a single finding may carry. A drifted store can count in
# the millions; the report is read by a human, so counts are enough.
_MAX_EVIDENCE_CHARS = 400


@dataclass(frozen=True)
class Finding:
    """One control-plane signal, already severity-resolved from the contract."""

    signal: str
    severity: str
    evidence: str


def evaluate(contract: Any, observations: Any) -> list[Finding]:
    """Return control-plane findings for one governor run.

    `contract` is the parsed `gsd_control_plane.yaml`; `observations` is the
    reader payload. Both are validated fail-closed before any signal is emitted.
    """
    problem = _contract_problem(contract)
    if problem is not None:
        return [Finding(_VALIDATION_SIGNAL, "high", problem)]

    missing = [key for key in _REQUIRED_OBSERVATIONS if key not in observations]
    if missing:
        return [
            Finding(
                _INCOMPLETE_SIGNAL,
                "high",
                f"observer did not supply {sorted(missing)}",
            )
        ]

    thresholds = contract["thresholds"]
    signals = set(contract["signals"])
    high_signals = set(contract.get("high_severity_signals") or ())
    counts: dict[str, Any] = observations["journal_counts"]

    findings: list[Finding] = []

    def emit(signal: str, evidence: str) -> None:
        if signal not in signals:
            # The oracle may only speak the declared vocabulary; an undeclared
            # signal means the contract and the code disagree.
            findings.append(
                Finding(
                    _VALIDATION_SIGNAL,
                    "high",
                    f"oracle emitted undeclared signal {signal!r}",
                )
            )
            return
        findings.append(
            Finding(
                signal,
                "high" if signal in high_signals else "medium",
                evidence[:_MAX_EVIDENCE_CHARS],
            )
        )

    for marker_key, threshold_key in (
        ("projection_contention", "projection_contention_max"),
        ("liveness_wedge", "liveness_wedge_max"),
        ("lease_conflict", "lease_conflict_max"),
        ("tech_verdict_block", "tech_verdict_block_max"),
    ):
        observed = _as_count(counts.get(marker_key))
        limit = thresholds[threshold_key]
        if observed > limit:
            emit(
                marker_key,
                f"{observed} occurrence(s) in the last {contract['window_days']} day(s), "
                f"limit {limit}; marker={contract['journal_markers'][marker_key]!r}",
            )

    if observations["orphan_projection_dir"]:
        emit(
            "orphan_projection_dir",
            ".gsd/gsd exists — a projection was written under a nested .gsd path "
            "instead of .gsd/; the artifact is misplaced, not merely duplicated",
        )

    failures = _as_count(observations["commit_failures"])
    if failures > thresholds["commit_hook_failures_max"]:
        hooks = observations["commit_failure_hooks"]
        dominant = ", ".join(
            f"{name}={count}"
            for name, count in sorted(_hook_items(hooks), key=lambda item: -item[1])[:4]
        )
        emit(
            "commit_hook_failures",
            f"{failures} git action(s) failed after the latest commit ({dominant}). "
            "A failed auto-commit cannot be retried by the engine, so the unit wedges; "
            f"{_fmt_coverage_note(observations.get('rust_verification'))}",
        )

    for signal, observed_count, limit in (
        (
            "unretained_exec_captures",
            observations["unretained_exec_captures"],
            thresholds["unretained_exec_captures_max"],
        ),
        (
            "unretained_quarantine_runs",
            observations["unretained_quarantine_runs"],
            thresholds["unretained_quarantine_runs_max"],
        ),
        ("tmp_scratch_bloat", observations["tmp_scratch_dirs"], thresholds["tmp_scratch_dirs_max"]),
    ):
        observed = _as_count(observed_count)
        if observed > limit:
            emit(
                signal,
                f"{observed} entr(ies) past the retention window (limit {limit}); "
                "clear with `python3 -m governor hygiene --apply`",
            )

    if not observations["doctor_baseline_present"]:
        latest = observations["doctor_latest"] or {}
        emit(
            "doctor_baseline_missing",
            "no doctor baseline at "
            f"{contract['doctor_baseline_path']}; each run re-reports historical "
            f"divergence (latest errors={_as_count(latest.get('errors'))}, "
            f"warnings={_as_count(latest.get('warnings'))}) and no run can ever be clean",
        )
    else:
        fresh = unbaselined_issues(
            observations["doctor_latest"] or {},
            observations["doctor_baseline_entries"],
        )
        if fresh:
            emit(
                "doctor_new_divergence",
                f"{len(fresh)} divergence(s) outside the baseline: {fresh[:8]}",
            )

    return findings


def _hook_items(hooks: Any) -> list[tuple[str, int]]:
    if not isinstance(hooks, dict):
        return []
    return [(str(name), _as_count(count)) for name, count in hooks.items()]


def _fmt_coverage_note(coverage: Any) -> str:
    """Explain the dominant cause from recorded verifications, not from a guess.

    Measured 2026-09-27: 44 of 54 cargo-verified tasks in the preceding week recorded
    no rustfmt check. Reported as evidence, never as its own standing flag, because a
    per-task rule fires on 81% of Rust work and a nearly-always-red flag is ignored.
    """
    if not isinstance(coverage, dict):
        return "recorded task verification unavailable"
    rust = _as_count(coverage.get("rust_tasks"))
    checked = _as_count(coverage.get("rust_tasks_with_fmt_check"))
    if rust == 0:
        return "no cargo-verified task in the window to explain it"
    return (
        f"in the same window {rust - checked} of {rust} cargo-verified task(s) recorded "
        "no rustfmt check, so run `cargo fmt` during the task, before the engine commits"
    )


def unbaselined_issues(latest: dict[str, Any], entries: Any) -> list[str]:
    """Return `code@milestone` labels not covered by an accepted-divergence entry.

    Public because the governor's `doctor-baseline` command reports the same list the
    flag does: an operator must be able to see exactly what would be accepted before
    accepting it, and one implementation keeps the two views identical.

    Suppression is scoped on purpose. An entry with `scope: code` accepts a code
    everywhere and is the deliberate, documented exception; an entry carrying
    `milestones` accepts that code only for those milestones, so a *new* occurrence
    inside current work still surfaces. Baseline by bare code would silently hide
    fresh divergence in the active milestone, which is the failure mode this check
    exists to prevent.
    """
    accepted: list[dict[str, Any]] = [item for item in entries if isinstance(item, dict)]
    code_wide = {entry.get("code") for entry in accepted if entry.get("scope") == "code"}
    scoped: dict[str, set[str]] = {}
    for entry in accepted:
        if entry.get("scope") == "code":
            continue
        milestones = entry.get("milestones")
        if isinstance(milestones, list):
            scoped.setdefault(str(entry.get("code")), set()).update(
                str(milestone) for milestone in milestones
            )

    fresh: list[str] = []
    for issue in latest.get("issues") or []:
        if not isinstance(issue, dict):
            continue
        code = str(issue.get("code") or "")
        if not code or code in code_wide:
            continue
        milestones = _MILESTONE_RE.findall(str(issue.get("message") or ""))
        # Each match is a group tuple because the pattern has two alternatives.
        labels = {
            f"M{number}"
            for match in milestones
            for number in (match[0] or match[1],)
            if number
        }
        if labels and any(label in scoped.get(code, set()) for label in labels):
            continue
        label = ",".join(sorted(labels)) or "unscoped"
        fresh.append(f"{code}@{label}")
    return sorted(set(fresh))


def _contract_problem(contract: Any) -> str | None:
    """Explain why the contract cannot be trusted, or None when it can."""
    if not isinstance(contract, dict):
        return "contract is not a mapping"

    signals = contract.get("signals")
    if not isinstance(signals, list) or not signals:
        # An empty vocabulary means the oracle cannot name what it sees.
        return "signals must be a non-empty list (closed signal vocabulary)"

    thresholds = contract.get("thresholds")
    if not isinstance(thresholds, dict):
        return "thresholds must be a mapping"
    missing = [key for key in _REQUIRED_THRESHOLDS if key not in thresholds]
    if missing:
        return f"thresholds missing {sorted(missing)}"
    bad = [key for key in _REQUIRED_THRESHOLDS if not isinstance(thresholds[key], int)]
    if bad:
        return f"thresholds must be integers: {sorted(bad)}"

    markers = contract.get("journal_markers")
    if not isinstance(markers, dict):
        return "journal_markers must be a mapping"
    absent = [key for key in _REQUIRED_JOURNAL_MARKERS if not markers.get(key)]
    if absent:
        return f"journal_markers missing {sorted(absent)}"

    if not isinstance(contract.get("window_days"), int) or contract["window_days"] < 1:
        return "window_days must be a positive integer"

    retention = contract.get("retention")
    if not isinstance(retention, dict):
        return "retention must be a mapping"
    absent_retention = [
        key
        for key in _REQUIRED_RETENTION_KEYS
        if not isinstance(retention.get(key), int) or retention.get(key) < 0
    ]
    if absent_retention:
        return f"retention entries must be non-negative integers: {sorted(absent_retention)}"

    if not isinstance(contract.get("doctor_baseline_path"), str):
        return "doctor_baseline_path must be a non-empty string"
    if not contract["doctor_baseline_path"]:
        return "doctor_baseline_path must be a non-empty string"

    return None


def _as_count(value: Any) -> int:
    """Coerce an observed count; anything unusable reads as 0 rather than raising."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return 0
    return int(value)
