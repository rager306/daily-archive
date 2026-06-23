#!/usr/bin/env python3
"""DEPRECATED legacy delegate for canonical catalog ingestion.

This script is preserved for the M045 trajectory check audit trail and
downstream callers that may reference it. New code should use
``scripts/ingest_to_canonical_catalog.py`` (the canonical CLI entry point)
or import directly from
``research_graph.infrastructure.corpus.ingestion.catalog_ingest``.

Migration history:
- 2026-06-13 (M061 S04): original 688-line implementation
- 2026-06-23 (M120 S05): replaced with ~80-line legacy delegate pointing to
  ``research_graph.infrastructure.corpus.ingestion.catalog_ingest``

The original logic (HTTP fetch with retry/backoff, build_article_record,
write_article_record, ingest_catalog orchestration, render_report) lives in
``src/research_graph/infrastructure/corpus/ingestion/catalog_ingest.py``.

Removed in this delegation:
- Inline arxiv API fetcher → moved to catalog_ingest::fetch_arxiv_metadata
- Inline RequestPacer → moved to catalog_ingest::RequestPacer
- Inline build/write_article_record → moved to catalog_ingest.{build,write}_article_record
- Inline load_selected_ids/load_pdf_paths → moved to catalog_ingest module
- Inline update_index_if_exists → moved to catalog_ingest module
- Inline render_report → moved to catalog_ingest module

Kept (intentionally):
- Path: ``scripts/m061_ingest_to_canonical_catalog.py`` (for trajectory check
  reference in ``scripts/check_project_trajectory.py``)
- ``--no-index`` CLI flag (compatible with new entry point)

Safety posture: fail-closed by default. ``SafetyOverride.external_network_authorized``
defaults to False in the new module unless explicitly set (legacy M061 default
permitted external network; preserved via ``SAFETY_OVERRIDE_M061_INGEST``).
"""

from __future__ import annotations

import argparse
import sys
import warnings
from pathlib import Path

# Emit DeprecationWarning at import time
warnings.warn(
    "scripts/m061_ingest_to_canonical_catalog.py is deprecated; "
    "use scripts/ingest_to_canonical_catalog.py instead. "
    "The original M061 logic now lives in "
    "research_graph.infrastructure.corpus.ingestion.catalog_ingest.",
    DeprecationWarning,
    stacklevel=2,
)

# Delegate to the canonical CLI entry point.
_CLI_PATH = Path(__file__).resolve().parent / "ingest_to_canonical_catalog.py"


def parse_args(argv: list[str]) -> argparse.Namespace:
    """Legacy argparse-compatible args parser."""
    parser = argparse.ArgumentParser(
        description="[DEPRECATED] Canonical catalog ingest (use scripts/ingest_to_canonical_catalog.py).",
    )
    parser.add_argument(
        "--no-index",
        action="store_true",
        help="Do not update index.json after ingestion",
    )
    parser.add_argument(
        "--m061-root",
        type=Path,
        default=None,
        help="M061 2-hop artifacts root (default: artifacts/m061-2hop)",
    )
    parser.add_argument(
        "--catalog-root",
        type=Path,
        default=None,
        help="Canonical article catalog root (default: data/article_catalog)",
    )
    parser.add_argument(
        "--report-path",
        type=Path,
        default=None,
        help="Markdown report output path",
    )
    parser.add_argument(
        "--no-network",
        action="store_true",
        help="Disable external network (arxiv API) calls",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)

    # Forward all flags to new CLI via subprocess to preserve original behavior.
    forward_args: list[str] = []
    if args.no_index:
        forward_args.append("--no-index")
    if args.no_network:
        forward_args.append("--no-network")
    if args.m061_root is not None:
        forward_args.extend(["--m061-root", str(args.m061_root)])
    if args.catalog_root is not None:
        forward_args.extend(["--catalog-root", str(args.catalog_root)])
    if args.report_path is not None:
        forward_args.extend(["--report-path", str(args.report_path)])

    import subprocess

    print(
        "[DEPRECATED] scripts/m061_ingest_to_canonical_catalog.py delegates to "
        "scripts/ingest_to_canonical_catalog.py",
        file=sys.stderr,
    )
    result = subprocess.run(  # noqa: S603
        [sys.executable, str(_CLI_PATH), *forward_args],
        check=False,
    )
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
