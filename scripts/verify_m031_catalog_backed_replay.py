#!/usr/bin/env python3
"""Verify M031 S02 catalog-backed acquisition and loader replay closeout.

The verifier composes the T01 selection, T02 acquisition summary, and T03
loader evidence summary into one deterministic S02 contract. It only inspects
metadata plus local captured artifact hashes/sizes; it does not parse article
contents, fetch the network, write graph data, or claim parser/conversion/chunk
readiness.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
import time
from collections import Counter, defaultdict
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S02"
SELECTION_ID = "m031-catalog-backed-replay-v1"
SCHEMA_VERSION = "m031-catalog-backed-closeout.v1"

EXPECTED_COUNTS = {
    "requested_ref_count": 4,
    "catalog_backed_count": 3,
    "typed_catalog_blocker_count": 1,
    "silent_missing_count": 0,
}

FALSE_SAFETY_FLAGS = {
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

TRUE_METADATA_FLAGS = {
    "metadata_only_selection",
    "metadata_only_acquisition",
    "metadata_only_loader_evidence",
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
FORBIDDEN_OUTPUT_SNIPPETS = ("<html", "</html", "%PDF-", "base64,", '"text":', '"raw_binary":')
SAFE_TEXT_KEYS = {
    "article_ref",
    "blocker_code",
    "code",
    "diagnostic_code",
    "event_path",
    "identity",
    "json_path",
    "local_path",
    "message",
    "path",
    "report_path",
    "source_role",
    "source_type",
    "status",
}


class CloseoutError(ValueError):
    """Typed contract failure used for deterministic closeout diagnostics."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        identity: str | None = None,
        article_ref: str | None = None,
        json_path: str = "$",
    ) -> None:
        super().__init__(message)
        self.code = code
        self.identity = identity
        self.article_ref = article_ref
        self.json_path = json_path


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


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


def load_json_object(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise CloseoutError(
            "malformed_json", f"malformed JSON at {path}: {exc}", json_path=str(path)
        ) from exc
    except OSError as exc:
        raise CloseoutError(
            "json_read_failed", f"failed to read {path}: {exc}", json_path=str(path)
        ) from exc
    if not isinstance(payload, dict):
        raise CloseoutError(
            "malformed_json_object", f"expected JSON object at {path}", json_path=str(path)
        )
    return payload


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_child_path(
    root: Path, rel_path: str | None, *, code: str = "unsafe_relative_path"
) -> Path:
    if not isinstance(rel_path, str) or not rel_path.strip():
        raise CloseoutError(code, f"empty unsafe relative path: {rel_path!r}")
    if "://" in rel_path:
        raise CloseoutError(
            "url_not_allowed_as_local_path", f"URL cannot be used as a local path: {rel_path}"
        )
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part in ("", ".") for part in normalized.parts)
    ):
        raise CloseoutError(code, f"unsafe relative path: {rel_path}")
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise CloseoutError(code, f"path escapes root: {rel_path}")
    return resolved


def _rows(payload: Mapping[str, Any], key: str, *, code: str) -> list[dict[str, Any]]:
    value = payload.get(key)
    if not isinstance(value, list):
        raise CloseoutError(code, f"{key} must be a list", json_path=f"$.{key}")
    rows: list[dict[str, Any]] = []
    for index, row in enumerate(value):
        if not isinstance(row, dict):
            raise CloseoutError(
                code, f"{key}[{index}] must be an object", json_path=f"$.{key}[{index}]"
            )
        rows.append(row)  # ty:ignore[invalid-argument-type]
    return rows


def _row_key(row: Mapping[str, Any]) -> tuple[str | None, str | None, str | None, str | None]:
    return (
        row.get("identity") if isinstance(row.get("identity"), str) else None,
        row.get("article_ref") if isinstance(row.get("article_ref"), str) else None,
        row.get("variant_id") if isinstance(row.get("variant_id"), str) else None,
        row.get("source_role") if isinstance(row.get("source_role"), str) else None,
    )


