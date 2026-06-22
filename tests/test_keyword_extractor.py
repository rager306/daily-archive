from pathlib import Path

from research_graph.infrastructure.corpus.ingestion import FullTextSource, ingest_full_text
from research_graph.infrastructure.corpus.parsing.parser import parse_article
from research_graph.infrastructure.papers.indexing import build_page_index
from research_graph.infrastructure.retrieval.keyword_extractor import KeywordExtractor

FULL_TEXT_FIXTURES = Path(__file__).parent / "fixtures" / "full_text"


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


def test_extract_for_parser_and_page_index_share_normalized_text_source() -> None:
    """Parser/PageIndex keyword entrypoints use the same normalized article text."""
    extractor = KeywordExtractor()
    ingestion = ingest_full_text(
        FullTextSource(
            paper_id="2605.12345",
            source_type="markdown",
            source_path=FULL_TEXT_FIXTURES / "structured_paper.md",
        )
    )
    parsed = parse_article(ingestion)
    document = build_page_index(ingestion)

    parsed_keywords = extractor.extract_for_parsed_article(parsed)
    page_index_keywords = extractor.extract_for_page_index(document)

    assert parsed_keywords == page_index_keywords
    assert any("local markdown" in keyword.lower() for keyword in parsed_keywords)
