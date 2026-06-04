#!/usr/bin/env python3
"""Build the M031 catalog-backed replay selection contract.

This command is intentionally local-only and metadata-only. It reads the bounded
M029/S01 selection, resolves catalog-backed rows only through the article catalog
index, loads article records only via indexed ``article_path`` values, and emits
a normalized ``articles`` contract for downstream acquisition and loader replay.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Mapping

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S02"
SELECTION_ID = "m031-catalog-backed-replay-v1"
SCHEMA_VERSION = "m031-catalog-backed-replay-selection.v1"
SOURCE_SELECTION_SCHEMA_VERSION = "article-corpus-selection.v00.02"

CATALOG_BACKED_STATUS = "already_cataloged"
TYPED_BLOCKER_STATUS = "typed_catalog_blocker"
ALLOWED_REF_STATUSES = {CATALOG_BACKED_STATUS, TYPED_BLOCKER_STATUS}

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_only_selection": True,
    "network_fetch_attempted": False,
    "source_acquisition_completed": False,
    "raw_article_text_embedded": False,
    "raw_article_html_embedded": False,
    "raw_pdf_bytes_embedded": False,
    "binary_payload_embedded": False,
    "base64_payload_embedded": False,
    "parser_ready_claimed": False,
    "chunk_ready_claimed": False,
    "kg_readiness_claimed": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
}

UNSAFE_TRUE_KEYS = {
    "source_acquired_now",
    "source_acquisition_completed",
    "raw_article_text_embedded",
    "raw_article_html_embedded",
    "raw_pdf_bytes_embedded",
    "binary_payload_embedded",
    "base64_payload_embedded",
    "metadata_manifests_embed_raw_text",
    "metadata_manifests_embed_raw_binary",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
    "raw_text_embedded_in_metadata",
    "raw_binary_embedded_in_metadata",
    "parser_ready_claimed",
    "parser_readiness_claimed",
    "parser_ready",
    "chunk_ready_claimed",
    "chunk_readiness_claimed",
    "chunk_ready",
    "kg_readiness_claimed",
    "graph_ready_claimed",
    "graph_readiness_claimed",
    "graph_ready",
    "graph_import_allowed",
    "production_ladybugdb_write_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "graph_write_attempted",
    "production_persistence_attempted",
}

FORBIDDEN_OUTPUT_KEYS = {
    "text",
    "raw_text",
    "html",
    "raw_html",
    "pdf",
    "binary",
    "bytes",
    "base64",
    "payload",
    "content",
    "body",
}
FORBIDDEN_OUTPUT_SNIPPETS = ("<html", "</html", "%PDF-", "base64,")


class SelectionError(ValueError):
    """Typed validation error for deterministic CLI diagnostics."""

    def __init__(self, code: str, message: str, *, identity: str | None = None, article_ref: str | None = None):
        super().__init__(message)
        self.code = code
        self.identity = identity
        self.article_ref = article_ref


def load_json_object(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError as exc:
        raise SelectionError("malformed_json", f"malformed JSON at {path}: {exc}") from exc
    except OSError as exc:
        raise SelectionError("json_read_failed", f"failed to read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise SelectionError("malformed_json_object", f"expected JSON object at {path}")
    return payload


def atomic_write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    tmp_path = Path(tmp_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2, sort_keys=False)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


def safe_child_path(root: Path, rel_path: str, *, code: str = "unsafe_relative_path") -> Path:
    if not isinstance(rel_path, str) or not rel_path.strip():
        raise SelectionError(code, f"empty unsafe relative path: {rel_path!r}")
    if "://" in rel_path:
        raise SelectionError("url_not_allowed_as_local_path", f"URL cannot be used as a local path: {rel_path}")
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part in ("", ".") for part in normalized.parts):
        raise SelectionError(code, f"unsafe relative path: {rel_path}")
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise SelectionError(code, f"path escapes root: {rel_path}")
    return resolved


def relative_to(path: Path, root: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def normalized_identity_from_ref(row: Mapping[str, Any]) -> str:
    identity = row.get("normalized_identity") or row.get("identity_key")
    if not isinstance(identity, str) or not identity.strip():
        raise SelectionError("missing_normalized_identity", "requested ref is missing normalized_identity")
    return identity


def assert_no_duplicate_identities(refs: list[Mapping[str, Any]]) -> None:
    seen: set[str] = set()
    duplicates: list[str] = []
    for row in refs:
        identity = normalized_identity_from_ref(row)
        if identity in seen:
            duplicates.append(identity)
        seen.add(identity)
    if duplicates:
        raise SelectionError("duplicate_normalized_identity", f"duplicate normalized identities: {', '.join(sorted(duplicates))}")


def assert_fail_closed_flags(container: Mapping[str, Any], *, context: str, identity: str | None = None, article_ref: str | None = None) -> None:
    for key, value in container.items():
        if isinstance(value, Mapping):
            assert_fail_closed_flags(value, context=f"{context}.{key}", identity=identity, article_ref=article_ref)
        elif isinstance(value, list):
            for index, item in enumerate(value):
                if isinstance(item, Mapping):
                    assert_fail_closed_flags(item, context=f"{context}.{key}[{index}]", identity=identity, article_ref=article_ref)
        elif key in UNSAFE_TRUE_KEYS and value is True:
            raise SelectionError(
                "unsafe_true_safety_flag",
                f"unsafe true safety flag at {context}.{key}",
                identity=identity,
                article_ref=article_ref,
            )


def catalog_rows_by_lookup(index_payload: Mapping[str, Any]) -> tuple[dict[str, Mapping[str, Any]], dict[str, Mapping[str, Any]]]:
    rows = index_payload.get("articles")
    if not isinstance(rows, list):
        raise SelectionError("malformed_index_articles", "catalog index must contain an articles list")
    by_ref: dict[str, Mapping[str, Any]] = {}
    by_lookup: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, Mapping):
            raise SelectionError("malformed_index_row", "catalog index articles must be objects")
        article_ref = row.get("article_ref")
        if not isinstance(article_ref, str) or not article_ref.strip():
            raise SelectionError("malformed_index_article_ref", "catalog index row is missing article_ref")
        if article_ref in by_ref:
            raise SelectionError("duplicate_index_article_ref", f"duplicate article_ref in index: {article_ref}")
        by_ref[article_ref] = row
        lookup_values = [
            article_ref,
            row.get("normalized_identity"),
            row.get("canonical_url"),
            row.get("seed_url"),
            row.get("article_key"),
            row.get("title"),
        ]
        for value in lookup_values:
            if isinstance(value, str) and value.strip():
                by_lookup.setdefault(value, row)
    return by_ref, by_lookup


def resolve_index_row(ref_row: Mapping[str, Any], by_ref: Mapping[str, Mapping[str, Any]], by_lookup: Mapping[str, Mapping[str, Any]]) -> Mapping[str, Any]:
    candidates = [
        ref_row.get("catalog_ref"),
        ref_row.get("normalized_identity"),
        ref_row.get("url"),
        ref_row.get("known_pdf_url"),
        ref_row.get("known_title"),
    ]
    identity = normalized_identity_from_ref(ref_row)
    if identity.startswith("arxiv:"):
        candidates.append(identity.removeprefix("arxiv:"))
    if identity.startswith("stanford:cs224n:"):
        candidates.append(identity.removeprefix("stanford:cs224n:"))
    for candidate in candidates:
        if isinstance(candidate, str) and candidate in by_ref:
            return by_ref[candidate]
        if isinstance(candidate, str) and candidate in by_lookup:
            return by_lookup[candidate]
    raise SelectionError("missing_index_row", f"no catalog index row for {identity}", identity=identity)


def article_record_path(catalog_root: Path, index_row: Mapping[str, Any], *, identity: str, article_ref: str) -> Path:
    article_path = index_row.get("article_path")
    if not isinstance(article_path, str):
        raise SelectionError("missing_index_article_path", "index row missing article_path", identity=identity, article_ref=article_ref)
    try:
        return safe_child_path(catalog_root, article_path, code="unsafe_index_article_path")
    except SelectionError as exc:
        exc.identity = identity
        exc.article_ref = article_ref
        raise


def normalized_variant(article_root: Path, variant: Mapping[str, Any], *, identity: str, article_ref: str) -> dict[str, Any]:
    assert_fail_closed_flags(variant, context="source_variant", identity=identity, article_ref=article_ref)
    path_value = variant.get("path")
    local_path_value = variant.get("local_path") if isinstance(variant.get("local_path"), str) else path_value
    safe_path: str | None = None
    catalog_relative_path: str | None = None
    source_path_status = "path_absent"
    if path_value is not None:
        if not isinstance(path_value, str):
            raise SelectionError("malformed_source_variant_path", "source variant path must be string or null", identity=identity, article_ref=article_ref)
        source_path = safe_child_path(article_root, path_value, code="unsafe_source_variant_path")
        safe_path = source_path.relative_to(article_root.resolve()).as_posix()
        catalog_relative_path = safe_path
        source_path_status = "safe_local_path"
    elif local_path_value is not None:
        if not isinstance(local_path_value, str):
            raise SelectionError("malformed_source_variant_local_path", "source variant local_path must be string or null", identity=identity, article_ref=article_ref)
        source_path = safe_child_path(article_root, local_path_value, code="unsafe_source_variant_path")
        safe_path = source_path.relative_to(article_root.resolve()).as_posix()
        catalog_relative_path = safe_path
        source_path_status = "safe_local_path"

    return {
        "variant_id": variant.get("variant_id") if isinstance(variant.get("variant_id"), str) else None,
        "source_role": variant.get("source_role") if isinstance(variant.get("source_role"), str) else None,
        "source_format": variant.get("source_format") if isinstance(variant.get("source_format"), str) else None,
        "source_origin": variant.get("source_origin") if isinstance(variant.get("source_origin"), str) else None,
        "is_primary": bool(variant.get("is_primary")) if isinstance(variant.get("is_primary"), bool) else False,
        "is_content_bearing": bool(variant.get("is_content_bearing")) if isinstance(variant.get("is_content_bearing"), bool) else False,
        "is_metadata_only": bool(variant.get("is_metadata_only")) if isinstance(variant.get("is_metadata_only"), bool) else False,
        "path": safe_path,
        "local_path": safe_path,
        "catalog_relative_path": catalog_relative_path,
        "source_path_status": source_path_status,
        "url": variant.get("url") if isinstance(variant.get("url"), str) else None,
        "media_type": variant.get("media_type") if isinstance(variant.get("media_type"), str) else None,
        "sha256": variant.get("sha256") if isinstance(variant.get("sha256"), str) else None,
        "byte_size": variant.get("byte_size") if isinstance(variant.get("byte_size"), int) else None,
        "capture_status": variant.get("capture_status") if isinstance(variant.get("capture_status"), str) else None,
        "loader_outcome": variant.get("loader_outcome") if isinstance(variant.get("loader_outcome"), str) else None,
        "requires_conversion": bool(variant.get("requires_conversion")) if isinstance(variant.get("requires_conversion"), bool) else False,
        "conversion_hint": variant.get("conversion_hint") if isinstance(variant.get("conversion_hint"), str) else None,
        "network_fetch_attempted": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "parser_ready_claimed": False,
        "chunk_ready_claimed": False,
        "graph_ready_claimed": False,
    }


def normalized_article(
    *,
    ref_row: Mapping[str, Any],
    index_row: Mapping[str, Any],
    article_payload: Mapping[str, Any],
    article_path: Path,
    catalog_root: Path,
) -> dict[str, Any]:
    identity = normalized_identity_from_ref(ref_row)
    article_ref = index_row.get("article_ref")
    if not isinstance(article_ref, str):
        raise SelectionError("missing_index_article_ref", "index row missing article_ref", identity=identity)
    assert_fail_closed_flags(ref_row, context="requested_ref", identity=identity, article_ref=article_ref)
    assert_fail_closed_flags(article_payload, context="article_record", identity=identity, article_ref=article_ref)
    variants = article_payload.get("source_variants")
    if not isinstance(variants, list):
        raise SelectionError("malformed_article_source_variants", "article source_variants must be a list", identity=identity, article_ref=article_ref)
    article_root = article_path.parent
    normalized_variants = [
        normalized_variant(article_root, variant, identity=identity, article_ref=article_ref)
        for variant in variants
        if isinstance(variant, Mapping)
    ]
    if len(normalized_variants) != len(variants):
        raise SelectionError("malformed_article_source_variant", "source_variants entries must be objects", identity=identity, article_ref=article_ref)
    if not normalized_variants:
        raise SelectionError("empty_article_source_variants", "catalog-backed article has no source variants", identity=identity, article_ref=article_ref)
    identity_obj = article_payload.get("identity") if isinstance(article_payload.get("identity"), Mapping) else {}
    return {
        "identity": identity,
        "requested_ref_id": ref_row.get("ref_id") if isinstance(ref_row.get("ref_id"), str) else None,
        "requested_url": ref_row.get("url") if isinstance(ref_row.get("url"), str) else None,
        "catalog_resolution": "catalog_backed_article_json",
        "article_ref": article_ref,
        "article_key": index_row.get("article_key") if isinstance(index_row.get("article_key"), str) else article_payload.get("article_key"),
        "article_path": relative_to(article_path, catalog_root),
        "article_json_schema_version": article_payload.get("schema_version") if isinstance(article_payload.get("schema_version"), str) else None,
        "source_code": article_payload.get("source_code") if isinstance(article_payload.get("source_code"), str) else index_row.get("source_code"),
        "coarse_topic_code": article_payload.get("coarse_topic_code") if isinstance(article_payload.get("coarse_topic_code"), str) else index_row.get("coarse_topic_code"),
        "title": identity_obj.get("title") if isinstance(identity_obj.get("title"), str) else index_row.get("title"),
        "canonical_url": identity_obj.get("canonical_url") if isinstance(identity_obj.get("canonical_url"), str) else index_row.get("canonical_url"),
        "primary_source_role": index_row.get("primary_source_role") if isinstance(index_row.get("primary_source_role"), str) else None,
        "source_variants": normalized_variants,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def requested_ref_contract(row: Mapping[str, Any]) -> dict[str, Any]:
    identity = normalized_identity_from_ref(row)
    status = row.get("catalog_status")
    if status not in ALLOWED_REF_STATUSES:
        raise SelectionError("unknown_ref_status", f"unknown catalog_status for {identity}: {status!r}", identity=identity)
    assert_fail_closed_flags(row, context="requested_ref", identity=identity)
    typed_blocker = row.get("typed_blocker") if isinstance(row.get("typed_blocker"), Mapping) else None
    return {
        "ref_id": row.get("ref_id") if isinstance(row.get("ref_id"), str) else None,
        "identity": identity,
        "url": row.get("url") if isinstance(row.get("url"), str) else None,
        "source_kind": row.get("source_kind") if isinstance(row.get("source_kind"), str) else None,
        "catalog_status": status,
        "catalog_resolution": "typed_catalog_blocker" if status == TYPED_BLOCKER_STATUS else "catalog_index_lookup_required",
        "known_title": row.get("known_title") if isinstance(row.get("known_title"), str) else None,
        "catalog_ref": row.get("catalog_ref") if isinstance(row.get("catalog_ref"), str) else None,
        "typed_blocker_code": typed_blocker.get("code") if typed_blocker and isinstance(typed_blocker.get("code"), str) else None,
        "next_pipeline_action": row.get("next_pipeline_action") if isinstance(row.get("next_pipeline_action"), str) else None,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def blocker_contract(row: Mapping[str, Any]) -> dict[str, Any]:
    identity = normalized_identity_from_ref(row)
    typed_blocker = row.get("typed_blocker")
    if not isinstance(typed_blocker, Mapping):
        raise SelectionError("missing_typed_blocker", f"typed blocker ref lacks typed_blocker object: {identity}", identity=identity)
    assert_fail_closed_flags(row, context="typed_blocker_ref", identity=identity)
    return {
        "identity": identity,
        "requested_ref_id": row.get("ref_id") if isinstance(row.get("ref_id"), str) else None,
        "requested_url": row.get("url") if isinstance(row.get("url"), str) else None,
        "catalog_resolution": "typed_catalog_blocker",
        "blocker_code": typed_blocker.get("code") if isinstance(typed_blocker.get("code"), str) else "typed_catalog_blocker",
        "blocker_status": typed_blocker.get("status") if isinstance(typed_blocker.get("status"), str) else "blocked",
        "evidence": typed_blocker.get("evidence") if isinstance(typed_blocker.get("evidence"), str) else None,
        "article_ref": None,
        "article_path": None,
        "source_role": row.get("source_kind") if isinstance(row.get("source_kind"), str) else None,
        "safe_local_paths": [],
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def validate_output_metadata_only(payload: Any, *, path: str = "$") -> None:
    if isinstance(payload, Mapping):
        for key, value in payload.items():
            if key in FORBIDDEN_OUTPUT_KEYS:
                raise SelectionError("raw_payload_output_key", f"forbidden raw-payload output key at {path}.{key}")
            validate_output_metadata_only(value, path=f"{path}.{key}")
    elif isinstance(payload, list):
        for index, item in enumerate(payload):
            validate_output_metadata_only(item, path=f"{path}[{index}]")
    elif isinstance(payload, str):
        lowered = payload.lower()
        if any(snippet.lower() in lowered for snippet in FORBIDDEN_OUTPUT_SNIPPETS):
            raise SelectionError("raw_payload_output_snippet", f"forbidden raw-payload snippet at {path}")


def build_selection(source_selection_path: Path, catalog_path: Path, index_path: Path) -> dict[str, Any]:
    source_selection = load_json_object(source_selection_path)
    catalog = load_json_object(catalog_path)
    index = load_json_object(index_path)
    catalog_root = catalog_path.parent

    refs = source_selection.get("refs")
    if not isinstance(refs, list):
        raise SelectionError("malformed_source_refs", "source selection must contain refs list")
    if not refs:
        raise SelectionError("empty_refs", "source selection refs list is empty")
    if any(not isinstance(row, Mapping) for row in refs):
        raise SelectionError("malformed_source_ref", "source selection refs must be objects")
    typed_refs = [row for row in refs if isinstance(row, Mapping) and row.get("catalog_status") == TYPED_BLOCKER_STATUS]
    catalog_refs = [row for row in refs if isinstance(row, Mapping) and row.get("catalog_status") == CATALOG_BACKED_STATUS]
    for row in refs:
        requested_ref_contract(row)  # validates status and fail-closed flags
    assert_no_duplicate_identities([row for row in refs if isinstance(row, Mapping)])
    assert_fail_closed_flags(source_selection.get("safety_flags", {}), context="source_selection.safety_flags")
    assert_fail_closed_flags(catalog.get("safety_flags", {}), context="catalog.safety_flags")

    by_ref, by_lookup = catalog_rows_by_lookup(index)
    articles: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    for row in catalog_refs:
        identity = normalized_identity_from_ref(row)
        index_row = resolve_index_row(row, by_ref, by_lookup)
        article_ref = index_row["article_ref"]
        path = article_record_path(catalog_root, index_row, identity=identity, article_ref=article_ref)
        article_payload = load_json_object(path)
        article = normalized_article(
            ref_row=row,
            index_row=index_row,
            article_payload=article_payload,
            article_path=path,
            catalog_root=catalog_root,
        )
        articles.append(article)
        diagnostics.append(
            {
                "code": "catalog_backed_article_json_resolved",
                "identity": identity,
                "article_ref": article_ref,
                "article_path": article["article_path"],
                "source_role": article["primary_source_role"],
                "safe_local_paths": [variant["local_path"] for variant in article["source_variants"] if variant.get("local_path")],
                "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
            }
        )

    blockers = [blocker_contract(row) for row in typed_refs]
    for blocker in blockers:
        diagnostics.append(
            {
                "code": blocker["blocker_code"],
                "identity": blocker["identity"],
                "article_ref": None,
                "article_path": None,
                "source_role": blocker["source_role"],
                "safe_local_paths": [],
                "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
            }
        )

    counts = {
        "requested_ref_count": len(refs),
        "catalog_backed_count": len(articles),
        "typed_catalog_blocker_count": len(blockers),
        "silent_missing_count": 0,
        "source_variant_count": sum(len(article["source_variants"]) for article in articles),
    }
    payload: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "selection_id": SELECTION_ID,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_selection_schema_version": source_selection.get("schema_version"),
        "catalog_schema_version": catalog.get("schema_version"),
        "index_schema_version": index.get("schema_version"),
        "provenance": {
            "source_selection_path": source_selection_path.as_posix(),
            "source_selection_id": source_selection.get("selection_id"),
            "catalog_path": catalog_path.as_posix(),
            "index_path": index_path.as_posix(),
            "lookup_policy": {
                "catalog_index_only": True,
                "full_tree_scan_allowed": False,
                "network_fetch_allowed": False,
                "article_records_loaded_only_via_index_article_path": True,
            },
        },
        "counts": counts,
        "requested_refs": [requested_ref_contract(row) for row in refs],
        "articles": articles,
        "catalog_blockers": blockers,
        "diagnostics": diagnostics,
        "safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }
    if counts["requested_ref_count"] != counts["catalog_backed_count"] + counts["typed_catalog_blocker_count"]:
        raise SelectionError("silent_missing_ref", "requested refs do not reconcile to catalog-backed rows plus typed blockers")
    validate_output_metadata_only(payload)
    return payload


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-selection", required=True, type=Path)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--validate-only", action="store_true", help="validate and print summary without writing output")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        payload = build_selection(args.source_selection, args.catalog, args.index)
        if not args.validate_only:
            atomic_write_json(args.output, payload)
        print(
            json.dumps(
                {
                    "status": "passed",
                    "selection_id": payload["selection_id"],
                    "counts": payload["counts"],
                    "output": None if args.validate_only else args.output.as_posix(),
                },
                sort_keys=True,
            )
        )
        return 0
    except SelectionError as exc:
        print(
            json.dumps(
                {
                    "status": "failed",
                    "code": exc.code,
                    "message": str(exc),
                    "identity": exc.identity,
                    "article_ref": exc.article_ref,
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
