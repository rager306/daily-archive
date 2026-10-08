"""Scorecard tests: the three axes and the four governor improvements.

The scorecard has one job: turn "how far has the milestone come" and "did the tree
regress" into numbers and findings a session reads before any work starts. These
tests pin each rule against inline fixtures so a green result never depends on the
live tree's current membership, then assert the live tree still exposes all three
axes and all five improvements with values and definitions.
"""

from __future__ import annotations

import json
import os
import time
from pathlib import Path
from types import SimpleNamespace

from governor.adapters.clippy_scanner import ClippyScanner
from governor.adapters.improvement_reader import (
    _has_test,
    last_milestone_close,
    observe_coverage,
    observe_momentum,
    read_readme,
)
from governor.adapters.git_reader import GitReader
from governor.checks.governor_improvements import (
    COVERAGE_DELTA_SIGNAL,
    DEAD_CODE_SIGNAL,
    MOMENTUM_SIGNAL,
    README_SIGNAL,
    coverage_delta_finding,
    dead_code_finding,
    feature_gated_observation,
    momentum_finding,
    readme_gate_finding,
)

REPO_ROOT = Path(__file__).resolve().parents[2]

#: The four signals the scorecard's rules may emit, and the axis names it must
#: expose. Kept here so a renamed signal fails loudly instead of silently.
IMPROVEMENT_FLAG_NAMES = {
    MOMENTUM_SIGNAL,
    README_SIGNAL,
    DEAD_CODE_SIGNAL,
    COVERAGE_DELTA_SIGNAL,
}
AXIS_NAMES = ("measurability", "declaration_sync_gap", "feature_gated_coverage")
IMPROVEMENT_NAMES = (
    "momentum",
    "readme_gate",
    "dead_code",
    "coverage_delta",
    "feature_gated_coverage",
)


# ── fixtures ─────────────────────────────────────────────────────────────────


def _write(root: Path, rel: str, text: str) -> Path:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


class _FakeGit:
    """State-based git fake: one added untested source and a stale close."""

    def __init__(self, commits: int = 39, added: list[str] | None = None) -> None:
        self.commits = commits
        self.added = added or ["crates/da-x/src/orphan.rs"]

    def is_repo(self) -> bool:
        return True

    def commits_since_date(self, since: str) -> int:  # noqa: ARG002
        return self.commits

    def added_files_since(self, since: str, pathspec: str) -> list[str]:  # noqa: ARG002
        return list(self.added)

    def untracked_files(self, pathspec: str) -> list[str]:  # noqa: ARG002
        return []


class _FakePlanning:
    def __init__(self, milestone_id: str | None) -> None:
        self._id = milestone_id

    def read(self):
        return SimpleNamespace(current_milestone_id=self._id)


def _scorecard_contract_yaml() -> str:
    return (
        "scorecard:\n"
        "  momentum:\n"
        "    max_days_since_close: 7\n"
        "    max_commits_since_close: 0\n"
        "  readme:\n"
        '    required_section: "Next gate"\n'
        "  dead_code:\n"
        "    enabled: false\n"
        "    max_findings: 0\n"
        "  coverage_delta:\n"
        "    max_untested_sources: 0\n"
        "    pathspec: crates\n"
        "  signals:\n"
        "    - scorecard_momentum_drift\n"
        "    - scorecard_readme_gate_missing\n"
        "    - scorecard_dead_code\n"
        "    - scorecard_coverage_delta\n"
    )


def _build_scorecard(tmp_path: Path):
    from governor.application.trajectory_service import TrajectoryService

    _write(tmp_path, "scripts/governor/checks/gsd_control_plane.yaml", _scorecard_contract_yaml())
    # A README with no Next gate section -> the rule reports its absence.
    _write(tmp_path, "README.md", "# tmp project\n\nno gate section here\n")
    # An old milestone-level close -> momentum is stale.
    summary = _write(tmp_path, ".gsd/phases/500-x/500-SUMMARY.md", "# 500 close\n")
    old = time.time() - 30 * 86400
    os.utime(summary, (old, old))
    # A new source with no test -> coverage delta fires.
    _write(tmp_path, "crates/da-x/src/orphan.rs", "pub fn orphan() {}\n")
    # A cached clippy capture with one dead_code warning -> dead-code fires.
    _write(
        tmp_path,
        "artifacts/project-trajectory/dead-code.json",
        json.dumps(
            {
                "generated_at": "2026-10-08T00:00:00Z",
                "warnings": [
                    {
                        "crate": "da-x",
                        "file": "crates/da-x/src/orphan.rs",
                        "line": 1,
                        "message": "function `orphan` is never used",
                        "lint": "dead_code",
                    }
                ],
            }
        ),
    )

    service = TrajectoryService.__new__(TrajectoryService)
    service.root = tmp_path
    service.git_reader = _FakeGit()
    service.gsd_planning_reader = _FakePlanning("M500")
    return service._build_scorecard()


