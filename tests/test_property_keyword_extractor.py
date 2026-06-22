"""Property-based tests for keyword extractor using Hypothesis."""

from hypothesis import Verbosity, given, settings
from hypothesis import strategies as st

from research_graph.infrastructure.retrieval.keyword_extractor import KeywordExtractor
from tests.helpers.modular_fixtures import FIXTURE_MARKDOWN


@settings(verbosity=Verbosity.verbose, max_examples=200)
@given(
    title=st.text(min_size=0, max_size=500),
    abstract=st.text(min_size=0, max_size=2000),
)
def test_extraction_never_crashes(title: str, abstract: str) -> None:
    """Keyword extraction must not crash on any text input."""
    extractor = KeywordExtractor()
    keywords = extractor.extract_for_paper(title, abstract)

    assert isinstance(keywords, list)
    assert all(isinstance(kw, str) for kw in keywords)


@settings(max_examples=300)
@given(text=st.text(min_size=10, max_size=500))
def test_keywords_are_strings_and_not_empty(text: str) -> None:
    """Extracted keywords must be non-empty strings."""
    extractor = KeywordExtractor()
    keywords = extractor.extract_for_paper(text, "")

    for kw in keywords:
        assert isinstance(kw, str), f"Keyword {kw!r} is not a string"
        assert len(kw) > 0, "Keyword must be non-empty"


@settings(max_examples=100)
@given(text=st.text(min_size=0, max_size=2000, alphabet=st.characters(whitelist_categories=["Lu", "Ll", "Nd", "Zs"])))
def test_empty_or_whitespace_handled(text: str) -> None:
    """Pure whitespace or empty text must not crash."""
    extractor = KeywordExtractor()
    keywords = extractor.extract_for_paper(text, text)
    assert isinstance(keywords, list)


@settings(max_examples=100)
@given(
    text=st.text(
        min_size=1,
        max_size=1000,
        alphabet=st.characters(
            whitelist_categories=["Lu", "Ll"],
            whitelist_characters=" ,.-"
        ),
    )
)
def test_repeated_word_increases_extraction(
    text: str,
) -> None:
    """Text with repeated domain words should extract those words."""
    extractor = KeywordExtractor()
    # Repeat a term 5 times in the text
    term = "graph neural network"
    repeated_text = f"{FIXTURE_MARKDOWN}\n{text} {' '.join([term] * 5)} {text}"

    keywords = extractor.extract_for_paper(repeated_text, "")

    # The repeated term should be in keywords (or its components)
    keyword_str = " ".join(keywords).lower()
    assert any(
        term[:10].lower() in keyword_str for _ in [1]
    ), f"Repeated term '{term}' not extracted from text with repetitions"
