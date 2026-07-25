"""Deterministic outline signals from cleaned text (M225 S02).

Markdown ATX headings and numbered heading candidates. No PDF font path,
no model. Pattern inspired by quant-mind outline signals — reimplemented.

Application pure; never authorizes import.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

_ATX_RE = re.compile(r"^(#{1,6})\s+(\S.*)$")
_NUMBERED_RE = re.compile(r"^(\d+(?:\.\d+)*)[.)]?\s+(\S.+)$")


@dataclass(frozen=True, slots=True)
class OutlineHeading:
    """One ordered heading candidate."""

    text: str
    level: int
    source: str  # atx | numbered
    line_index: int


@dataclass(frozen=True, slots=True)
class OutlineSignals:
    """Ordered outline hints for structure prep. Always import-blocked."""

    headings: tuple[OutlineHeading, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("outline signals cannot authorize import/writes")


def _level_from_number(number: str) -> int:
    return min(6, max(1, number.count(".") + 1))


def extract_outline_signals(text: str) -> OutlineSignals:
    """Extract ATX and numbered heading candidates in document order."""
    if not text.strip():
        return OutlineSignals(headings=())

    headings: list[OutlineHeading] = []
    for idx, raw_line in enumerate(text.splitlines()):
        line = raw_line.strip()
        if not line:
            continue
        atx = _ATX_RE.match(line)
        if atx:
            headings.append(
                OutlineHeading(
                    text=atx.group(2).strip(),
                    level=len(atx.group(1)),
                    source="atx",
                    line_index=idx,
                )
            )
            continue
        numbered = _NUMBERED_RE.match(line)
        if numbered:
            # Avoid treating long prose starting with a year as heading.
            num, title = numbered.group(1), numbered.group(2).strip()
            if len(title) > 120:
                continue
            if len(title.split()) > 20:
                continue
            headings.append(
                OutlineHeading(
                    text=f"{num} {title}",
                    level=_level_from_number(num),
                    source="numbered",
                    line_index=idx,
                )
            )

    return OutlineSignals(headings=tuple(headings))


__all__ = ["OutlineHeading", "OutlineSignals", "extract_outline_signals"]