def diagnostic(
    code: str,
    message: str,
    *,
    severity: str = "info",
    row: Mapping[str, Any] | None = None,
    json_path: str = "$",
) -> dict[str, Any]:
    row = row or {}
    return {
        "schema_version": SCHEMA_VERSION,
        "code": code,
        "severity": severity,
        "identity": row.get("identity"),
        "article_ref": row.get("article_ref"),
        "source_role": row.get("source_role"),
        "local_path": row.get("local_path"),
        "json_path": json_path,
        "message": message,
    }


def error_to_diagnostic(exc: CloseoutError) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "code": exc.code,
        "severity": "error",
        "identity": exc.identity,
        "article_ref": exc.article_ref,
        "source_role": None,
        "local_path": None,
        "json_path": exc.json_path,
        "message": str(exc),
    }


def assert_ids(
    selection: Mapping[str, Any], acquisition: Mapping[str, Any], loader: Mapping[str, Any]
) -> None:
    for name, payload in (
        ("selection", selection),
        ("acquisition", acquisition),
        ("loader", loader),
    ):
        if payload.get("milestone_id") != MILESTONE_ID:
            raise CloseoutError(
                "milestone_id_mismatch",
                f"{name} milestone_id mismatch",
                json_path=f"$.{name}.milestone_id",
            )
        if payload.get("slice_id") != SLICE_ID:
            raise CloseoutError(
                "slice_id_mismatch", f"{name} slice_id mismatch", json_path=f"$.{name}.slice_id"
            )
        if payload.get("selection_id") != SELECTION_ID:
            raise CloseoutError(
                "selection_id_mismatch",
                f"{name} selection_id mismatch",
                json_path=f"$.{name}.selection_id",
            )


def assert_selection_contract(
    selection: Mapping[str, Any], diagnostics: list[dict[str, Any]]
) -> tuple[set[str], list[dict[str, Any]], list[dict[str, Any]]]:
    counts = selection.get("counts")
    if not isinstance(counts, Mapping):
        raise CloseoutError(
            "selection_counts_missing", "selection is missing counts", json_path="$.counts"
        )
    for key, expected in EXPECTED_COUNTS.items():
        if counts.get(key) != expected:
            raise CloseoutError(
                "selection_count_mismatch",
                f"selection count {key}={counts.get(key)!r}, expected {expected}",
                json_path=f"$.counts.{key}",
            )
    requested_refs = _rows(selection, "requested_refs", code="selection_requested_refs_malformed")
    articles = _rows(selection, "articles", code="selection_articles_malformed")
    blockers = _rows(selection, "catalog_blockers", code="selection_blockers_malformed")
    identities = {
        row.get("identity") for row in requested_refs if isinstance(row.get("identity"), str)
    }
    if len(identities) != EXPECTED_COUNTS["requested_ref_count"]:
        raise CloseoutError(
            "requested_identity_count_mismatch",
            "requested identities are missing or duplicated",
            json_path="$.requested_refs",
        )
    represented = {
        row.get("identity") for row in articles + blockers if isinstance(row.get("identity"), str)
    }
    missing = identities - represented
    extra = represented - identities
    if missing or extra:
        raise CloseoutError(
            "requested_identity_not_represented",
            f"identity representation mismatch missing={sorted(missing)} extra={sorted(extra)}",
        )
    for article in articles:
        if not article.get("article_ref") or not article.get("article_path"):
            raise CloseoutError(
                "catalog_backed_row_missing_article_json",
                "catalog-backed row lacks article_ref/article_path",
                identity=article.get("identity"),
                json_path="$.articles",
            )
    for blocker in blockers:
        if not blocker.get("blocker_code"):
            raise CloseoutError(
                "typed_blocker_missing_code",
                "typed blocker row lacks blocker_code",
                identity=blocker.get("identity"),
                json_path="$.catalog_blockers",
            )
    diagnostics.append(
        diagnostic(
            "selection_contract_ok",
            "all requested identities are represented by catalog rows or typed blockers",
        )
    )
    return identities, articles, blockers  # ty:ignore[invalid-return-type]


def assert_flag_value(name: str, value: Any, *, json_path: str) -> None:
    if name in TRUE_METADATA_FLAGS:
        if value is not True:
            raise CloseoutError(
                "metadata_only_flag_not_true",
                f"metadata-only safety flag {name}={value!r}",
                json_path=json_path,
            )
    elif name in FALSE_SAFETY_FLAGS and value is not False:
        raise CloseoutError(
            "unsafe_safety_flag", f"unsafe safety flag {name}={value!r}", json_path=json_path
        )


