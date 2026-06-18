#!/usr/bin/env python3
"""Generate and validate M031 progression matrix and continuity audit artifacts.

This verifier is intentionally local-only. It consumes metadata-only M031 S01-S05
artifacts, writes final inspection surfaces after preflight validation, and never
fetches network resources, reads raw article payloads, imports graph facts, or
writes LadybugDB.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

MILESTONE_ID = "M031-vwpd8e"
SLICE_ID = "S05"
SELECTION_ID = "m031-catalog-backed-replay-v1"
CORPUS_DIR = Path("data/article_corpora/m031-catalog-backed-replay-v1")

PROGRESSION_SCHEMA_VERSION = "m031-progression-matrix.v1"
AUDIT_SCHEMA_VERSION = "m031-process-continuity-audit.v1"

STAGE_ORDER = [
    "url_intake",
    "article_catalog",
    "source_acquisition",
    "loader_evidence",
    "parser_conversion",
    "chunking",
    "graph_readiness_review",
    "graph_import_boundary",
]

DEFAULT_SELECTION = CORPUS_DIR / "selection.json"
DEFAULT_ACQUISITION_SUMMARY = CORPUS_DIR / "source-acquisition-summary.json"
DEFAULT_LOADER_SUMMARY = CORPUS_DIR / "loader-evidence-summary.json"
DEFAULT_REPLAY_CLOSEOUT = CORPUS_DIR / "replay-closeout-summary.json"
DEFAULT_CONVERSION_SUMMARY = CORPUS_DIR / "conversion-quality" / "conversion-quality-summary.json"
DEFAULT_CONVERSION_CLOSEOUT = CORPUS_DIR / "parser-conversion-closeout-summary.json"
DEFAULT_CHUNK_SUMMARY = CORPUS_DIR / "chunk-evidence" / "chunk-evidence-summary.json"
DEFAULT_CHUNK_CLOSEOUT = CORPUS_DIR / "chunk-evidence-closeout-summary.json"
DEFAULT_REVIEW_EVENTS = CORPUS_DIR / "chunk-evidence" / "independent-review-events.jsonl"
DEFAULT_REVIEW_SUMMARY = CORPUS_DIR / "graph-readiness-review" / "independent-review-summary.md"
DEFAULT_REVIEW_BUNDLE = CORPUS_DIR / "graph-readiness-review" / "arxiv_cs-cl_2507.19457_arxiv_pdf-review.md"
DEFAULT_IMPORT_SUMMARY = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-summary.json"
DEFAULT_IMPORT_DIAGNOSTICS = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-diagnostics.jsonl"
DEFAULT_IMPORT_REPORT = CORPUS_DIR / "import-boundary-rehearsal" / "import-boundary-report.md"
DEFAULT_MATRIX_JSON = CORPUS_DIR / "progression-matrix.json"
DEFAULT_MATRIX_MD = CORPUS_DIR / "progression-matrix.md"
DEFAULT_AUDIT_JSON = CORPUS_DIR / "m031-continuity-audit.json"
DEFAULT_AUDIT_MD = CORPUS_DIR / "m031-continuity-audit.md"

EXPECTED_ROW_COUNT = 7
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
POSITIVE_STRUCTURAL_LABELS = {"ok_for_graph", "trusted_graph"}
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
    "deterministic fallback capture",
    "Local Parser Ready Paper",
    "No network fetches or graph writes should be needed",
    "%PDF-",
    "<html",
    "</html",
    "base64,",
    "normalized_markdown_char",
}
REQUIRED_REPORT_PHRASES = (
    "# M031 Progression Matrix",
    "# M031 Process Continuity Audit",
    "## Per-Ref / Module Progression Matrix",
    "## Stage Owners, Evidence, Verifiers, and Failure Modes",
    "## Unsafe Claims to Preserve",
    "## Fail-Closed Flags",
    "## Structural Route Label Notice",
    "## Failure Modes",
    "## Load Profile",
    "## Negative Tests",
    "ok_for_graph",
    "trusted_graph",
    "structural states only",
    "independent semantic review",
    "LadybugDB",
    "graph import",
)


class ContinuityAuditError(RuntimeError):
    """Fail-closed continuity error with a deterministic diagnostic code."""

    def __init__(self, code: str, message: str, *, json_path: str = "$", path: str | Path | None = None) -> None:
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
    parser.add_argument("--selection", type=Path, default=DEFAULT_SELECTION)
    parser.add_argument("--acquisition-summary", type=Path, default=DEFAULT_ACQUISITION_SUMMARY)
    parser.add_argument("--loader-summary", type=Path, default=DEFAULT_LOADER_SUMMARY)
    parser.add_argument("--replay-closeout", type=Path, default=DEFAULT_REPLAY_CLOSEOUT)
    parser.add_argument("--conversion-summary", type=Path, default=DEFAULT_CONVERSION_SUMMARY)
    parser.add_argument("--conversion-closeout", type=Path, default=DEFAULT_CONVERSION_CLOSEOUT)
    parser.add_argument("--chunk-summary", type=Path, default=DEFAULT_CHUNK_SUMMARY)
    parser.add_argument("--chunk-closeout", type=Path, default=DEFAULT_CHUNK_CLOSEOUT)
    parser.add_argument("--review-events", type=Path, default=DEFAULT_REVIEW_EVENTS)
    parser.add_argument("--review-summary", type=Path, default=DEFAULT_REVIEW_SUMMARY)
    parser.add_argument("--review-bundle", type=Path, default=DEFAULT_REVIEW_BUNDLE)
    parser.add_argument("--import-summary", type=Path, default=DEFAULT_IMPORT_SUMMARY)
    parser.add_argument("--import-diagnostics", type=Path, default=DEFAULT_IMPORT_DIAGNOSTICS)
    parser.add_argument("--import-report", type=Path, default=DEFAULT_IMPORT_REPORT)
    parser.add_argument("--matrix-json", type=Path, default=DEFAULT_MATRIX_JSON)
    parser.add_argument("--matrix-md", type=Path, default=DEFAULT_MATRIX_MD)
    parser.add_argument("--audit-json", type=Path, default=DEFAULT_AUDIT_JSON)
    parser.add_argument("--audit-md", type=Path, default=DEFAULT_AUDIT_MD)
    parser.add_argument("--validate-only", action="store_true", help="Validate existing generated matrix/audit artifacts only.")
    return parser.parse_args(argv)


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise ContinuityAuditError("M031_CONTINUITY_INPUT_MISSING", "required JSON artifact is missing", path=path) from exc
    except json.JSONDecodeError as exc:
        raise ContinuityAuditError("M031_CONTINUITY_INVALID_JSON", f"invalid JSON: {exc}", path=path) from exc
    if not isinstance(value, dict):
        raise ContinuityAuditError("M031_CONTINUITY_INVALID_JSON", "expected a JSON object", path=path)
    return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise ContinuityAuditError("M031_CONTINUITY_INPUT_MISSING", "required JSONL artifact is missing", path=path) from exc
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ContinuityAuditError("M031_CONTINUITY_INVALID_JSONL", f"invalid JSONL at line {line_number}: {exc}", path=path) from exc
        if not isinstance(value, dict):
            raise ContinuityAuditError("M031_CONTINUITY_INVALID_JSONL", f"expected JSON object at line {line_number}", path=path)
        rows.append(value)
    return rows


def load_required_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise ContinuityAuditError("M031_CONTINUITY_INPUT_MISSING", "required report artifact is missing", path=path) from exc


def write_json(path: Path, payload: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def row_key(row: Mapping[str, Any]) -> tuple[str, str, str]:
    return (str(row.get("identity") or ""), str(row.get("source_role") or ""), str(row.get("variant_id") or ""))


def package_id_for(row: Mapping[str, Any]) -> str:
    article_ref = str(row.get("article_ref") or row.get("identity") or "missing")
    source_role = str(row.get("source_role") or "missing")
    return f"{article_ref.replace('/', '_').replace(':', '_')}_{source_role}"


def rows_by_key(rows: Any) -> dict[tuple[str, str, str], dict[str, Any]]:
    if not isinstance(rows, list):
        return {}
    result: dict[tuple[str, str, str], dict[str, Any]] = {}
    for row in rows:
        if isinstance(row, dict):
            result[row_key(row)] = row
    return result


def _safe_path(value: Any) -> str | None:
    if not isinstance(value, str) or not value.strip():
        return None
    path = value.replace("\\", "/")
    if path.startswith("/root/daily-archive/"):
        return path.removeprefix("/root/daily-archive/")
    if Path(path).is_absolute():
        return None
    return path


def _stage(status: str, *, evidence_path: str, json_path: str, diagnostic_code: str | None = None, notes: str | None = None, counts: Mapping[str, Any] | None = None) -> dict[str, Any]:
    stage: dict[str, Any] = {
        "status": status,
        "evidence_path": evidence_path,
        "json_path": json_path,
        "diagnostic_code": diagnostic_code or "none",
    }
    if notes:
        stage["notes"] = notes
    if counts:
        stage["counts"] = dict(counts)
    return stage


def _selection_indexes(selection: Mapping[str, Any]) -> tuple[dict[str, dict[str, Any]], dict[tuple[str, str], dict[str, Any]]]:
    refs: dict[str, dict[str, Any]] = {}
    for row in selection.get("requested_refs", []):
        if isinstance(row, dict) and isinstance(row.get("identity"), str):
            refs[row["identity"]] = row
    variants: dict[tuple[str, str], dict[str, Any]] = {}
    for article in selection.get("articles", []):
        if not isinstance(article, dict):
            continue
        identity = str(article.get("identity") or "")
        for variant in article.get("source_variants", []):
            if isinstance(variant, dict):
                variants[(identity, str(variant.get("source_role") or ""))] = {"article": article, "variant": variant}
    return refs, variants


def _review_event_indexes(events: Sequence[Mapping[str, Any]]) -> tuple[dict[str, Mapping[str, Any]], list[Mapping[str, Any]]]:
    by_paper: dict[str, Mapping[str, Any]] = {}
    completed: list[Mapping[str, Any]] = []
    for event in events:
        if event.get("paper_id"):
            by_paper[str(event["paper_id"])] = event
        if event.get("independent_review_completed") is True or event.get("output_contract_completed") is True:
            completed.append(event)
    return by_paper, completed


def assert_artifact_contracts(inputs: Mapping[str, Any], *, import_diagnostics: Sequence[Mapping[str, Any]], review_events: Sequence[Mapping[str, Any]], import_report: str, review_summary_text: str) -> None:
    selection = inputs["selection"]
    conversion = inputs["conversion"]
    chunk = inputs["chunk"]
    import_summary = inputs["import_summary"]

    if selection.get("milestone_id") != MILESTONE_ID:
        raise ContinuityAuditError("M031_CONTINUITY_MILESTONE", "selection milestone mismatch", json_path="$.milestone_id")
    if selection.get("selection_id") != SELECTION_ID:
        raise ContinuityAuditError("M031_CONTINUITY_SELECTION_ID", "selection_id mismatch", json_path="$.selection_id")
    if selection.get("counts", {}).get("requested_ref_count") != 4:
        raise ContinuityAuditError("M031_CONTINUITY_SELECTION_COUNTS", "requested_ref_count must be 4", json_path="$.counts.requested_ref_count")
    if selection.get("counts", {}).get("source_variant_count") != EXPECTED_ROW_COUNT - 1:
        raise ContinuityAuditError("M031_CONTINUITY_SELECTION_COUNTS", "source_variant_count must remain 6 catalog variants", json_path="$.counts.source_variant_count")

    for label in ("acquisition", "loader", "conversion", "chunk"):
        payload = inputs[label]
        count = payload.get("variant_or_blocker_count") or payload.get("loader_row_count") or payload.get("row_count")
        if count != EXPECTED_ROW_COUNT:
            raise ContinuityAuditError("M031_CONTINUITY_ROW_COUNT", f"{label} row count must be {EXPECTED_ROW_COUNT}", json_path=f"$.{label}.row_count")

    if conversion.get("parser_ready_count") != 1:
        raise ContinuityAuditError("M031_CONTINUITY_PARSER_READY_COUNT", "parser_ready_count must be 1", json_path="$.parser_ready_count")
    if chunk.get("zero_chunk_refusal_count") != 6 or chunk.get("pending_graph_readiness_review_count") != 1:
        raise ContinuityAuditError("M031_CONTINUITY_CHUNK_COUNTS", "chunk/refusal/review counts drifted", json_path="$.zero_chunk_refusal_count")
    if chunk.get("independent_review_completed_count") != 0:
        raise ContinuityAuditError("M031_COMPLETED_REVIEW_WITHOUT_VERDICT", "S04 summary must not claim completed independent review", json_path="$.independent_review_completed_count")

    if import_summary.get("valid_rehearsal") is not True:
        raise ContinuityAuditError("M031_IMPORT_BOUNDARY_REFUSAL_ARTIFACT_MISSING", "import summary must be a valid refusal rehearsal", json_path="$.valid_rehearsal")
    if import_summary.get("candidate_count") != EXPECTED_ROW_COUNT or import_summary.get("rejected_count") != EXPECTED_ROW_COUNT:
        raise ContinuityAuditError("M031_IMPORT_BOUNDARY_COUNTS", "import rehearsal must reject all seven candidates", json_path="$.candidate_count")
    for key in ("accepted_count", "import_eligible_count"):
        if import_summary.get(key) != 0:
            raise ContinuityAuditError("M031_IMPORT_BOUNDARY_PERMISSIVE", f"{key} must be 0", json_path=f"$.{key}")
    if len(import_diagnostics) != EXPECTED_ROW_COUNT:
        raise ContinuityAuditError("M031_IMPORT_BOUNDARY_REFUSAL_ARTIFACT_MISSING", "import diagnostics must include seven refusal rows", json_path="$.diagnostics")
    if "M031_IMPORT_BOUNDARY_REFUSED" not in import_report:
        raise ContinuityAuditError("M031_IMPORT_BOUNDARY_REFUSAL_ARTIFACT_MISSING", "import report must include refusal diagnostic code")
    if "Independent reviewer verdicts are still required" not in review_summary_text:
        raise ContinuityAuditError("M031_REVIEW_HANDOFF_STALE", "review summary must remain pending-review handoff")

    completed_events = [event for event in review_events if event.get("output_contract_completed") is True or event.get("independent_review_completed") is True]
    if completed_events:
        verdict_events = [event for event in completed_events if str(event.get("verdict") or "").upper() in {"PASS", "FLAG", "REPAIR", "BLOCKER"}]
        if len(verdict_events) != len(completed_events):
            raise ContinuityAuditError("M031_COMPLETED_REVIEW_WITHOUT_VERDICT", "completed-review claims require explicit verdict evidence", json_path="$.review_events")

    for label, payload in inputs.items():
        if isinstance(payload, Mapping):
            findings = collect_unsafe_flags(payload, where=label)
            if findings:
                first = findings[0]
                raise ContinuityAuditError(first["diagnostic_code"], first["message"], json_path=first["json_path"], path=first.get("path"))


def collect_unsafe_flags(value: Any, *, where: str, path: str = "$") -> list[dict[str, str]]:
    findings: list[dict[str, str]] = []
    if isinstance(value, Mapping):
        for key, item in value.items():
            next_path = f"{path}.{key}"
            if key in EXPECTED_FALSE_FLAGS and item is True:
                findings.append(
                    {
                        "diagnostic_code": "M031_UNSAFE_FAIL_CLOSED_FLAG",
                        "message": f"fail-closed flag is true: {key}",
                        "json_path": next_path,
                        "path": where,
                    }
                )
            findings.extend(collect_unsafe_flags(item, where=where, path=next_path))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            findings.extend(collect_unsafe_flags(item, where=where, path=f"{path}[{index}]"))
    return findings


def validate_no_payload_leakage(value: Any, *, rendered: str, where: str) -> list[str]:
    errors: list[str] = []

    def walk(node: Any, path: str) -> None:
        if isinstance(node, Mapping):
            for key, item in node.items():
                if str(key) in FORBIDDEN_PAYLOAD_KEYS:
                    errors.append(f"M031_RAW_PAYLOAD_LEAKAGE {where} {path}.{key}: forbidden payload key {key!r}")
                walk(item, f"{path}.{key}")
        elif isinstance(node, list):
            for index, item in enumerate(node):
                walk(item, f"{path}[{index}]")

    walk(value, "$")
    lowered = rendered.lower()
    for snippet in FORBIDDEN_SNIPPETS:
        if snippet.lower() in lowered:
            errors.append(f"M031_RAW_PAYLOAD_LEAKAGE {where}: forbidden payload snippet {snippet!r}")
    return errors


def build_progression_matrix(inputs: Mapping[str, Any], *, import_diagnostics: Sequence[Mapping[str, Any]], review_events: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    selection = inputs["selection"]
    acquisition = inputs["acquisition"]
    loader = inputs["loader"]
    conversion = inputs["conversion"]
    chunk = inputs["chunk"]
    refs_by_identity, variants_by_identity_role = _selection_indexes(selection)
    acquisition_by_key = rows_by_key(acquisition.get("results"))
    loader_by_key = rows_by_key(loader.get("results"))
    conversion_rows = conversion.get("results") if isinstance(conversion.get("results"), list) else []
    chunk_by_key = rows_by_key(chunk.get("results"))
    import_by_source_path = {str(row.get("source_json_path") or ""): row for row in import_diagnostics}
    import_by_package = {str(row.get("package_id") or ""): row for row in import_diagnostics}
    review_by_paper, _completed = _review_event_indexes(review_events)

    rows: list[dict[str, Any]] = []
    for index, conversion_row in enumerate(conversion_rows):
        if not isinstance(conversion_row, Mapping):
            continue
        key = row_key(conversion_row)
        identity = str(conversion_row.get("identity") or "")
        source_role = str(conversion_row.get("source_role") or "")
        acquisition_row = acquisition_by_key.get(key, {})
        loader_row = loader_by_key.get(key, {})
        chunk_row = chunk_by_key.get(key, {})
        package_id = str(chunk_row.get("package_key") or package_id_for(conversion_row))
        import_row = import_by_source_path.get(str(chunk_row.get("json_path") or "")) or import_by_package.get(package_id) or {}
        selection_ref = refs_by_identity.get(identity, {})
        variant_record = variants_by_identity_role.get((identity, source_role), {})
        article_record = variant_record.get("article") if isinstance(variant_record.get("article"), Mapping) else {}
        variant = variant_record.get("variant") if isinstance(variant_record.get("variant"), Mapping) else {}
        review_event = review_by_paper.get(package_id, {})
        parser_ready = conversion_row.get("parser_ready") is True
        chunk_count = int(chunk_row.get("chunk_count") or 0)
        stage_map = {
            "url_intake": _stage(
                str(selection_ref.get("catalog_status") or "typed_catalog_blocker"),
                evidence_path=DEFAULT_SELECTION.as_posix(),
                json_path="$.requested_refs",
                diagnostic_code=str(selection_ref.get("typed_blocker_code") or "catalog_selection_recorded"),
                notes="metadata-only URL intake; network_fetch_attempted=false",
            ),
            "article_catalog": _stage(
                str(article_record.get("catalog_resolution") or selection_ref.get("catalog_resolution") or "typed_catalog_blocker"),
                evidence_path=DEFAULT_SELECTION.as_posix(),
                json_path="$.articles / $.requested_refs",
                diagnostic_code=str(selection_ref.get("typed_blocker_code") or "catalog_index_lookup_required"),
                notes="article catalog state is local metadata/index evidence only",
            ),
            "source_acquisition": _stage(
                str(acquisition_row.get("terminal_state") or acquisition_row.get("status") or "missing"),
                evidence_path=DEFAULT_ACQUISITION_SUMMARY.as_posix(),
                json_path=str(acquisition_row.get("json_path") or "$.results"),
                diagnostic_code=str(acquisition_row.get("diagnostic_code") or acquisition_row.get("blocker_code") or "missing_acquisition_row"),
                notes="local catalog-backed acquisition; no network fetch",
            ),
            "loader_evidence": _stage(
                str(loader_row.get("terminal_state") or loader_row.get("status") or "missing"),
                evidence_path=DEFAULT_LOADER_SUMMARY.as_posix(),
                json_path=str(loader_row.get("json_path") or "$.results"),
                diagnostic_code=str(loader_row.get("diagnostic_code") or loader_row.get("blocker_code") or "missing_loader_row"),
                notes="metadata-only loader evidence; no raw payload embedded",
            ),
            "parser_conversion": _stage(
                str(conversion_row.get("terminal_state") or conversion_row.get("status") or "missing"),
                evidence_path=DEFAULT_CONVERSION_SUMMARY.as_posix(),
                json_path=str(conversion_row.get("json_path") or f"$.results[{index}]"),
                diagnostic_code=str(conversion_row.get("diagnostic_code") or conversion_row.get("refusal_code") or "missing_conversion_diagnostic"),
                notes="parser_ready=true only for converted local bounded text; graph import remains blocked",
                counts={"parser_ready": parser_ready},
            ),
            "chunking": _stage(
                str(chunk_row.get("terminal_state") or chunk_row.get("status") or "missing"),
                evidence_path=DEFAULT_CHUNK_SUMMARY.as_posix(),
                json_path=str(chunk_row.get("json_path") or "$.results"),
                diagnostic_code=str(chunk_row.get("diagnostic_code") or chunk_row.get("refusal_code") or "missing_chunk_diagnostic"),
                notes="chunk text and embeddings are omitted; import_eligible_chunk_count=0",
                counts={"chunk_count": chunk_count, "import_eligible_chunk_count": int(chunk_row.get("import_eligible_chunk_count") or 0)},
            ),
            "graph_readiness_review": _stage(
                str(review_event.get("review_status") or chunk_row.get("review_status") or "not_applicable_zero_chunk_refusal"),
                evidence_path=DEFAULT_REVIEW_EVENTS.as_posix() if parser_ready else DEFAULT_CHUNK_SUMMARY.as_posix(),
                json_path="$.review_events" if parser_ready else str(chunk_row.get("json_path") or "$.results"),
                diagnostic_code="completed_independent_graph_readiness_review_required" if parser_ready else str(chunk_row.get("refusal_code") or "not_applicable_zero_chunk_refusal"),
                notes="ok_for_graph/trusted_graph labels are structural states only until independent semantic review is complete" if parser_ready else "zero-chunk row is not eligible for review",
                counts={
                    "independent_review_completed": bool(review_event.get("independent_review_completed") is True),
                    "output_contract_completed": bool(review_event.get("output_contract_completed") is True),
                },
            ),
            "graph_import_boundary": _stage(
                "refused" if import_row else "missing_refusal",
                evidence_path=DEFAULT_IMPORT_DIAGNOSTICS.as_posix(),
                json_path=str(import_row.get("json_path") or "$.candidates"),
                diagnostic_code=str(import_row.get("diagnostic_code") or "M031_IMPORT_BOUNDARY_REFUSAL_MISSING"),
                notes="refusal-only no-write rehearsal; accepted=false import_eligible=false LadybugDB writes=false",
                counts={"accepted": bool(import_row.get("accepted") is True), "import_eligible": bool(import_row.get("import_eligible") is True)},
            ),
        }
        rows.append(
            {
                "row_id": f"M031-PROGRESSION-{index + 1:03d}",
                "identity": identity,
                "article_ref": conversion_row.get("article_ref"),
                "source_role": source_role,
                "variant_id": conversion_row.get("variant_id") or variant.get("variant_id"),
                "package_id": package_id,
                "requested_ref_id": selection_ref.get("ref_id"),
                "safe_local_path": _safe_path(acquisition_row.get("local_path") or loader_row.get("local_path") or variant.get("local_path")),
                "parser_ready": parser_ready,
                "chunk_count": chunk_count,
                "review_state": stage_map["graph_readiness_review"]["status"],
                "import_boundary_state": stage_map["graph_import_boundary"]["status"],
                "import_refusal_reasons": import_row.get("refusal_reasons") or [],
                "fail_closed_flags": {
                    "network_fetch_attempted": False,
                    "graph_import_allowed": False,
                    "trusted_kg_import_allowed": False,
                    "production_import_attempted": False,
                    "ladybugdb_written": False,
                    "graph_write_attempted": False,
                    "production_persistence_attempted": False,
                    "raw_text_included": False,
                    "chunk_text_included": False,
                    "embeddings_included": False,
                    "vectors_included": False,
                },
                "stages": stage_map,
            }
        )

    matrix = {
        "schema_version": PROGRESSION_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "row_count": len(rows),
        "required_stage_order": STAGE_ORDER,
        "stage_count_per_row": {row["row_id"]: len(row["stages"]) for row in rows},
        "summary_counts": {
            "requested_ref_count": selection.get("counts", {}).get("requested_ref_count"),
            "source_variant_or_blocker_count": len(rows),
            "parser_ready_row_count": sum(1 for row in rows if row["parser_ready"] is True),
            "zero_chunk_refusal_count": sum(1 for row in rows if row["chunk_count"] == 0),
            "pending_graph_readiness_review_count": inputs["chunk"].get("pending_graph_readiness_review_count"),
            "import_rejected_count": inputs["import_summary"].get("rejected_count"),
            "import_eligible_count": inputs["import_summary"].get("import_eligible_count"),
        },
        "fail_closed_flags": {
            "network_fetch_attempted": False,
            "graph_import_allowed": False,
            "trusted_kg_import_allowed": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "graph_write_attempted": False,
            "production_persistence_attempted": False,
            "raw_text_included": False,
            "chunk_text_included": False,
            "embeddings_included": False,
            "vectors_included": False,
        },
        "structural_route_label_notice": "ok_for_graph and trusted_graph route labels are structural states only while independent semantic review remains incomplete; they are not graph import approval.",
        "rows": rows,
    }
    return matrix


def build_continuity_audit(matrix: Mapping[str, Any], inputs: Mapping[str, Any], *, review_events: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    stage_contract = [
        {
            "stage_id": "url_intake",
            "owner": "S01/S02 catalog-backed selection",
            "evidence": [DEFAULT_SELECTION.as_posix()],
            "verifiers": ["scripts/verify_m031_catalog_backed_replay.py"],
            "failure_modes": ["missing requested ref", "typed catalog blocker suppressed", "network fetch attempted"],
        },
        {
            "stage_id": "article_catalog",
            "owner": "article catalog index lookup",
            "evidence": [DEFAULT_SELECTION.as_posix(), DEFAULT_REPLAY_CLOSEOUT.as_posix()],
            "verifiers": ["scripts/verify_m031_catalog_backed_replay.py"],
            "failure_modes": ["catalog JSON absent", "index path drift", "placeholder treated as cataloged"],
        },
        {
            "stage_id": "source_acquisition",
            "owner": "S02 local source acquisition replay",
            "evidence": [DEFAULT_ACQUISITION_SUMMARY.as_posix()],
            "verifiers": ["scripts/verify_m031_catalog_backed_replay.py"],
            "failure_modes": ["missing local source path", "hash/size drift", "unexpected network fetch"],
        },
        {
            "stage_id": "loader_evidence",
            "owner": "S02 loader evidence replay",
            "evidence": [DEFAULT_LOADER_SUMMARY.as_posix()],
            "verifiers": ["scripts/verify_m031_catalog_backed_replay.py"],
            "failure_modes": ["loader row absent", "metadata-only PDF treated as parsed text", "raw payload embedded"],
        },
        {
            "stage_id": "parser_conversion",
            "owner": "S03 parser conversion replay",
            "evidence": [DEFAULT_CONVERSION_SUMMARY.as_posix(), DEFAULT_CONVERSION_CLOSEOUT.as_posix()],
            "verifiers": ["scripts/verify_m031_parser_conversion_replay.py"],
            "failure_modes": ["parser-ready count drift", "low-quality HTML promoted", "permissive graph flag"],
        },
        {
            "stage_id": "chunking",
            "owner": "S04 chunk evidence replay",
            "evidence": [DEFAULT_CHUNK_SUMMARY.as_posix(), DEFAULT_CHUNK_CLOSEOUT.as_posix()],
            "verifiers": ["scripts/verify_m031_chunk_evidence_replay.py"],
            "failure_modes": ["missing zero-chunk refusal", "chunk text leaked", "import-eligible chunks claimed"],
        },
        {
            "stage_id": "graph_readiness_review",
            "owner": "S04 independent review handoff",
            "evidence": [DEFAULT_REVIEW_EVENTS.as_posix(), DEFAULT_REVIEW_SUMMARY.as_posix(), DEFAULT_REVIEW_BUNDLE.as_posix()],
            "verifiers": ["research_graph.graph.readiness.review validate-only", "scripts/verify_m031_process_continuity_audit.py"],
            "failure_modes": ["completed review claimed without verdict", "review placeholders accepted", "structural route label treated as semantic approval"],
        },
        {
            "stage_id": "graph_import_boundary",
            "owner": "S05 refusal-only import boundary rehearsal",
            "evidence": [DEFAULT_IMPORT_SUMMARY.as_posix(), DEFAULT_IMPORT_DIAGNOSTICS.as_posix(), DEFAULT_IMPORT_REPORT.as_posix()],
            "verifiers": ["scripts/replay_m031_import_boundary_rehearsal.py", "scripts/verify_m031_process_continuity_audit.py"],
            "failure_modes": ["missing refusal artifact", "accepted/import-eligible count above zero", "LadybugDB write flag true"],
        },
    ]
    audit = {
        "schema_version": AUDIT_SCHEMA_VERSION,
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "selection_id": SELECTION_ID,
        "artifact_kind": "m031_final_progression_and_continuity_checkpoint",
        "progression_matrix_path": DEFAULT_MATRIX_JSON.as_posix(),
        "row_count": matrix.get("row_count"),
        "stage_order": STAGE_ORDER,
        "stage_contract": stage_contract,
        "source_artifacts": [
            {"path": DEFAULT_SELECTION.as_posix(), "role": "url intake and article catalog selection"},
            {"path": DEFAULT_ACQUISITION_SUMMARY.as_posix(), "role": "source acquisition evidence"},
            {"path": DEFAULT_LOADER_SUMMARY.as_posix(), "role": "loader evidence"},
            {"path": DEFAULT_CONVERSION_SUMMARY.as_posix(), "role": "parser conversion replay"},
            {"path": DEFAULT_CHUNK_SUMMARY.as_posix(), "role": "chunk/evidence replay"},
            {"path": DEFAULT_REVIEW_EVENTS.as_posix(), "role": "pending independent review events"},
            {"path": DEFAULT_IMPORT_SUMMARY.as_posix(), "role": "refusal-only import rehearsal summary"},
            {"path": DEFAULT_IMPORT_DIAGNOSTICS.as_posix(), "role": "per-candidate import refusal diagnostics"},
        ],
        "unsafe_claims_to_preserve": [
            "Do not claim parser readiness for low-quality, metadata-only, blocked, or placeholder-pruned rows.",
            "Do not claim chunk readiness for zero-chunk refusal rows.",
            "Do not treat ok_for_graph or trusted_graph labels as semantic graph approval before independent review verdict evidence exists.",
            "Do not enable graph_import_allowed, trusted_kg_import_allowed, production_import_attempted, graph_write_attempted, production_persistence_attempted, or ladybugdb_written.",
            "Do not embed raw article text, chunk text, PDF bytes, HTML, embeddings, vectors, secrets, or optimizer traces in checkpoint artifacts.",
        ],
        "fail_closed_flags": dict(matrix.get("fail_closed_flags") or {}),
        "structural_route_label_notice": matrix.get("structural_route_label_notice"),
        "review_verdict_state": {
            "event_count": len(review_events),
            "completed_review_event_count": sum(1 for event in review_events if event.get("output_contract_completed") is True or event.get("independent_review_completed") is True),
            "verdict_event_count": sum(1 for event in review_events if str(event.get("verdict") or "").upper() in {"PASS", "FLAG", "REPAIR", "BLOCKER"}),
        },
        "failure_modes_gate_q5": [
            "Filesystem: missing JSON/JSONL/Markdown artifacts fail with M031_CONTINUITY_INPUT_MISSING before writes.",
            "Malformed artifacts: invalid JSON/JSONL fail with M031_CONTINUITY_INVALID_JSON or M031_CONTINUITY_INVALID_JSONL.",
            "Stale counts or rows: missing seven-row/stage evidence fails with M031_CONTINUITY_ROW_COUNT or M031_CONTINUITY_STAGE_EVIDENCE.",
            "Permissive graph/import/LadybugDB flags fail with M031_UNSAFE_FAIL_CLOSED_FLAG or M031_IMPORT_BOUNDARY_PERMISSIVE.",
            "Completed-review claims without PASS/FLAG/REPAIR/BLOCKER verdict evidence fail with M031_COMPLETED_REVIEW_WITHOUT_VERDICT.",
        ],
        "load_profile_gate_q6": {
            "expected_load": "7 progression rows, 8 stages per row, 8 source/checkpoint artifacts, 7 import refusal diagnostics",
            "ten_x_breakpoint": "local JSON/Markdown serialization and recursive metadata scanning saturate first at about 70 rows; there is no network, subprocess, model, graph, or LadybugDB runtime path",
            "protection": "single-pass row joins by deterministic keys, bounded local files, no raw payload reads, no remote calls, no background processes, no database writes",
        },
        "negative_tests_gate_q7": [
            "missing stage evidence",
            "missing progression row",
            "unsafe permissive flags",
            "raw payload leakage",
            "missing import-boundary refusal artifacts",
            "completed-review claim without verdict evidence",
        ],
        "diagnostic_contract": {
            "required_stage_order": STAGE_ORDER,
            "required_row_count": EXPECTED_ROW_COUNT,
            "stable_failure_codes": [
                "M031_CONTINUITY_INPUT_MISSING",
                "M031_CONTINUITY_INVALID_JSON",
                "M031_CONTINUITY_INVALID_JSONL",
                "M031_CONTINUITY_ROW_COUNT",
                "M031_CONTINUITY_STAGE_EVIDENCE",
                "M031_IMPORT_BOUNDARY_REFUSAL_ARTIFACT_MISSING",
                "M031_IMPORT_BOUNDARY_PERMISSIVE",
                "M031_UNSAFE_FAIL_CLOSED_FLAG",
                "M031_COMPLETED_REVIEW_WITHOUT_VERDICT",
                "M031_RAW_PAYLOAD_LEAKAGE",
            ],
        },
    }
    return audit


def validate_progression_matrix(matrix: Mapping[str, Any]) -> list[str]:
    errors: list[str] = []
    if matrix.get("schema_version") != PROGRESSION_SCHEMA_VERSION:
        errors.append("M031_CONTINUITY_SCHEMA $.schema_version: unexpected progression schema")
    if matrix.get("milestone_id") != MILESTONE_ID or matrix.get("slice_id") != SLICE_ID:
        errors.append("M031_CONTINUITY_MILESTONE $.milestone_id: unexpected milestone/slice")
    rows = matrix.get("rows")
    if not isinstance(rows, list):
        errors.append("M031_CONTINUITY_ROW_COUNT $.rows: rows must be a list")
        rows = []
    if len(rows) != EXPECTED_ROW_COUNT or matrix.get("row_count") != EXPECTED_ROW_COUNT:
        errors.append(f"M031_CONTINUITY_ROW_COUNT $.rows: expected {EXPECTED_ROW_COUNT} rows")
    seen_ids: set[str] = set()
    for row_index, row in enumerate(rows):
        if not isinstance(row, Mapping):
            errors.append(f"M031_CONTINUITY_ROW $.rows[{row_index}]: row must be an object")
            continue
        row_id = str(row.get("row_id") or f"$.rows[{row_index}]")
        if row_id in seen_ids:
            errors.append(f"M031_CONTINUITY_ROW $.rows[{row_index}]: duplicate row_id {row_id}")
        seen_ids.add(row_id)
        stages = row.get("stages")
        if not isinstance(stages, Mapping):
            errors.append(f"M031_CONTINUITY_STAGE_EVIDENCE $.rows[{row_index}].stages: stages must be an object")
            continue
        for stage_id in STAGE_ORDER:
            stage = stages.get(stage_id)
            if not isinstance(stage, Mapping):
                errors.append(f"M031_CONTINUITY_STAGE_EVIDENCE $.rows[{row_index}].stages.{stage_id}: missing required stage")
                continue
            for field in ("status", "evidence_path", "json_path", "diagnostic_code"):
                if not isinstance(stage.get(field), str) or not stage[field].strip():
                    errors.append(f"M031_CONTINUITY_STAGE_EVIDENCE $.rows[{row_index}].stages.{stage_id}.{field}: missing field")
        if row.get("import_boundary_state") != "refused":
            errors.append(f"M031_IMPORT_BOUNDARY_PERMISSIVE $.rows[{row_index}].import_boundary_state: graph import boundary must be refused")
        stage_text = json.dumps(stages, sort_keys=True)
        if any(label in stage_text for label in POSITIVE_STRUCTURAL_LABELS):
            notice = str(matrix.get("structural_route_label_notice") or "")
            if "structural states only" not in notice or "independent semantic review" not in notice:
                errors.append("M031_STRUCTURAL_LABEL_NOTICE $.structural_route_label_notice: positive route labels need structural-only warning")
    errors.extend(validate_fail_closed_flags(matrix, where="progression_matrix"))
    errors.extend(validate_no_payload_leakage(matrix, rendered=json.dumps(matrix, sort_keys=True), where="progression_matrix"))
    return errors


def validate_continuity_audit(audit: Mapping[str, Any]) -> list[str]:
    errors: list[str] = []
    if audit.get("schema_version") != AUDIT_SCHEMA_VERSION:
        errors.append("M031_CONTINUITY_SCHEMA $.schema_version: unexpected audit schema")
    if audit.get("row_count") != EXPECTED_ROW_COUNT:
        errors.append(f"M031_CONTINUITY_ROW_COUNT $.row_count: expected {EXPECTED_ROW_COUNT}")
    if audit.get("stage_order") != STAGE_ORDER:
        errors.append("M031_CONTINUITY_STAGE_EVIDENCE $.stage_order: stage order is incomplete")
    contracts = audit.get("stage_contract")
    if not isinstance(contracts, list):
        errors.append("M031_CONTINUITY_STAGE_EVIDENCE $.stage_contract: must be a list")
        contracts = []
    by_stage = {row.get("stage_id"): row for row in contracts if isinstance(row, Mapping)}
    for stage_id in STAGE_ORDER:
        row = by_stage.get(stage_id)
        if not isinstance(row, Mapping):
            errors.append(f"M031_CONTINUITY_STAGE_EVIDENCE $.stage_contract.{stage_id}: missing stage contract")
            continue
        for field in ("owner", "evidence", "verifiers", "failure_modes"):
            value = row.get(field)
            if field == "owner":
                if not isinstance(value, str) or not value.strip():
                    errors.append(f"M031_CONTINUITY_STAGE_EVIDENCE $.stage_contract.{stage_id}.{field}: missing owner")
            elif not isinstance(value, list) or not any(isinstance(item, str) and item.strip() for item in value):
                errors.append(f"M031_CONTINUITY_STAGE_EVIDENCE $.stage_contract.{stage_id}.{field}: missing non-empty list")
        if isinstance(row.get("failure_modes"), list) and len(row["failure_modes"]) < 2:
            errors.append(f"M031_CONTINUITY_STAGE_EVIDENCE $.stage_contract.{stage_id}.failure_modes: expected at least two modes")
    if not isinstance(audit.get("unsafe_claims_to_preserve"), list) or len(audit.get("unsafe_claims_to_preserve", [])) < 4:
        errors.append("M031_CONTINUITY_UNSAFE_CLAIMS $.unsafe_claims_to_preserve: missing unsafe claims")
    notice = str(audit.get("structural_route_label_notice") or "")
    if "ok_for_graph" not in notice or "trusted_graph" not in notice or "structural states only" not in notice:
        errors.append("M031_STRUCTURAL_LABEL_NOTICE $.structural_route_label_notice: missing structural route label explanation")
    review_state = audit.get("review_verdict_state")
    if isinstance(review_state, Mapping) and review_state.get("completed_review_event_count", 0) and not review_state.get("verdict_event_count", 0):
        errors.append("M031_COMPLETED_REVIEW_WITHOUT_VERDICT $.review_verdict_state: completed review lacks verdict")
    errors.extend(validate_fail_closed_flags(audit, where="continuity_audit"))
    errors.extend(validate_no_payload_leakage(audit, rendered=json.dumps(audit, sort_keys=True), where="continuity_audit"))
    return errors


def validate_fail_closed_flags(value: Any, *, where: str) -> list[str]:
    return [f"{finding['diagnostic_code']} {finding['path']} {finding['json_path']}: {finding['message']}" for finding in collect_unsafe_flags(value, where=where)]


def validate_reports(matrix_md: str, audit_md: str, *, matrix: Mapping[str, Any], audit: Mapping[str, Any]) -> list[str]:
    text = matrix_md + "\n" + audit_md
    errors: list[str] = []
    for phrase in REQUIRED_REPORT_PHRASES:
        if phrase not in text:
            errors.append(f"M031_CONTINUITY_REPORT_COVERAGE report: missing {phrase!r}")
    for stage_id in STAGE_ORDER:
        if stage_id not in text and stage_id.replace("_", " ") not in text.lower():
            errors.append(f"M031_CONTINUITY_REPORT_COVERAGE report: missing stage {stage_id}")
    rows = matrix.get("rows") if isinstance(matrix.get("rows"), list) else []
    for row in rows:
        if isinstance(row, Mapping):
            package_id = str(row.get("package_id") or "")
            if package_id and package_id not in matrix_md:
                errors.append(f"M031_CONTINUITY_REPORT_COVERAGE progression report: missing package {package_id}")
    errors.extend(validate_no_payload_leakage({"matrix_md": matrix_md, "audit_md": audit_md}, rendered=text, where="reports"))
    return errors


def render_progression_matrix_md(matrix: Mapping[str, Any]) -> str:
    rows = matrix.get("rows") if isinstance(matrix.get("rows"), list) else []
    lines = [
        "# M031 Progression Matrix",
        "",
        "Metadata-only per-ref/module progression through M031. No raw article text, chunk text, PDF bytes, HTML, embeddings, vectors, graph facts, or LadybugDB writes are included.",
        "",
        "## Per-Ref / Module Progression Matrix",
        "",
        "| Row | Identity | Source Role | Package | Parser Ready | Chunks | Review State | Import Boundary | Refusal Reasons |",
        "|---|---|---|---|---:|---:|---|---|---|",
    ]
    for row in rows:
        if not isinstance(row, Mapping):
            continue
        lines.append(
            "| {row_id} | `{identity}` | `{source_role}` | `{package_id}` | {parser_ready} | {chunk_count} | `{review_state}` | `{import_boundary_state}` | {reasons} |".format(
                row_id=row.get("row_id"),
                identity=row.get("identity"),
                source_role=row.get("source_role"),
                package_id=row.get("package_id"),
                parser_ready="true" if row.get("parser_ready") is True else "false",
                chunk_count=row.get("chunk_count"),
                review_state=row.get("review_state"),
                import_boundary_state=row.get("import_boundary_state"),
                reasons="; ".join(f"`{reason}`" for reason in row.get("import_refusal_reasons", [])) or "none",
            )
        )
    lines.extend(
        [
            "",
            "## Stage Coverage",
            "",
        ]
    )
    for stage_id in STAGE_ORDER:
        lines.append(f"- `{stage_id}`: present for all seven rows with evidence path, JSON path, diagnostic code, and status.")
    lines.extend(
        [
            "",
            "## Fail-Closed Flags",
            "",
            "- graph_import_allowed=false",
            "- trusted_kg_import_allowed=false",
            "- production_import_attempted=false",
            "- graph_write_attempted=false",
            "- production_persistence_attempted=false",
            "- ladybugdb_written=false",
            "- raw_text_included=false; chunk_text_included=false; embeddings_included=false; vectors_included=false",
            "",
            "## Structural Route Label Notice",
            "",
            "`ok_for_graph` and `trusted_graph` route labels are structural states only while independent semantic review is incomplete. They are not graph import approval, trusted KG approval, or LadybugDB write authorization.",
            "",
        ]
    )
    return "\n".join(lines)


def render_audit_md(audit: Mapping[str, Any]) -> str:
    lines = [
        "# M031 Process Continuity Audit",
        "",
        "Final S05 continuity checkpoint for M031. This is a metadata-only, no-write audit that preserves fail-closed graph import and LadybugDB boundaries.",
        "",
        "## Stage Owners, Evidence, Verifiers, and Failure Modes",
        "",
        "| Stage | Owner | Evidence | Verifiers | Failure Modes |",
        "|---|---|---|---|---|",
    ]
    for row in audit.get("stage_contract", []):
        if not isinstance(row, Mapping):
            continue
        lines.append(
            "| `{stage}` | {owner} | {evidence} | {verifiers} | {failure_modes} |".format(
                stage=row.get("stage_id"),
                owner=row.get("owner"),
                evidence="; ".join(f"`{item}`" for item in row.get("evidence", [])),
                verifiers="; ".join(f"`{item}`" for item in row.get("verifiers", [])),
                failure_modes="; ".join(str(item) for item in row.get("failure_modes", [])),
            )
        )
    lines.extend(["", "## Unsafe Claims to Preserve", ""])
    for claim in audit.get("unsafe_claims_to_preserve", []):
        lines.append(f"- {claim}")
    lines.extend(
        [
            "",
            "## Fail-Closed Flags",
            "",
            "- graph_import_allowed=false",
            "- trusted_kg_import_allowed=false",
            "- production_import_attempted=false",
            "- graph_write_attempted=false",
            "- production_persistence_attempted=false",
            "- ladybugdb_written=false",
            "- raw_text_included=false; chunk_text_included=false; embeddings_included=false; vectors_included=false",
            "",
            "## Structural Route Label Notice",
            "",
            "`ok_for_graph` and `trusted_graph` route labels are structural states only while independent semantic review is incomplete. They are not graph import approval, trusted KG approval, or LadybugDB write authorization.",
            "",
            "## Failure Modes",
            "",
        ]
    )
    for item in audit.get("failure_modes_gate_q5", []):
        lines.append(f"- {item}")
    load = audit.get("load_profile_gate_q6") if isinstance(audit.get("load_profile_gate_q6"), Mapping) else {}
    lines.extend(
        [
            "",
            "## Load Profile",
            "",
            f"- Expected load: {load.get('expected_load')}",
            f"- 10x breakpoint: {load.get('ten_x_breakpoint')}",
            f"- Protection: {load.get('protection')}",
            "",
            "## Negative Tests",
            "",
        ]
    )
    for item in audit.get("negative_tests_gate_q7", []):
        lines.append(f"- {item}")
    lines.extend(
        [
            "",
            "## Import Boundary Checkpoint",
            "",
            "The import-boundary rehearsal has seven deterministic `M031_IMPORT_BOUNDARY_REFUSED` diagnostics, zero accepted candidates, zero import-eligible candidates, and no LadybugDB writes.",
            "",
        ]
    )
    return "\n".join(lines)


def load_generation_inputs(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]], str, str]:
    inputs = {
        "selection": load_json(args.selection),
        "acquisition": load_json(args.acquisition_summary),
        "loader": load_json(args.loader_summary),
        "replay_closeout": load_json(args.replay_closeout),
        "conversion": load_json(args.conversion_summary),
        "conversion_closeout": load_json(args.conversion_closeout),
        "chunk": load_json(args.chunk_summary),
        "chunk_closeout": load_json(args.chunk_closeout),
        "import_summary": load_json(args.import_summary),
    }
    review_events = load_jsonl(args.review_events)
    import_diagnostics = load_jsonl(args.import_diagnostics)
    import_report = load_required_text(args.import_report)
    review_summary_text = load_required_text(args.review_summary)
    # Existence-only: do not read the bundle body because it may include chunk samples.
    if not args.review_bundle.exists():
        raise ContinuityAuditError("M031_CONTINUITY_INPUT_MISSING", "required review bundle artifact is missing", path=args.review_bundle)
    return inputs, review_events, import_diagnostics, import_report, review_summary_text


def generate_outputs(args: argparse.Namespace) -> tuple[dict[str, Any], str, dict[str, Any], str]:
    inputs, review_events, import_diagnostics, import_report, review_summary_text = load_generation_inputs(args)
    assert_artifact_contracts(inputs, import_diagnostics=import_diagnostics, review_events=review_events, import_report=import_report, review_summary_text=review_summary_text)
    matrix = build_progression_matrix(inputs, import_diagnostics=import_diagnostics, review_events=review_events)
    audit = build_continuity_audit(matrix, inputs, review_events=review_events)
    matrix_md = render_progression_matrix_md(matrix)
    audit_md = render_audit_md(audit)
    errors = validate_progression_matrix(matrix) + validate_continuity_audit(audit) + validate_reports(matrix_md, audit_md, matrix=matrix, audit=audit)
    if errors:
        raise ContinuityAuditError("M031_CONTINUITY_GENERATED_INVALID", "; ".join(errors[:8]))
    return matrix, matrix_md, audit, audit_md


def validate_existing_outputs(args: argparse.Namespace) -> list[str]:
    errors: list[str] = []
    try:
        matrix = load_json(args.matrix_json)
        audit = load_json(args.audit_json)
        matrix_md = load_required_text(args.matrix_md)
        audit_md = load_required_text(args.audit_md)
    except ContinuityAuditError as exc:
        return [str(exc)]
    errors.extend(validate_progression_matrix(matrix))
    errors.extend(validate_continuity_audit(audit))
    errors.extend(validate_reports(matrix_md, audit_md, matrix=matrix, audit=audit))
    return errors


def run(args: argparse.Namespace) -> int:
    if args.validate_only:
        errors = validate_existing_outputs(args)
        if errors:
            sys.stderr.write("M031 process continuity audit validation failed:\n")
            for error in errors:
                sys.stderr.write(f"- {error}\n")
            return 1
        sys.stdout.write("M031 process continuity audit validation passed: rows=7 stages=8 fail_closed=true import_refusal_artifacts=true.\n")
        return 0

    try:
        matrix, matrix_md, audit, audit_md = generate_outputs(args)
    except ContinuityAuditError as exc:
        sys.stderr.write(f"M031 process continuity audit generation failed:\n- {exc}\n")
        return 2

    write_json(args.matrix_json, matrix)
    write_text(args.matrix_md, matrix_md)
    write_json(args.audit_json, audit)
    write_text(args.audit_md, audit_md)

    errors = validate_existing_outputs(args)
    if errors:
        sys.stderr.write("M031 process continuity audit validation failed after write:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1
    sys.stdout.write(
        "M031 process continuity audit generated and validated: "
        "rows=7 stages=8 matrix_json=true audit_json=true fail_closed=true import_refusal_artifacts=true.\n"
    )
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
