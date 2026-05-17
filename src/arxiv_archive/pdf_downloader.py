from pathlib import Path

import httpx


class PDFDownloader:
    DEFAULT_CACHE_DIR = Path.home() / ".arxiv_cache"

    def __init__(self, cache_dir: Path | None = None) -> None:
        self.cache_dir = cache_dir if cache_dir is not None else self.DEFAULT_CACHE_DIR
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    def download(self, arxiv_id: str, pdf_url: str) -> Path:
        pdf_path = self.cache_dir / f"{arxiv_id}.pdf"
        if pdf_path.exists():
            return pdf_path

        client = httpx.Client(timeout=120.0, follow_redirects=True)
        try:
            response = client.get(pdf_url)
            response.raise_for_status()
            pdf_path.write_bytes(response.content)
        finally:
            client.close()

        return pdf_path