def assert_fail_closed_flags(payload: Any, *, json_path: str = "$") -> None:
    if isinstance(payload, Mapping):
        for key, value in payload.items():
            if key in FALSE_SAFETY_FLAGS or key in TRUE_METADATA_FLAGS:
                assert_flag_value(key, value, json_path=f"{json_path}.{key}")
            if (
                key == "fail_closed_safety_flags"
                or key == "safety_flags"
                or key.endswith("_safety_flags")
            ):
                if not isinstance(value, Mapping):
                    raise CloseoutError(
                        "safety_flags_malformed",
                        f"{key} must be an object",
                        json_path=f"{json_path}.{key}",
                    )
                for flag, flag_value in value.items():
                    if isinstance(flag, str):
                        assert_flag_value(flag, flag_value, json_path=f"{json_path}.{key}.{flag}")
            assert_fail_closed_flags(value, json_path=f"{json_path}.{key}")
    elif isinstance(payload, list):
        for index, item in enumerate(payload):
            assert_fail_closed_flags(item, json_path=f"{json_path}[{index}]")


def validate_metadata_only(payload: Any, *, path: str = "$", in_safe_key: bool = False) -> None:
    if isinstance(payload, Mapping):
        for key, value in payload.items():
            safe_key = str(key) in SAFE_TEXT_KEYS
            if key in FORBIDDEN_OUTPUT_KEYS:
                raise CloseoutError(
                    "raw_payload_output_key",
                    f"forbidden raw-payload output key at {path}.{key}",
                    json_path=f"{path}.{key}",
                )
            validate_metadata_only(value, path=f"{path}.{key}", in_safe_key=safe_key)
    elif isinstance(payload, list):
        for index, item in enumerate(payload):
            validate_metadata_only(item, path=f"{path}[{index}]", in_safe_key=in_safe_key)
    elif isinstance(payload, str) and not in_safe_key:
        lowered = payload.lower()
        for snippet in FORBIDDEN_OUTPUT_SNIPPETS:
            if snippet.lower() in lowered:
                raise CloseoutError(
                    "raw_payload_output_snippet",
                    f"forbidden raw-payload snippet at {path}",
                    json_path=path,
                )


