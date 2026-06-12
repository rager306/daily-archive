#!/usr/bin/env python3
"""Replay deterministic parser ingest artifacts from a PDF batch manifest.

M059 replay is deliberately local and artifact-level: safety defaults stay false,
so this tool does not call external parser services, write graph data, promote
facts, or run LLM calls. For deterministic parsers it materializes a replay copy
and verifies byte-identical SHA-256 output. Non-deterministic parsers are reported
without byte-identity claims.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Iterable

from m059_validate_pdf_batch import (
    DEFAULT_LOOPBACK_BASE_URL,
    SAFETY_DEFAULTS,
    ensure_safety_defaults,
    find_parser_expectation,
    read_json,
    rel,
    repo_path,
    resolve_output_paths,
    validate_manifest_contract,
)

DETERMINISTIC_PARSERS = {"grobid", "opendataloader", "plotextractor"}
NON_DETERMINISTIC_MARKERS = {"llm", "minimax", "openai", "anthropic", "claude", "gpt"}


@dataclass(frozen=True)
class ReplayResult:
    """Replay result for one PDF/parser pair."""

    arxiv_id: str
    parser: str
    status: str
    deterministic: bool
    source_path: str | None
    replay_path: str | None
    source_sha256: str | None
    replay_sha256: str | None
    byte_identical: bool | None
    message: str


@dataclass(frozen=True)
class ReplayReport:
    """Aggregate replay report for one manifest/parser pair."""

    manifest: str
    batch_id: str
    parser: str
    output_suffix: str
    total: int
    replayed: int
    skipped: int
    failed: int
    non_deterministic: int
    byte_identical: int
    safety_defaults: dict[str, bool]
    results: list[ReplayResult]

    def to_jsonable(self) -> dict[str, Any]:
        payload = asdict(self)
        payload["results"] = [asdict(result) for result in self.results]
        return payload


def sha256_file(path: Path) -> str:
    """Return the SHA-256 digest of a file."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def is_deterministic_parser(parser_name: str) -> bool:
    """Return whether the parser is expected to be byte-deterministic."""
    lowered = parser_name.casefold()
    if lowered in DETERMINISTIC_PARSERS:
        return True
    return not any(marker in lowered for marker in NON_DETERMINISTIC_MARKERS)


def replay_target_path(source_path: Path, parser_name: str, output_suffix: str, output_dir: Path | None) -> Path:
    """Build the target path for a replay artifact."""
    filename = f"{source_path.stem}.{output_suffix}{source_path.suffix}"
    if output_dir is None:
        return source_path.with_name(filename)
    return output_dir / parser_name / filename


def replay_pdf(
    pdf: dict[str, Any],
    parser_name: str,
    *,
    output_suffix: str,
    output_dir: Path | None = None,
) -> ReplayResult:
    """Replay one PDF/parser output and verify byte identity where applicable."""
    arxiv_id = str(pdf["arxiv_id"])
    expectation = find_parser_expectation(pdf, parser_name)
    if expectation is None:
        return ReplayResult(arxiv_id, parser_name, "failed", True, None, None, None, None, None, f"parser {parser_name!r} is not declared for {arxiv_id}")

    source_paths = resolve_output_paths(pdf, expectation)
    if not source_paths:
        return ReplayResult(arxiv_id, parser_name, "failed", True, None, None, None, None, None, "parser output path could not be resolved")

    source_path = source_paths[0]
    if not source_path.exists():
        return ReplayResult(arxiv_id, parser_name, "failed", True, rel(source_path), None, None, None, None, "source parser output is missing")

    deterministic = is_deterministic_parser(parser_name)
    source_sha = sha256_file(source_path)
    if not deterministic:
        return ReplayResult(
            arxiv_id,
            parser_name,
            "non_deterministic",
            False,
            rel(source_path),
            None,
            source_sha,
            None,
            None,
            "parser output is treated as non-deterministic; byte-identity replay is not asserted",
        )

    target_path = replay_target_path(source_path, parser_name, output_suffix, output_dir)
    target_path.parent.mkdir(parents=True, exist_ok=True)

    if target_path.exists():
        replay_sha = sha256_file(target_path)
        if replay_sha == source_sha:
            return ReplayResult(
                arxiv_id,
                parser_name,
                "skipped",
                True,
                rel(source_path),
                rel(target_path),
                source_sha,
                replay_sha,
                True,
                "replay artifact already exists with matching sha256",
            )
        return ReplayResult(
            arxiv_id,
            parser_name,
            "failed",
            True,
            rel(source_path),
            rel(target_path),
            source_sha,
            replay_sha,
            False,
            "existing replay artifact does not match source sha256",
        )

    shutil.copyfile(source_path, target_path)
    replay_sha = sha256_file(target_path)
    byte_identical = replay_sha == source_sha
    return ReplayResult(
        arxiv_id,
        parser_name,
        "replayed" if byte_identical else "failed",
        True,
        rel(source_path),
        rel(target_path),
        source_sha,
        replay_sha,
        byte_identical,
        "byte-identical replay materialized" if byte_identical else "replay sha256 mismatch",
    )


