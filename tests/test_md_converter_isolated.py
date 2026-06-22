import asyncio

import httpx
import pytest

from research_graph.infrastructure.corpus.sources.markdown_converter import (
    ConversionResult,
    MDConverter,
)


@pytest.fixture
def temp_cache(tmp_path, monkeypatch):
    monkeypatch.setattr(
        "research_graph.infrastructure.corpus.sources.markdown_converter.CACHE_DIR",
        tmp_path / ".arxiv_cache",
    )
    return tmp_path / ".arxiv_cache"


@pytest.fixture
def home_dir(tmp_path, monkeypatch):
    monkeypatch.setattr("pathlib.Path.home", lambda: tmp_path)
    return tmp_path


@pytest.mark.asyncio
async def test_cache_hit(temp_cache):
    converter = MDConverter()

    # Create fake cache
    temp_cache.mkdir(parents=True, exist_ok=True)
    (temp_cache / "2101.12345.md").write_text("# Cached content\n\nCached paper body")
    (temp_cache / "2101.12345.method").write_text("arxiv2md")

    result = await converter.convert("2101.12345")
    assert result.markdown == "# Cached content\n\nCached paper body"
    assert result.method == "arxiv2md"


@pytest.mark.asyncio
async def test_arxiv2md_success(temp_cache, monkeypatch):
    converter = MDConverter()

    class MockResponse:
        status_code = 200
        text = "# From API\n\nConverted paper body"

    class MockClient:
        async def get(self, url, params):
            assert url == "https://arxiv2md.org/api/markdown"
            assert params["url"] == "2101.12345"
            return MockResponse()

        async def aclose(self):
            pass

    async def get_client():
        return MockClient()

    monkeypatch.setattr(converter, "_get_http_client", get_client)

    result = await converter.convert("2101.12345")
    assert result.markdown == "# From API\n\nConverted paper body"

    assert result.method == "arxiv2md"

    # Check it was cached
    assert (temp_cache / "2101.12345.md").read_text() == "# From API\n\nConverted paper body"


@pytest.mark.asyncio
async def test_arxiv2md_404(temp_cache, monkeypatch):
    converter = MDConverter()

    class MockResponse:
        status_code = 404

    class MockClient:
        async def get(self, url, params):
            return MockResponse()

        async def aclose(self):
            pass

    async def get_client():
        return MockClient()

    monkeypatch.setattr(converter, "_get_http_client", get_client)
    monkeypatch.setattr(converter, "_needs_marker_fallback", lambda x: False)

    class FailingDownloader:
        def download(self, arxiv_id, pdf_url):
            raise RuntimeError("offline")

    converter._pdf_downloader = FailingDownloader()  # pyrefly: ignore[bad-assignment]

    result = await converter.convert("2101.12345")
    assert result.markdown is None
    assert result.error is not None
    assert "not found" in result.error


@pytest.mark.asyncio
async def test_arxiv2md_timeout(temp_cache, monkeypatch):
    converter = MDConverter()

    class MockClient:
        async def get(self, url, params):
            raise httpx.TimeoutException("Timeout")

        async def aclose(self):
            pass

    async def get_client():
        return MockClient()

    monkeypatch.setattr(converter, "_get_http_client", get_client)
    monkeypatch.setattr(converter, "_needs_marker_fallback", lambda x: False)

    class FailingDownloader:
        def download(self, arxiv_id, pdf_url):
            raise RuntimeError("offline")

    converter._pdf_downloader = FailingDownloader()  # pyrefly: ignore[bad-assignment]

    result = await converter.convert("2101.12345")
    assert result.markdown is None
    assert result.error is not None
    assert "timeout" in result.error


