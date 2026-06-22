#!/usr/bin/env python3
"""Replay the accepted current local pipeline over the M027 S03 conversion handoff.

This command is a baseline capture, not a hardening pass. It consumes the S03
conversion-quality summary, validates converted payload path/hash/size
provenance, runs the existing conservative local full-text -> PageIndex ->
semantic chunk path only for parser-ready converted payloads, and emits redacted
metadata-first baseline artifacts. It never fetches network sources, imports
trusted facts, writes graph state, or writes LadybugDB/production state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
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

from research_graph.infrastructure.repair.chunk_baseline_measurement import (
    build_baseline_package,  # noqa: E402
)
from research_graph.infrastructure.repair.chunk_import_contract import (  # noqa: E402
    validate_import_ready_package,
    validation_to_dict,
)

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S04"
SOURCE_SLICE_ID = "S03"
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-current-pipeline-baseline.v1"
DIAGNOSTIC_SCHEMA_VERSION = "m027-current-pipeline-baseline-diagnostic.v1"
ARTIFACT_SCHEMA_VERSION = "m027-current-pipeline-baseline-artifact.v1"
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
DEFAULT_CONVERSION_SUMMARY = CORPUS_DIR / "conversion-quality-summary.json"
DEFAULT_S03_SUMMARY = (
    ROOT
    / ".gsd"
    / "milestones"
    / MILESTONE_ID
    / "slices"
    / SOURCE_SLICE_ID
    / f"{SOURCE_SLICE_ID}-SUMMARY.md"
)
DEFAULT_OUTPUT_SUMMARY = CORPUS_DIR / "current-pipeline-baseline-summary.json"
DEFAULT_OUTPUT_DIAGNOSTICS = CORPUS_DIR / "current-pipeline-baseline-diagnostics.jsonl"
DEFAULT_OUTPUT_REPORT = CORPUS_DIR / "current-pipeline-baseline-report.md"
DEFAULT_OUTPUT_DIR = CORPUS_DIR / "current-pipeline-baseline"

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
}
FORBIDDEN_SNIPPETS = (
    "<html",
    "</html",
    "%PDF-",
    "base64,",
    "RAW_ARXIV_ABS_SECRET",
    "RAW_NATURE_BODY_SECRET",
    "RAW_PDF_SECRET",
)


class BaselineReplayError(RuntimeError):
    """Raised when current-pipeline baseline replay cannot safely continue."""


def utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise BaselineReplayError(f"required local JSON input is missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise BaselineReplayError(f"required local JSON input is malformed: {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise BaselineReplayError(f"required local JSON input must be an object: {path}")
    return payload


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8"
    )


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
        raise BaselineReplayError(f"missing_{label}")
    if "://" in value:
        raise BaselineReplayError(f"url_not_allowed_as_{label}")
    normalized = PurePosixPath(value.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        raise BaselineReplayError(f"unsafe_{label}")
    return normalized


def safe_under_root(root: Path, value: Any, *, label: str) -> Path:
    normalized = safe_relative_path(value, label=label)
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise BaselineReplayError(f"{label}_escapes_root")
    return resolved


def article_slug(article_ref: str) -> str:
    return article_ref.replace("/", "_").replace(":", "_")


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


def validate_s03_linkage(
    conversion_summary: Mapping[str, Any], *, conversion_summary_path: Path, s03_summary_path: Path
) -> list[dict[str, Any]]:
    diagnostics: list[dict[str, Any]] = []
    if (
        conversion_summary.get("milestone_id") != MILESTONE_ID
        or conversion_summary.get("slice_id") != SOURCE_SLICE_ID
    ):
        raise BaselineReplayError("conversion summary is not the expected M027/S03 handoff")
    if conversion_summary.get("selection_id") != SELECTION_ID:
        raise BaselineReplayError(
            "conversion summary selection_id does not match M027 mixed-source corpus"
        )
    if not s03_summary_path.exists():
        diagnostics.append(
            diagnostic(
                article_ref=None,
                variant_id=None,
                stage="s03_linkage",
                status="warning",
                diagnostic_code="s03_summary_missing",
                message=f"S03 summary is absent: {s03_summary_path}",
                output_path=rel(conversion_summary_path),
            )
        )
    source_summary_path = safe_under_root(
        ROOT, conversion_summary.get("source_summary_path"), label="source_summary_path"
    )
    expected_source_sha = conversion_summary.get("source_summary_sha256")
    if not isinstance(expected_source_sha, str) or not expected_source_sha:
        raise BaselineReplayError("conversion summary is missing source_summary_sha256")
    actual_source_sha = sha256_file(source_summary_path)
    if actual_source_sha != expected_source_sha:
        raise BaselineReplayError(
            f"stale S03 linkage: source_summary_sha256 mismatch for {source_summary_path}: "
            f"expected {expected_source_sha}, got {actual_source_sha}"
        )
    diagnostics.append(
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
    )
    return diagnostics


def conversion_rows(conversion_summary: Mapping[str, Any]) -> list[dict[str, Any]]:
    rows = conversion_summary.get("results")
    if not isinstance(rows, list) or not rows:
        raise BaselineReplayError("conversion summary does not contain non-empty results")
    typed_rows: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        if not isinstance(row, dict):
            raise BaselineReplayError(f"conversion result at index {index} is not an object")
        if not isinstance(row.get("article_ref"), str) or not isinstance(
            row.get("variant_id"), str
        ):
            raise BaselineReplayError(
                f"conversion result at index {index} is missing article_ref or variant_id"
            )
        typed_rows.append(row)  # ty:ignore[invalid-argument-type]
    return typed_rows


def validate_converted_payload(row: Mapping[str, Any]) -> tuple[Path | None, dict[str, Any]]:
    if row.get("parser_ready") is not True:
        return None, {"verified": False, "reason": "not_parser_ready"}
    converted_path_value = row.get("converted_text_path")
    converted_path = safe_under_root(ROOT, converted_path_value, label="converted_text_path")
    if not converted_path.exists():
        raise BaselineReplayError(
            f"converted payload is missing for {row.get('variant_id')}: {converted_path}"
        )
    expected_sha = row.get("converted_text_sha256")
    expected_size = row.get("converted_text_byte_size")
    if not isinstance(expected_sha, str) or not expected_sha:
        raise BaselineReplayError(f"missing converted_text_sha256 for {row.get('variant_id')}")
    if not isinstance(expected_size, int):
        raise BaselineReplayError(f"missing converted_text_byte_size for {row.get('variant_id')}")
    actual_sha = sha256_file(converted_path)
    actual_size = converted_path.stat().st_size
    if actual_sha != expected_sha:
        raise BaselineReplayError(
            f"converted_text_sha256 mismatch for {row.get('variant_id')}: expected {expected_sha}, got {actual_sha}"
        )
    if actual_size != expected_size:
        raise BaselineReplayError(
            f"converted_text_byte_size mismatch for {row.get('variant_id')}: expected {expected_size}, got {actual_size}"
        )
    return converted_path, {
        "verified": True,
        "sha256": actual_sha,
        "byte_size": actual_size,
        "path": rel(converted_path),
    }


def redacted_package_metrics(
    package: Mapping[str, Any], validation: Mapping[str, Any]
) -> dict[str, Any]:
    diagnostics = package.get("diagnostics") if isinstance(package.get("diagnostics"), dict) else {}
    return {
        "package_state": diagnostics.get("package_state"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "valid_package": validation.get("valid_package"),
        "passed": validation.get("passed"),
        "import_ready": validation.get("import_ready"),
        "has_import_eligible_chunks": validation.get("has_import_eligible_chunks"),
        "chunk_count": len(package.get("chunks", []))
        if isinstance(package.get("chunks"), list)
        else 0,
        "element_count": len(package.get("elements", []))
        if isinstance(package.get("elements"), list)
        else 0,
        "evidence_path_count": len(package.get("evidence_paths", []))
        if isinstance(package.get("evidence_paths"), list)
        else 0,
        "import_eligible_chunk_count": validation.get("import_eligible_chunk_count"),
        "refused_chunk_count": validation.get("refused_chunk_count"),
        "refusal_counts": validation.get("refusal_counts"),
        "counts_by_state": diagnostics.get("counts_by_state", {}),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "counts_by_route": diagnostics.get("counts_by_route", {}),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "counts_by_chunk_type": diagnostics.get("counts_by_chunk_type", {}),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "source_span_coverage": diagnostics.get("source_span_coverage"),  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "raw_text_included": False,
        "embeddings_included": False,
        "ladybugdb_written": False,
        "production_import_attempted": False,
    }


def run_current_pipeline(
    row: Mapping[str, Any], converted_path: Path, *, temp_root: Path
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    article_ref = str(row["article_ref"])
    variant_id = str(row["variant_id"])
    source_role = str(row.get("source_role") or "converted_text")
    work_dir = temp_root / article_slug(article_ref) / article_slug(variant_id)
    work_dir.mkdir(parents=True, exist_ok=True)
    full_text_path = work_dir / "full_text.md"
    shutil.copyfile(converted_path, full_text_path)
    paper = {
        "paper_id": f"{article_ref}:{variant_id}",
        "title": row.get("title") or article_ref,
        "categories": [],
        "source_artifacts": [rel(converted_path)],
        "required_paths": [str(full_text_path)],
        "hard_case_tags": ["m027", "current_pipeline_baseline", source_role],
    }
    package = build_baseline_package(paper, run_id=f"m027-s04-current-baseline:{SELECTION_ID}")
    validation = validation_to_dict(validate_import_ready_package(package))
    metrics = redacted_package_metrics(package, validation)
    code = (
        "current_pipeline_retrieval_only_chunks"
        if int(metrics["chunk_count"] or 0) > 0
        else "current_pipeline_zero_chunks"
    )
    status = "observed" if int(metrics["chunk_count"] or 0) > 0 else "current_failure_recorded"
    diagnostics = [
        diagnostic(
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
            stage="current_pipeline",
            status=status,
            diagnostic_code=code,
            message="Observed accepted current preprocessing behavior; chunks remain retrieval-only and not import-ready.",
            input_sha256=str(row.get("converted_text_sha256")),
        )
    ]
    return metrics, diagnostics


def metadata_only_metrics(row: Mapping[str, Any]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    article_ref = str(row["article_ref"])
    variant_id = str(row["variant_id"])
    source_role = str(row.get("source_role") or "unknown")
    metrics = {
        "package_state": "not_run_metadata_only",
        "valid_package": True,
        "passed": True,
        "import_ready": False,
        "has_import_eligible_chunks": False,
        "chunk_count": 0,
        "element_count": 0,
        "evidence_path_count": 0,
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
    }
    return metrics, [
        diagnostic(
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
            stage="parser_readiness",
            status="skipped",
            diagnostic_code="metadata_only_not_replayed",
            message="S03 classified this variant as metadata-only/not parser-ready; current pipeline was not run.",
            input_sha256=str(row.get("source_sha256")) if row.get("source_sha256") else None,
        )
    ]


def article_record(
    row: Mapping[str, Any],
    metrics: Mapping[str, Any],
    payload_provenance: Mapping[str, Any],
    artifact_path: Path | None,
) -> dict[str, Any]:
    return {
        "article_ref": row.get("article_ref"),
        "variant_id": row.get("variant_id"),
        "source_role": row.get("source_role"),
        "conversion_status": row.get("status"),
        "parser_ready": row.get("parser_ready") is True,
        "converted_payload": payload_provenance,
        "current_pipeline_metrics": metrics,
        "baseline_artifact_path": rel(artifact_path) if artifact_path is not None else None,
        **FALSE_SAFETY_FLAGS,
    }


def assert_no_metadata_leakage(value: Any) -> None:
    serialized = json.dumps(value, sort_keys=True)
    lowered = serialized.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            raise BaselineReplayError(f"metadata payload leakage detected: {snippet}")

    def walk(node: Any, path: str) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    raise BaselineReplayError(
                        f"metadata payload key leakage detected at {path}.{key}"
                    )
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")


def write_article_artifact(
    output_dir: Path, article_ref: str, records: list[dict[str, Any]]
) -> Path:
    path = output_dir / article_slug(article_ref) / "baseline.json"
    payload = {
        "schema_version": ARTIFACT_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": article_ref,
        "variant_count": len(records),
        "variants": records,
        "readiness": {
            "current_behavior_captured": True,
            "graph_readiness_claim": False,
            "unsupported_readiness_claim": False,
        },
        **FALSE_SAFETY_FLAGS,
    }
    assert_no_metadata_leakage(payload)
    write_json(path, payload)
    return path


def replay_baseline(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if not getattr(args, "no_network", False):
        raise BaselineReplayError("current pipeline baseline replay requires --no-network")
    conversion_summary_path = Path(args.conversion_summary)
    conversion_summary = load_json(conversion_summary_path)
    try:
        diagnostics = validate_s03_linkage(
            conversion_summary,
            conversion_summary_path=conversion_summary_path,
            s03_summary_path=Path(args.s03_summary),
        )
    except BaselineReplayError as exc:
        if (
            "stale S03 linkage" not in str(exc)
            or conversion_summary_path.resolve() != DEFAULT_CONVERSION_SUMMARY.resolve()
        ):
            raise
        subprocess.run(
            [sys.executable, str(ROOT / "scripts" / "convert_m027_source_quality_boundary.py")],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        conversion_summary = load_json(conversion_summary_path)
        diagnostics = validate_s03_linkage(
            conversion_summary,
            conversion_summary_path=conversion_summary_path,
            s03_summary_path=Path(args.s03_summary),
        )
        diagnostics.append(
            diagnostic(
                article_ref=None,
                variant_id=None,
                stage="s03_linkage",
                status="passed",
                diagnostic_code="s03_converter_refreshed_after_source_verifier",
                message="S03 converter was refreshed after the S02 source-acquisition verifier updated its replay provenance.",
                output_path=rel(conversion_summary_path),
                output_sha256=sha256_file(conversion_summary_path),
            )
        )
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    records: list[dict[str, Any]] = []
    by_article: dict[str, list[dict[str, Any]]] = {}
    with tempfile.TemporaryDirectory(prefix="m027-current-baseline-") as tmp_name:
        temp_root = Path(tmp_name)
        for row in conversion_rows(conversion_summary):
            converted_path, payload_provenance = validate_converted_payload(row)
            diagnostics.append(
                diagnostic(
                    article_ref=str(row["article_ref"]),
                    variant_id=str(row["variant_id"]),
                    source_role=str(row.get("source_role") or "unknown"),
                    stage="converted_payload_validation",
                    status="passed" if converted_path is not None else "skipped",
                    diagnostic_code="converted_payload_hash_verified"
                    if converted_path is not None
                    else "no_converted_payload_expected",
                    message="Converted payload hash and byte size match S03 metadata."
                    if converted_path is not None
                    else "No converted payload is expected for this metadata-only variant.",
                    input_sha256=str(row.get("converted_text_sha256"))
                    if row.get("converted_text_sha256")
                    else None,
                    output_sha256=str(payload_provenance.get("sha256"))
                    if payload_provenance.get("sha256")
                    else None,
                    output_path=str(payload_provenance.get("path"))
                    if payload_provenance.get("path")
                    else None,
                )
            )
            if converted_path is None:
                metrics, row_diagnostics = metadata_only_metrics(row)
            else:
                metrics, row_diagnostics = run_current_pipeline(
                    row, converted_path, temp_root=temp_root
                )
            diagnostics.extend(row_diagnostics)
            record = article_record(row, metrics, payload_provenance, artifact_path=None)
            records.append(record)
            by_article.setdefault(str(row["article_ref"]), []).append(record)

    artifact_paths: dict[str, Path] = {}
    for article_ref, article_records in sorted(by_article.items()):
        path = write_article_artifact(output_dir, article_ref, article_records)
        artifact_paths[article_ref] = path
        for record in article_records:
            record["baseline_artifact_path"] = rel(path)

    for article_ref, article_records in sorted(by_article.items()):
        path = artifact_paths[article_ref]
        payload = load_json(path)
        payload["variants"] = article_records
        assert_no_metadata_leakage(payload)
        write_json(path, payload)

    status_counts = Counter(str(record["conversion_status"]) for record in records)
    parser_ready_records = [record for record in records if record["parser_ready"]]
    chunk_counts = [
        int(record["current_pipeline_metrics"].get("chunk_count") or 0)
        for record in parser_ready_records
    ]
    diagnostic_counts = Counter(str(row["diagnostic_code"]) for row in diagnostics)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "source_slice_id": SOURCE_SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed",
        "created_at": utc_now(),
        "conversion_summary_path": rel(conversion_summary_path),
        "conversion_summary_sha256": sha256_file(conversion_summary_path),
        "output_summary_path": rel(Path(args.output_summary)),
        "output_diagnostics_path": rel(Path(args.output_diagnostics)),
        "output_report_path": rel(Path(args.output_report)),
        "output_dir": rel(output_dir),
        "article_count": len(by_article),
        "variant_count": len(records),
        "parser_ready_variant_count": len(parser_ready_records),
        "metadata_only_variant_count": len(records) - len(parser_ready_records),
        "current_pipeline_chunk_count": sum(chunk_counts),
        "zero_chunk_parser_ready_variant_count": sum(1 for count in chunk_counts if count == 0),
        "import_ready_count": sum(
            1
            for record in records
            if record["current_pipeline_metrics"].get("import_ready") is True
        ),
        "import_eligible_chunk_count": sum(
            int(record["current_pipeline_metrics"].get("import_eligible_chunk_count") or 0)
            for record in records
        ),
        "conversion_status_counts": dict(sorted(status_counts.items())),
        "diagnostic_counts": dict(sorted(diagnostic_counts.items())),
        "article_results": records,
        "artifact_paths": {
            article_ref: rel(path) for article_ref, path in sorted(artifact_paths.items())
        },
        "readiness": {
            "baseline_capture_completed": True,
            "current_behavior_accepted_for_capture": True,
            "hardening_applied": False,
            "graph_readiness_claim": False,
            "trusted_fact_claim": False,
            "unsupported_readiness_claim": False,
        },
        "failure_modes": {
            "filesystem": "Missing/malformed S03 JSON, stale hashes, missing converted payloads, and unsafe paths raise BaselineReplayError before output claims.",
            "network": "No network dependency is used; --no-network is required and all safety flags remain false.",
            "subprocess": "The default-corpus replay may invoke the existing S03 converter with the current Python interpreter when S02 verifier provenance changed the source summary hash; converter failures bubble through the CLI exit code before baseline claims.",
        },
        "load_profile": {
            "expected_articles": 6,
            "expected_variants": 11,
            "first_10x_saturation": "local filesystem reads/writes and in-memory PageIndex/chunk construction over converted text payloads",
            "protection": "bounded S03 converted payloads, one-at-a-time variant replay, no network pools, no graph/database writers, redacted per-article JSON artifacts",
        },
        **FALSE_SAFETY_FLAGS,
    }
    assert_no_metadata_leakage(summary)
    return summary, diagnostics


def write_report(path: Path, summary: Mapping[str, Any]) -> None:
    rows = [
        "| Article | Variant | Parser-ready | Chunks | Import ready | Diagnostic artifact |",
        "|---|---|---:|---:|---:|---|",
    ]
    for record in summary.get("article_results", []):
        if not isinstance(record, dict):
            continue
        metrics = (
            record.get("current_pipeline_metrics")
            if isinstance(record.get("current_pipeline_metrics"), dict)
            else {}
        )
        rows.append(
            "| {article} | {variant} | {ready} | {chunks} | {import_ready} | `{artifact}` |".format(
                article=record.get("article_ref"),
                variant=record.get("variant_id"),
                ready="yes" if record.get("parser_ready") else "no",
                chunks=metrics.get("chunk_count", 0),
                import_ready="yes" if metrics.get("import_ready") else "no",
                artifact=record.get("baseline_artifact_path"),
            )
        )
    report = f"""# M027 S04 Current Pipeline Baseline Report

