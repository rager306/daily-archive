#!/usr/bin/env python3
"""Complete M025 boundary replay over local metadata-safe artifacts.

The verifier composes the fixed article selection/catalog, local source loading,
parser normalization, PageIndex construction, chunking/evidence summaries, and
baseline/final replay comparisons into one S10 boundary surface. It is
intentionally local-only: no network fetches, graph imports, or LadybugDB writes
are attempted. Outputs are metadata-only hashes, counts, provenance, diagnostic
codes/json_paths, safety flags, deltas, and readiness blockers.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from arxiv_archive.full_text import FullTextSource, ingest_full_text
from arxiv_archive.indexing.page_index import build_page_index_from_parsed
from arxiv_archive.parsing.parser import parse_article

SCHEMA_VERSION = "m025-boundary-replay-summary.v00.01"
ARTIFACT_SCHEMA_VERSION = "m025-boundary-replay-artifact.v00.01"
EVENT_SCHEMA_VERSION = "m025-boundary-replay-event.v00.01"
EVIDENCE_TYPES = ("assets", "tables", "links", "identity")
BOUNDARY_TYPES = ("loader", "parser", "page_index", "chunking", "evidence", "baseline")
TEXT_KEY_FRAGMENTS = ("text", "content", "payload", "body", "base64", "raw")
FALSE_SAFETY_FLAGS = {
    "graph_import_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "production_ladybugdb_write_allowed": False,
    "ladybugdb_written": False,
    "graph_readiness_claim": False,
}
COMPARISON_CATEGORIES = {"exact_match", "metric_delta", "baseline_missing", "not_applicable"}


class BoundaryReplayError(RuntimeError):
    """Raised when boundary replay cannot safely continue."""


@dataclass(frozen=True)
class ArticleSelection:
    article_ref: str
    source_code: str
    selection_role: str


@dataclass(frozen=True)
class LocalSourceResolution:
    source_path: Path | None
    source_type: str
    diagnostics: list[dict[str, Any]]


def _looks_like_url(value: str) -> bool:
    lowered = value.lower()
    return lowered.startswith(("http://", "https://", "http:/", "https:/"))


def _reject_url_path(path: Path, label: str) -> None:
    if _looks_like_url(str(path)):
        raise BoundaryReplayError(f"{label} must be a local filesystem path, not a URL: {path}")


def _load_json(path: Path) -> dict[str, Any]:
    _reject_url_path(path, "input")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise BoundaryReplayError(f"required local input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise BoundaryReplayError(f"required local input is not valid JSON: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise BoundaryReplayError(f"required local input must be a JSON object: {path}")
    return payload


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    _reject_url_path(path, "output")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _article_slug(article_ref: str) -> str:
    return article_ref.replace("/", "-").replace(":", "-")


def _paper_id(article_ref: str) -> str:
    return article_ref.rsplit("/", maxsplit=1)[-1]


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _event(event_type: str, **fields: Any) -> dict[str, Any]:
    return {"schema_version": EVENT_SCHEMA_VERSION, "event_type": event_type, **fields}


def _diagnostic(code: str, json_path: str, message: str, *, severity: str = "error") -> dict[str, Any]:
    return {"code": code, "severity": severity, "json_path": json_path, "message": message}


def _catalog_by_ref(index_payload: dict[str, Any]) -> dict[str, dict[str, Any]]:
    articles = index_payload.get("articles")
    if not isinstance(articles, list):
        raise BoundaryReplayError("catalog index does not contain an articles list")
    result: dict[str, dict[str, Any]] = {}
    for idx, article in enumerate(articles):
        if not isinstance(article, dict):
            raise BoundaryReplayError(f"catalog index article at index {idx} is not an object")
        article_ref = article.get("article_ref")
        if isinstance(article_ref, str) and article_ref:
            result[article_ref] = article
    return result


def _selection_articles(selection_payload: dict[str, Any]) -> list[ArticleSelection]:
    articles = selection_payload.get("articles")
    if not isinstance(articles, list) or not articles:
        raise BoundaryReplayError("selection does not contain a non-empty articles list")
    selections: list[ArticleSelection] = []
    for idx, article in enumerate(articles):
        if not isinstance(article, dict):
            raise BoundaryReplayError(f"selection article at index {idx} is not an object")
        article_ref = article.get("article_ref")
        source_code = article.get("source_code")
        if not isinstance(article_ref, str) or not article_ref:
            raise BoundaryReplayError(f"selection article at index {idx} is missing article_ref")
        if not isinstance(source_code, str) or not source_code:
            raise BoundaryReplayError(f"selection article {article_ref} is missing source_code")
        selections.append(
            ArticleSelection(
                article_ref=article_ref,
                source_code=source_code,
                selection_role=str(article.get("selection_role") or "selected"),
            )
        )
    return selections


def _candidate_source_paths(catalog_entry: dict[str, Any], roots: list[Path]) -> list[Path]:
    raw_candidates: list[str] = []
    for key in ("full_text_path", "local_text_path", "source_path", "markdown_path", "text_path"):
        value = catalog_entry.get(key)
        if isinstance(value, str) and value:
            raw_candidates.append(value)
    article_path = catalog_entry.get("article_path")
    if isinstance(article_path, str) and article_path:
        raw_candidates.append(article_path)
    candidates: list[Path] = []
    for raw in raw_candidates:
        if _looks_like_url(raw):
            continue
        path = Path(raw)
        expanded = [path] if path.is_absolute() else [root / path for root in roots]
        for item in expanded:
            if item not in candidates:
                candidates.append(item)
    return candidates


def _resolve_local_source(
    catalog_entry: dict[str, Any], article: ArticleSelection, roots: list[Path]
) -> LocalSourceResolution:
    candidates = _candidate_source_paths(catalog_entry, roots)
    for candidate in candidates:
        if candidate.exists() and candidate.is_file():
            suffix = candidate.suffix.lower()
            source_type = "markdown" if suffix in {".md", ".markdown", ".html", ".htm"} else "text"
            return LocalSourceResolution(candidate, source_type, [])
    message = "No local source path was found in catalog/index metadata."
    if candidates:
        message = "No candidate local source file exists: " + ", ".join(str(path) for path in candidates)
    return LocalSourceResolution(
        None,
        "markdown",
        [_diagnostic("LOCAL_SOURCE_MISSING", "$.catalog_ref", message, severity="blocker")],
    )


def _loader_summary(article: ArticleSelection, resolution: LocalSourceResolution) -> dict[str, Any]:
    if resolution.source_path is None:
        return {
            "status": "blocked",
            "source_path": None,
            "source_type": resolution.source_type,
            "text_sha256": None,
            "char_count": 0,
            "line_count": 0,
            "quality_status": "missing_source",
            "warnings": [item["code"] for item in resolution.diagnostics],
            "diagnostics": resolution.diagnostics,
        }
    source = FullTextSource(
        paper_id=_paper_id(article.article_ref),
        source_type=resolution.source_type,
        source_path=resolution.source_path,
    )
    ingestion = ingest_full_text(source)
    diagnostics = [
        _diagnostic(
            "LOADER_QUALITY_WARNING",
            "$.loader.warnings",
            warning,
            severity="warning",
        )
        for warning in ingestion.warnings
    ]
    status = "loaded" if ingestion.text else "blocked"
    if not ingestion.text:
        diagnostics.append(
            _diagnostic(
                "LOADER_NO_TEXT",
                "$.loader.char_count",
                "Local loader produced no text; parser/PageIndex boundaries are blocked.",
                severity="blocker",
            )
        )
    return {
        "status": status,
        "source_path": str(ingestion.source_path),
        "source_type": ingestion.source_type,
        "extraction_mode": ingestion.extraction_mode,
        "text_sha256": _sha256_text(ingestion.text) if ingestion.text else None,
        "char_count": ingestion.quality.char_count,
        "line_count": ingestion.quality.line_count,
        "heading_count": ingestion.quality.heading_count,
        "quality_status": ingestion.quality.status,
        "fallback_reason": ingestion.fallback_reason,
        "warnings": list(ingestion.warnings),
        "diagnostics": diagnostics,
        "_ingestion": ingestion,
    }


def _parser_and_page_index_summaries(loader: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
    ingestion = loader.get("_ingestion")
    if ingestion is None:
        blocked = _diagnostic(
            "PARSER_BLOCKED_BY_LOADER",
            "$.loader.status",
            "Parser/PageIndex were not run because loader did not produce local text.",
            severity="blocker",
        )
        return (
            {"status": "blocked", "element_count": 0, "diagnostics": [blocked]},
            {"status": "blocked", "node_count": 0, "navigation_anchor_count": 0, "diagnostics": [blocked]},
        )
    try:
        parsed = parse_article(ingestion)
        parser_summary = {
            "status": "parsed",
            "element_count": len(parsed.elements),
            "validation_warning_count": len(parsed.validation_warnings),
            "parse_fallback": parsed.provenance.get("parse_fallback") == "true",
            "provenance": {
                "parser": parsed.provenance.get("parser"),
                "section_count": parsed.provenance.get("section_count"),
                "parse_fallback": parsed.provenance.get("parse_fallback"),
                "fallback_reason": parsed.provenance.get("fallback_reason"),
            },
            "element_ids_sha256": _sha256_text("\n".join(element.id for element in parsed.elements)),
            "diagnostics": [
                _diagnostic("PARSER_WARNING", "$.parser.validation_warnings", warning, severity="warning")
                for warning in parsed.validation_warnings
            ],
        }
        page_index = build_page_index_from_parsed(parsed)
        navigation_warnings = page_index.validate_navigation()
        page_summary = {
            "status": "indexed" if not navigation_warnings else "blocked",
            "node_count": len(page_index.nodes),
            "navigation_anchor_count": len(page_index.navigation_anchors),
            "root_id": page_index.root.id,
            "node_ids_sha256": _sha256_text("\n".join(node.id for node in page_index.nodes)),
            "provenance": {
                "page_index_builder": page_index.provenance.get("page_index_builder"),
                "node_count": page_index.provenance.get("node_count"),
                "navigation_anchor_count": page_index.provenance.get("navigation_anchor_count"),
            },
            "diagnostics": [
                _diagnostic("PAGE_INDEX_NAVIGATION_WARNING", "$.page_index", warning, severity="blocker")
                for warning in navigation_warnings
            ],
        }
        return parser_summary, page_summary
    except Exception as exc:  # noqa: BLE001 - converted to typed per-article blocker diagnostic.
        diagnostic = _diagnostic("PARSER_PAGE_INDEX_FAILED", "$.parser", repr(exc), severity="blocker")
        return (
            {"status": "blocked", "element_count": 0, "diagnostics": [diagnostic]},
            {"status": "blocked", "node_count": 0, "navigation_anchor_count": 0, "diagnostics": [diagnostic]},
        )


def _chunk_path(chunking_root: Path, article_ref: str) -> Path:
    slug = _article_slug(article_ref)
    candidates = [
        chunking_root / slug / "chunks.json",
        chunking_root / f"{slug}.json",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    raise BoundaryReplayError(
        f"missing local chunking artifact for {article_ref}; expected one of: "
        + ", ".join(str(candidate) for candidate in candidates)
    )


def _chunk_summary(chunking_root: Path, article_ref: str) -> dict[str, Any]:
    path = _chunk_path(chunking_root, article_ref)
    payload = _load_json(path)
    raw_chunks = payload.get("chunks") or payload.get("items") or []
    if not isinstance(raw_chunks, list):
        raise BoundaryReplayError(f"chunk artifact has non-list chunks/items: {path}")
    chunks = [chunk for chunk in raw_chunks if isinstance(chunk, dict)]
    diagnostics = payload.get("diagnostics") if isinstance(payload.get("diagnostics"), list) else []
    safe_diagnostics = [item for item in diagnostics if isinstance(item, dict)]
    if not chunks and not safe_diagnostics:
        safe_diagnostics.append(
            _diagnostic(
                "ZERO_CHUNKS_WITHOUT_DIAGNOSTIC",
                "$.chunks",
                "Chunk artifact contains zero chunks and no diagnostic explaining the state.",
                severity="blocker",
            )
        )
    return {
        "status": "summarized",
        "path": str(path),
        "chunk_count": len(chunks),
        "diagnostic_count": len(safe_diagnostics),
        "diagnostic_codes": [str(item.get("code") or "UNKNOWN") for item in safe_diagnostics],
        "chunk_ids_sha256": _sha256_text("\n".join(str(chunk.get("chunk_id") or chunk.get("id") or "") for chunk in chunks)),
        "diagnostics": safe_diagnostics,
    }


def _evidence_summary(evidence_root: Path, article_ref: str) -> dict[str, Any]:
    slug = _article_slug(article_ref)
    counts: dict[str, int] = {}
    diagnostic_counts: dict[str, int] = {}
    paths: dict[str, str] = {}
    diagnostics: list[dict[str, Any]] = []
    for evidence_type in EVIDENCE_TYPES:
        path = evidence_root / slug / f"{evidence_type}.json"
        if not path.exists():
            raise BoundaryReplayError(f"missing local evidence artifact for {article_ref}: {path}")
        payload = _load_json(path)
        paths[evidence_type] = str(path)
        summary = payload.get("summary") if isinstance(payload.get("summary"), dict) else {}
        counts[evidence_type] = int(summary.get("item_count") or 0)
        diagnostic_counts[evidence_type] = int(summary.get("diagnostic_count") or 0)
        flags = payload.get("safety_flags") if isinstance(payload.get("safety_flags"), dict) else {}
        if flags.get("raw_payloads_included") is True or flags.get("metadata_only") is False:
            diagnostics.append(
                _diagnostic(
                    "EVIDENCE_REDACTION_FLAG_UNSAFE",
                    f"$.evidence.{evidence_type}.safety_flags",
                    f"Evidence artifact {evidence_type} does not prove metadata-only redaction.",
                    severity="blocker",
                )
            )
    return {
        "status": "summarized",
        "paths": paths,
        "item_counts": counts,
        "diagnostic_counts": diagnostic_counts,
        "diagnostics": diagnostics,
    }


def _candidate_comparison_paths(root: Path, slug: str) -> list[Path]:
    return [root / slug / "boundary.json", root / slug / "final.json", root / f"{slug}.json"]


def _flat_metrics(artifact_metrics: dict[str, Any]) -> dict[str, int]:
    evidence_counts = artifact_metrics.get("evidence_counts") or artifact_metrics.get("evidence_item_counts") or {}
    if not isinstance(evidence_counts, dict):
        evidence_counts = {}
    return {
        "parser_element_count": int(artifact_metrics.get("parser_element_count") or 0),
        "page_index_node_count": int(artifact_metrics.get("page_index_node_count") or 0),
        "chunk_count": int(artifact_metrics.get("chunk_count") or 0),
        **{f"evidence_{key}_count": int(value or 0) for key, value in evidence_counts.items()},
    }


def _comparison(root: Path | None, slug: str, metrics: dict[str, Any], label: str) -> dict[str, Any]:
    if root is None:
        return {"label": label, "category": "not_applicable", "path": None, "metric_deltas": {}}
    if not root.exists():
        return {"label": label, "category": "baseline_missing", "path": str(root), "metric_deltas": {}}
    path = next((candidate for candidate in _candidate_comparison_paths(root, slug) if candidate.exists()), None)
    if path is None:
        return {"label": label, "category": "baseline_missing", "path": str(root), "metric_deltas": {}}
    payload = _load_json(path)
    baseline_metrics = payload.get("metrics") if isinstance(payload.get("metrics"), dict) else {}
    current = _flat_metrics(metrics)
    baseline = _flat_metrics(baseline_metrics)
    deltas = {key: current.get(key, 0) - baseline.get(key, 0) for key in sorted(set(current) | set(baseline))}
    return {
        "label": label,
        "category": "exact_match" if all(delta == 0 for delta in deltas.values()) else "metric_delta",
        "path": str(path),
        "metric_deltas": deltas,
    }


def _collect_diagnostics(*sections: dict[str, Any]) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    for section in sections:
        raw = section.get("diagnostics") if isinstance(section.get("diagnostics"), list) else []
        diagnostics.extend(item for item in raw if isinstance(item, dict))
    return diagnostics


def _sanitize_artifact(payload: dict[str, Any]) -> dict[str, Any]:
    clean = dict(payload)
    for section in ("loader", "parser", "page_index"):
        if isinstance(clean.get(section), dict):
            clean[section] = {key: value for key, value in clean[section].items() if key != "_ingestion"}
    return clean


def _contains_unsafe_payload_key(value: Any, path: str = "$") -> str | None:
    if isinstance(value, dict):
        for key, child in value.items():
            lowered = str(key).lower()
            if any(fragment in lowered for fragment in TEXT_KEY_FRAGMENTS) and key not in {
                "text_sha256",
                "source_type",
                "raw_article_text_included",
            }:
                return f"{path}.{key}"
            found = _contains_unsafe_payload_key(child, f"{path}.{key}")
            if found is not None:
                return found
    elif isinstance(value, list):
        for idx, child in enumerate(value):
            found = _contains_unsafe_payload_key(child, f"{path}[{idx}]")
            if found is not None:
                return found
    return None


def _artifact_for_article(
    *,
    args: argparse.Namespace,
    article: ArticleSelection,
    catalog_entry: dict[str, Any],
    roots: list[Path],
) -> dict[str, Any]:
    slug = _article_slug(article.article_ref)
    resolution = _resolve_local_source(catalog_entry, article, roots)
    loader = _loader_summary(article, resolution)
    parser_summary, page_summary = _parser_and_page_index_summaries(loader)
    chunking_root = getattr(args, "chunking", None) or (args.selection.parent / "chunking")
    evidence_root = getattr(args, "evidence", None) or (args.selection.parent / "evidence")
    chunking = _chunk_summary(chunking_root, article.article_ref)
    evidence = _evidence_summary(evidence_root, article.article_ref)
    metrics = {
        "loader_char_count": int(loader.get("char_count") or 0),
        "parser_element_count": int(parser_summary.get("element_count") or 0),
        "page_index_node_count": int(page_summary.get("node_count") or 0),
        "chunk_count": int(chunking.get("chunk_count") or 0),
        "evidence_counts": evidence["item_counts"],
        "evidence_diagnostic_counts": evidence["diagnostic_counts"],
    }
    comparisons = [
        _comparison(getattr(args, "baseline", None), slug, metrics, "baseline"),
        _comparison(getattr(args, "final_replay", None), slug, metrics, "final_replay"),
    ]
    diagnostics = _collect_diagnostics(loader, parser_summary, page_summary, chunking, evidence)
    for comparison in comparisons:
        if comparison["category"] == "baseline_missing":
            diagnostics.append(
                _diagnostic(
                    "COMPARISON_ARTIFACT_MISSING",
                    "$.comparisons",
                    f"{comparison['label']} comparison artifact is missing.",
                    severity="blocker",
                )
            )
    blocker_codes = [str(item.get("code") or "UNKNOWN") for item in diagnostics if item.get("severity") in {"error", "blocker"}]
    artifact = {
        "schema_version": ARTIFACT_SCHEMA_VERSION,
        "article_ref": article.article_ref,
        "source_code": article.source_code,
        "selection_role": article.selection_role,
        "catalog_ref": {
            "article_key": catalog_entry.get("article_key"),
            "article_path": catalog_entry.get("article_path"),
            "primary_source_role": catalog_entry.get("primary_source_role"),
            "title_sha256": _sha256_text(str(catalog_entry.get("title") or "")) if catalog_entry.get("title") else None,
        },
        "local_inputs": {
            "catalog": str(args.catalog),
            "index": str(args.index),
            "selection": str(args.selection),
            "source": loader.get("source_path"),
            "chunking": chunking["path"],
            "evidence": evidence["paths"],
        },
        "boundary_status": {
            "loader": loader["status"],
            "parser": parser_summary["status"],
            "page_index": page_summary["status"],
            "chunking": chunking["status"],
            "evidence": evidence["status"],
            "baseline": comparisons[0]["category"],
        },
        "loader": loader,
        "parser": parser_summary,
        "page_index": page_summary,
        "chunking": chunking,
        "evidence": evidence,
        "metrics": metrics,
        "comparisons": comparisons,
        "diagnostics": diagnostics,
        "redaction_checks": {"metadata_only": True, "raw_article_text_included": False},
        "provenance_coverage": {
            "source_path_recorded": loader.get("source_path") is not None,
            "chunking_path_recorded": True,
            "evidence_paths_recorded": set(evidence["paths"]) == set(EVIDENCE_TYPES),
            "comparison_paths_recorded": [comparison["path"] for comparison in comparisons],
        },
        "network": {"no_network_required": True, "network_fetch_attempted": False},
        "safety_state": {"metadata_only": True, "review_only": True, **FALSE_SAFETY_FLAGS},
        "readiness": {
            "boundary_replay_completed": not blocker_codes,
            "larger_preprocessing_validation_ready": not blocker_codes,
            "graph_readiness_claim": False,
            "blockers": blocker_codes,
        },
    }
    return _sanitize_artifact(artifact)


def run_replay(args: argparse.Namespace) -> list[dict[str, Any]]:
    if not getattr(args, "no_network", False):
        raise BoundaryReplayError("boundary replay requires --no-network so missing local artifacts fail closed")
    for label in ("catalog", "index", "selection", "boundary", "chunking", "evidence"):
        value = getattr(args, label, None)
        if value is not None:
            _reject_url_path(value, label)
    catalog = _load_json(args.catalog)
    index = _load_json(args.index)
    selection = _load_json(args.selection)
    catalog_refs = _catalog_by_ref(index)
    selected = _selection_articles(selection)
    roots = [args.selection.parent, args.catalog.parent, Path.cwd()]
    args.boundary.mkdir(parents=True, exist_ok=True)
    events = [
        _event(
            "boundary_replay.started",
            catalog_schema_version=catalog.get("schema_version"),
            index_schema_version=index.get("schema_version"),
            selection_id=selection.get("selection_id"),
            article_count=len(selected),
            no_network=True,
            network_fetch_attempted=False,
            graph_import_allowed=False,
            production_import_attempted=False,
            ladybugdb_written=False,
        )
    ]
    for article in selected:
        catalog_entry = catalog_refs.get(article.article_ref)
        if catalog_entry is None:
            raise BoundaryReplayError(f"selection article {article.article_ref} is absent from catalog index")
        slug = _article_slug(article.article_ref)
        artifact = _artifact_for_article(args=args, article=article, catalog_entry=catalog_entry, roots=roots)
        unsafe_path = _contains_unsafe_payload_key(artifact)
        if unsafe_path is not None:
            raise BoundaryReplayError(f"boundary artifact would include unsafe payload field at {unsafe_path}")
        output_path = args.boundary / slug / "boundary.json"
        _write_json(output_path, artifact)
        events.append(
            _event(
                "boundary_replay.article_completed",
                article_ref=article.article_ref,
                path=str(output_path),
                boundary_status=artifact["boundary_status"],
                diagnostic_count=len(artifact["diagnostics"]),
                readiness=artifact["readiness"],
                no_network=True,
                network_fetch_attempted=False,
                graph_import_allowed=False,
                production_import_attempted=False,
                ladybugdb_written=False,
            )
        )
    events.append(
        _event(
            "boundary_replay.completed",
            article_count=len(selected),
            boundary_path=str(args.boundary),
            no_network=True,
            network_fetch_attempted=False,
            graph_import_allowed=False,
            production_import_attempted=False,
            ladybugdb_written=False,
        )
    )
    return events


def _read_boundary_artifacts(boundary: Path) -> list[dict[str, Any]]:
    artifacts: list[dict[str, Any]] = []
    for path in sorted(boundary.glob("*/boundary.json")):
        payload = _load_json(path)
        payload["_path"] = str(path)
        artifacts.append(payload)
    if not artifacts:
        raise BoundaryReplayError(f"no boundary replay artifacts were found under {boundary}")
    return artifacts


def _events_from_path(path: Path) -> list[dict[str, Any]]:
    try:
        return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    except FileNotFoundError as exc:
        raise BoundaryReplayError(f"required local event log is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise BoundaryReplayError(f"event log is not valid JSONL: {path}: {exc}") from exc


def _summary_from_artifacts(args: argparse.Namespace, events: list[dict[str, Any]]) -> dict[str, Any]:
    artifacts = _read_boundary_artifacts(args.boundary)
    diagnostic_counts: dict[str, int] = {}
    comparison_counts: dict[str, int] = {}
    boundary_counts: dict[str, dict[str, int]] = {boundary: {} for boundary in BOUNDARY_TYPES}
    article_results: list[dict[str, Any]] = []
    findings: list[dict[str, Any]] = []
    safety_violations: list[dict[str, Any]] = []
    redaction_violations: list[dict[str, Any]] = []
    zero_chunk_violations: list[str] = []
    provenance_missing: list[str] = []

    for artifact in artifacts:
        article_ref = str(artifact.get("article_ref"))
        diagnostics = artifact.get("diagnostics") if isinstance(artifact.get("diagnostics"), list) else []
        for diagnostic in diagnostics:
            if isinstance(diagnostic, dict):
                code = str(diagnostic.get("code") or "UNKNOWN")
                diagnostic_counts[code] = diagnostic_counts.get(code, 0) + 1
                if diagnostic.get("severity") in {"error", "blocker"}:
                    findings.append({"article_ref": article_ref, **diagnostic})
        for comparison in artifact.get("comparisons", []):
            if isinstance(comparison, dict):
                category = str(comparison.get("category") or "unknown")
                comparison_counts[category] = comparison_counts.get(category, 0) + 1
        boundary_status = artifact.get("boundary_status") if isinstance(artifact.get("boundary_status"), dict) else {}
        for boundary, status in boundary_status.items():
            if boundary in boundary_counts:
                status_value = str(status)
                boundary_counts[boundary][status_value] = boundary_counts[boundary].get(status_value, 0) + 1
        safety_state = artifact.get("safety_state") if isinstance(artifact.get("safety_state"), dict) else {}
        violated = {key: safety_state.get(key) for key, expected in FALSE_SAFETY_FLAGS.items() if safety_state.get(key) is not expected}
        if violated:
            safety_violations.append({"article_ref": article_ref, "violations": violated})
        redaction = artifact.get("redaction_checks") if isinstance(artifact.get("redaction_checks"), dict) else {}
        unsafe_path = _contains_unsafe_payload_key({key: value for key, value in artifact.items() if key != "diagnostics"})
        if redaction.get("raw_article_text_included") is not False or unsafe_path is not None:
            redaction_violations.append({"article_ref": article_ref, "json_path": unsafe_path or "$.redaction_checks"})
        metrics = artifact.get("metrics") if isinstance(artifact.get("metrics"), dict) else {}
        if int(metrics.get("chunk_count") or 0) == 0 and not any(
            isinstance(item, dict) and item.get("code") == "ZERO_CHUNKS_WITHOUT_DIAGNOSTIC" for item in diagnostics
        ):
            zero_chunk_violations.append(article_ref)
        provenance = artifact.get("provenance_coverage") if isinstance(artifact.get("provenance_coverage"), dict) else {}
        if not provenance.get("chunking_path_recorded") or not provenance.get("evidence_paths_recorded"):
            provenance_missing.append(article_ref)
        readiness = artifact.get("readiness") if isinstance(artifact.get("readiness"), dict) else {}
        article_results.append(
            {
                "article_ref": article_ref,
                "path": artifact.get("_path"),
                "boundary_status": boundary_status,
                "diagnostic_count": len([item for item in diagnostics if isinstance(item, dict)]),
                "readiness": readiness,
                "metrics": metrics,
            }
        )

    no_network_proof = {
        "required": True,
        "network_fetch_attempted": any(event.get("network_fetch_attempted") is True for event in events),
        "all_events_no_network": all(event.get("no_network") is True for event in events if "no_network" in event),
    }
    blockers: list[str] = []
    if findings:
        blockers.append("boundary_diagnostics")
    if safety_violations:
        blockers.append("safety_flag_violation")
    if redaction_violations:
        blockers.append("redaction_violation")
    if zero_chunk_violations:
        blockers.append("zero_chunks_without_diagnostics")
    if provenance_missing:
        blockers.append("missing_provenance")
    if no_network_proof["network_fetch_attempted"]:
        blockers.append("network_fetch_attempted")
    if any(count for category, count in comparison_counts.items() if category == "baseline_missing"):
        blockers.append("comparison_artifact_missing")
    validation_passed = not blockers
    return {
        "schema_version": SCHEMA_VERSION,
        "summary_type": "m025-boundary-replay-completion-summary",
        "article_count": len(article_results),
        "boundary_path": str(args.boundary),
        "events_path": str(getattr(args, "events", getattr(args, "write_events", ""))),
        "boundary_counts": boundary_counts,
        "comparison_counts": comparison_counts,
        "diagnostic_counts": diagnostic_counts,
        "findings": findings,
        "article_results": article_results,
        "redaction_checks": {"passed": not redaction_violations, "violations": redaction_violations},
        "provenance_coverage": {"missing": provenance_missing},
        "zero_chunk_checks": {"violations": zero_chunk_violations},
        "no_network_proof": no_network_proof,
        "no_write_safety": {
            "safety_violations": safety_violations,
            "graph_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
        },
        "readiness": {
            "boundary_replay_completed": validation_passed,
            "larger_preprocessing_validation_ready": validation_passed,
            "decision": "ready" if validation_passed else "blocked",
            "blockers": blockers,
            "graph_readiness_claim": False,
        },
        "validation_passed": validation_passed,
    }


def _write_report(path: Path, summary: dict[str, Any]) -> None:
    rows = ["| Article | Loader | Parser | PageIndex | Chunks | Ready | Diagnostics |", "|---|---|---|---|---:|---|---:|"]
    for result in summary["article_results"]:
        status = result["boundary_status"]
        rows.append(
            "| {article_ref} | {loader} | {parser} | {page_index} | {chunks} | {ready} | {diagnostics} |".format(
                article_ref=result["article_ref"],
                loader=status.get("loader"),
                parser=status.get("parser"),
                page_index=status.get("page_index"),
                chunks=result["metrics"].get("chunk_count"),
                ready="yes" if result["readiness"].get("boundary_replay_completed") else "no",
                diagnostics=result["diagnostic_count"],
            )
        )
    blockers = summary["readiness"]["blockers"] or ["None"]
    report = f"""# M025 S10 Boundary Replay Completion Report