@pytest.mark.asyncio
async def test_arxiv2md_fails_and_fallback_to_marker(temp_cache, home_dir, monkeypatch):
    converter = MDConverter()

    class MockClient:
        async def get(self, url, params):
            class MockResp:
                status_code = 500

            return MockResp()

        async def aclose(self):
            pass

    async def get_client():
        return MockClient()

    monkeypatch.setattr(converter, "_get_http_client", get_client)

    # Mock PDF path so _try_marker proceeds
    pdf_dir = temp_cache
    pdf_dir.mkdir(parents=True, exist_ok=True)
    pdf_path = pdf_dir / "2101.12345.pdf"
    pdf_path.write_bytes(b"%PDF-1.4")

    # Needs fallback because of 500 error, mock subprocess
    class MockProcess:
        returncode = 0

        async def communicate(self):
            # Create the output md file
            out_dir = temp_cache / "marker_2101.12345"
            out_dir.mkdir(parents=True, exist_ok=True)
            (out_dir / "output.md").write_text("# From Marker")
            return b"stdout", b"stderr"

        async def wait(self):
            pass

        def kill(self):
            pass

    async def mock_exec(*args, **kwargs):
        return MockProcess()

    monkeypatch.setattr(asyncio, "create_subprocess_exec", mock_exec)
    monkeypatch.setattr("shutil.which", lambda x: "/usr/bin/marker")

    result = await converter.convert("2101.12345")
    assert result.markdown == "# From Marker"
    assert result.method == "marker"


@pytest.mark.asyncio
async def test_arxiv2md_low_quality_markdown_falls_back_to_marker(
    temp_cache, home_dir, monkeypatch
):
    converter = MDConverter()

    class MockClient:
        async def get(self, url, params):
            class MockResp:
                status_code = 200
                text = "# Title: navigation shell\n\n## Submission history\n\n## Access Paper:"

            return MockResp()

        async def aclose(self):
            pass

    async def get_client():
        return MockClient()

    monkeypatch.setattr(converter, "_get_http_client", get_client)

    pdf_path = temp_cache / "2605.14259v1.pdf"
    temp_cache.mkdir(parents=True, exist_ok=True)
    pdf_path.write_bytes(b"%PDF-1.4")

    class MockProcess:
        returncode = 0

        async def communicate(self):
            out_dir = temp_cache / "marker_2605.14259v1"
            out_dir.mkdir(parents=True, exist_ok=True)
            (out_dir / "output.md").write_text("# From Marker\n\nRecovered paper body")
            return b"stdout", b"stderr"

        async def wait(self):
            pass

        def kill(self):
            pass

    async def mock_exec(*args, **kwargs):
        return MockProcess()

    monkeypatch.setattr(asyncio, "create_subprocess_exec", mock_exec)
    monkeypatch.setattr("shutil.which", lambda x: "/usr/bin/marker")

    result = await converter.convert("2605.14259v1")

    assert "Recovered paper body" in result.markdown
    assert result.method in {"marker", "docling"}
    assert not (temp_cache / "2605.14259v1.md").read_text().startswith("# Title: navigation")


def test_low_quality_cached_markdown_is_ignored(temp_cache):
    converter = MDConverter()
    temp_cache.mkdir(parents=True, exist_ok=True)
    (temp_cache / "2605.14517v1.md").write_text(
        "# Computer Science > Computation and Language\n\n## Submission history\n\n## Access Paper:",
        encoding="utf-8",
    )
    (temp_cache / "2605.14517v1.method").write_text("arxiv2md", encoding="utf-8")

    assert converter._get_cached("2605.14517v1") is None


def test_deprecated_pymupdf_cached_markdown_is_ignored(temp_cache):
    converter = MDConverter()
    temp_cache.mkdir(parents=True, exist_ok=True)
    (temp_cache / "2605.14259v1.md").write_text(
        "# Converted from PDF\n\nRecovered body text", encoding="utf-8"
    )
    (temp_cache / "2605.14259v1.method").write_text("pymupdf", encoding="utf-8")

    assert converter._get_cached("2605.14259v1") is None


@pytest.mark.asyncio
async def test_marker_missing_pdf_reports_download_failure(temp_cache, monkeypatch):
    converter = MDConverter()

    class FailingDownloader:
        def download(self, arxiv_id, pdf_url):
            raise RuntimeError("network unavailable")

    converter._pdf_downloader = FailingDownloader()  # pyrefly: ignore[bad-assignment]
    monkeypatch.setattr(converter, "_get_pdf_path", lambda x: None)

    result = await converter._try_marker("2101.12345")
    assert result.markdown is None
    assert result.error is not None
    assert "PDF download failed" in result.error


