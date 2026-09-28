"""Bounded reader for `.gsd/git-action-failures.log` — the commit-failure record.

The engine appends one block per failed git action. The 2026-09-20..27 review found
141 of 188 recorded commit failures were the `cargo-fmt` pre-commit hook, and the
failure is not benign: a failed auto-commit cannot be retried by the engine, so the
task attempt dies with a claim mismatch and auto-mode wedges (documented in
KNOWLEDGE and MEM1839). Until this reader existed, that class was visible only to
whoever thought to grep the log.

Actionability follows the same commit-boundary rule as `GsdExecReader`: only failures
recorded *after* the latest commit are live, because a later commit is the boundary
that resolved the earlier ones. That also keeps the signal self-clearing — commit and
it goes quiet — instead of red forever.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

_MAX_LOG_BYTES = 2_000_000
_BLOCK_HEADER = re.compile(r"^\[([0-9T:\-.]+Z)\]\s+action=(\S+)")
_EXIT_CODE = re.compile(r"exit code:\s*(\d+)")
# Pre-commit writes one `- hook id: <id>` block per hook that did not pass, and each
# such block is followed by `- exit code: N`. A `- hook id:` line with no exit code
# after it is NOT a refusal: measured over all 188 recorded blocks, `docs-drift-check`
# appears that way in 188 of them and refused nothing, while `cargo-fmt` refused 141.
# Counting bare `hook id` lines would name a hook that never failed.
_HOOK_ID = re.compile(r"-\s*hook id:\s*([\w\-.]+)")
_EXIT_CODE_LINE = re.compile(r"^\s*-\s*exit code:\s*\d+")
_MAX_HOOK_SCAN_LINES = 400


@dataclass(frozen=True)
class CommitFailure:
    """One failed git action and the hooks that refused it."""

    timestamp: str
    action: str
    hooks: tuple[str, ...]
    exit_code: int | None


class GitActionFailuresReader:
    """Reads failed git actions from the engine's append-only log."""

    def __init__(self, project_root: Path):
        self.path = Path(project_root) / ".gsd" / "git-action-failures.log"

    def failures_since(self, since: datetime | None) -> list[CommitFailure]:
        """Return failed actions newer than `since`, oldest first."""
        if not self.path.is_file():
            return []
        text = _read_tail(self.path, _MAX_LOG_BYTES)
        failures: list[CommitFailure] = []
        for block in _split_blocks(text):
            header = _BLOCK_HEADER.match(block)
            if header is None:
                continue
            timestamp = _parse_timestamp(header.group(1))
            if timestamp is None:
                continue
            if since is not None and timestamp <= since:
                continue
            exit_codes = [int(value) for value in _EXIT_CODE.findall(block)]
            failures.append(
                CommitFailure(
                    timestamp=header.group(1),
                    action=header.group(2),
                    hooks=refusing_hooks(block),
                    exit_code=exit_codes[0] if exit_codes else None,
                )
            )
        return failures


def refusing_hooks(block: str) -> tuple[str, ...]:
    """Hook ids that actually refused this block, in first-seen order.

    A hook refused when its `- hook id:` line is immediately followed by an
    `- exit code:` line. `docs-drift-check` is written at the end of every block
    without one and has never refused a commit, so a looser parse reports a hook that
    did not fail. Only the head of the block is scanned: the tail carries the skipped
    summary the engine appends.
    """
    lines = block.splitlines()[:_MAX_HOOK_SCAN_LINES]
    refusing: list[str] = []
    for index, line in enumerate(lines):
        match = _HOOK_ID.match(line.strip())
        if match is None:
            continue
        following = lines[index + 1] if index + 1 < len(lines) else ""
        if _EXIT_CODE_LINE.match(following):
            refusing.append(match.group(1))
    return tuple(refusing)


def hook_counts(failures: list[CommitFailure]) -> dict[str, int]:
    """Count failures per refusing hook, so a report can name the dominant one."""
    counts: dict[str, int] = {}
    for failure in failures:
        for hook in failure.hooks or ("(no hook id recorded)",):
            counts[hook] = counts.get(hook, 0) + 1
    return counts


def _split_blocks(text: str) -> list[str]:
    """Split on the `[timestamp] action=` headers the engine writes."""
    blocks: list[str] = []
    current: list[str] = []
    for line in text.splitlines():
        if _BLOCK_HEADER.match(line):
            if current:
                blocks.append("\n".join(current))
            current = [line]
        elif current:
            current.append(line)
    if current:
        blocks.append("\n".join(current))
    return blocks


def _read_tail(path: Path, limit: int) -> str:
    try:
        size = path.stat().st_size
        with path.open("rb") as handle:
            if size > limit:
                handle.seek(size - limit)
            return handle.read().decode("utf-8", errors="replace")
    except OSError:
        return ""


def _parse_timestamp(value: str) -> datetime | None:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)
