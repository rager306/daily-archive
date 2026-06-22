#!/usr/bin/env python3
"""Validate and explicitly rebuild the M025 reusable article catalog index.

Normal validation uses ``index.json`` as the lookup surface. Full catalog tree
traversal is intentionally confined to the explicit ``--rebuild-index`` path so
pipeline/test phases can detect when a network refresh or index refresh would be
needed instead of silently scanning or fetching.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
import sys
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any

CATALOG_SCHEMA_VERSION = "article-catalog.v00.01"
ARTICLE_SCHEMA_VERSION = "article.v00.01"
INDEX_SCHEMA_VERSION = "article-catalog-index.v00.01"
SELECTION_SCHEMA_VERSION = "article-corpus-selection.v00.01"
EXPECTED_SELECTION_ID = "m025-rlm-dspy-pageindex-smoke-v1"
CATALOG_RECORD_DIR = "article_catalog"
DEFAULT_REPORT_TITLE = "Article Catalog Readiness Report"

FORBIDDEN_TRUE_FLAGS = {
    "metadata_manifests_embed_raw_text",
    "metadata_manifests_embed_raw_binary",
    "raw_text_embedded",
    "raw_binary_embedded",
    "graph_import_allowed",
    "production_ladybugdb_write_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
}

REBUILD_TRAVERSAL_FUNCTIONS = {"iter_canonical_article_record_paths", "rebuild_index_from_articles"}


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"malformed JSON in {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise ValueError(f"JSON root must be an object: {path}")
    return value


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = json.dumps(payload, indent=2, sort_keys=False, ensure_ascii=False) + "\n"
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False, prefix=f".{path.name}."
    ) as handle:
        temp_name = handle.name
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temp_name, path)


def write_jsonl_atomic(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False, prefix=f".{path.name}."
    ) as handle:
        temp_name = handle.name
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, ensure_ascii=False) + "\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temp_name, path)


def diagnostic(
    code: str, message: str, *, severity: str = "error", **fields: Any
) -> dict[str, Any]:
    row: dict[str, Any] = {"severity": severity, "code": code, "message": message}
    row.update({key: value for key, value in fields.items() if value is not None})
    return row


def require_equal(errors: list[str], path: str, actual: Any, expected: Any) -> None:
    if actual != expected:
        errors.append(f"{path} must be {expected!r}; got {actual!r}")


def check_safety_flags(errors: list[str], location: str, value: Any) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            child_location = f"{location}.{key}" if location else key
            if key in FORBIDDEN_TRUE_FLAGS and child is not False:
                errors.append(f"{child_location} must be false; got {child!r}")
            check_safety_flags(errors, child_location, child)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            check_safety_flags(errors, f"{location}[{index}]", child)


def normalize_posix_path(value: str) -> str:
    return PurePosixPath(value.replace("\\", "/")).as_posix()


def catalog_root(catalog_path: Path) -> Path:
    return catalog_path.parent.resolve()


def safe_catalog_path(catalog_path: Path, article_path: str) -> Path:
    normalized = normalize_posix_path(article_path)
    if normalized.startswith("/") or ".." in PurePosixPath(normalized).parts:
        raise ValueError(f"unsafe catalog-relative path: {article_path}")
    root = catalog_root(catalog_path)
    resolved = (catalog_path.parent / normalized).resolve()
    if not resolved.is_relative_to(root):
        raise ValueError(f"path resolves outside catalog root: {article_path}")
    return resolved


def article_manifest_path(catalog_path: Path, article_path: str) -> Path:
    return safe_catalog_path(catalog_path, article_path)


def article_ref_from_path(article_path: str) -> str:
    normalized = normalize_posix_path(article_path)
    prefix = f"{CATALOG_RECORD_DIR}/"
    suffix = "/article.json"
    if not normalized.startswith(prefix) or not normalized.endswith(suffix):
        raise ValueError(f"non-canonical article path: {article_path}")
    return normalized[len(prefix) : -len(suffix)]


def validate_catalog(catalog_path: Path, catalog: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    require_equal(
        errors, "catalog.schema_version", catalog.get("schema_version"), CATALOG_SCHEMA_VERSION
    )
    require_equal(
        errors,
        "catalog.article_schema_version",
        catalog.get("article_schema_version"),
        ARTICLE_SCHEMA_VERSION,
    )
    require_equal(errors, "catalog.root", catalog.get("root"), "data/article_catalog")
    require_equal(
        errors,
        "catalog.path_template",
        catalog.get("path_template"),
        "{source_code}/{coarse_topic_code}/{article_key}",
    )
    index_policy = catalog.get("index")
    if not isinstance(index_policy, dict):
        errors.append("catalog.index must be an object")
    else:
        require_equal(
            errors,
            "catalog.index.schema_version",
            index_policy.get("schema_version"),
            INDEX_SCHEMA_VERSION,
        )
        require_equal(errors, "catalog.index.path", index_policy.get("path"), "index.json")
        require_equal(
            errors, "catalog.index.cli_must_use_index", index_policy.get("cli_must_use_index"), True
        )
        require_equal(
            errors,
            "catalog.index.full_tree_scan_allowed",
            index_policy.get("full_tree_scan_allowed"),
            False,
        )
        require_equal(
            errors,
            "catalog.index.refresh_command_rebuilds_index",
            index_policy.get("refresh_command_rebuilds_index"),
            True,
        )
        lookup_keys = (
            set(index_policy.get("lookup_keys", []))
            if isinstance(index_policy.get("lookup_keys"), list)
            else set()
        )
        missing_lookup = sorted(
            {
                "article_key",
                "citation_key",
                "canonical_url",
                "source_code",
                "coarse_topic_code",
                "title",
            }
            - lookup_keys
        )
        if missing_lookup:
            errors.append(f"catalog.index.lookup_keys missing: {', '.join(missing_lookup)}")
    for schema_name in ("article-catalog-schema.v00.01.json", "article-schema.v00.01.json"):
        schema_path = catalog_path.parent / "schemas" / schema_name
        if not schema_path.exists():
            errors.append(f"missing schema file: {schema_path}")
        else:
            try:
                load_json(schema_path)
            except ValueError as exc:
                errors.append(str(exc))
    check_safety_flags(errors, "catalog", catalog)
    return errors


def check_duplicate_lookup_entries(index: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    articles = index.get("articles")
    if not isinstance(articles, list):
        return errors
    scalar_keys = ("article_ref", "article_key", "canonical_url", "citation_key", "title")
    seen: dict[tuple[str, str], str] = {}
    for position, entry in enumerate(articles):
        if not isinstance(entry, dict):
            continue
        article_ref = entry.get("article_ref")
        ref_label = article_ref if isinstance(article_ref, str) else f"articles[{position}]"
        for key in scalar_keys:
            value = entry.get(key)
            if not isinstance(value, str) or not value:
                continue
            prior = seen.get((key, value))
            if prior and prior != ref_label:
                errors.append(f"duplicate lookup key {key}={value!r}: {prior} and {ref_label}")
            else:
                seen[(key, value)] = ref_label
    return errors


def validate_index(
    catalog_path: Path,
    index: dict[str, Any],
    *,
    require_index: bool,
    check_index_titles: bool,
    check_safe_traversal: bool = False,
    check_duplicate_lookups: bool = False,
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    errors: list[str] = []
    if require_index:
        require_equal(
            errors, "index.schema_version", index.get("schema_version"), INDEX_SCHEMA_VERSION
        )
        require_equal(
            errors,
            "index.catalog_schema_version",
            index.get("catalog_schema_version"),
            CATALOG_SCHEMA_VERSION,
        )
        require_equal(
            errors,
            "index.article_schema_version",
            index.get("article_schema_version"),
            ARTICLE_SCHEMA_VERSION,
        )
        lookup_policy = index.get("lookup_policy")
        if not isinstance(lookup_policy, dict):
            errors.append("index.lookup_policy must be an object")
        else:
            require_equal(
                errors,
                "index.lookup_policy.cli_must_use_index",
                lookup_policy.get("cli_must_use_index"),
                True,
            )
            require_equal(
                errors,
                "index.lookup_policy.full_tree_scan_allowed",
                lookup_policy.get("full_tree_scan_allowed"),
                False,
            )
            require_equal(
                errors,
                "index.lookup_policy.refresh_command_rebuilds_index",
                lookup_policy.get("refresh_command_rebuilds_index"),
                True,
            )
    articles = index.get("articles")
    if not isinstance(articles, list) or not articles:
        return errors + ["index.articles must be a non-empty list"], {}

    if check_duplicate_lookups:
        errors.extend(check_duplicate_lookup_entries(index))

    by_ref: dict[str, dict[str, Any]] = {}
    for position, entry in enumerate(articles):
        if not isinstance(entry, dict):
            errors.append(f"index.articles[{position}] must be an object")
            continue
        article_ref = entry.get("article_ref")
        article_path = entry.get("article_path")
        if not isinstance(article_ref, str) or not article_ref:
            errors.append(f"index.articles[{position}].article_ref must be a non-empty string")
            continue
        if article_ref in by_ref:
            errors.append(f"duplicate index article_ref: {article_ref}")
        by_ref[article_ref] = entry  # ty:ignore[invalid-assignment]
        if not isinstance(article_path, str) or not article_path.endswith("/article.json"):
            errors.append(f"{article_ref} article_path must end with /article.json")
            continue
        try:
            if check_safe_traversal:
                expected_ref = article_ref_from_path(article_path)
                require_equal(errors, f"{article_ref}.article_path_ref", expected_ref, article_ref)
            manifest_path = article_manifest_path(catalog_path, article_path)
        except ValueError as exc:
            errors.append(f"{article_ref} {exc}")
            continue
        try:
            article = load_json(manifest_path)
        except ValueError as exc:
            errors.append(str(exc))
            continue
        require_equal(
            errors,
            f"{article_ref}.schema_version",
            article.get("schema_version"),
            ARTICLE_SCHEMA_VERSION,
        )
        require_equal(
            errors, f"{article_ref}.catalog_path", article.get("catalog_path"), article_ref
        )
        require_equal(
            errors,
            f"{article_ref}.source_code",
            article.get("source_code"),
            entry.get("source_code"),
        )
        require_equal(
            errors,
            f"{article_ref}.coarse_topic_code",
            article.get("coarse_topic_code"),
            entry.get("coarse_topic_code"),
        )
        require_equal(
            errors,
            f"{article_ref}.article_key",
            article.get("article_key"),
            entry.get("article_key"),
        )
        if check_index_titles:
            require_equal(
                errors,
                f"{article_ref}.index_title",
                entry.get("title"),
                article.get("identity", {}).get("title")
                if isinstance(article.get("identity"), dict)
                else None,
            )
        variants = article.get("source_variants")
        if not isinstance(variants, list) or not variants:
            errors.append(f"{article_ref} source_variants must be a non-empty list")
        check_safety_flags(errors, article_ref, article)

    indexes = index.get("indexes")
    if not isinstance(indexes, dict):
        errors.append("index.indexes must be an object")
    else:
        for lookup_name, lookup_key in (
            ("by_article_key", "article_key"),
            ("by_citation_key", "citation_key"),
            ("by_canonical_url", "canonical_url"),
            ("by_title", "title"),
        ):
            lookup = indexes.get(lookup_name)
            if not isinstance(lookup, dict):
                errors.append(f"index.indexes.{lookup_name} must be an object")
                continue
            for article_ref, entry in by_ref.items():
                value = entry.get(lookup_key)
                if isinstance(value, str) and value:
                    require_equal(
                        errors,
                        f"index.indexes.{lookup_name}[{value!r}]",
                        lookup.get(value),
                        article_ref,
                    )
        for group_name, lookup_key in (
            ("by_source_code", "source_code"),
            ("by_coarse_topic_code", "coarse_topic_code"),
        ):
            grouped = indexes.get(group_name)
            if not isinstance(grouped, dict):
                errors.append(f"index.indexes.{group_name} must be an object")
                continue
            for article_ref, entry in by_ref.items():
                group_value = entry.get(lookup_key)
                refs = grouped.get(group_value) if isinstance(group_value, str) else None
                if not isinstance(refs, list) or article_ref not in refs:
                    errors.append(
                        f"index.indexes.{group_name}[{group_value!r}] missing {article_ref}"
                    )
    check_safety_flags(errors, "index", index)
    return errors, by_ref


def validate_selection(
    selection: dict[str, Any],
    index_articles: dict[str, dict[str, Any]],
    *,
    expected_selection_id: str | None,
    require_selection_titles: bool = False,
    allow_index_superset: bool = False,
) -> list[str]:
    errors: list[str] = []
    require_equal(
        errors,
        "selection.schema_version",
        selection.get("schema_version"),
        SELECTION_SCHEMA_VERSION,
    )
    if expected_selection_id is not None:
        require_equal(
            errors, "selection.selection_id", selection.get("selection_id"), expected_selection_id
        )
    require_equal(
        errors,
        "selection.catalog_schema_version",
        selection.get("catalog_schema_version"),
        CATALOG_SCHEMA_VERSION,
    )
    require_equal(
        errors,
        "selection.article_schema_version",
        selection.get("article_schema_version"),
        ARTICLE_SCHEMA_VERSION,
    )
    network_policy = selection.get("network_policy")
    if not isinstance(network_policy, dict):
        errors.append("selection.network_policy must be an object")
    else:
        require_equal(
            errors,
            "selection.network_policy.test_phase_must_not_fetch",
            network_policy.get("test_phase_must_not_fetch"),
            True,
        )
        require_equal(
            errors,
            "selection.network_policy.pipeline_phase_reads_catalog_only",
            network_policy.get("pipeline_phase_reads_catalog_only"),
            True,
        )
    articles = selection.get("articles")
    if not isinstance(articles, list) or not articles:
        errors.append("selection.articles must be a non-empty list")
    else:
        selection_refs: set[str] = set()
        for position, row in enumerate(articles):
            if not isinstance(row, dict):
                errors.append(f"selection.articles[{position}] must be an object")
                continue
            article_ref = row.get("article_ref")
            if not isinstance(article_ref, str) or not article_ref:
                errors.append(
                    f"selection.articles[{position}].article_ref must be a non-empty string"
                )
                continue
            selection_refs.add(article_ref)
            if article_ref not in index_articles:
                errors.append(f"selection article_ref not present in index: {article_ref}")
            index_entry = index_articles.get(article_ref, {})
            index_source = index_entry.get("source_code")
            if row.get("source_code") != index_source:
                errors.append(f"selection {article_ref} source_code does not match index")
            title = row.get("title")
            index_title = index_entry.get("title")
            if require_selection_titles and (not isinstance(title, str) or not title):
                errors.append(f"selection {article_ref} title must be a non-empty string")
            if isinstance(title, str) and title and title != index_title:
                errors.append(f"selection {article_ref} title does not match index")
        missing_from_selection = sorted(set(index_articles) - selection_refs)
        if missing_from_selection and not allow_index_superset:
            errors.append(
                f"index articles missing from selection: {', '.join(missing_from_selection)}"
            )
    check_safety_flags(errors, "selection", selection)
    return errors


def _selection_article_records(
    catalog_path: Path,
    selection: dict[str, Any],
    index_articles: dict[str, dict[str, Any]],
) -> list[tuple[str, Path, dict[str, Any] | None, str | None]]:
    records: list[tuple[str, Path, dict[str, Any] | None, str | None]] = []
    articles = selection.get("articles") if isinstance(selection.get("articles"), list) else []
    for row in articles:  # ty:ignore[not-iterable]
        article_ref = row.get("article_ref") if isinstance(row, dict) else None
        if not isinstance(article_ref, str):
            continue
        entry = index_articles.get(article_ref)
        if not isinstance(entry, dict):
            records.append(
                (article_ref, catalog_path.parent / "<missing>", None, "missing_index_entry")
            )
            continue
        article_path = entry.get("article_path")
        if not isinstance(article_path, str):
            records.append(
                (article_ref, catalog_path.parent / "<missing>", None, "missing_article_path")
            )
            continue
        try:
            path = article_manifest_path(catalog_path, article_path)
            article = load_json(path)
        except ValueError as exc:
            records.append((article_ref, catalog_path.parent / article_path, None, str(exc)))
            continue
        records.append((article_ref, path, article, None))
    return records


def _local_article_artifact_path(article_path: Path, rel_path: str) -> Path:
    normalized = normalize_posix_path(rel_path)
    if normalized.startswith("/") or ".." in PurePosixPath(normalized).parts:
        raise ValueError(f"unsafe article-relative path: {rel_path}")
    root = article_path.parent.resolve()
    resolved = (article_path.parent / normalized).resolve()
    if not resolved.is_relative_to(root):
        raise ValueError(f"article artifact path escapes article directory: {rel_path}")
    return resolved


def check_captured_sources(
    catalog_path: Path,
    selection: dict[str, Any],
    index_articles: dict[str, dict[str, Any]],
    *,
    check_checksums: bool,
) -> list[str]:
    errors: list[str] = []
    for article_ref, article_path, article, load_error in _selection_article_records(
        catalog_path, selection, index_articles
    ):
        if load_error:
            errors.append(
                f"{article_ref} cannot load article record for capture checks: {load_error}"
            )
            continue
        assert article is not None
        variants = article.get("source_variants")
        if not isinstance(variants, list) or not variants:
            errors.append(f"{article_ref} source_variants must be present for capture checks")
            continue
        for position, variant in enumerate(variants):
            if not isinstance(variant, dict):
                errors.append(f"{article_ref}.source_variants[{position}] must be an object")
                continue
            variant_id = variant.get("variant_id", f"source_variants[{position}]")
            if variant.get("capture_status") != "captured":
                errors.append(f"{article_ref} {variant_id} capture_status must be 'captured'")
            rel_path = variant.get("path")
            if not isinstance(rel_path, str) or not rel_path:
                errors.append(f"{article_ref} {variant_id} path must be a non-empty string")
                continue
            try:
                source_path = _local_article_artifact_path(article_path, rel_path)
            except ValueError as exc:
                errors.append(f"{article_ref} {variant_id} {exc}")
                continue
            if not source_path.exists():
                errors.append(f"{article_ref} {variant_id} source file missing: {source_path}")
                continue
            byte_size = source_path.stat().st_size
            if variant.get("byte_size") != byte_size:
                errors.append(
                    f"{article_ref} {variant_id} byte_size drift: metadata={variant.get('byte_size')!r} actual={byte_size!r}"
                )
            if check_checksums:
                actual_sha = hashlib.sha256(source_path.read_bytes()).hexdigest()
                if variant.get("sha256") != actual_sha:
                    errors.append(f"{article_ref} {variant_id} sha256 drift")
            if (
                variant.get("raw_text_embedded") is not False
                or variant.get("raw_binary_embedded") is not False
            ):
                errors.append(
                    f"{article_ref} {variant_id} raw payload embedding flags must be false"
                )
    return errors


def _jsonl_rows(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    if not path.exists():
        return rows
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def check_loader_events(
    catalog_path: Path,
    selection: dict[str, Any],
    index_articles: dict[str, dict[str, Any]],
    *,
    check_redaction: bool,
) -> list[str]:
    errors: list[str] = []
    forbidden = [
        "raw_text",
        "raw_bytes",
        "binary_payload",
        "base64",
        "embedding",
        "embeddings",
        "vector",
        "vectors",
        "api_key",
    ]
    for article_ref, article_path, article, load_error in _selection_article_records(
        catalog_path, selection, index_articles
    ):
        if load_error:
            errors.append(
                f"{article_ref} cannot load article record for loader checks: {load_error}"
            )
            continue
        assert article is not None
        loader_dir = article_path.parent / "loader"
        event_path = loader_dir / "events.jsonl"
        summary_path = loader_dir / "summary.json"
        if not event_path.exists():
            errors.append(f"{article_ref} missing loader events: {event_path}")
            continue
        if not summary_path.exists():
            errors.append(f"{article_ref} missing loader summary: {summary_path}")
            continue
        try:
            summary = load_json(summary_path)
            events = _jsonl_rows(event_path)
        except (ValueError, json.JSONDecodeError) as exc:
            errors.append(f"{article_ref} malformed loader artifact: {exc}")
            continue
        variants = (
            article.get("source_variants")
            if isinstance(article.get("source_variants"), list)
            else []
        )
        terminal_events = [
            row
            for row in events
            if row.get("event")
            in {"source.load_completed", "source.load_failed", "source.load_metadata_only"}
        ]
        if len(terminal_events) != len(variants):  # ty:ignore[invalid-argument-type]
            errors.append(f"{article_ref} terminal loader event count must match source variants")
        if (
            summary.get("lookup_surface") != "index.json"
            or summary.get("full_tree_scan_attempted") is not False
        ):
            errors.append(f"{article_ref} loader summary must declare index-only lookup")
        for variant in variants:  # ty:ignore[not-iterable]
            if isinstance(variant, dict) and variant.get("loader_outcome") not in {
                "loaded",
                "loaded_metadata_only",
                "failed",
            }:
                errors.append(
                    f"{article_ref} {variant.get('variant_id')} has invalid loader_outcome {variant.get('loader_outcome')!r}"
                )
        if check_redaction:
            serialized = json.dumps({"events": events, "summary": summary}, sort_keys=True)
            for token in forbidden:
                if token in serialized:
                    errors.append(
                        f"{article_ref} loader artifacts contain forbidden token {token!r}"
                    )
    return errors


def iter_canonical_article_record_paths(
    catalog_path: Path,
) -> tuple[list[Path], list[dict[str, Any]]]:
    """Return only canonical source/topic/key/article.json records under the catalog root."""
    root = catalog_root(catalog_path)
    records_root = root / CATALOG_RECORD_DIR
    diagnostics: list[dict[str, Any]] = []
    if not records_root.exists():
        diagnostics.append(
            diagnostic(
                "missing_records_root",
                "catalog article record directory is missing",
                path=str(records_root),
            )
        )
        return [], diagnostics
    paths: list[Path] = []
    for source_dir in sorted(
        (p for p in records_root.iterdir() if p.is_dir()), key=lambda p: p.name
    ):
        for topic_dir in sorted(
            (p for p in source_dir.iterdir() if p.is_dir()), key=lambda p: p.name
        ):
            for article_dir in sorted(
                (p for p in topic_dir.iterdir() if p.is_dir()), key=lambda p: p.name
            ):
                article_path = article_dir / "article.json"
                if not article_path.exists():
                    diagnostics.append(
                        diagnostic(
                            "missing_article_record",
                            "canonical article directory is missing article.json",
                            path=str(article_path),
                        )
                    )
                    continue
                resolved = article_path.resolve()
                if not resolved.is_relative_to(root):
                    diagnostics.append(
                        diagnostic(
                            "unsafe_traversal",
                            "article record resolves outside catalog root",
                            path=str(article_path),
                        )
                    )
                    continue
                paths.append(article_path)
    return paths, diagnostics


def article_entry_from_record(
    catalog_path: Path, record_path: Path, article: dict[str, Any]
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    diags: list[dict[str, Any]] = []
    root = catalog_root(catalog_path)
    rel_path = record_path.resolve().relative_to(root).as_posix()
    try:
        article_ref = article_ref_from_path(rel_path)
    except ValueError as exc:
        return None, [diagnostic("non_canonical_record_path", str(exc), path=rel_path)]

    source_code, coarse_topic_code, article_key = article_ref.split("/", 2)
    identity = article.get("identity") if isinstance(article.get("identity"), dict) else {}
    variants = (
        article.get("source_variants") if isinstance(article.get("source_variants"), list) else []
    )
    primary_roles = [
        v.get("source_role")
        for v in variants  # ty:ignore[not-iterable]
        if isinstance(v, dict) and v.get("is_primary") is True
    ]
    fallback_roles = [
        v.get("source_role")
        for v in variants  # ty:ignore[not-iterable]
        if isinstance(v, dict)
        and v.get("is_content_bearing") is True
        and v.get("is_primary") is not True
    ]
    metadata_roles = [
        v.get("source_role")
        for v in variants  # ty:ignore[not-iterable]
        if isinstance(v, dict) and v.get("is_metadata_only") is True
    ]

    required = {
        "schema_version": ARTICLE_SCHEMA_VERSION,
        "catalog_path": article_ref,
        "source_code": source_code,
        "coarse_topic_code": coarse_topic_code,
        "article_key": article_key,
    }
    for key, expected in required.items():
        actual = article.get(key)
        if actual != expected:
            diags.append(
                diagnostic(
                    "malformed_article_record",
                    f"{article_ref}.{key} must be {expected!r}; got {actual!r}",
                    article_ref=article_ref,
                )
            )
    title = identity.get("title")  # ty:ignore[unresolved-attribute]
    canonical_url = identity.get("canonical_url")  # ty:ignore[unresolved-attribute]
    if not isinstance(title, str) or not title:
        diags.append(
            diagnostic(
                "malformed_article_record",
                "article identity.title must be a non-empty string",
                article_ref=article_ref,
            )
        )
    if not isinstance(canonical_url, str) or not canonical_url:
        diags.append(
            diagnostic(
                "malformed_article_record",
                "article identity.canonical_url must be a non-empty string",
                article_ref=article_ref,
            )
        )
    if not variants:
        diags.append(
            diagnostic(
                "malformed_article_record",
                "article source_variants must be a non-empty list",
                article_ref=article_ref,
            )
        )

    check_errors: list[str] = []
    check_safety_flags(check_errors, article_ref, article)
    for error in check_errors:
        diags.append(diagnostic("unsafe_safety_flag", error, article_ref=article_ref))

    entry: dict[str, Any] = {
        "article_ref": article_ref,
        "article_key": article_key,
        "source_code": source_code,
        "coarse_topic_code": coarse_topic_code,
        "canonical_url": canonical_url,
        "primary_source_role": primary_roles[0] if primary_roles else None,
        "content_fallback_roles": sorted(role for role in fallback_roles if isinstance(role, str)),
        "metadata_roles": sorted(role for role in metadata_roles if isinstance(role, str)),
        "article_path": rel_path,
        "title": title,
    }
    citation_key = identity.get("citation_key")  # ty:ignore[unresolved-attribute]
    if isinstance(citation_key, str) and citation_key:
        entry["citation_key"] = citation_key
    return entry, diags


def add_scalar_lookup(
    lookup: dict[str, str],
    diagnostics: list[dict[str, Any]],
    lookup_name: str,
    key: str,
    article_ref: str,
) -> None:
    if not key:
        return
    prior = lookup.get(key)
    if prior and prior != article_ref:
        diagnostics.append(
            diagnostic(
                "duplicate_lookup_key",
                f"duplicate {lookup_name} key {key!r}: {prior} and {article_ref}",
                lookup=lookup_name,
                lookup_key=key,
                article_ref=article_ref,
                prior_article_ref=prior,
            )
        )
    else:
        lookup[key] = article_ref


def build_lookup_maps(
    entries: list[dict[str, Any]], diagnostics: list[dict[str, Any]]
) -> dict[str, Any]:
    indexes: dict[str, Any] = {
        "by_article_key": {},
        "by_citation_key": {},
        "by_source_code": {},
        "by_coarse_topic_code": {},
        "by_canonical_url": {},
        "by_title": {},
    }
    for entry in entries:
        article_ref = entry["article_ref"]
        add_scalar_lookup(
            indexes["by_article_key"],
            diagnostics,
            "by_article_key",
            entry.get("article_key", ""),
            article_ref,
        )
        add_scalar_lookup(
            indexes["by_citation_key"],
            diagnostics,
            "by_citation_key",
            entry.get("citation_key", ""),
            article_ref,
        )
        add_scalar_lookup(
            indexes["by_canonical_url"],
            diagnostics,
            "by_canonical_url",
            entry.get("canonical_url", ""),
            article_ref,
        )
        add_scalar_lookup(
            indexes["by_title"], diagnostics, "by_title", entry.get("title", ""), article_ref
        )
        indexes["by_source_code"].setdefault(entry.get("source_code"), []).append(article_ref)
        indexes["by_coarse_topic_code"].setdefault(entry.get("coarse_topic_code"), []).append(
            article_ref
        )
    for map_name, lookup in indexes.items():
        if map_name in {"by_source_code", "by_coarse_topic_code"}:
            indexes[map_name] = {key: sorted(value) for key, value in sorted(lookup.items())}
        else:
            indexes[map_name] = {key: lookup[key] for key in sorted(lookup)}
    return indexes


def rebuild_index_from_articles(
    catalog_path: Path, existing_index: dict[str, Any]
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    records, diagnostics = iter_canonical_article_record_paths(catalog_path)
    entries: list[dict[str, Any]] = []
    seen_refs: set[str] = set()
    for record_path in records:
        try:
            article = load_json(record_path)
        except ValueError as exc:
            diagnostics.append(
                diagnostic("malformed_article_record", str(exc), path=str(record_path))
            )
            continue
        entry, entry_diags = article_entry_from_record(catalog_path, record_path, article)
        diagnostics.extend(entry_diags)
        if entry is None:
            continue
        if entry["article_ref"] in seen_refs:
            diagnostics.append(
                diagnostic(
                    "duplicate_lookup_key",
                    f"duplicate article_ref: {entry['article_ref']}",
                    article_ref=entry["article_ref"],
                )
            )
        seen_refs.add(entry["article_ref"])
        entries.append(entry)

    entries = sorted(entries, key=lambda row: row["article_ref"])
    indexes = build_lookup_maps(entries, diagnostics)
    rebuilt = {
        "schema_version": INDEX_SCHEMA_VERSION,
        "catalog_schema_version": CATALOG_SCHEMA_VERSION,
        "article_schema_version": ARTICLE_SCHEMA_VERSION,
        "index_id": existing_index.get(
            "index_id", "daily_archive_article_catalog_index_fixture_v00_01"
        ),
        "generated_from": f"{CATALOG_RECORD_DIR}/",
        "lookup_policy": {
            "cli_must_use_index": True,
            "full_tree_scan_allowed": False,
            "refresh_command_rebuilds_index": True,
        },
        "articles": entries,
        "indexes": indexes,
        "safety_flags": {
            "metadata_manifests_embed_raw_text": False,
            "metadata_manifests_embed_raw_binary": False,
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
    }
    return rebuilt, diagnostics


def normalize_lookup_maps(indexes: Any) -> Any:
    if not isinstance(indexes, dict):
        return indexes
    normalized: dict[str, Any] = {}
    for key, value in indexes.items():
        if isinstance(value, dict):
            normalized[key] = {
                child_key: sorted(child_value) if isinstance(child_value, list) else child_value
                for child_key, child_value in sorted(value.items())
            }
        else:
            normalized[key] = value
    return normalized


def compare_existing_to_rebuilt(
    existing: dict[str, Any], rebuilt: dict[str, Any]
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    existing_by_ref = {
        row.get("article_ref"): row for row in existing.get("articles", []) if isinstance(row, dict)
    }
    rebuilt_by_ref = {
        row.get("article_ref"): row for row in rebuilt.get("articles", []) if isinstance(row, dict)
    }
    # pyrefly: ignore [bad-specialization]
    for article_ref in sorted(set(rebuilt_by_ref) - set(existing_by_ref)):
        diagnostics.append(
            diagnostic(
                "missing_index_entry",
                f"existing index is missing {article_ref}",
                article_ref=article_ref,
            )
        )
    # pyrefly: ignore [bad-specialization]
    for article_ref in sorted(set(existing_by_ref) - set(rebuilt_by_ref)):
        diagnostics.append(
            diagnostic(
                "stale_index_entry",
                f"existing index has stale {article_ref}",
                article_ref=article_ref,
            )
        )
    drift_fields = {
        "article_path": "path_drift",
        "title": "title_drift",
        "source_code": "source_topic_drift",
        "coarse_topic_code": "source_topic_drift",
        "canonical_url": "canonical_url_drift",
        "article_key": "source_topic_drift",
    }
    # pyrefly: ignore [bad-specialization]
    for article_ref in sorted(set(existing_by_ref) & set(rebuilt_by_ref)):
        existing_row = existing_by_ref[article_ref]
        rebuilt_row = rebuilt_by_ref[article_ref]
        for field, code in drift_fields.items():
            if existing_row.get(field) != rebuilt_row.get(field):
                diagnostics.append(
                    diagnostic(
                        code,
                        f"{article_ref}.{field} drift: index has {existing_row.get(field)!r}; article record projects {rebuilt_row.get(field)!r}",
                        article_ref=article_ref,
                        field=field,
                    )
                )
    if normalize_lookup_maps(existing.get("indexes")) != normalize_lookup_maps(
        rebuilt.get("indexes")
    ):
        diagnostics.append(
            diagnostic("lookup_map_drift", "existing lookup maps differ from rebuilt projection")
        )
    return diagnostics


def build_rebuild_report(
    rebuilt: dict[str, Any],
    diagnostics: list[dict[str, Any]],
    *,
    existing_matches_rebuilt: bool,
    second_pass_matches: bool | None,
    write_index: Path | None,
) -> dict[str, Any]:
    counts: dict[str, int] = {}
    for row in diagnostics:
        code = str(row.get("code"))
        counts[code] = counts.get(code, 0) + 1
    error_count = sum(1 for row in diagnostics if row.get("severity") == "error")
    return {
        "schema_version": "article-catalog-index-rebuild-report.v00.01",
        "catalog_schema_version": CATALOG_SCHEMA_VERSION,
        "article_schema_version": ARTICLE_SCHEMA_VERSION,
        "index_schema_version": INDEX_SCHEMA_VERSION,
        "catalog_record_glob": f"{CATALOG_RECORD_DIR}/*/*/*/article.json",
        "normal_lookup_surface": "index.json",
        "full_tree_traversal_allowed_only_for": "--rebuild-index",
        "network_refresh_required": False,
        "network_fetch_attempted": False,
        "records_scanned": len(rebuilt.get("articles", [])),
        "entries_emitted": len(rebuilt.get("articles", [])),
        "diagnostic_counts": counts,
        "error_count": error_count,
        "existing_index_matches_rebuild": existing_matches_rebuilt,
        "idempotent": second_pass_matches
        if second_pass_matches is not None
        else existing_matches_rebuilt,
        "wrote_index": str(write_index) if write_index else None,
    }


def run_rebuild(
    args: argparse.Namespace, existing_index: dict[str, Any]
) -> tuple[list[str], dict[str, Any], list[dict[str, Any]]]:
    rebuilt, diagnostics = rebuild_index_from_articles(args.catalog, existing_index)
    diagnostics.extend(compare_existing_to_rebuilt(existing_index, rebuilt))
    existing_matches_rebuilt = existing_index == rebuilt
    second_pass_matches: bool | None = None
    if args.write_index:
        write_json_atomic(args.write_index, rebuilt)
        try:
            written = load_json(args.write_index)
        except ValueError as exc:
            diagnostics.append(diagnostic("write_index_failed", str(exc)))
            written = {}
        second_rebuilt, second_diagnostics = rebuild_index_from_articles(args.catalog, written)
        second_pass_matches = written == second_rebuilt and not [
            d for d in second_diagnostics if d.get("severity") == "error"
        ]
        diagnostics.extend(second_diagnostics)
    elif args.check_index_idempotent:
        second_rebuilt, second_diagnostics = rebuild_index_from_articles(args.catalog, rebuilt)
        second_pass_matches = rebuilt == second_rebuilt and not [
            d for d in second_diagnostics if d.get("severity") == "error"
        ]
        diagnostics.extend(second_diagnostics)

    report = build_rebuild_report(
        rebuilt,
        diagnostics,
        existing_matches_rebuilt=existing_matches_rebuilt,
        second_pass_matches=second_pass_matches,
        write_index=args.write_index,
    )
    if args.write_index_report:
        write_json_atomic(args.write_index_report, report)
    if args.write_diagnostics:
        write_jsonl_atomic(args.write_diagnostics, diagnostics)

    errors = [
        f"{row.get('code')}: {row['message']}"
        for row in diagnostics
        if row.get("severity") == "error"
    ]
    if args.check_index_idempotent and report["idempotent"] is not True:
        errors.append("rebuilt index is not idempotent")
    return errors, report, diagnostics


def _load_rebuild_report(catalog_path: Path) -> dict[str, Any]:
    report_path = catalog_path.parent / "index-rebuild-report.json"
    if not report_path.exists():
        return {
            "path": str(report_path),
            "present": False,
            "idempotent": None,
            "existing_index_matches_rebuild": None,
            "network_fetch_attempted": None,
        }
    try:
        report = load_json(report_path)
    except ValueError as exc:
        return {"path": str(report_path), "present": True, "load_error": str(exc)}
    report["path"] = str(report_path)
    report["present"] = True
    return report


def build_catalog_readiness_artifacts(
    catalog_path: Path,
    index: dict[str, Any],
    selection: dict[str, Any],
    index_articles: dict[str, dict[str, Any]],
    *,
    rebuild_report: dict[str, Any] | None,
    report_title: str = DEFAULT_REPORT_TITLE,
) -> tuple[dict[str, Any], list[dict[str, Any]], str]:
    diagnostics: list[dict[str, Any]] = []
    selected_rows = selection.get("articles") if isinstance(selection.get("articles"), list) else []
    selected_refs = [
        row.get("article_ref")
        for row in selected_rows  # ty:ignore[not-iterable]
        if isinstance(row, dict) and isinstance(row.get("article_ref"), str)
    ]
    stored_rebuild_report = _load_rebuild_report(catalog_path)
    rebuild_evidence = dict(stored_rebuild_report)
    if rebuild_report:
        rebuild_evidence.update(rebuild_report)
        rebuild_evidence.setdefault("path", stored_rebuild_report.get("path"))
        rebuild_evidence.setdefault("present", stored_rebuild_report.get("present"))
    diagnostics.append(
        diagnostic(
            "index_readiness",
            "catalog index readiness evidence",
            severity="info",
            index_article_count=len(index.get("articles", []))
            if isinstance(index.get("articles"), list)
            else 0,
            index_idempotent=rebuild_evidence.get("idempotent"),
            existing_index_matches_rebuild=rebuild_evidence.get("existing_index_matches_rebuild"),
            network_fetch_attempted=rebuild_evidence.get("network_fetch_attempted"),
            rebuild_report_path=rebuild_evidence.get("path"),
        )
    )

    articles: list[dict[str, Any]] = []
    total_variants = 0
    captured_variants = 0
    primary_lightweight = 0
    pdf_fallback_refs: list[str] = []
    blocked_articles: list[dict[str, Any]] = []
    loader_outcome_counts: dict[str, int] = {}
    capture_status_counts: dict[str, int] = {}
    lightweight_formats = {"html", "markdown", "text", "ocr_text"}

    for article_ref, article_path, article, load_error in _selection_article_records(
        catalog_path, selection, index_articles
    ):
        if load_error or article is None:
            blocked_articles.append(
                {"article_ref": article_ref, "reason": load_error or "missing_article"}
            )
            diagnostics.append(
                diagnostic(
                    "article_blocked",
                    "article record could not be loaded",
                    article_ref=article_ref,
                    reason=load_error,
                )
            )
            continue
        entry = index_articles.get(article_ref, {})
        identity = article.get("identity") if isinstance(article.get("identity"), dict) else {}
        variants = (
            article.get("source_variants")
            if isinstance(article.get("source_variants"), list)
            else []
        )
        primary = next(
            (
                variant
                for variant in variants  # ty:ignore[not-iterable]
                if isinstance(variant, dict) and variant.get("is_primary") is True
            ),
            None,
        )
        fallback_pdfs = [
            variant
            for variant in variants  # ty:ignore[not-iterable]
            if isinstance(variant, dict)
            and variant.get("source_format") == "pdf"
            and variant.get("is_content_bearing") is True
            and variant.get("is_primary") is not True
        ]
        if fallback_pdfs:
            pdf_fallback_refs.append(article_ref)
        article_blockers: list[str] = []
        if not isinstance(primary, dict):
            article_blockers.append("missing_primary_variant")
        else:
            if primary.get("capture_status") != "captured":
                article_blockers.append("primary_not_captured")
            if primary.get("source_format") not in lightweight_formats:
                article_blockers.append("primary_not_lightweight")
            if primary.get("loader_outcome") != "loaded":
                article_blockers.append("primary_loader_not_loaded")
            else:
                primary_lightweight += 1
        for variant in variants:  # ty:ignore[not-iterable]
            if not isinstance(variant, dict):
                continue
            total_variants += 1
            capture_status = str(variant.get("capture_status"))
            loader_outcome = str(variant.get("loader_outcome"))
            capture_status_counts[capture_status] = capture_status_counts.get(capture_status, 0) + 1
            loader_outcome_counts[loader_outcome] = loader_outcome_counts.get(loader_outcome, 0) + 1
            if variant.get("capture_status") == "captured":
                captured_variants += 1
            diagnostics.append(
                diagnostic(
                    "source_variant_readiness",
                    "selected source variant readiness",
                    severity="info",
                    article_ref=article_ref,
                    source_provider=article.get("source_code"),
                    coarse_topic_code=article.get("coarse_topic_code"),
                    article_key=article.get("article_key"),
                    title=identity.get("title"),  # ty:ignore[unresolved-attribute]
                    variant_id=variant.get("variant_id"),
                    source_role=variant.get("source_role"),
                    source_format=variant.get("source_format"),
                    is_primary=variant.get("is_primary"),
                    is_content_bearing=variant.get("is_content_bearing"),
                    is_metadata_only=variant.get("is_metadata_only"),
                    capture_status=variant.get("capture_status"),
                    checksum=variant.get("sha256"),
                    byte_size=variant.get("byte_size"),
                    loader_outcome=variant.get("loader_outcome"),
                    fallback_reason=variant.get("conversion_hint"),
                    raw_text_embedded=variant.get("raw_text_embedded"),
                    raw_binary_embedded=variant.get("raw_binary_embedded"),
                )
            )
        if article_blockers:
            blocked_articles.append(
                {"article_ref": article_ref, "reason": ",".join(article_blockers)}
            )
        article_summary = {
            "article_ref": article_ref,
            "article_key": article.get("article_key"),
            "source_code": article.get("source_code"),
            "coarse_topic_code": article.get("coarse_topic_code"),
            "title": identity.get("title") or entry.get("title"),  # ty:ignore[unresolved-attribute]
            "primary_source_role": primary.get("source_role")
            if isinstance(primary, dict)
            else None,
            "primary_source_format": primary.get("source_format")
            if isinstance(primary, dict)
            else None,
            "primary_loader_outcome": primary.get("loader_outcome")
            if isinstance(primary, dict)
            else None,
            "captured_variant_count": sum(
                1
                for variant in variants  # ty:ignore[not-iterable]
                if isinstance(variant, dict) and variant.get("capture_status") == "captured"
            ),
            "variant_count": len([variant for variant in variants if isinstance(variant, dict)]),  # ty:ignore[not-iterable]
            "pdf_fallback_preserved": bool(fallback_pdfs),
            "blocked_before_s02": bool(article_blockers),
            "blocker_reasons": article_blockers,
            "expected_profile": article.get("expected_profile"),
            "article_path": str(article_path),
        }
        articles.append(article_summary)
        diagnostics.append(
            diagnostic(
                "article_readiness",
                "selected article readiness",
                severity="info",
                **article_summary,
            )
        )

    summary = {
        "schema_version": "article-corpus-run-summary.v00.01",
        "selection_id": selection.get("selection_id"),
        "catalog_schema_version": CATALOG_SCHEMA_VERSION,
        "article_schema_version": ARTICLE_SCHEMA_VERSION,
        "selection_path": str(
            catalog_path.parent.parent
            / "article_corpora"
            / str(selection.get("selection_id"))
            / "selection.json"
        ),
        "article_count": len(selected_refs),
        "variant_count": total_variants,
        "captured_variant_count": captured_variants,
        "capture_status_counts": capture_status_counts,
        "loader_outcome_counts": loader_outcome_counts,
        "primary_lightweight_ready_count": primary_lightweight,
        "pdf_fallback_article_refs": pdf_fallback_refs,
        "index": {
            "path": str(catalog_path.parent / "index.json"),
            "lookup_surface": "index.json",
            "full_tree_scan_attempted": False,
            "rebuild_report_path": rebuild_evidence.get("path"),
            "rebuild_report_present": rebuild_evidence.get("present"),
            "existing_index_matches_rebuild": rebuild_evidence.get(
                "existing_index_matches_rebuild"
            ),
            "idempotent": rebuild_evidence.get("idempotent"),
            "network_fetch_attempted": rebuild_evidence.get("network_fetch_attempted"),
        },
        "network": {
            "test_phase_must_not_fetch": True,
            "pipeline_phase_reads_catalog_only": True,
            "network_fetch_attempted_during_validation": False,
        },
        "readiness": {
            "blocked_article_count": len(blocked_articles),
            "blocked_articles": blocked_articles,
            "ready_for_s02_baseline": not blocked_articles,
        },
        "safety_flags": {
            "graph_import_allowed": False,
            "production_ladybugdb_write_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
        "articles": articles,
    }
    report = render_catalog_report(summary, diagnostics, report_title=report_title)
    return summary, diagnostics, report


def render_catalog_report(
    summary: dict[str, Any],
    diagnostics: list[dict[str, Any]],
    *,
    report_title: str = DEFAULT_REPORT_TITLE,
) -> str:
    articles = summary.get("articles") if isinstance(summary.get("articles"), list) else []
    lines = [
        f"# {report_title}",
        "",
        "## Overview",
        f"- Selection: `{summary.get('selection_id')}`",
        f"- Articles selected: {summary.get('article_count')}",
        f"- Source variants captured: {summary.get('captured_variant_count')}/{summary.get('variant_count')}",
        f"- S02 blockers: {summary.get('readiness', {}).get('blocked_article_count')}",
        f"- Ready for S02 parser/chunking baseline: {summary.get('readiness', {}).get('ready_for_s02_baseline')}",
        "- Network replay guarantee: validation uses catalog/index/loader artifacts only and does not fetch.",
        "",
        "## Index Evidence",
        f"- Lookup surface: `{summary.get('index', {}).get('lookup_surface')}`",
        f"- Full tree scan attempted during validation: {summary.get('index', {}).get('full_tree_scan_attempted')}",
        f"- Rebuild report: `{summary.get('index', {}).get('rebuild_report_path')}` (present={summary.get('index', {}).get('rebuild_report_present')})",
        f"- Existing index matches rebuild: {summary.get('index', {}).get('existing_index_matches_rebuild')}",
        f"- Idempotent rebuild evidence: {summary.get('index', {}).get('idempotent')}",
        f"- Network fetch attempted: {summary.get('index', {}).get('network_fetch_attempted')}",
        "",
        "## Article Readiness",
        "| Article | Provider | Topic | Primary | Captured Variants | PDF Fallback | Blocked Before S02 |",
        "|---|---:|---:|---|---:|---:|---:|",
    ]
    for article in articles:  # ty:ignore[not-iterable]
        lines.append(
            "| "
            f"{article.get('article_ref')} | {article.get('source_code')} | {article.get('coarse_topic_code')} | "
            f"{article.get('primary_source_role')} / {article.get('primary_source_format')} / {article.get('primary_loader_outcome')} | "
            f"{article.get('captured_variant_count')}/{article.get('variant_count')} | "
            f"{article.get('pdf_fallback_preserved')} | {article.get('blocked_before_s02')} |"
        )
    lines.extend(
        [
            "",
            "## Source Variant Diagnostics",
            "| Article | Role | Format | Primary | Capability | Capture | Checksum | Loader Outcome | Fallback Reason |",
            "|---|---|---|---:|---|---|---|---|---|",
        ]
    )
    for row in diagnostics:
        if row.get("code") != "source_variant_readiness":
            continue
        capability = (
            "metadata"
            if row.get("is_metadata_only")
            else "content"
            if row.get("is_content_bearing")
            else "auxiliary"
        )
        checksum = str(row.get("checksum") or "")[:12]
        lines.append(
            "| "
            f"{row.get('article_ref')} | {row.get('source_role')} | {row.get('source_format')} | {row.get('is_primary')} | "
            f"{capability} | {row.get('capture_status')} | {checksum} | {row.get('loader_outcome')} | {row.get('fallback_reason') or ''} |"
        )
    lines.extend(
        [
            "",
            "## Failure Modes",
            "- Filesystem dependency: missing catalog, index, selection, article records, loader events, or captured source files bubble as validator errors with non-zero exit; artifact writes are atomic temp-file-plus-rename writes.",
            "- Malformed JSON dependency: `load_json` rejects malformed or non-object JSON and the CLI exits non-zero before writing readiness success claims.",
            "- Index drift dependency: `--check-index-idempotent` rebuilds the projection in memory and fails if the existing index no longer matches an idempotent rebuild report.",
            "- Network dependency: S01 report generation has no network mode; tests and pipeline validation report `network_fetch_attempted=false` instead of refreshing implicitly.",
            "- Subprocess dependency: callers observe standard CLI exit codes; failed validation prints concrete diagnostics to stderr.",
            "",
            "## Load Profile",
            "- Expected load is five selected articles and their local variants; 10x load first saturates local filesystem reads and optional checksum hashing, not network or database resources.",
            "- Protection: normal validation resolves articles through `index.json`, full tree traversal is confined to explicit rebuild/idempotency checks, and output writes are bounded JSON/JSONL/Markdown files.",
            "- No pool sizing or rate limiting is needed because report generation has no async, network, API, or database runtime dimension.",
            "",
            "## Negative Tests",
            "- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_index_title_drift` covers index/article title drift.",
            "- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_selection_not_in_index` covers invalid selection references.",
            "- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_rebuild_rejects_duplicate_lookup_key` covers duplicate lookup/index rebuild failures.",
            "- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_rejects_unsafe_index_traversal` covers unsafe catalog-relative paths.",
            "- `tests/test_m025_article_catalog_verifier.py::test_m025_article_catalog_verifier_writes_catalog_readiness_outputs` covers final summary/report/diagnostics generation and planned S02 blocker reporting.",
            "",
            "## Observability Impact",
            "- `run-summary.json` records article, variant, loader, rebuild, network, blocker, and safety counts for S02 handoff.",
            "- `diagnostics.jsonl` records machine-readable index, article, and source-variant readiness rows without raw payloads or vectors.",
            "- `catalog-report.md` provides a human-readable handoff stating primary lightweight variants, preserved PDF fallbacks, idempotency evidence, and S02 blockers.",
            "",
        ]
    )
    return "\n".join(lines)


def validate(args: argparse.Namespace) -> tuple[list[str], dict[str, Any] | None]:
    errors: list[str] = []
    try:
        catalog = load_json(args.catalog)
        index = load_json(args.index)
        selection = load_json(args.selection)
    except ValueError as exc:
        return [str(exc)], None

    errors.extend(validate_catalog(args.catalog, catalog))
    index_errors, index_articles = validate_index(
        args.catalog,
        index,
        require_index=args.require_index or args.check_index_lookup_only,
        check_index_titles=args.check_index_titles,
        check_safe_traversal=args.check_safe_traversal,
        check_duplicate_lookups=args.check_duplicate_lookups,
    )
    errors.extend(index_errors)
    errors.extend(
        validate_selection(
            selection,
            index_articles,
            expected_selection_id=args.expected_selection_id,
            require_selection_titles=args.require_selection_titles,
            allow_index_superset=args.allow_index_superset,
        )
    )
    if args.require_captured_sources:
        errors.extend(
            check_captured_sources(
                args.catalog, selection, index_articles, check_checksums=args.check_checksums
            )
        )
    if args.require_loader_events:
        errors.extend(
            check_loader_events(
                args.catalog, selection, index_articles, check_redaction=args.check_redaction
            )
        )
    report = None
    if args.rebuild_index:
        rebuild_errors, report, _diagnostics = run_rebuild(args, index)
        errors.extend(rebuild_errors)
    elif args.check_index_idempotent:
        rebuilt, diagnostics = rebuild_index_from_articles(args.catalog, index)
        second_rebuilt, second_diagnostics = rebuild_index_from_articles(args.catalog, rebuilt)
        diagnostics.extend(second_diagnostics)
        report = build_rebuild_report(
            rebuilt,
            diagnostics,
            existing_matches_rebuilt=index == rebuilt,
            second_pass_matches=rebuilt == second_rebuilt
            and not [d for d in second_diagnostics if d.get("severity") == "error"],
            write_index=None,
        )
        errors.extend(
            f"{row.get('code')}: {row['message']}"
            for row in diagnostics
            if row.get("severity") == "error"
        )
        if report["idempotent"] is not True:
            errors.append("rebuilt index is not idempotent")
    if args.check_index_lookup_only:
        errors.extend(check_static_lookup_policy(Path(__file__)))
    if not errors and (
        args.write_summary
        or args.write_report
        or (args.write_diagnostics and not args.rebuild_index)
    ):
        summary, readiness_diagnostics, markdown_report = build_catalog_readiness_artifacts(
            args.catalog,
            index,
            selection,
            index_articles,
            rebuild_report=report,
            report_title=args.report_title,
        )
        if args.write_summary:
            write_json_atomic(args.write_summary, summary)
        if args.write_diagnostics and not args.rebuild_index:
            write_jsonl_atomic(args.write_diagnostics, readiness_diagnostics)
        if args.write_report:
            args.write_report.parent.mkdir(parents=True, exist_ok=True)
            args.write_report.write_text(markdown_report, encoding="utf-8")
    return errors, report


def check_static_lookup_policy(script_path: Path) -> list[str]:
    """AST-aware guard: broad article scans may appear only in rebuild functions."""
    errors: list[str] = []
    tree = ast.parse(script_path.read_text(encoding="utf-8"))
    parents: dict[ast.AST, ast.AST] = {}
    for parent in ast.walk(tree):
        for child in ast.iter_child_nodes(parent):
            parents[child] = parent

    def enclosing_function(node: ast.AST) -> str | None:
        current = node
        while current in parents:
            current = parents[current]
            if isinstance(current, (ast.FunctionDef, ast.AsyncFunctionDef)):
                return current.name
        return None

    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if not isinstance(func, ast.Attribute) or func.attr not in {"glob", "rglob"}:
            continue
        literal_args = [
            arg.value
            for arg in node.args
            if isinstance(arg, ast.Constant) and isinstance(arg.value, str)
        ]
        if any("article.json" in value for value in literal_args):
            owner = enclosing_function(node)
            if owner not in REBUILD_TRAVERSAL_FUNCTIONS:
                errors.append(
                    f"broad catalog scan for article.json is outside rebuild function: {owner or '<module>'}"
                )
    return errors


def parse_args(
    argv: list[str],
    *,
    default_expected_selection_id: str | None = EXPECTED_SELECTION_ID,
    default_report_title: str = "M025 S01 Catalog Readiness Report",
) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument(
        "--validate-only",
        action="store_true",
        help="Validate existing local artifacts without fetching or rebuilding.",
    )
    parser.add_argument(
        "--rebuild-index",
        action="store_true",
        help="Explicitly rebuild index.json from canonical local article records.",
    )
    parser.add_argument(
        "--write-index", type=Path, help="Atomically write the rebuilt index to this path."
    )
    parser.add_argument(
        "--write-index-report",
        type=Path,
        help="Atomically write the rebuild report JSON to this path.",
    )
    parser.add_argument(
        "--write-summary",
        type=Path,
        help="Atomically write the catalog readiness run summary JSON to this path.",
    )
    parser.add_argument(
        "--write-diagnostics",
        type=Path,
        help="Atomically write readiness or rebuild diagnostics as JSONL to this path.",
    )
    parser.add_argument(
        "--write-report",
        type=Path,
        help="Write the human-readable catalog readiness report Markdown to this path.",
    )
    parser.add_argument(
        "--require-index",
        action="store_true",
        help="Require index policy fields for index-only CLI lookup.",
    )
    parser.add_argument(
        "--check-index-idempotent",
        action="store_true",
        help="Require rebuilt index projection to be idempotent.",
    )
    parser.add_argument(
        "--check-index-titles",
        action="store_true",
        help="Require every index title to mirror article identity.title.",
    )
    parser.add_argument(
        "--check-safe-traversal",
        action="store_true",
        help="Reject catalog paths that escape the catalog root or do not match canonical record layout.",
    )
    parser.add_argument(
        "--check-duplicate-lookups",
        action="store_true",
        help="Reject duplicate article lookup keys.",
    )
    parser.add_argument(
        "--require-captured-sources",
        action="store_true",
        help="Require selected source variants to have local captured files.",
    )
    parser.add_argument(
        "--check-checksums",
        action="store_true",
        help="Verify captured source byte sizes and sha256 checksums.",
    )
    parser.add_argument(
        "--require-loader-events",
        action="store_true",
        help="Require local loader summaries/events for selected source variants.",
    )
    parser.add_argument(
        "--check-redaction",
        action="store_true",
        help="Reject raw payload or vector/secret-like fields in loader artifacts.",
    )
    parser.add_argument(
        "--check-index-lookup-only",
        action="store_true",
        help="Run the AST-aware guard that normal lookup must not scan article records.",
    )
    parser.add_argument(
        "--expected-selection-id",
        default=default_expected_selection_id,
        help="Optional strict selection_id to require; omit in generic callers to accept any corpus selection.",
    )
    parser.add_argument(
        "--require-selection-titles",
        action="store_true",
        help="Require selected article rows to carry titles matching index.json.",
    )
    parser.add_argument(
        "--allow-index-superset",
        action="store_true",
        help="Allow index.json to contain catalog rows outside the current corpus selection.",
    )
    parser.add_argument(
        "--report-title", default=default_report_title, help="Markdown title for readiness reports."
    )
    args = parser.parse_args(argv)
    if args.write_index and not args.rebuild_index:
        parser.error("--write-index requires --rebuild-index")
    if args.write_index_report and not args.rebuild_index:
        parser.error("--write-index-report requires --rebuild-index")
    if not (
        args.validate_only
        or args.rebuild_index
        or args.require_captured_sources
        or args.require_loader_events
        or args.check_redaction
        or args.check_index_lookup_only
    ):
        parser.error(
            "choose --validate-only, --rebuild-index, or a concrete check mode; no network fetch mode exists"
        )
    return args


def main(
    argv: list[str],
    *,
    default_expected_selection_id: str | None = EXPECTED_SELECTION_ID,
    label: str = "M025 article catalog",
    default_report_title: str = "M025 S01 Catalog Readiness Report",
) -> int:
    args = parse_args(
        argv[1:],
        default_expected_selection_id=default_expected_selection_id,
        default_report_title=default_report_title,
    )
    errors, report = validate(args)
    if errors:
        sys.stderr.write(f"{label} validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    if args.rebuild_index:
        sys.stdout.write(
            f"{label} index rebuild passed: "
            f"{report['entries_emitted'] if report else 0} entries, "
            f"idempotent={report['idempotent'] if report else False}, "
            "normal lookup remains index-only, no network fetch attempted.\n"
        )
    else:
        sys.stdout.write(
            f"{label} validation passed: local scaffold, initial index, schemas, "
            "selection, titles, and fail-closed safety flags are consistent.\n"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
