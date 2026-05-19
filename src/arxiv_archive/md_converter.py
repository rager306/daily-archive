"""Markdown converter using arxiv2md.org REST API with Marker fallback.

Primary: arxiv2md.org REST API — fast (<1 sec), parses HTML (ar5iv/LaTeXML) for modern papers.
Fallback: Marker CLI on PDF (20-30 min on CPU) — only for papers before 2020 with no HTML.
"""

from __future__ import annotations

import asyncio
import re
import shutil
from dataclasses import dataclass
from pathlib import Path

import httpx

from arxiv_archive.full_text import assess_full_text_quality
from arxiv_archive.pdf_downloader import PDFDownloader, arxiv_pdf_url

ARXIV2MD_URL = "https://arxiv2md.org/api/markdown"
HTML_CUTOFF_YEAR = 2020
CACHE_DIR = Path.home() / ".arxiv_cache"
MARKER_TIMEOUT_SECONDS = 600  # 10 minutes


@dataclass
class ConversionResult:
    """Result of a markdown conversion attempt."""

    markdown: str | None
    method: str  # "arxiv2md", "marker", or "error"
    error: str | None


class MDConverter:
    """Converts arXiv papers to markdown using arxiv2md REST + PDF fallback."""

    def __init__(self, *, pdf_downloader: PDFDownloader | None = None) -> None:
        self._http_client: httpx.AsyncClient | None = None
        self._pdf_downloader = pdf_downloader or PDFDownloader(cache_dir=CACHE_DIR)

    async def _get_http_client(self) -> httpx.AsyncClient:
        if self._http_client is None:
            self._http_client = httpx.AsyncClient(timeout=60.0)
        return self._http_client

    async def close(self) -> None:
        if self._http_client is not None:
            await self._http_client.aclose()
            self._http_client = None

    async def convert(self, arxiv_id: str) -> ConversionResult:
        """Convert an arXiv paper to markdown.

        Args:
            arxiv_id: ArXiv paper ID (with or without "arxiv:" prefix).

        Returns:
            ConversionResult with markdown content and method used.
        """
        arxiv_id = self._normalize_id(arxiv_id)

        # Check cache first
        cached = self._get_cached(arxiv_id)
        if cached is not None:
            return cached

        # Try arxiv2md first (fast, works for modern papers with HTML)
        result = await self._try_arxiv2md(arxiv_id)

        if result.markdown is not None:
            self._cache_result(arxiv_id, result)
            return result

        # Fallback to Marker for pre-2020 papers or if arxiv2md failed
        if self._needs_marker_fallback(arxiv_id) or result.error:
            marker_result = await self._try_marker(arxiv_id)
            if marker_result.markdown is not None:
                self._cache_result(arxiv_id, marker_result)
                return marker_result

        # Return the arxiv2md result even if it failed (contains error info)
        return result

    async def _try_arxiv2md(self, arxiv_id: str) -> ConversionResult:
        """Call arxiv2md.org REST API to convert paper to markdown.

        Args:
            arxiv_id: ArXiv paper ID.

        Returns:
            ConversionResult with markdown or error.
        """
        client = await self._get_http_client()
        try:
            response = await client.get(ARXIV2MD_URL, params={"url": arxiv_id})
            if response.status_code == 200:
                quality = assess_full_text_quality(response.text)
                if quality.status != "ok":
                    return ConversionResult(
                        markdown=None,
                        method="arxiv2md",
                        error=(
                            f"arxiv2md returned low-quality markdown for {arxiv_id}: "
                            f"{quality.fallback_reason}"
                        ),
                    )
                return ConversionResult(
                    markdown=response.text,
                    method="arxiv2md",
                    error=None,
                )
            elif response.status_code == 404:
                return ConversionResult(
                    markdown=None,
                    method="arxiv2md",
                    error=f"Paper {arxiv_id} not found (404)",
                )
            else:
                return ConversionResult(
                    markdown=None,
                    method="arxiv2md",
                    error=f"arxiv2md API returned status {response.status_code}",
                )
        except httpx.TimeoutException:
            return ConversionResult(
                markdown=None,
                method="arxiv2md",
                error="arxiv2md API timeout",
            )
        except httpx.HTTPError as exc:
            return ConversionResult(
                markdown=None,
                method="arxiv2md",
                error=f"arxiv2md API error: {exc}",
            )

    async def _try_marker(self, arxiv_id: str) -> ConversionResult:
        """Run Marker CLI to convert PDF to markdown, downloading the PDF if needed.

        Args:
            arxiv_id: ArXiv paper ID.

        Returns:
            ConversionResult with markdown or error.
        """
        pdf_path = self._get_pdf_path(arxiv_id)
        if pdf_path is None or not pdf_path.exists():
            try:
                pdf_path = self._pdf_downloader.download(arxiv_id, arxiv_pdf_url(arxiv_id))
            except Exception as exc:
                return ConversionResult(
                    markdown=None,
                    method="pdf",
                    error=f"PDF download failed for {arxiv_id}: {exc}",
                )

        # Check marker is available
        marker_cmd = shutil.which("marker")
        if marker_cmd is None:
            return self._try_docling(arxiv_id, pdf_path)

        output_dir = CACHE_DIR / f"marker_{arxiv_id}"
        output_dir.mkdir(parents=True, exist_ok=True)

        proc = await asyncio.create_subprocess_exec(
            marker_cmd,
            str(pdf_path),
            str(output_dir),
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        try:
            stdout, stderr = await asyncio.wait_for(
                proc.communicate(), timeout=MARKER_TIMEOUT_SECONDS
            )
        except TimeoutError:
            proc.kill()
            await proc.wait()
            return ConversionResult(
                markdown=None,
                method="marker",
                error=f"Marker timed out after {MARKER_TIMEOUT_SECONDS}s",
            )

        if proc.returncode == 0:
            md_files = list(output_dir.glob("*.md"))
            if md_files:
                markdown = md_files[0].read_text(encoding="utf-8")
                return ConversionResult(
                    markdown=markdown,
                    method="marker",
                    error=None,
                )
            return ConversionResult(
                markdown=None,
                method="marker",
                error="Marker produced no markdown file",
            )
        else:
            stderr_text = stderr.decode() if stderr else ""
            return ConversionResult(
                markdown=None,
                method="marker",
                error=f"Marker failed with code {proc.returncode}: {stderr_text[:200]}",
            )

    def _try_docling(self, arxiv_id: str, pdf_path: Path) -> ConversionResult:
        """Convert a local PDF to Markdown with Docling when Marker is unavailable."""
        try:
            from docling.document_converter import DocumentConverter
        except ImportError:
            return ConversionResult(
                markdown=None,
                method="docling",
                error="Marker CLI not found in PATH and Docling is not installed",
            )

        try:
            converter = DocumentConverter()
            result = converter.convert(pdf_path)
            markdown = result.document.export_to_markdown()
            quality = assess_full_text_quality(markdown)
            if quality.status != "ok":
                return ConversionResult(
                    markdown=None,
                    method="docling",
                    error=(
                        f"Docling extracted low-quality markdown for {arxiv_id}: "
                        f"{quality.fallback_reason}"
                    ),
                )
            return ConversionResult(markdown=markdown, method="docling", error=None)
        except Exception as exc:
            return ConversionResult(
                markdown=None,
                method="docling",
                error=f"Docling PDF conversion failed for {arxiv_id}: {exc}",
            )

    def _needs_marker_fallback(self, arxiv_id: str) -> bool:
        """Determine if a paper needs Marker fallback.

        Papers before 2020 typically don't have HTML versions on ar5iv/LaTeXML,
        so they need Marker for PDF conversion.

        Args:
            arxiv_id: ArXiv paper ID in format YYMM.NNNNN or YYYYMM.NNNNN.

        Returns:
            True if paper year < 2020.
        """
        # Parse YYMM.NNNNN or YYYYMM.NNNNN format
        match = re.match(r"^(\d{2,4})(\d{2})\.(\d+)$", arxiv_id)
        if not match:
            return False  # Can't determine, don't fallback

        year_part = match.group(1)
        month = int(match.group(2))

        if len(year_part) == 2:
            year = 2000 + int(year_part)
        else:
            year = int(year_part)

        # Invalid month is a parsing error, don't fallback
        if month < 1 or month > 12:
            return False

        return year < HTML_CUTOFF_YEAR

    def _get_pdf_path(self, arxiv_id: str) -> Path | None:
        """Find the local PDF for an arXiv paper.

        Args:
            arxiv_id: ArXiv paper ID.

        Returns:
            Path to PDF if found, None otherwise.
        """
        # Common PDF locations
        candidates = [
            CACHE_DIR / f"{arxiv_id}.pdf",
            Path.home() / "arxiv" / f"{arxiv_id}.pdf",
            Path.home() / "Downloads" / f"{arxiv_id}.pdf",
        ]
        for candidate in candidates:
            if candidate.exists():
                return candidate
        return None

    def _normalize_id(self, arxiv_id: str) -> str:
        """Strip 'arxiv:' prefix if present.

        Args:
            arxiv_id: ArXiv ID with optional "arxiv:" prefix.

        Returns:
            Normalized arXiv ID without prefix.
        """
        arxiv_id = arxiv_id.strip()
        if arxiv_id.startswith("arxiv:"):
            return arxiv_id[6:].strip()
        return arxiv_id

    def _get_cached(self, arxiv_id: str) -> ConversionResult | None:
        """Read cached conversion result.

        Args:
            arxiv_id: ArXiv paper ID.

        Returns:
            Cached ConversionResult or None if not cached.
        """
        md_path = CACHE_DIR / f"{arxiv_id}.md"
        method_path = CACHE_DIR / f"{arxiv_id}.method"

        if not md_path.exists() or not method_path.exists():
            return None

        try:
            markdown = md_path.read_text(encoding="utf-8")
            method = method_path.read_text(encoding="utf-8").strip()
            if method == "pymupdf":
                return None
            if assess_full_text_quality(markdown).status != "ok":
                return None
            return ConversionResult(markdown=markdown, method=method, error=None)
        except Exception:
            return None

    def _cache_result(self, arxiv_id: str, result: ConversionResult) -> None:
        """Write conversion result to cache.

        Args:
            arxiv_id: ArXiv paper ID.
            result: ConversionResult to cache.
        """
        if result.markdown is None:
            return

        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        md_path = CACHE_DIR / f"{arxiv_id}.md"
        method_path = CACHE_DIR / f"{arxiv_id}.method"

        md_path.write_text(result.markdown, encoding="utf-8")
        method_path.write_text(result.method, encoding="utf-8")

    async def _convert_and_close(self, arxiv_id: str) -> ConversionResult:
        try:
            return await self.convert(arxiv_id)
        finally:
            await self.close()

    # Sync wrapper for backwards compatibility
    def convert_sync(self, arxiv_id: str) -> ConversionResult:
        """Synchronous wrapper for convert()."""
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            # No running loop — create one and close the async HTTP client bound to it.
            return asyncio.run(self._convert_and_close(arxiv_id))
        else:
            # Already in async context — schedule and block
            future = asyncio.ensure_future(self.convert(arxiv_id))
            return loop.run_until_complete(future)
