#!/usr/bin/env python3
"""Build R024 coverage report through the corpus coverage use case."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from research_graph.application.corpus.coverage import (
    CatalogCoverageInput,
    CorpusCoverageRequest,
    CorpusCoverageUseCase,
    CoverageSourceArtifact,
    GraphProbeCoverageInput,
    ParserCoverageInput,
)
from research_graph.infrastructure.corpus.reporting.coverage_report import (
    FilesystemCoverageReportWriter,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CORPUS_DIR = REPO_ROOT / "data" / "r024-218-document-corpus-v1"
DEFAULT_REPORT = DEFAULT_CORPUS_DIR / "R024-COVERAGE.md"
DEFAULT_SUMMARY = DEFAULT_CORPUS_DIR / "coverage-summary.json"
DEFAULT_INGEST_SUMMARY = DEFAULT_CORPUS_DIR / "ingest-summary.json"
DEFAULT_PARSER_SUMMARY = DEFAULT_CORPUS_DIR / "parser-chunking" / "summary.json"
DEFAULT_NETWORKX_SUMMARY = DEFAULT_CORPUS_DIR / "networkx-probe" / "summary.json"
DEFAULT_MEMORY_PROFILE = DEFAULT_CORPUS_DIR / "networkx-probe" / "memory-profile.json"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-id", default="r024-218-document-corpus-v1")
    parser.add_argument("--corpus-dir", type=Path, default=DEFAULT_CORPUS_DIR)
    parser.add_argument("--ingest-summary", type=Path, default=DEFAULT_INGEST_SUMMARY)
    parser.add_argument("--parser-summary", type=Path, default=DEFAULT_PARSER_SUMMARY)
    parser.add_argument("--networkx-summary", type=Path, default=DEFAULT_NETWORKX_SUMMARY)
    parser.add_argument("--memory-profile", type=Path, default=DEFAULT_MEMORY_PROFILE)
    parser.add_argument("--report-path", type=Path, default=DEFAULT_REPORT)
    parser.add_argument("--summary-path", type=Path, default=DEFAULT_SUMMARY)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    request = build_request(args)
    result = CorpusCoverageUseCase().run(request)
    emitted = FilesystemCoverageReportWriter(
        markdown_path=args.report_path,
        json_path=args.summary_path,
        milestone="M122 Pipeline Script Architecture Migration",
    ).write(result)

    print(f"report={emitted.markdown_path}")
    print(f"summary={emitted.json_path}")
    print(f"schema_version={emitted.schema_version}")
    print(f"catalog_records={result.catalog_records}")
    print(f"source_backed_records={result.source_backed_records}")
    print(f"metadata_only_records={result.metadata_only_records}")
    print(f"parser_errors={result.parser_errors}")
    print(f"failed_denominator_checks={len([d for d in result.denominators if d.errors])}")
    return 0 if result.succeeded else 1


def build_request(args: argparse.Namespace) -> CorpusCoverageRequest:
    ingest = _load_json(args.ingest_summary)
    parser = _load_json(args.parser_summary)
    networkx = _load_json(args.networkx_summary) if args.networkx_summary.exists() else None
    memory = _load_json(args.memory_profile) if args.memory_profile.exists() else None
    return CorpusCoverageRequest(
        corpus_id=str(args.corpus_id),
        catalog=CatalogCoverageInput(
            total_records=int(ingest.get("total_records", 0)),
            index_entries=_optional_int(ingest.get("index_entries")),
            ingested_count=int(ingest.get("ingested_count", 0)),
            skipped_count=int(ingest.get("skipped_count", 0)),
            failed_count=int(ingest.get("failed_count", 0)),
            source_artifact=CoverageSourceArtifact(
                path=_display_path(args.ingest_summary),
                artifact_type="json-summary",
                schema_version=str(ingest.get("schema_version")) if ingest.get("schema_version") else None,
            ),
        ),
        parser=ParserCoverageInput(
            total=int(parser.get("total", 0)),
            completed=int(parser.get("ok", 0)),
            skipped=int(parser.get("skipped", 0)),
            errors=int(parser.get("errors", 0)),
            chunk_count_total=int(parser.get("chunk_count_total", 0)),
            source_kind_counts=_int_mapping(parser.get("source_kind_counts", {})),
            skip_reason_counts=_int_mapping(parser.get("skip_reason_counts", {})),
            skipped_article_refs=_skipped_article_refs(args.parser_summary.parent / "events.jsonl"),
            source_artifact=CoverageSourceArtifact(
                path=_display_path(args.parser_summary),
                artifact_type="json-summary",
                schema_version=str(parser.get("schema_version")) if parser.get("schema_version") else None,
            ),
        ),
        graph_probe=_graph_input(networkx, memory, args.networkx_summary) if networkx else None,
    )


def _graph_input(
    networkx: dict[str, Any],
    memory: dict[str, Any] | None,
    path: Path,
) -> GraphProbeCoverageInput:
    return GraphProbeCoverageInput(
        corpus_size=int(networkx.get("corpus_size", 0)),
        skipped_metadata_only=int(networkx.get("skipped_metadata_only", 0)),
        chunk_count_total=int(networkx.get("chunk_count_total", 0)),
        n_nodes=int(networkx.get("n_nodes", 0)),
        n_edges=int(networkx.get("n_edges", 0)),
        citation_relations_count=int(networkx.get("citation_relations_count", 0)),
        peak_memory_mb=(float(memory["peak_mb"]) if memory and "peak_mb" in memory else None),
        source_artifact=CoverageSourceArtifact(
            path=_display_path(path),
            artifact_type="json-summary",
            schema_version=str(networkx.get("schema_version")) if networkx.get("schema_version") else None,
        ),
    )


def _load_json(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return data


def _skipped_article_refs(events_path: Path) -> tuple[str, ...]:
    if not events_path.exists():
        return ()
    refs: list[str] = []
    for line in events_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        event = json.loads(line)
        if event.get("event") == "parser_chunking_skipped_metadata_only":
            refs.append(str(event.get("article_ref", "")))
    return tuple(ref for ref in refs if ref)


def _optional_int(value: object) -> int | None:
    if value is None:
        return None
    if isinstance(value, str | int | float):
        return int(value)
    raise TypeError(f"expected int-compatible value, got {type(value).__name__}")


def _int_mapping(value: object) -> dict[str, int]:
    if not isinstance(value, dict):
        return {}
    result: dict[str, int] = {}
    for key, item in value.items():
        if not isinstance(item, str | int | float):
            raise TypeError(f"expected int-compatible mapping value, got {type(item).__name__}")
        result[str(key)] = int(item)
    return result


def _display_path(path: Path) -> str:
    try:
        return path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return path.as_posix()


if __name__ == "__main__":
    sys.exit(main())
