#!/usr/bin/env python3
"""Validate the M025 reusable article catalog scaffold.

The verifier is intentionally local-only: it reads the catalog, initial index,
selection, schema files, and article manifests already present on disk. It never
fetches network sources or rebuilds the index from a tree scan; later S01 tasks
own explicit index rebuild/capture behavior.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

CATALOG_SCHEMA_VERSION = "article-catalog.v00.01"
ARTICLE_SCHEMA_VERSION = "article.v00.01"
INDEX_SCHEMA_VERSION = "article-catalog-index.v00.01"
SELECTION_SCHEMA_VERSION = "article-corpus-selection.v00.01"
EXPECTED_SELECTION_ID = "m025-rlm-dspy-pageindex-smoke-v1"

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


def article_manifest_path(catalog_path: Path, article_path: str) -> Path:
    return catalog_path.parent / article_path


def validate_catalog(catalog_path: Path, catalog: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    require_equal(errors, "catalog.schema_version", catalog.get("schema_version"), CATALOG_SCHEMA_VERSION)
    require_equal(errors, "catalog.article_schema_version", catalog.get("article_schema_version"), ARTICLE_SCHEMA_VERSION)
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
        require_equal(errors, "catalog.index.schema_version", index_policy.get("schema_version"), INDEX_SCHEMA_VERSION)
        require_equal(errors, "catalog.index.path", index_policy.get("path"), "index.json")
        require_equal(errors, "catalog.index.cli_must_use_index", index_policy.get("cli_must_use_index"), True)
        require_equal(errors, "catalog.index.full_tree_scan_allowed", index_policy.get("full_tree_scan_allowed"), False)
        lookup_keys = set(index_policy.get("lookup_keys", [])) if isinstance(index_policy.get("lookup_keys"), list) else set()
        missing_lookup = sorted({"article_key", "citation_key", "canonical_url", "source_code", "coarse_topic_code", "title"} - lookup_keys)
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


def validate_index(
    catalog_path: Path,
    index: dict[str, Any],
    *,
    require_index: bool,
    check_index_titles: bool,
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    errors: list[str] = []
    if require_index:
        require_equal(errors, "index.schema_version", index.get("schema_version"), INDEX_SCHEMA_VERSION)
        require_equal(errors, "index.catalog_schema_version", index.get("catalog_schema_version"), CATALOG_SCHEMA_VERSION)
        require_equal(errors, "index.article_schema_version", index.get("article_schema_version"), ARTICLE_SCHEMA_VERSION)
        lookup_policy = index.get("lookup_policy")
        if not isinstance(lookup_policy, dict):
            errors.append("index.lookup_policy must be an object")
        else:
            require_equal(errors, "index.lookup_policy.cli_must_use_index", lookup_policy.get("cli_must_use_index"), True)
            require_equal(errors, "index.lookup_policy.full_tree_scan_allowed", lookup_policy.get("full_tree_scan_allowed"), False)
    articles = index.get("articles")
    if not isinstance(articles, list) or not articles:
        return errors + ["index.articles must be a non-empty list"], {}

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
        by_ref[article_ref] = entry
        if not isinstance(article_path, str) or not article_path.endswith("/article.json"):
            errors.append(f"{article_ref} article_path must end with /article.json")
            continue
        manifest_path = article_manifest_path(catalog_path, article_path)
        try:
            article = load_json(manifest_path)
        except ValueError as exc:
            errors.append(str(exc))
            continue
        require_equal(errors, f"{article_ref}.schema_version", article.get("schema_version"), ARTICLE_SCHEMA_VERSION)
        require_equal(errors, f"{article_ref}.catalog_path", article.get("catalog_path"), article_ref)
        require_equal(errors, f"{article_ref}.source_code", article.get("source_code"), entry.get("source_code"))
        require_equal(errors, f"{article_ref}.coarse_topic_code", article.get("coarse_topic_code"), entry.get("coarse_topic_code"))
        require_equal(errors, f"{article_ref}.article_key", article.get("article_key"), entry.get("article_key"))
        if check_index_titles:
            require_equal(
                errors,
                f"{article_ref}.index_title",
                entry.get("title"),
                article.get("identity", {}).get("title") if isinstance(article.get("identity"), dict) else None,
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
                    require_equal(errors, f"index.indexes.{lookup_name}[{value!r}]", lookup.get(value), article_ref)
        for group_name, lookup_key in (("by_source_code", "source_code"), ("by_coarse_topic_code", "coarse_topic_code")):
            grouped = indexes.get(group_name)
            if not isinstance(grouped, dict):
                errors.append(f"index.indexes.{group_name} must be an object")
                continue
            for article_ref, entry in by_ref.items():
                group_value = entry.get(lookup_key)
                refs = grouped.get(group_value) if isinstance(group_value, str) else None
                if not isinstance(refs, list) or article_ref not in refs:
                    errors.append(f"index.indexes.{group_name}[{group_value!r}] missing {article_ref}")
    check_safety_flags(errors, "index", index)
    return errors, by_ref


def validate_selection(selection: dict[str, Any], index_articles: dict[str, dict[str, Any]]) -> list[str]:
    errors: list[str] = []
    require_equal(errors, "selection.schema_version", selection.get("schema_version"), SELECTION_SCHEMA_VERSION)
    require_equal(errors, "selection.selection_id", selection.get("selection_id"), EXPECTED_SELECTION_ID)
    require_equal(errors, "selection.catalog_schema_version", selection.get("catalog_schema_version"), CATALOG_SCHEMA_VERSION)
    require_equal(errors, "selection.article_schema_version", selection.get("article_schema_version"), ARTICLE_SCHEMA_VERSION)
    network_policy = selection.get("network_policy")
    if not isinstance(network_policy, dict):
        errors.append("selection.network_policy must be an object")
    else:
        require_equal(errors, "selection.network_policy.test_phase_must_not_fetch", network_policy.get("test_phase_must_not_fetch"), True)
        require_equal(errors, "selection.network_policy.pipeline_phase_reads_catalog_only", network_policy.get("pipeline_phase_reads_catalog_only"), True)
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
                errors.append(f"selection.articles[{position}].article_ref must be a non-empty string")
                continue
            selection_refs.add(article_ref)
            if article_ref not in index_articles:
                errors.append(f"selection article_ref not present in index: {article_ref}")
            index_source = index_articles.get(article_ref, {}).get("source_code")
            if row.get("source_code") != index_source:
                errors.append(f"selection {article_ref} source_code does not match index")
        missing_from_selection = sorted(set(index_articles) - selection_refs)
        if missing_from_selection:
            errors.append(f"index articles missing from selection: {', '.join(missing_from_selection)}")
    check_safety_flags(errors, "selection", selection)
    return errors


def validate(args: argparse.Namespace) -> list[str]:
    errors: list[str] = []
    try:
        catalog = load_json(args.catalog)
        index = load_json(args.index)
        selection = load_json(args.selection)
    except ValueError as exc:
        return [str(exc)]

    errors.extend(validate_catalog(args.catalog, catalog))
    index_errors, index_articles = validate_index(
        args.catalog,
        index,
        require_index=args.require_index,
        check_index_titles=args.check_index_titles,
    )
    errors.extend(index_errors)
    errors.extend(validate_selection(selection, index_articles))
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--validate-only", action="store_true", help="Validate existing local artifacts without fetching or rebuilding.")
    parser.add_argument("--require-index", action="store_true", help="Require index policy fields for index-only CLI lookup.")
    parser.add_argument("--check-index-titles", action="store_true", help="Require every index title to mirror article identity.title.")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv[1:])
    if not args.validate_only:
        sys.stderr.write("ERROR: only --validate-only is supported for this scaffold verifier; no network fetch or rebuild is performed.\n")
        return 2
    errors = validate(args)
    if errors:
        sys.stderr.write("M025 article catalog validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    sys.stdout.write(
        "M025 article catalog validation passed: local scaffold, initial index, schemas, "
        "selection, titles, and fail-closed safety flags are consistent.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
