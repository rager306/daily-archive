#!/usr/bin/env python3
"""Redacted audit helper for candidate locator evidence artifacts.

Usage:
    uv run python scripts/audit_locator_evidence.py path/to/locator-batch.json --output audit.json
    uv run python scripts/audit_locator_evidence.py path/to/locator-batch.json --non-strict

The helper reads only the locator JSON artifact. It does not read source corpus
files named by the artifact, and its audit model contains stable IDs, counts,
distributions, validation paths/codes, and safety blockers only.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from research_graph.repair.candidate_locators import (  # noqa: E402
    CANDIDATE_LOCATOR_PROTOCOL_VERSION,
    default_safety_flags,
    find_forbidden_payload_keys,
    validate_candidate_locator_artifact,
)

M021_EXPECTED_INVARIANTS: dict[str, Any] = {
    "schema_version": CANDIDATE_LOCATOR_PROTOCOL_VERSION,
    "paper_count": 10,
    "source_count": 10,
    "locator_count": 26,
    "located_count": 26,
    "review_required_count": 0,
    "missing_span_count": 0,
    "ambiguous_span_count": 20,
    "conflicting_evidence_count": 0,
    "retrieval_only_count": 6,
    "repair_required_count": 0,
    "import_eligible_count": 0,
    "promoted_to_fact_count": 0,
}

SUMMARY_COUNT_KEYS = tuple(key for key in M021_EXPECTED_INVARIANTS if key != "schema_version")

FORBIDDEN_OUTPUT_KEYS = {
    "text",
    "raw_text",
    "chunk_text",
    "embedding",
    "vector",
    "secret",
    "token",
    "password",
    "api_key",
    "prompt",
    "response",
    "completion",
}


class LocatorEvidenceAuditError(ValueError):
    """Raised when locator evidence fails redacted audit checks."""


def load_locator_artifact(path: str | Path) -> dict[str, Any]:
    """Load one JSON locator artifact with clear file and JSON errors."""
    input_path = Path(path)
    if not input_path.exists():
        raise FileNotFoundError(f"locator evidence input file not found: {input_path}")
    try:
        payload = json.loads(input_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise LocatorEvidenceAuditError(
            f"malformed locator evidence JSON at {input_path}: line {exc.lineno} column {exc.colno}"
        ) from exc
    if not isinstance(payload, dict):
        raise LocatorEvidenceAuditError(f"locator evidence root must be a JSON object: {input_path}")
    return payload


def audit_locator_evidence(
    artifact: dict[str, Any],
    *,
    strict: bool = True,
    expected_invariants: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Return a deterministic redacted audit model for one locator artifact.

    Strict mode compares first-proof invariants to ``expected_invariants``. When
    omitted, strict mode uses the M021 deterministic locator batch invariants.
    """
    expected = M021_EXPECTED_INVARIANTS if expected_invariants is None else expected_invariants
    validation_diagnostics = sorted(validate_candidate_locator_artifact(artifact))
    forbidden_payload_key_paths = sorted(find_forbidden_payload_keys(artifact))
    if validation_diagnostics:
        raise LocatorEvidenceAuditError(
            "candidate locator validator reported errors: " + ", ".join(validation_diagnostics)
        )
    if forbidden_payload_key_paths:
        raise LocatorEvidenceAuditError(
            "forbidden payload keys present: " + ", ".join(forbidden_payload_key_paths)
        )

    locators = _require_list(artifact, "locators")
    source_ledger = _require_list(artifact, "source_ledger")
    summary = artifact.get("summary") if isinstance(artifact.get("summary"), dict) else {}

    first_proof_invariants = _first_proof_invariants(artifact, source_ledger, locators)
    summary_drift = _summary_drift(summary, first_proof_invariants)
    unsafe_safety_flag_paths = _unsafe_safety_flag_paths(artifact)
    missing_span_locator_ids = sorted(
        str(locator.get("locator_id", "unknown"))
        for locator in locators
        if not isinstance(locator.get("source_spans"), list) or not locator.get("source_spans")
    )
    invariant_drift = _invariant_drift(first_proof_invariants, expected) if strict else []

    failures = [*summary_drift, *unsafe_safety_flag_paths, *missing_span_locator_ids, *invariant_drift]
    if failures:
        labels: list[str] = []
        if summary_drift:
            labels.append("summary drift: " + "; ".join(summary_drift))
        if unsafe_safety_flag_paths:
            labels.append("unsafe safety flags: " + ", ".join(unsafe_safety_flag_paths))
        if missing_span_locator_ids:
            labels.append("locators missing source spans: " + ", ".join(missing_span_locator_ids))
        if invariant_drift:
            labels.append("invariant drift: " + "; ".join(invariant_drift))
        raise LocatorEvidenceAuditError("; ".join(labels))

    spans = [span for locator in locators for span in locator.get("source_spans", []) if isinstance(span, dict)]
    audit = {
        "schema_version": "locator_evidence_audit.v1",
        "input_schema_version": artifact.get("schema_version"),
        "strict": strict,
        "first_proof_invariants": first_proof_invariants,
        "stable_ids": {
            "source_ids": sorted(str(source.get("source_id", "unknown")) for source in source_ledger),
            "locator_ids": sorted(str(locator.get("locator_id", "unknown")) for locator in locators),
            "span_ids": sorted(str(span.get("span_id", "unknown")) for span in spans),
        },
        "distributions": _distributions(locators),
        "diagnostic_code_classes": _diagnostic_code_classes(locators),
        "source_span_coverage": _source_span_coverage(locators, spans),
        "source_ledger_safety": _source_ledger_safety(source_ledger),
        "repair_context_gaps": _repair_context_gaps(locators, spans),
        "safety_blockers": {
            "validator_diagnostics": validation_diagnostics,
            "forbidden_payload_key_paths": forbidden_payload_key_paths,
            "unsafe_safety_flag_paths": unsafe_safety_flag_paths,
            "summary_drift": summary_drift,
            "invariant_drift": invariant_drift,
            "no_import_blocker_intact": first_proof_invariants["import_eligible_count"] == 0
            and first_proof_invariants["promoted_to_fact_count"] == 0,
        },
    }
    _assert_audit_is_redacted(audit)
    return audit


