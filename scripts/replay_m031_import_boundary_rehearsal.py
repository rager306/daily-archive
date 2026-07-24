#!/usr/bin/env python3
"""Replay the M031 refusal-only import-boundary rehearsal.

This CLI consumes local S04 chunk/evidence artifacts and writes redacted S05
import-boundary rehearsal surfaces only after all preflight and contract checks
pass. It never reads raw article payloads, performs network fetches, writes graph
state, or touches LadybugDB.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

from research_graph.infrastructure.staging.import_boundary import (
    build_import_boundary_rehearsal,
    validate_import_boundary_rehearsal,
)

CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")
CHUNK_EVIDENCE_DIR = CORPUS_DIR / "chunk-evidence"
DEFAULT_CLOSEOUT_SUMMARY = CORPUS_DIR / "chunk-evidence-closeout-summary.json"
DEFAULT_CHUNK_EVIDENCE_SUMMARY = CHUNK_EVIDENCE_DIR / "chunk-evidence-summary.json"
DEFAULT_STRUCTURE_AWARE_PACKAGE = (
    CHUNK_EVIDENCE_DIR
    / "packages"
    / "arxiv_cs-cl_2507.19457_arxiv_pdf"
    / "structure-aware-package.json"
)
DEFAULT_GRAPH_READINESS_PACKAGE = (
    CHUNK_EVIDENCE_DIR
    / "packages"
    / "arxiv_cs-cl_2507.19457_arxiv_pdf"
    / "graph-readiness-package.json"
)
DEFAULT_REVIEW_EVENTS = CHUNK_EVIDENCE_DIR / "independent-review-events.jsonl"
DEFAULT_OUTPUT_DIR = CORPUS_DIR / "import-boundary-rehearsal"

REFUSAL_DIAGNOSTIC_CODE = "M031_IMPORT_BOUNDARY_REFUSED"
FAIL_CLOSED_FLAGS = (
    "raw_text_included",
    "chunk_text_included",
    "raw_binary_included",
    "base64_included",
    "embeddings_included",
    "vectors_included",
    "secrets_included",
    "optimizer_traces_included",
    "network_fetch_attempted",
    "raw_payload_embedded_in_metadata",
    "chunk_ready_claimed_for_non_parser_ready_rows",
    "graph_import_allowed",
    "trusted_kg_import_allowed",
    "kg_readiness_claimed",
    "graph_write_attempted",
    "ladybugdb_written",
    "production_ladybugdb_write_allowed",
    "production_persistence_attempted",
    "production_import_attempted",
)


class RehearsalReplayError(RuntimeError):
    """Fail-closed replay error with a deterministic diagnostic code."""

    def __init__(self, code: str, message: str, *, json_path: str = "$") -> None:
        super().__init__(f"{code} {json_path}: {message}")
        self.code = code
        self.json_path = json_path
        self.message = message


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--closeout-summary", type=Path, default=DEFAULT_CLOSEOUT_SUMMARY)
    parser.add_argument("--summary", type=Path, default=DEFAULT_CHUNK_EVIDENCE_SUMMARY)
    parser.add_argument(
        "--structure-aware-package", type=Path, default=DEFAULT_STRUCTURE_AWARE_PACKAGE
    )
    parser.add_argument(
        "--graph-readiness-package", type=Path, default=DEFAULT_GRAPH_READINESS_PACKAGE
    )
    parser.add_argument("--independent-review-events", type=Path, default=DEFAULT_REVIEW_EVENTS)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        run_replay(
            closeout_summary_path=args.closeout_summary,
            summary_path=args.summary,
            structure_aware_package_path=args.structure_aware_package,
            graph_readiness_package_path=args.graph_readiness_package,
            independent_review_events_path=args.independent_review_events,
            output_dir=args.output_dir,
        )
    except RehearsalReplayError as exc:
        sys.stderr.write(f"{exc}\n")
        return 2
    return 0


def run_replay(
    *,
    closeout_summary_path: Path,
    summary_path: Path,
    structure_aware_package_path: Path,
    graph_readiness_package_path: Path,
    independent_review_events_path: Path,
    output_dir: Path,
) -> dict[str, Path]:
    """Build, validate, and write the M031 import-boundary rehearsal artifacts."""
    _assert_existing_json_path(closeout_summary_path, code="M031_CLOSEOUT_PATH_UNSAFE")
    _assert_existing_json_path(summary_path, code="M031_SUMMARY_PATH_UNSAFE")
    _assert_existing_json_path(
        structure_aware_package_path, code="M031_STRUCTURE_PACKAGE_PATH_UNSAFE"
    )
    _assert_existing_json_path(graph_readiness_package_path, code="M031_GRAPH_PACKAGE_PATH_UNSAFE")
    _assert_existing_jsonl_path(
        independent_review_events_path, code="M031_REVIEW_EVENTS_PATH_UNSAFE"
    )

    closeout_summary = _read_json_object(closeout_summary_path)
    summary = _read_json_object(summary_path)
    structure_package = _read_json_object(structure_aware_package_path)
    graph_package = _read_json_object(graph_readiness_package_path)
    review_events = _read_jsonl_objects(independent_review_events_path)

    _validate_preconditions(
        closeout_summary=closeout_summary,
        summary=summary,
        structure_package=structure_package,
        graph_package=graph_package,
        review_events=review_events,
    )

    contract = build_import_boundary_rehearsal(
        summary_path=summary_path,
        closeout_summary_path=closeout_summary_path,
        graph_readiness_package_paths=[graph_readiness_package_path],
        independent_review_events_path=independent_review_events_path,
    )
    validation = validate_import_boundary_rehearsal(contract)
    if not validation.valid_rehearsal:
        reasons = ",".join(validation.refusal_counts)
        raise RehearsalReplayError(
            "M031_IMPORT_BOUNDARY_CONTRACT_INVALID", f"contract validation failed: {reasons}"
        )

    _validate_contract_counts(contract=contract, closeout_summary=closeout_summary, summary=summary)
    summary_record = _summary_record(
        contract, validation_diagnostic_count=len(validation.diagnostics)
    )
    diagnostic_records = _diagnostic_records(contract)
    report = _render_report(summary_record=summary_record, diagnostics=diagnostic_records)

    output_dir.mkdir(parents=True, exist_ok=True)
    summary_path_out = output_dir / "import-boundary-summary.json"
    diagnostics_path_out = output_dir / "import-boundary-diagnostics.jsonl"
    report_path_out = output_dir / "import-boundary-report.md"
    summary_path_out.write_text(
        json.dumps(summary_record, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    diagnostics_path_out.write_text(
        "".join(
            json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n"
            for record in diagnostic_records
        ),
        encoding="utf-8",
    )
    report_path_out.write_text(report, encoding="utf-8")
    sys.stdout.write(
        "M031 import-boundary rehearsal generated and validated: "
        f"summary={summary_path_out.as_posix()} "
        f"diagnostics={diagnostics_path_out.as_posix()} "
        f"report={report_path_out.as_posix()}\n"
    )
    return {
        "summary_path": summary_path_out,
        "diagnostics_path": diagnostics_path_out,
        "report_path": report_path_out,
    }


def _validate_preconditions(
    *,
    closeout_summary: dict[str, Any],
    summary: dict[str, Any],
    structure_package: dict[str, Any],
    graph_package: dict[str, Any],
    review_events: list[dict[str, Any]],
) -> None:
    if closeout_summary.get("status") != "passed":
        raise RehearsalReplayError(
            "M031_CLOSEOUT_NOT_PASSED", "S04 closeout status must be passed", json_path="$.status"
        )
    if not review_events:
        raise RehearsalReplayError(
            "M031_REVIEW_EVENTS_ABSENT", "independent-review events must be present"
        )
    event_names = {str(event.get("event")) for event in review_events}
    if "independent_review.requested" not in event_names:
        raise RehearsalReplayError(
            "M031_REVIEW_REQUEST_EVENT_ABSENT",
            "independent_review.requested event is required",
            json_path="$.independent_review_events",
        )
    if summary.get("row_count") != closeout_summary.get("row_count"):
        raise RehearsalReplayError(
            "M031_ROW_COUNT_DRIFT",
            "summary and closeout row counts differ",
            json_path="$.row_count",
        )
    if summary.get("parser_ready_row_count") != closeout_summary.get("parser_ready_row_count"):
        raise RehearsalReplayError(
            "M031_PARSER_READY_COUNT_DRIFT",
            "summary and closeout parser-ready counts differ",
            json_path="$.parser_ready_row_count",
        )
    if structure_package.get("paper_id") != graph_package.get("package_key"):
        raise RehearsalReplayError(
            "M031_PACKAGE_ID_DRIFT",
            "structure-aware and graph-readiness package IDs differ",
            json_path="$.package_key",
        )
    for label, payload in (
        ("closeout_summary", closeout_summary),
        ("chunk_evidence_summary", summary),
        ("graph_readiness_package", graph_package),
    ):
        _assert_fail_closed_flags(payload, label=label)


def _validate_contract_counts(
    *, contract: dict[str, Any], closeout_summary: dict[str, Any], summary: dict[str, Any]
) -> None:
    if contract.get("candidate_count") != closeout_summary.get("row_count"):
        raise RehearsalReplayError(
            "M031_CANDIDATE_COUNT_DRIFT",
            "candidate count must match S04 row count",
            json_path="$.candidate_count",
        )
    if contract.get("candidate_count") != len(contract.get("candidates", [])):
        raise RehearsalReplayError(
            "M031_CANDIDATE_COUNT_DRIFT",
            "candidate count must match candidates length",
            json_path="$.candidates",
        )
    import_eligible_count = sum(
        1
        for candidate in contract.get("candidates", [])
        if candidate.get("import_eligible") is True
    )
    if contract.get("accepted_count") != 0 or import_eligible_count != 0:
        raise RehearsalReplayError(
            "M031_STRUCTURAL_LABEL_MISINTERPRETED_AS_APPROVAL",
            "refusal-only rehearsal must not accept or mark candidates import-eligible",
            json_path="$.accepted_count",
        )
    if contract.get("source_import_boundary_summary", {}).get("zero_chunk_refusal_count") != summary.get(
        "zero_chunk_refusal_count"
    ):
        raise RehearsalReplayError(
            "M031_ZERO_CHUNK_REFUSAL_COUNT_DRIFT",
            "contract and summary zero-chunk refusal counts differ",
            json_path="$.source_import_boundary_summary.zero_chunk_refusal_count",
        )


def _summary_record(
    contract: dict[str, Any], *, validation_diagnostic_count: int
) -> dict[str, Any]:
    candidate_records = list(contract.get("candidates", []))
    import_eligible_count = sum(
        1 for candidate in candidate_records if candidate.get("import_eligible") is True
    )
    summary = {key: value for key, value in contract.items() if key != "candidates"}
    summary.update(
        {
            "valid_rehearsal": True,
            "validation_diagnostic_count": validation_diagnostic_count,
            "import_eligible_count": import_eligible_count,
            "diagnostic_code_counts": {REFUSAL_DIAGNOSTIC_CODE: len(candidate_records)},
            "artifact_kind": "refusal_only_import_boundary_rehearsal",
        }
    )
    return summary


def _diagnostic_records(contract: dict[str, Any]) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for index, candidate in enumerate(contract.get("candidates", [])):
        refusal_reasons = [str(reason) for reason in candidate.get("refusal_reasons", [])]
        records.append(
            {
                "diagnostic_code": REFUSAL_DIAGNOSTIC_CODE,
                "json_path": f"$.candidates[{index}]",
                "candidate_id": candidate.get("candidate_id"),
                "candidate_type": candidate.get("candidate_type"),
                "package_id": candidate.get("package_id"),
                "source_json_path": candidate.get("source_json_path"),
                "route": candidate.get("route"),
                "state": candidate.get("state"),
                "accepted": candidate.get("accepted"),
                "rejected": candidate.get("rejected"),
                "import_eligible": candidate.get("import_eligible"),
                "blocks_import": True,
                "refusal_reasons": refusal_reasons,
                "review_state": candidate.get("review_state"),
                "output_contract_completed": candidate.get("output_contract_completed"),
                "independent_review_completed": candidate.get("independent_review_completed"),
                "fail_closed_flags": {
                    flag: candidate.get(flag, False)
                    for flag in FAIL_CLOSED_FLAGS
                    if flag in candidate
                },
            }
        )
    return records


def _render_report(*, summary_record: dict[str, Any], diagnostics: list[dict[str, Any]]) -> str:
    refusal_counts = summary_record.get("refusal_counts", {})
    diagnostic_counts = Counter(record["diagnostic_code"] for record in diagnostics)
    lines = [
        "# M031 Import Boundary Rehearsal Report",
        "",
        "This is a refusal-only, no-write rehearsal. It is not graph or LadybugDB readiness approval.",
        "",
        "## Summary",
        f"- rehearsal_id: `{summary_record.get('rehearsal_id')}`",
        f"- source_benchmark_id: `{summary_record.get('source_benchmark_id')}`",
        f"- candidates: {summary_record.get('candidate_count')}",
        f"- accepted/import-eligible candidates: {summary_record.get('accepted_count')}/{summary_record.get('import_eligible_count')}",
        f"- rejected candidates: {summary_record.get('rejected_count')}",
        "",
        "## Fail-Closed Flags",
        f"- trusted KG import allowed: {str(summary_record.get('trusted_kg_import_allowed')).lower()}",
        f"- graph import allowed: {str(summary_record.get('graph_import_allowed')).lower()}",
        f"- production import attempted: {str(summary_record.get('production_import_attempted')).lower()}",
        f"- LadybugDB writes: {str(summary_record.get('ladybugdb_written')).lower()}",
        f"- network fetch attempted: {str(summary_record.get('network_fetch_attempted')).lower()}",
        "",
        "## Diagnostic Codes",
    ]
    for code, count in sorted(diagnostic_counts.items()):
        lines.append(f"- `{code}`: {count}")
    lines.extend(["", "## Refusal Counts"])
    for reason, count in sorted(refusal_counts.items()):
        lines.append(f"- `{reason}`: {count}")
    lines.extend(
        [
            "",
            "## Candidate Matrix",
            "| JSON Path | Package | Type | Route | State | Refusal Reasons |",
            "|---|---|---|---|---|---|",
        ]
    )
    for record in diagnostics:
        lines.append(
            "| {json_path} | `{package_id}` | `{candidate_type}` | `{route}` | `{state}` | `{reasons}` |".format(
                json_path=record["json_path"],
                package_id=record.get("package_id"),
                candidate_type=record.get("candidate_type"),
                route=record.get("route"),
                state=record.get("state"),
                reasons=", ".join(record.get("refusal_reasons", [])),
            )
        )
    lines.extend(
        [
            "",
            "No raw text, chunk text, PDF bytes, HTML, embeddings, vectors, secrets, model traces, optimizer traces, external fetch state, graph writes, or LadybugDB writes are included.",
            "",
        ]
    )
    return "\n".join(lines)


def _assert_existing_json_path(path: Path, *, code: str) -> None:
    _assert_existing_path(path, code=code)
    if path.suffix != ".json":
        raise RehearsalReplayError(code, "expected a .json path", json_path=str(path))


def _assert_existing_jsonl_path(path: Path, *, code: str) -> None:
    _assert_existing_path(path, code=code)
    if path.suffix != ".jsonl":
        raise RehearsalReplayError(code, "expected a .jsonl path", json_path=str(path))


def _assert_existing_path(path: Path, *, code: str) -> None:
    if ".." in path.parts:
        raise RehearsalReplayError(code, "path traversal is not allowed", json_path=str(path))
    if not path.exists() or not path.is_file():
        raise RehearsalReplayError(code, "input path does not exist", json_path=str(path))


def _assert_fail_closed_flags(payload: dict[str, Any], *, label: str) -> None:
    nested_flags = payload.get("fail_closed_safety_flags")
    for flag in FAIL_CLOSED_FLAGS:
        if payload.get(flag) is True:
            raise RehearsalReplayError(
                "M031_FAIL_CLOSED_FLAG_TRUE",
                f"{label}.{flag} must remain false",
                json_path=f"$.{flag}",
            )
        if isinstance(nested_flags, dict) and nested_flags.get(flag) is True:
            raise RehearsalReplayError(
                "M031_FAIL_CLOSED_FLAG_TRUE",
                f"{label}.fail_closed_safety_flags.{flag} must remain false",
                json_path=f"$.fail_closed_safety_flags.{flag}",
            )


def _read_json_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RehearsalReplayError("M031_MALFORMED_JSON", exc.msg, json_path=str(path)) from exc
    if not isinstance(value, dict):
        raise RehearsalReplayError(
            "M031_MALFORMED_JSON", "expected JSON object", json_path=str(path)
        )
    return value


def _read_jsonl_objects(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError as exc:
        raise RehearsalReplayError(
            "M031_MALFORMED_JSONL", "JSONL must be UTF-8", json_path=str(path)
        ) from exc
    for line_number, line in enumerate(lines, start=1):
        stripped = line.strip()
        if not stripped:
            continue
        try:
            value = json.loads(stripped)
        except json.JSONDecodeError as exc:
            raise RehearsalReplayError(
                "M031_MALFORMED_JSONL", exc.msg, json_path=f"{path}:{line_number}"
            ) from exc
        if not isinstance(value, dict):
            raise RehearsalReplayError(
                "M031_MALFORMED_JSONL", "expected JSON object", json_path=f"{path}:{line_number}"
            )
        records.append(value)
    return records


if __name__ == "__main__":
    raise SystemExit(main())
