import httpx
import pytest

from research_graph.infrastructure.retrieval.embedder import (
    DegradedEmbeddingSignal,
    Embedder,
    FdAuthError,
    FdDegradedEmbeddingsError,
    is_zero_embedding_batch,
    is_zero_vector,
    validate_fd_api_key,
)


async def _async_mock_client_factory(client):
    return client


class MockResponse:
    """Mock httpx.Response that matches the real embedder's expectations."""

    def __init__(self, json_data, status_code: int = 200):
        self._json_data = json_data
        self.status_code = status_code
        self.headers = {}

    def raise_for_status(self):
        if self.status_code >= 400:
            raise httpx.HTTPStatusError(
                f"HTTP {self.status_code}",
                request=httpx.Request("POST", "http://test"),
                response=httpx.Response(self.status_code),
            )

    def json(self):
        return self._json_data


@pytest.mark.asyncio
async def test_embedder_batching(monkeypatch):
    embedder = Embedder(batch_size=2, dimensions=512, require_api_key=False)

    post_calls = []

    class MockClient:
        async def post(self, url, json, **kwargs):
            post_calls.append(json)
            num_inputs = len(json["input"])
            data = [
                {"object": "embedding", "embedding": [0.1] * 512, "index": i}
                for i in range(num_inputs)
            ]
            return MockResponse({"object": "list", "data": data, "model": "test"})

        async def aclose(self):
            pass

    client = MockClient()
    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(client))

    texts = ["t1", "t2", "t3", "t4", "t5"]
    results = await embedder.embed_all(texts)

    assert len(post_calls) == 3
    assert len(post_calls[0]["input"]) == 2
    assert len(post_calls[2]["input"]) == 1

    for call in post_calls:
        assert call["dimensions"] == 512

    assert len(results) == 5
    assert len(results[0]) == 512


@pytest.mark.asyncio
async def test_embedder_empty():
    embedder = Embedder(require_api_key=False)
    results = await embedder.embed_all([])
    assert results == []


@pytest.mark.asyncio
async def test_embedder_sync_wrappers_fail_inside_running_loop():
    embedder = Embedder(require_api_key=False)

    with pytest.raises(RuntimeError, match="await embed_batch"):
        embedder.embed_batch_sync(["text"])
    with pytest.raises(RuntimeError, match="await embed_all"):
        embedder.embed_all_sync(["text"])


@pytest.mark.asyncio
async def test_embedder_http_error(monkeypatch):
    embedder = Embedder(
        max_attempts=1, circuit_failure_threshold=99, graceful_degradation_enabled=False,
        require_api_key=False,
    )

    class MockClient:
        async def post(self, url, json, **kwargs):
            raise httpx.RequestError("Server down")

        async def aclose(self):
            pass

    client = MockClient()
    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(client))

    with pytest.raises(httpx.RequestError):
        await embedder.embed_batch(["test"])


@pytest.mark.asyncio
async def test_embedder_get_client():
    embedder = Embedder(require_api_key=False)
    client = await embedder._get_client()
    assert isinstance(client, httpx.AsyncClient)

    # second call should return the same client
    client2 = await embedder._get_client()
    assert client is client2

    await embedder.close()
    assert embedder._client is None


@pytest.mark.asyncio
async def test_embedder_close_does_not_close_injected_client():
    class InjectedClient:
        def __init__(self):
            self.closed = False

        async def aclose(self):
            self.closed = True

    client = InjectedClient()
    embedder = Embedder(client=client, require_api_key=False)  # type: ignore[arg-type]

    await embedder.close()

    assert client.closed is False
    assert embedder._client is None


@pytest.mark.asyncio
async def test_embedder_malformed_response(monkeypatch):
    embedder = Embedder(
        max_attempts=1, circuit_failure_threshold=99, graceful_degradation_enabled=False,
        require_api_key=False,
    )

    class MockClient:
        async def post(self, url, json, **kwargs):
            return MockResponse({"not": "a list"})

        async def aclose(self):
            pass

    client = MockClient()
    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(client))

    with pytest.raises((ValueError, Exception)):
        await embedder.embed_batch(["test"])


def test_is_zero_vector_and_batch_predicates():
    assert is_zero_vector([0.0, 0.0, 0.0])
    assert not is_zero_vector([0.0, 0.1])
    assert is_zero_embedding_batch([[0.0, 0.0], [0.0, 0.0]])
    assert not is_zero_embedding_batch([[0.0, 0.0], [0.1, 0.0]])
    assert not is_zero_embedding_batch([])


