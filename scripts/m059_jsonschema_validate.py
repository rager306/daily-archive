#!/usr/bin/env python3
"""Validate parser outputs referenced by a daily-archive PDF batch manifest."""

from __future__ import annotations

import argparse
import glob
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LOOPBACK_BASE_URL = "http://127.0.0.1:8070"

SAFETY_DEFAULTS: dict[str, bool] = {
    "external_network_authorized": False,
    "graph_writes_authorized": False,
    "production_import_authorized": False,
    "fact_promotion_authorized": False,
    "llm_calls_authorized": False,
}


@dataclass(frozen=True)
class ValidationResult:
    """One parser-output validation result."""

    arxiv_id: str
    parser: str
    output_path: str | None
    passed: bool
    error_count: int
    message: str


def read_json(path: Path) -> Any:
    """Read JSON from an absolute or repository-relative path."""
    actual = path if path.is_absolute() else ROOT / path
    return json.loads(actual.read_text())


def repo_path(path: str | Path) -> Path:
    """Resolve a path relative to the repository root."""
    candidate = Path(path)
    return candidate if candidate.is_absolute() else ROOT / candidate


def load_validator(schema_path: str) -> Draft7Validator:
    """Load and check a draft-07 JSON Schema."""
    schema = read_json(Path(schema_path))
    Draft7Validator.check_schema(schema)
    return Draft7Validator(schema)


def find_parser_expectation(pdf: dict[str, Any], parser_name: str) -> dict[str, Any] | None:
    """Find the manifest parser expectation for a requested parser name."""
    requested = parser_name.casefold()
    for expectation in pdf.get("expected_parsers", []):
        name = str(expectation.get("name", "")).casefold()
        aliases = [str(alias).casefold() for alias in expectation.get("aliases", [])]
        if requested == name or requested in aliases:
            return expectation
    return None


def resolve_output_path(pdf: dict[str, Any], expectation: dict[str, Any]) -> Path | None:
    """Resolve a per-PDF or batch parser output path from a manifest expectation."""
    arxiv_id = str(pdf["arxiv_id"])
    template = expectation.get("output_path_template")
    if template:
        formatted = str(template).format(arxiv_id=arxiv_id, article_key=pdf.get("article_key", arxiv_id))
        if "*" in formatted:
            matches = sorted(Path(match) for match in glob.glob(str(repo_path(formatted)), recursive=True))
            return matches[0] if matches else None
        return repo_path(formatted)
    batch_output = expectation.get("batch_output_path")
    if batch_output:
        return repo_path(str(batch_output))
    return None


def validate_one(pdf: dict[str, Any], parser_name: str) -> ValidationResult:
    """Validate one PDF parser output and return a structured result."""
    arxiv_id = str(pdf["arxiv_id"])
    expectation = find_parser_expectation(pdf, parser_name)
    if expectation is None:
        return ValidationResult(arxiv_id, parser_name, None, False, 1, f"parser {parser_name!r} is not declared for {arxiv_id}")

    output_path = resolve_output_path(pdf, expectation)
    if output_path is None:
        return ValidationResult(arxiv_id, parser_name, None, False, 1, "parser output path could not be resolved")
    if not output_path.exists():
        return ValidationResult(arxiv_id, parser_name, output_path.relative_to(ROOT).as_posix(), False, 1, "parser output is missing")

    schema_path = str(expectation["expected_output_schema"])
    validator = load_validator(schema_path)
    payload = read_json(output_path)
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.path))
    if errors:
        first = errors[0]
        location = "/".join(str(part) for part in first.path) or "<root>"
        return ValidationResult(
            arxiv_id,
            parser_name,
            output_path.relative_to(ROOT).as_posix(),
            False,
            len(errors),
            f"{len(errors)} schema error(s); first at {location}: {first.message}",
        )
    return ValidationResult(arxiv_id, parser_name, output_path.relative_to(ROOT).as_posix(), True, 0, "ok")


def validate_manifest(manifest_path: Path, parser_name: str) -> list[ValidationResult]:
    """Validate all parser outputs for one parser across a manifest."""
    manifest = read_json(manifest_path)
    safety_defaults = manifest.get("safety_defaults")
    if safety_defaults != SAFETY_DEFAULTS:
        raise ValueError("manifest safety defaults must be the five explicit false M059 defaults")
    return [validate_one(pdf, parser_name) for pdf in manifest.get("pdfs", [])]


def print_results(results: list[ValidationResult]) -> None:
    """Print per-PDF validation results and aggregate stats."""
    passed = sum(1 for result in results if result.passed)
    failed = len(results) - passed
    missing = sum(1 for result in results if result.output_path is None or result.message == "parser output is missing")
    for result in results:
        status = "PASS" if result.passed else "FAIL"
        path = result.output_path or "<unresolved>"
        print(f"{status} {result.arxiv_id} parser={result.parser} output={path} errors={result.error_count} message={result.message}")
    print(f"aggregate total={len(results)} passed={passed} failed={failed} missing={missing}")


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate parser JSON outputs referenced by a PDF batch manifest.")
    parser.add_argument("--manifest", required=True, help="Repository-relative manifest JSON path.")
    parser.add_argument("--parser", required=True, help="Parser name declared in expected_parsers[].")
    args = parser.parse_args(list(argv) if argv is not None else None)

    try:
        results = validate_manifest(Path(args.manifest), args.parser)
    except Exception as exc:  # pragma: no cover - CLI defensive boundary
        print(f"ERROR manifest validation setup failed: {exc}", file=sys.stderr)
        return 2

    print_results(results)
    return 0 if all(result.passed for result in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
