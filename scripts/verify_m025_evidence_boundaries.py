#!/usr/bin/env python3
"""Replay M025 separated metadata-safe evidence artifacts.

The command is intentionally local and fail-closed. It reads the catalog index,
corpus selection, and S06 chunking artifact directory, then writes per-article
assets/tables/links/identity evidence JSON plus an events JSONL file. Missing S06
chunking artifacts are a hard error with an explicit diagnostic so replay cannot
silently produce empty evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_PREFIX = "m025-article-evidence"
RUN_SCHEMA_VERSION = "m025-article-evidence-replay.v00.01"
EVIDENCE_TYPES = ("assets", "tables", "links", "identity")
ASSET_CHUNK_TYPES = {"figure_caption_context", "equation_context"}
TABLE_CHUNK_TYPES = {"table_context", "table_row_group"}
LINK_CHUNK_TYPES = {"citation_context", "reference_entry"}
FALSE_SAFETY_FLAGS = {
    "trusted_kg_import_allowed": False,
    "ladybugdb_written": False,
    "production_import_attempted": False,
    "raw_payloads_included": False,
}


class EvidenceReplayError(RuntimeError):
    """Raised when replay cannot safely produce evidence artifacts."""


@dataclass(frozen=True)
class ArticleSelection:
    article_ref: str
    source_code: str
    selection_role: str


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise EvidenceReplayError(f"required input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise EvidenceReplayError(f"required input is not valid JSON: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise EvidenceReplayError(f"required input must be a JSON object: {path}")
    return payload


def _article_slug(article_ref: str) -> str:
    return article_ref.replace("/", "-").replace(":", "-")


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _catalog_by_ref(index_payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    articles = index_payload.get("articles")
    if not isinstance(articles, list):
        raise EvidenceReplayError("catalog index does not contain an articles list")
    result: dict[str, dict[str, Any]] = {}
    for article in articles:
        if isinstance(article, dict) and isinstance(article.get("article_ref"), str):
            result[str(article["article_ref"])] = article
    return result


def _selection_articles(selection_payload: dict[str, Any]) -> list[ArticleSelection]:
    articles = selection_payload.get("articles")
    if not isinstance(articles, list) or not articles:
        raise EvidenceReplayError("selection does not contain a non-empty articles list")
    selections: list[ArticleSelection] = []
    for idx, article in enumerate(articles):
        if not isinstance(article, dict):
            raise EvidenceReplayError(f"selection article at index {idx} is not an object")
        article_ref = article.get("article_ref")
        source_code = article.get("source_code")
        if not isinstance(article_ref, str) or not article_ref:
            raise EvidenceReplayError(f"selection article at index {idx} is missing article_ref")
        if not isinstance(source_code, str) or not source_code:
            raise EvidenceReplayError(f"selection article {article_ref} is missing source_code")
        selections.append(
            ArticleSelection(
                article_ref=article_ref,
                source_code=source_code,
                selection_role=str(article.get("selection_role") or "selected"),
            )
        )
    return selections


def _chunk_manifest_for_article(chunks_dir: Path, article_ref: str) -> Path:
    slug = _article_slug(article_ref)
    candidates = [
        chunks_dir / slug / "chunks.json",
        chunks_dir / f"{slug}.json",
        chunks_dir / article_ref / "chunks.json",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    matches = sorted(chunks_dir.rglob(f"*{slug}*.json"))
    if matches:
        return matches[0]
    raise EvidenceReplayError(
        f"missing S06 chunking artifact for {article_ref}; expected one of: "
        + ", ".join(str(candidate) for candidate in candidates)
    )


def _read_chunks(chunks_dir: Path, article_ref: str) -> tuple[Path, list[dict[str, Any]]]:
    manifest_path = _chunk_manifest_for_article(chunks_dir, article_ref)
    payload = _load_json(manifest_path)
    raw_chunks = payload.get("chunks") or payload.get("items") or []
    if not isinstance(raw_chunks, list):
        raise EvidenceReplayError(f"chunk manifest {manifest_path} has non-list chunks/items")
    chunks = [chunk for chunk in raw_chunks if isinstance(chunk, dict)]
    return manifest_path, chunks


def _source_ref(article: ArticleSelection, catalog_entry: dict[str, Any]) -> dict[str, Any]:
    article_path = str(catalog_entry.get("article_path") or "")
    return {
        "source_id": f"{article.article_ref}:source:{catalog_entry.get('primary_source_role') or article.source_code}",
        "source_code": article.source_code,
        "article_path": article_path,
        "sha256": str(catalog_entry.get("sha256") or _sha256_text(article.article_ref + article_path)),
    }


def _chunk_id(article_ref: str, chunk: dict[str, Any], idx: int) -> str:
    return str(chunk.get("chunk_id") or chunk.get("id") or f"{article_ref}:chunk:{idx + 1:04d}")


def _chunk_type(chunk: dict[str, Any]) -> str:
    return str(chunk.get("chunk_type") or chunk.get("type") or chunk.get("route") or "unknown")


def _source_span_id(article_ref: str, evidence_type: str, idx: int) -> str:
    return f"{article_ref}:span:{evidence_type}:{idx + 1:04d}"


def _source_element_id(article_ref: str, element_type: str, idx: int) -> str:
    return f"{article_ref}:element:{element_type}:{idx + 1:04d}"


def _chunk_refs(article: ArticleSelection, chunk_manifest_path: Path, chunks: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        {
            "chunk_id": _chunk_id(article.article_ref, chunk, idx),
            "chunk_manifest_path": str(chunk_manifest_path),
        }
        for idx, chunk in enumerate(chunks)
    ]


def _base_payload(
    evidence_type: str,
    article: ArticleSelection,
    catalog_entry: dict[str, Any],
    chunk_manifest_path: Path,
    chunks: list[dict[str, Any]],
) -> dict[str, Any]:
    chunk_refs = _chunk_refs(article, chunk_manifest_path, chunks)
    diagnostics: list[dict[str, Any]] = []
    if not chunks:
        diagnostics.append(
            {
                "code": "S06_CHUNKS_EMPTY",
                "severity": "warning",
                "json_path": "$.chunks",
                "message": "S06 chunking manifest exists but contains no chunk entries.",
            }
        )
    return {
        "schema_version": f"{SCHEMA_PREFIX}-{evidence_type}.v00.01",
        "evidence_type": evidence_type,
        "article_ref": article.article_ref,
        "source_ref": _source_ref(article, catalog_entry),
        "chunk_refs": chunk_refs,
        "items": [],
        "summary": {
            "item_count": 0,
            "unsupported_type_count": 0,
            "diagnostic_count": len(diagnostics),
        },
        "safety_flags": {
            "metadata_only": True,
            "review_only": True,
            **FALSE_SAFETY_FLAGS,
        },
        "import_eligible_count": 0,
        "promoted_to_fact_count": 0,
        "diagnostics": diagnostics,
    }


def _asset_payload(base: dict[str, Any], article: ArticleSelection, chunks: list[dict[str, Any]]) -> dict[str, Any]:
    items: list[dict[str, Any]] = []
    for idx, chunk in enumerate(chunks):
        chunk_type = _chunk_type(chunk)
        if chunk_type not in ASSET_CHUNK_TYPES:
            continue
        asset_type = "equation" if chunk_type == "equation_context" else "figure"
        ordinal = len(items) + 1
        items.append(
            {
                "asset_id": f"{article.article_ref}:asset:{asset_type}:{ordinal:04d}",
                "asset_type": asset_type,
                "element_id": _source_element_id(article.article_ref, asset_type, idx),
                "source_span_id": _source_span_id(article.article_ref, asset_type, idx),
                "page_index_node_id": f"{article.article_ref}:page-index:artifact:{asset_type}:{ordinal:04d}",
                "chunk_ids": [_chunk_id(article.article_ref, chunk, idx)],
                "media_type": "metadata/unknown",
                "byte_size": None,
                "content_sha256": None,
                "raw_text_embedded": False,
                "raw_binary_embedded": False,
                "interpretation_status": "metadata_only",
            }
        )
    return _payload_with_items_or_diagnostic(base, items, evidence_type="assets")


def _table_payload(base: dict[str, Any], article: ArticleSelection, chunks: list[dict[str, Any]]) -> dict[str, Any]:
    items: list[dict[str, Any]] = []
    for idx, chunk in enumerate(chunks):
        if _chunk_type(chunk) not in TABLE_CHUNK_TYPES and str(chunk.get("route") or "") != "table_extraction":
            continue
        ordinal = len(items) + 1
        items.append(
            {
                "table_id": f"{article.article_ref}:table:{ordinal:04d}",
                "element_id": _source_element_id(article.article_ref, "table", idx),
                "source_span_id": _source_span_id(article.article_ref, "table", idx),
                "page_index_node_id": f"{article.article_ref}:page-index:artifact:table:{ordinal:04d}",
                "chunk_ids": [_chunk_id(article.article_ref, chunk, idx)],
                "column_count": None,
                "row_count": None,
                "structure_sha256": _sha256_text(f"{article.article_ref}:table:{_chunk_id(article.article_ref, chunk, idx)}"),
                "cell_payload_embedded": False,
                "raw_text_embedded": False,
                "interpretation_status": "metadata_only",
            }
        )
    return _payload_with_items_or_diagnostic(base, items, evidence_type="tables")


def _link_payload(base: dict[str, Any], article: ArticleSelection, chunks: list[dict[str, Any]]) -> dict[str, Any]:
    items: list[dict[str, Any]] = []
    for idx, chunk in enumerate(chunks):
        if _chunk_type(chunk) not in LINK_CHUNK_TYPES and str(chunk.get("route") or "") != "citation_graph":
            continue
        ordinal = len(items) + 1
        items.append(
            {
                "link_id": f"{article.article_ref}:link:citation:{ordinal:04d}",
                "link_family": "citation",
                "source_element_id": _source_element_id(article.article_ref, "citation", idx),
                "target_ref": {
                    "target_type": "reference_entry",
                    "target_id": f"{article.article_ref}:reference:{ordinal:04d}",
                },
                "source_span_ids": [_source_span_id(article.article_ref, "citation", idx)],
                "source_page_index_anchor_id": f"{article.article_ref}:page-index-anchor:citation:{ordinal:04d}",
                "chunk_ids": [_chunk_id(article.article_ref, chunk, idx)],
                "review_state": "review_required",
                "raw_text_embedded": False,
                "import_eligible": False,
                "promoted_to_fact": False,
            }
        )
    return _payload_with_items_or_diagnostic(base, items, evidence_type="links")


def _identity_payload(base: dict[str, Any], article: ArticleSelection, catalog_entry: dict[str, Any]) -> dict[str, Any]:
    item = {
        "identity_id": f"{article.article_ref}:identity:catalog-ref",
        "identity_type": "article_ref",
        "article_ref": article.article_ref,
        "source_element_id": f"{article.article_ref}:element:metadata:article-ref",
        "source_span_id": f"{article.article_ref}:span:metadata:article-ref",
        "normalized_value": str(catalog_entry.get("article_key") or article.article_ref.rsplit("/", 1)[-1]).lower(),
        "normalization": {"algorithm": "catalog-key-lowercase", "version": 1},
        "dedup_decision": "source_identity_review_required",
        "review_state": "review_required",
        "raw_text_embedded": False,
        "import_eligible": False,
        "promoted_to_fact": False,
    }
    payload = dict(base)
    payload["items"] = [item]
    payload["summary"] = {**dict(base["summary"]), "item_count": 1}
    return payload


def _payload_with_items_or_diagnostic(base: dict[str, Any], items: list[dict[str, Any]], *, evidence_type: str) -> dict[str, Any]:
    payload = dict(base)
    diagnostics = list(base["diagnostics"])
    unsupported_type_count = int(base["summary"].get("unsupported_type_count", 0))
    if not items:
        diagnostics.append(
            {
                "code": "EVIDENCE_TYPE_NOT_OBSERVED",
                "severity": "info",
                "json_path": f"$.{evidence_type}.items",
                "message": f"No {evidence_type} evidence-bearing chunks were observed in the S06 manifest; empty output is diagnostic, not silent.",
            }
        )
    payload["items"] = items
    payload["summary"] = {
        **dict(base["summary"]),
        "item_count": len(items),
        "unsupported_type_count": unsupported_type_count,
        "diagnostic_count": len(diagnostics),
    }
    payload["diagnostics"] = diagnostics
    return payload


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _event(event_type: str, **fields: Any) -> dict[str, Any]:
    return {"schema_version": RUN_SCHEMA_VERSION, "event_type": event_type, **fields}


def replay(args: argparse.Namespace) -> list[dict[str, Any]]:
    catalog = _load_json(args.catalog)
    index = _load_json(args.index)
    selection = _load_json(args.selection)
    if not args.chunks.exists():
        raise EvidenceReplayError(
            f"missing S06 chunking directory: {args.chunks}; run S06 chunking replay before S07 evidence replay"
        )
    if not args.chunks.is_dir():
        raise EvidenceReplayError(f"S06 chunking path is not a directory: {args.chunks}")

    catalog_refs = _catalog_by_ref(index)
    events: list[dict[str, Any]] = [
        _event(
            "evidence.replay_started",
            catalog_schema_version=catalog.get("schema_version"),
            selection_id=selection.get("selection_id"),
        )
    ]
    for article in _selection_articles(selection):
        catalog_entry = catalog_refs.get(article.article_ref)
        if catalog_entry is None:
            raise EvidenceReplayError(f"selection article {article.article_ref} is absent from catalog index")
        chunk_manifest, chunks = _read_chunks(args.chunks, article.article_ref)
        article_dir = args.evidence / _article_slug(article.article_ref)
        events.append(_event("evidence.article_started", article_ref=article.article_ref))
        for evidence_type in EVIDENCE_TYPES:
            payload = _base_payload(evidence_type, article, catalog_entry, chunk_manifest, chunks)
            if evidence_type == "assets":
                payload = _asset_payload(payload, article, chunks)
            elif evidence_type == "tables":
                payload = _table_payload(payload, article, chunks)
            elif evidence_type == "links":
                payload = _link_payload(payload, article, chunks)
            elif evidence_type == "identity":
                payload = _identity_payload(payload, article, catalog_entry)
            else:
                raise EvidenceReplayError(f"unsupported evidence type configured: {evidence_type}")
            output_path = article_dir / f"{evidence_type}.json"
            _write_json(output_path, payload)
            events.append(
                _event(
                    "evidence.artifact_written",
                    article_ref=article.article_ref,
                    evidence_type=evidence_type,
                    path=str(output_path),
                    item_count=payload["summary"]["item_count"],
                    diagnostic_count=payload["summary"]["diagnostic_count"],
                    metadata_only=True,
                    trusted_kg_import_allowed=False,
                    ladybugdb_written=False,
                    production_import_attempted=False,
                )
            )
        events.append(_event("evidence.article_completed", article_ref=article.article_ref))
    return events


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise EvidenceReplayError(f"required events file is missing: {path}") from exc
    events: list[dict[str, Any]] = []
    for idx, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            raise EvidenceReplayError(f"events file is not valid JSONL at line {idx}: {path}: {exc}") from exc
        if not isinstance(payload, dict):
            raise EvidenceReplayError(f"events file line {idx} must be a JSON object: {path}")
        events.append(payload)
    if not events:
        raise EvidenceReplayError(f"events file contains no events: {path}")
    return events


def _iter_strings(value: Any) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, dict):
        strings: list[str] = []
        for item in value.values():
            strings.extend(_iter_strings(item))
        return strings
    if isinstance(value, list):
        strings = []
        for item in value:
            strings.extend(_iter_strings(item))
        return strings
    return []


def _redaction_findings(payload: dict[str, Any], *, path: Path) -> list[dict[str, Any]]:
    forbidden_fragments = (
        "chunk_text",
        "raw_article_text",
        "article_body",
        "base64,",
        "BEGIN PRIVATE KEY",
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
    )
    findings: list[dict[str, Any]] = []
    for text_value in _iter_strings(payload):
        lowered = text_value.lower()
        for fragment in forbidden_fragments:
            if fragment.lower() in lowered:
                findings.append(
                    {
                        "code": "REDACTION_FORBIDDEN_FRAGMENT",
                        "severity": "error",
                        "path": str(path),
                        "fragment": fragment,
                    }
                )
    return findings


def _validate_safety_flags(payload: dict[str, Any], *, path: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    flags = payload.get("safety_flags")
    if not isinstance(flags, dict):
        return [{"code": "SAFETY_FLAGS_MISSING", "severity": "error", "path": str(path)}]
    expected = {"metadata_only": True, "review_only": True, **FALSE_SAFETY_FLAGS}
    for key, expected_value in expected.items():
        if flags.get(key) is not expected_value:
            findings.append(
                {
                    "code": "SAFETY_FLAG_MISMATCH",
                    "severity": "error",
                    "path": str(path),
                    "json_path": f"$.safety_flags.{key}",
                    "expected": expected_value,
                    "actual": flags.get(key),
                }
            )
    for key in ("import_eligible_count", "promoted_to_fact_count"):
        if payload.get(key) != 0:
            findings.append(
                {
                    "code": "IMPORT_COUNTER_NONZERO",
                    "severity": "error",
                    "path": str(path),
                    "json_path": f"$.{key}",
                    "expected": 0,
                    "actual": payload.get(key),
                }
            )
    return findings


def _validate_provenance(payload: dict[str, Any], *, path: Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    if not isinstance(payload.get("source_ref"), dict):
        findings.append({"code": "SOURCE_REF_MISSING", "severity": "error", "path": str(path), "json_path": "$.source_ref"})
    chunk_refs = payload.get("chunk_refs")
    if not isinstance(chunk_refs, list) or not chunk_refs:
        findings.append({"code": "CHUNK_REFS_MISSING", "severity": "error", "path": str(path), "json_path": "$.chunk_refs"})
    items = payload.get("items")
    if not isinstance(items, list):
        findings.append({"code": "ITEMS_NOT_LIST", "severity": "error", "path": str(path), "json_path": "$.items"})
        return findings
    for idx, item in enumerate(items):
        if not isinstance(item, dict):
            findings.append({"code": "ITEM_NOT_OBJECT", "severity": "error", "path": str(path), "json_path": f"$.items[{idx}]"})
            continue
        serialized = json.dumps(item, sort_keys=True)
        has_chunk = "chunk_id" in serialized
        has_span = "span" in serialized
        has_element = "element" in serialized
        if not (has_chunk or has_span or has_element):
            findings.append(
                {
                    "code": "ITEM_PROVENANCE_MISSING",
                    "severity": "error",
                    "path": str(path),
                    "json_path": f"$.items[{idx}]",
                }
            )
    return findings


def validate_evidence(args: argparse.Namespace) -> dict[str, Any]:
    index = _load_json(args.index)
    selection = _load_json(args.selection)
    events = _load_jsonl(args.events)
    catalog_refs = _catalog_by_ref(index)
    selected = _selection_articles(selection)
    article_summaries: list[dict[str, Any]] = []
    aggregate_counts = dict.fromkeys(EVIDENCE_TYPES, 0)
    aggregate_diagnostics: dict[str, int] = {}
    findings: list[dict[str, Any]] = []
    provenance_item_count = 0
    provenance_checked_count = 0

    for article in selected:
        if article.article_ref not in catalog_refs:
            raise EvidenceReplayError(f"selection article {article.article_ref} is absent from catalog index")
        article_dir = args.evidence / _article_slug(article.article_ref)
        per_article = {
            "article_ref": article.article_ref,
            "source_code": article.source_code,
            "evidence_counts": {},
            "diagnostic_counts": {},
            "artifact_paths": {},
        }
        for evidence_type in EVIDENCE_TYPES:
            path = article_dir / f"{evidence_type}.json"
            payload = _load_json(path)
            if payload.get("article_ref") != article.article_ref:
                findings.append(
                    {
                        "code": "ARTICLE_REF_MISMATCH",
                        "severity": "error",
                        "path": str(path),
                        "expected": article.article_ref,
                        "actual": payload.get("article_ref"),
                    }
                )
            summary = payload.get("summary") if isinstance(payload.get("summary"), dict) else {}
            item_count = int(summary.get("item_count") or 0)
            diagnostic_count = int(summary.get("diagnostic_count") or 0)
            per_article["evidence_counts"][evidence_type] = item_count
            per_article["diagnostic_counts"][evidence_type] = diagnostic_count
            per_article["artifact_paths"][evidence_type] = str(path)
            aggregate_counts[evidence_type] += item_count
            for diagnostic in payload.get("diagnostics", []):
                if isinstance(diagnostic, dict):
                    code = str(diagnostic.get("code") or "UNKNOWN_DIAGNOSTIC")
                    aggregate_diagnostics[code] = aggregate_diagnostics.get(code, 0) + 1
            items = payload.get("items") if isinstance(payload.get("items"), list) else []
            provenance_item_count += len(items)
            before = len(findings)
            findings.extend(_validate_provenance(payload, path=path))
            provenance_checked_count += len(items) if len(findings) == before else 0
            if getattr(args, "require_no_import_flags", False):
                findings.extend(_validate_safety_flags(payload, path=path))
            if getattr(args, "require_redaction", False):
                findings.extend(_redaction_findings(payload, path=path))
        article_summaries.append(per_article)

    event_counts: dict[str, int] = {}
    for event in events:
        event_type = str(event.get("event_type") or "unknown")
        event_counts[event_type] = event_counts.get(event_type, 0) + 1
        if getattr(args, "require_no_import_flags", False) and event_type == "evidence.artifact_written":
            for key in ("trusted_kg_import_allowed", "ladybugdb_written", "production_import_attempted"):
                if event.get(key) is not False:
                    findings.append(
                        {
                            "code": "EVENT_IMPORT_FLAG_MISMATCH",
                            "severity": "error",
                            "event_type": event_type,
                            "field": key,
                            "expected": False,
                            "actual": event.get(key),
                        }
                    )

    expected_artifact_events = len(selected) * len(EVIDENCE_TYPES)
    if event_counts.get("evidence.artifact_written", 0) != expected_artifact_events:
        findings.append(
            {
                "code": "EVENT_ARTIFACT_COUNT_MISMATCH",
                "severity": "error",
                "expected": expected_artifact_events,
                "actual": event_counts.get("evidence.artifact_written", 0),
            }
        )

    missing_evidence_diagnostics = [
        {"code": code, "count": count}
        for code, count in sorted(aggregate_diagnostics.items())
        if code in {"EVIDENCE_TYPE_NOT_OBSERVED", "S06_CHUNKS_EMPTY", "S06_ROADMAP_HANDOFF_RECONSTRUCTED"}
    ]
    summary = {
        "schema_version": "m025-article-evidence-summary.v00.01",
        "selection_id": selection.get("selection_id"),
        "article_count": len(selected),
        "evidence_counts": aggregate_counts,
        "article_summaries": article_summaries,
        "event_counts": event_counts,
        "missing_or_unsupported_evidence_diagnostics": missing_evidence_diagnostics,
        "all_diagnostic_counts": dict(sorted(aggregate_diagnostics.items())),
        "provenance_coverage": {
            "artifact_count": len(selected) * len(EVIDENCE_TYPES),
            "items_with_provenance_checked": provenance_checked_count,
            "item_count": provenance_item_count,
            "source_ref_required": True,
            "chunk_refs_required": True,
        },
        "redaction_checks": {
            "required": bool(getattr(args, "require_redaction", False)),
            "forbidden_value_findings": sum(1 for finding in findings if finding["code"] == "REDACTION_FORBIDDEN_FRAGMENT"),
            "passed": not any(finding["code"] == "REDACTION_FORBIDDEN_FRAGMENT" for finding in findings),
        },
        "safety_state": {
            "metadata_only": True,
            "review_only": True,
            "trusted_kg_import_allowed": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
            "import_eligible_count": 0,
            "promoted_to_fact_count": 0,
        },
        "findings": findings,
        "validation_passed": not findings,
    }
    return summary


def _write_report(path: Path, summary: dict[str, Any]) -> None:
    lines = [
        "# M025 S07 Evidence Boundary Report",
        "",
        "## Scope",
        f"- Selection: `{summary.get('selection_id')}`",
        f"- Articles: {summary.get('article_count')}",
        "- Boundary: assets, tables, links, identity, and evidence metadata are separated from chunk text.",
        "",
        "## Per-Article Counts",
        "| Article | Assets | Tables | Links | Identity | Diagnostics |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for article in summary["article_summaries"]:
        counts = article["evidence_counts"]
        diag_total = sum(article["diagnostic_counts"].values())
        lines.append(
            f"| `{article['article_ref']}` | {counts.get('assets', 0)} | {counts.get('tables', 0)} | {counts.get('links', 0)} | {counts.get('identity', 0)} | {diag_total} |"
        )
    lines.extend(
        [
            "",
            "## Aggregate Counts",
            "| Evidence Type | Count |",
            "|---|---:|",
        ]
    )
    for evidence_type, count in summary["evidence_counts"].items():
        lines.append(f"| {evidence_type} | {count} |")
    lines.extend(["", "## Missing or Unsupported Evidence Diagnostics"])
    diagnostics = summary.get("missing_or_unsupported_evidence_diagnostics") or []
    if diagnostics:
        for diagnostic in diagnostics:
            lines.append(f"- `{diagnostic['code']}`: {diagnostic['count']}")
    else:
        lines.append("- None observed in the separated evidence artifacts.")
    coverage = summary["provenance_coverage"]
    lines.extend(
        [
            "",
            "## Provenance Coverage",
            f"- Evidence artifacts checked: {coverage['artifact_count']}",
            f"- Evidence items checked for provenance: {coverage['items_with_provenance_checked']} / {coverage['item_count']}",
            "- Each artifact must carry `source_ref` and `chunk_refs`; each evidence item must carry chunk, span, or element provenance.",
            "",
            "## Redaction Checks",
            f"- Required: {summary['redaction_checks']['required']}",
            f"- Forbidden value findings: {summary['redaction_checks']['forbidden_value_findings']}",
            f"- Passed: {summary['redaction_checks']['passed']}",
            "",
            "## No-Import / No-Write Safety State",
            "- `metadata_only=true` and `review_only=true`.",
            "- `trusted_kg_import_allowed=false`, `ladybugdb_written=false`, and `production_import_attempted=false`.",
            "- `import_eligible_count=0` and `promoted_to_fact_count=0`.",
            "",
            "## Failure Modes",
            "- Local filesystem inputs (`catalog`, `index`, `selection`, `evidence`, `events`) fail with explicit path-bearing `EvidenceReplayError` messages when missing or malformed.",
            "- JSON and JSONL decoding errors bubble as validation failures with the offending path and line where applicable.",
            "- Missing or mismatched no-import flags, event counts, provenance pointers, or redaction checks are accumulated as findings and cause a non-zero verifier exit.",
            "- No network, API, database, or graph-write dependency is used by this report path.",
            "",
            "## Load Profile",
            "- The first 10x load pressure point is local filesystem JSON reads across `articles × evidence_types`; report generation is linear and bounded by the explicit selection file.",
            "- Protection: no repository-wide scan, no raw article payload loading, no embeddings/model calls, no network calls, and one bounded JSON parse per selected evidence artifact plus one JSONL event pass.",
            "",
            "## Negative Tests",
            "- `tests/test_m025_evidence_replay.py::test_validate_evidence_writes_summary_and_report` covers the positive reporting contract, aggregate counts, provenance, redaction, and fail-closed safety state.",
            "- `tests/test_m025_evidence_replay.py::test_validate_evidence_fails_on_import_flag_violation` covers non-zero/unsafe import flag rejection.",
            "- Existing replay tests cover missing chunking input, empty evidence diagnostics, event emission, and metadata-only separated artifacts.",
            "",
            "## Validation Findings",
        ]
    )
    if summary["findings"]:
        for finding in summary["findings"]:
            lines.append(f"- `{finding['code']}`: `{json.dumps(finding, sort_keys=True)}`")
    else:
        lines.append("- None. Validation passed.")
    lines.append("")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines), encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--chunks", type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--write-events", type=Path)
    parser.add_argument("--events", type=Path)
    parser.add_argument("--require-redaction", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--write-summary", type=Path)
    parser.add_argument("--write-report", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.write_events is not None or args.chunks is not None:
            if args.chunks is None or args.write_events is None:
                raise EvidenceReplayError("replay mode requires both --chunks and --write-events")
            events = replay(args)
            args.write_events.parent.mkdir(parents=True, exist_ok=True)
            args.write_events.write_text("".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8")
            completed_count = sum(1 for event in events if event["event_type"] == "evidence.article_completed")
            sys.stdout.write(
                f"wrote separated evidence for {completed_count} articles to {args.evidence}; events={args.write_events}\n"
            )
            return 0
        if args.events is None:
            raise EvidenceReplayError("validation mode requires --events")
        summary = validate_evidence(args)
        if args.write_summary is not None:
            _write_json(args.write_summary, summary)
        if args.write_report is not None:
            _write_report(args.write_report, summary)
        if not summary["validation_passed"]:
            sys.stderr.write(f"ERROR: evidence boundary validation failed with {len(summary['findings'])} finding(s)\n")
            return 2
        sys.stdout.write(
            f"validated evidence boundaries for {summary['article_count']} articles; findings=0; "
            f"summary={args.write_summary}; report={args.write_report}\n"
        )
        return 0
    except EvidenceReplayError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
