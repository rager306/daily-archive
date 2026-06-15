#!/usr/bin/env python3
"""Embed M058 plotextractor v2 figure captions through local fd."""

from __future__ import annotations

import argparse
import json
import os
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
ARTIFACT_ROOT = ROOT / "artifacts" / "m058-plotextractor"
DEFAULT_BASE_URL = os.environ.get("TEI_URL", os.environ.get("FD_EMBEDDINGS_ENDPOINT_BASE", "http://127.0.0.1:8000"))
DEFAULT_API_KEY = os.environ.get("FD_API_KEY")
DEFAULT_MODEL_ID = os.environ.get("MODEL_ID", os.environ.get("FD_MODEL_NAME", "deepvk/USER-bge-m3"))
DEFAULT_REDIS_HOST = os.environ.get("REDIS_HOST", "127.0.0.1")
DEFAULT_REDIS_PORT = os.environ.get("REDIS_PORT", "6379")
DEFAULT_CORPUS = ARTIFACT_ROOT / "figure-caption-corpus.json"
DEFAULT_OUTPUT = ARTIFACT_ROOT / "embeddings.json"
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

    def embed_batch(self, texts: list[str], *, dimensions: int = DEFAULT_DIMENSIONS) -> list[list[float]]:
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
                raw = response.read().decode("utf-8")
        except (TimeoutError, urllib.error.URLError, urllib.error.HTTPError) as exc:
            raise FdEmbeddingError(f"fd embedding request failed: {exc}") from exc
        response_payload = json.loads(raw)
        data = response_payload.get("data")
        if not isinstance(data, list) or len(data) != len(texts):
            raise FdEmbeddingError("fd embedding response data length mismatch")
        embeddings: list[list[float]] = []
        for item in data:
            vector = item.get("embedding") if isinstance(item, dict) else None
            if not isinstance(vector, list) or len(vector) != dimensions:
                raise FdEmbeddingError(f"fd embedding dimension mismatch; expected {dimensions}")
            embeddings.append([float(value) for value in vector])
        return embeddings


def load_corpus(path: Path = DEFAULT_CORPUS) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    figures = payload.get("figures") if isinstance(payload, dict) else payload
    if not isinstance(figures, list):
        raise ValueError("figure corpus must contain a figures[] list")
    return figures


def _embedding_text(figure: dict[str, Any]) -> str:
    caption = str(figure.get("caption") or figure.get("caption_text") or "").strip()
    label = str(figure.get("label") or "").strip()
    arxiv_id = str(figure.get("arxiv_id") or "").strip()
    return f"Figure from {arxiv_id} {label}: {caption}".strip()


def embed_corpus(
    *,
    corpus_path: Path = DEFAULT_CORPUS,
    output_path: Path = DEFAULT_OUTPUT,
    base_url: str = DEFAULT_BASE_URL,
    batch_size: int = DEFAULT_BATCH_SIZE,
    dimensions: int = DEFAULT_DIMENSIONS,
    timeout_seconds: float = 120.0,
) -> dict[str, Any]:
    figures = load_corpus(corpus_path)
    client = FdEmbeddingClient(base_url=base_url, timeout_seconds=timeout_seconds)
    embedding_map: dict[str, list[float]] = {}
    latency_ms: list[float] = []
    for start in range(0, len(figures), batch_size):
        batch = figures[start : start + batch_size]
        texts = [_embedding_text(figure) for figure in batch]
        before = time.perf_counter()
        vectors = client.embed_batch(texts, dimensions=dimensions)
        latency_ms.append(round((time.perf_counter() - before) * 1000, 3))
        for figure, vector in zip(batch, vectors, strict=True):
            embedding_map[str(figure["figure_id"])] = vector
    payload: dict[str, Any] = {
        "schema_version": "m058.plotextractor.embeddings.v1",
        "safety_defaults": SAFETY_DEFAULTS,
        "base_url": base_url,
        "corpus_path": str(corpus_path),
        "dimensions": dimensions,
        "batch_size": batch_size,
        "figure_count": len(figures),
        "embedding_count": len(embedding_map),
        "batch_latency_ms": latency_ms,
        "embeddings": embedding_map,
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
    parser.add_argument("--timeout-seconds", type=float, default=120.0)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    payload = embed_corpus(
        corpus_path=args.corpus,
        output_path=args.output,
        base_url=args.base_url,
        batch_size=args.batch_size,
        dimensions=args.dimensions,
        timeout_seconds=args.timeout_seconds,
    )
    print(
        json.dumps(
            {
                "output": str(args.output),
                "figure_count": payload["figure_count"],
                "embedding_count": payload["embedding_count"],
                "dimensions": payload["dimensions"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
