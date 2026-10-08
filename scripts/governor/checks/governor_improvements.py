"""Four governor improvements as pure oracles over injected observations.

M516-th0oby/S06 installs the numeric scorecard (three axes) and four named
governor improvements next to the fifth, the feature-gated coverage axis built
by the sibling adapter. This module owns the four *decisions*:

* **momentum** — commits accumulated since the last milestone close, reported as
  drift once the close is older than the DATA threshold while commits exist.
* **README gate reference** — the mandatory ``Next gate`` section must reference
  the active milestone; an absent section is reported as absence, a section that
  names a different milestone is reported as a stale reference.
* **dead code** — clippy ``dead_code`` findings surface at ``medium`` severity so
  an unused item is visible in the trajectory report instead of only in a CLI
  print that nobody runs.
* **coverage delta** — source files added since the last close that carry no
  corresponding test.

Everything here is a pure function over observations supplied by
``governor.adapters.improvement_reader``. No filesystem, git or cargo access
happens in this module, so every rule can be tested against committed fixtures
(ADR-051: state-based fakes beat choreography). A missing observation is the
caller's problem to state explicitly; these functions never fabricate a clean
result — they only decline to emit a finding when the observation is genuinely
within policy.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, Sequence

# The closed vocabulary of signal ids these oracles may emit. The DATA contract
# (`gsd_control_plane.yaml#scorecard.signals`) lists the same names; a signal that
# is not declared there is a contract/code disagreement, not a silent pass.
MOMENTUM_SIGNAL = "scorecard_momentum_drift"
README_SIGNAL = "scorecard_readme_gate_missing"
DEAD_CODE_SIGNAL = "scorecard_dead_code"
COVERAGE_DELTA_SIGNAL = "scorecard_coverage_delta"

IMPROVEMENT_SIGNALS: tuple[str, ...] = (
    MOMENTUM_SIGNAL,
    README_SIGNAL,
    DEAD_CODE_SIGNAL,
    COVERAGE_DELTA_SIGNAL,
)

# Clippy's dead-code family is emitted with several message shapes depending on the
# item kind (function/struct/field/const). The lint name in the `#[warn(...)]` note
# is authoritative when present; the message markers are the fallback, because the
# scanner historically never populated `lint`.
_DEAD_CODE_MARKERS: tuple[str, ...] = (
    "never used",
    "never read",
    "never constructed",
    "is never used",
    "dead_code",
)


@dataclass(frozen=True)
class ImprovementFinding:
    """One improvement's finding, already severity-resolved."""

    signal: str
    severity: str
    evidence: str


@dataclass(frozen=True)
class ImprovementObservation:
    """A scorecard entry for one improvement: its value and its policy status.

    ``status`` is one of ``ok`` (measured and within policy), ``drift`` (measured
    and outside policy), or ``not_measured`` (the mechanism did not run, so the
    entry carries the reason instead of a false clean).
    """

    name: str
    value: str
    status: str
    definition: str
    evidence: str = ""

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "value": self.value,
            "status": self.status,
            "definition": self.definition,
            "evidence": self.evidence,
        }


# ── momentum ─────────────────────────────────────────────────────────────────


def momentum_finding(
    *,
    commits_since_close: int,
    days_since_close: float | None,
    max_days_since_close: int,
    max_commits_since_close: int,
) -> ImprovementFinding | None:
    """Drift when the last close is older than the threshold while commits exist.

    Two thresholds guard the finding so a quiet or a busy tree both read clearly:
    a close is stale only after ``max_days_since_close`` days, and only when more
    than ``max_commits_since_close`` commits landed since it. With a threshold of
    zero commits any commit after the stale-close window is drift, which is the
    "commits exist" reading the slice verify names.
    """
    if days_since_close is None:
        return None
    if commits_since_close <= max_commits_since_close:
        return None
    if days_since_close <= max_days_since_close:
        return None
    return ImprovementFinding(
        MOMENTUM_SIGNAL,
        "medium",
        f"{commits_since_close} commit(s) since the last milestone close, which is "
        f"{days_since_close:.1f} day(s) old (limit {max_days_since_close}d, "
        f"{max_commits_since_close} commit(s)); close a milestone or record the drift",
    )