def assert_artifact_redacted(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    lowered = text.lower()
    found = [snippet for snippet in FORBIDDEN_OUTPUT_SNIPPETS if snippet.lower() in lowered]
    if found:
        raise CloseoutError(
            "raw_payload_artifact_snippet",
            f"metadata artifact is not redacted: {path}: {found}",
            json_path=path.as_posix(),
        )


def assert_hashes_and_paths(
    acquisition_rows: list[dict[str, Any]],
    loader_rows: list[dict[str, Any]],
    *,
    source_dir: Path,
    loader_dir: Path,
    diagnostics: list[dict[str, Any]],
) -> None:
    for row in acquisition_rows:
        local_path = row.get("local_path")
        if isinstance(local_path, str):
            source_path = safe_child_path(
                source_dir, local_path, code="unsafe_acquisition_local_path"
            )
            for safe_path in row.get("safe_local_paths", []):
                safe_child_path(source_dir, safe_path, code="unsafe_acquisition_safe_local_path")
            if row.get("status") == "captured":
                if not source_path.exists() or not source_path.is_file():
                    raise CloseoutError(
                        "captured_file_missing",
                        f"captured file is missing: {local_path}",
                        identity=row.get("identity"),
                        article_ref=row.get("article_ref"),
                    )
                expected_hash = row.get("sha256")
                expected_size = row.get("byte_size")
                actual_hash = sha256_file(source_path)
                actual_size = source_path.stat().st_size
                if expected_hash != actual_hash:
                    raise CloseoutError(
                        "captured_hash_mismatch",
                        f"captured hash mismatch for {local_path}",
                        identity=row.get("identity"),
                        article_ref=row.get("article_ref"),
                    )
                if expected_size != actual_size:
                    raise CloseoutError(
                        "captured_byte_size_mismatch",
                        f"captured byte size mismatch for {local_path}",
                        identity=row.get("identity"),
                        article_ref=row.get("article_ref"),
                    )
                diagnostics.append(
                    diagnostic(
                        "captured_file_hash_ok", "captured file hash and byte size match", row=row
                    )
                )
    for row in loader_rows:
        if isinstance(row.get("local_path"), str):
            safe_child_path(source_dir, row["local_path"], code="unsafe_loader_local_path")
        if isinstance(row.get("event_path"), str):
            event_path = safe_child_path(
                loader_dir, row["event_path"], code="unsafe_loader_event_path"
            )
            if row.get("loader_attempted") is True and not event_path.exists():
                raise CloseoutError(
                    "loader_event_log_missing",
                    f"loader event log is missing: {row['event_path']}",
                    identity=row.get("identity"),
                    article_ref=row.get("article_ref"),
                )
            if event_path.exists():
                assert_artifact_redacted(event_path)
                diagnostics.append(
                    diagnostic(
                        "loader_event_log_redacted",
                        "loader event log path is confined and redacted",
                        row=row,
                    )
                )


def assert_acquisition_loader_alignment(
    acquisition_rows: list[dict[str, Any]],
    loader_rows: list[dict[str, Any]],
    diagnostics: list[dict[str, Any]],
) -> None:
    acquisition_by_key = {_row_key(row): row for row in acquisition_rows}
    loader_by_key = {_row_key(row): row for row in loader_rows}
    if len(acquisition_by_key) != len(acquisition_rows):
        raise CloseoutError("duplicate_acquisition_row", "acquisition rows are not uniquely keyed")
    if len(loader_by_key) != len(loader_rows):
        raise CloseoutError("duplicate_loader_row", "loader rows are not uniquely keyed")
    missing_loader = set(acquisition_by_key) - set(loader_by_key)
    unexpected_loader = set(loader_by_key) - set(acquisition_by_key)
    if missing_loader or unexpected_loader:
        raise CloseoutError(
            "loader_acquisition_row_mismatch",
            f"loader/acquisition rows differ missing={sorted(missing_loader)} extra={sorted(unexpected_loader)}",
        )
    captured_keys = {
        key for key, row in acquisition_by_key.items() if row.get("status") == "captured"
    }
    attempted_keys = {
        key for key, row in loader_by_key.items() if row.get("loader_attempted") is True
    }
    if captured_keys != attempted_keys:
        raise CloseoutError(
            "loader_attempt_captured_mismatch",
            f"loader attempts do not match captured acquisition rows missing={sorted(captured_keys - attempted_keys)} extra={sorted(attempted_keys - captured_keys)}",
        )
    for key, acquisition_row in acquisition_by_key.items():
        loader_row = loader_by_key[key]
        if acquisition_row.get("status") != "captured":
            if (
                loader_row.get("status") != "blocked"
                or loader_row.get("loader_attempted") is not False
            ):
                raise CloseoutError(
                    "loader_blocker_missing",
                    "non-captured acquisition row is not a loader blocker",
                    identity=acquisition_row.get("identity"),
                    article_ref=acquisition_row.get("article_ref"),
                )
            if not loader_row.get("blocker_code"):
                raise CloseoutError(
                    "loader_blocker_missing_code",
                    "loader blocker row lacks blocker_code",
                    identity=acquisition_row.get("identity"),
                    article_ref=acquisition_row.get("article_ref"),
                )
        else:
            if loader_row.get("sha256") != acquisition_row.get("sha256") or loader_row.get(
                "byte_size"
            ) != acquisition_row.get("byte_size"):
                raise CloseoutError(
                    "loader_capture_hash_size_mismatch",
                    "loader row does not preserve acquisition hash/size",
                    identity=acquisition_row.get("identity"),
                    article_ref=acquisition_row.get("article_ref"),
                )
    diagnostics.append(
        diagnostic(
            "acquisition_loader_alignment_ok",
            "loader attempts exactly match captured acquisition rows and blockers align to non-captured rows",
        )
    )


def assert_summary_counts(
    selection_identities: set[str],
    selection_articles: list[dict[str, Any]],
    selection_blockers: list[dict[str, Any]],
    acquisition: Mapping[str, Any],
    loader: Mapping[str, Any],
    diagnostics: list[dict[str, Any]],
) -> dict[str, Any]:
    acquisition_rows = _rows(acquisition, "results", code="acquisition_results_malformed")
    loader_rows = _rows(loader, "results", code="loader_results_malformed")
    acquisition_counts = dict(Counter(row.get("status") for row in acquisition_rows))
    loader_status_counts = Counter(row.get("status") for row in loader_rows)
    loader_counts = {
        "loader_attempted": sum(1 for row in loader_rows if row.get("loader_attempted") is True),
        "loaded": loader_status_counts.get("loaded", 0),
        "loaded_metadata_only": loader_status_counts.get("loaded_metadata_only", 0),
        "failed": loader_status_counts.get("failed", 0),
        "loader_blocked": loader_status_counts.get("blocked", 0),
    }
    if acquisition.get("counts") != {
        "captured": acquisition_counts.get("captured", 0),
        "blocked": acquisition_counts.get("blocked", 0),
        "failed": acquisition_counts.get("failed", 0),
    }:
        raise CloseoutError(
            "acquisition_count_mismatch",
            "acquisition summary counts do not match result rows",
            json_path="$.acquisition.counts",
        )
    if loader.get("counts") != loader_counts:
        raise CloseoutError(
            "loader_count_mismatch",
            "loader summary counts do not match result rows",
            json_path="$.loader.counts",
        )
    row_identities = {
        row.get("identity")
        for row in acquisition_rows + loader_rows
        if isinstance(row.get("identity"), str)
    }
    if row_identities != selection_identities:
        raise CloseoutError(
            "terminal_identity_coverage_mismatch",
            f"terminal rows do not cover requested identities: {sorted(selection_identities - row_identities)}",
        )
    expected_terminal_rows = sum(
        len(article.get("source_variants", [])) for article in selection_articles
    ) + len(selection_blockers)
    if (
        len(acquisition_rows) != expected_terminal_rows
        or len(loader_rows) != expected_terminal_rows
    ):
        raise CloseoutError(
            "terminal_row_count_mismatch",
            f"expected {expected_terminal_rows} terminal rows, got acquisition={len(acquisition_rows)} loader={len(loader_rows)}",
        )
    diagnostics.append(
        diagnostic("summary_counts_ok", "selection, acquisition, and loader counts agree")
    )
    return {
        "acquisition_counts": dict(acquisition_counts),
        "loader_counts": loader_counts,
        "terminal_row_count": expected_terminal_rows,
    }


def per_identity_counts(
    rows: list[dict[str, Any]], *, status_key: str = "status"
) -> dict[str, dict[str, int]]:
    counts: dict[str, dict[str, int]] = defaultdict(dict)
    for row in rows:
        identity = (
            row.get("identity") if isinstance(row.get("identity"), str) else "<missing-identity>"
        )
        status = str(row.get(status_key))
        counts[identity][status] = counts[identity].get(status, 0) + 1  # ty:ignore[invalid-argument-type]
    return {key: dict(value) for key, value in sorted(counts.items())}


def verify_contract(
    *,
    selection_path: Path,
    acquisition_summary_path: Path,
    loader_summary_path: Path,
    source_dir: Path,
    loader_dir: Path,
    duration_ms: int = 0,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    selection = load_json_object(selection_path)
    acquisition = load_json_object(acquisition_summary_path)
    loader = load_json_object(loader_summary_path)
    assert_ids(selection, acquisition, loader)
    selection_identities, selection_articles, selection_blockers = assert_selection_contract(
        selection, diagnostics
    )
    assert_fail_closed_flags(selection, json_path="$.selection")
    assert_fail_closed_flags(acquisition, json_path="$.acquisition")
    assert_fail_closed_flags(loader, json_path="$.loader")
    count_payload = assert_summary_counts(
        selection_identities,
        selection_articles,
        selection_blockers,
        acquisition,
        loader,
        diagnostics,
    )
    acquisition_rows = _rows(acquisition, "results", code="acquisition_results_malformed")
    loader_rows = _rows(loader, "results", code="loader_results_malformed")
    assert_hashes_and_paths(
        acquisition_rows,
        loader_rows,
        source_dir=source_dir,
        loader_dir=loader_dir,
        diagnostics=diagnostics,
    )
    assert_acquisition_loader_alignment(acquisition_rows, loader_rows, diagnostics)
    redaction_payload = {"selection": selection, "acquisition": acquisition, "loader": loader}
    validate_metadata_only(redaction_payload)
    captured_count = count_payload["acquisition_counts"].get("captured", 0)
    blocked_count = count_payload["acquisition_counts"].get("blocked", 0)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed",
        "requested_ref_count": len(selection_identities),
        "catalog_backed_count": len(selection_articles),
        "typed_catalog_blocker_count": len(selection_blockers),
        "terminal_row_count": count_payload["terminal_row_count"],
        "counts": {
            "captured_acquisition_rows": captured_count,
            "typed_or_terminal_blocker_rows": blocked_count,
            **count_payload["loader_counts"],
        },
        "per_identity_acquisition_state_counts": per_identity_counts(acquisition_rows),
        "per_identity_loader_state_counts": per_identity_counts(loader_rows),
        "input_paths": {
            "selection": selection_path.as_posix(),
            "acquisition_summary": acquisition_summary_path.as_posix(),
            "loader_summary": loader_summary_path.as_posix(),
            "source_dir": source_dir.as_posix(),
            "loader_dir": loader_dir.as_posix(),
        },
        "fail_closed_safety_flags": {
            "metadata_only_closeout": True,
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
        },
        "duration_ms": duration_ms,
        "generated_at": utc_now(),
    }
    validate_metadata_only(summary)
    diagnostics.append(
        diagnostic("closeout_contract_passed", "S02 closeout evidence contract passed")
    )
    return summary, diagnostics


def render_report(summary: Mapping[str, Any], diagnostics: list[dict[str, Any]]) -> str:
    counts = summary.get("counts") if isinstance(summary.get("counts"), Mapping) else {}
    lines = [
        "# M031 S02 Catalog-Backed Replay Closeout Report",
        "",
        "This report is metadata-only and local-only. It records acquisition and loader evidence only; it does not embed article text, raw HTML, PDF bytes, binary payloads, or base64 data.",
        "",
        f"- Status: `{summary.get('status')}`",
        f"- Requested identities: {summary.get('requested_ref_count')}",
        f"- Catalog-backed identities: {summary.get('catalog_backed_count')}",
        f"- Typed catalog blockers: {summary.get('typed_catalog_blocker_count')}",
        f"- Terminal acquisition/loader rows: {summary.get('terminal_row_count')}",
        f"- Captured acquisition rows: {counts.get('captured_acquisition_rows', 0)}",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        f"- Blocked acquisition rows: {counts.get('typed_or_terminal_blocker_rows', 0)}",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        f"- Loader attempted rows: {counts.get('loader_attempted', 0)}",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        f"- Loader blockers: {counts.get('loader_blocked', 0)}",  # pyrefly: ignore [bad-assignment, missing-attribute]  # ty:ignore[unresolved-attribute]
        "- Graph/import/LadybugDB flags: false",
        "",
        "## Scope Boundary",
        "",
        "S02 supplies deterministic catalog-backed acquisition and loader replay evidence only. Parser readiness, conversion readiness, chunk readiness, graph import readiness, trusted KG import, production import, and LadybugDB writes remain explicitly false and are left to downstream slices.",
        "",
        "## Failure Modes",
        "",
        "- Filesystem inputs: missing or malformed JSON, missing captured files, unsafe relative paths, and Markdown write failures fail the verifier with typed diagnostics.",
        "- Local artifact integrity: stale hash or byte-size mismatches fail closed before reporting success.",
        "- Loader/acquisition agreement: omitted blocked rows, unexpected loader attempts, and missing loader blockers fail closed.",
        "- Report safety: raw payload snippets, forbidden output keys, or unsafe true graph/import/LadybugDB flags fail closed; report generation is required and is not warning-only.",
        "- External APIs/network/subprocesses: none are invoked by this verifier.",
        "",
        "## Load Profile",
        "",
        "The verifier is linear over selected terminal rows and captured local files. At 10x the current four-ref scope, local disk hashing of captured artifacts saturates before JSON processing; there is no network, subprocess, graph write, recursive catalog scan, or database write path.",
        "",
        "## Negative Tests",
        "",
        "Covered in `tests/test_m031_catalog_backed_acquisition_loader.py`: omitted identity or blocked row, selected variant without terminal acquisition state, missing loader blocker, loader/acquisition mismatch, loader event text leakage, unsafe true graph/import/production/LadybugDB flags, hash mismatch, and path escape rejection.",
        "",
        "## Diagnostics",
        "",
    ]
    for item in diagnostics:
        lines.append(f"- `{item.get('code')}` ({item.get('severity')}): {item.get('message')}")
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--acquisition-summary", required=True, type=Path)
    parser.add_argument("--loader-summary", required=True, type=Path)
    parser.add_argument("--source-dir", required=True, type=Path)
    parser.add_argument("--loader-dir", required=True, type=Path)
    parser.add_argument("--write-summary", required=True, type=Path)
    parser.add_argument("--write-diagnostics", required=True, type=Path)
    parser.add_argument("--write-report", required=True, type=Path)
    return parser.parse_args(argv)


def _reject_unsafe_cli_paths(paths: Iterable[Path]) -> None:
    for cli_path in paths:
        normalized = PurePosixPath(str(cli_path).replace("\\", "/"))
        if not cli_path.is_absolute() and ".." in normalized.parts:
            raise CloseoutError("unsafe_cli_path", f"unsafe CLI path: {cli_path}")


def failed_summary(exc: CloseoutError, *, duration_ms: int) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "failed",
        "failure": {
            "code": exc.code,
            "message": str(exc),
            "identity": exc.identity,
            "article_ref": exc.article_ref,
            "json_path": exc.json_path,
        },
        "fail_closed_safety_flags": {
            "metadata_only_closeout": True,
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
        },
        "duration_ms": duration_ms,
        "generated_at": utc_now(),
    }


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    started = time.perf_counter()
    try:
        _reject_unsafe_cli_paths(
            (
                args.selection,
                args.acquisition_summary,
                args.loader_summary,
                args.source_dir,
                args.loader_dir,
                args.write_summary,
                args.write_diagnostics,
                args.write_report,
            )
        )
        summary, diagnostics = verify_contract(
            selection_path=args.selection,
            acquisition_summary_path=args.acquisition_summary,
            loader_summary_path=args.loader_summary,
            source_dir=args.source_dir,
            loader_dir=args.loader_dir,
            duration_ms=int((time.perf_counter() - started) * 1000),
        )
        report = render_report(summary, diagnostics)
        validate_metadata_only({"summary": summary, "diagnostics": diagnostics, "report": report})
        write_json(args.write_summary, summary)
        write_jsonl(args.write_diagnostics, diagnostics)
        atomic_write_text(args.write_report, report)
        for artifact_path in (args.write_summary, args.write_diagnostics, args.write_report):
            assert_artifact_redacted(artifact_path)
        sys.stdout.write(
            json.dumps(
                {
                    "status": "passed",
                    "summary": args.write_summary.as_posix(),
                    "counts": summary["counts"],
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 0
    except CloseoutError as exc:
        duration_ms = int((time.perf_counter() - started) * 1000)
        diagnostics = [error_to_diagnostic(exc)]
        summary = failed_summary(exc, duration_ms=duration_ms)
        try:
            report = render_report(summary, diagnostics)
            write_json(args.write_summary, summary)
            write_jsonl(args.write_diagnostics, diagnostics)
            atomic_write_text(args.write_report, report)
        except OSError as write_exc:
            sys.stderr.write(
                json.dumps(
                    {
                        "status": "failed",
                        "code": "closeout_report_write_failed",
                        "message": str(write_exc),
                    },
                    sort_keys=True,
                )
                + "\n"
            )
            return 2
        sys.stderr.write(
            json.dumps({"status": "failed", "code": exc.code, "message": str(exc)}, sort_keys=True)
            + "\n"
        )
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
