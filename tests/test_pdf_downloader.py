from pathlib import Path

from research_graph.infrastructure.corpus.ingestion import PDFDownloader, arxiv_pdf_url


def test_pdf_downloader_init() -> None:
    # Test with default cache_dir
    downloader = PDFDownloader()
    assert downloader.cache_dir == PDFDownloader.DEFAULT_CACHE_DIR

    # Test with custom cache_dir
    custom_dir = Path("/tmp/custom_cache")
    downloader2 = PDFDownloader(cache_dir=custom_dir)
    assert downloader2.cache_dir == custom_dir


def test_arxiv_pdf_url_accepts_versioned_ids() -> None:
    assert arxiv_pdf_url("2605.14259v1") == "https://arxiv.org/pdf/2605.14259v1"


def test_download_returns_path(tmp_path: Path) -> None:
    downloader = PDFDownloader(cache_dir=tmp_path)
    arxiv_id = "2310.00001"
    pdf_url = "https://arxiv.org/pdf/2310.00001.pdf"

    result_path = downloader.download(arxiv_id, pdf_url)

    assert isinstance(result_path, Path)
    assert result_path.exists()
    assert result_path.name == f"{arxiv_id}.pdf"
    assert result_path.stat().st_size > 0


def test_download_rejects_non_pdf_response(tmp_path: Path, monkeypatch) -> None:
    downloader = PDFDownloader(cache_dir=tmp_path)

    class MockResponse:
        content = b"<html>not found</html>"
        headers = {"content-type": "text/html"}

        def raise_for_status(self):
            return None

    class MockClient:
        def __init__(self, *args, **kwargs):
            pass

        def get(self, url):
            return MockResponse()

        def close(self):
            return None

    monkeypatch.setattr("httpx.Client", MockClient)

    try:
        downloader.download("2605.bad", "https://arxiv.org/pdf/2605.bad")
    except ValueError as exc:
        assert "did not return a PDF" in str(exc)
    else:
        raise AssertionError("expected non-PDF response to fail")
