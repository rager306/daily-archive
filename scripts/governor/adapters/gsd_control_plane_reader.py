"""Bounded reader that turns the GSD runtime stores into control-plane observations.

Companion to `governor.checks.gsd_control_plane`: this adapter owns all filesystem
access, the oracle owns all decisions. Every read is best-effort — a missing or
unreadable store degrades to a zero/None observation instead of raising, because a
governor run must not fail because a log rotated away.

Bounds, because these stores grow without limit by design:

- only journal files inside the window are opened, and each is read whole only up
  to `_MAX_JOURNAL_BYTES` — the window filter is by filename date, which the engine
  writes as `YYYY-MM-DD.jsonl`;
- directory counts use `os.scandir` and never stat individual entries.

The observation payload is deliberately flat and JSON-serialisable so it can be
committed as a fixture and asserted in tests.
"""

from __future__ import annotations

import json
import os
from datetime import date, datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from governor.adapters.retention_store_reader import unretained_counts
from governor.adapters.git_action_failures_reader import GitActionFailuresReader, hook_counts
from governor.adapters.task_verification_reader import rust_verification_coverage
from governor.domain.retention import RetentionPolicy

_MAX_JOURNAL_BYTES = 4_000_000
_MAX_DOCTOR_BYTES = 2_000_000
_MAX_DOCTOR_ISSUES = 60
_TMP_SCRATCH_PREFIX = "governor-test-"


class GsdControlPlaneReader:
    """Collects control-plane observations from the GSD stores for one project."""

    def __init__(self, project_root: Path, tmp_dir: Path | None = None):
        self.root = Path(project_root)
        self.gsd_dir = self.root / ".gsd"
        self.tmp_dir = Path(tmp_dir) if tmp_dir is not None else Path("/tmp")

    def observe(
        self,
        window_days: int,
        journal_markers: dict[str, str],
        baseline_path: str = ".gsd/doctor-baseline.json",
        policy: RetentionPolicy | None = None,
        commit_since: datetime | None = None,
    ) -> dict[str, Any]:
        """Return the observation payload the oracle expects.

        `baseline_path` is resolved against the project root, so the accepted-
        divergence record can live inside the repository where it is versioned and
        reviewable, instead of in the untracked `.gsd` runtime directory. `policy`
        comes from the same contract the oracle validates, so the count that fires and
        the command that clears it share one definition. `commit_since` is the commit
        boundary for the git-failure class: a later commit resolves earlier failures,
        which keeps that signal self-clearing rather than permanently red.
        """
        baseline = self._baseline_file(baseline_path)
        # One counting pass: each call scans the store, so calling it twice per store
        # would double the report cost for no extra information.
        unretained = unretained_counts(self.root, policy)
        failures = GitActionFailuresReader(self.root).failures_since(commit_since)
        return {
            "journal_counts": self._journal_counts(window_days, journal_markers),
            "unretained_exec_captures": unretained["unretained_exec_captures"],
            "unretained_quarantine_runs": unretained["unretained_quarantine_runs"],
            "tmp_scratch_dirs": self._count_prefixed_dirs(self.tmp_dir, _TMP_SCRATCH_PREFIX),
            "orphan_projection_dir": (self.gsd_dir / "gsd").is_dir(),
            "doctor_latest": self.latest_doctor_run(),
            "doctor_baseline_entries": self._doctor_baseline_entries(baseline),
            "doctor_baseline_present": baseline.is_file(),
            "commit_failures": len(failures),
            "commit_failure_hooks": hook_counts(failures),
            "rust_verification": rust_verification_coverage(self.root, window_days),
        }

    def _baseline_file(self, baseline_path: str) -> Path:
        candidate = Path(baseline_path)
        return candidate if candidate.is_absolute() else self.root / candidate

    # ── journals ────────────────────────────────────────────────────────────

    def _journal_counts(self, window_days: int, journal_markers: dict[str, str]) -> dict[str, int]:
        counts = {key: 0 for key in journal_markers}
        journal_dir = self.gsd_dir / "journal"
        if not journal_dir.is_dir() or window_days < 1:
            return counts
        cutoff = date.today() - timedelta(days=window_days - 1)
        for path in sorted(journal_dir.glob("*.jsonl")):
            if not _journal_in_window(path.name, cutoff):
                continue
            text = _read_prefix(path, _MAX_JOURNAL_BYTES)
            if not text:
                continue
            for key, marker in journal_markers.items():
                if marker:
                    counts[key] = counts.get(key, 0) + text.count(marker)
        return counts

    # ── doctor ──────────────────────────────────────────────────────────────

    def latest_doctor_run(self) -> dict[str, Any] | None:
        """The most recent engine-doctor record, or None when there is no history.

        Public because `governor doctor-baseline` reports the same record the flag
        does, without paying for the retention scan `observe` also performs.
        """
        history = self.gsd_dir / "doctor-history.jsonl"
        if not history.is_file():
            return None
        text = _read_suffix(history, _MAX_DOCTOR_BYTES)
        for line in reversed(text.splitlines()):
            line = line.strip()
            if not line:
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(record, dict):
                continue
            issues = [
                {
                    "code": str(issue.get("code")),
                    "severity": str(issue.get("severity") or ""),
                    "message": str(issue.get("message") or "")[:400],
                }
                for issue in (record.get("issues") or [])
                if isinstance(issue, dict) and issue.get("code")
            ]
            return {
                "ts": record.get("ts"),
                "ok": record.get("ok"),
                "errors": record.get("errors"),
                "warnings": record.get("warnings"),
                # The engine persists only a sample of its findings, so any
                # baseline comparison over this list is best-effort by
                # construction, not a complete reconciliation.
                "issues": issues[:_MAX_DOCTOR_ISSUES],
                "codes": sorted({issue["code"] for issue in issues}),
            }
        return None

    def _doctor_baseline_entries(self, path: Path) -> list[dict[str, Any]]:
        """Accepted-divergence entries from the baseline file.

        Best-effort on purpose: a trajectory report must still render when the baseline
        is unreadable, so a broken file degrades to "nothing accepted" here.
        `governor doctor-baseline` deliberately does the opposite and refuses to
        operate on junk, because silently treating a broken baseline as empty would
        invite re-accepting divergence that is already recorded.
        """
        if not path.is_file():
            return []
        try:
            payload = json.loads(path.read_text(encoding="utf-8", errors="replace"))
        except (OSError, json.JSONDecodeError):
            return []
        return normalize_baseline_entries(payload)

    # ── bounded counting ────────────────────────────────────────────────────

    @staticmethod
    def _count_prefixed_dirs(directory: Path, prefix: str) -> int:
        return _count_entries(directory, want_dir=True, prefix=prefix)


