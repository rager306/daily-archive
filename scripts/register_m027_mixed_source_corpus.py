#!/usr/bin/env python3
"""Register the M027 mixed-source corpus as metadata-only catalog records.

The command is intentionally deterministic: article identities and titles are
frozen in this file from a bounded registration lookup, and subsequent runs do
not fetch network content. It refreshes six metadata-only article records, merges
six title-bearing index rows while preserving existing catalog entries, and
writes the M027 corpus selection.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

SELECTION_ID = "m027-mixed-source-corpus-v1"
ARTICLE_SCHEMA_VERSION = "article.v00.01"
CATALOG_SCHEMA_VERSION = "article-catalog.v00.01"
INDEX_SCHEMA_VERSION = "article-catalog-index.v00.01"
REGISTRATION_ID = "m027_mixed_source_corpus_registration_v1"

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_manifests_embed_raw_text": False,
    "metadata_manifests_embed_raw_binary": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
}


@dataclass(frozen=True)
class ArticleSpec:
    article_ref: str
    source_code: str
    source_type: str
    publisher: str
    coarse_topic_code: str
    article_key: str
    title: str
    canonical_url: str
    seed_url: str
    topic_tags: tuple[str, ...]
    identity_extra: dict[str, Any]
    strategy: dict[str, Any]
    variants: tuple[dict[str, Any], ...]


# Titles were resolved during an explicit bounded registration step and are now
# frozen here so normal registration/validation runs stay local-only.
ARTICLE_SPECS: tuple[ArticleSpec, ...] = (
    ArticleSpec(
        article_ref="arxiv/mixed-source/2605.20897",
        source_code="arxiv",
        source_type="preprint_server",
        publisher="arxiv",
        coarse_topic_code="mixed-source",
        article_key="2605.20897",
        title="Creating Robust and Fair Graph Structures for Connectivity and Clustering",
        canonical_url="https://arxiv.org/abs/2605.20897",
        seed_url="https://arxiv.org/pdf/2605.20897",
        topic_tags=("graph-structures", "connectivity", "clustering", "fairness"),
        identity_extra={
            "arxiv_id": "2605.20897",
            "abs_url": "https://arxiv.org/abs/2605.20897",
            "pdf_url": "https://arxiv.org/pdf/2605.20897",
            "seed_url_preserved": "https://arxiv.org/pdf/2605.20897",
        },
        strategy={
            "primary_source_variant_id": "2605.20897:source:arxiv-abs",
            "preferred_content_order": ["arxiv_abs_page", "arxiv_pdf"],
            "metadata_order": ["arxiv_abs_page", "arxiv_api_metadata"],
            "pdf_policy": "seed_was_direct_pdf_preserve_as_content_fallback",
            "fallback_policy": "use_pdf_when_abs_metadata_or_future_html_is_missing_low_quality_or_inconsistent",
        },
        variants=(
            {
                "variant_id": "2605.20897:source:arxiv-abs",
                "source_role": "arxiv_abs_page",
                "source_format": "html_metadata",
                "source_origin": "provider_landing_page",
                "is_primary": True,
                "is_content_bearing": False,
                "is_metadata_only": True,
                "path": None,
                "url": "https://arxiv.org/abs/2605.20897",
                "media_type": "text/html",
                "capture_status": "not_captured",
                "capture_policy": "metadata_registration_only_no_network_fetch",
                "loader_outcome": "not_loaded_metadata_only",
                "requires_conversion": False,
                "conversion_hint": None,
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
            },
            {
                "variant_id": "2605.20897:source:arxiv-pdf-seed",
                "source_role": "arxiv_pdf",
                "source_format": "pdf",
                "source_origin": "seed_direct_pdf_url",
                "is_primary": False,
                "is_content_bearing": True,
                "is_metadata_only": False,
                "path": None,
                "url": "https://arxiv.org/pdf/2605.20897",
                "media_type": "application/pdf",
                "capture_status": "not_captured",
                "capture_policy": "metadata_registration_only_preserve_seed_no_fetch",
                "loader_outcome": "not_loaded",
                "requires_conversion": True,
                "conversion_hint": "future_pdf_to_markdown_conversion_after_acquisition",
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
            },
        ),
    ),
    ArticleSpec(
        article_ref="arxiv/mixed-source/2605.21401",
        source_code="arxiv",
        source_type="preprint_server",
        publisher="arxiv",
        coarse_topic_code="mixed-source",
        article_key="2605.21401",
        title="Open-source LLMs administer maximum electric shocks in a Milgram-like obedience experiment",
        canonical_url="https://arxiv.org/abs/2605.21401",
        seed_url="https://arxiv.org/abs/2605.21401",
        topic_tags=("llm-behavior", "obedience-experiment", "safety"),
        identity_extra={"arxiv_id": "2605.21401", "abs_url": "https://arxiv.org/abs/2605.21401", "pdf_url": "https://arxiv.org/pdf/2605.21401"},
        strategy={
            "primary_source_variant_id": "2605.21401:source:arxiv-abs",
            "preferred_content_order": ["arxiv_abs_page", "arxiv_pdf"],
            "metadata_order": ["arxiv_abs_page", "arxiv_api_metadata"],
            "pdf_policy": "capture_later_as_content_fallback",
            "fallback_policy": "use_pdf_when_abs_or_future_html_is_missing_low_quality_or_inconsistent",
        },
        variants=(),
    ),
    ArticleSpec(
        article_ref="nature/mixed-source/s44387-025-00019-5",
        source_code="nature",
        source_type="publisher",
        publisher="nature",
        coarse_topic_code="mixed-source",
        article_key="s44387-025-00019-5",
        title="Exploring the role of large language models in the scientific method: from hypothesis to discovery",
        canonical_url="https://www.nature.com/articles/s44387-025-00019-5",
        seed_url="https://www.nature.com/articles/s44387-025-00019-5",
        topic_tags=("large-language-models", "scientific-method", "hypothesis-discovery"),
        identity_extra={"doi_path": "s44387-025-00019-5", "publisher_url": "https://www.nature.com/articles/s44387-025-00019-5"},
        strategy={
            "primary_source_variant_id": "s44387-025-00019-5:source:nature-html",
            "preferred_content_order": ["nature_html"],
            "metadata_order": ["nature_html", "citation_metadata"],
            "pdf_policy": "no_pdf_claimed_until_acquisition_discovers_supported_variant",
            "fallback_policy": "manual_review_when_publisher_html_missing_or_low_quality",
            "parser_readiness": "not_claimed",
        },
        variants=(
            {
                "variant_id": "s44387-025-00019-5:source:nature-html",
                "source_role": "nature_html",
                "source_format": "html_metadata",
                "source_origin": "publisher_landing_page",
                "is_primary": True,
                "is_content_bearing": False,
                "is_metadata_only": True,
                "path": None,
                "url": "https://www.nature.com/articles/s44387-025-00019-5",
                "media_type": "text/html",
                "capture_status": "not_captured",
                "capture_policy": "metadata_registration_only_no_network_fetch",
                "loader_outcome": "not_loaded_metadata_only",
                "requires_conversion": False,
                "conversion_hint": None,
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
                "parser_readiness_claimed": False,
            },
            {
                "variant_id": "s44387-025-00019-5:source:citation-metadata",
                "source_role": "citation_metadata",
                "source_format": "metadata",
                "source_origin": "publisher_metadata",
                "is_primary": False,
                "is_content_bearing": False,
                "is_metadata_only": True,
                "path": None,
                "url": "https://www.nature.com/articles/s44387-025-00019-5",
                "media_type": "application/json",
                "capture_status": "not_captured",
                "capture_policy": "metadata_registration_only_no_network_fetch",
                "loader_outcome": "not_loaded_metadata_only",
                "requires_conversion": False,
                "conversion_hint": None,
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
            },
        ),
    ),
    ArticleSpec(
        article_ref="arxiv/mixed-source/2605.25522",
        source_code="arxiv",
        source_type="preprint_server",
        publisher="arxiv",
        coarse_topic_code="mixed-source",
        article_key="2605.25522",
        title="Co-Designing Graph-based Approximate Nearest Neighbor Search at Billion Scale for Processing-in-Memory",
        canonical_url="https://arxiv.org/abs/2605.25522",
        seed_url="https://arxiv.org/abs/2605.25522",
        topic_tags=("graph-ann", "processing-in-memory", "billion-scale-search"),
        identity_extra={"arxiv_id": "2605.25522", "abs_url": "https://arxiv.org/abs/2605.25522", "pdf_url": "https://arxiv.org/pdf/2605.25522"},
        strategy={},
        variants=(),
    ),
    ArticleSpec(
        article_ref="arxiv/mixed-source/2603.04448",
        source_code="arxiv",
        source_type="preprint_server",
        publisher="arxiv",
        coarse_topic_code="mixed-source",
        article_key="2603.04448",
        title="SkillNet: Create, Evaluate, and Connect AI Skills",
        canonical_url="https://arxiv.org/abs/2603.04448",
        seed_url="https://arxiv.org/abs/2603.04448",
        topic_tags=("ai-skills", "skill-evaluation", "agent-tools"),
        identity_extra={"arxiv_id": "2603.04448", "abs_url": "https://arxiv.org/abs/2603.04448", "pdf_url": "https://arxiv.org/pdf/2603.04448"},
        strategy={},
        variants=(),
    ),
    ArticleSpec(
        article_ref="arxiv/mixed-source/2604.18478",
        source_code="arxiv",
        source_type="preprint_server",
        publisher="arxiv",
        coarse_topic_code="mixed-source",
        article_key="2604.18478",
        title="WorldDB: A Vector Graph-of-Worlds Memory Engine with Ontology-Aware Write-Time Reconciliation",
        canonical_url="https://arxiv.org/abs/2604.18478",
        seed_url="https://arxiv.org/abs/2604.18478",
        topic_tags=("world-model-memory", "vector-graph", "ontology-reconciliation"),
        identity_extra={"arxiv_id": "2604.18478", "abs_url": "https://arxiv.org/abs/2604.18478", "pdf_url": "https://arxiv.org/pdf/2604.18478"},
        strategy={},
        variants=(),
    ),
)


def _default_arxiv_strategy(spec: ArticleSpec) -> dict[str, Any]:
    return {
        "primary_source_variant_id": f"{spec.article_key}:source:arxiv-abs",
        "preferred_content_order": ["arxiv_abs_page", "arxiv_pdf"],
        "metadata_order": ["arxiv_abs_page", "arxiv_api_metadata"],
        "pdf_policy": "capture_later_as_content_fallback",
        "fallback_policy": "use_pdf_when_abs_or_future_html_is_missing_low_quality_or_inconsistent",
    }


def _default_arxiv_variants(spec: ArticleSpec) -> tuple[dict[str, Any], ...]:
    return (
        {
            "variant_id": f"{spec.article_key}:source:arxiv-abs",
            "source_role": "arxiv_abs_page",
            "source_format": "html_metadata",
            "source_origin": "provider_landing_page",
            "is_primary": True,
            "is_content_bearing": False,
            "is_metadata_only": True,
            "path": None,
            "url": spec.canonical_url,
            "media_type": "text/html",
            "capture_status": "not_captured",
            "capture_policy": "metadata_registration_only_no_network_fetch",
            "loader_outcome": "not_loaded_metadata_only",
            "requires_conversion": False,
            "conversion_hint": None,
            "raw_text_embedded": False,
            "raw_binary_embedded": False,
            "network_fetch_attempted": False,
        },
        {
            "variant_id": f"{spec.article_key}:source:arxiv-pdf",
            "source_role": "arxiv_pdf",
            "source_format": "pdf",
            "source_origin": "provider_pdf",
            "is_primary": False,
            "is_content_bearing": True,
            "is_metadata_only": False,
            "path": None,
            "url": spec.identity_extra["pdf_url"],
            "media_type": "application/pdf",
            "capture_status": "not_captured",
            "capture_policy": "metadata_registration_only_no_network_fetch",
            "loader_outcome": "not_loaded",
            "requires_conversion": True,
            "conversion_hint": "future_pdf_to_markdown_conversion_after_acquisition",
            "raw_text_embedded": False,
            "raw_binary_embedded": False,
            "network_fetch_attempted": False,
        },
    )


def _path_safe(value: str) -> bool:
    return bool(value) and value == value.strip("/") and ".." not in value.split("/") and all(part for part in value.split("/"))


def _validate_specs(specs: Iterable[ArticleSpec]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    seen_refs: set[str] = set()
    seen_urls: set[str] = set()
    seen_titles: set[str] = set()
    for spec in specs:
        base = {
            "selection_id": SELECTION_ID,
            "article_ref": spec.article_ref,
            "seed_url": spec.seed_url,
            "network_fetch_attempted": False,
            "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        }
        if spec.article_ref in seen_refs:
            diagnostics.append({**base, "level": "error", "code": "duplicate_article_ref", "json_path": "$.articles", "message": "article_ref must be unique"})
        seen_refs.add(spec.article_ref)
        if spec.seed_url in seen_urls:
            diagnostics.append({**base, "level": "error", "code": "duplicate_seed_url", "json_path": "$.articles", "message": "seed_url must be unique"})
        seen_urls.add(spec.seed_url)
        if spec.title in seen_titles:
            diagnostics.append({**base, "level": "error", "code": "duplicate_title", "json_path": "$.articles", "message": "title must be unique for title lookup"})
        seen_titles.add(spec.title)
        if not _path_safe(spec.article_ref):
            diagnostics.append({**base, "level": "error", "code": "unsafe_article_ref", "json_path": "$.article_ref", "message": "article_ref must be path-safe"})
        if not spec.title.strip():
            diagnostics.append({**base, "level": "error", "code": "missing_title", "json_path": "$.identity.title", "message": "title must be frozen before registration"})
        if spec.source_code not in {"arxiv", "nature"}:
            diagnostics.append({**base, "level": "error", "code": "unsupported_source", "json_path": "$.source_code", "message": f"unsupported source_code {spec.source_code!r}"})
        if spec.source_code == "arxiv" and not re.fullmatch(r"\d{4}\.\d{4,5}", spec.article_key):
            diagnostics.append({**base, "level": "error", "code": "malformed_arxiv_key", "json_path": "$.article_key", "message": "arxiv article_key must be normalized without /abs or /pdf suffix"})
    return diagnostics


def _article_record(spec: ArticleSpec) -> dict[str, Any]:
    strategy = spec.strategy or _default_arxiv_strategy(spec)
    variants = spec.variants or _default_arxiv_variants(spec)
    identity = {
        "title": spec.title,
        "canonical_url": spec.canonical_url,
        "seed_url": spec.seed_url,
        **spec.identity_extra,
    }
    return {
        "schema_version": ARTICLE_SCHEMA_VERSION,
        "article_key": spec.article_key,
        "catalog_path": spec.article_ref,
        "source_code": spec.source_code,
        "source_type": spec.source_type,
        "publisher": spec.publisher,
        "coarse_topic_code": spec.coarse_topic_code,
        "topic_tags": list(spec.topic_tags),
        "identity": identity,
        "source_strategy": strategy,
        "source_variants": list(variants),
        "expected_profile": {
            "should_load": False,
            "should_parse_text": False,
            "should_chunk": False,
            "known_risks": [
                "metadata_only_registration_no_source_artifact_captured",
                "future_acquisition_must_replay_source_loader_before_parser_or_graph_use",
            ],
        },
        "safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        "registration_summary": {
            "registration_id": REGISTRATION_ID,
            "selection_id": SELECTION_ID,
            "status": "registered_metadata_only",
            "title_status": "frozen",
            "source_artifact_captured": False,
            "raw_payload_embedded_in_metadata": False,
            "network_fetch_attempted": False,
        },
    }


def _index_entry(spec: ArticleSpec, article: dict[str, Any]) -> dict[str, Any]:
    primary = next(variant for variant in article["source_variants"] if variant.get("is_primary"))
    content_fallback_roles = [
        variant["source_role"]
        for variant in article["source_variants"]
        if not variant.get("is_primary") and variant.get("is_content_bearing")
    ]
    metadata_roles = [variant["source_role"] for variant in article["source_variants"] if variant.get("is_metadata_only")]
    return {
        "article_ref": spec.article_ref,
        "article_key": spec.article_key,
        "source_code": spec.source_code,
        "coarse_topic_code": spec.coarse_topic_code,
        "canonical_url": spec.canonical_url,
        "primary_source_role": primary["source_role"],
        "content_fallback_roles": content_fallback_roles,
        "metadata_roles": metadata_roles,
        "article_path": f"article_catalog/{spec.article_ref}/article.json",
        "title": spec.title,
        "seed_url": spec.seed_url,
        "registration_id": REGISTRATION_ID,
    }


def _selection_payload(specs: Iterable[ArticleSpec]) -> dict[str, Any]:
    return {
        "schema_version": "article-corpus-selection.v00.01",
        "selection_id": SELECTION_ID,
        "catalog_schema_version": CATALOG_SCHEMA_VERSION,
        "article_schema_version": ARTICLE_SCHEMA_VERSION,
        "purpose": "M027 mixed-source metadata-only corpus registration for future acquisition/conversion work.",
        "selection_mode": "manual_url_seed",
        "registration_id": REGISTRATION_ID,
        "network_policy": {
            "capture_phase_may_fetch": False,
            "registration_command_fetches_network": False,
            "test_phase_must_not_fetch": True,
            "pipeline_phase_reads_catalog_only": True,
        },
        "articles": [
            {
                "article_ref": spec.article_ref,
                "source_code": spec.source_code,
                "article_key": spec.article_key,
                "title": spec.title,
                "seed_url": spec.seed_url,
                "canonical_url": spec.canonical_url,
                "article_path": f"article_catalog/{spec.article_ref}/article.json",
                "title_status": "frozen",
                "network_fetch_attempted": False,
            }
            for spec in specs
        ],
        "safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
    }


def _read_json(path: Path, default: dict[str, Any]) -> dict[str, Any]:
    if not path.exists():
        return default
    try:
        with path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"malformed JSON in {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise RuntimeError(f"expected JSON object in {path}")
    return payload


def _atomic_write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(payload, indent=2, sort_keys=False) + "\n"
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_name, path)
    except Exception:
        try:
            os.unlink(tmp_name)
        except FileNotFoundError:
            pass
        raise


def _build_indexes(entries: list[dict[str, Any]]) -> dict[str, Any]:
    indexes: dict[str, Any] = {
        "by_article_key": {},
        "by_citation_key": {},
        "by_source_code": {},
        "by_coarse_topic_code": {},
        "by_canonical_url": {},
        "by_title": {},
    }
    for entry in entries:
        ref = entry["article_ref"]
        indexes["by_article_key"][entry["article_key"]] = ref
        indexes["by_canonical_url"][entry["canonical_url"]] = ref
        indexes["by_title"][entry["title"]] = ref
        if entry.get("citation_key"):
            indexes["by_citation_key"][entry["citation_key"]] = ref
        indexes["by_source_code"].setdefault(entry["source_code"], []).append(ref)
        indexes["by_coarse_topic_code"].setdefault(entry["coarse_topic_code"], []).append(ref)
    for name in ("by_source_code", "by_coarse_topic_code"):
        indexes[name] = {key: sorted(value) for key, value in sorted(indexes[name].items())}
    for name in ("by_article_key", "by_citation_key", "by_canonical_url", "by_title"):
        indexes[name] = {key: indexes[name][key] for key in sorted(indexes[name])}
    return indexes


def _merge_index(existing: dict[str, Any], new_entries: list[dict[str, Any]]) -> dict[str, Any]:
    replaced_refs = {entry["article_ref"] for entry in new_entries}
    preserved_entries = [entry for entry in existing.get("articles", []) if entry.get("article_ref") not in replaced_refs]
    entries = sorted([*preserved_entries, *new_entries], key=lambda row: row["article_ref"])
    return {
        "schema_version": existing.get("schema_version", INDEX_SCHEMA_VERSION),
        "catalog_schema_version": existing.get("catalog_schema_version", CATALOG_SCHEMA_VERSION),
        "article_schema_version": existing.get("article_schema_version", ARTICLE_SCHEMA_VERSION),
        "index_id": existing.get("index_id", "daily_archive_article_catalog_index_fixture_v00_01"),
        "generated_from": existing.get("generated_from", "article_catalog/"),
        "lookup_policy": existing.get(
            "lookup_policy",
            {
                "cli_must_use_index": True,
                "full_tree_scan_allowed": False,
                "refresh_command_rebuilds_index": True,
            },
        ),
        "articles": entries,
        "indexes": _build_indexes(entries),
        "safety_flags": {**FAIL_CLOSED_SAFETY_FLAGS, **existing.get("safety_flags", {})},
    }


def _diagnostic_for_write(spec: ArticleSpec, article_path: Path, index_entry: dict[str, Any]) -> dict[str, Any]:
    return {
        "level": "info",
        "code": "registered_metadata_only_article",
        "selection_id": SELECTION_ID,
        "article_ref": spec.article_ref,
        "seed_url": spec.seed_url,
        "lookup_key": spec.article_key,
        "file_path": str(article_path),
        "normalized_source_strategy": index_entry["primary_source_role"],
        "title_status": "frozen",
        "failing_invariant": None,
        "network_fetch_attempted": False,
        "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
    }


def register(catalog_root: Path, corpora_root: Path, *, write: bool) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    diagnostics = _validate_specs(ARTICLE_SPECS)
    if diagnostics:
        return diagnostics, {"status": "blocked", "selection_id": SELECTION_ID, "article_count": 0}

    index_path = catalog_root / "index.json"
    existing_index = _read_json(index_path, {"articles": []})

    articles: list[tuple[ArticleSpec, dict[str, Any], dict[str, Any], Path]] = []
    for spec in ARTICLE_SPECS:
        article = _article_record(spec)
        entry = _index_entry(spec, article)
        path = catalog_root / "article_catalog" / spec.article_ref / "article.json"
        articles.append((spec, article, entry, path))
        diagnostics.append(_diagnostic_for_write(spec, path, entry))

    new_index = _merge_index(existing_index, [entry for _, _, entry, _ in articles])
    selection = _selection_payload(ARTICLE_SPECS)
    selection_path = corpora_root / SELECTION_ID / "selection.json"

    summary = {
        "status": "registered" if write else "dry_run",
        "registration_id": REGISTRATION_ID,
        "selection_id": SELECTION_ID,
        "article_count": len(articles),
        "index_path": str(index_path),
        "selection_path": str(selection_path),
        "network_fetch_attempted": False,
        "metadata_only": True,
        "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        "article_refs": [spec.article_ref for spec in ARTICLE_SPECS],
    }

    if write:
        for _, article, _, path in articles:
            _atomic_write_json(path, article)
        _atomic_write_json(index_path, new_index)
        _atomic_write_json(selection_path, selection)

    return diagnostics, summary


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Write/refresh catalog article records, index, and M027 selection.")
    parser.add_argument("--catalog-root", type=Path, default=Path("data/article_catalog"), help="Catalog root containing index.json and article_catalog/.")
    parser.add_argument("--corpora-root", type=Path, default=Path("data/article_corpora"), help="Root directory for corpus selections.")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        diagnostics, summary = register(args.catalog_root, args.corpora_root, write=args.write)
    except Exception as exc:
        error = {
            "level": "error",
            "code": "registration_io_failed",
            "selection_id": SELECTION_ID,
            "article_ref": None,
            "lookup_key": None,
            "file_path": None,
            "failing_invariant": "json_read_or_atomic_write",
            "network_fetch_attempted": False,
            "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
            "message": str(exc),
        }
        print(json.dumps(error, sort_keys=True), file=sys.stderr)
        return 1

    for diagnostic in diagnostics:
        stream = sys.stderr if diagnostic.get("level") == "error" else sys.stdout
        print(json.dumps(diagnostic, sort_keys=True), file=stream)
    print(json.dumps(summary, sort_keys=True))
    return 1 if any(diagnostic.get("level") == "error" for diagnostic in diagnostics) else 0


if __name__ == "__main__":
    raise SystemExit(main())
