from pathlib import Path

from arxiv_archive.pdf_downloader import PDFDownloader


def test_pdf_downloader_init() -> None:
    # Test with default cache_dir
    downloader = PDFDownloader()
    assert downloader.cache_dir == PDFDownloader.DEFAULT_CACHE_DIR

    # Test with custom cache_dir
    custom_dir = Path("/tmp/custom_cache")
    downloader2 = PDFDownloader(cache_dir=custom_dir)
    assert downloader2.cache_dir == custom_dir


def test_download_returns_path(tmp_path: Path) -> None:
    downloader = PDFDownloader(cache_dir=tmp_path)
    arxiv_id = "2310.00001"
    pdf_url = "https://arxiv.org/pdf/2310.00001.pdf"

    result_path = downloader.download(arxiv_id, pdf_url)

    assert isinstance(result_path, Path)
    assert result_path.exists()
    assert result_path.name == f"{arxiv_id}.pdf"
    assert result_path.stat().st_size > 0