def audit_locator_evidence_file(
    input_path: str | Path,
    *,
    output_path: str | Path | None = None,
    json_output_path: str | Path | None = None,
    markdown_output_path: str | Path | None = None,
    strict: bool = True,
    expected_invariants: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Load, audit, and optionally write sorted JSON/Markdown after all checks pass."""
    audit = audit_locator_evidence(
        load_locator_artifact(input_path), strict=strict, expected_invariants=expected_invariants
    )
    audit = {"input_path": str(Path(input_path)), **audit}
    json_destination = json_output_path if json_output_path is not None else output_path
    if json_destination is not None:
        destination = Path(json_destination)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(audit, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if markdown_output_path is not None:
        destination = Path(markdown_output_path)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(render_locator_evidence_audit_markdown(audit), encoding="utf-8")
    return audit


def render_locator_evidence_audit_markdown(audit: dict[str, Any]) -> str:
    """Render a reviewer-facing Markdown audit without source snippets or payload text."""
    invariants = audit["first_proof_invariants"]
    distributions = audit["distributions"]
    coverage = audit["source_span_coverage"]
    ledger = audit["source_ledger_safety"]
    gaps = audit["repair_context_gaps"]
    blockers = audit["safety_blockers"]
    explicit_blockers = [
        "Positive KG import is blocked: import_eligible_count=0 and promoted_to_fact_count=0.",
        "Production LadybugDB writes are blocked by candidate locator safety flags.",
        "Semantic readiness claims are blocked; this audit records locator evidence only.",
        "Embeddings and vectors are blocked; no vector payloads are imported or serialized.",
        "Source snippets, chunk payloads, and corpus text are blocked from this audit surface.",
        "DSPy, MiniMax, converter, and optimizer activation are blocked for this slice.",
        "Broad scaling is blocked until S02 repair-context contracts consume this bounded set.",
    ]
    lines: list[str] = [
        "# S01 Locator Evidence Audit",
        "",
        "This reviewer-facing audit summarizes the bounded M021 deterministic locator evidence set. It reports stable IDs, counts, distributions, span coverage, lineage gaps, and safety blockers only; it does not include snippets or source payload text.",
        "",
        "## Pinned Input",
        "",
        f"- Input path: `{audit['input_path']}`",
        f"- Audit schema version: `{audit['schema_version']}`",
        f"- Locator input schema version: `{audit['input_schema_version']}`",
        f"- Strict mode: `{audit['strict']}`",
        "",
        "## Expected First-Proof Invariants",
        "",
        "| Invariant | Observed |",
        "|---|---:|",
    ]
    for key, value in invariants.items():
        lines.append(f"| `{key}` | `{value}` |")
    lines.extend([
        "",
        "## Per-Target Summary",
        "",
        f"- Papers: `{invariants['paper_count']}`",
        f"- Sources: `{invariants['source_count']}`",
        f"- Locators: `{invariants['locator_count']}`",
        f"- Located locators: `{invariants['located_count']}`",
        f"- Review-required locators: `{invariants['review_required_count']}`",
        f"- Retrieval-only locators: `{invariants['retrieval_only_count']}`",
        f"- Ambiguous-span locators: `{invariants['ambiguous_span_count']}`",
        f"- Import-eligible locators: `{invariants['import_eligible_count']}`",
        "",
        "## Ambiguity Taxonomy",
        "",
        "### Routes",
        *(_bullet_counts(distributions["routes"])),
        "",
        "### States",
        *(_bullet_counts(distributions["states"])),
        "",
        "### Diagnostic Code Classes",
        *(_bullet_counts(audit["diagnostic_code_classes"])),
        "",
        "### Diagnostic Codes",
        *(_bullet_counts(distributions["diagnostic_codes"])),
        "",
        "## Source-Span Coordinate and Hash Coverage",
        "",
        f"- Locator count: `{coverage['locator_count']}`",
        f"- Locators with source spans: `{coverage['locators_with_source_spans']}`",
        f"- Locators without source spans: `{coverage['locators_without_source_spans']}`",
        f"- Span count: `{coverage['span_count']}`",
        f"- Spans with hashes: `{coverage['spans_with_hash']}`",
        f"- Coordinate spans with character bounds: `{coverage['coordinate_spans_with_char_bounds']}`",
        f"- Coordinate spans with line bounds: `{coverage['coordinate_spans_with_line_bounds']}`",
        f"- Artifact-record span count: `{coverage['artifact_record_span_count']}`",
        "",
        "### Coordinate Spaces",
        *(_bullet_counts(coverage["coordinate_space_distribution"])),
        "",
        "## Source Ledger Safety Summary",
        "",
        f"- Source ledger entries: `{ledger['source_count']}`",
        f"- Sources with hashes: `{ledger['sources_with_hash']}`",
        f"- Source text embedded non-false paths: `{len(ledger['source_text_embedded_nonfalse_paths'])}`",
        f"- Source binary embedded non-false paths: `{len(ledger['source_binary_embedded_nonfalse_paths'])}`",
        "",
        "### Conversion Statuses",
        *(_bullet_counts(ledger["conversion_status_distribution"])),
        "",
        "### Source Hash Algorithms",
        *(_bullet_counts(ledger["source_hash_algorithm_distribution"])),
        "",
        "## S02 Repair-Context Gaps",
        "",
        f"- Missing-span locator IDs: `{len(gaps['missing_span_locator_ids'])}`",
        f"- Repair-required locator IDs: `{len(gaps['repair_required_locator_ids'])}`",
        f"- Conflicting-evidence locator IDs: `{len(gaps['conflicting_evidence_locator_ids'])}`",
        f"- Artifact-record span IDs: `{len(gaps['artifact_record_span_ids'])}`",
        "",
        "## Safety Blockers",
        "",
        f"- Validator diagnostics: `{len(blockers['validator_diagnostics'])}`",
        f"- Forbidden payload key paths: `{len(blockers['forbidden_payload_key_paths'])}`",
        f"- Unsafe safety flag paths: `{len(blockers['unsafe_safety_flag_paths'])}`",
        f"- Summary drift entries: `{len(blockers['summary_drift'])}`",
        f"- Invariant drift entries: `{len(blockers['invariant_drift'])}`",
        f"- No-import blocker intact: `{blockers['no_import_blocker_intact']}`",
        "",
        "### Explicit No-Go Constraints",
    ])
    lines.extend(f"- {blocker}" for blocker in explicit_blockers)
    lines.append("")
    return "\n".join(lines)


def _bullet_counts(values: dict[str, int]) -> list[str]:
    if not values:
        return ["- None: `0`"]
    return [f"- `{key}`: `{value}`" for key, value in sorted(values.items())]


def _require_list(artifact: dict[str, Any], key: str) -> list[dict[str, Any]]:
    value = artifact.get(key)
    if not isinstance(value, list):
        raise LocatorEvidenceAuditError(f"locator evidence key must be a list: /{key}")
    return [item for item in value if isinstance(item, dict)]


def _first_proof_invariants(
    artifact: dict[str, Any], source_ledger: list[dict[str, Any]], locators: list[dict[str, Any]]
) -> dict[str, Any]:
    states = Counter(str(locator.get("state", "unknown")) for locator in locators)
    return {
        "schema_version": artifact.get("schema_version"),
        "paper_count": len({str(locator.get("paper_id", "unknown")) for locator in locators}),
        "source_count": len(source_ledger),
        "locator_count": len(locators),
        "located_count": sum(1 for locator in locators if locator.get("state") != "missing_span"),
        "review_required_count": states["review_required"],
        "missing_span_count": states["missing_span"],
        "ambiguous_span_count": states["ambiguous_span"],
        "conflicting_evidence_count": states["conflicting_evidence"],
        "retrieval_only_count": states["retrieval_only"],
        "repair_required_count": states["repair_required"],
        "import_eligible_count": sum(1 for locator in locators if locator.get("import_eligible") is True),
        "promoted_to_fact_count": sum(1 for locator in locators if locator.get("promoted_to_fact") is True),
    }


def _summary_drift(summary: dict[str, Any], observed: dict[str, Any]) -> list[str]:
    drift = []
    for key in SUMMARY_COUNT_KEYS:
        if key in summary and summary.get(key) != observed.get(key):
            drift.append(f"/{key}:summary={summary.get(key)!r}:observed={observed.get(key)!r}")
    return sorted(drift)


def _invariant_drift(observed: dict[str, Any], expected: dict[str, Any]) -> list[str]:
    drift = []
    for key, expected_value in sorted(expected.items()):
        if observed.get(key) != expected_value:
            drift.append(f"/{key}:expected={expected_value!r}:observed={observed.get(key)!r}")
    return drift


def _unsafe_safety_flag_paths(artifact: dict[str, Any]) -> list[str]:
    safety_flags = artifact.get("safety_flags")
    if not isinstance(safety_flags, dict):
        return ["/safety_flags"]
    unsafe = []
    for key, expected in default_safety_flags().items():
        if safety_flags.get(key) is not expected:
            unsafe.append(f"/safety_flags/{key}")
    return sorted(unsafe)


def _distributions(locators: list[dict[str, Any]]) -> dict[str, dict[str, int]]:
    return {
        "routes": _counter_dict(locator.get("route") for locator in locators),
        "states": _counter_dict(locator.get("state") for locator in locators),
        "candidate_types": _counter_dict(locator.get("candidate_type") for locator in locators),
        "review_queue_reasons": _counter_dict(locator.get("review_queue_reason") for locator in locators),
        "support_levels": _counter_dict(locator.get("support_level") for locator in locators),
        "uncertainty_labels": _counter_dict(locator.get("uncertainty_label") for locator in locators),
        "diagnostic_codes": _counter_dict(
            code for locator in locators for code in locator.get("diagnostic_codes", [])
        ),
    }


def _diagnostic_code_classes(locators: list[dict[str, Any]]) -> dict[str, int]:
    classes: Counter[str] = Counter()
    for locator in locators:
        codes = locator.get("diagnostic_codes", [])
        if not codes:
            classes["none"] += 1
        for code in codes:
            code_text = str(code)
            if code_text.startswith("source_"):
                classes["source_ledger"] += 1
            elif "missing" in code_text:
                classes["missing_span"] += 1
            elif "broad" in code_text or "overlap" in code_text or "ambiguous" in code_text:
                classes["ambiguous_span"] += 1
            elif "conflict" in code_text or "contradict" in code_text:
                classes["conflicting_evidence"] += 1
            elif "review" in code_text:
                classes["review_required"] += 1
            else:
                classes["other"] += 1
    return dict(sorted(classes.items()))


def _source_span_coverage(locators: list[dict[str, Any]], spans: list[dict[str, Any]]) -> dict[str, Any]:
    coordinate_spans = [span for span in spans if span.get("coordinate_space") != "artifact_record"]
    return {
        "locator_count": len(locators),
        "locators_with_source_spans": sum(1 for locator in locators if locator.get("source_spans")),
        "locators_without_source_spans": sum(1 for locator in locators if not locator.get("source_spans")),
        "span_count": len(spans),
        "coordinate_space_distribution": _counter_dict(span.get("coordinate_space") for span in spans),
        "spans_with_hash": sum(1 for span in spans if isinstance(span.get("span_hash"), str) and bool(span.get("span_hash"))),
        "coordinate_spans_with_char_bounds": sum(
            1 for span in coordinate_spans if isinstance(span.get("char_start"), int) and isinstance(span.get("char_end"), int)
        ),
        "coordinate_spans_with_line_bounds": sum(
            1 for span in coordinate_spans if isinstance(span.get("line_start"), int) and isinstance(span.get("line_end"), int)
        ),
        "artifact_record_span_count": sum(1 for span in spans if span.get("coordinate_space") == "artifact_record"),
    }


def _source_ledger_safety(source_ledger: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "source_count": len(source_ledger),
        "conversion_status_distribution": _counter_dict(source.get("conversion_status") for source in source_ledger),
        "source_hash_algorithm_distribution": _counter_dict(source.get("source_hash_algorithm") for source in source_ledger),
        "sources_with_hash": sum(1 for source in source_ledger if source.get("source_hash") not in {None, "", "missing"}),
        "source_text_embedded_nonfalse_paths": sorted(
            f"/source_ledger[{index}]/raw_text_embedded"
            for index, source in enumerate(source_ledger)
            if source.get("raw_text_embedded") is not False
        ),
        "source_binary_embedded_nonfalse_paths": sorted(
            f"/source_ledger[{index}]/raw_binary_embedded"
            for index, source in enumerate(source_ledger)
            if source.get("raw_binary_embedded") is not False
        ),
    }


def _repair_context_gaps(locators: list[dict[str, Any]], spans: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "missing_span_locator_ids": sorted(
            str(locator.get("locator_id", "unknown"))
            for locator in locators
            if locator.get("state") == "missing_span"
        ),
        "repair_required_locator_ids": sorted(
            str(locator.get("locator_id", "unknown"))
            for locator in locators
            if locator.get("state") == "repair_required"
        ),
        "conflicting_evidence_locator_ids": sorted(
            str(locator.get("locator_id", "unknown"))
            for locator in locators
            if locator.get("state") == "conflicting_evidence"
        ),
        "artifact_record_span_ids": sorted(
            str(span.get("span_id", "unknown"))
            for span in spans
            if span.get("coordinate_space") == "artifact_record"
        ),
    }


def _counter_dict(values: Any) -> dict[str, int]:
    return dict(sorted(Counter(str(value) for value in values).items()))


def _assert_audit_is_redacted(value: Any, path: str = "") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = f"{path}/{key}" if path else f"/{key}"
            if key in FORBIDDEN_OUTPUT_KEYS:
                raise LocatorEvidenceAuditError(f"audit output would serialize forbidden key: {child_path}")
            _assert_audit_is_redacted(child, child_path)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _assert_audit_is_redacted(child, f"{path}[{index}]")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", nargs="?", type=Path, help="Candidate locator artifact JSON to audit")
    parser.add_argument("--input", dest="input_option", type=Path, help="Candidate locator artifact JSON to audit")
    parser.add_argument("--output", type=Path, help="Legacy alias for --json-output")
    parser.add_argument("--json-output", type=Path, help="Optional path for the redacted audit JSON")
    parser.add_argument("--markdown-output", type=Path, help="Optional path for the reviewer-facing Markdown audit")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--strict",
        action="store_true",
        default=True,
        help="Enforce M021 invariant comparisons while preserving schema/safety validation (default)",
    )
    mode.add_argument(
        "--non-strict",
        action="store_true",
        help="Skip M021 invariant comparisons while preserving schema/safety validation",
    )
    args = parser.parse_args(argv)
    input_path = args.input_option if args.input_option is not None else args.input
    if input_path is None:
        parser.error("locator evidence input path is required")
    try:
        audit = audit_locator_evidence_file(
            input_path,
            output_path=args.output,
            json_output_path=args.json_output,
            markdown_output_path=args.markdown_output,
            strict=not args.non_strict,
        )
    except (FileNotFoundError, LocatorEvidenceAuditError) as exc:
        sys.stderr.write(f"locator evidence audit failed: {exc}\n")
        return 2
    if args.output is None and args.json_output is None:
        sys.stdout.write(f"{json.dumps(audit, indent=2, sort_keys=True)}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