def momentum_observation(
    *,
    commits_since_close: int,
    days_since_close: float | None,
    last_close: str | None,
    max_days_since_close: int,
    max_commits_since_close: int,
    finding: ImprovementFinding | None,
) -> ImprovementObservation:
    """The scorecard line for momentum, derived from the same decision."""
    if days_since_close is None:
        return ImprovementObservation(
            name="momentum",
            value="unknown",
            status="not_measured",
            definition=(
                "commits accumulated since the last milestone close over the DATA "
                "threshold beyond which the gap is reported as drift"
            ),
            evidence="no milestone close found under .gsd/phases/*/<MM>-SUMMARY.md",
        )
    return ImprovementObservation(
        name="momentum",
        value=f"{commits_since_close} commit(s) / {days_since_close:.1f}d",
        status="drift" if finding else "ok",
        definition=(
            "commits accumulated since the last milestone close over the DATA "
            "threshold beyond which the gap is reported as drift"
        ),
        evidence=f"last close {last_close or 'unknown'}",
    )


# ── README gate reference ────────────────────────────────────────────────────


def readme_gate_finding(
    *,
    readme_text: str | None,
    required_section: str,
    active_milestone_id: str | None,
) -> ImprovementFinding | None:
    """The required README section must exist and reference the active milestone.

    Absence of the section and a stale section that names no current milestone are
    two different findings, but both mean the same thing to a reader: the README's
    gate pointer cannot be trusted. They share one signal so the report does not
    double-count one problem.
    """
    if readme_text is None:
        return ImprovementFinding(
            README_SIGNAL,
            "medium",
            f"README.md is absent; the required '{required_section}' gate section "
            "and its active-milestone reference cannot be read",
        )
    body = _section_body(readme_text, required_section)
    if body is None:
        return ImprovementFinding(
            README_SIGNAL,
            "medium",
            f"README.md has no '{required_section}' section",
        )
    if not active_milestone_id:
        return None
    if active_milestone_id not in body:
        return ImprovementFinding(
            README_SIGNAL,
            "medium",
            f"README.md '{required_section}' section does not reference the active "
            f"milestone {active_milestone_id}",
        )
    return None


def readme_observation(
    *,
    readme_text: str | None,
    required_section: str,
    active_milestone_id: str | None,
    finding: ImprovementFinding | None,
) -> ImprovementObservation:
    if readme_text is None:
        value = "README absent"
    elif _section_body(readme_text, required_section) is None:
        value = f"'{required_section}' section absent"
    else:
        value = f"active milestone {active_milestone_id or '(unknown)'} referenced"
    return ImprovementObservation(
        name="readme_gate",
        value=value,
        status="drift" if finding else "ok",
        definition=(
            f"the README '{required_section}' section exists and references the "
            "active milestone, or its absence is reported"
        ),
        evidence=active_milestone_id or "",
    )


def _section_body(readme_text: str, section_name: str) -> str | None:
    """Body text of the first markdown heading whose text contains ``section_name``.

    The heading in README.md is ``## Next gate (trajectory enforcement)``, so an
    exact match on ``Next gate`` would never find it. Substring matching is
    deliberate; the caller's DATA names the stable prefix, not the prose suffix.
    """
    wanted = section_name.strip().lower()
    lines = readme_text.splitlines()
    start: int | None = None
    level = 0
    for index, line in enumerate(lines):
        stripped = line.strip()
        if not stripped.startswith("#"):
            continue
        heading = stripped.lstrip("#").strip().lower()
        if wanted in heading:
            start = index
            level = len(stripped) - len(stripped.lstrip("#"))
            break
    if start is None:
        return None
    body: list[str] = []
    for line in lines[start + 1 :]:
        stripped = line.strip()
        if stripped.startswith("#"):
            current = len(stripped) - len(stripped.lstrip("#"))
            if current <= level:
                break
        body.append(line)
    return "\n".join(body)


# ── dead code ────────────────────────────────────────────────────────────────


def _is_dead_code(warning) -> bool:
    """True for a clippy/rustc dead-code warning, by lint id or message shape.

    ``ClippyWarning`` is duck-typed (``lint`` + ``message``) so the oracle does not
    depend on the adapter class. ``lint`` is authoritative when the scanner
    populated it; the message markers cover older captures where it did not.
    """
    lint = (getattr(warning, "lint", "") or "").strip()
    if lint == "dead_code":
        return True
    message = (getattr(warning, "message", "") or "").lower()
    return any(marker in message for marker in _DEAD_CODE_MARKERS)