# ── momentum ─────────────────────────────────────────────────────────────────


def test_momentum_reports_drift_when_close_stale_and_commits_exist():
    finding = momentum_finding(
        commits_since_close=39,
        days_since_close=30.0,
        max_days_since_close=7,
        max_commits_since_close=0,
    )
    assert finding is not None
    assert finding.signal == MOMENTUM_SIGNAL
    assert finding.severity == "medium"
    assert "39" in finding.evidence


def test_momentum_is_quiet_when_the_close_is_fresh():
    assert (
        momentum_finding(
            commits_since_close=39,
            days_since_close=3.5,
            max_days_since_close=7,
            max_commits_since_close=0,
        )
        is None
    )


def test_momentum_is_quiet_when_no_commits_landed():
    assert (
        momentum_finding(
            commits_since_close=0,
            days_since_close=30.0,
            max_days_since_close=7,
            max_commits_since_close=0,
        )
        is None
    )


def test_momentum_is_unknown_without_a_close():
    assert (
        momentum_finding(
            commits_since_close=5,
            days_since_close=None,
            max_days_since_close=7,
            max_commits_since_close=0,
        )
        is None
    )


# ── README gate reference ────────────────────────────────────────────────────


def test_readme_rule_reports_an_absent_section():
    finding = readme_gate_finding(
        readme_text="# project\n\nno gate here\n",
        required_section="Next gate",
        active_milestone_id="M516-th0oby",
    )
    assert finding is not None
    assert finding.signal == README_SIGNAL
    assert "has no" in finding.evidence


def test_readme_rule_reports_a_stale_milestone_reference():
    finding = readme_gate_finding(
        readme_text="# project\n\n## Next gate\n\nPhase 3 active\n",
        required_section="Next gate",
        active_milestone_id="M516-th0oby",
    )
    assert finding is not None
    assert "does not reference" in finding.evidence
    assert "M516-th0oby" in finding.evidence


def test_readme_rule_is_quiet_when_the_section_names_the_active_milestone():
    assert (
        readme_gate_finding(
            readme_text="# project\n\n## Next gate (trajectory enforcement)\n\nM516-th0oby\n",
            required_section="Next gate",
            active_milestone_id="M516-th0oby",
        )
        is None
    )


# ── dead code ────────────────────────────────────────────────────────────────


_CLIPPY_DEAD_CODE_OUTPUT = (
    "warning: function `unused_fn` is never used\n"
    "   --> crates/da-domain/src/foo.rs:10:4\n"
    "    |\n"
    " 10 | fn unused_fn() {}\n"
    "    |    ^^^^^^^^^\n"
    "    |\n"
    "    = note: `#[warn(dead_code)]` on by default\n"
)


def test_dead_code_rule_reports_on_a_deliberately_unused_item():
    """Real clippy output for an unused fn parses and fires the rule."""
    scanner = ClippyScanner(Path("."))
    warnings = scanner._parse_warnings(_CLIPPY_DEAD_CODE_OUTPUT, "da-domain")
    assert len(warnings) == 1
    assert warnings[0].lint == "dead_code"
    finding = dead_code_finding(warnings, max_findings=0)
    assert finding is not None
    assert finding.signal == DEAD_CODE_SIGNAL
    assert "unused_fn" in finding.evidence


def test_dead_code_rule_is_quiet_on_a_clean_scan():
    assert dead_code_finding([], max_findings=0) is None


def test_dead_code_rule_ignores_non_dead_code_warnings():
    class _W:
        crate = "da-domain"
        file = "crates/da-domain/src/foo.rs"
        line = 1
        message = "needless return"
        lint = "clippy::needless_return"

    assert dead_code_finding([_W()], max_findings=0) is None


# ── coverage delta ───────────────────────────────────────────────────────────


def test_coverage_delta_rule_reports_a_new_source_without_a_test():
    finding = coverage_delta_finding(["crates/da-x/src/orphan.rs"], max_untested_sources=0)
    assert finding is not None
    assert finding.signal == COVERAGE_DELTA_SIGNAL
    assert "orphan.rs" in finding.evidence


def test_coverage_delta_rule_is_quiet_when_every_new_source_is_tested():
    assert coverage_delta_finding([], max_untested_sources=0) is None


