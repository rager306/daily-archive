#!/usr/bin/env python3
"""Replay M031 catalog-backed source acquisition from already-local artifacts.

This command consumes the T01 M031 selection contract. It never fetches the
network and never mutates article catalog records. It copies only existing
catalog source files into the M031 corpus-local ``source/`` tree and emits
metadata-only terminal-state evidence for each selected source variant or typed
catalog blocker.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
import tempfile
import time
from collections import defaultdict
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S02"
SELECTION_ID = "m031-catalog-backed-replay-v1"
SCHEMA_VERSION = "m031-catalog-backed-acquisition.v1"

ROLE_TARGETS: dict[str, str] = {
    "arxiv_html": "source/article.html",
    "arxiv_pdf": "source/original.pdf",
    "arxiv_abs_page": "source/abs.html",
    "external_pdf": "source/original.pdf",
}

FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_only_acquisition": True,
    "network_fetch_attempted": False,
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


class AcquisitionError(ValueError):
    """Typed validation error for deterministic CLI diagnostics."""

    def __init__(self, code: str, message: str, *, identity: str | None = None, article_ref: str | None = None):
        super().__init__(message)
        self.code = code
        self.identity = identity
        self.article_ref = article_ref


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def load_json_object(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError as exc:
        raise AcquisitionError("malformed_json", f"malformed JSON at {path}: {exc}") from exc
    except OSError as exc:
        raise AcquisitionError("json_read_failed", f"failed to read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise AcquisitionError("malformed_json_object", f"expected JSON object at {path}")
    return payload


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    tmp_path = Path(tmp_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        tmp_path.replace(path)
    except Exception:
        tmp_path.unlink(missing_ok=True)
        raise


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    atomic_write_text(path, json.dumps(payload, indent=2, sort_keys=False) + "\n")


def write_jsonl(path: Path, rows: Iterable[Mapping[str, Any]]) -> None:
    atomic_write_text(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_child_path(root: Path, rel_path: str, *, code: str = "unsafe_relative_path") -> Path:
    if not isinstance(rel_path, str) or not rel_path.strip():
        raise AcquisitionError(code, f"empty unsafe relative path: {rel_path!r}")
    if "://" in rel_path:
        raise AcquisitionError("url_not_allowed_as_local_path", f"URL cannot be used as a local path: {rel_path}")
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if normalized.is_absolute() or ".." in normalized.parts or any(part in ("", ".") for part in normalized.parts):
        raise AcquisitionError(code, f"unsafe relative path: {rel_path}")
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise AcquisitionError(code, f"path escapes root: {rel_path}")
    return resolved


def safe_article_segment(article_ref: str | None, article_key: str | None) -> str:
    raw = article_ref or f"unresolved/{article_key or 'unknown'}"
    normalized = PurePosixPath(raw.replace("\\", "/"))
    parts = [part for part in normalized.parts if part not in ("", ".")]
    if not parts or normalized.is_absolute() or ".." in parts:
        return f"unsafe/{hashlib.sha256(raw.encode('utf-8')).hexdigest()[:16]}"
    return "/".join(parts)


def confined_source_target(output_dir: Path, article_ref: str | None, article_key: str | None, role_target: str) -> tuple[Path, str]:
    rel_path = f"{safe_article_segment(article_ref, article_key)}/{role_target}"
    target = safe_child_path(output_dir, rel_path, code="unsafe_output_source_path")
    return target, target.relative_to(output_dir.resolve()).as_posix()


def _string_value(row: Mapping[str, Any], key: str) -> str | None:
    value = row.get(key)
    return value if isinstance(value, str) else None


def result_base(
    *,
    identity: str | None,
    requested_ref_id: str | None,
    requested_url: str | None,
    article_ref: str | None,
    article_key: str | None,
    article_path: str | None,
    variant: Mapping[str, Any] | None,
    source_role: str | None,
    status: str,
    diagnostic_code: str,
    failure_reason: str | None,
    local_path: str | None,
    source_catalog_path: str | None,
    sha256: str | None = None,
    byte_size: int | None = None,
    media_type: str | None = None,
) -> dict[str, Any]:
    url = _string_value(variant, "url") if variant is not None else None
    result = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "identity": identity,
        "requested_ref_id": requested_ref_id,
        "requested_url": requested_url,
        "article_ref": article_ref,
        "article_key": article_key,
        "article_path": article_path,
        "variant_id": _string_value(variant, "variant_id") if variant is not None else None,
        "source_role": source_role,
        "url": url or requested_url,
        "status": status,
        "terminal_state": status,
        "diagnostic_code": diagnostic_code,
        "blocker_code": diagnostic_code if status == "blocked" else None,
        "failure_reason": failure_reason,
        "local_path": local_path,
        "source_catalog_path": source_catalog_path,
        "safe_local_paths": [local_path] if local_path else [],
        "sha256": sha256,
        "byte_size": byte_size,
        "media_type": media_type,
        "is_metadata_only": bool(variant.get("is_metadata_only")) if variant is not None and isinstance(variant.get("is_metadata_only"), bool) else None,
        "requires_conversion": bool(variant.get("requires_conversion")) if variant is not None and isinstance(variant.get("requires_conversion"), bool) else None,
        "network_fetch_attempted": False,
        "network_fetch_allowed": False,
        "captured_at": utc_now() if status == "captured" else None,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
    }
    forbidden_present = set(result) & FORBIDDEN_OUTPUT_KEYS
    if forbidden_present:
        raise AssertionError(f"forbidden metadata keys present: {sorted(forbidden_present)}")
    return result


def blocker_result(blocker: Mapping[str, Any]) -> dict[str, Any]:
    return result_base(
        identity=_string_value(blocker, "identity"),
        requested_ref_id=_string_value(blocker, "requested_ref_id"),
        requested_url=_string_value(blocker, "requested_url"),
        article_ref=None,
        article_key=None,
        article_path=None,
        variant=None,
        source_role=_string_value(blocker, "source_role"),
        status="blocked",
        diagnostic_code=_string_value(blocker, "blocker_code") or "typed_catalog_blocker",
        failure_reason=_string_value(blocker, "evidence") or "catalog row is a typed blocker",
        local_path=None,
        source_catalog_path=None,
        media_type=None,
    )


def source_result_for_variant(
    *,
    article: Mapping[str, Any],
    variant: Mapping[str, Any],
    catalog_root: Path,
    output_dir: Path,
    write: bool,
) -> dict[str, Any]:
    identity = _string_value(article, "identity")
    requested_ref_id = _string_value(article, "requested_ref_id")
    requested_url = _string_value(article, "requested_url")
    article_ref = _string_value(article, "article_ref")
    article_key = _string_value(article, "article_key")
    article_path = _string_value(article, "article_path")
    role = _string_value(variant, "source_role")
    media_type = _string_value(variant, "media_type")

    if role not in ROLE_TARGETS:
        return result_base(
            identity=identity,
            requested_ref_id=requested_ref_id,
            requested_url=requested_url,
            article_ref=article_ref,
            article_key=article_key,
            article_path=article_path,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code="unsupported_source_role",
            failure_reason="source role is not part of the M031 acquisition replay boundary",
            local_path=None,
            source_catalog_path=None,
            media_type=media_type,
        )

    local_value = variant.get("local_path") or variant.get("path") or variant.get("catalog_relative_path")
    if not isinstance(local_value, str):
        return result_base(
            identity=identity,
            requested_ref_id=requested_ref_id,
            requested_url=requested_url,
            article_ref=article_ref,
            article_key=article_key,
            article_path=article_path,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code="missing_local_source_path",
            failure_reason="catalog source variant does not expose a local source path",
            local_path=None,
            source_catalog_path=None,
            media_type=media_type,
        )

    if not article_path:
        raise AcquisitionError("missing_article_path", "catalog-backed article is missing article_path", identity=identity, article_ref=article_ref)
    try:
        article_json_path = safe_child_path(catalog_root, article_path, code="unsafe_article_path")
        source_path = safe_child_path(article_json_path.parent, local_value, code="unsafe_catalog_source_path")
    except AcquisitionError as exc:
        return result_base(
            identity=identity,
            requested_ref_id=requested_ref_id,
            requested_url=requested_url,
            article_ref=article_ref,
            article_key=article_key,
            article_path=article_path,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code=exc.code,
            failure_reason="catalog source path is not allowed",
            local_path=None,
            source_catalog_path=local_value,
            media_type=media_type,
        )

    target, local_path = confined_source_target(output_dir, article_ref, article_key, ROLE_TARGETS[role])
    source_catalog_path = source_path.relative_to(catalog_root.resolve()).as_posix()
    if not source_path.exists():
        return result_base(
            identity=identity,
            requested_ref_id=requested_ref_id,
            requested_url=requested_url,
            article_ref=article_ref,
            article_key=article_key,
            article_path=article_path,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code="local_source_missing",
            failure_reason="catalog metadata references a source artifact that is absent locally",
            local_path=local_path,
            source_catalog_path=source_catalog_path,
            media_type=media_type,
        )
    if source_path.stat().st_size == 0:
        return result_base(
            identity=identity,
            requested_ref_id=requested_ref_id,
            requested_url=requested_url,
            article_ref=article_ref,
            article_key=article_key,
            article_path=article_path,
            variant=variant,
            source_role=role,
            status="failed",
            diagnostic_code="empty_local_source",
            failure_reason="catalog source artifact is empty",
            local_path=local_path,
            source_catalog_path=source_catalog_path,
            media_type=media_type,
        )

    if write:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_path, target)
    artifact_path = target if write else source_path
    return result_base(
        identity=identity,
        requested_ref_id=requested_ref_id,
        requested_url=requested_url,
        article_ref=article_ref,
        article_key=article_key,
        article_path=article_path,
        variant=variant,
        source_role=role,
        status="captured",
        diagnostic_code="captured_local_source_artifact",
        failure_reason=None,
        local_path=local_path,
        source_catalog_path=source_catalog_path,
        sha256=sha256_file(artifact_path),
        byte_size=artifact_path.stat().st_size,
        media_type=media_type,
    )


def replay_selection(*, selection_path: Path, catalog_root: Path, output_dir: Path, write: bool = True) -> list[dict[str, Any]]:
    selection = load_json_object(selection_path)
    articles = selection.get("articles")
    if not isinstance(articles, list):
        raise AcquisitionError("malformed_selection_articles", "selection articles must be a list")
    blockers = selection.get("catalog_blockers")
    if not isinstance(blockers, list):
        raise AcquisitionError("malformed_selection_blockers", "selection catalog_blockers must be a list")

    results: list[dict[str, Any]] = []
    for article in articles:
        if not isinstance(article, Mapping):
            raise AcquisitionError("malformed_selection_article", "selection article rows must be objects")
        variants = article.get("source_variants")
        if not isinstance(variants, list) or not variants:
            raise AcquisitionError(
                "malformed_selection_source_variants",
                "selection catalog-backed articles must expose source_variants",
                identity=_string_value(article, "identity"),
                article_ref=_string_value(article, "article_ref"),
            )
        for variant in variants:
            if not isinstance(variant, Mapping):
                raise AcquisitionError(
                    "malformed_selection_source_variant",
                    "selection source variant rows must be objects",
                    identity=_string_value(article, "identity"),
                    article_ref=_string_value(article, "article_ref"),
                )
            results.append(source_result_for_variant(article=article, variant=variant, catalog_root=catalog_root, output_dir=output_dir, write=write))

    for blocker in blockers:
        if not isinstance(blocker, Mapping):
            raise AcquisitionError("malformed_selection_blocker", "selection blocker rows must be objects")
        results.append(blocker_result(blocker))
    return results


def build_summary(
    results: list[dict[str, Any]],
    *,
    selection_path: Path,
    catalog_root: Path,
    output_dir: Path,
    duration_ms: int,
) -> dict[str, Any]:
    counts: dict[str, int] = {"captured": 0, "blocked": 0, "failed": 0}
    per_identity: dict[str, dict[str, int]] = defaultdict(lambda: {"captured": 0, "blocked": 0, "failed": 0})
    per_role: dict[str, dict[str, int]] = defaultdict(lambda: {"captured": 0, "blocked": 0, "failed": 0})
    for result in results:
        status = str(result.get("status"))
        if status not in counts:
            continue
        counts[status] += 1
        identity = result.get("identity") if isinstance(result.get("identity"), str) else "<missing-identity>"
        role = result.get("source_role") if isinstance(result.get("source_role"), str) else "<missing-role>"
        per_identity[identity][status] += 1
        per_role[role][status] += 1
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed_with_diagnostics" if counts["blocked"] or counts["failed"] else "captured",
        "variant_or_blocker_count": len(results),
        "counts": counts,
        "per_identity_terminal_state_counts": {key: dict(value) for key, value in sorted(per_identity.items())},
        "per_role_terminal_state_counts": {key: dict(value) for key, value in sorted(per_role.items())},
        "results": results,
        "input_paths": {"selection": selection_path.as_posix(), "catalog_root": catalog_root.as_posix()},
        "output_paths": {"source_dir": output_dir.as_posix()},
        "duration_ms": duration_ms,
        "network_fetch_allowed": False,
        "network_fetch_attempted_count": 0,
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
        "parser_ready_claimed": False,
        "chunk_ready_claimed": False,
        "kg_readiness_claimed": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "generated_at": utc_now(),
    }


def render_report(summary: Mapping[str, Any]) -> str:
    counts = summary.get("counts") if isinstance(summary.get("counts"), Mapping) else {}
    lines = [
        "# M031 Catalog-Backed Source Acquisition Replay Report",
        "",
        "This report is metadata-only and local-only. It does not embed article text, HTML snippets, PDF bytes, or base64 payloads.",
        "",
        f"- Milestone: `{summary.get('milestone_id')}`",
        f"- Slice: `{summary.get('slice_id')}`",
        f"- Selection: `{summary.get('selection_id')}`",
        f"- Status: `{summary.get('status')}`",
        f"- Captured: {counts.get('captured', 0)}",
        f"- Blocked: {counts.get('blocked', 0)}",
        f"- Failed: {counts.get('failed', 0)}",
        "- Network fetch attempted count: 0",
        "- Graph/import/LadybugDB writes: false",
        "",
        "## Failure Modes",
        "",
        "- Missing or null local source paths become `missing_local_source_path` blocked rows.",
        "- Absent local artifacts become `local_source_missing` blocked rows.",
        "- Empty local artifacts become `empty_local_source` failed rows.",
        "- Malformed JSON or unsafe input paths fail the CLI with typed diagnostics.",
        "- Network dependencies are deliberately absent: no fetch code path exists.",
        "",
        "## Load Profile",
        "",
        "The replay is bounded by the selected variants and copies one file per materialized local source. At 10x this four-ref corpus, disk I/O and metadata report size saturate before CPU or memory; recursive copying, network fetches, and catalog tree scans are not used.",
        "",
        "## Negative Tests",
        "",
        "Covered in `tests/test_m031_catalog_backed_acquisition_loader.py`: null paths, external PDF metadata-only blockers, typed catalog blocker rows, unsafe `../` source paths, absent artifacts, empty artifacts, and redaction of summary/report text.",
        "",
        "## Role Counts",
        "",
    ]
    role_counts = summary.get("per_role_terminal_state_counts") if isinstance(summary.get("per_role_terminal_state_counts"), Mapping) else {}
    for role, value in role_counts.items():
        if isinstance(value, Mapping):
            lines.append(f"- `{role}`: captured={value.get('captured', 0)} blocked={value.get('blocked', 0)} failed={value.get('failed', 0)}")
    lines.extend(["", "## Identity Counts", ""])
    identity_counts = summary.get("per_identity_terminal_state_counts") if isinstance(summary.get("per_identity_terminal_state_counts"), Mapping) else {}
    for identity, value in identity_counts.items():
        if isinstance(value, Mapping):
            lines.append(f"- `{identity}`: captured={value.get('captured', 0)} blocked={value.get('blocked', 0)} failed={value.get('failed', 0)}")
    lines.extend(["", "## Results", ""])
    for result in summary.get("results", []):
        if isinstance(result, Mapping):
            local_path = result.get("local_path") or "<none>"
            lines.append(
                f"- `{result.get('identity')}` `{result.get('source_role')}`: {result.get('status')} "
                f"({result.get('diagnostic_code')}) -> `{local_path}`"
            )
    return "\n".join(lines) + "\n"


def validate_output_metadata_only(payload: Any, *, path: str = "$", in_safe_key: bool = False) -> None:
    if isinstance(payload, Mapping):
        for key, value in payload.items():
            safe_key = key in {"media_type", "source_role", "diagnostic_code", "blocker_code", "failure_reason", "source_catalog_path", "local_path"}
            if key in FORBIDDEN_OUTPUT_KEYS:
                raise AcquisitionError("raw_payload_output_key", f"forbidden raw-payload output key at {path}.{key}")
            validate_output_metadata_only(value, path=f"{path}.{key}", in_safe_key=safe_key)
    elif isinstance(payload, list):
        for index, item in enumerate(payload):
            validate_output_metadata_only(item, path=f"{path}[{index}]", in_safe_key=in_safe_key)
    elif isinstance(payload, str):
        lowered = payload.lower()
        for snippet in FORBIDDEN_OUTPUT_SNIPPETS:
            if snippet.lower() in lowered and not in_safe_key:
                raise AcquisitionError("raw_payload_output_snippet", f"forbidden raw-payload snippet at {path}")


def assert_metadata_artifact_is_redacted(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    lowered = text.lower()
    found = [token for token in FORBIDDEN_OUTPUT_SNIPPETS if token.lower() in lowered]
    if found:
        raise AcquisitionError("raw_payload_artifact_snippet", f"metadata artifact is not redacted: {path}: {found}")


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--catalog-root", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--write-summary", required=True, type=Path)
    parser.add_argument("--write-diagnostics", required=True, type=Path)
    parser.add_argument("--write-report", required=True, type=Path)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    started = time.perf_counter()
    try:
        for cli_path in (args.selection, args.catalog_root, args.output_dir, args.write_summary, args.write_diagnostics, args.write_report):
            if not cli_path.is_absolute() and ".." in PurePosixPath(str(cli_path).replace("\\", "/")).parts:
                raise AcquisitionError("unsafe_cli_path", f"unsafe CLI path: {cli_path}")
        output_dir = args.output_dir.resolve()
        output_dir.mkdir(parents=True, exist_ok=True)
        results = replay_selection(
            selection_path=args.selection,
            catalog_root=args.catalog_root,
            output_dir=output_dir,
            write=not args.dry_run,
        )
        summary = build_summary(
            results,
            selection_path=args.selection,
            catalog_root=args.catalog_root,
            output_dir=args.output_dir,
            duration_ms=int((time.perf_counter() - started) * 1000),
        )
        validate_output_metadata_only(summary)
        report = render_report(summary)
        write_json(args.write_summary, summary)
        write_jsonl(args.write_diagnostics, results)
        atomic_write_text(args.write_report, report)
        for artifact_path in (args.write_summary, args.write_diagnostics, args.write_report):
            assert_metadata_artifact_is_redacted(artifact_path)
        sys.stdout.write(json.dumps({"status": summary["status"], "counts": summary["counts"], "summary": args.write_summary.as_posix()}, sort_keys=True) + "\n")
        return 0 if summary["counts"]["failed"] == 0 else 1
    except AcquisitionError as exc:
        sys.stderr.write(
            json.dumps(
                {
                    "status": "failed",
                    "code": exc.code,
                    "message": str(exc),
                    "identity": exc.identity,
                    "article_ref": exc.article_ref,
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
