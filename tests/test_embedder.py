import httpx
import pytest

from arxiv_archive.embedder import DEFAULT_EMBEDDING_ENDPOINT, Embedder


async def async_client_factory(client):
    return client


class FakeAsyncClient:
    def __init__(self, responses):
        self.responses = responses
        self.calls = []

    async def post(self, url, json):
        self.calls.append({"url": url, "json": json})
        return self.responses[len(self.calls) - 1]

    async def aclose(self):
        return None


def response(status_code, payload):
    return httpx.Response(
        status_code,
        json=payload,
        request=httpx.Request("POST", DEFAULT_EMBEDDING_ENDPOINT),
    )


def success_payload(count, dimensions):
    return {
        "object": "list",
        "data": [
            {"object": "embedding", "index": index, "embedding": [0.1] * dimensions}
            for index in range(count)
        ],
    }


@pytest.mark.asyncio
async def test_embedder_batching(monkeypatch):
    embedder = Embedder(batch_size=2, dimensions=512, retry_sleep=False)
    fake_client = FakeAsyncClient(
        [
            response(200, success_payload(2, dimensions=512)),
            response(200, success_payload(2, dimensions=512)),
            response(200, success_payload(1, dimensions=512)),
        ]
    )
    monkeypatch.setattr(embedder, "_get_client", lambda: async_client_factory(fake_client))

    texts = ["t1", "t2", "t3", "t4", "t5"]
    results = await embedder.embed_all(texts)

    assert len(fake_client.calls) == 3
    assert [call["json"]["input"] for call in fake_client.calls] == [
        ["t1", "t2"],
        ["t3", "t4"],
        ["t5"],
    ]
    assert all(call["json"]["dimensions"] == 512 for call in fake_client.calls)
    assert len(results) == 5
    assert all(len(embedding) == 512 for embedding in results)
