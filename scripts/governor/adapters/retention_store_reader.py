"""Counting reader for GSD runtime stores — the "unretained" measure.

An adapter, so all filesystem access lives here rather than in `domain` (pure values)
or in the checker that consumes the count.

Why counting and not sizing: an absolute size limit on a store that grows every day
is permanently red, and a flag nobody can clear is a flag nobody reads. The count of
entries *past their retention window and still present* is the one measure a healthy
tree clears to zero, and it is exactly what `governor hygiene` removes.
"""

from __future__ import annotations

import os
import time
from pathlib import Path

from governor.domain.retention import RetentionPolicy


def unretained_counts(
    project_root: Path, policy: RetentionPolicy | None = None, *, now: float | None = None
) -> dict[str, int]:
    """Count entries past their retention window that have not been pruned yet.

    One `scandir` pass per store, measured at ~330 ms for exec plus quarantine on a
    22k-capture store, so the trajectory report keeps its "fast, ~1s" contract.
    """
    policy = policy or RetentionPolicy()
    gsd = Path(project_root) / ".gsd"
    reference = time.time() if now is None else now
    return {
        "unretained_exec_captures": count_unretained_entries(
            gsd / "exec", policy.exec_cutoff_seconds(reference), suffix=".meta.json"
        ),
        "unretained_quarantine_runs": count_unretained_entries(
            gsd / "quarantine" / "projections", policy.quarantine_cutoff_seconds(reference)
        ),
    }


def count_unretained_entries(directory: Path, cutoff: float, *, suffix: str | None = None) -> int:
    """Count directory entries with mtime older than `cutoff`, optionally filtered."""
    if not directory.is_dir():
        return 0
    total = 0
    try:
        with os.scandir(directory) as entries:
            for entry in entries:
                if suffix is not None and not entry.name.endswith(suffix):
                    continue
                try:
                    if entry.stat(follow_symlinks=False).st_mtime < cutoff:
                        total += 1
                except OSError:
                    continue
    except OSError:
        return 0
    return total
