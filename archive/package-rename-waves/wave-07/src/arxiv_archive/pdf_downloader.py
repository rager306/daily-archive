"""Compatibility delegates for PDF acquisition fetchers.

Formerly: src/arxiv_archive/pdf_downloader.py"""

from __future__ import annotations

from arxiv_archive.ingestion.fetchers import ARXIV_PDF_BASE_URL, PDFDownloader, arxiv_pdf_url

__all__ = ["ARXIV_PDF_BASE_URL", "PDFDownloader", "arxiv_pdf_url"]
