"""Source acquisition fetchers for ingestion.

Formerly: src/arxiv_archive/ingestion/fetchers.py"""

from __future__ import annotations

import tempfile
from pathlib import Path

import httpx

ARXIV_PDF_BASE_URL = "https://arxiv.org/pdf"


class PDFDownloader:
    """Download and cache arXiv PDFs with content-type/signature validation."""

    DEFAULT_CACHE_DIR = Path.home() / ".arxiv_cache"

    def __init__(self, cache_dir: Path | None = None) -> None:
        self.cache_dir = cache_dir if cache_dir is not None else self.DEFAULT_CACHE_DIR
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def download(self, arxiv_id: str, pdf_url: str | None = None) -> Path:
        """Fetch one PDF into the cache and return its path.

        Existing cached files are returned without network access. HTTP, timeout,
        malformed response, and non-PDF response failures bubble to callers so
        fallback selection remains explicit at higher ingestion layers.
        """
        pdf_path = self.cache_dir / f"{arxiv_id}.pdf"
        if pdf_path.exists():
            return pdf_path

        url = pdf_url or arxiv_pdf_url(arxiv_id)
        client = httpx.Client(timeout=120.0, follow_redirects=True)
        try:
            response = client.get(url)
            response.raise_for_status()
            content_type = response.headers.get("content-type", "").casefold()
            if "pdf" not in content_type and not response.content.startswith(b"%PDF-"):
                raise ValueError(
                    f"arXiv PDF download for {arxiv_id} did not return a PDF: {content_type or 'unknown content-type'}"
                )
            _atomic_write_bytes(pdf_path, response.content)
        finally:
            client.close()

        return pdf_path


def _atomic_write_bytes(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_name: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "wb",
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as temp_file:
            temp_file.write(content)
            temp_name = temp_file.name
        Path(temp_name).replace(path)
    except Exception:
        if temp_name is not None:
            Path(temp_name).unlink(missing_ok=True)
        raise



def arxiv_pdf_url(arxiv_id: str) -> str:
    """Return the canonical arXiv PDF URL for an arXiv identifier."""
    return f"{ARXIV_PDF_BASE_URL}/{arxiv_id}"


__all__ = ["ARXIV_PDF_BASE_URL", "PDFDownloader", "arxiv_pdf_url"]
