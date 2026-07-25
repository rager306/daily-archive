#!/usr/bin/env python3
"""Verify and write the M031 S05 closeout regression artifacts.

This closeout verifier is local-only and fail-closed. It consumes the S04
chunk/evidence closeout plus S05 import-boundary rehearsal and continuity audit
artifacts, validates that graph/import/LadybugDB boundaries remain refused, and
writes operator-facing closeout surfaces only after every check passes.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S05"
SELECTION_ID = "m031-catalog-backed-replay-v1"
CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")

DEFAULT_S04_CLOSEOUT = CORPUS_DIR / "chunk-evidence-closeout-summary.json"
DEFAULT_IMPORT_SUMMARY = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-summary.json"
DEFAULT_IMPORT_DIAGNOSTICS = (
    CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-diagnostics.jsonl"
)
DEFAULT_IMPORT_REPORT = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-report.md"
DEFAULT_MATRIX_JSON = CORPUS_DIR / "progression-matrix.json"
DEFAULT_MATRIX_MD = CORPUS_DIR / "progression-matrix.md"
DEFAULT_AUDIT_JSON = CORPUS_DIR / "m031-continuity-audit.json"
DEFAULT_AUDIT_MD = CORPUS_DIR / "m031-continuity-audit.md"
DEFAULT_REVIEW_EVENTS = CORPUS_DIR / "chunk-evidence" / "independent-review-events.jsonl"
DEFAULT_SUMMARY_OUT = CORPUS_DIR / "s05-closeout-summary.json"
DEFAULT_DIAGNOSTICS_OUT = CORPUS_DIR / "s05-closeout-diagnostics.jsonl"
DEFAULT_REPORT_OUT = CORPUS_DIR / "s05-closeout-report.md"

EXPECTED_ROW_COUNT = 7
EXPECTED_STAGE_COUNT = 8
EXPECTED_REQUIRED_SECTIONS = (
    "## Stage Owners, Evidence, Verifiers, and Failure Modes",
    "## Unsafe Claims to Preserve",
    "## Fail-Closed Flags",
    "## Structural Route Label Notice",
    "## Failure Modes",
    "## Load Profile",
    "## Negative Tests",
    "## Import Boundary Checkpoint",
)
EXPECTED_FALSE_FLAGS = {
    "base64_included",
    "base64_payload_embedded",
    "binary_payload_embedded",
    "chunk_ready_claimed",
    "chunk_ready_claimed_for_non_parser_ready_rows",
    "chunk_text_included",
    "embeddings_included",
    "graph_import_allowed",
    "graph_write_attempted",
    "kg_readiness_claimed",
    "ladybugdb_written",
    "model_call_attempted",
    "network_fetch_attempted",
    "optimizer_traces_included",
    "parser_ready_claimed_without_conversion",
    "production_import_attempted",
    "production_ladybugdb_write_allowed",
    "production_persistence_attempted",
    "raw_article_html_embedded",
    "raw_article_text_embedded",
    "raw_binary_embedded",
    "raw_binary_included",
    "raw_payload_embedded_in_metadata",
    "raw_pdf_bytes_embedded",
    "raw_text_embedded",
    "raw_text_included",
    "secrets_included",
    "trusted_kg_import_allowed",
    "vectors_included",
}
FORBIDDEN_PAYLOAD_KEYS = {
    "text",
    "raw_text",
    "chunk_text",
    "paper_text",
    "claim_text",
    "html",
    "raw_html",
    "pdf_bytes",
    "binary_payload",
    "base64_payload",
    "embedding",
    "embeddings",
    "vector",
    "vectors",
    "converted_text",
    "normalized_markdown",
}
FORBIDDEN_SNIPPETS = {
    "The original user question is typically complex",
    "Your goal: generate a query",
    "First-hop documents often cover",
    "Local Parser Ready Paper",
    "No network fetches or graph writes should be needed",
    "%PDF-",
    "<html",
    "</html",
    "base64,",
    "normalized_markdown",
}
VERDICT_VALUES = {"PASS", "FLAG", "REPAIR", "BLOCKER"}


class S05CloseoutError(RuntimeError):
    """Deterministic fail-closed S05 closeout validation error."""

    def __init__(
        self, code: str, message: str, *, json_path: str = "$", path: str | Path | None = None
    ) -> None:
        prefix = f"{code} {json_path}"
        if path is not None:
            prefix = f"{prefix} {Path(path).as_posix()}"
        super().__init__(f"{prefix}: {message}")
        self.code = code
        self.json_path = json_path
        self.path = Path(path).as_posix() if isinstance(path, Path) else path
        self.message = message


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--s04-closeout", type=Path, default=DEFAULT_S04_CLOSEOUT)
    parser.add_argument("--import-summary", type=Path, default=DEFAULT_IMPORT_SUMMARY)
    parser.add_argument("--import-diagnostics", type=Path, default=DEFAULT_IMPORT_DIAGNOSTICS)
    parser.add_argument("--import-report", type=Path, default=DEFAULT_IMPORT_REPORT)
    parser.add_argument("--matrix-json", type=Path, default=DEFAULT_MATRIX_JSON)
    parser.add_argument("--matrix-md", type=Path, default=DEFAULT_MATRIX_MD)
    parser.add_argument("--audit-json", type=Path, default=DEFAULT_AUDIT_JSON)
    parser.add_argument("--audit-md", type=Path, default=DEFAULT_AUDIT_MD)
    parser.add_argument("--review-events", type=Path, default=DEFAULT_REVIEW_EVENTS)
    parser.add_argument("--summary-out", type=Path, default=DEFAULT_SUMMARY_OUT)
    parser.add_argument("--diagnostics-out", type=Path, default=DEFAULT_DIAGNOSTICS_OUT)
    parser.add_argument("--report-out", type=Path, default=DEFAULT_REPORT_OUT)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        summary, diagnostics, report = run_closeout(args)
    except S05CloseoutError as exc:
        sys.stderr.write(f"{exc}\n")
        return 1
    write_json(args.summary_out, summary)
    write_jsonl(args.diagnostics_out, diagnostics)
    write_text(args.report_out, report)
    sys.stdout.write(
        "S05 closeout passed: "
        f"rows={summary['progression_row_count']} "
        f"rejected={summary['rejected_import_candidate_count']} "
        f"failures={summary['failure_count']} "
        "fail_closed=true\n"
    )
    sys.stdout.write(f"Wrote {args.summary_out.as_posix()}\n")
    sys.stdout.write(f"Wrote {args.diagnostics_out.as_posix()}\n")
    sys.stdout.write(f"Wrote {args.report_out.as_posix()}\n")
    return 0


def run_closeout(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]], str]:
    inputs = {
        "s04_closeout": load_json(args.s04_closeout),
        "import_summary": load_json(args.import_summary),
        "matrix": load_json(args.matrix_json),
        "audit": load_json(args.audit_json),
    }
    import_diagnostics = load_jsonl(args.import_diagnostics)
    review_events = load_jsonl(args.review_events)
    import_report = load_text(args.import_report)
    matrix_md = load_text(args.matrix_md)
    audit_md = load_text(args.audit_md)

    validate_contracts(
        inputs,
        import_diagnostics=import_diagnostics,
        review_events=review_events,
        import_report=import_report,
        matrix_md=matrix_md,
        audit_md=audit_md,
    )
    summary = build_summary(
        inputs, import_diagnostics=import_diagnostics, review_events=review_events
    )
    report = render_report(summary, inputs, import_diagnostics=import_diagnostics)
    validate_no_payload_leakage(
        summary, rendered=json.dumps(summary, sort_keys=True) + report, where="s05_closeout_outputs"
    )
    return summary, [], report


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise S05CloseoutError(
            "M031_S05_INPUT_MISSING", "required JSON artifact is missing", path=path
        ) from exc
    except json.JSONDecodeError as exc:
        raise S05CloseoutError("M031_S05_INVALID_JSON", f"invalid JSON: {exc}", path=path) from exc
    if not isinstance(value, dict):
        raise S05CloseoutError("M031_S05_INVALID_JSON", "expected a JSON object", path=path)
    return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise S05CloseoutError(
            "M031_S05_INPUT_MISSING", "required JSONL artifact is missing", path=path
        ) from exc
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise S05CloseoutError(
                "M031_S05_INVALID_JSONL", f"invalid JSONL at line {line_number}: {exc}", path=path
            ) from exc
        if not isinstance(value, dict):
            raise S05CloseoutError(
                "M031_S05_INVALID_JSONL", f"expected JSON object at line {line_number}", path=path
            )
        rows.append(value)
    return rows


def load_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise S05CloseoutError(
            "M031_S05_INPUT_MISSING", "required report artifact is missing", path=path
        ) from exc


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in rows),
        encoding="utf-8",
    )


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def validate_contracts(
    inputs: Mapping[str, Any],
    *,
    import_diagnostics: Sequence[Mapping[str, Any]],
    review_events: Sequence[Mapping[str, Any]],
    import_report: str,
    matrix_md: str,
    audit_md: str,
) -> None:
    s04_closeout = inputs["s04_closeout"]
    import_summary = inputs["import_summary"]
    matrix = inputs["matrix"]
    audit = inputs["audit"]

    if s04_closeout.get("status") != "passed" or s04_closeout.get("failure_count") != 0:
        raise S05CloseoutError(
            "M031_S05_S04_CLOSEOUT_NOT_PASSED", "S04 closeout must be passing", json_path="$.status"
        )
    if s04_closeout.get("row_count") != EXPECTED_ROW_COUNT:
        raise S05CloseoutError(
            "M031_S05_S04_ROW_COUNT",
            "S04 closeout row count must be seven",
            json_path="$.row_count",
        )
    if s04_closeout.get("independent_review_completed_count") != 0:
        raise S05CloseoutError(
            "M031_S05_COMPLETED_REVIEW_WITHOUT_VERDICT",
            "S04 closeout must not claim completed review",
            json_path="$.independent_review_completed_count",
        )

    if import_summary.get("valid_rehearsal") is not True:
        raise S05CloseoutError(
            "M031_S05_IMPORT_BOUNDARY_INVALID",
            "import-boundary rehearsal must validate",
            json_path="$.valid_rehearsal",
        )
    if (
        import_summary.get("candidate_count") != EXPECTED_ROW_COUNT
        or import_summary.get("rejected_count") != EXPECTED_ROW_COUNT
    ):
        raise S05CloseoutError(
            "M031_S05_IMPORT_BOUNDARY_COUNTS",
            "all seven candidates must be rejected",
            json_path="$.candidate_count",
        )
    for key in ("accepted_count", "import_eligible_count"):
        if import_summary.get(key) != 0:
            raise S05CloseoutError(
                "M031_S05_IMPORT_BOUNDARY_PERMISSIVE", f"{key} must be zero", json_path=f"$.{key}"
            )
    if len(import_diagnostics) != EXPECTED_ROW_COUNT:
        raise S05CloseoutError(
            "M031_S05_IMPORT_DIAGNOSTIC_COUNT",
            "seven refusal diagnostics are required",
            json_path="$.diagnostics",
        )
    for index, diagnostic in enumerate(import_diagnostics):
        if diagnostic.get("diagnostic_code") != "M031_IMPORT_BOUNDARY_REFUSED":
            raise S05CloseoutError(
                "M031_S05_IMPORT_DIAGNOSTIC_CODE",
                "unexpected import diagnostic code",
                json_path=f"$.diagnostics[{index}].diagnostic_code",
            )
        for bool_key, expected in (
            ("accepted", False),
            ("import_eligible", False),
            ("blocks_import", True),
            ("rejected", True),
        ):
            if diagnostic.get(bool_key) is not expected:
                raise S05CloseoutError(
                    "M031_S05_IMPORT_BOUNDARY_PERMISSIVE",
                    f"diagnostic {bool_key} must be {expected}",
                    json_path=f"$.diagnostics[{index}].{bool_key}",
                )
    if "M031_IMPORT_BOUNDARY_REFUSED" not in import_report:
        raise S05CloseoutError(
            "M031_S05_IMPORT_REPORT_STALE", "import report must include refusal diagnostic code"
        )

    if (
        matrix.get("row_count") != EXPECTED_ROW_COUNT
        or len(matrix.get("rows", [])) != EXPECTED_ROW_COUNT
    ):
        raise S05CloseoutError(
            "M031_S05_PROGRESSION_ROW_COUNT",
            "progression matrix must contain seven rows",
            json_path="$.row_count",
        )
    for index, row in enumerate(matrix.get("rows", [])):
        if not isinstance(row, Mapping):
            raise S05CloseoutError(
                "M031_S05_PROGRESSION_ROW_SHAPE",
                "progression row must be an object",
                json_path=f"$.rows[{index}]",
            )
        stages = row.get("stages")
        if not isinstance(stages, Mapping) or len(stages) != EXPECTED_STAGE_COUNT:
            raise S05CloseoutError(
                "M031_S05_PROGRESSION_STAGE_COUNT",
                "each progression row must retain eight stages",
                json_path=f"$.rows[{index}].stages",
            )
        if row.get("import_boundary_state") != "refused":
            raise S05CloseoutError(
                "M031_S05_IMPORT_BOUNDARY_PERMISSIVE",
                "row import boundary state must remain refused",
                json_path=f"$.rows[{index}].import_boundary_state",
            )

    if audit.get("row_count") != EXPECTED_ROW_COUNT:
        raise S05CloseoutError(
            "M031_S05_AUDIT_ROW_COUNT", "audit row count must be seven", json_path="$.row_count"
        )
    verdict_state = (
        audit.get("review_verdict_state")
        if isinstance(audit.get("review_verdict_state"), Mapping)
        else {}
    )
    if verdict_state.get("completed_review_event_count") != 0:
        raise S05CloseoutError(
            "M031_S05_COMPLETED_REVIEW_WITHOUT_VERDICT",
            "continuity audit must preserve pending review state",
            json_path="$.review_verdict_state.completed_review_event_count",
        )
    for section in EXPECTED_REQUIRED_SECTIONS:
        if section not in audit_md:
            raise S05CloseoutError(
                "M031_S05_AUDIT_SECTION_MISSING", f"required continuity section missing: {section}"
            )
    if "# M031 Progression Matrix" not in matrix_md:
        raise S05CloseoutError(
            "M031_S05_MATRIX_REPORT_STALE", "progression matrix Markdown is missing its title"
        )

    completed_events = [
        event
        for event in review_events
        if event.get("output_contract_completed") is True
        or event.get("independent_review_completed") is True
    ]
    verdict_events = [
        event
        for event in completed_events
        if str(event.get("verdict") or "").upper() in VERDICT_VALUES
    ]
    if completed_events and len(verdict_events) != len(completed_events):
        raise S05CloseoutError(
            "M031_S05_COMPLETED_REVIEW_WITHOUT_VERDICT",
            "completed-review claims require explicit verdict evidence",
            json_path="$.review_events",
        )

    for label, payload in inputs.items():
        collect_unsafe_flags(payload, where=label)
        validate_no_payload_leakage(
            payload, rendered=json.dumps(payload, sort_keys=True), where=label
        )
    for label, text in (
        ("import_report", import_report),
        ("matrix_md", matrix_md),
        ("audit_md", audit_md),
    ):
        validate_no_payload_leakage({}, rendered=text, where=label)


def collect_unsafe_flags(value: Any, *, where: str, path: str = "$") -> None:
    if isinstance(value, Mapping):
        for key, item in value.items():
            next_path = f"{path}.{key}"
            if key in EXPECTED_FALSE_FLAGS and item is True:
                raise S05CloseoutError(
                    "M031_S05_UNSAFE_FAIL_CLOSED_FLAG",
                    f"fail-closed flag is true: {key}",
                    json_path=next_path,
                    path=where,
                )
            collect_unsafe_flags(item, where=where, path=next_path)
    elif isinstance(value, list):
        for index, item in enumerate(value):
            collect_unsafe_flags(item, where=where, path=f"{path}[{index}]")


def validate_no_payload_leakage(value: Any, *, rendered: str, where: str) -> None:
    def walk(node: Any, path: str) -> None:
        if isinstance(node, Mapping):
            for key, item in node.items():
                if str(key) in FORBIDDEN_PAYLOAD_KEYS:
                    raise S05CloseoutError(
                        "M031_S05_RAW_PAYLOAD_LEAKAGE",
                        f"forbidden payload key {key!r}",
                        json_path=f"{path}.{key}",
                        path=where,
                    )
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")
    lowered = rendered.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            raise S05CloseoutError(
                "M031_S05_RAW_PAYLOAD_LEAKAGE", f"forbidden payload snippet {snippet!r}", path=where
            )


def build_summary(
    inputs: Mapping[str, Any],
    *,
    import_diagnostics: Sequence[Mapping[str, Any]],
    review_events: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    s04_closeout = inputs["s04_closeout"]
    import_summary = inputs["import_summary"]
    matrix = inputs["matrix"]
    refusal_counts = Counter(str(record.get("diagnostic_code")) for record in import_diagnostics)
    completed_review_count = sum(
        1
        for event in review_events
        if event.get("output_contract_completed") is True
        or event.get("independent_review_completed") is True
    )
    summary: dict[str, Any] = {
        "schema_version": "m031-s05-closeout-verifier.v1",
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "status": "passed",
        "failure_count": 0,
        "progression_row_count": matrix.get("row_count"),
        "progression_stage_count_per_row": EXPECTED_STAGE_COUNT,
        "rejected_import_candidate_count": import_summary.get("rejected_count"),
        "accepted_count": import_summary.get("accepted_count"),
        "import_eligible_count": import_summary.get("import_eligible_count"),
        "import_refusal_diagnostic_count": len(import_diagnostics),
        "diagnostic_code_counts": dict(sorted(refusal_counts.items())),
        "s04_row_count": s04_closeout.get("row_count"),
        "s04_zero_chunk_refusal_count": s04_closeout.get("zero_chunk_refusal_count"),
        "s04_graph_readiness_package_count": s04_closeout.get("graph_readiness_package_count"),
        "independent_review_completed_count": completed_review_count,
        "completed_review_refusal_in_force": completed_review_count == 0,
        "required_continuity_sections_present": list(EXPECTED_REQUIRED_SECTIONS),
        "network_fetch_attempted": False,
        "model_call_attempted": False,
        "graph_import_allowed": False,
        "trusted_kg_import_allowed": False,
        "production_import_attempted": False,
        "graph_write_attempted": False,
        "production_persistence_attempted": False,
        "ladybugdb_written": False,
        "raw_text_included": False,
        "chunk_text_included": False,
        "embeddings_included": False,
        "vectors_included": False,
        "fail_closed_flags": {
            "network_fetch_attempted": False,
            "model_call_attempted": False,
            "graph_import_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "graph_write_attempted": False,
            "production_persistence_attempted": False,
            "ladybugdb_written": False,
            "raw_text_included": False,
            "chunk_text_included": False,
            "embeddings_included": False,
            "vectors_included": False,
        },
        "source_artifacts": [
            DEFAULT_S04_CLOSEOUT.as_posix(),
            DEFAULT_IMPORT_SUMMARY.as_posix(),
            DEFAULT_IMPORT_DIAGNOSTICS.as_posix(),
            DEFAULT_IMPORT_REPORT.as_posix(),
            DEFAULT_MATRIX_JSON.as_posix(),
            DEFAULT_MATRIX_MD.as_posix(),
            DEFAULT_AUDIT_JSON.as_posix(),
            DEFAULT_AUDIT_MD.as_posix(),
            DEFAULT_REVIEW_EVENTS.as_posix(),
        ],
        "failure_modes_gate_q5": [
            "Filesystem: missing JSON/JSONL/Markdown artifacts fail with M031_S05_INPUT_MISSING before writes.",
            "Malformed artifacts: invalid JSON/JSONL fail with M031_S05_INVALID_JSON or M031_S05_INVALID_JSONL.",
            "Scope drift: stale counts, missing stages, missing continuity sections, or permissive import flags fail before closeout artifacts are written.",
            "Review boundary drift: completed-review claims without explicit verdict evidence fail with M031_S05_COMPLETED_REVIEW_WITHOUT_VERDICT.",
        ],
        "load_profile_gate_q6": {
            "expected_load": "7 progression rows, 8 stages per row, 7 import refusal diagnostics, bounded local summary/report artifacts",
            "ten_x_breakpoint": "local JSON/Markdown parsing and recursive payload scanning saturate first at about 70 rows; no network, model, subprocess, graph, or LadybugDB runtime path exists",
            "protection": "single-pass local validation, deterministic counts, no remote calls, no raw payload reads, no database writes, and outputs written only after all preflight checks pass",
        },
        "negative_tests_gate_q7": [
            "permissive import summary accepted_count/import_eligible_count",
            "missing progression matrix row",
            "completed review claim without verdict evidence",
            "raw payload leakage in source artifacts",
        ],
        "recovery_commands": [
            "uv run python scripts/replay_m031_import_boundary_rehearsal.py",
            "uv run python scripts/verify_m031_process_continuity_audit.py",
            "uv run python scripts/verify_m031_s05_closeout.py",
        ],
    }
    return summary


def render_report(
    summary: Mapping[str, Any],
    inputs: Mapping[str, Any],
    *,
    import_diagnostics: Sequence[Mapping[str, Any]],
) -> str:
    audit = inputs["audit"]
    lines = [
        "# M031 S05 Closeout Report",
        "",
        "Final S05 regression and fail-closed scope audit. This report is metadata-only and does not approve graph import, trusted KG import, production persistence, model calls, network fetches, or LadybugDB writes.",
        "",
        "## Summary",
        f"- status: `{summary.get('status')}`",
        f"- failure_count: {summary.get('failure_count')}",
        f"- progression rows: {summary.get('progression_row_count')}",
        f"- rejected import candidates: {summary.get('rejected_import_candidate_count')}",
        f"- accepted/import-eligible candidates: {summary.get('accepted_count')}/{summary.get('import_eligible_count')}",
        f"- independent-review completed count: {summary.get('independent_review_completed_count')}",
        f"- completed-review refusal remains in force: {str(summary.get('completed_review_refusal_in_force')).lower()}",
        "",
        "## Fail-Closed Flags",
        f"- network_fetch_attempted={str(summary.get('network_fetch_attempted')).lower()}",
        f"- model_call_attempted={str(summary.get('model_call_attempted')).lower()}",
        f"- graph_import_allowed={str(summary.get('graph_import_allowed')).lower()}",
        f"- trusted_kg_import_allowed={str(summary.get('trusted_kg_import_allowed')).lower()}",
        f"- production_import_attempted={str(summary.get('production_import_attempted')).lower()}",
        f"- graph_write_attempted={str(summary.get('graph_write_attempted')).lower()}",
        f"- production_persistence_attempted={str(summary.get('production_persistence_attempted')).lower()}",
        f"- ladybugdb_written={str(summary.get('ladybugdb_written')).lower()}",
        "",
        "## Diagnostic Codes",
    ]
    for code, count in sorted(summary.get("diagnostic_code_counts", {}).items()):
        lines.append(f"- `{code}`: {count}")
    lines.extend(
        [
            "",
            "## Continuity Sections Checked",
        ]
    )
    for section in summary.get("required_continuity_sections_present", []):
        lines.append(f"- `{section}`")
    lines.extend(
        [
            "",
            "## Refusal Candidate Matrix",
            "| JSON Path | Package | Diagnostic | Blocks Import | Accepted | Import Eligible |",
            "|---|---|---|---|---|---|",
        ]
    )
    for diagnostic in import_diagnostics:
        lines.append(
            "| {json_path} | `{package_id}` | `{diagnostic_code}` | {blocks_import} | {accepted} | {import_eligible} |".format(
                json_path=diagnostic.get("json_path"),
                package_id=diagnostic.get("package_id"),
                diagnostic_code=diagnostic.get("diagnostic_code"),
                blocks_import=str(diagnostic.get("blocks_import")).lower(),
                accepted=str(diagnostic.get("accepted")).lower(),
                import_eligible=str(diagnostic.get("import_eligible")).lower(),
            )
        )
    lines.extend(
        [
            "",
            "## Failure Modes",
            *[f"- {item}" for item in summary.get("failure_modes_gate_q5", [])],
            "",
            "## Load Profile",
            f"- Expected load: {summary.get('load_profile_gate_q6', {}).get('expected_load')}",
            f"- 10x breakpoint: {summary.get('load_profile_gate_q6', {}).get('ten_x_breakpoint')}",
            f"- Protection: {summary.get('load_profile_gate_q6', {}).get('protection')}",
            "",
            "## Negative Tests",
            *[f"- {item}" for item in summary.get("negative_tests_gate_q7", [])],
            "",
            "## Recovery Commands",
            *[f"- `{command}`" for command in summary.get("recovery_commands", [])],
            "",
            "## Downstream Boundary",
            str(audit.get("structural_route_label_notice")),
            "",
            "No raw text, chunk text, PDF bytes, HTML, embeddings, vectors, secrets, model traces, optimizer traces, external fetch state, graph writes, production persistence, or LadybugDB writes are included.",
            "",
        ]
    )
    return "\n".join(lines)


if __name__ == "__main__":
    raise SystemExit(main())
