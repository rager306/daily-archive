#!/usr/bin/env python3
# DEPRECATED: legacy M057 helper retained for historical reproduction only.
# Use research_graph.infrastructure.retrieval.embedder.Embedder for all new fd embedding calls.
"""Embed M057 S02 table text through the local fd embeddings API."""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BASE_URL = os.environ.get(
    "TEI_URL", os.environ.get("FD_EMBEDDINGS_ENDPOINT_BASE", "http://127.0.0.1:8000")
)
DEFAULT_API_KEY = os.environ.get("FD_API_KEY")
DEFAULT_MODEL_ID = os.environ.get("MODEL_ID", os.environ.get("FD_MODEL_NAME", "deepvk/USER-bge-m3"))
DEFAULT_REDIS_HOST = os.environ.get("REDIS_HOST", "127.0.0.1")
DEFAULT_REDIS_PORT = os.environ.get("REDIS_PORT", "6379")
DEFAULT_CORPUS = (
    ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "table-text-corpus.json"
)
DEFAULT_OUTPUT = ROOT / "artifacts" / "m057-fd-marker" / "table-similarity" / "embeddings.json"
DEFAULT_BATCH_SIZE = 32
DEFAULT_DIMENSIONS = 1024

SAFETY_DEFAULTS: dict[str, bool] = {
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "external_network_authorized": False,
    "llm_calls_authorized": False,
}


class FdEmbeddingError(RuntimeError):
    """Raised when fd returns an unusable embedding response."""


class FdEmbeddingClient:
    def __init__(
        self,
        base_url: str = DEFAULT_BASE_URL,
        timeout_seconds: float = 120.0,
        api_key: str | None = DEFAULT_API_KEY,
        model_id: str = DEFAULT_MODEL_ID,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout_seconds = timeout_seconds
        self.api_key = api_key
        self.model_id = model_id

    def embed_batch(
        self, texts: list[str], *, dimensions: int = DEFAULT_DIMENSIONS
    ) -> list[list[float]]:
        payload = {"input": texts, "model": self.model_id, "dimensions": dimensions}
        headers = {"Content-Type": "application/json", "X-Model-Id": self.model_id}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        request = urllib.request.Request(
            f"{self.base_url}/v1/embeddings",
            data=json.dumps(payload).encode("utf-8"),
            headers=headers,
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=self.timeout_seconds) as response:
                body = response.read().decode("utf-8")
        except urllib.error.HTTPError as exc:
            error_body = exc.read().decode("utf-8", errors="replace")[:500]
            raise FdEmbeddingError(
                f"fd embedding request failed with HTTP {exc.code}: {error_body}"
            ) from exc
        except urllib.error.URLError as exc:
            raise FdEmbeddingError(f"fd embedding request failed: {exc}") from exc
        try:
            data = json.loads(body)
        except json.JSONDecodeError as exc:
            raise FdEmbeddingError(f"fd returned non-JSON response: {body[:200]}") from exc
        rows = data.get("data")
        if not isinstance(rows, list):
            raise FdEmbeddingError("fd response is missing data[]")
        ordered_rows = sorted(rows, key=lambda item: int(item.get("index", 0)))
        embeddings: list[list[float]] = []
        for row in ordered_rows:
            embedding = row.get("embedding")  # ty:ignore[unresolved-attribute]
            if not isinstance(embedding, list) or len(embedding) != dimensions:
                raise FdEmbeddingError(
                    "fd response contained an embedding with the wrong dimension"
                )
            embeddings.append([float(value) for value in embedding])
        if len(embeddings) != len(texts):
            raise FdEmbeddingError("fd response length did not match request length")
        return embeddings


def load_tables(corpus_path: Path) -> list[dict[str, Any]]:
    payload = json.loads(corpus_path.read_text(encoding="utf-8"))
    tables = payload.get("tables") if isinstance(payload, dict) else payload
    if not isinstance(tables, list):
        raise ValueError("table corpus must contain a tables[] list")
    return tables


def batched(values: list[dict[str, Any]], size: int) -> list[list[dict[str, Any]]]:
    return [values[index : index + size] for index in range(0, len(values), size)]


def embed_batch_with_split(
    client: FdEmbeddingClient,
    texts: list[str],
    *,
    dimensions: int = DEFAULT_DIMENSIONS,
) -> list[list[float]]:
    """Embed a batch, recursively splitting if fd rejects the combined request."""

    try:
        return client.embed_batch(texts, dimensions=dimensions)
    except FdEmbeddingError:
        if len(texts) == 1:
            raise
        midpoint = len(texts) // 2
        left = embed_batch_with_split(client, texts[:midpoint], dimensions=dimensions)
        right = embed_batch_with_split(client, texts[midpoint:], dimensions=dimensions)
        return left + right


def embed_corpus(
    *,
    corpus_path: Path = DEFAULT_CORPUS,
    output_path: Path = DEFAULT_OUTPUT,
    base_url: str = DEFAULT_BASE_URL,
    batch_size: int = DEFAULT_BATCH_SIZE,
    dimensions: int = DEFAULT_DIMENSIONS,
) -> dict[str, Any]:
    tables = load_tables(corpus_path)
    client = FdEmbeddingClient(base_url=base_url)
    embeddings: dict[str, list[float]] = {}
    split_fallback_batches = 0
    started = time.perf_counter()
    for batch in batched(tables, batch_size):
        text_batch = [str(table["text_repr"]) for table in batch]
        try:
            vector_batch = client.embed_batch(text_batch, dimensions=dimensions)
        except FdEmbeddingError:
            split_fallback_batches += 1
            vector_batch = embed_batch_with_split(client, text_batch, dimensions=dimensions)
        for table, vector in zip(batch, vector_batch, strict=True):
            embeddings[str(table["table_id"])] = vector
    elapsed_ms = round((time.perf_counter() - started) * 1000, 3)
    payload = {
        "schema_version": "m057.table-embeddings.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "base_url": base_url,
        "dimensions": dimensions,
        "batch_size": batch_size,
        "table_count": len(tables),
        "embedding_count": len(embeddings),
        "elapsed_ms": elapsed_ms,
        "split_fallback_batches": split_fallback_batches,
        "embeddings": dict(sorted(embeddings.items())),
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--batch-size", type=int, default=DEFAULT_BATCH_SIZE)
    parser.add_argument("--dimensions", type=int, default=DEFAULT_DIMENSIONS)
    args = parser.parse_args()
    payload = embed_corpus(
        corpus_path=args.corpus,
        output_path=args.output,
        base_url=args.base_url,
        batch_size=args.batch_size,
        dimensions=args.dimensions,
    )
    sys.stdout.write(
        json.dumps(
            {
                "output": str(args.output),
                "embedding_count": payload["embedding_count"],
                "elapsed_ms": payload["elapsed_ms"],
            },
            sort_keys=True,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
