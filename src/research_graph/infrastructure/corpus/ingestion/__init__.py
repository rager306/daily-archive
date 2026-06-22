"""Corpus ingestion loader stack public API."""

from research_graph.infrastructure.corpus.ingestion.fetchers import (
    ARXIV_PDF_BASE_URL,
    PDFDownloader,
    arxiv_pdf_url,
)
from research_graph.infrastructure.corpus.ingestion.loader import (
    ArticleLoadResult,
    ArticleLoadSource,
    ArticleOutcome,
    ArticleSourceMetadata,
    ArticleSourceType,
    ExtractionMode,
    FullTextIngestionResult,
    FullTextQualityReport,
    FullTextQualityStatus,
    FullTextSource,
    FullTextSourceType,
    assess_full_text_quality,
    classify_article_source,
    full_text_source_for_paper,
    ingest_full_text,
    load_article_source,
)
from research_graph.infrastructure.corpus.ingestion.logging import (
    ArticleEventLogger,
    ArticleJsonlLogger,
)

__all__ = [
    "ARXIV_PDF_BASE_URL",
    "ArticleEventLogger",
    "ArticleJsonlLogger",
    "ArticleLoadResult",
    "ArticleLoadSource",
    "ArticleOutcome",
    "ArticleSourceMetadata",
    "ArticleSourceType",
    "ExtractionMode",
    "FullTextIngestionResult",
    "FullTextQualityReport",
    "FullTextQualityStatus",
    "FullTextSource",
    "FullTextSourceType",
    "PDFDownloader",
    "arxiv_pdf_url",
    "assess_full_text_quality",
    "classify_article_source",
    "full_text_source_for_paper",
    "ingest_full_text",
    "load_article_source",
]