def test_has_test_recognizes_inline_and_missing(tmp_path: Path):
    tested = _write(tmp_path, "crates/da-x/src/with_test.rs", "#[cfg(test)]\nmod tests {}\n")
    untested = _write(tmp_path, "crates/da-x/src/no_test.rs", "pub fn f() {}\n")
    assert _has_test(tmp_path, str(tested.relative_to(tmp_path))) is True
    assert _has_test(tmp_path, str(untested.relative_to(tmp_path))) is False


def test_observe_coverage_names_only_untested_new_sources(tmp_path: Path):
    _write(tmp_path, "crates/da-x/src/covered.rs", "#[test]\nfn t() {}\n")
    _write(tmp_path, "crates/da-x/src/bare.rs", "pub fn t() {}\n")
    git = _FakeGit(added=["crates/da-x/src/covered.rs", "crates/da-x/src/bare.rs"])
    obs = observe_coverage(tmp_path, git, since_date="2026-10-04 23:08")
    assert set(obs.new_sources) == {
        "crates/da-x/src/covered.rs",
        "crates/da-x/src/bare.rs",
    }
    assert obs.untested == ("crates/da-x/src/bare.rs",)


# ── reader ───────────────────────────────────────────────────────────────────


def test_last_milestone_close_uses_only_milestone_level_summaries(tmp_path: Path):
    old = _write(tmp_path, ".gsd/phases/500-x/500-SUMMARY.md", "x")
    newer_slice = _write(tmp_path, ".gsd/phases/500-x/S06-T04-SUMMARY.md", "y")
    now = time.time()
    os.utime(old, (now - 1000, now - 1000))
    os.utime(newer_slice, (now, now))
    close = last_milestone_close(tmp_path)
    assert close is not None
    assert close[0].name == "500-SUMMARY.md"


def test_observe_momentum_degrades_to_unknown_without_a_close(tmp_path: Path):
    obs = observe_momentum(tmp_path, _FakeGit())
    assert obs.days_since_close is None
    assert obs.since_date is None


def test_read_readme_returns_none_when_absent(tmp_path: Path):
    assert read_readme(tmp_path) is None


# ── integration: the four rules flag, the fifth is named ─────────────────────


def test_build_scorecard_emits_all_four_improvement_findings(tmp_path: Path):
    scorecard, flags = _build_scorecard(tmp_path)
    names = {flag.flag for flag in flags}
    assert IMPROVEMENT_FLAG_NAMES <= names
    assert all(flag.severity == "medium" for flag in flags if flag.flag in IMPROVEMENT_FLAG_NAMES)
    entries = {entry["name"]: entry for entry in scorecard["improvements"]}
    assert set(entries) == set(IMPROVEMENT_NAMES)
    assert entries["momentum"]["status"] == "drift"
    assert entries["readme_gate"]["status"] == "drift"
    assert entries["dead_code"]["status"] == "drift"
    assert entries["coverage_delta"]["status"] == "drift"


def test_feature_gated_improvement_names_zero_executing_targets():
    entry = feature_gated_observation(
        executed=19,
        gated=31,
        zero_executing=["da-adapters/tests/hub_ocr_probe.rs"],
    )
    assert entry.name == "feature_gated_coverage"
    assert entry.status == "attention"
    assert "19/31" in entry.value
    assert "hub_ocr_probe" in entry.evidence


# ── live tree: the scorecard is real ─────────────────────────────────────────


def test_live_scorecard_exposes_three_axes_and_five_improvements():
    from governor.application.trajectory_service import TrajectoryService

    service = TrajectoryService.__new__(TrajectoryService)
    service.root = REPO_ROOT
    service.git_reader = GitReader(REPO_ROOT)
    from governor.adapters.filesystem_gsd_planning_reader import (
        FilesystemGsdPlanningReader,
    )

    service.gsd_planning_reader = FilesystemGsdPlanningReader(REPO_ROOT)
    scorecard, _flags = service._build_scorecard()

    axes = {entry["axis"]: entry for entry in scorecard["axes"]}
    assert set(axes) == set(AXIS_NAMES)
    for entry in axes.values():
        assert entry["value"], f"{entry['axis']} has no value"
        assert entry["definition"], f"{entry['axis']} has no definition"
        assert entry["lines"] and entry["lines"][0]
    # The three numbers are the point: 5/21, a gap count, and executed/gated.
    assert axes["measurability"]["value"].startswith("5/21")
    assert axes["declaration_sync_gap"]["value"].endswith("gaps")
    assert "/31 " in axes["feature_gated_coverage"]["value"]

    improvements = {entry["name"]: entry for entry in scorecard["improvements"]}
    assert set(improvements) == set(IMPROVEMENT_NAMES)
    for entry in improvements.values():
        assert entry["value"]
        assert entry["status"] in {"ok", "drift", "not_measured", "attention"}
        assert entry["definition"]
