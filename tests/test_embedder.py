import httpx
import pytest

from arxiv_archive.embedder import Embedder


async def _async_mock_client_factory(client):
    return client

@pytest.mark.asyncio
async def test_embedder_batching(monkeypatch):
    embedder = Embedder(batch_size=2, dimensions=512)

    post_calls = []

    class MockResponse:
        def __init__(self, json_data):
            self._json_data = json_data

        def raise_for_status(self):
            pass

        def json(self):
            return self._json_data

    class MockClient:
        async def post(self, url, json):
            post_calls.append(json)
            num_inputs = len(json["inputs"])
            return MockResponse([[0.1] * 512 for _ in range(num_inputs)])

        async def aclose(self): pass

    client = MockClient()
    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(client))

    texts = ["t1", "t2", "t3", "t4", "t5"]
    results = await embedder.embed_all(texts)

    assert len(post_calls) == 3
    assert len(post_calls[0]["inputs"]) == 2
    assert len(post_calls[2]["inputs"]) == 1

    for call in post_calls:
        assert call["dimensions"] == 512
        assert call["truncate"] is True

    assert len(results) == 5
    assert len(results[0]) == 512

@pytest.mark.asyncio
async def test_embedder_empty():
    embedder = Embedder()
    results = await embedder.embed_all([])
    assert results == []

@pytest.mark.asyncio
async def test_embedder_http_error(monkeypatch):
    embedder = Embedder()

    class MockClient:
        async def post(self, url, json):
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
    embedder = Embedder()

    class MockClient:
        async def post(self, url, json):
            class MockResp:
                def raise_for_status(self): pass
                def json(self): return {"not": "a list"}
            return MockResp()
        async def aclose(self): pass

    client = MockClient()
    monkeypatch.setattr(embedder, "_get_client", lambda: _async_mock_client_factory(client))

    with pytest.raises(ValueError):
        await embedder.embed_batch(["test"])
