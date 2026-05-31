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


def _base_payload(
    evidence_type: str,
    article: ArticleSelection,
    catalog_entry: dict[str, Any],
    chunk_manifest_path: Path,
    chunks: list[dict[str, Any]],
) -> dict[str, Any]:
    chunk_refs = [
        {
            "chunk_id": str(chunk.get("chunk_id") or chunk.get("id") or f"{article.article_ref}:chunk:{idx + 1:04d}"),
            "chunk_manifest_path": str(chunk_manifest_path),
        }
        for idx, chunk in enumerate(chunks[:3])
    ]
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
            if evidence_type == "identity":
                payload = _identity_payload(payload, article, catalog_entry)
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


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--chunks", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("--write-events", required=True, type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        events = replay(args)
    except EvidenceReplayError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2
    args.write_events.parent.mkdir(parents=True, exist_ok=True)
    args.write_events.write_text("".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8")
    completed_count = sum(1 for event in events if event["event_type"] == "evidence.article_completed")
    sys.stdout.write(f"wrote separated evidence for {completed_count} articles to {args.evidence}; events={args.write_events}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
