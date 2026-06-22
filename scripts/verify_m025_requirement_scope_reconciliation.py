#!/usr/bin/env python3
"""Validate the M025 requirement scope reconciliation matrix.

The verifier treats the S11 matrix as executable closeout evidence. It checks
that the six intended requirements keep their M025 scope boundaries, evidence
paths are well-formed and present, rendered markdown is in sync at a basic
requirement-row level, and unsafe graph/import/production-write/raw-payload
claims fail closed.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REQUIRED_REQUIREMENT_IDS = {"R024", "R027", "R029", "R030", "R036", "R040"}

EXPECTED_CLASSIFICATIONS = {
    "R024": {
        "current_status": "active",
        "m025_applicability": "m025_advanced_preprocessing_only",
        "s11_verdict": "advanced_not_validated",
        "recommended_requirement_action": "remain_active",
    },
    "R027": {
        "current_status": "active",
        "m025_applicability": "m025_advanced_preprocessing_diagnostics",
        "s11_verdict": "advanced_not_validated",
        "recommended_requirement_action": "remain_active",
    },
    "R029": {
        "current_status": "active",
        "m025_applicability": "m025_advanced_traceable_chunks",
        "s11_verdict": "advanced_not_validated",
        "recommended_requirement_action": "remain_active",
    },
    "R030": {
        "current_status": "validated",
        "m025_applicability": "already_validated_supported_by_m025",
        "s11_verdict": "covered_by_existing_validation_or_supported",
        "recommended_requirement_action": "stay_already_validated",
    },
    "R036": {
        "current_status": "active",
        "m025_applicability": "m025_advanced_audit_provenance",
        "s11_verdict": "advanced_not_fully_validated",
        "recommended_requirement_action": "remain_active",
    },
    "R040": {
        "current_status": "active",
        "m025_applicability": "constraint_followed_not_validated",
        "s11_verdict": "satisfied_as_constraint",
        "recommended_requirement_action": "treat_as_followed_constraint",
    },
}

REQUIRED_TOP_LEVEL_KEYS = {
    "milestone_id",
    "slice_id",
    "schema_version",
    "metadata_only",
    "scope_boundary",
    "global_forbidden_claims",
    "requirements",
    "negative_tests_for_later_verifier",
    "review_notes",
}

REQUIRED_ROW_KEYS = {
    "requirement_id",
    "current_status",
    "m025_applicability",
    "s11_verdict",
    "recommended_requirement_action",
    "evidence_paths",
    "observed_m025_evidence",
    "allowed_claims",
    "forbidden_claims",
    "remaining_work",
    "rationale",
}

REQUIRED_COVERAGE_PHRASES = {
    "doc/validation/m025_requirement_scope_matrix.json",
    "doc/validation/m025_requirement_scope_matrix.md",
    "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-summary.json",
    "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/final-replay-report.md",
    "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/readiness-decision.json",
    "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-summary.json",
    "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-report.md",
    "data/article_corpora/m025-rlm-dspy-pageindex-smoke-v1/boundary-replay-events.jsonl",
    "m025_advanced_preprocessing_only",
    "m025_advanced_preprocessing_diagnostics",
    "m025_advanced_traceable_chunks",
    "already_validated_supported_by_m025",
    "m025_advanced_audit_provenance",
    "constraint_followed_not_validated",
    "Q5 — Failure Modes",
    "Q6 — Load Profile",
    "Q7 — Negative Tests",
}

# These phrases are safe when they appear in forbidden-claim or out-of-scope
# sections, but unsafe when they appear as allowed/positive closeout claims.
UNSAFE_POSITIVE_CLAIM_PHRASES = {
    "m025 validates graph readiness",
    "m025 validates positive graph readiness",
    "m025 accepts the graph-readiness quality contract",
    "m025 authorizes kg import readiness",
    "m025 authorizes semantic kg import readiness",
    "m025 validates kg import readiness",
    "m025 validates import-ready typed chunk packages",
    "m025 authorizes production ladybugdb writes",
    "m025 authorizes production kg import",
    "m025 validates 20-document scientific kg behavior",
    "m025 validates one-week corpus scientific kg behavior",
    "m025 validates scientific kg graph quality",
    "m025 activates dspy",
    "m025 activates rlm",
    "m025 activates minimax",
    "m025 activates optimizer",
    "m025 fully validates r024",
    "m025 fully validates r027",
    "m025 fully validates r029",
    "m025 fully validates r036",
    "m025 fully validates all future infrastructure safety wrapping",
    "m025 newly validates r030",
    "m025 embeds raw article text",
    "m025 embeds raw binary assets",
    "m025 embeds base64 payloads",
    "m025 exposes raw article payload text",
}

UNSAFE_TRUE_BOOLEAN_KEYS = {
    "graph_readiness_claim",
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "kg_import_ready",
    "import_ready",
    "import_eligible",
    "production_import_attempted",
    "production_ladybugdb_write_allowed",
    "ladybugdb_written",
    "dspy_activated",
    "rlm_activated",
    "optimizer_activated",
    "raw_article_text_included",
    "raw_payloads_included",
    "binary_payloads_included",
    "base64_payloads_included",
    "vectors_included",
    "secrets_included",
}

UNSAFE_FIELD_NAME_FRAGMENTS = {
    "raw_article_text",
    "raw_text",
    "raw_payload",
    "binary_payload",
    "base64_payload",
    "vector_payload",
    "embedding_payload",
    "secret_value",
    "production_connection",
}

POSITIVE_TEXT_KEYS = {
    "allowed_claims",
    "observed_m025_evidence",
    "rationale",
    "remaining_work",
    "review_notes",
}

PLANNING_PATH_PREFIXES = (".gsd/", ".planning/", ".audits/")


class MatrixValidationError(RuntimeError):
    """Raised for unexpected verifier misuse rather than matrix failures."""


def _strings_from(value: Any) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        strings: list[str] = []
        for item in value:
            strings.extend(_strings_from(item))
        return strings
    if isinstance(value, dict):
        strings: list[str] = []
        for item in value.values():
            strings.extend(_strings_from(item))
        return strings
    return []


def _walk(value: Any, path: str = "$") -> list[tuple[str, Any]]:
    items = [(path, value)]
    if isinstance(value, dict):
        for key, child in value.items():
            items.extend(_walk(child, f"{path}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            items.extend(_walk(child, f"{path}[{index}]"))
    return items


def _is_planning_path(path: str) -> bool:
    normalized = path.replace("\\", "/")
    return normalized.startswith(PLANNING_PATH_PREFIXES)


def _validate_evidence_path(
    *,
    requirement_id: str,
    path_value: str,
    repo_root: Path,
    allow_planning_evidence: bool,
) -> list[str]:
    errors: list[str] = []
    if not path_value or path_value.strip() != path_value:
        return [f"{requirement_id} evidence path is blank or padded: {path_value!r}"]
    if "://" in path_value:
        errors.append(
            f"{requirement_id} evidence path must be a repo-relative path, not a URL: {path_value}"
        )
    path = Path(path_value)
    if path.is_absolute():
        errors.append(f"{requirement_id} evidence path must be relative: {path_value}")
        return errors
    if any(part == ".." for part in path.parts):
        errors.append(f"{requirement_id} evidence path must not escape the repo: {path_value}")
        return errors
    if _is_planning_path(path_value) and not allow_planning_evidence:
        return errors
    resolved = (repo_root / path).resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError:
        errors.append(f"{requirement_id} evidence path escapes repo root: {path_value}")
        return errors
    if not resolved.exists():
        errors.append(f"{requirement_id} evidence path does not exist: {path_value}")
    return errors


def _positive_text_for_matrix(matrix: dict[str, Any]) -> dict[str, str]:
    positive: dict[str, str] = {}
    scope = matrix.get("scope_boundary")
    if isinstance(scope, dict):
        positive["$.scope_boundary.summary"] = "\n".join(_strings_from(scope.get("summary")))
        positive["$.scope_boundary.in_scope"] = "\n".join(_strings_from(scope.get("in_scope")))
        positive["$.scope_boundary.safe_next_step"] = "\n".join(
            _strings_from(scope.get("safe_next_step"))
        )
    positive["$.review_notes"] = "\n".join(_strings_from(matrix.get("review_notes")))
    requirements = matrix.get("requirements")
    if isinstance(requirements, list):
        for index, row in enumerate(requirements):
            if not isinstance(row, dict):
                continue
            rid = (
                row.get("requirement_id")
                if isinstance(row.get("requirement_id"), str)
                else f"index {index}"
            )
            for key in POSITIVE_TEXT_KEYS:
                if key in row:
                    positive[f"$.requirements[{index}]({rid}).{key}"] = "\n".join(
                        _strings_from(row.get(key))
                    )
    return positive


def validate_matrix(
    matrix: dict[str, Any],
    rendered_markdown: str,
    *,
    repo_root: Path | None = None,
    required_requirements: set[str] | None = None,
    reject_unsafe_claims: bool = True,
    allow_planning_evidence: bool = True,
) -> list[str]:
    """Return validation diagnostics for the M025 matrix and rendered markdown."""

    repo_root = repo_root or Path.cwd()
    required = required_requirements or REQUIRED_REQUIREMENT_IDS
    errors: list[str] = []

    missing_top = sorted(REQUIRED_TOP_LEVEL_KEYS - set(matrix))
    if missing_top:
        errors.append(f"missing top-level keys: {', '.join(missing_top)}")
    if matrix.get("milestone_id") != "M025-6xovy3":
        errors.append("milestone_id must be M025-6xovy3")
    if matrix.get("slice_id") != "S11":
        errors.append("slice_id must be S11")
    if matrix.get("metadata_only") is not True:
        errors.append("$.metadata_only must be true")

    requirements = matrix.get("requirements")
    if not isinstance(requirements, list):
        return errors + ["$.requirements must be a list"]

    seen: dict[str, int] = {}
    rows: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(requirements):
        if not isinstance(row, dict):
            errors.append(f"$.requirements[{index}] must be an object")
            continue
        rid = row.get("requirement_id")
        if not isinstance(rid, str) or not rid:
            errors.append(f"$.requirements[{index}].requirement_id missing or not a string")
            continue
        seen[rid] = seen.get(rid, 0) + 1
        rows[rid] = row
        missing_row_keys = sorted(REQUIRED_ROW_KEYS - set(row))
        if missing_row_keys:
            errors.append(f"{rid} missing required fields: {', '.join(missing_row_keys)}")

    missing_ids = sorted(required - set(seen))
    extra_ids = sorted(set(seen) - required)
    duplicate_ids = sorted(rid for rid, count in seen.items() if count > 1)
    if missing_ids:
        errors.append(f"missing requirement rows: {', '.join(missing_ids)}")
    if extra_ids:
        errors.append(f"unexpected requirement rows: {', '.join(extra_ids)}")
    if duplicate_ids:
        errors.append(f"duplicate requirement rows: {', '.join(duplicate_ids)}")

    for rid, expected in EXPECTED_CLASSIFICATIONS.items():
        row = rows.get(rid)
        if row is None:
            continue
        for key, expected_value in expected.items():
            if row.get(key) != expected_value:
                errors.append(f"{rid} {key} must be {expected_value}, found {row.get(key)!r}")

    for rid, row in rows.items():
        evidence_paths = row.get("evidence_paths")
        if not isinstance(evidence_paths, list) or not evidence_paths:
            errors.append(f"{rid} evidence_paths must be a non-empty list")
        else:
            for path_value in evidence_paths:
                if not isinstance(path_value, str):
                    errors.append(f"{rid} evidence path must be a string: {path_value!r}")
                    continue
                errors.extend(
                    _validate_evidence_path(
                        requirement_id=rid,
                        path_value=path_value,
                        repo_root=repo_root,
                        allow_planning_evidence=allow_planning_evidence,
                    )
                )
        for key in (
            "allowed_claims",
            "forbidden_claims",
            "remaining_work",
            "observed_m025_evidence",
        ):
            value = row.get(key)
            if (
                not isinstance(value, list)
                or not value
                or not all(isinstance(item, str) and item for item in value)
            ):
                errors.append(f"{rid} {key} must be a non-empty list of strings")
        if not isinstance(row.get("rationale"), str) or not row.get("rationale"):
            errors.append(f"{rid} rationale must be a non-empty string")

    # Requirement-specific semantic guards against status drift.
    for rid in ("R024", "R027", "R029"):
        row = rows.get(rid)
        if row is None:
            continue
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if (
            "fully validates" in positive
            or "validated_by_m025" in positive
            or row.get("current_status") == "validated"
        ):
            errors.append(f"{rid} must not be marked fully validated by M025 evidence")
    row = rows.get("R036")
    if row is not None:
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if "fully validates" in positive or row.get("current_status") == "validated":
            errors.append(
                "R036 must remain active/advanced unless full CLI provenance is separately proven"
            )
    row = rows.get("R030")
    if row is not None:
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if row.get("current_status") != "validated" or "already validated" not in positive:
            errors.append(
                "R030 must be treated as already validated/supported, not reopened or downgraded"
            )
        if "newly validates" in positive:
            errors.append("R030 must not be claimed as newly validated by M025")
    row = rows.get("R040")
    if row is not None:
        positive = "\n".join(
            _strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})
        ).lower()
        if (
            "fully validates all future infrastructure" in positive
            or row.get("current_status") == "validated"
        ):
            errors.append(
                "R040 must be treated as a followed constraint, not universal future safety validation"
            )

    # Unsafe positive claims and unsafe fields/booleans.
    if reject_unsafe_claims:
        for path, text in _positive_text_for_matrix(matrix).items():
            lowered = text.lower()
            for phrase in sorted(UNSAFE_POSITIVE_CLAIM_PHRASES):
                if phrase in lowered:
                    errors.append(f"{path} contains unsafe claim phrase: {phrase}")
    for path, value in _walk(matrix):
        key = path.rsplit(".", maxsplit=1)[-1].lower()
        key_without_index = key.split("[", maxsplit=1)[0]
        if (
            isinstance(value, bool)
            and value is True
            and key_without_index in UNSAFE_TRUE_BOOLEAN_KEYS
        ):
            errors.append(f"{path} unsafe boolean field must not be true")
        if any(fragment in key_without_index for fragment in UNSAFE_FIELD_NAME_FRAGMENTS):
            if key_without_index not in {"negative_tests_for_later_verifier"}:
                errors.append(f"{path} contains unsafe raw/binary/base64/vector/secret field name")

    # Rendered markdown sync checks: every required row and key classification must appear.
    if not rendered_markdown.strip():
        errors.append("rendered markdown is empty")
    for rid in sorted(required):
        if rid not in rendered_markdown:
            errors.append(f"rendered markdown missing requirement id: {rid}")
        expected = EXPECTED_CLASSIFICATIONS.get(rid, {})
        for key, expected_value in expected.items():
            if expected_value not in rendered_markdown:
                errors.append(f"rendered markdown missing {rid} {key}: {expected_value}")
    if "doc/validation/m025_requirement_scope_matrix.json" not in rendered_markdown:
        errors.append("rendered markdown does not reference the source matrix JSON path")

    return errors


def validate_coverage_handoff(coverage_markdown: str) -> list[str]:
    """Return diagnostics for the S11 human coverage handoff."""

    errors: list[str] = []
    if not coverage_markdown.strip():
        return ["coverage handoff is empty"]
    for phrase in sorted(REQUIRED_COVERAGE_PHRASES):
        if phrase not in coverage_markdown:
            errors.append(f"coverage handoff missing required phrase/path: {phrase}")
    for rid in sorted(REQUIRED_REQUIREMENT_IDS):
        if rid not in coverage_markdown:
            errors.append(f"coverage handoff missing requirement id: {rid}")
    return errors


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise MatrixValidationError(f"matrix file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise MatrixValidationError(f"malformed JSON at {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise MatrixValidationError("matrix root must be an object")
    return payload


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--matrix", required=True, type=Path, help="Path to m025_requirement_scope_matrix.json"
    )
    parser.add_argument(
        "--rendered", required=True, type=Path, help="Path to rendered matrix markdown"
    )
    parser.add_argument("--coverage", type=Path, help="Optional path to S11-COVERAGE.md handoff")
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--require-requirements", nargs="+", default=sorted(REQUIRED_REQUIREMENT_IDS)
    )
    parser.add_argument("--reject-unsafe-claims", action="store_true")
    parser.add_argument(
        "--disallow-planning-evidence",
        action="store_true",
        help="Do not read .gsd/.planning/.audits evidence paths; used by tests to stay outside planning state.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        matrix = _load_json(args.matrix)
        try:
            rendered = args.rendered.read_text(encoding="utf-8")
        except FileNotFoundError as exc:
            raise MatrixValidationError(
                f"rendered markdown file not found: {args.rendered}"
            ) from exc
        errors = validate_matrix(
            matrix,
            rendered,
            repo_root=args.repo_root,
            required_requirements=set(args.require_requirements),
            reject_unsafe_claims=args.reject_unsafe_claims,
            allow_planning_evidence=not args.disallow_planning_evidence,
        )
        if args.coverage is not None:
            try:
                coverage = args.coverage.read_text(encoding="utf-8")
            except FileNotFoundError as exc:
                raise MatrixValidationError(
                    f"coverage handoff file not found: {args.coverage}"
                ) from exc
            errors.extend(validate_coverage_handoff(coverage))
    except MatrixValidationError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2
    if errors:
        sys.stderr.write("M025 requirement scope reconciliation validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    sys.stdout.write(
        "M025 requirement scope reconciliation validation passed: "
        f"{len(matrix['requirements'])} requirement rows checked.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
