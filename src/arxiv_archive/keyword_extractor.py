from dataclasses import dataclass

import yake


@dataclass
class KeywordScore:
    """Represents a keyword and its YAKE score.

    Lower score indicates better/more relevant keyword.
    """

    keyword: str
    score: float


class KeywordExtractor:
    """YAKE-based keyword extractor for academic papers."""

    def __init__(self, language: str = "en", top_k: int = 20) -> None:
        """Initialize YAKE keyword extractor.

        Args:
            language: Language code for keyword extraction (default: "en").
            top_k: Default number of top keywords to extract (default: 20).
        """
        self.language = language
        self.top_k = top_k
        self._extractor = yake.KeywordExtractor(
            lan=language,
            top=top_k,
            # YAKE scoring parameters
            n=2,  # Max n-gram size
            dedupLim=0.7,  # Deduplication threshold
            dedupFunc="seqm",  # Deduplication function
            windowsSize=1,
        )

    def extract(self, text: str, top_k: int | None = None) -> list[tuple[str, float]]:
        """Extract keywords from text.

        Args:
            text: Input text to extract keywords from.
            top_k: Override default top_k for this call. Uses default if None.

        Returns:
            List of (keyword, score) tuples sorted by score (lower = better).
        """
        extractor = self._extractor
        if top_k is not None:
            # Create a temporary extractor with different top_k
            extractor = yake.KeywordExtractor(
                lan=self.language,
                top=top_k,
                n=2,
                dedupLim=0.7,
                dedupFunc="seqm",
                windowsSize=1,
            )
        keywords = extractor.extract_keywords(text)
        return [(kw, score) for kw, score in keywords]

    def extract_for_paper(self, title: str, abstract: str) -> list[str]:
        """Extract keywords from a paper's title and abstract.

        Combines title and abstract, extracts keywords, and returns
        just the keyword strings sorted by relevance.

        Args:
            title: Paper title.
            abstract: Paper abstract.

        Returns:
            List of keyword strings sorted by YAKE score (lower = better).
        """
        combined = f"{title} {abstract}"
        results = self.extract(combined, self.top_k)
        return [keyword for keyword, _score in results]