def replay_batch(
    manifest_path: str | Path,
    parser_name: str,
    *,
    output_suffix: str = "replay",
    output_dir: str | Path | None = None,
    arxiv_ids: set[str] | None = None,
) -> ReplayReport:
    """Replay one parser across a manifest and return an aggregate report."""
    if not output_suffix or any(part in output_suffix for part in ("/", "\\")):
        raise ValueError("output_suffix must be a non-empty filename suffix, not a path")

    manifest_actual = repo_path(manifest_path)
    manifest = read_json(manifest_actual)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be an object")
    validate_manifest_contract(manifest)
    ensure_safety_defaults(manifest.get("safety_defaults"), context="manifest")

    output_dir_path = repo_path(output_dir) if output_dir is not None else None
    pdfs = manifest.get("pdfs", [])
    if arxiv_ids is not None:
        pdfs = [pdf for pdf in pdfs if str(pdf.get("arxiv_id")) in arxiv_ids]

    results = [
        replay_pdf(pdf, parser_name, output_suffix=output_suffix, output_dir=output_dir_path)
        for pdf in pdfs
    ]
    replayed = sum(1 for result in results if result.status == "replayed")
    skipped = sum(1 for result in results if result.status == "skipped")
    failed = sum(1 for result in results if result.status == "failed")
    non_deterministic = sum(1 for result in results if result.status == "non_deterministic")
    byte_identical = sum(1 for result in results if result.byte_identical is True)

    return ReplayReport(
        manifest=rel(manifest_actual),
        batch_id=str(manifest.get("batch_id", "")),
        parser=parser_name,
        output_suffix=output_suffix,
        total=len(results),
        replayed=replayed,
        skipped=skipped,
        failed=failed,
        non_deterministic=non_deterministic,
        byte_identical=byte_identical,
        safety_defaults=SAFETY_DEFAULTS.copy(),
        results=results,
    )


def print_report(report: ReplayReport, *, json_output: bool = False) -> None:
    """Print a replay report to stdout."""
    if json_output:
        print(json.dumps(report.to_jsonable(), indent=2, sort_keys=True))
        return

    for result in report.results:
        identical = "-" if result.byte_identical is None else str(result.byte_identical).lower()
        print(
            f"{result.status.upper()} {result.arxiv_id} parser={result.parser} "
            f"source={result.source_path or '<unresolved>'} replay={result.replay_path or '<none>'} "
            f"byte_identical={identical} message={result.message}"
        )
    print(
        "aggregate "
        f"batch={report.batch_id} parser={report.parser} total={report.total} "
        f"replayed={report.replayed} skipped={report.skipped} failed={report.failed} "
        f"non_deterministic={report.non_deterministic} byte_identical={report.byte_identical}"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, help="Repository-relative manifest JSON path.")
    parser.add_argument("--parser", required=True, help="Parser name declared in expected_parsers[].")
    parser.add_argument("--output-suffix", default="replay", help="Suffix for replay artifacts, default: replay.")
    parser.add_argument("--output-dir", help="Optional repository-relative directory for replay artifacts.")
    parser.add_argument("--arxiv-id", action="append", dest="arxiv_ids", help="Limit replay to one arXiv ID; may be repeated.")
    parser.add_argument("--json", action="store_true", help="Emit the full replay report as JSON.")
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(list(argv) if argv is not None else None)
    try:
        report = replay_batch(
            args.manifest,
            args.parser,
            output_suffix=args.output_suffix,
            output_dir=args.output_dir,
            arxiv_ids=set(args.arxiv_ids) if args.arxiv_ids else None,
        )
    except Exception as exc:  # pragma: no cover - CLI defensive boundary
        print(f"ERROR replay setup failed: {exc}", file=sys.stderr)
        return 2

    print_report(report, json_output=args.json)
    return 0 if report.failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
