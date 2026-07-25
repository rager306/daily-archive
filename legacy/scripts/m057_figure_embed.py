#!/usr/bin/env python3
"""Embed M057 S03 figure captions through the local fd embeddings API."""

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
    ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "figure-caption-corpus.json"
)
DEFAULT_OUTPUT = ROOT / "artifacts" / "m057-fd-marker" / "figure-links" / "embeddings.json"
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
        ordered_rows = sorted(rows, key=lambda row: row.get("index", 0))
        vectors: list[list[float]] = []
        for row in ordered_rows:
            embedding = row.get("embedding") if isinstance(row, dict) else None
            if not isinstance(embedding, list) or len(embedding) != dimensions:
                raise FdEmbeddingError(
                    f"fd returned invalid embedding dimension: expected {dimensions}, got "
                    f"{len(embedding) if isinstance(embedding, list) else 'non-list'}"
                )
            vectors.append([float(value) for value in embedding])  # ty:ignore[invalid-argument-type]
        if len(vectors) != len(texts):
            raise FdEmbeddingError(f"fd returned {len(vectors)} embeddings for {len(texts)} inputs")
        return vectors


def load_figures(path: Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    figures = payload.get("figures") if isinstance(payload, dict) else payload
    if not isinstance(figures, list):
        raise ValueError("figure corpus must contain a figures[] list")
    return figures


def batched(items: list[dict[str, Any]], size: int) -> list[list[dict[str, Any]]]:
    return [items[index : index + size] for index in range(0, len(items), size)]


def embed_figures(
    *,
    corpus_path: Path = DEFAULT_CORPUS,
    output_path: Path = DEFAULT_OUTPUT,
    base_url: str = DEFAULT_BASE_URL,
    batch_size: int = DEFAULT_BATCH_SIZE,
    dimensions: int = DEFAULT_DIMENSIONS,
) -> dict[str, Any]:
    figures = load_figures(corpus_path)
    client = FdEmbeddingClient(base_url=base_url)
    embeddings: dict[str, list[float]] = {}
    started = time.perf_counter()
    for batch in batched(figures, batch_size):
        texts = [str(figure["text_repr"]) for figure in batch]
        vectors = client.embed_batch(texts, dimensions=dimensions)
        for figure, vector in zip(batch, vectors, strict=True):
            embeddings[str(figure["figure_id"])] = vector
    elapsed_ms = round((time.perf_counter() - started) * 1000, 3)
    payload: dict[str, Any] = {
        "schema_version": "m057.figure-caption-embeddings.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "base_url": base_url,
        "batch_size": batch_size,
        "dimensions": dimensions,
        "embedding_count": len(embeddings),
        "elapsed_ms": elapsed_ms,
        "embeddings": embeddings,
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=DEFAULT_CORPUS)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--batch-size", type=int, default=DEFAULT_BATCH_SIZE)
    parser.add_argument("--dimensions", type=int, default=DEFAULT_DIMENSIONS)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    payload = embed_figures(
        corpus_path=args.corpus,
        output_path=args.output,
        base_url=args.base_url,
        batch_size=args.batch_size,
        dimensions=args.dimensions,
    )
    print(
        json.dumps(
            {
                "output": str(args.output),
                "embedding_count": payload["embedding_count"],
                "dimensions": payload["dimensions"],
                "elapsed_ms": payload["elapsed_ms"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
