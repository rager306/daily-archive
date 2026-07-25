"""M231 S01: map preprocess language codes to YAKE lan."""

from __future__ import annotations

from research_graph.workflows.composition.yake_keyword_inject import (
    yake_language_code,
)


def test_known_languages_map() -> None:
    assert yake_language_code("en") == "en"
    assert yake_language_code("ru") == "ru"
    assert yake_language_code("de") == "de"
    assert yake_language_code("fr") == "fr"


def test_case_and_region_normalized() -> None:
    assert yake_language_code("EN") == "en"
    assert yake_language_code("en-US") == "en"
    assert yake_language_code("ru_RU") == "ru"


def test_unknown_and_empty_fall_back_to_en() -> None:
    assert yake_language_code("unknown") == "en"
    assert yake_language_code("") == "en"
    assert yake_language_code("xx") == "en"
    assert yake_language_code(None) == "en"  # type: ignore[arg-type]