def test_validate_fd_api_key_missing_and_placeholder():
    with pytest.raises(FdAuthError) as ei:
        validate_fd_api_key(None)
    assert ei.value.code == "FD_AUTH_MISSING"
    assert "FD_API_KEY" in ei.value.diagnostic
    assert "sk-" not in ei.value.diagnostic

    with pytest.raises(FdAuthError) as ei:
        validate_fd_api_key("   ")
    assert ei.value.code == "FD_AUTH_MISSING"

    with pytest.raises(FdAuthError) as ei:
        validate_fd_api_key("short")
    assert ei.value.code == "FD_AUTH_INVALID"

    with pytest.raises(FdAuthError) as ei:
        validate_fd_api_key("<your-fd-api-key-here>")
    assert ei.value.code == "FD_AUTH_INVALID"

    ok = validate_fd_api_key("V" * 43)
    assert ok == "V" * 43


@pytest.mark.asyncio
async def test_preflight_blocks_embed_without_key(monkeypatch):
    embedder = Embedder(api_key=None, require_api_key=True, max_attempts=1)

    class MockClient:
        async def post(self, url, json, **kwargs):
            raise AssertionError("HTTP must not be called when preflight fails")

        async def aclose(self):
            pass

    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(MockClient()))
    with pytest.raises(FdAuthError) as ei:
        await embedder.embed_batch(["hello"])
    assert ei.value.code == "FD_AUTH_MISSING"


@pytest.mark.asyncio
async def test_graceful_degradation_sets_last_degraded(monkeypatch):
    embedder = Embedder(
        max_attempts=1,
        circuit_failure_threshold=1,
        graceful_degradation_enabled=True,
        require_api_key=False,
    )

    class MockClient:
        async def post(self, url, json, **kwargs):
            raise httpx.RequestError("Server down")

        async def aclose(self):
            pass

    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(MockClient()))
    # First failure opens circuit when threshold=1
    results = await embedder.embed_batch(["t1", "t2"])
    assert len(results) == 2
    assert is_zero_embedding_batch(results)
    assert embedder.last_degraded is not None
    assert isinstance(embedder.last_degraded, DegradedEmbeddingSignal)
    assert embedder.last_degraded.code == "FD_DEGRADED_ZERO_VECTORS"
    assert "FD_DEGRADED" in embedder.last_degraded.diagnostic


@pytest.mark.asyncio
async def test_response_all_zero_marks_degraded(monkeypatch):
    embedder = Embedder(dimensions=4, require_api_key=False, max_attempts=1)

    class MockClient:
        async def post(self, url, json, **kwargs):
            data = [
                {"object": "embedding", "embedding": [0.0] * 4, "index": i}
                for i in range(len(json["input"]))
            ]
            return MockResponse({"object": "list", "data": data, "model": "test"})

        async def aclose(self):
            pass

    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(MockClient()))
    results = await embedder.embed_batch(["a"])
    assert is_zero_embedding_batch(results)
    assert embedder.last_degraded is not None
    assert embedder.last_degraded.reason == "response_all_zero"


@pytest.mark.asyncio
async def test_cli_refuses_degraded_embeddings(monkeypatch, tmp_path):
    from datetime import date

    from research_graph.cli import run_analysis_async
    from research_graph.infrastructure.corpus.sources.arxiv_client import ArxivClient, ArxivPaper

    paper = ArxivPaper(
        id="2501.1",
        title="t",
        abstract="abstract text for embedding",
        authors=["a"],
        published=date(2026, 5, 14),
        updated=date(2026, 5, 14),
        categories=["cs.AI"],
        pdf_url="https://arxiv.org/pdf/2501.1.pdf",
    )

    def fake_fetch(self, start_date, end_date=None, categories=None):
        return [paper]

    monkeypatch.setattr(ArxivClient, "fetch_papers", fake_fetch)
    monkeypatch.setattr("research_graph.cli.QUEUE_DIR", tmp_path, raising=False)

    class DegradedEmbedder:
        def __init__(self, *a, **k):
            self.last_degraded = DegradedEmbeddingSignal(
                reason="circuit_open", batch_size=1, dimensions=4
            )

        async def embed_all(self, texts):
            return [[0.0] * 4 for _ in texts]

        async def close(self):
            return None

    monkeypatch.setattr("research_graph.cli.Embedder", DegradedEmbedder)
    # scoring path may still run — ensure KeywordExtractor/ScoringEngine work as real

    with pytest.raises(FdDegradedEmbeddingsError) as ei:
        await run_analysis_async(date(2026, 5, 14))
    assert "FD_DEGRADED_ZERO_VECTORS" in ei.value.diagnostic
