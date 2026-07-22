"""Source acquisition fetchers for ingestion.

Formerly: src/arxiv_archive/ingestion/fetchers.py"""

from __future__ import annotations

import re
import tempfile
from pathlib import Path

import httpx

ARXIV_PDF_BASE_URL = "https://arxiv.org/pdf"
ARXIV_HTML_BASE_URL = "https://arxiv.org/html"
ARXIV_ABS_BASE_URL = "https://arxiv.org/abs"

_ARXIV_ID_CORE = r"(?P<id>\d{4}\.\d{4,5}(?:v\d+)?|[a-z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?)"
_ARXIV_REF_RE = re.compile(
    rf"(?:https?://(?:export\.)?arxiv\.org/(?:abs|pdf|html)/)?"
    rf"(?:arxiv:)?"
    rf"{_ARXIV_ID_CORE}"
    rf"(?:\.pdf)?/?",
    re.IGNORECASE,
)


def normalize_arxiv_ref(value: str) -> str:
    """Normalize abs/pdf/html/arxiv: ids to a bare arXiv id (version kept)."""
    text = value.strip()
    match = _ARXIV_REF_RE.fullmatch(text) or _ARXIV_REF_RE.search(text)
    if match is None:
        raise ValueError(f"unrecognized arXiv reference: {value!r}")
    return match.group("id")


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
        arxiv_id = normalize_arxiv_ref(arxiv_id)
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
                    f"arXiv PDF download for {arxiv_id} did not return a PDF: "
                    f"{content_type or 'unknown content-type'}"
                )
            _atomic_write_bytes(pdf_path, response.content)
        finally:
            client.close()

        return pdf_path


class HTMLDownloader:
    """Download and cache arXiv HTML fulltext (ar5iv-style) for local load."""

    DEFAULT_CACHE_DIR = Path.home() / ".arxiv_cache" / "html"

    def __init__(self, cache_dir: Path | None = None) -> None:
        self.cache_dir = cache_dir if cache_dir is not None else self.DEFAULT_CACHE_DIR
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def download(self, arxiv_id: str, html_url: str | None = None) -> Path:
        """Fetch HTML into cache; return existing cache without network."""
        arxiv_id = normalize_arxiv_ref(arxiv_id)
        html_path = self.cache_dir / f"{arxiv_id}.html"
        if html_path.exists() and html_path.stat().st_size > 0:
            return html_path

        url = html_url or arxiv_html_url(arxiv_id)
        client = httpx.Client(timeout=120.0, follow_redirects=True)
        try:
            response = client.get(url)
            response.raise_for_status()
            content_type = response.headers.get("content-type", "").casefold()
            body = response.content
            if "html" not in content_type and b"<html" not in body[:2048].lower():
                raise ValueError(
                    f"arXiv HTML download for {arxiv_id} did not return HTML: "
                    f"{content_type or 'unknown content-type'}"
                )
            _atomic_write_bytes(html_path, body)
        finally:
            client.close()

        return html_path


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
    return f"{ARXIV_PDF_BASE_URL}/{normalize_arxiv_ref(arxiv_id)}"


def arxiv_html_url(arxiv_id: str) -> str:
    """Return the canonical arXiv HTML fulltext URL for an arXiv identifier."""
    return f"{ARXIV_HTML_BASE_URL}/{normalize_arxiv_ref(arxiv_id)}"


def arxiv_abs_url(arxiv_id: str) -> str:
    """Return the canonical arXiv abstract URL for an arXiv identifier."""
    return f"{ARXIV_ABS_BASE_URL}/{normalize_arxiv_ref(arxiv_id)}"


__all__ = [
    "ARXIV_ABS_BASE_URL",
    "ARXIV_HTML_BASE_URL",
    "ARXIV_PDF_BASE_URL",
    "HTMLDownloader",
    "PDFDownloader",
    "arxiv_abs_url",
    "arxiv_html_url",
    "arxiv_pdf_url",
    "normalize_arxiv_ref",
]
