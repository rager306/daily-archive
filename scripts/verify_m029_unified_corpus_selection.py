#!/usr/bin/env python3
"""Verify the M029 unified corpus selection registry."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

ARXIV_URL_RE = re.compile(r"^https://arxiv\.org/(abs|pdf|html)/(\d{4}\.\d{4,5})(v\d+)?(?:\.pdf)?/?$")
REQUIRED_SAFETY_FALSE = (
    "graph_import_allowed",
    "production_ladybugdb_write_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
)


def _read_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"missing JSON file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc


def _canonical_url(url: str) -> str:
    clean = url.rstrip("/")
    match = ARXIV_URL_RE.match(clean)
    if match:
        _, arxiv_id, version = match.groups()
        return f"https://arxiv.org/abs/{arxiv_id}{version or ''}"
    return clean


def _load_catalog_refs(catalog_path: Path) -> tuple[set[str], set[str]]:
    catalog = _read_json(catalog_path)
    index_path = catalog_path.parent / catalog.get("index", {}).get("path", "index.json")
    index = _read_json(index_path)
    refs = {entry["article_ref"] for entry in index.get("articles", []) if entry.get("article_ref")}
    urls = {entry["canonical_url"] for entry in index.get("articles", []) if entry.get("canonical_url")}
    return refs, urls


def verify(selection_path: Path, catalog_path: Path, expected_count: int, expected_duplicate_url: str | None) -> dict[str, Any]:
    selection = _read_json(selection_path)
    provenance_path = selection_path.with_name("selection-provenance.json")
    summary_path = selection_path.with_name("selection-summary.json")
    provenance = _read_json(provenance_path)
    summary = _read_json(summary_path)
    catalog_refs, catalog_urls = _load_catalog_refs(catalog_path)

    diagnostics: list[dict[str, Any]] = []
    articles = selection.get("articles")
    if not isinstance(articles, list):
        diagnostics.append({"level": "error", "code": "selection_articles_missing", "path": str(selection_path)})
        articles = []

    identity_keys = [article.get("identity_key") for article in articles]
    canonical_urls = [article.get("canonical_url") for article in articles]
    if len(articles) != expected_count:
        diagnostics.append({"level": "error", "code": "unexpected_unique_article_count", "expected": expected_count, "actual": len(articles)})
    if len(set(identity_keys)) != len(identity_keys):
        diagnostics.append({"level": "error", "code": "duplicate_identity_keys", "duplicates": sorted({key for key in identity_keys if identity_keys.count(key) > 1})})
    if len(set(canonical_urls)) != len(canonical_urls):
        diagnostics.append({"level": "error", "code": "duplicate_canonical_urls_in_selection"})

    for article in articles:
        for field in ("identity_key", "source_code", "article_key", "canonical_url", "seed_url", "source_strategy", "catalog_resolution", "provenance_sources"):
            if field not in article:
                diagnostics.append({"level": "error", "code": "article_missing_required_field", "field": field, "article": article})
        resolution = article.get("catalog_resolution")
        if resolution not in {"resolved", "unresolved"}:
            diagnostics.append({"level": "error", "code": "invalid_catalog_resolution", "article": article})
        if resolution == "resolved":
            if article.get("article_ref") not in catalog_refs:
                diagnostics.append({"level": "error", "code": "resolved_article_ref_not_in_catalog", "article_ref": article.get("article_ref")})
            if _canonical_url(str(article.get("canonical_url"))) not in {_canonical_url(url) for url in catalog_urls}:
                diagnostics.append({"level": "error", "code": "resolved_canonical_url_not_in_catalog", "canonical_url": article.get("canonical_url")})

    safety = selection.get("safety_flags", {})
    for flag in REQUIRED_SAFETY_FALSE:
        if safety.get(flag) is not False:
            diagnostics.append({"level": "error", "code": "unsafe_flag_not_false", "flag": flag, "actual": safety.get(flag)})

    summary_count = summary.get("unique_article_count")
    provenance_count = provenance.get("unique_article_count")
    if summary_count != len(articles) or provenance_count != len(articles):
        diagnostics.append({"level": "error", "code": "artifact_count_mismatch", "selection": len(articles), "summary": summary_count, "provenance": provenance_count})

    duplicate_urls = summary.get("duplicate_urls", {})
    if expected_duplicate_url and duplicate_urls.get(expected_duplicate_url, 0) < 2:
        diagnostics.append({"level": "error", "code": "expected_duplicate_url_missing", "url": expected_duplicate_url, "duplicates": duplicate_urls})
    if summary.get("duplicate_url_count", 0) != len(duplicate_urls):
        diagnostics.append({"level": "error", "code": "duplicate_url_counter_mismatch"})

    resolution_counts = {"resolved": 0, "unresolved": 0}
    for article in articles:
        resolution_counts[str(article.get("catalog_resolution"))] = resolution_counts.get(str(article.get("catalog_resolution")), 0) + 1
    if summary.get("index_resolution") != {key: value for key, value in sorted(resolution_counts.items()) if value}:
        diagnostics.append({"level": "error", "code": "index_resolution_counter_mismatch", "expected": resolution_counts, "actual": summary.get("index_resolution")})

    m028_expansion_count = sum(1 for row in provenance.get("articles", []) for obs in row.get("observations", []) if obs.get("source_id") == "M028" and obs.get("source_subset") == "newly_accepted_expansion_refs")
    if m028_expansion_count != 7:
        diagnostics.append({"level": "error", "code": "m028_expansion_provenance_count_mismatch", "expected": 7, "actual": m028_expansion_count})

    return {
        "status": "passed" if not diagnostics else "failed",
        "selection_id": selection.get("selection_id"),
        "unique_article_count": len(articles),
        "duplicate_url_count": summary.get("duplicate_url_count"),
        "index_resolution": summary.get("index_resolution"),
        "diagnostics": diagnostics,
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--catalog", type=Path, required=True)
    parser.add_argument("--expect-unique-article-count", type=int, required=True)
    parser.add_argument("--expect-duplicate-url")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        result = verify(args.selection, args.catalog, args.expect_unique_article_count, args.expect_duplicate_url)
    except Exception as exc:
        print(json.dumps({"status": "failed", "code": "verification_error", "message": str(exc)}, sort_keys=True), file=sys.stderr)
        return 1
    stream = sys.stdout if result["status"] == "passed" else sys.stderr
    print(json.dumps(result, sort_keys=True), file=stream)
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
