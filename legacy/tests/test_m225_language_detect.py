"""M225 S01: deterministic language detect helper."""

from __future__ import annotations

import pytest

from research_graph.application.corpus.language_detect import (
    LanguageDetectResult,
    detect_text_language,
)


def test_empty_is_unknown() -> None:
    result = detect_text_language("")
    assert result.language == "unknown"
    assert result.confidence == 0.0
    assert result.import_eligible is False


def test_english_sample() -> None:
    text = (
        "The method and results of this experiment show that graph neural networks "
        "are effective for citation prediction and molecular property tasks."
    )
    result = detect_text_language(text)
    assert result.language == "en"
    assert result.confidence >= 0.4


def test_russian_cyrillic_sample() -> None:
    text = (
        "Метод и результаты эксперимента показывают, что графовые нейронные сети "
        "эффективны для задач предсказания цитирования и свойств молекул."
    )
    result = detect_text_language(text)
    assert result.language == "ru"
    assert result.confidence >= 0.5


def test_short_latin_defaults_en_soft() -> None:
    result = detect_text_language("hello world")
    assert result.language in {"en", "unknown"}
    assert result.import_eligible is False


def test_result_rejects_import_true() -> None:
    with pytest.raises(ValueError, match="import"):
        LanguageDetectResult(
            language="en",
            confidence=0.9,
            import_eligible=True,
        )
