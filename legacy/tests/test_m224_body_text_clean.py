"""M224 S01: pure body text clean helpers (no I/O, no LLM, no import)."""

from __future__ import annotations

from research_graph.application.corpus.body_text_clean import (
    clean_body_text,
    collapse_whitespace,
    dedupe_consecutive_lines,
    normalize_unicode,
)


def test_normalize_unicode_ligatures_and_smart_quotes() -> None:
    raw = "ﬁrst “quote” and — dash\u00a0space"
    out = normalize_unicode(raw)
    assert "fi" in out
    assert '"' in out
    assert "-" in out
    assert "\u00a0" not in out
    assert "\ufb01" not in out


def test_normalize_unicode_drops_control_chars() -> None:
    raw = "hello\x00world\x07"
    assert normalize_unicode(raw) == "helloworld"


def test_collapse_whitespace_preserves_paragraphs() -> None:
    raw = "a   b\t\tc\n\n\n\nd"
    out = collapse_whitespace(raw)
    assert "a b c" in out.replace("\n", " ") or out.startswith("a b c")
    assert "\n\n\n" not in out
    assert out.count("\n\n") <= 1 or "\n\n" in out


def test_dedupe_consecutive_lines_drops_repeated_headers() -> None:
    # Consecutive duplicates only (quant-mind style); non-consecutive kept.
    raw = "Title\nPage 3 of 12\nPage 3 of 12\nBody line\nPage 3 of 12\n"
    out = dedupe_consecutive_lines(raw)
    assert out.count("Page 3 of 12") == 2
    assert "\nPage 3 of 12\nPage 3 of 12\n" not in out
    assert "Body line" in out
    assert "Title" in out


def test_dedupe_does_not_remove_nonconsecutive_repeats() -> None:
    raw = "A\nB\nA\n"
    out = dedupe_consecutive_lines(raw)
    assert out.splitlines() == ["A", "B", "A"]


def test_clean_body_text_composes_ops() -> None:
    raw = "ﬁrst  line\n\n\nPage 1\nPage 1\nrest"
    result = clean_body_text(raw)
    assert "fi" in result.text
    assert result.text.count("Page 1") == 1
    assert "normalize_unicode" in result.ops
    assert "collapse_whitespace" in result.ops
    assert "dedupe_consecutive_lines" in result.ops
    assert result.import_eligible is False
    assert result.graph_writes_allowed is False


def test_clean_body_text_empty() -> None:
    result = clean_body_text("")
    assert result.text == ""
    assert result.import_eligible is False
