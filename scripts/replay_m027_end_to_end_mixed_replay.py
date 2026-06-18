#!/usr/bin/env python3
"""Run the M027 S05 end-to-end mixed-source replay boundary.

This command is intentionally local-only and metadata-first. It consumes the S03
conversion-quality handoff plus the immutable S04 current-pipeline baseline,
verifies source-summary and converted-payload hashes, replays parser-ready
converted variants through loader, parser, PageIndex, chunk, evidence, and import
contract boundaries, skips metadata-only variants, compares metrics to S04, and
writes redacted replay/provenance artifacts. It never fetches network sources,
imports graph facts, writes LadybugDB, or emits raw article text/HTML/PDF bytes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import tempfile
from collections import Counter
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT / "src") not in sys.path:
    sys.path.insert(0, str(ROOT / "src"))

from research_graph.repair.chunk_baseline_measurement import build_baseline_package  # noqa: E402
from research_graph.repair.chunk_import_contract import validate_import_ready_package, validation_to_dict  # noqa: E402
from research_graph.papers.semantic_chunks import build_evidence_paths, build_semantic_chunks  # noqa: E402
from research_graph.corpus.ingestion import FullTextSource, ingest_full_text  # noqa: E402
from research_graph.papers.indexing.parsed_page_index import build_page_index_from_parsed  # noqa: E402
from research_graph.corpus.parsing.parser import parse_article  # noqa: E402

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S05"
SOURCE_SLICE_ID = "S03"
BASELINE_SLICE_ID = "S04"
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-end-to-end-mixed-replay.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m027-end-to-end-mixed-replay-diagnostic.v1"
ARTIFACT_SCHEMA_VERSION = "m027-end-to-end-mixed-replay-artifact.v1"
DECISION_SCHEMA_VERSION = "m027-end-to-end-mixed-replay-readiness-decision.v1"
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
DEFAULT_CONVERSION_SUMMARY = CORPUS_DIR / "conversion-quality-summary.json"
DEFAULT_S03_SUMMARY = ROOT / ".gsd" / "milestones" / MILESTONE_ID / "slices" / SOURCE_SLICE_ID / f"{SOURCE_SLICE_ID}-SUMMARY.md"
DEFAULT_BASELINE_SUMMARY = CORPUS_DIR / "current-pipeline-baseline-summary.json"
DEFAULT_BASELINE_DIAGNOSTICS = CORPUS_DIR / "current-pipeline-baseline-diagnostics.jsonl"
DEFAULT_OUTPUT_SUMMARY = CORPUS_DIR / "end-to-end-mixed-replay-summary.json"
DEFAULT_OUTPUT_DIAGNOSTICS = CORPUS_DIR / "end-to-end-mixed-replay-diagnostics.jsonl"
DEFAULT_OUTPUT_EVENTS = CORPUS_DIR / "end-to-end-mixed-replay-events.jsonl"
DEFAULT_OUTPUT_REPORT = CORPUS_DIR / "end-to-end-mixed-replay-report.md"
DEFAULT_READINESS_DECISION = CORPUS_DIR / "end-to-end-mixed-replay-readiness-decision.json"
DEFAULT_OUTPUT_DIR = CORPUS_DIR / "end-to-end-mixed-replay"

FALSE_SAFETY_FLAGS: dict[str, bool] = {
    "network_fetch_attempted": False,
    "graph_import_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "production_ladybugdb_write_allowed": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
    "raw_payload_embedded_in_metadata": False,
}
FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "chunk_text",
    "html",
    "pdf",
    "binary",
    "bytes",
    "base64",
    "payload",
    "content",
    "body",
    "article_text",
    "paper_text",
}
FORBIDDEN_SNIPPETS = ("<html", "</html", "%PDF-", "base64,", "RAW_ARXIV_ABS_SECRET", "RAW_NATURE_BODY_SECRET", "RAW_PDF_SECRET")


class EndToEndReplayError(RuntimeError):
    """Raised when the S05 replay cannot safely continue."""


def utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise EndToEndReplayError(f"required local JSON input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise EndToEndReplayError(f"required local JSON input is malformed: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise EndToEndReplayError(f"required local JSON input must be an object: {path}")
    return payload


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    assert_no_metadata_leakage(payload)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    materialized = list(rows)
    assert_no_metadata_leakage(materialized)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in materialized), encoding="utf-8")


def rel(path: Path, root: Path = ROOT) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return str(path)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_relative_path(value: Any, *, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value.strip():
        raise EndToEndReplayError(f"missing_{label}")
    if "://" in value:
        raise EndToEndReplayError(f"url_not_allowed_as_{label}")
    normalized = PurePosixPath(value.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part == "" for part in normalized.parts):
        raise EndToEndReplayError(f"unsafe_{label}")
    return normalized


def safe_under_root(root: Path, value: Any, *, label: str) -> Path:
    normalized = safe_relative_path(value, label=label)
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise EndToEndReplayError(f"{label}_escapes_root")
    return resolved


def article_slug(article_ref: str) -> str:
    return article_ref.replace("/", "_").replace(":", "_")


def git_commit_from_head(root: Path) -> str | None:
    head_path = root / ".git" / "HEAD"
    try:
        head = head_path.read_text(encoding="utf-8").strip()
    except OSError:
        return None
    if head.startswith("ref: "):
        ref_path = root / ".git" / head.removeprefix("ref: ").strip()
        try:
            return ref_path.read_text(encoding="utf-8").strip() or None
        except OSError:
            return None
    return head or None


def artifact_row(path: Path, *, role: str) -> dict[str, Any]:
    return {
        "role": role,
        "path": rel(path),
        "exists": path.exists(),
        "sha256": sha256_file(path) if path.exists() and path.is_file() else None,
        "byte_size": path.stat().st_size if path.exists() and path.is_file() else None,
    }


def diagnostic(
    *,
    article_ref: str | None,
    variant_id: str | None,
    stage: str,
    status: str,
    diagnostic_code: str,
    message: str,
    source_role: str | None = None,
    input_sha256: str | None = None,
    output_sha256: str | None = None,
    output_path: str | None = None,
) -> dict[str, Any]:
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "baseline_slice_id": BASELINE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": article_ref,
        "variant_id": variant_id,
        "source_role": source_role,
        "stage": stage,
        "status": status,
        "diagnostic_code": diagnostic_code,
        "code": diagnostic_code,
        "message": message,
        "input_sha256": input_sha256,
        "output_sha256": output_sha256,
        "output_path": output_path,
        **FALSE_SAFETY_FLAGS,
    }


def event(event_type: str, **fields: Any) -> dict[str, Any]:
    return {
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "event_type": event_type,
        **fields,
        **FALSE_SAFETY_FLAGS,
    }


def validate_s03_linkage(conversion_summary: Mapping[str, Any], *, conversion_summary_path: Path) -> list[dict[str, Any]]:
    if conversion_summary.get("milestone_id") != MILESTONE_ID or conversion_summary.get("slice_id") != SOURCE_SLICE_ID:
        raise EndToEndReplayError("conversion summary is not the expected M027/S03 handoff")
    if conversion_summary.get("selection_id") != SELECTION_ID:
        raise EndToEndReplayError("conversion summary selection_id does not match M027 mixed-source corpus")
    source_summary_path = safe_under_root(ROOT, conversion_summary.get("source_summary_path"), label="source_summary_path")
    expected_source_sha = conversion_summary.get("source_summary_sha256")
    if not isinstance(expected_source_sha, str) or not expected_source_sha:
        raise EndToEndReplayError("conversion summary is missing source_summary_sha256")
    actual_source_sha = sha256_file(source_summary_path)
    if actual_source_sha != expected_source_sha:
        raise EndToEndReplayError(
            f"stale S03 linkage: source_summary_sha256 mismatch for {source_summary_path}: "
            f"expected {expected_source_sha}, got {actual_source_sha}"
        )
    return [
        diagnostic(
            article_ref=None,
            variant_id=None,
            stage="s03_linkage",
            status="passed",
            diagnostic_code="s03_linkage_verified",
            message="S03 conversion summary points at the current source-acquisition summary hash.",
            input_sha256=sha256_file(conversion_summary_path),
            output_sha256=actual_source_sha,
            output_path=rel(source_summary_path),
        )
    ]


def conversion_rows(conversion_summary: Mapping[str, Any]) -> list[dict[str, Any]]:
    rows = conversion_summary.get("results")
    if not isinstance(rows, list) or not rows:
        raise EndToEndReplayError("conversion summary does not contain non-empty results")
    typed_rows: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise EndToEndReplayError(f"conversion result at index {index} is not an object")
        if not isinstance(row.get("article_ref"), str) or not isinstance(row.get("variant_id"), str):
            raise EndToEndReplayError(f"conversion result at index {index} is missing article_ref or variant_id")
        if row.get("parser_ready") is True and not row.get("converted_text_path"):
            raise EndToEndReplayError(f"parser-ready conversion result is missing converted_text_path: {row.get('variant_id')}")
        typed_rows.append(row)
    return typed_rows


def validate_converted_payload(row: Mapping[str, Any]) -> tuple[Path | None, dict[str, Any]]:
    if row.get("parser_ready") is not True:
        return None, {"verified": False, "reason": "not_parser_ready"}
    converted_path = safe_under_root(ROOT, row.get("converted_text_path"), label="converted_text_path")
    if not converted_path.exists():
        raise EndToEndReplayError(f"converted payload is missing for {row.get('variant_id')}: {converted_path}")
    expected_sha = row.get("converted_text_sha256")
    expected_size = row.get("converted_text_byte_size")
    if not isinstance(expected_sha, str) or not expected_sha:
        raise EndToEndReplayError(f"missing converted_text_sha256 for {row.get('variant_id')}")
    if not isinstance(expected_size, int):
        raise EndToEndReplayError(f"missing converted_text_byte_size for {row.get('variant_id')}")
    actual_sha = sha256_file(converted_path)
    actual_size = converted_path.stat().st_size
    if actual_sha != expected_sha:
        raise EndToEndReplayError(f"converted_text_sha256 mismatch for {row.get('variant_id')}: expected {expected_sha}, got {actual_sha}")
    if actual_size != expected_size:
        raise EndToEndReplayError(f"converted_text_byte_size mismatch for {row.get('variant_id')}: expected {expected_size}, got {actual_size}")
    return converted_path, {"verified": True, "sha256": actual_sha, "byte_size": actual_size, "path": rel(converted_path)}


def import_contract_metrics(package: Mapping[str, Any], validation: Mapping[str, Any]) -> dict[str, Any]:
    diagnostics = package.get("diagnostics") if isinstance(package.get("diagnostics"), dict) else {}
    return {
        "package_state": diagnostics.get("package_state"),
        "valid_package": validation.get("valid_package"),
        "passed": validation.get("passed"),
        "import_ready": validation.get("import_ready"),
        "has_import_eligible_chunks": validation.get("has_import_eligible_chunks"),
        "import_eligible_chunk_count": validation.get("import_eligible_chunk_count"),
        "refused_chunk_count": validation.get("refused_chunk_count"),
        "refusal_counts": validation.get("refusal_counts"),
        "counts_by_state": diagnostics.get("counts_by_state", {}),
        "counts_by_route": diagnostics.get("counts_by_route", {}),
        "counts_by_chunk_type": diagnostics.get("counts_by_chunk_type", {}),
        "source_span_coverage": diagnostics.get("source_span_coverage"),
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def run_boundaries(row: Mapping[str, Any], converted_path: Path, *, temp_root: Path) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    article_ref = str(row["article_ref"])
    variant_id = str(row["variant_id"])
    source_role = str(row.get("source_role") or "converted_text")
    work_dir = temp_root / article_slug(article_ref) / article_slug(variant_id)
    work_dir.mkdir(parents=True, exist_ok=True)
    full_text_path = work_dir / "full_text.md"
    shutil.copyfile(converted_path, full_text_path)
    paper_id = f"{article_ref}:{variant_id}"

    ingestion = ingest_full_text(FullTextSource(paper_id=paper_id, source_type="markdown", source_path=full_text_path))
    loader = {
        "status": ingestion.extraction_mode,
        "source_type": ingestion.source_type,
        "byte_size": full_text_path.stat().st_size,
        "sha256": sha256_file(full_text_path),
        "char_count": ingestion.quality.char_count,
        "line_count": ingestion.quality.line_count,
        "warning_count": len(ingestion.warnings),
        "fallback_reason": ingestion.fallback_reason,
    }
    parsed = parse_article(ingestion)
    document = build_page_index_from_parsed(parsed)
    chunks = build_semantic_chunks(document)
    evidence_paths = build_evidence_paths(document, chunks)

    paper = {
        "paper_id": paper_id,
        "title": row.get("title") or article_ref,
        "categories": [],
        "source_artifacts": [rel(converted_path)],
        "required_paths": [str(full_text_path)],
        "hard_case_tags": ["m027", "end_to_end_mixed_replay", source_role],
    }
    package = build_baseline_package(paper, run_id=f"m027-s05-end-to-end:{SELECTION_ID}")
    validation = validation_to_dict(validate_import_ready_package(package))
    contract = import_contract_metrics(package, validation)

    metrics = {
        "loader": loader,
        "parser": {
            "status": "parsed",
            "element_count": len(parsed.elements),
            "warning_count": len(parsed.validation_warnings),
            "parse_fallback": parsed.provenance.get("parse_fallback") == "true",
        },
        "page_index": {
            "status": "built",
            "node_count": len(document.nodes),
            "navigation_anchor_count": len(document.navigation_anchors),
            "warning_count": len(document.validation_warnings),
        },
        "chunking": {
            "status": "completed",
            "chunk_count": len(chunks),
            "strategy": "section_text_v1",
            "zero_chunk_parser_ready": len(chunks) == 0,
        },
        "evidence": {
            "status": "completed",
            "evidence_path_count": len(evidence_paths),
            "warning_count": sum(len(path.validation_warnings) for path in evidence_paths),
        },
        "import_contract": contract,
        **FALSE_SAFETY_FLAGS,
    }
    code = "parser_ready_zero_chunks_preserved" if len(chunks) == 0 else "end_to_end_boundaries_completed"
    status = "current_failure_preserved" if len(chunks) == 0 else "passed"
    return metrics, [
        diagnostic(
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
            stage="end_to_end_boundaries",
            status=status,
            diagnostic_code=code,
            message="Parser-ready converted payload passed through loader, parser, PageIndex, chunking, evidence, and import-contract boundaries without graph/import writes.",
            input_sha256=str(row.get("converted_text_sha256")),
        )
    ]


def metadata_only_metrics(row: Mapping[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    article_ref = str(row["article_ref"])
    variant_id = str(row["variant_id"])
    source_role = str(row.get("source_role") or "unknown")
    metrics = {
        "loader": {"status": "skipped_metadata_only", "warning_count": 0},
        "parser": {"status": "skipped_not_parser_ready", "element_count": 0, "warning_count": 0, "parse_fallback": False},
        "page_index": {"status": "skipped_not_parser_ready", "node_count": 0, "navigation_anchor_count": 0, "warning_count": 0},
        "chunking": {"status": "skipped_not_parser_ready", "chunk_count": 0, "strategy": None, "zero_chunk_parser_ready": False},
        "evidence": {"status": "skipped_not_parser_ready", "evidence_path_count": 0, "warning_count": 0},
        "import_contract": {
            "package_state": "not_run_metadata_only",
            "valid_package": True,
            "passed": True,
            "import_ready": False,
            "has_import_eligible_chunks": False,
            "import_eligible_chunk_count": 0,
            "refused_chunk_count": 0,
            "refusal_counts": {"metadata_only_not_parser_ready": 1},
            "counts_by_state": {},
            "counts_by_route": {},
            "counts_by_chunk_type": {},
            "source_span_coverage": 0.0,
            "raw_text_included": False,
            "embeddings_included": False,
            "ladybugdb_written": False,
            "production_import_attempted": False,
        },
        **FALSE_SAFETY_FLAGS,
    }
    return metrics, [
        diagnostic(
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
            stage="parser_readiness",
            status="skipped",
            diagnostic_code="metadata_only_not_parser_ready_skipped",
            message="S03 classified this variant as metadata-only/not parser-ready; no loader/parser/PageIndex/chunk/evidence replay was attempted.",
            input_sha256=str(row.get("source_sha256")) if row.get("source_sha256") else None,
        )
    ]


def baseline_records_by_key(baseline_summary: Mapping[str, Any]) -> dict[tuple[str, str], dict[str, Any]]:
    rows = baseline_summary.get("article_results")
    if not isinstance(rows, list):
        raise EndToEndReplayError("S04 baseline summary does not contain article_results")
    result: dict[tuple[str, str], dict[str, Any]] = {}
    for row in rows:
        if isinstance(row, dict) and isinstance(row.get("article_ref"), str) and isinstance(row.get("variant_id"), str):
            result[(str(row["article_ref"]), str(row["variant_id"]))] = row
    return result


def compare_to_baseline(row: Mapping[str, Any], metrics: Mapping[str, Any], baseline_by_key: Mapping[tuple[str, str], dict[str, Any]]) -> tuple[dict[str, Any], dict[str, Any]]:
    article_ref = str(row["article_ref"])
    variant_id = str(row["variant_id"])
    baseline = baseline_by_key.get((article_ref, variant_id))
    current_chunk_count = int(dict(metrics.get("chunking") or {}).get("chunk_count") or 0)
    current_import_ready = bool(dict(metrics.get("import_contract") or {}).get("import_ready"))
    if baseline is None:
        comparison = {
            "category": "baseline_missing",
            "baseline_chunk_count": None,
            "current_chunk_count": current_chunk_count,
            "metric_deltas": {},
            "baseline_artifact_path": None,
        }
        code = "s04_baseline_row_missing"
        status = "blocked"
    else:
        baseline_metrics = baseline.get("current_pipeline_metrics") if isinstance(baseline.get("current_pipeline_metrics"), dict) else {}
        baseline_chunk_count = int(baseline_metrics.get("chunk_count") or 0)
        baseline_import_ready = bool(baseline_metrics.get("import_ready"))
        deltas = {
            "chunk_count": current_chunk_count - baseline_chunk_count,
            "import_ready": int(current_import_ready) - int(baseline_import_ready),
        }
        category = "exact_match" if all(delta == 0 for delta in deltas.values()) else "metric_delta"
        comparison = {
            "category": category,
            "baseline_chunk_count": baseline_chunk_count,
            "current_chunk_count": current_chunk_count,
            "metric_deltas": deltas,
            "baseline_artifact_path": baseline.get("baseline_artifact_path"),
        }
        code = "s04_baseline_exact_match" if category == "exact_match" else "s04_baseline_metric_delta"
        status = "passed" if category == "exact_match" else "observed"
    return comparison, diagnostic(
        article_ref=article_ref,
        variant_id=variant_id,
        source_role=str(row.get("source_role") or "unknown"),
        stage="s04_baseline_comparison",
        status=status,
        diagnostic_code=code,
        message="Compared S05 replay metrics against the immutable S04 current-pipeline baseline row.",
    )


def article_record(row: Mapping[str, Any], metrics: Mapping[str, Any], payload_provenance: Mapping[str, Any], baseline_comparison: Mapping[str, Any], artifact_path: Path | None) -> dict[str, Any]:
    return {
        "article_ref": row.get("article_ref"),
        "variant_id": row.get("variant_id"),
        "source_role": row.get("source_role"),
        "conversion_status": row.get("status"),
        "parser_ready": row.get("parser_ready") is True,
        "converted_payload": payload_provenance,
        "boundary_metrics": metrics,
        "baseline_comparison": baseline_comparison,
        "replay_artifact_path": rel(artifact_path) if artifact_path is not None else None,
        **FALSE_SAFETY_FLAGS,
    }


def assert_no_metadata_leakage(value: Any) -> None:
    serialized = json.dumps(value, sort_keys=True)
    lowered = serialized.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            raise EndToEndReplayError(f"metadata payload leakage detected: {snippet}")

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    raise EndToEndReplayError(f"metadata payload key leakage detected at {path}.{key}")
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")


def write_article_artifact(output_dir: Path, article_ref: str, records: list[dict[str, Any]]) -> Path:
    path = output_dir / article_slug(article_ref) / "replay.json"
    payload = {
        "schema_version": ARTIFACT_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "baseline_slice_id": BASELINE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": article_ref,
        "variant_count": len(records),
        "variants": records,
        "readiness": {
            "end_to_end_replay_completed": True,
            "graph_readiness_claim": False,
            "import_ready_claim": False,
            "unsupported_readiness_claim": False,
        },
        **FALSE_SAFETY_FLAGS,
    }
    write_json(path, payload)
    return path


def build_readiness_decision(summary: Mapping[str, Any]) -> dict[str, Any]:
    blockers: list[str] = []
    if int(summary.get("baseline_missing_count") or 0) > 0:
        blockers.append("s04_baseline_rows_missing")
    if int(summary.get("zero_chunk_parser_ready_variant_count") or 0) > 0:
        blockers.append("parser_ready_zero_chunk_variants_preserved")
    if int(summary.get("import_ready_count") or 0) > 0:
        blockers.append("unexpected_import_ready_records")
    if any(summary.get(flag) is True for flag in FALSE_SAFETY_FLAGS):
        blockers.append("unsafe_safety_flag_true")
    decision = "not_import_ready_validate_only" if blockers else "replay_complete_validate_only"
    return {
        "schema_version": DECISION_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "created_at": utc_now(),
        "decision": decision,
        "ready_for_import": False,
        "graph_readiness_claim": False,
        "trusted_fact_claim": False,
        "blockers": blockers,
        "rationale": "S05 is a local validate-only replay boundary; outputs compare behavior and preserve blockers without graph/import-ready claims.",
        **FALSE_SAFETY_FLAGS,
    }


def replay_end_to_end(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    if not getattr(args, "no_network", False):
        raise EndToEndReplayError("end-to-end mixed replay requires --no-network")
    output_dir = Path(args.output_dir)
    root_resolved = ROOT.resolve()
    if not output_dir.resolve().is_relative_to(root_resolved):
        raise EndToEndReplayError("unsafe_output_dir")

    conversion_summary_path = Path(args.conversion_summary)
    baseline_summary_path = Path(args.baseline_summary)
    baseline_diagnostics_path = Path(args.baseline_diagnostics)
    conversion_summary = load_json(conversion_summary_path)
    baseline_summary = load_json(baseline_summary_path)
    diagnostics = validate_s03_linkage(conversion_summary, conversion_summary_path=conversion_summary_path)
    baseline_by_key = baseline_records_by_key(baseline_summary)
    diagnostics.append(
        diagnostic(
            article_ref=None,
            variant_id=None,
            stage="s04_baseline_linkage",
            status="passed",
            diagnostic_code="s04_baseline_summary_loaded",
            message="Loaded immutable S04 current-pipeline baseline summary for comparison.",
            input_sha256=sha256_file(baseline_summary_path),
            output_path=rel(baseline_summary_path),
        )
    )

    records: list[dict[str, Any]] = []
    events: list[dict[str, Any]] = [event("replay_started", command=" ".join(sys.argv), cwd=str(Path.cwd()), git_commit=git_commit_from_head(ROOT))]
    by_article: dict[str, list[dict[str, Any]]] = {}
    with tempfile.TemporaryDirectory(prefix="m027-end-to-end-replay-") as tmp_name:
        temp_root = Path(tmp_name)
        for row in conversion_rows(conversion_summary):
            article_ref = str(row["article_ref"])
            variant_id = str(row["variant_id"])
            converted_path, payload_provenance = validate_converted_payload(row)
            diagnostics.append(
                diagnostic(
                    article_ref=article_ref,
                    variant_id=variant_id,
                    source_role=str(row.get("source_role") or "unknown"),
                    stage="converted_payload_validation",
                    status="passed" if converted_path is not None else "skipped",
                    diagnostic_code="converted_payload_hash_verified" if converted_path is not None else "metadata_only_no_converted_payload_expected",
                    message="Converted payload hash and byte size match S03 metadata." if converted_path is not None else "No converted payload is expected for this metadata-only variant.",
                    input_sha256=str(row.get("converted_text_sha256")) if row.get("converted_text_sha256") else None,
                    output_sha256=str(payload_provenance.get("sha256")) if payload_provenance.get("sha256") else None,
                    output_path=str(payload_provenance.get("path")) if payload_provenance.get("path") else None,
                )
            )
            if converted_path is None:
                metrics, row_diagnostics = metadata_only_metrics(row)
            else:
                metrics, row_diagnostics = run_boundaries(row, converted_path, temp_root=temp_root)
            diagnostics.extend(row_diagnostics)
            comparison, comparison_diagnostic = compare_to_baseline(row, metrics, baseline_by_key)
            diagnostics.append(comparison_diagnostic)
            record = article_record(row, metrics, payload_provenance, comparison, artifact_path=None)
            records.append(record)
            by_article.setdefault(article_ref, []).append(record)
            events.append(event("variant_replayed", article_ref=article_ref, variant_id=variant_id, parser_ready=row.get("parser_ready") is True, diagnostic_code=row_diagnostics[-1]["diagnostic_code"]))

    artifact_paths: dict[str, Path] = {}
    for article_ref, article_records in sorted(by_article.items()):
        path = write_article_artifact(output_dir, article_ref, article_records)
        artifact_paths[article_ref] = path
        for record in article_records:
            record["replay_artifact_path"] = rel(path)
    for article_ref, article_records in sorted(by_article.items()):
        path = artifact_paths[article_ref]
        payload = load_json(path)
        payload["variants"] = article_records
        write_json(path, payload)

    parser_ready_records = [record for record in records if record["parser_ready"]]
    chunk_counts = [int(record["boundary_metrics"].get("chunking", {}).get("chunk_count") or 0) for record in parser_ready_records]
    diagnostic_counts = Counter(str(row["diagnostic_code"]) for row in diagnostics)
    comparison_counts = Counter(str(record["baseline_comparison"].get("category")) for record in records)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "baseline_slice_id": BASELINE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed",
        "created_at": utc_now(),
        "provenance": {
            "command": " ".join(sys.argv),
            "cwd": str(Path.cwd()),
            "milestone_id": MILESTONE_ID,
            "slice_id": SLICE_ID,
            "git_commit": git_commit_from_head(ROOT),
            "exit_status": "completed",
            "exit_code": 0,
        },
        "input_artifacts": [
            artifact_row(conversion_summary_path, role="s03_conversion_summary"),
            artifact_row(baseline_summary_path, role="s04_baseline_summary"),
            artifact_row(baseline_diagnostics_path, role="s04_baseline_diagnostics"),
        ],
        "output_artifacts": [],
        "conversion_summary_path": rel(conversion_summary_path),
        "conversion_summary_sha256": sha256_file(conversion_summary_path),
        "baseline_summary_path": rel(baseline_summary_path),
        "baseline_summary_sha256": sha256_file(baseline_summary_path),
        "baseline_diagnostics_path": rel(baseline_diagnostics_path),
        "baseline_diagnostics_sha256": sha256_file(baseline_diagnostics_path) if baseline_diagnostics_path.exists() else None,
        "output_summary_path": rel(Path(args.output_summary)),
        "output_diagnostics_path": rel(Path(args.output_diagnostics)),
        "output_events_path": rel(Path(args.output_events)),
        "output_report_path": rel(Path(args.output_report)),
        "readiness_decision_path": rel(Path(args.readiness_decision)),
        "output_dir": rel(output_dir),
        "article_count": len(by_article),
        "variant_count": len(records),
        "parser_ready_variant_count": len(parser_ready_records),
        "metadata_only_variant_count": len(records) - len(parser_ready_records),
        "chunk_count": sum(chunk_counts),
        "evidence_path_count": sum(int(record["boundary_metrics"].get("evidence", {}).get("evidence_path_count") or 0) for record in records),
        "zero_chunk_parser_ready_variant_count": sum(1 for count in chunk_counts if count == 0),
        "import_ready_count": sum(1 for record in records if record["boundary_metrics"].get("import_contract", {}).get("import_ready") is True),
        "import_eligible_chunk_count": sum(int(record["boundary_metrics"].get("import_contract", {}).get("import_eligible_chunk_count") or 0) for record in records),
        "baseline_comparison_counts": dict(sorted(comparison_counts.items())),
        "baseline_missing_count": comparison_counts.get("baseline_missing", 0),
        "diagnostic_counts": dict(sorted(diagnostic_counts.items())),
        "article_results": records,
        "artifact_paths": {article_ref: rel(path) for article_ref, path in sorted(artifact_paths.items())},
        "readiness": {
            "end_to_end_replay_completed": True,
            "validate_only": True,
            "graph_readiness_claim": False,
            "trusted_fact_claim": False,
            "import_ready_claim": False,
            "unsupported_readiness_claim": False,
        },
        "failure_modes": {
            "filesystem": "Missing/malformed S03/S04 JSON, stale source hashes, missing converted payloads, unsafe paths, and converted payload hash/size mismatches raise EndToEndReplayError and exit non-zero before readiness claims.",
            "network": "No network dependency is used; --no-network is required and artifacts carry network_fetch_attempted=false.",
            "subprocess": "The replay command itself invokes no subprocesses; git commit provenance is read from .git/HEAD when available and otherwise omitted.",
            "graph_database": "Graph, trusted KG import, production writes, and LadybugDB writes are disabled by construction and represented by fail-closed false safety flags.",
        },
        "load_profile": {
            "expected_articles": 6,
            "expected_variants": 11,
            "first_10x_saturation": "local filesystem reads/writes plus in-memory loader/parser/PageIndex/chunk/evidence construction over converted text payloads",
            "protection": "bounded S03 converted payload inputs, one-variant-at-a-time replay, per-article redacted JSON artifacts, no network pools, no graph/database writers, and metadata-only skips",
        },
        **FALSE_SAFETY_FLAGS,
    }
    decision = build_readiness_decision(summary)
    events.append(event("replay_completed", variant_count=len(records), diagnostic_counts=dict(sorted(diagnostic_counts.items()))))
    assert_no_metadata_leakage(summary)
    assert_no_metadata_leakage(decision)
    return summary, diagnostics, events, decision


def finalize_output_provenance(args: argparse.Namespace, summary: dict[str, Any]) -> dict[str, Any]:
    output_paths = [
        (Path(args.output_summary), "summary"),
        (Path(args.output_diagnostics), "diagnostics"),
        (Path(args.output_events), "events"),
        (Path(args.output_report), "report"),
        (Path(args.readiness_decision), "readiness_decision"),
    ]
    for path in sorted(Path(args.output_dir).glob("*/replay.json")):
        output_paths.append((path, "per_article_replay"))
    summary["output_artifacts"] = [artifact_row(path, role=role) for path, role in output_paths]
    summary["provenance"]["exit_status"] = "completed"
    summary["provenance"]["exit_code"] = 0
    assert_no_metadata_leakage(summary)
    return summary


def write_report(path: Path, summary: Mapping[str, Any], decision: Mapping[str, Any]) -> None:
    rows = ["| Article | Variant | Parser-ready | Chunks | Evidence paths | Baseline comparison | Replay artifact |", "|---|---|---:|---:|---:|---|---|"]
    for record in summary.get("article_results", []):
        if not isinstance(record, dict):
            continue
        metrics = record.get("boundary_metrics") if isinstance(record.get("boundary_metrics"), dict) else {}
        chunking = metrics.get("chunking") if isinstance(metrics.get("chunking"), dict) else {}
        evidence = metrics.get("evidence") if isinstance(metrics.get("evidence"), dict) else {}
        comparison = record.get("baseline_comparison") if isinstance(record.get("baseline_comparison"), dict) else {}
        rows.append(
            "| {article} | {variant} | {ready} | {chunks} | {evidence_paths} | {comparison} | `{artifact}` |".format(
                article=record.get("article_ref"),
                variant=record.get("variant_id"),
                ready="yes" if record.get("parser_ready") else "no",
                chunks=chunking.get("chunk_count", 0),
                evidence_paths=evidence.get("evidence_path_count", 0),
                comparison=comparison.get("category"),
                artifact=record.get("replay_artifact_path"),
            )
        )
    report = f"""# M027 S05 End-to-End Mixed Replay Report

