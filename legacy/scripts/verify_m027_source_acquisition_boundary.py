#!/usr/bin/env python3
"""Local-only replay verifier for the M027 source acquisition boundary.

The verifier is fixed to the M027 mixed-source corpus by default, but accepts
path overrides for tests. It reads catalog metadata, the catalog index, the S01
selection, selected article records, and the source-acquisition handoff artifacts.
It never fetches network sources, repairs missing bytes, imports graph state, or
writes LadybugDB/production state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Iterable, Mapping
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S02"
SELECTION_ID = "m027-mixed-source-corpus-v1"
SCHEMA_VERSION = "m027-source-acquisition-replay.v1"
ROOT = Path(__file__).resolve().parents[1]

CATALOG_PATH = ROOT / "data" / "article_catalog" / "catalog.json"
CATALOG_ROOT = ROOT / "data" / "article_catalog"
INDEX_PATH = ROOT / "data" / "article_catalog" / "index.json"
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
SELECTION_PATH = CORPUS_DIR / "selection.json"
SUMMARY_PATH = CORPUS_DIR / "source-acquisition-summary.json"
DIAGNOSTICS_PATH = CORPUS_DIR / "source-acquisition-diagnostics.jsonl"
REPORT_PATH = CORPUS_DIR / "source-acquisition-report.md"

SELECTED_SOURCE_ROLES = {"arxiv_abs_page", "arxiv_pdf", "nature_html", "publisher_html"}
PDF_ROLES = {"arxiv_pdf", "publisher_pdf"}
HTML_ROLES = {"arxiv_abs_page", "nature_html", "publisher_html"}
TERMINAL_STATUSES = {"captured", "blocked", "failed"}

FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "html",
    "pdf",
    "binary",
    "bytes",
    "base64",
    "payload",
    "content",
    "body",
}
FORBIDDEN_SNIPPETS = {
    "<html",
    "</html",
    "%PDF-",
    "base64,",
    "fixture arxiv abstract page",
    "fixture nature article page",
}
UNSAFE_TRUE_FLAGS = {
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "production_import_attempted",
    "ladybugdb_written",
    "production_ladybugdb_write_allowed",
    "metadata_manifests_embed_raw_text",
    "metadata_manifests_embed_raw_binary",
    "raw_text_embedded",
    "raw_binary_embedded",
    "raw_payload_embedded_in_metadata",
    "raw_text_embedded_in_metadata",
    "raw_binary_embedded_in_metadata",
}
FAIL_CLOSED_SAFETY_FLAGS: dict[str, bool] = {
    "metadata_manifests_embed_raw_text": False,
    "metadata_manifests_embed_raw_binary": False,
    "graph_import_allowed": False,
    "production_ladybugdb_write_allowed": False,
    "trusted_kg_import_allowed": False,
    "production_import_attempted": False,
    "ladybugdb_written": False,
    "raw_text_embedded_in_metadata": False,
    "raw_binary_embedded_in_metadata": False,
}


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"expected JSON object: {path}")
    return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    if not path.exists():
        return rows
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        value = json.loads(line)
        if not isinstance(value, dict):
            raise ValueError(f"expected JSON object at {path}:{line_number}")
        rows.append(value)
    return rows


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=False) + "\n", encoding="utf-8")


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


def git_commit(root: Path) -> str | None:
    git_dir = root / ".git"
    head_path = git_dir / "HEAD"
    try:
        head = head_path.read_text(encoding="utf-8").strip()
    except OSError:
        return None
    if head.startswith("ref:"):
        ref = head.split(" ", 1)[1].strip()
        try:
            return (git_dir / ref).read_text(encoding="utf-8").strip() or None
        except OSError:
            try:
                for line in (git_dir / "packed-refs").read_text(encoding="utf-8").splitlines():
                    if line and not line.startswith("#") and line.endswith(f" {ref}"):
                        return line.split(" ", 1)[0]
            except OSError:
                return None
            return None
    return head or None


def safe_catalog_path(catalog_root: Path, rel_path: str) -> Path:
    if not isinstance(rel_path, str) or not rel_path.strip():
        raise ValueError("empty_catalog_relative_path")
    if "://" in rel_path:
        raise ValueError("url_not_allowed_as_local_path")
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        raise ValueError("unsafe_catalog_relative_path")
    root_resolved = catalog_root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError("catalog_path_escapes_root")
    return resolved


def safe_local_artifact_path(article_dir: Path, local_path: Any) -> Path:
    if not isinstance(local_path, str) or not local_path.strip():
        raise ValueError("missing_local_path")
    if "://" in local_path:
        raise ValueError("url_not_allowed_as_local_path")
    normalized = PurePosixPath(local_path.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        raise ValueError("unsafe_local_path")
    article_root = article_dir.resolve()
    resolved = (article_root / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(article_root):
        raise ValueError("local_path_escapes_article_dir")
    return resolved


def diagnostic(
    code: str,
    message: str,
    *,
    path: Path | str | None = None,
    json_path: str = "$",
    article_ref: str | None = None,
    variant_id: str | None = None,
    source_role: str | None = None,
    severity: str = "error",
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "severity": severity,
        "diagnostic_code": code,
        "code": code,
        "message": message,
        "failure_reason": message,
        "path": rel(path) if isinstance(path, Path) else path,
        "json_path": json_path,
        "article_ref": article_ref,
        "variant_id": variant_id,
        "source_role": source_role,
        "network_fetch_attempted": False,
        "validate_only": True,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def add_error(errors: list[dict[str, Any]], code: str, message: str, **kwargs: Any) -> None:
    errors.append(diagnostic(code, message, **kwargs))


def walk_json(value: Any, path: str = "$") -> Iterable[tuple[str, Any]]:
    yield path, value
    if isinstance(value, dict):
        for key, item in value.items():
            yield from walk_json(item, f"{path}.{key}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from walk_json(item, f"{path}[{index}]")


def validate_no_payload_keys(
    value: Any, errors: list[dict[str, Any]], *, artifact_path: Path, root_path: str = "$"
) -> None:
    for json_path, item in walk_json(value, root_path):
        if isinstance(item, dict):
            for key in item:
                if key in FORBIDDEN_PAYLOAD_KEYS:
                    add_error(
                        errors,
                        "raw_payload_key_leakage",
                        f"metadata contains forbidden raw payload key {key!r}",
                        path=artifact_path,
                        json_path=f"{json_path}.{key}",
                    )
                if key in UNSAFE_TRUE_FLAGS and item.get(key) is True:
                    add_error(
                        errors,
                        "unsafe_true_safety_flag",
                        f"unsafe safety flag {key!r} is true",
                        path=artifact_path,
                        json_path=f"{json_path}.{key}",
                    )


def validate_text_redaction(path: Path, errors: list[dict[str, Any]]) -> None:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        add_error(
            errors,
            "missing_metadata_artifact",
            f"failed to read metadata artifact: {exc}",
            path=path,
        )
        return
    lowered = text.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            add_error(
                errors,
                "raw_payload_snippet_leakage",
                f"metadata artifact contains forbidden payload snippet {snippet!r}",
                path=path,
            )


def selected_article_paths(
    catalog_root: Path,
    index: Mapping[str, Any],
    selection: Mapping[str, Any],
    errors: list[dict[str, Any]],
) -> list[tuple[str, Path]]:
    index_rows = index.get("articles")
    selection_rows = selection.get("articles")
    if not isinstance(index_rows, list):
        add_error(
            errors,
            "malformed_index",
            "index articles must be a list",
            path=INDEX_PATH,
            json_path="$.articles",
        )
        return []
    if not isinstance(selection_rows, list):
        add_error(
            errors,
            "malformed_selection",
            "selection articles must be a list",
            path=SELECTION_PATH,
            json_path="$.articles",
        )
        return []

    by_ref: dict[str, Mapping[str, Any]] = {}
    for row in index_rows:
        if isinstance(row, dict) and isinstance(row.get("article_ref"), str):
            by_ref[str(row["article_ref"])] = row

    paths: list[tuple[str, Path]] = []
    seen: set[Path] = set()
    for position, selected in enumerate(selection_rows):
        article_ref = selected.get("article_ref") if isinstance(selected, dict) else None
        if not isinstance(article_ref, str):
            add_error(
                errors,
                "malformed_selection_article",
                "selection article_ref is missing",
                json_path=f"$.articles[{position}]",
            )
            continue
        row = by_ref.get(article_ref)
        if row is None:
            add_error(
                errors,
                "selection_article_missing_from_index",
                f"selected article not present in index: {article_ref}",
                article_ref=article_ref,
            )
            continue
        article_path = row.get("article_path")
        if not isinstance(article_path, str):
            add_error(
                errors,
                "missing_article_path",
                f"index row missing article_path: {article_ref}",
                article_ref=article_ref,
            )
            continue
        try:
            resolved = safe_catalog_path(catalog_root, article_path)
        except ValueError as exc:
            add_error(
                errors,
                str(exc),
                f"unsafe article path for {article_ref}: {article_path}",
                article_ref=article_ref,
            )
            continue
        if resolved in seen:
            add_error(
                errors,
                "duplicate_article_path",
                f"duplicate selected article path: {article_path}",
                article_ref=article_ref,
            )
            continue
        seen.add(resolved)
        paths.append((article_ref, resolved))
    return paths


def variant_terminal_status(variant: Mapping[str, Any]) -> str | None:
    for key in ("capture_status", "acquisition_status", "status"):
        status = variant.get(key)
        if isinstance(status, str):
            return status
    return None


def validate_captured_variant(
    article_path: Path,
    article_ref: str,
    variant: Mapping[str, Any],
    errors: list[dict[str, Any]],
    *,
    json_path: str,
) -> None:
    variant_id = variant.get("variant_id") if isinstance(variant.get("variant_id"), str) else None
    source_role = (
        variant.get("source_role") if isinstance(variant.get("source_role"), str) else None
    )
    try:
        local_artifact = safe_local_artifact_path(
            article_path.parent, variant.get("path") or variant.get("local_path")
        )
    except ValueError as exc:
        add_error(
            errors,
            str(exc),
            f"captured variant has unsafe local_path: {exc}",
            path=article_path,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )
        return

    if not local_artifact.exists():
        add_error(
            errors,
            "missing_captured_file",
            f"captured local artifact does not exist: {rel(local_artifact)}",
            path=local_artifact,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )
        return
    if not local_artifact.is_file():
        add_error(
            errors,
            "captured_path_not_file",
            f"captured local artifact is not a file: {rel(local_artifact)}",
            path=local_artifact,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )
        return

    try:
        actual_size = local_artifact.stat().st_size
        actual_hash = sha256_file(local_artifact)
    except OSError as exc:
        add_error(
            errors,
            "captured_file_unreadable",
            f"captured local artifact cannot be read: {exc}",
            path=local_artifact,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )
        return

    if variant.get("byte_size") != actual_size:
        add_error(
            errors,
            "byte_size_mismatch",
            f"recorded byte_size {variant.get('byte_size')} does not match actual {actual_size}",
            path=local_artifact,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )
    if variant.get("sha256") != actual_hash:
        add_error(
            errors,
            "sha256_mismatch",
            "recorded sha256 does not match captured bytes",
            path=local_artifact,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )

    with local_artifact.open("rb") as handle:
        prefix = handle.read(5)
    if source_role in PDF_ROLES and prefix != b"%PDF-":
        add_error(
            errors,
            "bad_pdf_signature",
            "captured PDF artifact does not start with %PDF-",
            path=local_artifact,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )
    if source_role in HTML_ROLES and actual_size == 0:
        add_error(
            errors,
            "empty_html_artifact",
            "captured HTML artifact is empty",
            path=local_artifact,
            json_path=json_path,
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )


def validate_blocked_or_failed_variant(
    article_path: Path,
    article_ref: str,
    variant: Mapping[str, Any],
    errors: list[dict[str, Any]],
    *,
    json_path: str,
) -> None:
    variant_id = variant.get("variant_id") if isinstance(variant.get("variant_id"), str) else None
    source_role = (
        variant.get("source_role") if isinstance(variant.get("source_role"), str) else None
    )
    if (
        not isinstance(variant.get("diagnostic_code"), str)
        or not str(variant.get("diagnostic_code")).strip()
    ):
        add_error(
            errors,
            "missing_diagnostic_code",
            "blocked or failed variant lacks diagnostic_code",
            path=article_path,
            json_path=f"{json_path}.diagnostic_code",
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )
    if (
        not isinstance(variant.get("failure_reason"), str)
        or not str(variant.get("failure_reason")).strip()
    ):
        add_error(
            errors,
            "missing_failure_reason",
            "blocked or failed variant lacks failure_reason",
            path=article_path,
            json_path=f"{json_path}.failure_reason",
            article_ref=article_ref,
            variant_id=variant_id,
            source_role=source_role,
        )


def validate_article_records(
    article_paths: list[tuple[str, Path]], errors: list[dict[str, Any]]
) -> tuple[int, int]:
    selected_article_count = len(article_paths)
    selected_variant_count = 0
    for article_ref, article_path in article_paths:
        try:
            article = load_json(article_path)
        except (OSError, json.JSONDecodeError, ValueError) as exc:
            add_error(
                errors,
                "malformed_article_json",
                f"failed to load selected article JSON: {exc}",
                path=article_path,
                article_ref=article_ref,
            )
            continue
        validate_no_payload_keys(article, errors, artifact_path=article_path)
        variants = article.get("source_variants")
        if not isinstance(variants, list):
            add_error(
                errors,
                "malformed_source_variants",
                "article source_variants must be a list",
                path=article_path,
                article_ref=article_ref,
                json_path="$.source_variants",
            )
            continue
        for position, variant in enumerate(variants):
            if not isinstance(variant, dict):
                add_error(
                    errors,
                    "malformed_source_variant",
                    "source variant must be an object",
                    path=article_path,
                    article_ref=article_ref,
                    json_path=f"$.source_variants[{position}]",
                )
                continue
            source_role = variant.get("source_role")
            if source_role not in SELECTED_SOURCE_ROLES:
                continue
            selected_variant_count += 1
            json_path = f"$.source_variants[{position}]"
            status = variant_terminal_status(variant)  # ty:ignore[invalid-argument-type]
            if status not in TERMINAL_STATUSES:
                add_error(
                    errors,
                    "non_terminal_variant_status",
                    f"selected variant is not captured, blocked, or failed: {status}",
                    path=article_path,
                    article_ref=article_ref,
                    source_role=str(source_role),
                    json_path=json_path,
                )
                continue
            if status == "captured":
                validate_captured_variant(
                    article_path,
                    article_ref,
                    variant,  # ty:ignore[invalid-argument-type]
                    errors,
                    json_path=json_path,
                )
            else:
                validate_blocked_or_failed_variant(
                    article_path,
                    article_ref,
                    variant,  # ty:ignore[invalid-argument-type]
                    errors,
                    json_path=json_path,
                )
    return selected_article_count, selected_variant_count


def validate_summary(
    summary: Mapping[str, Any],
    errors: list[dict[str, Any]],
    *,
    selected_article_count: int,
    selected_variant_count: int,
) -> None:
    validate_no_payload_keys(summary, errors, artifact_path=SUMMARY_PATH)
    if summary.get("article_count") != selected_article_count:
        add_error(
            errors,
            "article_count_mismatch",
            f"summary article_count {summary.get('article_count')} does not match selected {selected_article_count}",
            path=SUMMARY_PATH,
            json_path="$.article_count",
        )
    if summary.get("variant_count") != selected_variant_count:
        add_error(
            errors,
            "variant_count_mismatch",
            f"summary variant_count {summary.get('variant_count')} does not match selected {selected_variant_count}",
            path=SUMMARY_PATH,
            json_path="$.variant_count",
        )
    if summary.get("network_fetch_attempted") is True:
        add_error(
            errors,
            "replay_network_attempted",
            "summary indicates network_fetch_attempted=true during replay",
            path=SUMMARY_PATH,
            json_path="$.network_fetch_attempted",
        )


def validate_diagnostics(rows: list[dict[str, Any]], errors: list[dict[str, Any]]) -> None:
    if not rows:
        add_error(
            errors,
            "missing_diagnostic_rows",
            "source-acquisition-diagnostics.jsonl contains no variant diagnostics",
            path=DIAGNOSTICS_PATH,
        )
    for position, row in enumerate(rows):
        validate_no_payload_keys(
            row, errors, artifact_path=DIAGNOSTICS_PATH, root_path=f"$[{position}]"
        )
        status = row.get("status")
        if status in {"blocked", "failed"}:
            if (
                not isinstance(row.get("diagnostic_code"), str)
                or not str(row.get("diagnostic_code")).strip()
            ):
                add_error(
                    errors,
                    "missing_diagnostic_code",
                    "blocked or failed diagnostic row lacks diagnostic_code",
                    path=DIAGNOSTICS_PATH,
                    json_path=f"$[{position}].diagnostic_code",
                )
            if (
                not isinstance(row.get("failure_reason"), str)
                or not str(row.get("failure_reason")).strip()
            ):
                add_error(
                    errors,
                    "missing_failure_reason",
                    "blocked or failed diagnostic row lacks failure_reason",
                    path=DIAGNOSTICS_PATH,
                    json_path=f"$[{position}].failure_reason",
                )


def input_paths(
    catalog_path: Path,
    index_path: Path,
    selection_path: Path,
    article_paths: list[tuple[str, Path]],
) -> list[Path]:
    return [catalog_path, index_path, selection_path, *[path for _ref, path in article_paths]]


def file_hashes(paths: Iterable[Path]) -> dict[str, str | None]:
    hashes: dict[str, str | None] = {}
    for path in paths:
        try:
            hashes[rel(path)] = sha256_file(path)
        except OSError:
            hashes[rel(path)] = None
    return hashes


def build_provenance(
    args: argparse.Namespace,
    article_paths: list[tuple[str, Path]],
    *,
    exit_code: int,
    duration_ms: int,
) -> dict[str, Any]:
    _outputs = [args.summary, args.diagnostics, args.report]
    hashable_outputs = [args.diagnostics, args.report]
    return {
        "schema_version": SCHEMA_VERSION,
        "command": ["uv", "run", "python", "scripts/verify_m027_source_acquisition_boundary.py"],
        "argv": ["scripts/verify_m027_source_acquisition_boundary.py"],
        "cwd": str(ROOT),
        "git_commit": git_commit(ROOT),
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "input_hashes": file_hashes(
            input_paths(args.catalog, args.index, args.selection, article_paths)
        ),
        "output_hashes": file_hashes(hashable_outputs),
        "output_hash_note": "source-acquisition-summary.json is intentionally excluded to avoid self-referential stale hashes",
        "exit_code": exit_code,
        "duration_ms": duration_ms,
        "validate_only": True,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
    }


def render_report_section(
    provenance: Mapping[str, Any],
    errors: list[dict[str, Any]],
    *,
    selected_article_count: int,
    selected_variant_count: int,
) -> str:
    lines = [
        "",
        "## Local-Only Replay Verification",
        "",
        "This section is metadata-only and does not embed article text, HTML snippets, PDF text, binary bytes, or base64 payloads.",
        f"- Verifier schema: `{SCHEMA_VERSION}`",
        f"- Validate only: `{provenance.get('validate_only')}`",
        "- Network fetch attempted: `False`",
        "- Production import attempted: `False`",
        "- LadybugDB written: `False`",
        "- Trusted KG import allowed: `False`",
        "- Graph import allowed: `False`",
        f"- Selected articles verified: {selected_article_count}",
        f"- Selected source variants verified: {selected_variant_count}",
        f"- Exit code: {provenance.get('exit_code')}",
        f"- Error diagnostics: {len(errors)}",
        f"- Command: `{provenance.get('command')}`",
        f"- CWD: `{provenance.get('cwd')}`",
        f"- Git commit: `{provenance.get('git_commit')}`",
    ]
    return "\n".join(lines) + "\n"


def refresh_artifacts(
    args: argparse.Namespace,
    summary: dict[str, Any],
    diagnostic_rows: list[dict[str, Any]],
    errors: list[dict[str, Any]],
    provenance: dict[str, Any],
    *,
    selected_article_count: int,
    selected_variant_count: int,
) -> None:
    summary["local_only_replay_verification"] = {
        "schema_version": SCHEMA_VERSION,
        "status": "passed" if not errors else "failed",
        "diagnostic_count": len(errors),
        "selected_article_count": selected_article_count,
        "selected_variant_count": selected_variant_count,
        "validate_only": True,
        "network_fetch_attempted": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "trusted_kg_import_allowed": False,
        "graph_import_allowed": False,
        "provenance": provenance,
    }
    write_json(args.summary, summary)
    verification_row = diagnostic(
        "local_only_replay_verification_passed"
        if not errors
        else "local_only_replay_verification_failed",
        "M027 local-only source acquisition replay verification completed",
        severity="info" if not errors else "error",
        path=args.summary,
    )
    verification_row["error_count"] = len(errors)
    retained_diagnostic_rows = [
        row
        for row in diagnostic_rows
        if str(row.get("diagnostic_code") or row.get("code") or "")
        not in {"local_only_replay_verification_passed", "local_only_replay_verification_failed"}
    ]
    write_jsonl(args.diagnostics, [*retained_diagnostic_rows, verification_row, *errors])
    existing_report = (
        args.report.read_text(encoding="utf-8")
        if args.report.exists()
        else "# M027 Source Acquisition Report\n"
    )
    marker = "\n## Local-Only Replay Verification\n"
    base_report = existing_report.split(marker, 1)[0].rstrip() + "\n"
    args.report.write_text(
        base_report
        + render_report_section(
            provenance,
            errors,
            selected_article_count=selected_article_count,
            selected_variant_count=selected_variant_count,
        ),
        encoding="utf-8",
    )
    provenance["output_hashes"] = file_hashes([args.diagnostics, args.report])
    summary["local_only_replay_verification"]["provenance"] = provenance
    write_json(args.summary, summary)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", type=Path, default=CATALOG_PATH)
    parser.add_argument("--catalog-root", type=Path, default=CATALOG_ROOT)
    parser.add_argument("--index", type=Path, default=INDEX_PATH)
    parser.add_argument("--selection", type=Path, default=SELECTION_PATH)
    parser.add_argument("--summary", type=Path, default=SUMMARY_PATH)
    parser.add_argument("--diagnostics", type=Path, default=DIAGNOSTICS_PATH)
    parser.add_argument("--report", type=Path, default=REPORT_PATH)
    return parser.parse_args(argv[1:])


def main(argv: list[str] | None = None) -> int:
    argv = argv or sys.argv
    args = parse_args(argv)
    errors: list[dict[str, Any]] = []

    try:
        catalog = load_json(args.catalog)
        index = load_json(args.index)
        selection = load_json(args.selection)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        sys.stderr.write(
            f"M027 source acquisition replay verification failed before artifact refresh: {exc}\n"
        )
        return 1

    validate_no_payload_keys(catalog, errors, artifact_path=args.catalog)
    validate_no_payload_keys(index, errors, artifact_path=args.index)
    validate_no_payload_keys(selection, errors, artifact_path=args.selection)

    article_paths = selected_article_paths(args.catalog_root, index, selection, errors)
    if len(selection.get("articles", [])) != 6:
        add_error(
            errors,
            "selected_article_count_mismatch",
            f"expected exactly six selected articles, found {len(selection.get('articles', []))}",
            path=args.selection,
            json_path="$.articles",
        )
    selected_article_count, selected_variant_count = validate_article_records(article_paths, errors)

    try:
        summary = load_json(args.summary)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        summary = {}
        add_error(
            errors,
            "malformed_summary_artifact",
            f"failed to load summary artifact: {exc}",
            path=args.summary,
        )
    try:
        diagnostic_rows = load_jsonl(args.diagnostics)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        diagnostic_rows = []
        add_error(
            errors,
            "malformed_diagnostics_artifact",
            f"failed to load diagnostics artifact: {exc}",
            path=args.diagnostics,
        )

    validate_summary(
        summary,
        errors,
        selected_article_count=selected_article_count,
        selected_variant_count=selected_variant_count,
    )
    validate_diagnostics(diagnostic_rows, errors)
    for artifact_path in (args.summary, args.diagnostics, args.report):
        validate_text_redaction(artifact_path, errors)

    duration_ms = 0
    exit_code = 0 if not errors else 1
    provenance = build_provenance(args, article_paths, exit_code=exit_code, duration_ms=duration_ms)
    refresh_artifacts(
        args,
        summary,
        diagnostic_rows,
        errors,
        provenance,
        selected_article_count=selected_article_count,
        selected_variant_count=selected_variant_count,
    )

    if errors:
        sys.stderr.write("M027 source acquisition replay verification failed:\n")
        for row in errors:
            sys.stderr.write(f"- {row['diagnostic_code']}: {row['message']}\n")
        return 1
    sys.stdout.write(
        "M027 source acquisition replay verification passed: six selected articles, "
        "terminal variant states, local artifact hashes, redaction constraints, and "
        "fail-closed graph/production flags are valid.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
