"""Optional composition wire for paper summary stage (M254 S04).

Stage is OFF by default. Application package remains pure;
this module only exposes enablement policy for composition roots.
"""

from __future__ import annotations

DEFAULT_SUMMARY_STAGE_ENABLED = False


def should_run_summary_stage(*, enabled: bool | None = None) -> bool:
    """Return whether optional summary stage should run.

    ``None`` → default off. Explicit True required to enable.
    """
    if enabled is None:
        return DEFAULT_SUMMARY_STAGE_ENABLED
    return bool(enabled)


__all__ = ["DEFAULT_SUMMARY_STAGE_ENABLED", "should_run_summary_stage"]
