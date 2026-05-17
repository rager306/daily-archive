import logging

import httpx

logger = logging.getLogger(__name__)

class Embedder:
    """Async HTTP client for generating text embeddings using local TEI container."""

    def __init__(self, endpoint: str = "http://localhost:30080/embed", dimensions: int = 512, batch_size: int = 32):
        """Initialize the embedder.

        Args:
            endpoint: URL to the Text Embeddings Inference server.
            dimensions: Matryoshka dimension truncation limit.
            batch_size: Max number of texts to send in one request.
        """
        self.endpoint = endpoint
        self.dimensions = dimensions
        self.batch_size = batch_size
        self._client: httpx.AsyncClient | None = None

    async def _get_client(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=30.0)
        return self._client

    async def close(self) -> None:
        if self._client is not None:
            await self._client.aclose()
            self._client = None

    async def embed_batch(self, texts: list[str]) -> list[list[float]]:
        """Generate embeddings for a batch of texts.

        Args:
            texts: List of strings (e.g., abstracts).

        Returns:
            List of float lists representing the embeddings.
        """
        if not texts:
            return []

        client = await self._get_client()
        payload = {
            "inputs": texts,
            "truncate": True,
            "dimensions": self.dimensions
        }

        try:
            response = await client.post(self.endpoint, json=payload)
            response.raise_for_status()

            # TEI server returns a list of float arrays
            data = response.json()
            if not isinstance(data, list):
                raise ValueError(f"Expected list response from TEI, got {type(data)}")

            return data

        except httpx.HTTPError as e:
            logger.error(f"HTTP error during embedding generation: {e}")
            raise
        except Exception as e:
            logger.error(f"Unexpected error parsing embedding response: {e}")
            raise

    async def embed_all(self, texts: list[str]) -> list[list[float]]:
        """Embed all texts by splitting them into batches."""
        all_embeddings = []
        for i in range(0, len(texts), self.batch_size):
            batch = texts[i:i + self.batch_size]
            embeddings = await self.embed_batch(batch)
            all_embeddings.extend(embeddings)
        return all_embeddings
