#!/usr/bin/env python3
"""Register M029 missing requested refs as metadata-only catalog records.

This command is intentionally local-only. It freezes the two user-requested
identities from the M029 selection file, writes article metadata records, merges
index rows, and updates catalog source metadata needed for Stanford course notes.
It never fetches network content, captures source payloads, parses, chunks,
imports to graph stores, or writes to production/LadybugDB.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tempfile
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SELECTION_ID = "m029-pipeline-architecture-audit-v1"
ARTICLE_SCHEMA_VERSION = "article.v00.01"
CATALOG_SCHEMA_VERSION = "article-catalog.v00.01"
INDEX_SCHEMA_VERSION = "article-catalog-index.v00.01"
REGISTRATION_ID = "m029_missing_refs_metadata_registration_v1"

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
    "parser_ready_claimed": False,
    "chunk_ready_claimed": False,
    "kg_readiness_claimed": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}

EXPECTED_PROFILE: dict[str, bool] = {
    "should_load": False,
    "should_parse_text": False,
    "should_chunk": False,
    "graph_ready": False,
    "parser_ready": False,
    "chunk_ready": False,
}


@dataclass(frozen=True)
class ArticleSpec:
    article_ref: str
    normalized_identity: str
    source_code: str
    source_type: str
    publisher: str
    coarse_topic_code: str
    article_key: str
    title: str
    canonical_url: str
    seed_url: str
    source_kind: str
    topic_tags: tuple[str, ...]
    identity_extra: dict[str, Any]
    strategy: dict[str, Any]
    variants: tuple[dict[str, Any], ...]


ARTICLE_SPECS: tuple[ArticleSpec, ...] = (
    ArticleSpec(
        article_ref="stanford/cs224n/gradient-notes",
        normalized_identity="stanford:cs224n:gradient-notes",
        source_code="stanford",
        source_type="course_material",
        publisher="stanford",
        coarse_topic_code="cs224n",
        article_key="gradient-notes",
        title="CS224n Gradient Notes",
        canonical_url="https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf",
        seed_url="https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf",
        source_kind="external_pdf_url",
        topic_tags=("gradients", "neural-networks", "course-notes"),
        identity_extra={
            "course_code": "cs224n",
            "document_key": "gradient-notes",
            "observed_sha256_from_selection": "a2823e49e1a2849b1adb887c3091fe537a273aa5bbc190de117965afaac072b2",
        },
        strategy={
            "primary_source_variant_id": "gradient-notes:source:external-pdf-metadata",
            "preferred_content_order": ["external_pdf"],
            "metadata_order": ["manual_metadata", "external_pdf_metadata"],
            "pdf_policy": "future_local_acquisition_required_before_conversion_or_parser_readiness",
            "fallback_policy": "manual_review_when_pdf_missing_or_hash_differs_from_selection_observation",
            "parser_readiness": "not_claimed",
            "chunk_readiness": "not_claimed",
            "graph_readiness": "not_claimed",
        },
        variants=(
            {
                "variant_id": "gradient-notes:source:external-pdf-metadata",
                "source_role": "external_pdf",
                "source_format": "pdf",
                "source_origin": "stanford_course_reading_url",
                "is_primary": True,
                "is_content_bearing": True,
                "is_metadata_only": False,
                "path": None,
                "url": "https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf",
                "media_type": "application/pdf",
                "capture_status": "not_captured",
                "capture_policy": "metadata_registration_only_no_network_fetch",
                "loader_outcome": "not_loaded",
                "requires_conversion": True,
                "conversion_hint": "future_pdf_to_markdown_conversion_after_local_acquisition",
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
                "parser_readiness_claimed": False,
                "chunk_readiness_claimed": False,
                "graph_readiness_claimed": False,
            },
        ),
    ),
    ArticleSpec(
        article_ref="arxiv/mixed-source/2605.29548",
        normalized_identity="arxiv:2605.29548",
        source_code="arxiv",
        source_type="preprint_server",
        publisher="arxiv",
        coarse_topic_code="mixed-source",
        article_key="2605.29548",
        title="Why Larger Models Learn More: Effects of Capacity, Interference, and Rare-Task Retention",
        canonical_url="https://arxiv.org/abs/2605.29548",
        seed_url="https://arxiv.org/abs/2605.29548",
        source_kind="arxiv_abs_url",
        topic_tags=("model-capacity", "interference", "rare-task-retention"),
        identity_extra={
            "arxiv_id": "2605.29548",
            "abs_url": "https://arxiv.org/abs/2605.29548",
            "pdf_url": "https://arxiv.org/pdf/2605.29548",
        },
        strategy={
            "primary_source_variant_id": "2605.29548:source:arxiv-abs",
            "preferred_content_order": ["arxiv_abs_page", "arxiv_pdf"],
            "metadata_order": ["arxiv_abs_page", "arxiv_api_metadata"],
            "pdf_policy": "future_local_acquisition_required_before_conversion_or_parser_readiness",
            "fallback_policy": "use_pdf_when_abs_metadata_or_future_html_is_missing_low_quality_or_inconsistent",
            "parser_readiness": "not_claimed",
            "chunk_readiness": "not_claimed",
            "graph_readiness": "not_claimed",
        },
        variants=(
            {
                "variant_id": "2605.29548:source:arxiv-abs",
                "source_role": "arxiv_abs_page",
                "source_format": "html_metadata",
                "source_origin": "provider_landing_page",
                "is_primary": True,
                "is_content_bearing": False,
                "is_metadata_only": True,
                "path": None,
                "url": "https://arxiv.org/abs/2605.29548",
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
                "chunk_readiness_claimed": False,
                "graph_readiness_claimed": False,
            },
            {
                "variant_id": "2605.29548:source:arxiv-pdf-metadata",
                "source_role": "arxiv_pdf",
                "source_format": "pdf",
                "source_origin": "provider_pdf",
                "is_primary": False,
                "is_content_bearing": True,
                "is_metadata_only": False,
                "path": None,
                "url": "https://arxiv.org/pdf/2605.29548",
                "media_type": "application/pdf",
                "capture_status": "not_captured",
                "capture_policy": "metadata_registration_only_no_network_fetch",
                "loader_outcome": "not_loaded",
                "requires_conversion": True,
                "conversion_hint": "future_pdf_to_markdown_conversion_after_local_acquisition",
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "network_fetch_attempted": False,
                "parser_readiness_claimed": False,
                "chunk_readiness_claimed": False,
                "graph_readiness_claimed": False,
            },
        ),
    ),
)


def _path_safe(value: str) -> bool:
    return (
        bool(value)
        and value == value.strip("/")
        and ".." not in value.split("/")
        and all(part for part in value.split("/"))
    )


def _unsafe_true_paths(payload: Any, prefix: str = "$") -> list[str]:
    unsafe_names = {
        "metadata_manifests_embed_raw_text",
        "metadata_manifests_embed_raw_binary",
        "graph_import_allowed",
        "production_ladybugdb_write_allowed",
        "trusted_kg_import_allowed",
        "production_import_attempted",
        "ladybugdb_written",
        "raw_text_embedded",
        "raw_binary_embedded",
        "raw_text_embedded_in_metadata",
        "raw_binary_embedded_in_metadata",
        "parser_ready_claimed",
        "chunk_ready_claimed",
        "kg_readiness_claimed",
        "graph_write_attempted",
        "production_persistence_attempted",
        "network_fetch_attempted",
        "parser_readiness_claimed",
        "chunk_readiness_claimed",
        "graph_readiness_claimed",
        "should_load",
        "should_parse_text",
        "should_chunk",
        "graph_ready",
        "parser_ready",
        "chunk_ready",
    }
    paths: list[str] = []
    if isinstance(payload, dict):
        for key, value in payload.items():
            path = f"{prefix}.{key}"
            if key in unsafe_names and value is True:
                paths.append(path)
            paths.extend(_unsafe_true_paths(value, path))
    elif isinstance(payload, list):
        for index, value in enumerate(payload):
            paths.extend(_unsafe_true_paths(value, f"{prefix}[{index}]"))
    return paths


def _article_record(spec: ArticleSpec) -> dict[str, Any]:
    identity = {
        "title": spec.title,
        "canonical_url": spec.canonical_url,
        "seed_url": spec.seed_url,
        "normalized_identity": spec.normalized_identity,
        "source_kind": spec.source_kind,
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
        "source_strategy": spec.strategy,
        "source_variants": list(spec.variants),
        "expected_profile": {
            **EXPECTED_PROFILE,
            "known_risks": [
                "metadata_only_registration_no_source_artifact_captured",
                "future_acquisition_must_replay_source_loader_before_parser_or_graph_use",
                "not_safe_for_ladybugdb_or_production_import",
            ],
        },
        "safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        "registration_summary": {
            "registration_id": REGISTRATION_ID,
            "selection_id": SELECTION_ID,
            "status": "registered_metadata_only",
            "title_status": "frozen_from_selection",
            "source_artifact_captured": False,
            "parser_readiness_claimed": False,
            "chunk_readiness_claimed": False,
            "graph_readiness_claimed": False,
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
    metadata_roles = [
        variant["source_role"]
        for variant in article["source_variants"]
        if variant.get("is_metadata_only")
    ]
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
        "normalized_identity": spec.normalized_identity,
        "registration_id": REGISTRATION_ID,
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
        if entry.get("canonical_url"):
            indexes["by_canonical_url"][entry["canonical_url"]] = ref
        if entry.get("title"):
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
    preserved_entries = [
        entry
        for entry in existing.get("articles", [])
        if entry.get("article_ref") not in replaced_refs
        and entry.get("catalog_record_present") is not False
    ]
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


def _merge_catalog_sources(catalog: dict[str, Any]) -> dict[str, Any]:
    stanford_source = {
        "source_code": "stanford",
        "source_type": "course_material",
        "allowed_source_roles": ["external_pdf", "manual_metadata", "external_pdf_metadata"],
    }
    sources = list(catalog.get("sources", []))
    if not any(source.get("source_code") == "stanford" for source in sources):
        sources.append(stanford_source)
    else:
        sources = [
            {
                **source,
                "allowed_source_roles": sorted(
                    set(source.get("allowed_source_roles", []))
                    | set(stanford_source["allowed_source_roles"])
                ),
            }
            if source.get("source_code") == "stanford"
            else source
            for source in sources
        ]
    return {
        **catalog,
        "sources": sorted(sources, key=lambda source: source.get("source_code", "")),
        "safety_flags": {**FAIL_CLOSED_SAFETY_FLAGS, **catalog.get("safety_flags", {})},
    }


def _validate_specs(specs: Iterable[ArticleSpec]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    seen_refs: set[str] = set()
    seen_identities: set[str] = set()
    seen_urls: set[str] = set()
    seen_titles: set[str] = set()
    for spec in specs:
        base = {
            "selection_id": SELECTION_ID,
            "registration_id": REGISTRATION_ID,
            "article_ref": spec.article_ref,
            "normalized_identity": spec.normalized_identity,
            "seed_url": spec.seed_url,
            "network_fetch_attempted": False,
            "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        }
        if spec.article_ref in seen_refs:
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "duplicate_article_ref",
                    "json_path": "$.articles",
                    "message": "article_ref must be unique",
                }
            )
        seen_refs.add(spec.article_ref)
        if spec.normalized_identity in seen_identities:
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "duplicate_normalized_identity",
                    "json_path": "$.identity.normalized_identity",
                    "message": "normalized identity must be unique",
                }
            )
        seen_identities.add(spec.normalized_identity)
        if spec.seed_url in seen_urls:
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "duplicate_seed_url",
                    "json_path": "$.articles",
                    "message": "seed_url must be unique",
                }
            )
        seen_urls.add(spec.seed_url)
        if spec.title in seen_titles:
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "duplicate_title",
                    "json_path": "$.articles",
                    "message": "title must be unique for title lookup",
                }
            )
        seen_titles.add(spec.title)
        if not _path_safe(spec.article_ref):
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "unsafe_article_ref",
                    "json_path": "$.article_ref",
                    "message": "article_ref must be path-safe",
                }
            )
        if not spec.title.strip() or not spec.canonical_url.strip() or not spec.seed_url.strip():
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "missing_metadata",
                    "json_path": "$.identity",
                    "message": "title, canonical_url, and seed_url must be frozen before registration",
                }
            )
        if spec.source_code == "arxiv" and not re.fullmatch(r"\d{4}\.\d{4,5}", spec.article_key):
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "malformed_arxiv_key",
                    "json_path": "$.article_key",
                    "message": "arxiv article_key must be normalized without /abs or /pdf suffix",
                }
            )
        article = _article_record(spec)
        unsafe_paths = _unsafe_true_paths(article)
        if unsafe_paths:
            diagnostics.append(
                {
                    **base,
                    "level": "error",
                    "code": "unsafe_readiness_or_persistence_flag",
                    "json_path": unsafe_paths[0],
                    "message": f"unsafe true flag blocks metadata-only registration: {unsafe_paths[0]}",
                }
            )
    return diagnostics


def _diagnostic_for_write(
    spec: ArticleSpec, article_path: Path, index_entry: dict[str, Any]
) -> dict[str, Any]:
    return {
        "level": "info",
        "code": "registered_metadata_only_article",
        "selection_id": SELECTION_ID,
        "registration_id": REGISTRATION_ID,
        "article_ref": spec.article_ref,
        "normalized_identity": spec.normalized_identity,
        "seed_url": spec.seed_url,
        "lookup_key": spec.article_key,
        "file_path": str(article_path),
        "normalized_source_strategy": index_entry["primary_source_role"],
        "title_status": "frozen_from_selection",
        "failing_invariant": None,
        "network_fetch_attempted": False,
        "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
    }


def register(catalog_root: Path, *, write: bool) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    diagnostics = _validate_specs(ARTICLE_SPECS)
    if diagnostics:
        return diagnostics, {"status": "blocked", "selection_id": SELECTION_ID, "article_count": 0}

    catalog_path = catalog_root / "catalog.json"
    index_path = catalog_root / "index.json"
    catalog = _read_json(
        catalog_path,
        {
            "schema_version": CATALOG_SCHEMA_VERSION,
            "article_schema_version": ARTICLE_SCHEMA_VERSION,
            "sources": [],
        },
    )
    existing_index = _read_json(index_path, {"articles": []})

    articles: list[tuple[ArticleSpec, dict[str, Any], dict[str, Any], Path]] = []
    for spec in ARTICLE_SPECS:
        article = _article_record(spec)
        entry = _index_entry(spec, article)
        path = catalog_root / "article_catalog" / spec.article_ref / "article.json"
        articles.append((spec, article, entry, path))
        diagnostics.append(_diagnostic_for_write(spec, path, entry))

    new_catalog = _merge_catalog_sources(catalog)
    new_index = _merge_index(existing_index, [entry for _, _, entry, _ in articles])

    summary = {
        "status": "registered" if write else "dry_run",
        "registration_id": REGISTRATION_ID,
        "selection_id": SELECTION_ID,
        "article_count": len(articles),
        "catalog_path": str(catalog_path),
        "index_path": str(index_path),
        "network_fetch_attempted": False,
        "metadata_only": True,
        "source_artifact_captured": False,
        "parser_readiness_claimed": False,
        "chunk_readiness_claimed": False,
        "graph_readiness_claimed": False,
        "production_or_ladybugdb_write_attempted": False,
        "fail_closed_safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        "article_refs": [spec.article_ref for spec in ARTICLE_SPECS],
        "normalized_identities": [spec.normalized_identity for spec in ARTICLE_SPECS],
    }

    if write:
        for _, article, _, path in articles:
            _atomic_write_json(path, article)
        _atomic_write_json(catalog_path, new_catalog)
        _atomic_write_json(index_path, new_index)

    return diagnostics, summary


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="Write/refresh metadata-only catalog article records and index rows.",
    )
    parser.add_argument(
        "--catalog-root",
        type=Path,
        default=Path("data/article_catalog"),
        help="Catalog root containing catalog.json, index.json, and article_catalog/.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        diagnostics, summary = register(args.catalog_root, write=args.write)
    except Exception as exc:
        error = {
            "level": "error",
            "code": "registration_io_failed",
            "selection_id": SELECTION_ID,
            "registration_id": REGISTRATION_ID,
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
