from pathlib import Path

import httpx

ARXIV_PDF_BASE_URL = "https://arxiv.org/pdf"


class PDFDownloader:
    DEFAULT_CACHE_DIR = Path.home() / ".arxiv_cache"

    def __init__(self, cache_dir: Path | None = None) -> None:
        self.cache_dir = cache_dir if cache_dir is not None else self.DEFAULT_CACHE_DIR
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def download(self, arxiv_id: str, pdf_url: str | None = None) -> Path:
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
            pdf_path.write_bytes(response.content)
        finally:
            client.close()

        return pdf_path


def arxiv_pdf_url(arxiv_id: str) -> str:
    """Return the canonical arXiv PDF URL for an arXiv identifier."""
    return f"{ARXIV_PDF_BASE_URL}/{arxiv_id}"
