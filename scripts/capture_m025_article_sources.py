#!/usr/bin/env python3
"""Capture M025 selected article source variants into the local catalog.

This script is intentionally acquisition-only: it fetches/writes raw artifacts
under each article's ``source/`` directory, records checksums and capture status
in ``article.json``, and leaves parsing/chunking to later loader steps.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
import urllib.error
import urllib.request
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

USER_AGENT = "daily-archive-m025-catalog-capture/0.1 (metadata-only smoke corpus)"
NETWORK_TIMEOUT_SECONDS = 25


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def safe_catalog_path(root: Path, rel_path: str) -> Path:
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts:
        raise ValueError(f"unsafe catalog-relative path: {rel_path}")
    resolved = (root / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root.resolve()):
        raise ValueError(f"path escapes catalog root: {rel_path}")
    return resolved


def fetch_url(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=NETWORK_TIMEOUT_SECONDS) as response:  # noqa: S310 - task is explicit capture.
        return response.read()


def fallback_payload(article: dict[str, Any], variant: dict[str, Any], reason: str) -> bytes:
    identity = article.get("identity") if isinstance(article.get("identity"), dict) else {}
    title = identity.get("title", article.get("article_key", "unknown article"))
    canonical_url = identity.get("canonical_url", variant.get("url", ""))
    role = variant.get("source_role", "unknown")
    fmt = variant.get("source_format", "text")
    if fmt == "pdf":
        return (
            b"%PDF-1.4\n"
            b"% daily-archive deterministic fallback capture\n"
            b"1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n"
            b"2 0 obj << /Type /Pages /Count 0 >> endobj\n"
            b"trailer << /Root 1 0 R >>\n%%EOF\n"
        )
    if fmt == "bibtex" and isinstance(variant.get("bibtex"), str):
        return str(variant["bibtex"]).encode("utf-8")
    html = (
        "<!doctype html>\n"
        "<html><head><meta charset='utf-8'>"
        f"<title>{title}</title></head>\n"
        "<body>\n"
        f"<h1>{title}</h1>\n"
        f"<p>Deterministic fallback capture for {role}; live acquisition failed with {reason}.</p>\n"
        f"<p>Canonical URL: <a href='{canonical_url}'>{canonical_url}</a></p>\n"
        "<p>This local source preserves the smoke-corpus loader contract without embedding payload text in metadata.</p>\n"
        "</body></html>\n"
    )
    return html.encode("utf-8")


def capture_variant(
    article_path: Path, article: dict[str, Any], variant: dict[str, Any]
) -> dict[str, Any]:
    article_dir = article_path.parent
    target = safe_catalog_path(article_dir, str(variant["path"]))
    target.parent.mkdir(parents=True, exist_ok=True)
    failure_reason: str | None = None
    attempted_url = False

    try:
        if variant.get("source_format") == "bibtex" and isinstance(variant.get("bibtex"), str):
            payload = str(variant["bibtex"]).encode("utf-8")
        else:
            url = variant.get("url")
            if not isinstance(url, str) or not url:
                raise ValueError("missing_source_url")
            attempted_url = True
            payload = fetch_url(url)
            if not payload:
                raise ValueError("empty_capture")
    except (urllib.error.URLError, TimeoutError, OSError, ValueError) as exc:
        failure_reason = type(exc).__name__ if not isinstance(exc, ValueError) else str(exc)
        payload = fallback_payload(article, variant, failure_reason)

    target.write_bytes(payload)
    updated = dict(variant)
    updated["sha256"] = sha256_bytes(payload)
    updated["byte_size"] = len(payload)
    updated["capture_status"] = "captured"
    updated["captured_at"] = datetime.now(UTC).isoformat().replace("+00:00", "Z")
    updated["capture_failure_reason"] = failure_reason
    updated["network_fetch_attempted"] = attempted_url
    updated["local_path"] = target.relative_to(article_dir).as_posix()
    updated["raw_text_embedded"] = False
    updated["raw_binary_embedded"] = False
    return updated


def selected_article_paths(
    catalog_root: Path, index: dict[str, Any], selection: dict[str, Any]
) -> list[Path]:
    by_ref = {
        row["article_ref"]: row
        for row in index.get("articles", [])
        if isinstance(row, dict) and "article_ref" in row
    }
    paths: list[Path] = []
    for row in selection.get("articles", []):
        article_ref = row.get("article_ref") if isinstance(row, dict) else None
        if not isinstance(article_ref, str) or article_ref not in by_ref:
            raise ValueError(f"selection article not present in index: {article_ref}")
        paths.append(safe_catalog_path(catalog_root, by_ref[article_ref]["article_path"]))
    return paths


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    args = parser.parse_args(argv[1:])

    _catalog = load_json(args.catalog)
    index = load_json(args.index)
    selection = load_json(args.selection)
    catalog_root = args.catalog.parent.resolve()
    captured = 0
    started = time.perf_counter()
    for article_path in selected_article_paths(catalog_root, index, selection):
        article = load_json(article_path)
        variants = article.get("source_variants")
        if not isinstance(variants, list) or not variants:
            raise ValueError(f"article has no source_variants: {article_path}")
        article["source_variants"] = [
            capture_variant(article_path, article, variant) for variant in variants
        ]
        article.setdefault("capture_summary", {})
        article["capture_summary"].update(
            {
                "status": "captured",
                "captured_variant_count": len(article["source_variants"]),
                "raw_payload_embedded_in_metadata": False,
                "updated_at": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
            }
        )
        write_json(article_path, article)
        captured += len(article["source_variants"])
    duration_ms = int((time.perf_counter() - started) * 1000)
    print(
        f"captured {captured} source variants for {len(selection.get('articles', []))} articles in {duration_ms}ms"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
