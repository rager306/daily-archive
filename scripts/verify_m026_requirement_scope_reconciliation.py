#!/usr/bin/env python3
"""Validate the M026 requirement scope reconciliation matrix.

The verifier treats the S05 matrix as executable closeout evidence. It checks
that every active/touched requirement row stays within the M026 boundary,
evidence paths are repo-relative and safe, rendered markdown is in sync at a
basic requirement-row level, and unsafe loader/KG/import/production/raw-payload
claims fail closed.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

REQUIRED_REQUIREMENT_IDS = {
    "R001",
    "R002",
    "R003",
    "R004",
    "R005",
    "R006",
    "R007",
    "R008",
    "R009",
    "R010",
    "R014",
    "R019",
    "R022",
    "R023",
    "R024",
    "R027",
    "R029",
    "R030",
    "R031",
    "R032",
    "R033",
    "R035",
    "R036",
    "R040",
    "R050",
    "R051",
    "R052",
}

EXISTING_CONTEXT_REQUIREMENT_IDS = {
    "R001",
    "R002",
    "R003",
    "R004",
    "R005",
    "R006",
    "R007",
    "R008",
    "R009",
    "R010",
    "R014",
    "R030",
}

OUT_OF_SCOPE_ACTIVE_REQUIREMENT_IDS = {
    "R019",
    "R022",
    "R023",
    "R024",
    "R027",
    "R029",
    "R031",
    "R032",
    "R033",
    "R035",
    "R051",
    "R052",
}

EXPECTED_CLASSIFICATIONS = {
    **{
        rid: {
            "current_status": "validated",
            "m026_applicability": "existing_hermes_daily_archive_context",
            "s05_verdict": "existing_coverage_context_not_revalidated",
            "recommended_requirement_action": "preserve_existing_validated_status",
        }
        for rid in ("R001", "R002", "R003", "R004", "R005", "R006", "R007", "R008", "R009", "R010")
    },
    "R014": {
        "current_status": "validated",
        "m026_applicability": "existing_validated_compatibility_context",
        "s05_verdict": "existing_coverage_supported_not_revalidated",
        "recommended_requirement_action": "preserve_existing_validated_status",
    },
    **{
        rid: {
            "current_status": "active",
            "m026_applicability": "out_of_scope_active_requirement",
            "s05_verdict": "not_advanced_not_validated",
            "recommended_requirement_action": "remain_active",
        }
        for rid in OUT_OF_SCOPE_ACTIVE_REQUIREMENT_IDS
    },
    "R036": {
        "current_status": "active",
        "m026_applicability": "adjacent_evidence_not_full_requirement",
        "s05_verdict": "advanced_not_validated",
        "recommended_requirement_action": "remain_active",
    },
    "R040": {
        "current_status": "active",
        "m026_applicability": "in_scope_constraint_followed",
        "s05_verdict": "satisfied_for_m026_not_globally_validated",
        "recommended_requirement_action": "treat_as_followed_constraint_keep_active",
    },
    "R050": {
        "current_status": "active",
        "m026_applicability": "out_of_scope_future_consumer",
        "s05_verdict": "not_implemented_not_validated",
        "recommended_requirement_action": "remain_active",
    },
    "R030": {
        "current_status": "validated",
        "m026_applicability": "existing_validated_supporting_context",
        "s05_verdict": "existing_coverage_supported_not_revalidated",
        "recommended_requirement_action": "preserve_existing_validated_status",
    },
}

REQUIRED_TOP_LEVEL_KEYS = {
    "milestone_id",
    "slice_id",
    "schema_version",
    "metadata_only",
    "no_runtime_surface_changed",
    "safety_flags",
    "scope_boundary",
    "global_forbidden_claims",
    "required_requirement_ids",
    "requirements",
    "verifier_expectations",
    "quality_gates",
}

REQUIRED_ROW_KEYS = {
    "requirement_id",
    "current_status",
    "m026_applicability",
    "s05_verdict",
    "recommended_requirement_action",
    "evidence_paths",
    "observed_m026_evidence",
    "allowed_claims",
    "forbidden_claims",
    "remaining_work",
    "rationale",
}

REQUIRED_SAFETY_FLAGS = {
    "loader_implementation_claimed",
    "batch_selection_implementation_claimed",
    "kg_import_or_readiness_claimed",
    "graph_validation_claimed",
    "import_ready_chunks_claimed",
    "scientific_kg_corpus_validation_claimed",
    "dspy_rlm_minimax_activation_claimed",
    "raw_payloads_embedded",
    "binary_payloads_embedded",
    "vector_payloads_embedded",
    "secrets_embedded",
    "production_ladybugdb_writes_claimed",
}

# These phrases are safe when they appear in forbidden-claim/out-of-scope text,
# but unsafe when they appear as allowed or positive closeout claims.
UNSAFE_POSITIVE_CLAIM_PHRASES = {
    "m026 implements the loader",
    "m026 implements loader",
    "m026 implements batch selection",
    "m026 authorizes kg import",
    "m026 authorizes kg import readiness",
    "m026 validates kg import readiness",
    "m026 validates graph readiness",
    "m026 validates scientific kg corpus behavior",
    "m026 validates import-ready chunks",
    "m026 validates import ready chunks",
    "m026 activates dspy",
    "m026 activates rlm",
    "m026 activates minimax",
    "m026 activates optimizer",
    "m026 writes to production ladybugdb",
    "m026 authorizes production ladybugdb writes",
    "m026 embeds raw article text",
    "m026 embeds raw pdfs",
    "m026 embeds pdf bytes",
    "m026 embeds binary payloads",
    "m026 embeds base64 payloads",
    "m026 embeds vector payloads",
    "m026 embeds secrets",
    "m026 newly validates r001",
    "m026 newly validates r002",
    "m026 newly validates r003",
    "m026 newly validates r004",
    "m026 newly validates r005",
    "m026 newly validates r006",
    "m026 newly validates r007",
    "m026 newly validates r008",
    "m026 newly validates r009",
    "m026 newly validates r010",
    "m026 newly validates r014",
    "m026 newly validates r030",
    "m026 fully validates r019",
    "m026 fully validates r022",
    "m026 fully validates r023",
    "m026 fully validates r024",
    "m026 fully validates r027",
    "m026 fully validates r029",
    "m026 fully validates r031",
    "m026 fully validates r032",
    "m026 fully validates r033",
    "m026 fully validates r035",
    "m026 fully validates r036",
    "m026 fully validates r051",
    "m026 fully validates r052",
    "m026 globally validates r040",
    "m026 implements r050",
    "m026 implements r050 with",
}

UNSAFE_TRUE_BOOLEAN_KEYS = {
    "loader_implementation_claimed",
    "batch_selection_implementation_claimed",
    "kg_import_or_readiness_claimed",
    "graph_validation_claimed",
    "import_ready_chunks_claimed",
    "scientific_kg_corpus_validation_claimed",
    "dspy_rlm_minimax_activation_claimed",
    "raw_payloads_embedded",
    "binary_payloads_embedded",
    "base64_payloads_embedded",
    "vector_payloads_embedded",
    "secrets_embedded",
    "production_ladybugdb_writes_claimed",
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
    "minimax_activated",
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

ALLOWED_EVIDENCE_SUFFIXES = {".json", ".jsonl", ".md", ".txt", ".yaml", ".yml"}
PLANNING_PATH_PREFIXES = (".gsd/", ".planning/", ".audits/")
POSITIVE_TEXT_KEYS = {
    "allowed_claims",
    "observed_m026_evidence",
    "rationale",
    "remaining_work",
    "review_notes",
}


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


def _field_name_from_path(path: str) -> str:
    return path.rsplit(".", maxsplit=1)[-1].split("[", maxsplit=1)[0].lower()


def _validate_evidence_path(
    *,
    requirement_id: str,
    path_value: str,
    repo_root: Path,
    require_planning_evidence: bool,
) -> list[str]:
    errors: list[str] = []
    if not path_value or path_value.strip() != path_value:
        return [f"{requirement_id} evidence path is blank or padded: {path_value!r}"]
    if "://" in path_value:
        errors.append(f"{requirement_id} evidence path must be a repo-relative path, not a URL: {path_value}")
    path = Path(path_value)
    if path.is_absolute():
        errors.append(f"{requirement_id} evidence path must be relative: {path_value}")
        return errors
    if any(part == ".." for part in path.parts):
        errors.append(f"{requirement_id} evidence path must not escape the repo: {path_value}")
        return errors
    if path.suffix and path.suffix not in ALLOWED_EVIDENCE_SUFFIXES:
        errors.append(f"{requirement_id} evidence path has unsupported extension: {path_value}")
    if _is_planning_path(path_value) and not require_planning_evidence:
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
        positive["$.scope_boundary.safe_next_step"] = "\n".join(_strings_from(scope.get("safe_next_step")))
    positive["$.quality_gates.observability_impact"] = "\n".join(
        _strings_from(matrix.get("quality_gates", {}).get("observability_impact") if isinstance(matrix.get("quality_gates"), dict) else None)
    )
    requirements = matrix.get("requirements")
    if isinstance(requirements, list):
        for index, row in enumerate(requirements):
            if not isinstance(row, dict):
                continue
            rid = row.get("requirement_id") if isinstance(row.get("requirement_id"), str) else f"index {index}"
            for key in POSITIVE_TEXT_KEYS:
                if key in row:
                    positive[f"$.requirements[{index}]({rid}).{key}"] = "\n".join(_strings_from(row.get(key)))
    return positive


def _validate_safety_flags(matrix: dict[str, Any]) -> list[str]:
    safety_flags = matrix.get("safety_flags")
    if not isinstance(safety_flags, dict):
        return ["$.safety_flags must be an object"]
    errors: list[str] = []
    missing = sorted(REQUIRED_SAFETY_FLAGS - set(safety_flags))
    if missing:
        errors.append(f"$.safety_flags missing required fields: {', '.join(missing)}")
    for key in sorted(REQUIRED_SAFETY_FLAGS & set(safety_flags)):
        if safety_flags.get(key) is not False:
            errors.append(f"$.safety_flags.{key} must be false")
    return errors


def validate_matrix(
    matrix: dict[str, Any],
    rendered_markdown: str,
    *,
    repo_root: Path | None = None,
    required_requirements: set[str] | None = None,
    reject_unsafe_claims: bool = True,
    require_planning_evidence: bool = False,
) -> list[str]:
    """Return validation diagnostics for the M026 matrix and rendered markdown."""

    repo_root = repo_root or Path.cwd()
    required = required_requirements or REQUIRED_REQUIREMENT_IDS
    errors: list[str] = []

    missing_top = sorted(REQUIRED_TOP_LEVEL_KEYS - set(matrix))
    if missing_top:
        errors.append(f"missing top-level keys: {', '.join(missing_top)}")
    if matrix.get("milestone_id") != "M026-3rvvgp":
        errors.append("milestone_id must be M026-3rvvgp")
    if matrix.get("slice_id") != "S05":
        errors.append("slice_id must be S05")
    if matrix.get("metadata_only") is not True:
        errors.append("$.metadata_only must be true")
    if matrix.get("no_runtime_surface_changed") is not True:
        errors.append("$.no_runtime_surface_changed must be true")
    errors.extend(_validate_safety_flags(matrix))

    declared_required = matrix.get("required_requirement_ids")
    if not isinstance(declared_required, list) or not all(isinstance(item, str) for item in declared_required):
        errors.append("$.required_requirement_ids must be a list of strings")
    elif set(declared_required) != required:
        missing_declared = sorted(required - set(declared_required))
        extra_declared = sorted(set(declared_required) - required)
        if missing_declared:
            errors.append(f"$.required_requirement_ids missing ids: {', '.join(missing_declared)}")
        if extra_declared:
            errors.append(f"$.required_requirement_ids has unexpected ids: {', '.join(extra_declared)}")

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
                        require_planning_evidence=require_planning_evidence,
                    )
                )
        for key in ("allowed_claims", "forbidden_claims", "remaining_work", "observed_m026_evidence"):
            value = row.get(key)
            if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
                errors.append(f"{rid} {key} must be a non-empty list of strings")
        if not isinstance(row.get("rationale"), str) or not row.get("rationale"):
            errors.append(f"{rid} rationale must be a non-empty string")

    for rid in OUT_OF_SCOPE_ACTIVE_REQUIREMENT_IDS:
        row = rows.get(rid)
        if row is None:
            continue
        positive = "\n".join(_strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})).lower()
        if row.get("current_status") == "validated" or "fully validates" in positive or "validated_by_m026" in positive:
            errors.append(f"{rid} must remain active/out-of-scope and must not be marked validated by M026")

    for rid in EXISTING_CONTEXT_REQUIREMENT_IDS:
        row = rows.get(rid)
        if row is None:
            continue
        positive = "\n".join(_strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})).lower()
        if row.get("current_status") != "validated":
            errors.append(f"{rid} current_status must remain validated existing context")
        if "newly validates" in positive:
            errors.append(f"{rid} must not be claimed as newly validated by M026")

    row = rows.get("R036")
    if row is not None:
        positive = "\n".join(_strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})).lower()
        if row.get("current_status") == "validated" or "fully validates" in positive:
            errors.append("R036 must remain active/adjacent evidence unless full CLI provenance is separately proven")

    row = rows.get("R040")
    if row is not None:
        positive = "\n".join(_strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})).lower()
        if row.get("current_status") == "validated" or "globally validates" in positive or "all future infrastructure" in positive:
            errors.append("R040 must be milestone-local followed-constraint evidence, not global validation")

    row = rows.get("R050")
    if row is not None:
        positive = "\n".join(_strings_from({key: row.get(key) for key in POSITIVE_TEXT_KEYS})).lower()
        if row.get("current_status") == "validated" or "m026 implements r050" in positive:
            errors.append("R050 must remain a future consumer, not implemented or validated by M026")

    if reject_unsafe_claims:
        for path, text in _positive_text_for_matrix(matrix).items():
            lowered = text.lower()
            for phrase in sorted(UNSAFE_POSITIVE_CLAIM_PHRASES):
                if phrase in lowered:
                    errors.append(f"{path} contains unsafe claim phrase: {phrase}")

    for path, value in _walk(matrix):
        key_without_index = _field_name_from_path(path)
        if isinstance(value, bool) and value is True and key_without_index in UNSAFE_TRUE_BOOLEAN_KEYS:
            errors.append(f"{path} unsafe boolean field must not be true")
        if any(fragment in key_without_index for fragment in UNSAFE_FIELD_NAME_FRAGMENTS):
            if not path.startswith("$.safety_flags."):
                errors.append(f"{path} contains unsafe raw/binary/base64/vector/secret field name")

    if not rendered_markdown.strip():
        errors.append("rendered markdown is empty")
    for rid in sorted(required):
        if rid not in rendered_markdown:
            errors.append(f"rendered markdown missing requirement id: {rid}")
        expected = EXPECTED_CLASSIFICATIONS.get(rid, {})
        for key, expected_value in expected.items():
            if expected_value not in rendered_markdown:
                errors.append(f"rendered markdown missing {rid} {key}: {expected_value}")
    if "doc/validation/m026_requirement_scope_matrix.json" not in rendered_markdown:
        errors.append("rendered markdown does not reference the source matrix JSON path")

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
    parser.add_argument("--matrix", required=True, type=Path, help="Path to m026_requirement_scope_matrix.json")
    parser.add_argument("--rendered", required=True, type=Path, help="Path to rendered matrix markdown")
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--require-requirements", nargs="+", default=sorted(REQUIRED_REQUIREMENT_IDS))
    parser.add_argument(
        "--require-active-scope",
        action="store_true",
        help="Require all active/touched M026 requirement rows, classifications, and safety flags.",
    )
    parser.add_argument("--reject-unsafe-claims", action="store_true")
    parser.add_argument(
        "--require-planning-evidence",
        action="store_true",
        help="Also require .gsd/.planning/.audits evidence paths to exist; default skips these gitignored planning paths.",
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
            raise MatrixValidationError(f"rendered markdown file not found: {args.rendered}") from exc
        required = set(args.require_requirements) if args.require_active_scope else set(args.require_requirements)
        errors = validate_matrix(
            matrix,
            rendered,
            repo_root=args.repo_root,
            required_requirements=required,
            reject_unsafe_claims=args.reject_unsafe_claims,
            require_planning_evidence=args.require_planning_evidence,
        )
    except MatrixValidationError as exc:
        sys.stderr.write(f"ERROR: {exc}\n")
        return 2
    if errors:
        sys.stderr.write("M026 requirement scope reconciliation validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    sys.stdout.write(
        "M026 requirement scope reconciliation validation passed: "
        f"{len(matrix['requirements'])} requirement rows checked.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
