"""Deduplication and alias-chain helpers for review/staging candidates."""

from __future__ import annotations

from collections.abc import MutableSequence
from typing import Any


def append_unique(values: MutableSequence[str], value: str) -> None:
    """Append ``value`` exactly once while preserving existing order."""
    if value not in values:
        values.append(value)


def ranges_overlap(left_start: int, left_end: int, right_start: int, right_end: int) -> bool:
    """Return whether two half-open character ranges overlap."""
    return left_start < right_end and right_start < left_end


def annotate_overlapping_signal_windows(locators: list[dict[str, Any]]) -> None:
    """Mark locators whose source spans overlap as ambiguous aliases.

    The mutation is intentionally local to staging artifacts: it exposes the
    alias decision in diagnostic fields without promoting either candidate to a
    graph fact or changing import eligibility.
    """
    coordinate_locators = [
        locator
        for locator in locators
        if locator.get("source_spans")
        and locator["source_spans"][0].get("coordinate_space") != "artifact_record"
        and isinstance(locator["source_spans"][0].get("char_start"), int)
        and isinstance(locator["source_spans"][0].get("char_end"), int)
    ]
    overlapped_ids: set[str] = set()
    for index, left in enumerate(coordinate_locators):
        left_span = left["source_spans"][0]
        for right in coordinate_locators[index + 1 :]:
            right_span = right["source_spans"][0]
            if left_span.get("source_id") != right_span.get("source_id"):
                continue
            if ranges_overlap(
                left_span["char_start"],
                left_span["char_end"],
                right_span["char_start"],
                right_span["char_end"],
            ):
                overlapped_ids.add(str(left["locator_id"]))
                overlapped_ids.add(str(right["locator_id"]))
    for locator in coordinate_locators:
        if locator["locator_id"] not in overlapped_ids:
            continue
        append_unique(locator["diagnostic_codes"], "overlapping_signal_window")
        append_unique(locator["source_spans"][0]["ambiguity_diagnostics"], "overlapping_signal_window")
        if locator["state"] not in {"missing_span", "repair_required"}:
            locator["state"] = "ambiguous_span"
            locator["support_level"] = "nearby_context"
            locator["uncertainty_label"] = "high"
            locator["review_queue_reason"] = "span_ambiguous"


__all__ = ["annotate_overlapping_signal_windows", "append_unique", "ranges_overlap"]