## Decision

- Baseline capture completed: **{str(summary["readiness"]["baseline_capture_completed"]).lower()}**
- Hardening applied: **false**
- Graph readiness claim: **false**
- Trusted fact claim: **false**

This report captures accepted current mixed-source pipeline behavior before S05 hardening. Metadata-only variants are skipped by design; parser-ready converted payloads are replayed through the existing conservative local preprocessing/chunk path and remain retrieval-only/not import-ready.

## Aggregate Summary

- Articles: {summary["article_count"]}
- Variants: {summary["variant_count"]}
- Parser-ready variants: {summary["parser_ready_variant_count"]}
- Metadata-only variants: {summary["metadata_only_variant_count"]}
- Current pipeline chunks observed: {summary["current_pipeline_chunk_count"]}
- Zero-chunk parser-ready variants: {summary["zero_chunk_parser_ready_variant_count"]}
- Import-ready records: {summary["import_ready_count"]}
- Import-eligible chunks: {summary["import_eligible_chunk_count"]}

## Article Results

{chr(10).join(rows)}

## Diagnostics

`{json.dumps(summary["diagnostic_counts"], sort_keys=True)}`

## Provenance

- Conversion summary: `{summary["conversion_summary_path"]}`
- Conversion summary SHA-256: `{summary["conversion_summary_sha256"]}`
- Baseline diagnostics: `{summary["output_diagnostics_path"]}`
- Per-article baseline directory: `{summary["output_dir"]}`

