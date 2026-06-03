#!/usr/bin/env python3
"""Validate M028 smoke replay closeout artifacts without rewriting them.

The verifier consumes the metadata-only closeout summary, stage events, and
markdown report produced by ``replay_m028_smoke_closeout.py``. It fails closed
on stale scope, missing provenance, unsafe claims, payload leakage, path/hash
drift, or missing report evidence while preserving the inspected artifacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Sequence

MILESTONE_ID = "M028-8hwqjk"
SLICE_ID = "S06"
EXPECTED_SCHEMA_VERSION = "m028.smoke-closeout-summary.v1"
EXPECTED_EVENT_SCHEMA_VERSION = "m028.smoke-closeout-stage-event.v1"
EXPECTED_URL_REFS = 21
EXPECTED_NORMALIZED_IDENTITIES = 20
EXPECTED_TERMINAL_EVENTS = 21
EXPECTED_EXPANSION_REFS = [f"R{index:02d}" for index in range(15, 22)]
EXPECTED_DUPLICATE_IDENTITY = "arxiv:2605.20897"
EXPECTED_STAGE_ORDER = [
    "source_acquisition_preflight",
    "S02_build_source_metadata_adapters",
    "S02_verify_source_metadata_adapters",
    "S03_build_pdf_acquisition_diagnostics",
    "S03_verify_pdf_acquisition_diagnostics",
    "S04_build_universal_loader_evidence_bundles",
    "S04_verify_universal_loader_evidence_bundles",
    "S05_build_hermes_digest_projection",
    "S05_verify_hermes_digest_projection",
]
REQUIRED_REPORT_SECTIONS = [
    "## Metadata-only Boundary",
    "## Source Acquisition Preflight",
    "## Stage Replay",
    "## Safety Flags",
    "## Failure Modes",
    "## Load Profile",
    "## Negative Tests",
    "## Diagnostics",
]

# Keep these broad enough to catch leaks but specific enough not to reject safe
# report prose such as "does not perform graph imports".
FORBIDDEN_VALUE_MARKERS = (
    "<html",
    "</html>",
    "<!doctype html",
    "%pdf-",
    "raw_article_text=",
    "raw_pdf_bytes=",
    "raw_text:",
    "raw_payload:",
    "source_body:",
    "body_text:",
    "html_document:",
    "pdf_bytes:",
    "chunk_text:",
    "chunk_payload:",
    "model_output:",
    "trusted_fact=true",
    "graph_ready=true",
    "kg_ready=true",
    "parser_ready=true",
    "promoted_to_fact=true",
    "ladybugdb_written=true",
)
PAYLOAD_KEY_NAMES = {
    "raw_article_text",
    "raw_pdf_bytes",
    "raw_text",
    "raw_payload",
    "source_body",
    "body_text",
    "html_document",
    "pdf_bytes",
    "binary_payload",
    "chunk_text",
    "chunk_payload",
    "model_output",
}
UNSAFE_CLAIM_PATTERNS = (
    re.compile(r"\b(?:kg|graph|parser|chunker)[_-]?(?:ready|readiness)\s*(?:=|:)?\s*(?:true|pass|passed|ready)\b"),
    re.compile(r"\b(?:ladybugdb|graph|kg|production)[_-]?(?:write|written|import|persistence|persisted|enabled)\s*(?:=|:)?\s*(?:true|yes|enabled|pass|passed)\b"),
    re.compile(r"\b(?:model|crawler|network)[_-]?(?:call|fetch|attempted|enabled)\s*(?:=|:)?\s*(?:true|yes|enabled|pass|passed)\b"),
    re.compile(r"\b(?:trusted_fact|promoted_to_fact|import_eligible)\s*(?:=|:)?\s*(?:true|yes|[1-9])\b"),
)
UNSAFE_FLAG_NAMES = (
    "graph_write_attempted",
    "graph_write_attempted_in_replay",
    "kg_readiness_claimed",
    "kg_readiness_claimed_in_replay",
    "parser_readiness_claimed",
    "parser_readiness_claimed_in_replay",
    "production_write_attempted",
    "production_persistence_attempted",
    "production_persistence_attempted_in_replay",
    "production_import_attempted",
    "ladybugdb_written",
    "model_call_attempted",
    "model_calls_attempted",
    "network_fetch_attempted",
    "network_calls_attempted",
    "crawler_attempted",
    "parser_attempted",
    "chunker_attempted",
    "dspy_attempted",
    "rlm_attempted",
    "minimax_attempted",
    "raw_payload_embedded",
    "raw_article_text_embedded",
    "raw_article_text_embedded_in_summary",
    "raw_pdf_bytes_embedded",
    "binary_payload_embedded",
    "binary_payload_embedded_in_summary",
    "html_source_embedded",
    "source_payload_embedded",
    "chunk_content_embedded",
    "chunk_payload_embedded",
    "model_output_embedded",
    "kg_ready_claimed",
    "graph_ready_claimed",
)
UNSAFE_COUNTER_NAMES = tuple(UNSAFE_FLAG_NAMES) + (
    "unsafe_claim_count",
    "unsafe_counter_total",
    "import_eligible_count",
    "promoted_to_fact_count",
    "graph_write_count",
    "ladybugdb_write_count",
    "model_call_count",
    "crawler_call_count",
    "network_fetch_count",
    "hermes_digest_count",
    "hermes_digest_generated",
)


@dataclass(frozen=True)
class Diagnostic:
    code: str
    json_path: str
    message: str

    def format(self) -> str:
        return f"{self.code} {self.json_path} {self.message}"


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def resolve_input_path(path: Path, *, fallback_prefix: str = "smoke-replay-closeout-") -> Path:
    """Resolve repo-relative input path, allowing T01's shorter closeout-* names."""
    root = repo_root()
    candidate = path if path.is_absolute() else root / path
    if candidate.exists():
        return candidate
    name = candidate.name
    if name.startswith(fallback_prefix):
        fallback = candidate.with_name(name.replace(fallback_prefix, "closeout-", 1))
        if fallback.exists():
            return fallback
    return candidate