@pytest.mark.asyncio
async def test_marker_downloads_pdf_when_missing_and_marker_unavailable(temp_cache, monkeypatch):
    converter = MDConverter()
    downloaded_pdf = temp_cache / "2101.12345.pdf"

    class FakeDownloader:
        def download(self, arxiv_id, pdf_url):
            downloaded_pdf.write_bytes(b"%PDF")
            return downloaded_pdf

    converter._pdf_downloader = FakeDownloader()  # pyrefly: ignore[bad-assignment]
    monkeypatch.setattr(converter, "_get_pdf_path", lambda x: None)
    monkeypatch.setattr("shutil.which", lambda x: None)
    monkeypatch.setattr(
        converter,
        "_try_docling",
        lambda arxiv_id, pdf_path: ConversionResult(
            markdown="# From PDF\n\nRecovered body", method="docling", error=None
        ),
    )

    result = await converter._try_marker("2101.12345")

    assert result.markdown == "# From PDF\n\nRecovered body"
    assert result.method == "docling"


@pytest.mark.asyncio
async def test_marker_cli_not_found_falls_back_to_docling(temp_cache, monkeypatch):
    converter = MDConverter()

    # create fake pdf
    temp_cache.mkdir(parents=True, exist_ok=True)
    pdf_path = temp_cache / "2101.12345.pdf"
    pdf_path.write_bytes(b"%PDF")

    monkeypatch.setattr(converter, "_get_pdf_path", lambda x: pdf_path)
    monkeypatch.setattr("shutil.which", lambda x: None)
    monkeypatch.setattr(
        converter,
        "_try_docling",
        lambda arxiv_id, pdf_path: ConversionResult(
            markdown="# From Docling\n\nRecovered body", method="docling", error=None
        ),
    )

    result = await converter._try_marker("2101.12345")
    assert result.markdown == "# From Docling\n\nRecovered body"
    assert result.method == "docling"


def test_convert_sync(monkeypatch):
    converter = MDConverter()

    async def mock_convert(arxiv_id):
        return ConversionResult(markdown="sync test", method="arxiv2md", error=None)

    monkeypatch.setattr(converter, "convert", mock_convert)

    result = converter.convert_sync("2101.12345")
    assert result.markdown == "sync test"


@pytest.mark.asyncio
async def test_get_http_client():
    converter = MDConverter()
    client = await converter._get_http_client()
    assert isinstance(client, httpx.AsyncClient)
    assert converter._http_client is client

    # Second call returns same client
    client2 = await converter._get_http_client()
    assert client is client2

    await converter.close()
    assert converter._http_client is None

    # Close again is safe
    await converter.close()


@pytest.mark.asyncio
async def test_arxiv2md_httperror(temp_cache, monkeypatch):
    converter = MDConverter()

    class MockClient:
        async def get(self, url, params):
            raise httpx.RequestError("Request failed")

        async def aclose(self):
            pass

    async def get_client():
        return MockClient()

    monkeypatch.setattr(converter, "_get_http_client", get_client)
    monkeypatch.setattr(converter, "_needs_marker_fallback", lambda x: False)

    class FailingDownloader:
        def download(self, arxiv_id, pdf_url):
            raise RuntimeError("offline")

    converter._pdf_downloader = FailingDownloader()  # pyrefly: ignore[bad-assignment]

    result = await converter.convert("2101.12345")
    assert result.markdown is None
    assert result.error is not None
    assert "arxiv2md API error: Request failed" in result.error