def dead_code_finding(
    warnings: Iterable,
    *,
    max_findings: int,
) -> ImprovementFinding | None:
    """Report clippy dead-code findings at medium severity, naming the items.

    The scan never fails close: a crate clippy could not compile yields fewer
    warnings, not a false clean, so the caller treats an empty scan as clean and a
    non-empty one as a finding. Only dead-code warnings are counted; other clippy
    lints stay with the existing clippy surfaces.
    """
    dead = dead_code_warnings(warnings)
    if len(dead) <= max_findings:
        return None
    sample = ", ".join(_describe(w) for w in dead[:8])
    remainder = f" (+{len(dead) - 8} more)" if len(dead) > 8 else ""
    return ImprovementFinding(
        DEAD_CODE_SIGNAL,
        "medium",
        f"{len(dead)} clippy dead_code finding(s) in our crates: {sample}{remainder}",
    )


def dead_code_warnings(warnings: Iterable) -> list:
    """The dead-code subset of a clippy warning list.

    Public so the scorecard entry counts exactly what the finding counts, instead
    of re-implementing the predicate and drifting from it.
    """
    return [w for w in warnings if _is_dead_code(w)]


def _describe(warning) -> str:
    crate = getattr(warning, "crate", "?")
    file = getattr(warning, "file", "?")
    line = getattr(warning, "line", 0)
    message = getattr(warning, "message", "")
    return f"{crate}:{file}:{line} ({message})"


def dead_code_observation(
    *,
    scanned: bool,
    findings: int,
    finding: ImprovementFinding | None,
    note: str = "",
) -> ImprovementObservation:
    if not scanned:
        return ImprovementObservation(
            name="dead_code",
            value="not scanned",
            status="not_measured",
            definition="clippy dead_code findings in our crates, at medium severity",
            evidence=note or "enable with scorecard.dead_code.enabled or run `governor --check-dead-code`",
        )
    return ImprovementObservation(
        name="dead_code",
        value=f"{findings} dead_code finding(s)",
        status="drift" if finding else "ok",
        definition="clippy dead_code findings in our crates, at medium severity",
        evidence=note,
    )


# ── test coverage delta ──────────────────────────────────────────────────────


def coverage_delta_finding(
    uncovered_sources: Sequence[str],
    *,
    max_untested_sources: int,
) -> ImprovementFinding | None:
    """Report source files added since the last close that carry no test.

    ``uncovered_sources`` are repo-relative paths the reader decided are new and
    untested; the oracle only decides whether the count is within policy, so the
    threshold lives in DATA and the file-selection policy stays in one reader.
    """
    if len(uncovered_sources) <= max_untested_sources:
        return None
    shown = ", ".join(uncovered_sources[:8])
    remainder = f" (+{len(uncovered_sources) - 8} more)" if len(uncovered_sources) > 8 else ""
    return ImprovementFinding(
        COVERAGE_DELTA_SIGNAL,
        "medium",
        f"{len(uncovered_sources)} source file(s) added since the last close carry no "
        f"test: {shown}{remainder}",
    )


def coverage_delta_observation(
    *,
    new_sources: int,
    uncovered: int,
    finding: ImprovementFinding | None,
    since_label: str,
) -> ImprovementObservation:
    return ImprovementObservation(
        name="coverage_delta",
        value=f"{uncovered} of {new_sources} new source(s) untested",
        status="drift" if finding else "ok",
        definition=(
            "source files added since the last milestone close that carry no "
            "corresponding test"
        ),
        evidence=since_label,
    )


# ── feature-gated coverage (the fifth improvement, built by the sibling axis) ──


def feature_gated_observation(
    *,
    executed: int,
    gated: int,
    zero_executing: Sequence[str],
) -> ImprovementObservation:
    """The fifth improvement as a scorecard entry.

    This does not emit a standing drift flag on purpose. The live tree has known
    feature combinations the default gate does not build (kg-storage
    algorithms/samyama, da-adapters pdf-layout/pdf-classifier/redb); a flag that
    can never clear is a flag nobody reads. The entry names the zero-executing
    targets so the M515 class is visible as a number, which is the point of the
    axis, while the four rules above own the findings.
    """
    return ImprovementObservation(
        name="feature_gated_coverage",
        value=(
            f"{executed}/{gated} gated targets execute tests; "
            f"{len(zero_executing)} execute zero"
        ),
        status="attention" if zero_executing else "ok",
        definition=(
            "gated test targets that execute a non-zero number of tests over all "
            "gated targets, with zero-executing targets named individually"
        ),
        evidence=", ".join(zero_executing[:8])
        + (f" (+{len(zero_executing) - 8} more)" if len(zero_executing) > 8 else ""),
    )