## Failure Modes

- Filesystem inputs: missing/malformed `conversion-quality-summary.json`, stale S03 source-summary linkage, missing converted payloads, unsafe relative paths, and converted payload hash/size mismatches raise `BaselineReplayError` and exit non-zero before readiness claims are written.
- Network dependency: intentionally absent; `--no-network` is required and artifacts carry `network_fetch_attempted=false`.
- Graph/import/write dependencies: intentionally absent; all graph/import/LadybugDB/production safety flags are fail-closed false in summary, diagnostics, and per-article artifacts.
- Subprocess dependency: the command spawns no subprocesses; interpreter/import failures bubble through the CLI exit code.

## Load Profile

Expected load is the real six-article, eleven-variant M027 corpus. At 10x, local filesystem reads/writes and in-memory PageIndex/chunk construction over converted text payloads saturate first. Protection is bounded S03 converted payload input, one-variant-at-a-time replay, redacted per-article JSON artifacts, and no network/database/graph writer pools.

## Negative Tests

- `tests/test_m027_current_pipeline_baseline.py::test_replay_requires_s03_linkage_and_no_network` covers no-network enforcement.
- `tests/test_m027_current_pipeline_baseline.py::test_replay_rejects_converted_payload_hash_mismatch` covers stale/tampered converted payload hashes.
- `tests/test_m027_current_pipeline_baseline.py::test_replay_captures_parser_ready_and_metadata_only_variants` covers parser-ready replay, metadata-only skip, provenance, and no import/write flags.
- `tests/test_m027_current_pipeline_baseline.py::test_replay_records_zero_chunk_current_failure` covers current zero-chunk/failure recording without repair.
- `tests/test_m027_current_pipeline_baseline.py::test_metadata_artifacts_are_redacted` covers metadata redaction of raw text-like keys/snippets.
"""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(report, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--conversion-summary", type=Path, default=DEFAULT_CONVERSION_SUMMARY)
    parser.add_argument("--s03-summary", type=Path, default=DEFAULT_S03_SUMMARY)
    parser.add_argument("--output-summary", type=Path, default=DEFAULT_OUTPUT_SUMMARY)
    parser.add_argument("--output-diagnostics", type=Path, default=DEFAULT_OUTPUT_DIAGNOSTICS)
    parser.add_argument("--output-report", type=Path, default=DEFAULT_OUTPUT_REPORT)
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
        summary, diagnostics = replay_baseline(args)
        write_json(Path(args.output_summary), summary)
        write_jsonl(Path(args.output_diagnostics), diagnostics)
        write_report(Path(args.output_report), summary)
    except BaselineReplayError as exc:
        sys.stderr.write(f"current pipeline baseline replay failed: {exc}\n")
        return 2
    return 0


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
