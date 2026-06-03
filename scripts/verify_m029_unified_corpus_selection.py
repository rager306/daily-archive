#!/usr/bin/env python3
"""Verify the M029 unified corpus selection registry and catalog index surface."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path, PurePosixPath
from typing import Any

ARXIV_URL_RE = re.compile(r"^https://arxiv\.org/(abs|pdf|html)/(\d{4}\.\d{4,5})(v\d+)?(?:\.pdf)?/?$")
REQUIRED_SAFETY_FALSE = (
    "graph_import_allowed",
    "production_ladybugdb_write_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
)
INDEX_SCHEMA_VERSION = "article-catalog-index.v00.01"
INDEX_REBUILD_REPORT_SCHEMA_VERSION = "article-catalog-index-rebuild-report.v00.01"
CATALOG_RECORD_GLOB = "article_catalog/*/*/*/article.json"


def _read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ValueError(f"missing JSON file: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"expected JSON object in {path}")
    return payload


def _write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2, sort_keys=False) + "\n"
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            stream.write(text)
        os.replace(tmp_name, path)
    except Exception:
        try:
            os.unlink(tmp_name)
        except FileNotFoundError:
            pass
        raise


def _write_jsonl_atomic(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            stream.write(text)
        os.replace(tmp_name, path)
    except Exception:
        try:
            os.unlink(tmp_name)
        except FileNotFoundError:
            pass
        raise


def _canonical_url(url: str) -> str:
    clean = url.rstrip("/")
    match = ARXIV_URL_RE.match(clean)
    if match:
        _, arxiv_id, version = match.groups()
        return f"https://arxiv.org/abs/{arxiv_id}{version or ''}"
    return clean


def _is_safe_ref(value: str) -> bool:
    if not value or value.startswith("/") or "\\" in value:
        return False
    parts = PurePosixPath(value).parts
    return ".." not in parts and all(part not in {"", "."} for part in parts)


def _safe_ref_from_selection(article: dict[str, Any]) -> str:
    source_code = str(article.get("source_code") or "unknown")
    article_key = str(article.get("article_key") or "unknown")
    coarse_topic_code = str(article.get("coarse_topic_code") or "mixed-source")
    ref = f"{source_code}/{coarse_topic_code}/{article_key}"
    if not _is_safe_ref(ref):
        raise ValueError(f"cannot derive safe article_ref for selection article: {article}")
    return ref


def _article_path_from_ref(article_ref: str) -> str:
    if not _is_safe_ref(article_ref):
        raise ValueError(f"unsafe article_ref: {article_ref}")
    return f"article_catalog/{article_ref}/article.json"


def _variant_role(variant: dict[str, Any]) -> str | None:
    role = variant.get("source_role")
    return str(role) if role else None


def _entry_from_catalog_record(record: dict[str, Any], relative_path: Path) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    article_ref = str(record.get("catalog_path") or relative_path.parent.as_posix())
    if article_ref.startswith("article_catalog/"):
        article_ref = article_ref.removeprefix("article_catalog/")
    if not _is_safe_ref(article_ref):
        diagnostics.append({"level": "error", "code": "unsafe_catalog_article_ref", "article_ref": article_ref, "path": relative_path.as_posix()})
        return None, diagnostics

    identity = record.get("identity") if isinstance(record.get("identity"), dict) else {}
    source_strategy = record.get("source_strategy") if isinstance(record.get("source_strategy"), dict) else {}
    variants = record.get("source_variants") if isinstance(record.get("source_variants"), list) else []
    primary_variant_id = source_strategy.get("primary_source_variant_id")
    primary_variant = next((variant for variant in variants if variant.get("variant_id") == primary_variant_id), None)
    if primary_variant is None:
        primary_variant = next((variant for variant in variants if variant.get("is_primary") is True), None)
    primary_role = _variant_role(primary_variant or {})
    if primary_role is None:
        preferred = source_strategy.get("preferred_content_order")
        if isinstance(preferred, list) and preferred:
            primary_role = str(preferred[0])
    if primary_role is None:
        diagnostics.append({"level": "warning", "code": "missing_primary_source_role", "article_ref": article_ref})
        primary_role = "unknown"

    content_fallback_roles: list[str] = []
    metadata_roles: list[str] = []
    for variant in variants:
        role = _variant_role(variant)
        if not role:
            continue
        if variant.get("is_content_bearing") is True and role != primary_role and role not in content_fallback_roles:
            content_fallback_roles.append(role)
        if variant.get("is_metadata_only") is True and role not in metadata_roles:
            metadata_roles.append(role)

    title = identity.get("title") or record.get("title")
    canonical_url = identity.get("canonical_url") or identity.get("abs_url") or identity.get("url")
    if not title:
        diagnostics.append({"level": "warning", "code": "missing_catalog_title", "article_ref": article_ref})
    if not canonical_url:
        diagnostics.append({"level": "error", "code": "missing_catalog_canonical_url", "article_ref": article_ref})
        return None, diagnostics

    entry: dict[str, Any] = {
        "article_ref": article_ref,
        "article_key": str(record.get("article_key") or PurePosixPath(article_ref).name),
        "source_code": str(record.get("source_code") or PurePosixPath(article_ref).parts[0]),
        "coarse_topic_code": str(record.get("coarse_topic_code") or PurePosixPath(article_ref).parts[1]),
        "canonical_url": str(canonical_url),
        "primary_source_role": primary_role,
        "content_fallback_roles": content_fallback_roles,
        "metadata_roles": metadata_roles,
        "article_path": _article_path_from_ref(article_ref),
        "title": str(title) if title else "",
    }
    citation_key = identity.get("citation_key") or record.get("citation_key")
    if citation_key:
        entry["citation_key"] = str(citation_key)
    seed_url = identity.get("seed_url")
    if seed_url:
        entry["seed_url"] = str(seed_url)
    registration_summary = record.get("registration_summary") if isinstance(record.get("registration_summary"), dict) else {}
    registration_id = registration_summary.get("registration_id") or record.get("registration_id")
    if registration_id:
        entry["registration_id"] = str(registration_id)
    return entry, diagnostics


def _selection_stub_entry(article: dict[str, Any]) -> dict[str, Any]:
    article_ref = str(article.get("article_ref") or _safe_ref_from_selection(article))
    source_strategy = str(article.get("source_strategy") or "unknown")
    return {
        "article_ref": article_ref,
        "article_key": str(article.get("article_key")),
        "source_code": str(article.get("source_code")),
        "coarse_topic_code": str(article.get("coarse_topic_code") or "mixed-source"),
        "canonical_url": str(article.get("canonical_url")),
        "seed_url": str(article.get("seed_url") or article.get("canonical_url")),
        "primary_source_role": source_strategy,
        "content_fallback_roles": ["arxiv_pdf"] if source_strategy == "arxiv_abs_page" else [],
        "metadata_roles": [source_strategy] if source_strategy != "unknown" else [],
        "article_path": _article_path_from_ref(article_ref),
        "catalog_record_present": False,
        "title_status": "unresolved_catalog_placeholder",
        "provenance_sources": list(article.get("provenance_sources") or []),
        "registration_id": "m029_unified_corpus_index_rebuild_v1",
    }


def _build_indexes(entries: list[dict[str, Any]]) -> dict[str, Any]:
    by_source_code: dict[str, list[str]] = defaultdict(list)
    by_coarse_topic_code: dict[str, list[str]] = defaultdict(list)
    by_article_key: dict[str, str] = {}
    by_citation_key: dict[str, str] = {}
    by_canonical_url: dict[str, str] = {}
    by_title: dict[str, str] = {}

    for entry in entries:
        article_ref = str(entry["article_ref"])
        by_article_key[str(entry["article_key"])] = article_ref
        if entry.get("citation_key"):
            by_citation_key[str(entry["citation_key"])] = article_ref
        by_source_code[str(entry["source_code"])].append(article_ref)
        by_coarse_topic_code[str(entry["coarse_topic_code"])].append(article_ref)
        by_canonical_url[str(entry["canonical_url"])] = article_ref
        if entry.get("title"):
            by_title[str(entry["title"])] = article_ref

    return {
        "by_article_key": dict(sorted(by_article_key.items())),
        "by_citation_key": dict(sorted(by_citation_key.items())),
        "by_source_code": {key: sorted(value) for key, value in sorted(by_source_code.items())},
        "by_coarse_topic_code": {key: sorted(value) for key, value in sorted(by_coarse_topic_code.items())},
        "by_canonical_url": dict(sorted(by_canonical_url.items())),
        "by_title": dict(sorted(by_title.items())),
    }


def rebuild_index(catalog_path: Path, selection_path: Path | None = None) -> tuple[dict[str, Any], dict[str, Any], list[dict[str, Any]]]:
    catalog = _read_json(catalog_path)
    catalog_root = catalog_path.parent
    record_root = catalog_root / "article_catalog"
    diagnostics: list[dict[str, Any]] = []
    entries_by_ref: dict[str, dict[str, Any]] = {}
    records_scanned = 0

    for article_file in sorted(record_root.glob("*/*/*/article.json")):
        records_scanned += 1
        try:
            record = _read_json(article_file)
            relative_path = article_file.relative_to(catalog_root)
            entry, row_diagnostics = _entry_from_catalog_record(record, relative_path)
            diagnostics.extend(row_diagnostics)
            if entry is not None:
                entries_by_ref[str(entry["article_ref"])] = entry
        except Exception as exc:
            diagnostics.append({"level": "error", "code": "catalog_record_read_failed", "path": str(article_file), "message": str(exc)})

    selection_entries_considered = 0
    selection_stub_entries_added = 0
    if selection_path is not None:
        selection = _read_json(selection_path)
        articles = selection.get("articles")
        if not isinstance(articles, list):
            diagnostics.append({"level": "error", "code": "selection_articles_missing", "path": str(selection_path)})
            articles = []
        for article in articles:
            if not isinstance(article, dict):
                diagnostics.append({"level": "error", "code": "selection_article_not_object", "article": article})
                continue
            selection_entries_considered += 1
            try:
                article_ref = str(article.get("article_ref") or _safe_ref_from_selection(article))
                if article_ref not in entries_by_ref:
                    entries_by_ref[article_ref] = _selection_stub_entry(article)
                    selection_stub_entries_added += 1
            except Exception as exc:
                diagnostics.append({"level": "error", "code": "selection_stub_entry_failed", "article": article, "message": str(exc)})

    entries = sorted(entries_by_ref.values(), key=lambda item: str(item["article_ref"]))
    article_refs = [str(entry["article_ref"]) for entry in entries]
    article_keys = [str(entry["article_key"]) for entry in entries]
    canonical_urls = [str(entry["canonical_url"]) for entry in entries]
    for label, values in (("article_ref", article_refs), ("article_key", article_keys), ("canonical_url", canonical_urls)):
        duplicates = sorted(value for value, count in Counter(values).items() if count > 1)
        if duplicates:
            diagnostics.append({"level": "error", "code": f"duplicate_index_{label}", "duplicates": duplicates})

    index = {
        "schema_version": INDEX_SCHEMA_VERSION,
        "catalog_schema_version": catalog.get("schema_version"),
        "article_schema_version": catalog.get("article_schema_version"),
        "index_id": "daily_archive_article_catalog_index_fixture_v00_01",
        "generated_from": "article_catalog/",
        "lookup_policy": catalog.get("index", {}).get(
            "lookup_policy",
            {
                "cli_must_use_index": True,
                "full_tree_scan_allowed": False,
                "refresh_command_rebuilds_index": True,
            },
        ),
        "articles": entries,
        "indexes": _build_indexes(entries),
        "safety_flags": catalog.get("safety_flags", {}),
    }
    index["lookup_policy"] = {
        "cli_must_use_index": bool(index["lookup_policy"].get("cli_must_use_index", True)),
        "full_tree_scan_allowed": bool(index["lookup_policy"].get("full_tree_scan_allowed", False)),
        "refresh_command_rebuilds_index": bool(index["lookup_policy"].get("refresh_command_rebuilds_index", True)),
    }

    report = {
        "schema_version": INDEX_REBUILD_REPORT_SCHEMA_VERSION,
        "catalog_schema_version": catalog.get("schema_version"),
        "article_schema_version": catalog.get("article_schema_version"),
        "index_schema_version": INDEX_SCHEMA_VERSION,
        "catalog_record_glob": CATALOG_RECORD_GLOB,
        "normal_lookup_surface": catalog.get("index", {}).get("path", "index.json"),
        "full_tree_traversal_allowed_only_for": "--rebuild-index",
        "network_refresh_required": False,
        "network_fetch_attempted": False,
        "records_scanned": records_scanned,
        "selection_entries_considered": selection_entries_considered,
        "selection_stub_entries_added": selection_stub_entries_added,
        "entries_emitted": len(entries),
        "index_lookup_url_count": len(index["indexes"]["by_canonical_url"]),
        "index_lookup_article_key_count": len(index["indexes"]["by_article_key"]),
        "catalog_record_present_count": len([entry for entry in entries if entry.get("catalog_record_present") is not False]),
        "placeholder_entry_count": len([entry for entry in entries if entry.get("catalog_record_present") is False]),
        "diagnostic_counts": dict(sorted(Counter(row.get("code", "unknown") for row in diagnostics).items())),
        "error_count": sum(1 for row in diagnostics if row.get("level") == "error"),
    }
    return index, report, diagnostics


def _load_catalog_refs(catalog_path: Path) -> tuple[set[str], set[str]]:
    catalog = _read_json(catalog_path)
    index_path = catalog_path.parent / catalog.get("index", {}).get("path", "index.json")
    index = _read_json(index_path)
    refs = {entry["article_ref"] for entry in index.get("articles", []) if entry.get("article_ref") and entry.get("catalog_record_present") is not False}
    urls = {entry["canonical_url"] for entry in index.get("articles", []) if entry.get("canonical_url") and entry.get("catalog_record_present") is not False}
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


def _verify_rebuilt_index(
    index: dict[str, Any],
    selection_path: Path,
    check_index_titles: bool,
    check_safe_traversal: bool,
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    selection = _read_json(selection_path)
    articles = selection.get("articles") if isinstance(selection.get("articles"), list) else []
    by_canonical_url = index.get("indexes", {}).get("by_canonical_url", {})
    by_article_key = index.get("indexes", {}).get("by_article_key", {})
    index_entries_by_ref = {entry.get("article_ref"): entry for entry in index.get("articles", []) if isinstance(entry, dict)}

    for article in articles:
        canonical_url = article.get("canonical_url")
        article_key = article.get("article_key")
        if canonical_url not in by_canonical_url:
            diagnostics.append({"level": "error", "code": "selected_url_missing_from_index", "canonical_url": canonical_url})
        if article_key not in by_article_key:
            diagnostics.append({"level": "error", "code": "selected_article_key_missing_from_index", "article_key": article_key})

    if check_index_titles:
        for entry in index_entries_by_ref.values():
            if entry.get("catalog_record_present") is False:
                if entry.get("title"):
                    diagnostics.append({"level": "error", "code": "placeholder_entry_has_invented_title", "article_ref": entry.get("article_ref")})
            elif not entry.get("title"):
                diagnostics.append({"level": "error", "code": "catalog_entry_missing_title", "article_ref": entry.get("article_ref")})

    if check_safe_traversal:
        for entry in index_entries_by_ref.values():
            article_ref = str(entry.get("article_ref", ""))
            article_path = str(entry.get("article_path", ""))
            if not _is_safe_ref(article_ref):
                diagnostics.append({"level": "error", "code": "unsafe_index_article_ref", "article_ref": article_ref})
            if not article_path.startswith("article_catalog/") or not article_path.endswith("/article.json") or not _is_safe_ref(article_path):
                diagnostics.append({"level": "error", "code": "unsafe_index_article_path", "article_path": article_path})
    return diagnostics


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", type=Path, required=True)
    parser.add_argument("--catalog", type=Path, required=True)
    parser.add_argument("--expect-unique-article-count", type=int, default=18)
    parser.add_argument("--expect-duplicate-url")
    parser.add_argument("--rebuild-index", action="store_true")
    parser.add_argument("--write-index", type=Path)
    parser.add_argument("--write-index-report", type=Path)
    parser.add_argument("--write-diagnostics", type=Path)
    parser.add_argument("--check-index-titles", action="store_true")
    parser.add_argument("--check-index-idempotent", action="store_true")
    parser.add_argument("--check-safe-traversal", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        result = verify(args.selection, args.catalog, args.expect_unique_article_count, args.expect_duplicate_url)
        rebuild_result: dict[str, Any] | None = None
        diagnostics: list[dict[str, Any]] = list(result.get("diagnostics", []))
        if args.rebuild_index:
            index, report, rebuild_diagnostics = rebuild_index(args.catalog, args.selection)
            diagnostics.extend(rebuild_diagnostics)
            diagnostics.extend(_verify_rebuilt_index(index, args.selection, args.check_index_titles, args.check_safe_traversal))
            existing_index_matches_rebuild = False
            if args.write_index and args.write_index.exists():
                existing_index_matches_rebuild = _read_json(args.write_index) == index
            report["existing_index_matches_rebuild"] = existing_index_matches_rebuild
            if args.write_index:
                _write_json_atomic(args.write_index, index)
                report["wrote_index"] = str(args.write_index)
            if args.check_index_idempotent:
                second_index, _, second_diagnostics = rebuild_index(args.catalog, args.selection)
                diagnostics.extend(second_diagnostics)
                report["idempotent"] = second_index == index
                if not report["idempotent"]:
                    diagnostics.append({"level": "error", "code": "index_rebuild_not_idempotent"})
            report["diagnostic_counts"] = dict(sorted(Counter(row.get("code", "unknown") for row in diagnostics).items()))
            report["error_count"] = sum(1 for row in diagnostics if row.get("level") == "error")
            if args.write_index_report:
                _write_json_atomic(args.write_index_report, report)
            if args.write_diagnostics:
                _write_jsonl_atomic(args.write_diagnostics, diagnostics)
            rebuild_result = {
                "entries_emitted": report["entries_emitted"],
                "records_scanned": report["records_scanned"],
                "selection_stub_entries_added": report["selection_stub_entries_added"],
                "index_lookup_url_count": report["index_lookup_url_count"],
                "idempotent": report.get("idempotent"),
                "existing_index_matches_rebuild": report.get("existing_index_matches_rebuild"),
            }
        result["diagnostics"] = diagnostics
        if rebuild_result is not None:
            result["index_rebuild"] = rebuild_result
        result["status"] = "passed" if not any(row.get("level") == "error" for row in diagnostics) else "failed"
    except Exception as exc:
        print(json.dumps({"status": "failed", "code": "verification_error", "message": str(exc)}, sort_keys=True), file=sys.stderr)
        return 1
    stream = sys.stdout if result["status"] == "passed" else sys.stderr
    print(json.dumps(result, sort_keys=True), file=stream)
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