@pytest.mark.asyncio
async def test_marker_timeout(temp_cache, monkeypatch):
    converter = MDConverter()

    # create fake pdf
    temp_cache.mkdir(parents=True, exist_ok=True)
    (temp_cache / "2101.12345.pdf").write_bytes(b"%PDF")
    monkeypatch.setattr(converter, "_get_pdf_path", lambda x: temp_cache / "2101.12345.pdf")
    monkeypatch.setattr("shutil.which", lambda x: "/usr/bin/marker")

    class MockProcess:
        def kill(self):
            pass

        async def wait(self):
            pass

        async def communicate(self):
            raise TimeoutError()

    async def mock_exec(*args, **kwargs):
        return MockProcess()

    monkeypatch.setattr(asyncio, "create_subprocess_exec", mock_exec)

    result = await converter._try_marker("2101.12345")
    assert result.markdown is None
    assert result.error is not None
    assert "Marker timed out" in result.error


@pytest.mark.asyncio
async def test_marker_failed_code(temp_cache, monkeypatch):
    converter = MDConverter()

    # create fake pdf
    temp_cache.mkdir(parents=True, exist_ok=True)
    (temp_cache / "2101.12345.pdf").write_bytes(b"%PDF")
    monkeypatch.setattr(converter, "_get_pdf_path", lambda x: temp_cache / "2101.12345.pdf")
    monkeypatch.setattr("shutil.which", lambda x: "/usr/bin/marker")

    class MockProcess:
        returncode = 1

        async def communicate(self):
            return b"", b"Some error"

    async def mock_exec(*args, **kwargs):
        return MockProcess()

    monkeypatch.setattr(asyncio, "create_subprocess_exec", mock_exec)

    result = await converter._try_marker("2101.12345")
    assert result.markdown is None
    assert result.error is not None
    assert "Marker failed with code 1" in result.error
    assert result.error is not None
    assert "Some error" in result.error


def test_cache_read_exception(temp_cache, monkeypatch):
    converter = MDConverter()
    temp_cache.mkdir(parents=True, exist_ok=True)
    md_path = temp_cache / "2101.12345.md"
    method_path = temp_cache / "2101.12345.method"

    md_path.write_text("# Cached\n\nCached body")
    method_path.write_text("arxiv2md")

    # Mock read_text to throw
    def mock_read(self, *args, **kwargs):
        raise PermissionError("Access denied")

    import pathlib

    monkeypatch.setattr(pathlib.Path, "read_text", mock_read)

    # Should silently return None
    assert converter._get_cached("2101.12345") is None


def test_cache_none_result(temp_cache):
    converter = MDConverter()
    result = ConversionResult(markdown=None, method="error", error="Fail")
    converter._cache_result("2101.12345", result)

    # Nothing written
    assert not (temp_cache / "2101.12345.md").exists()


def test_convert_sync_in_async_loop(monkeypatch):
    converter = MDConverter()

    class FakeFuture:
        def result(self):
            return ConversionResult(markdown="sync in async", method="arxiv2md", error=None)

    future = FakeFuture()

    import asyncio

    class FakeLoop:
        def run_until_complete(self, f):
            return f.result()

    monkeypatch.setattr(asyncio, "get_running_loop", lambda: FakeLoop())
    monkeypatch.setattr(asyncio, "ensure_future", lambda f: future)
    monkeypatch.setattr(converter, "convert", lambda arxiv_id: future)

    result = converter.convert_sync("2101.12345")
    assert result.markdown == "sync in async"


@pytest.mark.asyncio
async def test_marker_no_markdown_output(temp_cache, monkeypatch):
    converter = MDConverter()

    # create fake pdf
    temp_cache.mkdir(parents=True, exist_ok=True)
    (temp_cache / "2101.12345.pdf").write_bytes(b"%PDF")
    monkeypatch.setattr(converter, "_get_pdf_path", lambda x: temp_cache / "2101.12345.pdf")
    monkeypatch.setattr("shutil.which", lambda x: "/usr/bin/marker")

    class MockProcess:
        returncode = 0

        async def communicate(self):
            return b"stdout", b"stderr"

    async def mock_exec(*args, **kwargs):
        return MockProcess()

    monkeypatch.setattr(asyncio, "create_subprocess_exec", mock_exec)

    result = await converter._try_marker("2101.12345")
    assert result.markdown is None
    assert result.error is not None
    assert "Marker produced no markdown file" in result.error
