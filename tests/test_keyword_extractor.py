import pytest

from arxiv_archive.keyword_extractor import KeywordExtractor, KeywordScore


def test_keyword_extractor_init() -> None:
    """Test KeywordExtractor initialization with default and custom parameters."""
    # Default initialization
    extractor = KeywordExtractor()
    assert extractor.language == "en"
    assert extractor.top_k == 20

    # Custom initialization
    extractor_custom = KeywordExtractor(language="es", top_k=10)
    assert extractor_custom.language == "es"
    assert extractor_custom.top_k == 10


def test_extract_keywords() -> None:
    """Test extract method returns list of (str, float) tuples."""
    extractor = KeywordExtractor()
    text = "graph neural networks for knowledge graphs and machine learning"

    results = extractor.extract(text)

    # Check result is a list
    assert isinstance(results, list)

    # Check each item is a tuple of (str, float)
    for item in results:
        assert isinstance(item, tuple)
        assert len(item) == 2
        keyword, score = item
        assert isinstance(keyword, str)
        assert isinstance(score, float)

    # Check keywords are from the text
    all_keywords = " ".join([k for k, _ in results]).lower()
    assert "graph" in all_keywords or "neural" in all_keywords or "learning" in all_keywords


def test_extract_for_paper() -> None:
    """Test extract_for_paper combines title and abstract."""
    extractor = KeywordExtractor()

    title = "Deep Learning for Natural Language Processing"
    abstract = (
        "This paper presents a survey of deep learning methods applied to NLP tasks. "
        "We cover neural networks, transformers, and attention mechanisms."
    )

    keywords = extractor.extract_for_paper(title, abstract)

    # Check result is a list of strings
    assert isinstance(keywords, list)
    for kw in keywords:
        assert isinstance(kw, str)

    # Should have extracted some keywords
    assert len(keywords) > 0
