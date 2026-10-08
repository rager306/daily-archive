"""Adapter: observations for the four governor improvements.

Reads the filesystem and git state the pure oracles in
``governor.checks.governor_improvements`` decide over:

* the newest milestone-level close summary under ``.gsd/phases/*/<MM>-SUMMARY.md``
  (``516-SUMMARY.md``, never a slice/task summary) and the commits landed since it;
* the README text;
* the source files added since that close and whether each carries a test.

Nothing here decides policy — that lives in the oracle module and the DATA
contract. This adapter only supplies observations, so a fixture repository can
substitute for the live tree.
"""

from __future__ import annotations

import time
from dataclasses import dataclass
from pathlib import Path

# Structural Rust files that carry wiring rather than behaviour; a new `lib.rs`
# or `main.rs` is not a module that owes a test of its own.
_STRUCTURAL_STEMS = frozenset({"lib", "main", "mod", "build"})

# The source root a coverage delta scans. Tests, benches and examples are not
# sources to be covered, so they never appear as "untested source".
_SOURCE_PATHSPEC = "crates"


@dataclass(frozen=True)
class MomentumObservation:
    """Commits since the last milestone close and how old that close is."""

    commits_since_close: int
    days_since_close: float | None
    last_close: str | None
    since_date: str | None


@dataclass(frozen=True)
class CoverageObservation:
    """New source files since the last close and the untested subset."""

    new_sources: tuple[str, ...]
    untested: tuple[str, ...]
    since_date: str | None


def last_milestone_close(root: Path) -> tuple[Path, float] | None:
    """Newest milestone-level SUMMARY under ``.gsd/phases/*/``.

    Only ``<MM>-SUMMARY.md`` files count. Slice (``S06-...``) and task
    (``S06-T04-...``) summaries are newer by construction and would move the
    "close" from when a milestone closed to when any task finished, which is the
    opposite of what momentum measures.
    """
    phases = root / ".gsd" / "phases"
    if not phases.is_dir():
        return None
    newest: tuple[Path, float] | None = None
    try:
        candidates = list(phases.glob("*/*-SUMMARY.md"))
    except OSError:
        return None
    for path in candidates:
        stem = path.name[: -len("-SUMMARY.md")]
        if not stem.isdigit():
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            continue
        if newest is None or mtime > newest[1]:
            newest = (path, mtime)
    return newest


def _since_date(mtime: float) -> str:
    """A git ``--since`` string local to the close; minute precision is enough."""
    return time.strftime("%Y-%m-%d %H:%M", time.localtime(mtime))


def observe_momentum(
    root: Path,
    git_reader=None,
    *,
    now: float | None = None,
) -> MomentumObservation:
    """Observe commits since the last close; degrade to unknown, never to clean.

    Without a repository or a close the result carries ``None`` fields so the
    oracle reports ``not_measured`` instead of a fabricated zero.
    """
    close = last_milestone_close(Path(root))
    if close is None:
        return MomentumObservation(0, None, None, None)
    path, mtime = close
    current = time.time() if now is None else now
    days = max(0.0, (current - mtime) / 86400.0)
    since = _since_date(mtime)
    commits = 0
    if git_reader is not None and _is_repo(git_reader):
        commits = git_reader.commits_since_date(since)
    return MomentumObservation(
        commits_since_close=commits,
        days_since_close=days,
        last_close=f"{path.parent.name}/{path.name}",
        since_date=since,
    )


def _is_repo(git_reader) -> bool:
    try:
        return bool(git_reader.is_repo())
    except Exception:  # a git probe must never take the report down
        return False


def read_readme(root: Path) -> str | None:
    """README text, or ``None`` when there is no README to read."""
    path = Path(root) / "README.md"
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None


def _is_source(rel: str) -> bool:
    """A new ``.rs`` file under ``crates/*/src/`` worth a test obligation."""
    parts = Path(rel).parts
    if len(parts) < 4 or parts[0] != "crates" or parts[2] != "src":
        return False
    if not rel.endswith(".rs"):
        return False
    return Path(rel).stem not in _STRUCTURAL_STEMS


def _has_test(root: Path, rel: str) -> bool:
    """True when the source carries or is paired with a test.

    A source counts as tested when it holds an inline ``#[test]`` /
    ``#[cfg(test)]`` module, when a same-stem file exists under the crate's
    ``tests/`` directory, or when a same-stem module directory holds one. This is
    deliberately generous: the flag must name a *deliberately added untested*
    source, not every private helper.
    """
    path = Path(root) / rel
    if not path.is_file():
        return True
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return True
    if "#[test]" in text or "#[cfg(test)]" in text:
        return True
    parts = Path(rel).parts
    crate = parts[1]
    stem = Path(rel).stem
    tests_dir = Path(root) / "crates" / crate / "tests"
    if tests_dir.is_dir():
        for candidate in tests_dir.glob("*.rs"):
            if candidate.stem == stem:
                return True
    module_dir = Path(root) / "crates" / crate / "src" / stem
    if module_dir.is_dir():
        for candidate in module_dir.rglob("*.rs"):
            try:
                nested = candidate.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            if "#[test]" in nested or "#[cfg(test)]" in nested:
                return True
    return False


def observe_coverage(
    root: Path,
    git_reader=None,
    *,
    since_date: str | None = None,
    pathspec: str = _SOURCE_PATHSPEC,
) -> CoverageObservation:
    """Source files added since ``since_date`` and the ones without a test.

    The committed adds and the untracked/staged adds are unioned, so a file added
    but not yet committed is caught now rather than after the next commit. Without
    a date or a repository the observation is empty and explicitly dated ``None``.
    """
    root = Path(root)
    if since_date is None or git_reader is None or not _is_repo(git_reader):
        return CoverageObservation((), (), since_date)
    discovered: list[str] = []
    for method in ("added_files_since", "untracked_files"):
        try:
            if method == "added_files_since":
                found = git_reader.added_files_since(since_date, pathspec)
            else:
                found = git_reader.untracked_files(pathspec)
        except Exception:  # a git probe must never take the report down
            found = []
        discovered.extend(found)
    sources = sorted({rel for rel in discovered if _is_source(rel)})
    untested = tuple(rel for rel in sources if not _has_test(root, rel))
    return CoverageObservation(tuple(sources), untested, since_date)