## Decision

- Boundary replay completed: **{str(summary['readiness']['boundary_replay_completed']).lower()}**
- Larger preprocessing validation ready: **{str(summary['readiness']['larger_preprocessing_validation_ready']).lower()}**
- Decision: **{summary['readiness']['decision']}**
- Blockers: {', '.join(blockers)}
- Graph readiness claim: **false**

This report only evaluates metadata-safe boundary completion over the fixed M025 smoke corpus. It does not claim graph readiness or import eligibility.

## Boundary Results

- Boundary path: `{summary['boundary_path']}`
- Boundary counts: `{json.dumps(summary['boundary_counts'], sort_keys=True)}`
- Comparison counts: `{json.dumps(summary['comparison_counts'], sort_keys=True)}`

{chr(10).join(rows)}

## Diagnostics

`{json.dumps(summary['diagnostic_counts'], sort_keys=True)}`

## Readiness Blockers

{chr(10).join(f'- {blocker}' for blocker in blockers)}

## Redaction Checks

`{json.dumps(summary['redaction_checks'], sort_keys=True)}`

## Provenance Coverage

`{json.dumps(summary['provenance_coverage'], sort_keys=True)}`

## No-Network Proof

`{json.dumps(summary['no_network_proof'], sort_keys=True)}`

## No-Import / No-Write Safety State

