"""M224 S02: profile-scoped body quality diagnostics (soft only)."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.body_quality import (
    BodyQualityReport,
    assess_body_quality,
)


def _lorem_paragraph(n_words: int = 80) -> str:
    words = ["alpha", "beta", "gamma", "delta", "epsilon", "method", "result"]
    return " ".join(words[i % len(words)] for i in range(n_words))


def test_empty_text_flags_empty() -> None:
    report = assess_body_quality("", profile="web")
    assert report.status == "empty"
    assert "empty" in report.rule_hits
    assert report.import_eligible is False
    assert report.graph_writes_allowed is False


def test_web_flags_too_short() -> None:
    report = assess_body_quality("short junk", profile="web")
    assert report.profile == "web"
    assert "too_short" in report.rule_hits
    assert report.status in {"low_quality", "too_short"}


def test_scholarly_short_abstract_is_soft_not_hard_drop() -> None:
    # ~40 words — short abstract; web would flag, scholarly soft-scores only.
    text = " ".join(["token"] * 40)
    web = assess_body_quality(text, profile="web")
    sch = assess_body_quality(text, profile="scholarly")
    assert "too_short" in web.rule_hits
    assert "too_short" not in sch.rule_hits
    assert sch.status in {"ok", "soft_signal"}
    assert sch.import_eligible is False


def test_ok_body_web_and_scholarly() -> None:
    text = _lorem_paragraph(120)
    for profile in ("web", "scholarly"):
        report = assess_body_quality(text, profile=profile)  # type: ignore[arg-type]
        assert report.word_count >= 50
        assert report.status == "ok"
        assert report.import_eligible is False
        assert isinstance(report.scores.get("mean_word_len"), float)


def test_high_symbol_ratio_flagged() -> None:
    text = "!!! ### ... " * 40 + " " + _lorem_paragraph(30)
    report = assess_body_quality(text, profile="web")
    assert "high_symbol_ratio" in report.rule_hits or report.scores.get(
        "symbol_ratio", 0
    ) > 0.1


def test_ngram_repetition_flagged_on_spam() -> None:
    spam = ("buy now cheap " * 50).strip()
    report = assess_body_quality(spam, profile="web")
    assert "ngram_repetition" in report.rule_hits or report.status != "ok"


def test_unknown_profile_raises() -> None:
    with pytest.raises(ValueError, match="profile"):
        assess_body_quality("hello world " * 20, profile="unknown")  # type: ignore[arg-type]


def test_report_frozen_and_fail_closed_constructor() -> None:
    with pytest.raises(ValueError, match="import"):
        BodyQualityReport(
            profile="web",
            status="ok",
            word_count=10,
            scores={},
            rule_hits=(),
            import_eligible=True,
        )