def json_path(parent: str, key: str | int) -> str:
    if isinstance(key, int):
        return f"{parent}[{key}]"
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key):
        return f"{parent}.{key}"
    return f"{parent}[{json.dumps(key)}]"


def read_json(path: Path) -> tuple[dict[str, Any] | None, list[Diagnostic]]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return None, [Diagnostic("INPUT_MISSING", "$", f"missing JSON input: {path.as_posix()}")]
    except json.JSONDecodeError as exc:
        return None, [Diagnostic("JSON_MALFORMED", "$", f"malformed JSON at {exc.lineno}:{exc.colno}")]
    if not isinstance(payload, dict):
        return None, [Diagnostic("JSON_OBJECT_REQUIRED", "$", "top-level JSON must be an object")]
    return payload, []


def read_jsonl(path: Path) -> tuple[list[dict[str, Any]], list[Diagnostic]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        return [], [Diagnostic("INPUT_MISSING", "$", f"missing JSONL input: {path.as_posix()}")]
    rows: list[dict[str, Any]] = []
    diagnostics: list[Diagnostic] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            diagnostics.append(Diagnostic("JSONL_MALFORMED", f"$[{line_number}]", f"malformed JSONL at column {exc.colno}"))
            continue
        if not isinstance(row, dict):
            diagnostics.append(Diagnostic("JSONL_OBJECT_REQUIRED", f"$[{line_number}]", "JSONL row must be an object"))
            continue
        rows.append(row)
    return rows, diagnostics


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def is_repo_relative_path(path_value: Any) -> bool:
    if not isinstance(path_value, str) or not path_value.strip() or "://" in path_value:
        return False
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    return not normalized.is_absolute() and ".." not in normalized.parts and all(part for part in normalized.parts)


def path_on_disk(path_value: str) -> Path | None:
    if not is_repo_relative_path(path_value):
        return None
    root = repo_root().resolve()
    candidate = (root / path_value).resolve()
    try:
        candidate.relative_to(root)
    except ValueError:
        return None
    return candidate


def iter_nodes(payload: Any, path: str = "$", key: str | None = None) -> Iterable[tuple[str, str | None, Any]]:
    yield path, key, payload
    if isinstance(payload, dict):
        for child_key, value in payload.items():
            yield from iter_nodes(value, json_path(path, str(child_key)), str(child_key))
    elif isinstance(payload, list):
        for index, value in enumerate(payload):
            yield from iter_nodes(value, json_path(path, index), key)


def validate_payload_hygiene(payload: Any, *, label: str, reject_unsafe_claims: bool) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    for path, key, value in iter_nodes(payload):
        key_lower = (key or "").lower()
        if key_lower in PAYLOAD_KEY_NAMES:
            diagnostics.append(Diagnostic("PAYLOAD_KEY_FORBIDDEN", path, f"payload-bearing key is forbidden in {label}: {key}"))
        if isinstance(value, str):
            lower_value = value.lower()
            for marker in FORBIDDEN_VALUE_MARKERS:
                if marker in lower_value:
                    diagnostics.append(Diagnostic("RAW_PAYLOAD_MARKER", path, f"forbidden payload marker in {label}: {marker}"))
            if reject_unsafe_claims:
                for pattern in UNSAFE_CLAIM_PATTERNS:
                    if pattern.search(lower_value):
                        diagnostics.append(Diagnostic("UNSAFE_CLAIM_WORDING", path, f"unsafe readiness/write/import/model/crawler claim in {label}"))
                        break
    return diagnostics


def validate_unsafe_values(payload: Any, *, reject_unsafe_claims: bool) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    if not reject_unsafe_claims:
        return diagnostics
    for path, key, value in iter_nodes(payload):
        if key in UNSAFE_FLAG_NAMES and value is not False and value != 0:
            diagnostics.append(Diagnostic("UNSAFE_FLAG_TRUE", path, f"unsafe flag must be false: {key}"))
        if key in UNSAFE_COUNTER_NAMES:
            counter_value: int | None = None
            if isinstance(value, bool):
                counter_value = int(value)
            elif isinstance(value, int):
                counter_value = value
            if counter_value is not None and counter_value != 0:
                diagnostics.append(Diagnostic("UNSAFE_COUNTER_NONZERO", path, f"unsafe counter must be zero: {key}={counter_value}"))
    return diagnostics


def validate_hash_record(record: Any, path: str) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    if not isinstance(record, dict):
        return [Diagnostic("HASH_RECORD_OBJECT_REQUIRED", path, "hash record must be an object")]
    path_value = record.get("path")
    if not is_repo_relative_path(path_value):
        diagnostics.append(Diagnostic("ARTIFACT_PATH_UNSAFE", json_path(path, "path"), "artifact path must be repo-relative and non-escaping"))
        return diagnostics
    assert isinstance(path_value, str)
    disk_path = path_on_disk(path_value)
    if disk_path is None or not disk_path.exists():
        diagnostics.append(Diagnostic("ARTIFACT_PATH_MISSING_ON_DISK", json_path(path, "path"), f"artifact path does not exist: {path_value}"))
        return diagnostics
    if record.get("exists") is not True:
        diagnostics.append(Diagnostic("ARTIFACT_EXISTS_FALSE", json_path(path, "exists"), "hash record must mark existing artifact true"))
    if record.get("bytes") != disk_path.stat().st_size:
        diagnostics.append(Diagnostic("ARTIFACT_BYTE_COUNT_MISMATCH", json_path(path, "bytes"), "artifact byte count does not match disk"))
    expected_sha = record.get("sha256")
    if not isinstance(expected_sha, str) or expected_sha != sha256_file(disk_path):
        diagnostics.append(Diagnostic("ARTIFACT_SHA256_MISMATCH", json_path(path, "sha256"), "artifact SHA-256 does not match disk"))
    return diagnostics


def validate_summary(summary: dict[str, Any], events: Sequence[dict[str, Any]]) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    if summary.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        diagnostics.append(Diagnostic("SUMMARY_SCHEMA_MISMATCH", "$.schema_version", "unexpected closeout summary schema"))
    if summary.get("milestone_id") != MILESTONE_ID:
        diagnostics.append(Diagnostic("MILESTONE_ID_MISMATCH", "$.milestone_id", f"expected {MILESTONE_ID}"))
    if summary.get("slice_id") != SLICE_ID:
        diagnostics.append(Diagnostic("SLICE_ID_MISMATCH", "$.slice_id", f"expected {SLICE_ID}"))
    if summary.get("cwd") != ".":
        diagnostics.append(Diagnostic("CWD_NOT_REPO_RELATIVE", "$.cwd", "cwd must be repo-relative '.'"))
    if summary.get("status") != "pass":
        diagnostics.append(Diagnostic("SUMMARY_STATUS_NOT_PASS", "$.status", "summary status must be pass"))
    if summary.get("diagnostics") not in ([], None):
        diagnostics.append(Diagnostic("SUMMARY_DIAGNOSTICS_PRESENT", "$.diagnostics", "passing closeout summary must not carry diagnostics"))

    for path_key in ("corpus_dir", "out_dir", "replay_artifacts_dir"):
        if not is_repo_relative_path(summary.get(path_key)):
            diagnostics.append(Diagnostic("SUMMARY_PATH_UNSAFE", f"$.{path_key}", "summary path must be repo-relative and non-escaping"))

    preflight = summary.get("source_acquisition_preflight")
    if not isinstance(preflight, dict):
        diagnostics.append(Diagnostic("PREFLIGHT_OBJECT_REQUIRED", "$.source_acquisition_preflight", "source preflight object is required"))
    else:
        if preflight.get("status") != "pass":
            diagnostics.append(Diagnostic("PREFLIGHT_STATUS_NOT_PASS", "$.source_acquisition_preflight.status", "source preflight must pass"))
        if preflight.get("url_ref_count") == 14:
            diagnostics.append(Diagnostic("STALE_14_REF_SCOPE", "$.source_acquisition_preflight.url_ref_count", "stale 14-ref scope is not accepted"))
        if preflight.get("url_ref_count") != EXPECTED_URL_REFS:
            diagnostics.append(Diagnostic("URL_REF_COUNT_MISMATCH", "$.source_acquisition_preflight.url_ref_count", f"expected {EXPECTED_URL_REFS} URL refs"))
        if preflight.get("normalized_identity_count") != EXPECTED_NORMALIZED_IDENTITIES:
            diagnostics.append(Diagnostic("NORMALIZED_IDENTITY_COUNT_MISMATCH", "$.source_acquisition_preflight.normalized_identity_count", f"expected {EXPECTED_NORMALIZED_IDENTITIES} normalized identities"))
        if preflight.get("terminal_event_count") != EXPECTED_TERMINAL_EVENTS:
            diagnostics.append(Diagnostic("TERMINAL_EVENT_COUNT_MISMATCH", "$.source_acquisition_preflight.terminal_event_count", f"expected {EXPECTED_TERMINAL_EVENTS} terminal events"))
        if preflight.get("expansion_refs") != EXPECTED_EXPANSION_REFS:
            diagnostics.append(Diagnostic("EXPANSION_REFS_MISMATCH", "$.source_acquisition_preflight.expansion_refs", "expected expanded refs R15-R21"))
        if preflight.get("duplicate_identity") != EXPECTED_DUPLICATE_IDENTITY or preflight.get("duplicate_identity_ref_count") != 2:
            diagnostics.append(Diagnostic("DUPLICATE_IDENTITY_MISMATCH", "$.source_acquisition_preflight.duplicate_identity", f"expected preserved duplicate {EXPECTED_DUPLICATE_IDENTITY}"))
        for key in ("selection", "source_acquisition_events", "source_acquisition_summary", "acquisition_report"):
            diagnostics.extend(validate_hash_record(preflight.get(key), f"$.source_acquisition_preflight.{key}"))

    summary_stage_events = summary.get("stage_events")
    if not isinstance(summary_stage_events, list):
        diagnostics.append(Diagnostic("SUMMARY_STAGE_EVENTS_LIST_REQUIRED", "$.stage_events", "stage_events must be a list"))
        summary_stage_events = []
    stage_names = [event.get("stage") for event in summary_stage_events if isinstance(event, dict)]
    expected_without_preflight = EXPECTED_STAGE_ORDER[1:]
    if stage_names != expected_without_preflight:
        diagnostics.append(Diagnostic("STAGE_ORDER_MISMATCH", "$.stage_events", "expected S02-S05 build/verify stages in order"))
    if len(events) != len(expected_without_preflight):
        diagnostics.append(Diagnostic("EVENT_COUNT_MISMATCH", "$events", f"expected {len(expected_without_preflight)} stage event rows"))
    event_names = [event.get("stage") for event in events]
    if event_names != expected_without_preflight:
        diagnostics.append(Diagnostic("EVENT_STAGE_ORDER_MISMATCH", "$events", "events JSONL must contain all S02-S05 build/verify stages in order"))
    if len(summary_stage_events) == len(events):
        for index, (summary_event, event) in enumerate(zip(summary_stage_events, events, strict=True)):
            if summary_event != event:
                diagnostics.append(Diagnostic("SUMMARY_EVENT_JSONL_MISMATCH", f"$.stage_events[{index}]", "summary stage event must match JSONL row"))
                break
    diagnostics.extend(validate_unsafe_values(summary, reject_unsafe_claims=True))
    return diagnostics


def validate_event(event: dict[str, Any], index: int, *, reject_unsafe_claims: bool) -> list[Diagnostic]:
    base = f"$events[{index}]"
    diagnostics: list[Diagnostic] = []
    if event.get("schema_version") != EXPECTED_EVENT_SCHEMA_VERSION:
        diagnostics.append(Diagnostic("EVENT_SCHEMA_MISMATCH", json_path(base, "schema_version"), "unexpected stage event schema"))
    if event.get("milestone_id") != MILESTONE_ID:
        diagnostics.append(Diagnostic("EVENT_MILESTONE_ID_MISMATCH", json_path(base, "milestone_id"), f"expected {MILESTONE_ID}"))
    if event.get("slice_id") != SLICE_ID:
        diagnostics.append(Diagnostic("EVENT_SLICE_ID_MISMATCH", json_path(base, "slice_id"), f"expected {SLICE_ID}"))
    if event.get("status") != "pass":
        diagnostics.append(Diagnostic("STAGE_STATUS_NOT_PASS", json_path(base, "status"), "stage status must be pass"))
    if event.get("exit_code") != 0:
        diagnostics.append(Diagnostic("STAGE_EXIT_NONZERO", json_path(base, "exit_code"), "stage exit code must be zero"))
    if event.get("cwd") != ".":
        diagnostics.append(Diagnostic("STAGE_CWD_NOT_REPO_RELATIVE", json_path(base, "cwd"), "stage cwd must be repo-relative '.'"))
    command = event.get("command")
    if not isinstance(command, list) or not command or not all(isinstance(part, str) for part in command):
        diagnostics.append(Diagnostic("STAGE_COMMAND_MISSING", json_path(base, "command"), "stage command must be a non-empty string array"))
    elif any(Path(part).is_absolute() for part in command):
        diagnostics.append(Diagnostic("STAGE_COMMAND_ABSOLUTE_PATH", json_path(base, "command"), "stage command must not contain absolute paths"))
    if not isinstance(event.get("git_commit"), str) or not event.get("git_commit"):
        diagnostics.append(Diagnostic("GIT_COMMIT_MISSING", json_path(base, "git_commit"), "git commit provenance is required"))
    for collection_key in ("input_hashes", "output_hashes"):
        records = event.get(collection_key)
        if not isinstance(records, list) or not records:
            diagnostics.append(Diagnostic("HASH_COLLECTION_MISSING", json_path(base, collection_key), f"{collection_key} must be a non-empty list"))
            continue
        for record_index, record in enumerate(records):
            diagnostics.extend(validate_hash_record(record, json_path(json_path(base, collection_key), record_index)))
    if event.get("diagnostics") not in ([], None):
        diagnostics.append(Diagnostic("STAGE_DIAGNOSTICS_PRESENT", json_path(base, "diagnostics"), "passing stage must not carry diagnostics"))
    diagnostics.extend(validate_payload_hygiene(event, label=f"event {index}", reject_unsafe_claims=reject_unsafe_claims))
    diagnostics.extend(validate_unsafe_values(event, reject_unsafe_claims=reject_unsafe_claims))
    return diagnostics


def validate_report(report_path: Path, *, reject_unsafe_claims: bool) -> list[Diagnostic]:
    try:
        report_text = report_path.read_text(encoding="utf-8", errors="replace")
    except FileNotFoundError:
        return [Diagnostic("INPUT_MISSING", "$report", f"missing report input: {report_path.as_posix()}")]
    diagnostics: list[Diagnostic] = []
    for section in REQUIRED_REPORT_SECTIONS:
        if section not in report_text:
            diagnostics.append(Diagnostic("REPORT_SECTION_MISSING", "$report", f"missing report section: {section}"))
    if MILESTONE_ID not in report_text:
        diagnostics.append(Diagnostic("REPORT_MILESTONE_MISSING", "$report", f"report must mention {MILESTONE_ID}"))
    if SLICE_ID not in report_text:
        diagnostics.append(Diagnostic("REPORT_SLICE_MISSING", "$report", f"report must mention {SLICE_ID}"))
    diagnostics.extend(validate_payload_hygiene(report_text, label="report", reject_unsafe_claims=reject_unsafe_claims))
    return diagnostics


def validate_closeout(summary_path: Path, events_path: Path, report_path: Path, *, reject_unsafe_claims: bool) -> list[Diagnostic]:
    summary_path = resolve_input_path(summary_path)
    events_path = resolve_input_path(events_path)
    report_path = resolve_input_path(report_path)
    summary, diagnostics = read_json(summary_path)
    events, event_diagnostics = read_jsonl(events_path)
    diagnostics.extend(event_diagnostics)
    diagnostics.extend(validate_report(report_path, reject_unsafe_claims=reject_unsafe_claims))
    if summary is None:
        return diagnostics
    diagnostics.extend(validate_payload_hygiene(summary, label="summary", reject_unsafe_claims=reject_unsafe_claims))
    diagnostics.extend(validate_summary(summary, events))
    for index, event in enumerate(events):
        diagnostics.extend(validate_event(event, index, reject_unsafe_claims=reject_unsafe_claims))
    return diagnostics


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summary", type=Path, required=True, help="closeout summary JSON")
    parser.add_argument("--events", type=Path, required=True, help="closeout stage events JSONL")
    parser.add_argument("--report", type=Path, required=True, help="closeout markdown report")
    parser.add_argument("--reject-unsafe-claims", action="store_true", help="fail on any nonzero unsafe flag/counter or unsafe readiness/write wording")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    diagnostics = validate_closeout(args.summary, args.events, args.report, reject_unsafe_claims=args.reject_unsafe_claims)
    if diagnostics:
        for diagnostic in diagnostics:
            print(diagnostic.format(), file=sys.stderr)
        print(f"m028_smoke_closeout_verdict=fail diagnostics={len(diagnostics)}", file=sys.stderr)
        return 1
    print("m028_smoke_closeout_verdict=pass diagnostics=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
