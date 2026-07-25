#!/usr/bin/env python3
"""Validate parser outputs for one parser across a PDF batch manifest.

This tool is intentionally read-only. It validates existing artifact JSON files
against the parser-specific schema declared in the manifest and enforces the
M059 safety defaults before touching parser outputs.
"""

from __future__ import annotations

import argparse
import glob
import json
import sys
from collections.abc import Iterable
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from jsonschema import Draft7Validator
from jsonschema.exceptions import ValidationError

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
class PdfValidationResult:
    """Validation result for one PDF/parser pair."""

    arxiv_id: str
    parser: str
    output_paths: list[str]
    passed: bool
    error_count: int
    missing_fields: list[str]
    message: str


@dataclass(frozen=True)
class BatchValidationReport:
    """Aggregate validation report for one manifest/parser pair."""

    manifest: str
    batch_id: str
    parser: str
    total: int
    passed: int
    failed: int
    success_rate: float
    missing_outputs: int
    missing_fields: dict[str, int]
    safety_defaults: dict[str, bool]
    results: list[PdfValidationResult]

    def to_jsonable(self) -> dict[str, Any]:
        payload = asdict(self)
        payload["results"] = [asdict(result) for result in self.results]
        return payload


def repo_path(path: str | Path) -> Path:
    """Resolve an absolute or repository-relative path."""
    candidate = Path(path)
    return candidate if candidate.is_absolute() else ROOT / candidate


def rel(path: Path) -> str:
    """Return a repository-relative path when possible."""
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def read_json(path: str | Path) -> Any:
    """Read a JSON file from an absolute or repository-relative path."""
    actual = repo_path(path)
    try:
        return json.loads(actual.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise FileNotFoundError(f"JSON file does not exist: {rel(actual)}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"Invalid JSON in {rel(actual)}: {exc}") from exc


def load_validator(schema_path: str | Path) -> Draft7Validator:  # ty:ignore[invalid-type-form]
    """Load and validate a Draft 7 JSON Schema."""
    schema = read_json(schema_path)
    Draft7Validator.check_schema(schema)
    return Draft7Validator(schema)


def ensure_safety_defaults(value: Any, *, context: str) -> None:
    """Require the five explicit false M059 safety defaults."""
    if not isinstance(value, dict):
        raise ValueError(f"{context} safety_defaults must be an object")
    missing = sorted(key for key in SAFETY_DEFAULTS if key not in value)
    unsafe = sorted(key for key in SAFETY_DEFAULTS if value.get(key) is not False)
    if missing or unsafe:
        details: list[str] = []
        if missing:
            details.append(f"missing={','.join(missing)}")
        if unsafe:
            details.append(f"not_false={','.join(unsafe)}")
        raise ValueError(
            f"{context} safety_defaults must include five explicit false values ({'; '.join(details)})"
        )


def validate_manifest_contract(manifest: dict[str, Any]) -> None:
    """Validate the manifest itself when it declares its schema."""
    schema_path = manifest.get("manifest_schema")
    if not schema_path:
        return
    validator = load_validator(str(schema_path))
    errors = sorted(validator.iter_errors(manifest), key=lambda error: list(error.path))
    if errors:
        first = errors[0]
        location = "/".join(str(part) for part in first.path) or "<root>"
        raise ValueError(f"manifest schema validation failed at {location}: {first.message}")


def find_parser_expectation(pdf: dict[str, Any], parser_name: str) -> dict[str, Any] | None:
    """Find the manifest parser expectation for a requested parser name or alias."""
    requested = parser_name.casefold()
    for expectation in pdf.get("expected_parsers", []):
        name = str(expectation.get("name", "")).casefold()
        aliases = [str(alias).casefold() for alias in expectation.get("aliases", [])]
        if requested == name or requested in aliases:
            return expectation
    return None


def _expand_template(template: str, pdf: dict[str, Any]) -> list[Path]:
    arxiv_id = str(pdf["arxiv_id"])
    formatted = template.format(arxiv_id=arxiv_id, article_key=pdf.get("article_key", arxiv_id))
    if "*" in formatted:
        return sorted(Path(match) for match in glob.glob(str(repo_path(formatted)), recursive=True))
    return [repo_path(formatted)]


def resolve_output_paths(pdf: dict[str, Any], expectation: dict[str, Any]) -> list[Path]:
    """Resolve per-PDF or batch-level output paths declared by a parser expectation."""
    paths: list[Path] = []
    for key in ("output_path", "batch_output_path"):
        value = expectation.get(key)
        if value:
            paths.append(repo_path(str(value)))

    output_paths = expectation.get("output_paths")
    if isinstance(output_paths, list):
        paths.extend(repo_path(str(path)) for path in output_paths)
    elif isinstance(output_paths, dict):
        paths.extend(repo_path(str(path)) for path in output_paths.values())

    template = expectation.get("output_path_template")
    if template:
        paths.extend(_expand_template(str(template), pdf))

    seen: set[Path] = set()
    unique: list[Path] = []
    for path in paths:
        resolved = path.resolve()
        if resolved not in seen:
            unique.append(path)
            seen.add(resolved)
    return unique


def _missing_fields_from_errors(errors: Iterable[ValidationError]) -> list[str]:
    fields: set[str] = set()
    for error in errors:
        if error.validator == "required":
            for missing in error.validator_value:  # pyrefly: ignore[not-iterable]
                if missing not in error.instance:
                    fields.add(str(missing))
    return sorted(fields)


def _format_schema_error(error: ValidationError) -> str:
    location = "/".join(str(part) for part in error.path) or "<root>"
    return f"first at {location}: {error.message}"


def validate_pdf(pdf: dict[str, Any], parser_name: str) -> PdfValidationResult:
    """Validate one PDF's output for the requested parser."""
    arxiv_id = str(pdf["arxiv_id"])
    expectation = find_parser_expectation(pdf, parser_name)
    if expectation is None:
        return PdfValidationResult(
            arxiv_id,
            parser_name,
            [],
            False,
            1,
            [],
            f"parser {parser_name!r} is not declared for {arxiv_id}",
        )

    output_paths = resolve_output_paths(pdf, expectation)
    if not output_paths:
        return PdfValidationResult(
            arxiv_id, parser_name, [], False, 1, [], "parser output path could not be resolved"
        )

    missing_paths = [path for path in output_paths if not path.exists()]
    if missing_paths:
        return PdfValidationResult(
            arxiv_id,
            parser_name,
            [rel(path) for path in output_paths],
            False,
            len(missing_paths),
            [],
            "parser output is missing: " + ", ".join(rel(path) for path in missing_paths),
        )

    schema_path = str(expectation.get("expected_output_schema", ""))
    if not schema_path:
        return PdfValidationResult(
            arxiv_id,
            parser_name,
            [rel(path) for path in output_paths],
            False,
            1,
            ["expected_output_schema"],
            "parser expectation has no expected_output_schema",
        )

    validator = load_validator(schema_path)
    error_count = 0
    missing_fields: set[str] = set()
    first_error: ValidationError | None = None
    for output_path in output_paths:
        payload = read_json(output_path)
        errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.path))
        if errors:
            error_count += len(errors)
            missing_fields.update(_missing_fields_from_errors(errors))
            first_error = first_error or errors[0]

    if error_count:
        message = (
            f"{error_count} schema error(s); {_format_schema_error(first_error)}"
            if first_error
            else f"{error_count} schema error(s)"
        )
        return PdfValidationResult(
            arxiv_id,
            parser_name,
            [rel(path) for path in output_paths],
            False,
            error_count,
            sorted(missing_fields),
            message,
        )

    return PdfValidationResult(
        arxiv_id, parser_name, [rel(path) for path in output_paths], True, 0, [], "ok"
    )