def normalize_baseline_entries(payload: Any) -> list[dict[str, Any]]:
    """Normalise a baseline payload into `{code, scope, milestones}` entries.

    One normaliser for both consumers: the oracle (through the reader) and the
    `doctor-baseline` command. An entry's scope is explicit when written and inferred
    from a `milestones` list otherwise, so a hand-written file still behaves.
    """
    if not isinstance(payload, dict):
        return []
    accepted = payload.get("accepted")
    if not isinstance(accepted, list):
        return []
    entries: list[dict[str, Any]] = []
    for item in accepted:
        if not isinstance(item, dict) or not item.get("code"):
            continue
        milestones = item.get("milestones")
        entries.append(
            {
                "code": str(item["code"]),
                "scope": str(item.get("scope") or ("milestones" if milestones else "code")),
                "milestones": [str(m) for m in milestones] if isinstance(milestones, list) else [],
            }
        )
    return entries


def _count_entries(directory: Path, want_dir: bool, prefix: str | None = None) -> int:
    if not directory.is_dir():
        return 0
    total = 0
    try:
        with os.scandir(directory) as entries:
            for entry in entries:
                if prefix is not None and not entry.name.startswith(prefix):
                    continue
                try:
                    if entry.is_dir() is want_dir:
                        total += 1
                except OSError:
                    continue
    except OSError:
        return 0
    return total


def _journal_in_window(filename: str, cutoff: date) -> bool:
    """True when a `YYYY-MM-DD.jsonl` journal name falls inside the window."""
    stem = filename.removesuffix(".jsonl")
    try:
        return datetime.strptime(stem, "%Y-%m-%d").date() >= cutoff
    except ValueError:
        # An unexpected name is not silently skipped: journals are dated by the
        # engine, so a non-conforming name is outside the window contract.
        return False


def _read_prefix(path: Path, limit: int) -> str:
    try:
        with path.open("rb") as handle:
            return handle.read(limit).decode("utf-8", errors="replace")
    except OSError:
        return ""


def _read_suffix(path: Path, limit: int) -> str:
    """Read the tail of a growing line-delimited file (newest records last)."""
    try:
        size = path.stat().st_size
        with path.open("rb") as handle:
            if size > limit:
                handle.seek(size - limit)
            return handle.read().decode("utf-8", errors="replace")
    except OSError:
        return ""


def utc_now_iso() -> str:
    """Timestamp helper shared with the baseline writer."""
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