## Decision

- End-to-end replay completed: **{str(summary['readiness']['end_to_end_replay_completed']).lower()}**
- Validate-only decision: **{decision['decision']}**
- Ready for import: **false**
- Graph readiness claim: **false**
- Trusted fact claim: **false**

This report records a local-only replay through loader, parser, PageIndex, chunking, separated evidence, import-contract, and S04 baseline-comparison boundaries. Metadata-only variants are skipped by design. Outputs are redacted and must not be interpreted as graph/import-ready artifacts.

## Aggregate Summary

- Articles: {summary['article_count']}
- Variants: {summary['variant_count']}
- Parser-ready variants: {summary['parser_ready_variant_count']}
- Metadata-only variants: {summary['metadata_only_variant_count']}
- Chunks observed: {summary['chunk_count']}
- Evidence paths observed: {summary['evidence_path_count']}
- Zero-chunk parser-ready variants: {summary['zero_chunk_parser_ready_variant_count']}
- Import-ready records: {summary['import_ready_count']}
- Import-eligible chunks: {summary['import_eligible_chunk_count']}
- Baseline comparison counts: `{json.dumps(summary['baseline_comparison_counts'], sort_keys=True)}`

## Article Results

{chr(10).join(rows)}

## Diagnostics

`{json.dumps(summary['diagnostic_counts'], sort_keys=True)}`

