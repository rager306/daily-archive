"""Locate keyword char spans in cleaned body text (M228 S01).

Casefold matching, ordered by start offset. No YAKE — keywords are injected.
Application pure; never authorizes import.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class KeywordSpan:
    """One occurrence of a keyword in body text."""

    keyword: str
    start: int
    end: int
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("keyword span cannot authorize import/writes")
        if self.start < 0 or self.end < self.start:
            raise ValueError("invalid span offsets")


@dataclass(frozen=True, slots=True)
class KeywordSpanResult:
    """All located spans. Always import-blocked."""

    spans: tuple[KeywordSpan, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("keyword span result cannot authorize import/writes")


def locate_keyword_spans(
    text: str,
    keywords: list[str] | tuple[str, ...],
    *,
    max_per_keyword: int = 8,
) -> KeywordSpanResult:
    """Find casefold occurrences of each keyword; ordered by start."""
    if not text or not keywords:
        return KeywordSpanResult(spans=())

    hay = text.casefold()
    found: list[KeywordSpan] = []
    for raw_kw in keywords:
        kw = raw_kw.strip()
        if not kw:
            continue
        needle = kw.casefold()
        if not needle:
            continue
        count = 0
        pos = 0
        while count < max_per_keyword:
            idx = hay.find(needle, pos)
            if idx < 0:
                break
            found.append(
                KeywordSpan(keyword=kw.casefold(), start=idx, end=idx + len(needle))
            )
            count += 1
            pos = idx + max(1, len(needle))

    found.sort(key=lambda s: (s.start, s.end, s.keyword))
    return KeywordSpanResult(spans=tuple(found))


__all__ = ["KeywordSpan", "KeywordSpanResult", "locate_keyword_spans"]
