#!/usr/bin/env python3
"""Validate the M027 mixed-source corpus and emit local-only handoff artifacts.

This wrapper is intentionally fixed to the registered M027 corpus paths. It does
not fetch network sources, acquire article payloads, convert documents, claim
parser/chunk readiness, import trusted facts, or write LadybugDB/production
graph state. It delegates catalog invariants to the shared article catalog
verifier, then annotates the resulting artifacts with M027 provenance and
fail-closed handoff context for downstream executors.
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path
from typing import Any

from verify_m025_article_catalog import (
    diagnostic,
    parse_args,
    validate,
    write_json_atomic,
    write_jsonl_atomic,
)

MILESTONE_ID = "M027-aakeky"
SLICE_ID = "S01"
SELECTION_ID = "m027-mixed-source-corpus-v1"
REPORT_TITLE = "M027 Mixed Source Catalog Validation Report"
ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = ROOT / "data" / "article_catalog" / "catalog.json"
INDEX_PATH = ROOT / "data" / "article_catalog" / "index.json"
CORPUS_DIR = ROOT / "data" / "article_corpora" / SELECTION_ID
SELECTION_PATH = CORPUS_DIR / "selection.json"
SUMMARY_PATH = CORPUS_DIR / "catalog-summary.json"
DIAGNOSTICS_PATH = CORPUS_DIR / "catalog-diagnostics.jsonl"
REPORT_PATH = CORPUS_DIR / "catalog-report.md"

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

OUT_OF_SCOPE: dict[str, bool] = {
    "captured_sources_claimed": False,
    "conversion_claimed": False,
    "parser_readiness_claimed": False,
    "chunks_claimed": False,
    "production_import_claimed": False,
    "trusted_facts_claimed": False,
    "ladybugdb_write_claimed": False,
}

COMMAND = [
    "uv",
    "run",
    "python",
    "scripts/verify_m027_mixed_source_catalog.py",
]

VALIDATOR_ARGS = [
    "--catalog",
    str(CATALOG_PATH),
    "--index",
    str(INDEX_PATH),
    "--selection",
    str(SELECTION_PATH),
    "--validate-only",
    "--require-index",
    "--check-index-idempotent",
    "--check-index-titles",
    "--check-safe-traversal",
    "--check-duplicate-lookups",
    "--check-index-lookup-only",
    "--require-selection-titles",
    "--allow-index-superset",
    "--expected-selection-id",
    SELECTION_ID,
    "--write-summary",
    str(SUMMARY_PATH),
    "--write-diagnostics",
    str(DIAGNOSTICS_PATH),
    "--write-report",
    str(REPORT_PATH),
    "--report-title",
    REPORT_TITLE,
]


def _rel(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return str(path)


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            rows.append(json.loads(line))
    return rows


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _hash_or_diagnostic(path: Path, diagnostics: list[dict[str, Any]]) -> str | None:
    try:
        return _sha256(path)
    except OSError as exc:
        diagnostics.append(
            diagnostic(
                "artifact_hash",
                f"failed to hash artifact {path}: {exc}",
                path=_rel(path),
                json_path="$.provenance.output_hashes",
                failing_invariant="output artifact must be hashable after validation",
                network_fetch_attempted=False,
                fail_closed_safety_flags=FAIL_CLOSED_SAFETY_FLAGS,
            )
        )
        return None


def _git_commit() -> str | None:
    """Resolve the current commit by reading .git metadata without running git."""
    git_dir = ROOT / ".git"
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
            packed_refs = git_dir / "packed-refs"
            try:
                for line in packed_refs.read_text(encoding="utf-8").splitlines():
                    if line and not line.startswith("#") and line.endswith(f" {ref}"):
                        return line.split(" ", 1)[0]
            except OSError:
                return None
            return None
    return head or None


def _input_paths(selection: dict[str, Any]) -> list[Path]:
    paths = [CATALOG_PATH, INDEX_PATH, SELECTION_PATH]
    for row in selection.get("articles", []):
        if not isinstance(row, dict):
            continue
        article_path = row.get("article_path")
        if isinstance(article_path, str) and article_path:
            paths.append(INDEX_PATH.parent / article_path)
    return paths


def _build_provenance(
    selection: dict[str, Any], diagnostics: list[dict[str, Any]], *, exit_code: int
) -> dict[str, Any]:
    input_hashes = {
        _rel(path): _hash_or_diagnostic(path, diagnostics) for path in _input_paths(selection)
    }
    output_hashes = {
        _rel(DIAGNOSTICS_PATH): _hash_or_diagnostic(DIAGNOSTICS_PATH, diagnostics),
        _rel(REPORT_PATH): _hash_or_diagnostic(REPORT_PATH, diagnostics),
    }
    return {
        "command": COMMAND,
        "validator_args": VALIDATOR_ARGS,
        "inputs": [_rel(path) for path in _input_paths(selection)],
        "input_hashes": input_hashes,
        "output_hashes": output_hashes,
        "summary_self_hash": None,
        "summary_self_hash_reason": "omitted because embedding the summary hash would make the JSON self-referential",
        "exit_code": exit_code,
        "cwd": str(ROOT),
        "git_commit": _git_commit(),
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "network_fetch_attempted": False,
        "validate_only": True,
        "safety_flags": FAIL_CLOSED_SAFETY_FLAGS,
        "out_of_scope": OUT_OF_SCOPE,
    }


def _article_lookup(selection: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {
        row["article_ref"]: row
        for row in selection.get("articles", [])
        if isinstance(row, dict) and isinstance(row.get("article_ref"), str)
    }


def _enrich_diagnostics(
    rows: list[dict[str, Any]], selection: dict[str, Any]
) -> list[dict[str, Any]]:
    by_ref = _article_lookup(selection)
    enriched: list[dict[str, Any]] = []
    for row in rows:
        item = dict(row)
        article_ref = item.get("article_ref") if isinstance(item.get("article_ref"), str) else None
        selection_row = by_ref.get(article_ref or "", {})
        item.setdefault("selection_id", SELECTION_ID)
        item.setdefault("milestone_id", MILESTONE_ID)
        item.setdefault("slice_id", SLICE_ID)
        item.setdefault("network_fetch_attempted", False)
        item.setdefault("fail_closed_safety_flags", FAIL_CLOSED_SAFETY_FLAGS)
        item.setdefault(
            "lookup_key",
            selection_row.get("article_key") or selection_row.get("canonical_url") or article_ref,
        )
        item.setdefault("file_path", selection_row.get("article_path") or _rel(SELECTION_PATH))
        item.setdefault("json_path", "$.articles" if article_ref else "$")
        item.setdefault("failing_invariant", item.get("message"))
        enriched.append(item)
    return enriched


def _failure_diagnostics(errors: list[str]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for position, error in enumerate(errors):
        rows.append(
            diagnostic(
                "catalog_validation_failed",
                error,
                path=_rel(SELECTION_PATH),
                json_path=f"$.validation_errors[{position}]",
                failing_invariant=error,
                selection_id=SELECTION_ID,
                milestone_id=MILESTONE_ID,
                slice_id=SLICE_ID,
                lookup_key=None,
                file_path=_rel(SELECTION_PATH),
                network_fetch_attempted=False,
                fail_closed_safety_flags=FAIL_CLOSED_SAFETY_FLAGS,
            )
        )
    return rows


def _append_report_sections(report: str, summary: dict[str, Any]) -> str:
    report = report.replace(
        "Expected load is five selected articles", "Expected load is six selected articles"
    )
    report = report.replace("`run-summary.json` records", "`catalog-summary.json` records")
    report = report.replace("`diagnostics.jsonl` records", "`catalog-diagnostics.jsonl` records")
    articles = summary.get("articles", []) if isinstance(summary.get("articles"), list) else []
    lines = [report.rstrip(), "", "## M027 Local-Only Handoff", ""]
    lines.extend(
        [
            f"- Milestone: `{MILESTONE_ID}`",
            f"- Slice: `{SLICE_ID}`",
            f"- Selection: `{SELECTION_ID}`",
            "- Validate-only network_fetch_attempted=false; no network acquisition or refresh is performed.",
            "- Fail-closed safety flags keep graph import, trusted fact promotion, production import, and LadybugDB writes disabled.",
            "- Out of scope: captured sources, conversion, parser readiness, chunks, production imports, trusted facts, and LadybugDB writes.",
            "",
            "## Seed URL Mapping",
            "| Seed URL | Article Ref | Title |",
            "|---|---|---|",
        ]
    )
    for article in articles:
        lines.append(
            f"| {article.get('seed_url')} | `{article.get('article_ref')}` | {article.get('title')} |"
        )
    lines.extend(
        [
            "",
            "## Provenance",
            f"- Command: `{' '.join(COMMAND)}`",
            f"- CWD: `{summary.get('provenance', {}).get('cwd')}`",
            f"- Git commit: `{summary.get('provenance', {}).get('git_commit')}`",
            f"- Exit code: {summary.get('provenance', {}).get('exit_code')}",
            "- Output hashes are recorded in `catalog-summary.json` for non-self-referential outputs.",
            "",
        ]
    )
    return "\n".join(lines)


def _enrich_summary(
    summary: dict[str, Any],
    selection: dict[str, Any],
    diagnostics: list[dict[str, Any]],
    *,
    exit_code: int,
) -> dict[str, Any]:
    by_ref = _article_lookup(selection)
    for article in summary.get("articles", []):
        if not isinstance(article, dict):
            continue
        selection_row = by_ref.get(str(article.get("article_ref")), {})
        article["seed_url"] = selection_row.get("seed_url")
        article["canonical_url"] = selection_row.get("canonical_url")
        article["selection_article_path"] = selection_row.get("article_path")
        article["network_fetch_attempted"] = False
    summary["milestone_id"] = MILESTONE_ID
    summary["slice_id"] = SLICE_ID
    summary["local_only_validation"] = {
        "validate_only": True,
        "network_fetch_attempted": False,
        "index_lookup_only": True,
        "allow_index_superset": True,
    }
    summary["out_of_scope"] = OUT_OF_SCOPE
    summary["safety_flags"] = FAIL_CLOSED_SAFETY_FLAGS
    summary["provenance"] = _build_provenance(selection, diagnostics, exit_code=exit_code)
    return summary


def _has_artifact_hash_error(diagnostics: list[dict[str, Any]]) -> bool:
    return any(
        row.get("code") == "artifact_hash" and row.get("severity", "error") == "error"
        for row in diagnostics
    )


def main() -> int:
    os.chdir(ROOT)
    args = parse_args(
        VALIDATOR_ARGS,
        default_expected_selection_id=SELECTION_ID,
        default_report_title=REPORT_TITLE,
    )
    errors, _report = validate(args)
    selection: dict[str, Any] = {}
    try:
        selection = _load_json(SELECTION_PATH)
    except (OSError, json.JSONDecodeError):
        selection = {"articles": [], "selection_id": SELECTION_ID}

    if errors:
        rows = _failure_diagnostics(errors)
        rows = _enrich_diagnostics(rows, selection)
        write_jsonl_atomic(DIAGNOSTICS_PATH, rows)
        sys.stderr.write("M027 mixed-source catalog validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1

    diagnostics = _enrich_diagnostics(_load_jsonl(DIAGNOSTICS_PATH), selection)
    summary = _enrich_summary(_load_json(SUMMARY_PATH), selection, diagnostics, exit_code=0)
    report = _append_report_sections(REPORT_PATH.read_text(encoding="utf-8"), summary)
    REPORT_PATH.write_text(report, encoding="utf-8")
    diagnostics.append(
        diagnostic(
            "m027_local_only_handoff",
            "M027 catalog validation artifacts emitted for local-only handoff",
            severity="info",
            selection_id=SELECTION_ID,
            milestone_id=MILESTONE_ID,
            slice_id=SLICE_ID,
            json_path="$",
            lookup_key=None,
            file_path=_rel(SUMMARY_PATH),
            failing_invariant="local-only handoff artifacts must not claim acquisition, parser readiness, graph import, or LadybugDB writes",
            network_fetch_attempted=False,
            fail_closed_safety_flags=FAIL_CLOSED_SAFETY_FLAGS,
            out_of_scope=OUT_OF_SCOPE,
        )
    )
    summary["provenance"]["output_hashes"][_rel(REPORT_PATH)] = _hash_or_diagnostic(
        REPORT_PATH, diagnostics
    )
    write_jsonl_atomic(DIAGNOSTICS_PATH, diagnostics)
    summary["provenance"]["output_hashes"][_rel(DIAGNOSTICS_PATH)] = _hash_or_diagnostic(
        DIAGNOSTICS_PATH, diagnostics
    )
    if _has_artifact_hash_error(diagnostics):
        write_jsonl_atomic(DIAGNOSTICS_PATH, diagnostics)
        sys.stderr.write(
            "M027 mixed-source catalog validation failed: unable to hash one or more output artifacts.\n"
        )
        return 1
    write_json_atomic(SUMMARY_PATH, summary)
    sys.stdout.write(
        "M027 mixed-source catalog validation passed: six-row local-only selection, "
        "index-only lookup, title checks, safe traversal, duplicate lookup checks, "
        "idempotency evidence, and fail-closed safety flags are consistent.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