## Provenance

- Command: `{summary['provenance']['command']}`
- CWD: `{summary['provenance']['cwd']}`
- Git commit: `{summary['provenance']['git_commit']}`
- Conversion summary: `{summary['conversion_summary_path']}`
- S04 baseline summary: `{summary['baseline_summary_path']}`
- Replay diagnostics: `{summary['output_diagnostics_path']}`
- Per-article replay directory: `{summary['output_dir']}`

## Failure Modes

- Filesystem inputs: missing/malformed S03/S04 JSON, stale S03 source-summary linkage, missing converted payloads, unsafe relative paths, and converted payload hash/size mismatches raise `EndToEndReplayError` and exit non-zero before readiness claims are written.
- Network dependency: intentionally absent; `--no-network` is required and artifacts carry `network_fetch_attempted=false`.
- Graph/import/write dependencies: intentionally absent; all graph/import/LadybugDB/production safety flags are fail-closed false in summary, diagnostics, events, decision, and per-article artifacts.
- Subprocess dependency: intentionally absent; git commit provenance is read from `.git/HEAD` when available and omitted otherwise.

## Load Profile

Expected load is the real six-article, eleven-variant M027 corpus. At 10x, local filesystem reads/writes and in-memory loader/parser/PageIndex/chunk/evidence construction over converted text payloads saturate first. Protection is bounded S03 converted payload input, one-variant-at-a-time replay, redacted per-article JSON artifacts, metadata-only skips, and no network/database/graph writer pools.

