"""Term-dense evidence window over keyword spans (M228 S02).

Picks a max_chars window maximizing keyword-hit count (yago-style
query-biased snippet idea). Application pure; never authorizes import.
"""

from __future__ import annotations

from dataclasses import dataclass

from research_graph.application.corpus.keyword_spans import KeywordSpan


@dataclass(frozen=True, slots=True)
class TermDenseWindow:
    """Local evidence snippet. Always import-blocked."""

    start: int
    end: int
    snippet: str
    hit_count: int
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("term dense window cannot authorize import/writes")
        if self.start < 0 or self.end < self.start:
            raise ValueError("invalid window offsets")


def term_dense_window(
    text: str,
    *,
    spans: tuple[KeywordSpan, ...] | list[KeywordSpan],
    max_chars: int = 320,
) -> TermDenseWindow:
    """Return window of at most max_chars maximizing keyword hit count."""
    if max_chars < 1:
        raise ValueError("max_chars must be >= 1")
    if not text:
        return TermDenseWindow(start=0, end=0, snippet="", hit_count=0)

    n = len(text)
    window = min(max_chars, n)
    if not spans:
        return TermDenseWindow(
            start=0,
            end=window,
            snippet=text[:window],
            hit_count=0,
        )

    # Candidate starts: 0 and each span start centered in window.
    candidates: set[int] = {0}
    for span in spans:
        center = (span.start + span.end) // 2
        start = max(0, min(n - window, center - window // 2))
        candidates.add(start)

    best_start = 0
    best_hits = -1
    for start in sorted(candidates):
        end = start + window
        hits = sum(1 for s in spans if s.start >= start and s.end <= end)
        if hits > best_hits or (hits == best_hits and start < best_start):
            best_hits = hits
            best_start = start

    end = best_start + window
    return TermDenseWindow(
        start=best_start,
        end=end,
        snippet=text[best_start:end],
        hit_count=max(0, best_hits),
    )


__all__ = ["TermDenseWindow", "term_dense_window"]
