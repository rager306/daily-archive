#!/usr/bin/env python3
"""Local-only source materialization for the M029 unified corpus.

M029 starts from a mixed selection: some rows point at fully registered catalog
records with previously captured source artifacts, while the M028 expansion rows
are selection-only placeholders. This command never fetches the network. It
copies already-local catalog artifacts into the corpus-local ``source/`` tree and
emits explicit blocked diagnostics for unresolved or missing sources.
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

MILESTONE_ID = "M029-eb0ljz"
SLICE_ID = "S02"
SELECTION_ID = "m029-unified-corpus-v1"
SCHEMA_VERSION = "m029-source-acquisition.v1"

ROLE_TARGETS: dict[str, str] = {
    "arxiv_html": "source/article.html",
    "arxiv_abs_page": "source/abs.html",
    "arxiv_pdf": "source/original.pdf",
    "nature_html": "source/article.html",
    "publisher_html": "source/article.html",
    "web_article_html": "source/article.html",
    "vendor_blog_html": "source/article.html",
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

FORBIDDEN_RESULT_KEYS = {
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
FORBIDDEN_SNIPPETS = ("<html", "</html", "%PDF-", "base64,")


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, dict):
        raise ValueError(f"expected JSON object: {path}")
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
        raise ValueError(f"empty_{code}")
    if "://" in rel_path:
        raise ValueError("url_not_allowed_as_local_path")
    normalized = PurePosixPath(rel_path.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        raise ValueError(code)
    root_resolved = root.resolve()
    resolved = (root_resolved / normalized.as_posix()).resolve()
    if not resolved.is_relative_to(root_resolved):
        raise ValueError(code)
    return resolved


def safe_article_segment(article_ref: str | None, article_key: str | None) -> str:
    raw = article_ref or f"unresolved/{article_key or 'unknown'}"
    normalized = PurePosixPath(raw.replace("\\", "/"))
    parts = [part for part in normalized.parts if part not in ("", ".")]
    if not parts or normalized.is_absolute() or ".." in parts:
        return f"unsafe/{hashlib.sha256(raw.encode('utf-8')).hexdigest()[:16]}"
    return "/".join(parts)


def confined_source_target(
    output_dir: Path, article_ref: str | None, article_key: str | None, role_target: str
) -> tuple[Path, str]:
    rel_path = f"{safe_article_segment(article_ref, article_key)}/{role_target}"
    target = safe_child_path(output_dir, rel_path, code="unsafe_output_source_path")
    return target, target.relative_to(output_dir.resolve()).as_posix()


def normalize_strategy_role(row: Mapping[str, Any]) -> str | None:
    role = row.get("source_strategy")
    if role == "nature_html":
        return "nature_html"
    if role == "web_article_html":
        return "web_article_html"
    if role == "arxiv_html":
        return "arxiv_html"
    if role == "arxiv_abs_page":
        return "arxiv_abs_page"
    if isinstance(row.get("source_code"), str) and row.get("source_code") == "arxiv":
        return "arxiv_abs_page"
    return role if isinstance(role, str) else None


def result_base(
    *,
    selection_row: Mapping[str, Any],
    variant: Mapping[str, Any] | None,
    source_role: str | None,
    status: str,
    diagnostic_code: str,
    failure_reason: str | None,
    local_path: str | None,
    source_catalog_path: str | None,
    sha256: str | None = None,
    byte_size: int = 0,
    media_type: str | None = None,
) -> dict[str, Any]:
    article_ref = (
        selection_row.get("article_ref")
        if isinstance(selection_row.get("article_ref"), str)
        else None
    )
    article_key = (
        selection_row.get("article_key")
        if isinstance(selection_row.get("article_key"), str)
        else None
    )
    url = None
    if variant and isinstance(variant.get("url"), str):
        url = variant.get("url")
    if url is None:
        url = (
            selection_row.get("seed_url")
            if isinstance(selection_row.get("seed_url"), str)
            else selection_row.get("canonical_url")
        )
    result = {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "article_ref": article_ref,
        "article_key": article_key,
        "identity_key": selection_row.get("identity_key")
        if isinstance(selection_row.get("identity_key"), str)
        else None,
        "catalog_resolution": selection_row.get("catalog_resolution")
        if isinstance(selection_row.get("catalog_resolution"), str)
        else None,
        "variant_id": variant.get("variant_id")
        if variant and isinstance(variant.get("variant_id"), str)
        else None,
        "source_role": source_role,
        "url_role": source_role,
        "url": url,
        "source_strategy": selection_row.get("source_strategy")
        if isinstance(selection_row.get("source_strategy"), str)
        else None,
        "status": status,
        "terminal_state": status,
        "diagnostic_code": diagnostic_code,
        "failure_reason": failure_reason,
        "local_path": local_path,
        "source_catalog_path": source_catalog_path,
        "sha256": sha256,
        "byte_size": byte_size,
        "media_type": media_type,
        "network_fetch_attempted": False,
        "capture_phase_network_allowed": False,
        "replay_phase_network_allowed": False,
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
    }
    assert not (set(result) & FORBIDDEN_RESULT_KEYS)
    return result


def selected_index_rows(index: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    rows = index.get("articles")
    if not isinstance(rows, list):
        raise ValueError("malformed index articles")
    by_ref: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if isinstance(row, dict) and isinstance(row.get("article_ref"), str):
            by_ref[str(row["article_ref"])] = row
    return by_ref


def source_result_for_variant(
    *,
    selection_row: Mapping[str, Any],
    article_path: Path,
    variant: Mapping[str, Any],
    output_dir: Path,
    write: bool,
) -> dict[str, Any]:
    role = variant.get("source_role") if isinstance(variant.get("source_role"), str) else None
    if role not in ROLE_TARGETS:
        return result_base(
            selection_row=selection_row,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code="unsupported_source_role",
            failure_reason="source role is not part of the M029 acquisition boundary",
            local_path=None,
            source_catalog_path=None,
            media_type=variant.get("media_type")
            if isinstance(variant.get("media_type"), str)
            else None,
        )

    article_dir = article_path.parent
    supplied = variant.get("local_path") or variant.get("path")
    if not isinstance(supplied, str):
        return result_base(
            selection_row=selection_row,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code="missing_local_source_path",
            failure_reason="catalog source variant does not expose a local path",
            local_path=None,
            source_catalog_path=None,
            media_type=variant.get("media_type")
            if isinstance(variant.get("media_type"), str)
            else None,
        )
    try:
        source_path = safe_child_path(article_dir, supplied, code="unsafe_catalog_source_path")
    except ValueError as exc:
        return result_base(
            selection_row=selection_row,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code=str(exc),
            failure_reason="catalog source path is not allowed",
            local_path=None,
            source_catalog_path=supplied,
            media_type=variant.get("media_type")
            if isinstance(variant.get("media_type"), str)
            else None,
        )
    target, local_path = confined_source_target(
        output_dir,
        selection_row.get("article_ref")
        if isinstance(selection_row.get("article_ref"), str)
        else None,
        selection_row.get("article_key")
        if isinstance(selection_row.get("article_key"), str)
        else None,
        ROLE_TARGETS[role],
    )
    if not source_path.exists():
        return result_base(
            selection_row=selection_row,
            variant=variant,
            source_role=role,
            status="blocked",
            diagnostic_code="local_source_missing",
            failure_reason="catalog metadata references a source artifact that is absent locally",
            local_path=local_path,
            source_catalog_path=source_path.as_posix(),
            media_type=variant.get("media_type")
            if isinstance(variant.get("media_type"), str)
            else None,
        )
    if source_path.stat().st_size == 0:
        return result_base(
            selection_row=selection_row,
            variant=variant,
            source_role=role,
            status="failed",
            diagnostic_code="empty_local_source",
            failure_reason="catalog source artifact is empty",
            local_path=local_path,
            source_catalog_path=source_path.as_posix(),
            media_type=variant.get("media_type")
            if isinstance(variant.get("media_type"), str)
            else None,
        )
    if write:
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_path, target)
    artifact_path = target if write else source_path
    return result_base(
        selection_row=selection_row,
        variant=variant,
        source_role=role,
        status="captured",
        diagnostic_code="captured_local_source_artifact",
        failure_reason=None,
        local_path=local_path,
        source_catalog_path=source_path.as_posix(),
        sha256=sha256_file(artifact_path),
        byte_size=artifact_path.stat().st_size,
        media_type=variant.get("media_type")
        if isinstance(variant.get("media_type"), str)
        else None,
    )


def unresolved_result(selection_row: Mapping[str, Any], output_dir: Path) -> dict[str, Any]:
    role = normalize_strategy_role(selection_row)
    local_path = None
    if role in ROLE_TARGETS:
        _, local_path = confined_source_target(
            output_dir,
            selection_row.get("article_ref")
            if isinstance(selection_row.get("article_ref"), str)
            else None,
            selection_row.get("article_key")
            if isinstance(selection_row.get("article_key"), str)
            else None,
            ROLE_TARGETS[role],
        )
    return result_base(
        selection_row=selection_row,
        variant=None,
        source_role=role,
        status="blocked",
        diagnostic_code="catalog_unresolved",
        failure_reason="selection row has no catalog article record or local source variant yet",
        local_path=local_path,
        source_catalog_path=None,
        media_type="text/html" if role and role.endswith(("html", "page")) else None,
    )


def capture_selection(
    *,
    catalog_root: Path,
    index_path: Path,
    selection_path: Path,
    output_dir: Path,
    write: bool = True,
) -> list[dict[str, Any]]:
    index = load_json(index_path)
    selection = load_json(selection_path)
    selected = selection.get("articles")
    if not isinstance(selected, list):
        raise ValueError("selection articles must be a list")
    by_ref = selected_index_rows(index)
    results: list[dict[str, Any]] = []
    for row in selected:
        if not isinstance(row, dict):
            raise ValueError("selection row must be an object")
        article_ref = row.get("article_ref") if isinstance(row.get("article_ref"), str) else None
        index_row = by_ref.get(article_ref) if article_ref else None
        article_path_value = index_row.get("article_path") if index_row else None
        if not isinstance(article_path_value, str):
            results.append(unresolved_result(row, output_dir))
            continue
        article_path = safe_child_path(catalog_root, article_path_value, code="unsafe_article_path")
        if not article_path.exists():
            results.append(
                result_base(
                    selection_row=row,
                    variant=None,
                    source_role=normalize_strategy_role(row),
                    status="blocked",
                    diagnostic_code="catalog_article_missing",
                    failure_reason="index points at an article record that is absent locally",
                    local_path=None,
                    source_catalog_path=article_path.as_posix(),
                )
            )
            continue
        article = load_json(article_path)
        variants = article.get("source_variants")
        if not isinstance(variants, list) or not variants:
            results.append(
                result_base(
                    selection_row=row,
                    variant=None,
                    source_role=normalize_strategy_role(row),
                    status="blocked",
                    diagnostic_code="no_source_variants",
                    failure_reason="catalog article has no source variants",
                    local_path=None,
                    source_catalog_path=article_path.as_posix(),
                )
            )
            continue
        selected_variants = [
            variant
            for variant in variants
            if isinstance(variant, dict) and variant.get("source_role") in ROLE_TARGETS
        ]
        if not selected_variants:
            results.append(
                result_base(
                    selection_row=row,
                    variant=None,
                    source_role=normalize_strategy_role(row),
                    status="blocked",
                    diagnostic_code="no_supported_source_variants",
                    failure_reason="catalog article has no M029-supported source variants",
                    local_path=None,
                    source_catalog_path=article_path.as_posix(),
                )
            )
            continue
        for variant in selected_variants:
            results.append(
                source_result_for_variant(
                    selection_row=row,
                    article_path=article_path,
                    variant=variant,
                    output_dir=output_dir,
                    write=write,
                )
            )
    return results


def build_summary(
    results: list[dict[str, Any]],
    *,
    selection_path: Path,
    catalog_path: Path,
    index_path: Path,
    output_dir: Path,
    duration_ms: int,
) -> dict[str, Any]:
    counts: dict[str, int] = {"captured": 0, "blocked": 0, "failed": 0}
    by_url: dict[str, dict[str, int]] = defaultdict(
        lambda: {"captured": 0, "blocked": 0, "failed": 0}
    )
    by_role: dict[str, dict[str, int]] = defaultdict(
        lambda: {"captured": 0, "blocked": 0, "failed": 0}
    )
    for result in results:
        status = str(result.get("status"))
        if status in counts:
            counts[status] += 1
            url = result.get("url") if isinstance(result.get("url"), str) else "<missing-url>"
            role = (
                result.get("source_role")
                if isinstance(result.get("source_role"), str)
                else "<missing-role>"
            )
            by_url[url][status] += 1
            by_role[role][status] += 1
    return {
        "schema_version": SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "completed_with_diagnostics"
        if counts["blocked"] or counts["failed"]
        else "captured",
        "article_count": len(
            {(r.get("article_ref"), r.get("article_key"), r.get("url")) for r in results}
        ),
        "variant_count": len(results),
        "counts": counts,
        "per_url_terminal_state_counts": {
            url: dict(value) for url, value in sorted(by_url.items())
        },
        "per_role_terminal_state_counts": {
            role: dict(value) for role, value in sorted(by_role.items())
        },
        "results": results,
        "input_paths": {
            "selection": selection_path.as_posix(),
            "catalog": catalog_path.as_posix(),
            "index": index_path.as_posix(),
        },
        "output_paths": {"source_dir": output_dir.as_posix()},
        "duration_ms": duration_ms,
        "capture_phase_network_allowed": False,
        "replay_phase_network_allowed": False,
        "network_fetch_attempted_count": 0,
        "graph_import_allowed": False,
        "production_ladybugdb_write_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "ladybugdb_written": False,
        "raw_text_embedded": False,
        "raw_binary_embedded": False,
        "raw_payload_embedded_in_metadata": False,
        "fail_closed_safety_flags": dict(FAIL_CLOSED_SAFETY_FLAGS),
        "generated_at": utc_now(),
    }


def render_report(summary: Mapping[str, Any]) -> str:
    counts = summary.get("counts") if isinstance(summary.get("counts"), dict) else {}
    lines = [
        "# M029 Source Acquisition Report",
        "",
        "This report is metadata-only and local-only. It does not embed article text, HTML snippets, PDF bytes, or base64 payloads.",
        "",
        f"- Milestone: `{summary.get('milestone_id')}`",
        f"- Slice: `{summary.get('slice_id')}`",
        f"- Selection: `{summary.get('selection_id')}`",
        f"- Status: `{summary.get('status')}`",
        f"- Captured: {counts.get('captured', 0)}",  # pyrefly: ignore[bad-assignment]
        f"- Blocked: {counts.get('blocked', 0)}",  # pyrefly: ignore[bad-assignment]
        f"- Failed: {counts.get('failed', 0)}",  # pyrefly: ignore[bad-assignment]
        "- Capture phase network allowed: false",
        "- Network fetch attempted count: 0",
        "- Graph/import/LadybugDB writes: false",
        "",
        "## Role Counts",
        "",
    ]
    role_counts = (
        summary.get("per_role_terminal_state_counts")
        if isinstance(summary.get("per_role_terminal_state_counts"), dict)
        else {}
    )
    for role, value in role_counts.items():
        if isinstance(value, dict):
            lines.append(
                f"- `{role}`: captured={value.get('captured', 0)} blocked={value.get('blocked', 0)} failed={value.get('failed', 0)}"
            )
    lines.extend(["", "## URLs", ""])
    url_counts = (
        summary.get("per_url_terminal_state_counts")
        if isinstance(summary.get("per_url_terminal_state_counts"), dict)
        else {}
    )
    for url, value in url_counts.items():
        if isinstance(value, dict):
            lines.append(
                f"- `{url}`: captured={value.get('captured', 0)} blocked={value.get('blocked', 0)} failed={value.get('failed', 0)}"
            )
    lines.extend(["", "## Results", ""])
    for result in summary.get("results", []):
        if isinstance(result, dict):
            lines.append(
                f"- `{result.get('url')}` `{result.get('source_role')}`: {result.get('status')} "
                f"({result.get('diagnostic_code')}) -> `{result.get('local_path')}`"
            )
    return "\n".join(lines) + "\n"


def assert_metadata_artifact_is_redacted(path: Path) -> None:
    payload = path.read_text(encoding="utf-8")
    found = [token for token in FORBIDDEN_SNIPPETS if token in payload]
    if found:
        raise ValueError(f"metadata artifact is not redacted: {path}: {found}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--index", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv[1:])

    started = time.perf_counter()
    for path in (args.selection, args.catalog, args.index, args.output_dir):
        if not path.is_absolute() and ".." in PurePosixPath(str(path).replace("\\", "/")).parts:
            raise ValueError(f"unsafe CLI path: {path}")

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    results = capture_selection(
        catalog_root=args.catalog.parent,
        index_path=args.index,
        selection_path=args.selection,
        output_dir=output_dir,
        write=not args.dry_run,
    )
    summary = build_summary(
        results,
        selection_path=args.selection,
        catalog_path=args.catalog,
        index_path=args.index,
        output_dir=args.output_dir,
        duration_ms=int((time.perf_counter() - started) * 1000),
    )
    corpus_dir = output_dir.parent
    summary_path = corpus_dir / "source-acquisition-summary.json"
    diagnostics_path = corpus_dir / "source-acquisition-diagnostics.jsonl"
    report_path = corpus_dir / "source-acquisition-report.md"
    write_json(summary_path, summary)
    write_jsonl(diagnostics_path, results)
    atomic_write_text(report_path, render_report(summary))
    for artifact_path in (summary_path, diagnostics_path, report_path):
        assert_metadata_artifact_is_redacted(artifact_path)
    print(
        json.dumps(
            {
                "summary_path": summary_path.as_posix(),
                "variant_count": len(results),
                "counts": summary["counts"],
            },
            sort_keys=True,
        )
    )
    return 0 if summary["counts"]["failed"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
