"""Retention policy — how long each GSD runtime store may keep entries.

Pure value type: no IO and no filesystem access, matching `governor.domain`'s
contract. The policy is DATA in the governor contract
(`scripts/governor/checks/gsd_control_plane.yaml`), so the threshold that fires and
the command that clears it are read from one place.

The rule that a "store is too big" signal must be expressed as *unretained entries*
rather than total size lives here as intent, because an absolute size limit on a
store that grows daily is permanently red, and a flag nobody can clear is a flag
nobody reads.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

DEFAULT_DAYS_EXEC = 30
DEFAULT_DAYS_QUARANTINE = 30
DEFAULT_DAYS_ACTIVITY = 14
DEFAULT_MIN_AGE_HOURS = 24


@dataclass(frozen=True)
class RetentionPolicy:
    """How long each runtime store is allowed to keep entries."""

    days_exec: int = DEFAULT_DAYS_EXEC
    days_quarantine: int = DEFAULT_DAYS_QUARANTINE
    days_activity: int = DEFAULT_DAYS_ACTIVITY
    min_age_hours: int = DEFAULT_MIN_AGE_HOURS

    @classmethod
    def from_mapping(cls, mapping: Any) -> "RetentionPolicy":
        """Read a policy from the contract's `retention` block, ignoring junk.

        A missing or malformed block falls back to the defaults rather than raising:
        the retention windows are a housekeeping preference, and a typo in them must
        not take down the whole trajectory report.
        """
        if not isinstance(mapping, dict):
            return cls()

        def non_negative(key: str, fallback: int) -> int:
            value = mapping.get(key)
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                return fallback
            return value

        return cls(
            days_exec=non_negative("days_exec", DEFAULT_DAYS_EXEC),
            days_quarantine=non_negative("days_quarantine", DEFAULT_DAYS_QUARANTINE),
            days_activity=non_negative("days_activity", DEFAULT_DAYS_ACTIVITY),
            min_age_hours=non_negative("min_age_hours", DEFAULT_MIN_AGE_HOURS),
        )

    def exec_cutoff_seconds(self, reference: float) -> float:
        """Oldest mtime an exec capture may carry and still count as retained.

        The window is the *longer* of the retention window and the min-age floor,
        because each is an independent reason to keep something: `days_exec` says how
        far back retention reaches, and `min_age_hours` protects a store a live run may
        still be writing. Combining them with `min` would silently shorten a 30-day
        window to 24 hours and schedule almost the whole evidence store for deletion.
        """
        return reference - _window_seconds(self.days_exec, self.min_age_hours)

    def quarantine_cutoff_seconds(self, reference: float) -> float:
        """Oldest mtime a quarantined projection may carry and stay retained."""
        return reference - _window_seconds(self.days_quarantine, self.min_age_hours)

    def activity_cutoff_seconds(self, reference: float) -> float:
        """Oldest mtime an activity log may carry and stay retained."""
        return reference - _window_seconds(self.days_activity, self.min_age_hours)

    def scratch_cutoff_seconds(self, reference: float) -> float:
        """Newest mtime a scratch directory may carry and still be swept.

        Scratch is pure garbage, so only the floor applies: a directory touched within
        `min_age_hours` may belong to a live test run.
        """
        return reference - self.min_age_hours * 3600


def _window_seconds(days: int, min_age_hours: int) -> float:
    """Combine a retention window and a min-age floor into one duration.

    The longer duration wins: keeping something longer is always the safe direction,
    and `min_age_hours` only tightens the window when it exceeds it.
    """
    return max(days * 86400.0, min_age_hours * 3600.0)
