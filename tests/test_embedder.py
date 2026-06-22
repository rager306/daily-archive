import httpx
import pytest

from research_graph.infrastructure.retrieval.embedder import Embedder


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
    embedder = Embedder(batch_size=2, dimensions=512)

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

        async def aclose(self): pass

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
    embedder = Embedder()
    results = await embedder.embed_all([])
    assert results == []


@pytest.mark.asyncio
async def test_embedder_http_error(monkeypatch):
    embedder = Embedder(max_attempts=1, circuit_failure_threshold=99, graceful_degradation_enabled=False)

    class MockClient:
        async def post(self, url, json, **kwargs):
            raise httpx.RequestError("Server down")

        async def aclose(self): pass

    client = MockClient()
    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(client))

    with pytest.raises(httpx.RequestError):
        await embedder.embed_batch(["test"])


@pytest.mark.asyncio
async def test_embedder_get_client():
    embedder = Embedder()
    client = await embedder._get_client()
    assert isinstance(client, httpx.AsyncClient)

    # second call should return the same client
    client2 = await embedder._get_client()
    assert client is client2

    await embedder.close()
    assert embedder._client is None


@pytest.mark.asyncio
async def test_embedder_malformed_response(monkeypatch):
    embedder = Embedder(max_attempts=1, circuit_failure_threshold=99, graceful_degradation_enabled=False)

    class MockClient:
        async def post(self, url, json, **kwargs):
            return MockResponse({"not": "a list"})

        async def aclose(self): pass

    client = MockClient()
    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(client))

    with pytest.raises((ValueError, Exception)):
        await embedder.embed_batch(["test"])