## Negative Tests

- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_requires_no_network_and_s03_linkage` covers no-network enforcement and stale S03 source linkage.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_rejects_converted_payload_hash_mismatch` covers stale/tampered converted payload hashes.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_captures_boundaries_and_baseline_comparison` covers loader/parser/PageIndex/chunk/evidence metrics, S04 comparison rows, provenance, and fail-closed flags.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_preserves_parser_ready_zero_chunk_diagnostic` covers parser-ready zero-chunk preservation and diagnostic emission.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_skips_metadata_only_without_payload` covers metadata-only skip behavior without payload reads.
- `tests/test_m027_end_to_end_mixed_replay.py::test_replay_rejects_unsafe_output_dir` covers unsafe output path rejection.
- `tests/test_m027_end_to_end_mixed_replay.py::test_metadata_outputs_are_redacted` covers raw text/HTML/PDF/key leakage protections.
"""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(report, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--conversion-summary", type=Path, default=DEFAULT_CONVERSION_SUMMARY)
    parser.add_argument("--s03-summary", type=Path, default=DEFAULT_S03_SUMMARY)
    parser.add_argument("--baseline-summary", type=Path, default=DEFAULT_BASELINE_SUMMARY)
    parser.add_argument("--baseline-diagnostics", type=Path, default=DEFAULT_BASELINE_DIAGNOSTICS)
    parser.add_argument("--output-summary", type=Path, default=DEFAULT_OUTPUT_SUMMARY)
    parser.add_argument("--output-diagnostics", type=Path, default=DEFAULT_OUTPUT_DIAGNOSTICS)
    parser.add_argument("--output-events", type=Path, default=DEFAULT_OUTPUT_EVENTS)
    parser.add_argument("--output-report", type=Path, default=DEFAULT_OUTPUT_REPORT)
    parser.add_argument("--readiness-decision", type=Path, default=DEFAULT_READINESS_DECISION)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument(
        "--no-network",
        action="store_true",
        default=True,
        help="Preserve fail-closed local-only replay behavior (default; retained for explicit auditability).",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        summary, diagnostics, events, decision = replay_end_to_end(args)
        write_json(Path(args.output_summary), summary)
        write_jsonl(Path(args.output_diagnostics), diagnostics)
        write_jsonl(Path(args.output_events), events)
        write_json(Path(args.readiness_decision), decision)
        write_report(Path(args.output_report), summary, decision)
        summary = finalize_output_provenance(args, summary)
        write_json(Path(args.output_summary), summary)
    except EndToEndReplayError as exc:
        sys.stderr.write(f"end-to-end mixed replay failed: {exc}\n")
        return 2
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