`{json.dumps(summary['no_write_safety'], sort_keys=True)}`

## Failure Modes

- Filesystem inputs: missing or malformed catalog, index, selection, chunking, evidence, event, or comparison JSON raises `BoundaryReplayError` and exits non-zero before claiming readiness.
- Network dependency: there is no network client; `--no-network` is required and missing local artifacts fail closed rather than fetching.
- Parser/PageIndex dependency: local parser or PageIndex exceptions become per-article blocker diagnostics unless unsafe payload keys would be written, which is a hard failure.
- Production graph writes/imports: artifacts carry false graph/import/write safety flags; validation blocks readiness on any violation.

## Load Profile

Expected load is the fixed five-article smoke corpus. At 10x, local JSON enumeration and report size saturate first; there is no network, subprocess, database, or graph writer pool. Protection is deterministic one-artifact-per-article processing with streaming JSONL events and bounded metadata summaries rather than raw article text.

## Negative Tests

- `tests/test_m025_boundary_replay_completion.py::test_boundary_replay_requires_no_network` covers fail-closed no-network enforcement.
- `tests/test_m025_boundary_replay_completion.py::test_boundary_replay_rejects_missing_local_evidence` covers missing local metadata artifacts.
- `tests/test_m025_boundary_replay_completion.py::test_malformed_selection_fails_before_writing_ready_summary` covers malformed selection schema.
- `tests/test_m025_boundary_replay_completion.py::test_validation_blocks_unsafe_safety_flags_redaction_graph_claim_and_zero_chunks` covers unsafe flags, redaction failure, graph readiness claims, and zero chunks without diagnostics.
"""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(report, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--boundary", dest="boundary", type=Path, help="Directory for per-article boundary replay artifacts.")
    parser.add_argument(
        "--boundary-replay",
        dest="boundary",
        type=Path,
        help="Compatibility alias for --boundary used by the S10 task/slice plans.",
    )
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--final-replay", type=Path)
    parser.add_argument("--chunking", type=Path, help="Directory containing per-article chunking artifacts.")
    parser.add_argument("--evidence", type=Path, help="Directory containing per-article evidence artifacts.")
    parser.add_argument("--write-events", type=Path)
    parser.add_argument("--events", type=Path, help="Read an existing boundary event log instead of rewriting it.")
    parser.add_argument("--no-network", action="store_true")
    parser.add_argument("--require-no-network", action="store_true")
    parser.add_argument("--require-no-import-flags", action="store_true")
    parser.add_argument("--require-redaction", action="store_true")
    parser.add_argument("--expect-article-count", type=int)
    parser.add_argument("--reject-zero-chunk-without-diagnostic", action="store_true")
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--write-summary", type=Path)
    parser.add_argument("--write-report", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.boundary is None:
        parser.error("the following arguments are required: --boundary (or --boundary-replay)")
    args.no_network = bool(args.no_network or args.require_no_network)
    try:
        if args.validate_only or (args.events is not None and args.write_events is None):
            events = _events_from_path(args.events) if args.events is not None else []
            _read_boundary_artifacts(args.boundary)
        else:
            events = run_replay(args)
            if args.write_events is not None:
                args.write_events.parent.mkdir(parents=True, exist_ok=True)
                args.write_events.write_text(
                    "".join(json.dumps(event, sort_keys=True) + "\n" for event in events), encoding="utf-8"
                )
        summary = None
        if args.write_summary is not None or args.write_report is not None or args.validate_only:
            summary = _summary_from_artifacts(args, events)
            if args.require_no_network and summary["no_network_proof"]["network_fetch_attempted"]:
                raise BoundaryReplayError("network fetch was attempted despite --require-no-network")
            if args.require_no_import_flags and summary["no_write_safety"]["safety_violations"]:
                raise BoundaryReplayError("boundary artifacts contain graph/import/write safety flag violations")
            if args.require_redaction and not summary["redaction_checks"]["passed"]:
                raise BoundaryReplayError("boundary artifacts failed metadata-only redaction checks")
            if args.expect_article_count is not None and summary["article_count"] != args.expect_article_count:
                raise BoundaryReplayError(
                    f"expected {args.expect_article_count} boundary articles, found {summary['article_count']}"
                )
            if args.reject_zero_chunk_without_diagnostic and summary["zero_chunk_checks"]["violations"]:
                raise BoundaryReplayError("one or more zero-chunk articles lacked an explicit diagnostic")
            if summary["readiness"]["blockers"]:
                raise BoundaryReplayError(
                    "boundary replay readiness is blocked: " + ", ".join(summary["readiness"]["blockers"])
                )
        if args.write_summary is not None and summary is not None:
            _write_json(args.write_summary, summary)
        if args.write_report is not None and summary is not None:
            _write_report(args.write_report, summary)
        completed = sum(1 for event in events if event.get("event_type") == "boundary_replay.article_completed")
        sys.stdout.write(
            f"wrote boundary replay completion for {completed} articles to {args.boundary}; "
            f"summary={args.write_summary}; report={args.write_report}\n"
        )
        return 0
    except BoundaryReplayError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
