#!/usr/bin/env python3
"""Run the M059 S02 end-to-end validation and replay proof on the M054 batch."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Iterable
from pathlib import Path
from typing import Any

from m059_replay_ingest import replay_batch
from m059_validate_pdf_batch import read_json, rel, validate_batch

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "artifacts" / "m054-pdf-acquisition" / "manifest.json"
DEFAULT_OUTPUT_DIR = ROOT / "artifacts" / "m059-architecture"
PARSERS = ("grobid", "opendataloader")


def write_json(path: Path, payload: dict[str, Any]) -> None:
    """Write deterministic, human-readable JSON."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def display_path(path: Path) -> str:
    """Return a repo-relative path when the output lives under the repository."""
    return rel(path) if path.resolve().is_relative_to(ROOT) else path.as_posix()


def first_manifest_arxiv_id(manifest_path: Path) -> str:
    """Return the first arXiv ID in the manifest."""
    manifest = read_json(manifest_path)
    pdfs = manifest.get("pdfs", [])
    if not pdfs:
        raise ValueError(f"manifest has no pdfs: {manifest_path}")
    return str(pdfs[0]["arxiv_id"])


def run_e2e(
    manifest_path: Path = DEFAULT_MANIFEST, output_dir: Path = DEFAULT_OUTPUT_DIR
) -> dict[str, Any]:
    """Run validation for two parsers and replay GROBID for one PDF."""
    output_dir.mkdir(parents=True, exist_ok=True)

    validation_reports = {parser: validate_batch(manifest_path, parser) for parser in PARSERS}
    validation_payload = {
        parser: report.to_jsonable() for parser, report in validation_reports.items()
    }
    validation_report_path = output_dir / "m054-validation-report.json"
    write_json(validation_report_path, validation_payload)

    replay_arxiv_id = first_manifest_arxiv_id(manifest_path)
    replay_report = replay_batch(
        manifest_path,
        "grobid",
        output_suffix="e2e-replay",
        output_dir=output_dir / "replay",
        arxiv_ids={replay_arxiv_id},
    )
    replay_report_path = output_dir / "m054-grobid-replay-report.json"
    write_json(replay_report_path, replay_report.to_jsonable())

    validation_passed = all(report.failed == 0 for report in validation_reports.values())
    replay_passed = replay_report.failed == 0 and replay_report.byte_identical >= 1
    summary = {
        "manifest": display_path(manifest_path),
        "validation_report": display_path(validation_report_path),
        "replay_report": display_path(replay_report_path),
        "validated_parsers": list(PARSERS),
        "validated_pdf_count": validation_reports["grobid"].total,
        "replay_parser": "grobid",
        "replay_arxiv_id": replay_arxiv_id,
        "validation_passed": validation_passed,
        "replay_passed": replay_passed,
        "passed": validation_passed and replay_passed,
    }
    e2e_report_path = output_dir / "m059-s02-e2e-report.json"
    write_json(e2e_report_path, summary)
    summary["e2e_report"] = display_path(e2e_report_path)
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        default=str(DEFAULT_MANIFEST.relative_to(ROOT)),
        help="Repository-relative manifest path.",
    )
    parser.add_argument(
        "--output-dir",
        default=str(DEFAULT_OUTPUT_DIR.relative_to(ROOT)),
        help="Repository-relative output directory for reports.",
    )
    parser.add_argument("--json", action="store_true", help="Emit the e2e summary as JSON.")
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(list(argv) if argv is not None else None)
    try:
        summary = run_e2e(ROOT / args.manifest, ROOT / args.output_dir)
    except Exception as exc:  # pragma: no cover - CLI defensive boundary
        print(f"ERROR e2e failed: {exc}", file=sys.stderr)
        return 2

    if args.json:
        print(json.dumps(summary, indent=2, sort_keys=True))
    else:
        print(
            "m059 s02 e2e "
            f"passed={str(summary['passed']).lower()} "
            f"validation_report={summary['validation_report']} "
            f"replay_report={summary['replay_report']} "
            f"e2e_report={summary['e2e_report']}"
        )
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