def validate_batch(manifest_path: str | Path, parser_name: str) -> BatchValidationReport:
    """Validate all outputs for one parser across a manifest."""
    manifest_actual = repo_path(manifest_path)
    manifest = read_json(manifest_actual)
    if not isinstance(manifest, dict):
        raise ValueError("manifest root must be an object")
    validate_manifest_contract(manifest)
    ensure_safety_defaults(manifest.get("safety_defaults"), context="manifest")

    results = [validate_pdf(pdf, parser_name) for pdf in manifest.get("pdfs", [])]
    passed = sum(1 for result in results if result.passed)
    failed = len(results) - passed
    missing_outputs = sum(
        1
        for result in results
        if "parser output is missing" in result.message or not result.output_paths
    )
    missing_fields: dict[str, int] = {}
    for result in results:
        for field in result.missing_fields:
            missing_fields[field] = missing_fields.get(field, 0) + 1

    total = len(results)
    return BatchValidationReport(
        manifest=rel(manifest_actual),
        batch_id=str(manifest.get("batch_id", "")),
        parser=parser_name,
        total=total,
        passed=passed,
        failed=failed,
        success_rate=(passed / total) if total else 0.0,
        missing_outputs=missing_outputs,
        missing_fields=dict(sorted(missing_fields.items())),
        safety_defaults=SAFETY_DEFAULTS.copy(),
        results=results,
    )


def print_report(report: BatchValidationReport, *, json_output: bool = False) -> None:
    """Print a validation report to stdout."""
    if json_output:
        print(json.dumps(report.to_jsonable(), indent=2, sort_keys=True))
        return

    for result in report.results:
        status = "PASS" if result.passed else "FAIL"
        paths = ",".join(result.output_paths) if result.output_paths else "<unresolved>"
        missing = ",".join(result.missing_fields) if result.missing_fields else "-"
        print(
            f"{status} {result.arxiv_id} parser={result.parser} outputs={paths} "
            f"errors={result.error_count} missing_fields={missing} message={result.message}"
        )
    print(
        "aggregate "
        f"batch={report.batch_id} parser={report.parser} total={report.total} "
        f"passed={report.passed} failed={report.failed} "
        f"success_rate={report.success_rate:.3f} missing_outputs={report.missing_outputs} "
        f"missing_fields={json.dumps(report.missing_fields, sort_keys=True)}"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, help="Repository-relative manifest JSON path.")
    parser.add_argument(
        "--parser", required=True, help="Parser name declared in expected_parsers[]."
    )
    parser.add_argument(
        "--json", action="store_true", help="Emit the full validation report as JSON."
    )
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = build_parser().parse_args(list(argv) if argv is not None else None)
    try:
        report = validate_batch(args.manifest, args.parser)
    except Exception as exc:  # pragma: no cover - CLI defensive boundary
        print(f"ERROR validation setup failed: {exc}", file=sys.stderr)
        return 2

    print_report(report, json_output=args.json)
    return 0 if report.failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
