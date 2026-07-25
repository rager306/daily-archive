#!/usr/bin/env python3
"""Replay the M028 mixed-source smoke pipeline into an isolated closeout directory.

The closeout runner composes the completed M028 S02-S05 metadata-only stages
against the fixed smoke corpus. It records per-stage provenance and safety
status while keeping parser, crawler, model, graph, LadybugDB, and production
write behavior fail-closed and out of scope.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path, PurePosixPath
from typing import Any

MILESTONE_ID = "M028-8hwqjk"
SLICE_ID = "S06"
DEFAULT_CORPUS_DIR = Path("data/article_corpora/m028-universal-loader-runtime-smoke-v1")
DEFAULT_OUT_DIRNAME = "smoke-replay-closeout"
EXPECTED_URL_REFS = 21
EXPECTED_NORMALIZED_IDENTITIES = 20
EXPECTED_TERMINAL_EVENTS = 21
EXPECTED_EXPANSION_REFS = [f"R{index:02d}" for index in range(15, 22)]
EXPECTED_DUPLICATE_IDENTITY = "arxiv:2605.20897"

SUMMARY_FILENAME = "smoke-replay-closeout-summary.json"
EVENTS_FILENAME = "smoke-replay-closeout-events.jsonl"
REPORT_FILENAME = "smoke-replay-closeout-report.md"
REPLAY_ARTIFACTS_DIRNAME = "replay-artifacts"
PYTHON_CMD = os.environ.get("PYTHON", "python")

FORBIDDEN_REPORT_MARKERS = (
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

UNSAFE_FLAG_NAMES = (
    "graph_write_attempted",
    "graph_write_attempted_in_replay",
    "kg_readiness_claimed",
    "kg_readiness_claimed_in_replay",
    "parser_readiness_claimed",
    "parser_readiness_claimed_in_replay",
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
    "hermes_digest_count",
    "hermes_digest_generated",
)


@dataclass(frozen=True)
class Diagnostic:
    """Stable closeout diagnostic with an inspectable JSON path."""

    code: str
    json_path: str
    message: str

    def as_dict(self) -> dict[str, str]:
        return {"code": self.code, "json_path": self.json_path, "message": self.message}


class CloseoutError(RuntimeError):
    """Raised when the replay must fail closed."""

    def __init__(self, diagnostics: Sequence[Diagnostic]):
        self.diagnostics = list(diagnostics)
        super().__init__("; ".join(f"{d.code}:{d.json_path}:{d.message}" for d in self.diagnostics))


@dataclass(frozen=True)
class Stage:
    name: str
    argv: list[str]
    input_paths: list[Path]
    output_paths: list[Path]


def utc_now_iso() -> str:
    return datetime.now(UTC).isoformat()


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def repo_relative(path: Path, root: Path) -> str:
    resolved = path.resolve()
    try:
        return resolved.relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def safe_repo_relative(path_value: Any, root: Path) -> tuple[Path | None, Diagnostic | None]:
    if not isinstance(path_value, str) or not path_value.strip():
        return None, Diagnostic(
            "ARTIFACT_PATH_MISSING",
            "$.artifact_path",
            "artifact path must be a non-empty repo-relative string",
        )
    if "://" in path_value:
        return None, Diagnostic(
            "ARTIFACT_PATH_IS_URL", "$.artifact_path", "artifact path must not be a URL"
        )
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    if (
        normalized.is_absolute()
        or ".." in normalized.parts
        or any(part == "" for part in normalized.parts)
    ):
        return None, Diagnostic(
            "ARTIFACT_PATH_UNSAFE", "$.artifact_path", "artifact path must stay repo-relative"
        )
    candidate = (root / normalized.as_posix()).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError:
        return None, Diagnostic(
            "ARTIFACT_PATH_ESCAPES_REPO", "$.artifact_path", "artifact path escapes repository root"
        )
    if not candidate.exists():
        return None, Diagnostic(
            "ARTIFACT_PATH_MISSING_ON_DISK",
            "$.artifact_path",
            f"artifact path does not exist: {normalized.as_posix()}",
        )
    return candidate, None


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_fingerprint(path: Path, root: Path) -> dict[str, Any]:
    return {
        "path": repo_relative(path, root),
        "exists": path.exists(),
        "bytes": path.stat().st_size if path.exists() else None,
        "sha256": sha256_file(path) if path.exists() else None,
    }


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise CloseoutError(
            [
                Diagnostic(
                    "INPUT_MISSING", f"$.inputs.{path.name}", f"missing input: {path.as_posix()}"
                )
            ]
        ) from exc
    except json.JSONDecodeError as exc:
        raise CloseoutError(
            [
                Diagnostic(
                    "JSON_MALFORMED",
                    f"$.inputs.{path.name}",
                    f"malformed JSON at {exc.lineno}:{exc.colno}",
                )
            ]
        ) from exc
    if not isinstance(payload, dict):
        raise CloseoutError(
            [
                Diagnostic(
                    "JSON_OBJECT_REQUIRED",
                    f"$.inputs.{path.name}",
                    "top-level JSON must be an object",
                )
            ]
        )
    return payload


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise CloseoutError(
            [
                Diagnostic(
                    "INPUT_MISSING", f"$.inputs.{path.name}", f"missing input: {path.as_posix()}"
                )
            ]
        ) from exc
    rows: list[dict[str, Any]] = []
    diagnostics: list[Diagnostic] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            diagnostics.append(
                Diagnostic(
                    "JSONL_MALFORMED",
                    f"$.inputs.{path.name}[{line_number}]",
                    f"malformed JSONL at column {exc.colno}",
                )
            )
            continue
        if not isinstance(row, dict):
            diagnostics.append(
                Diagnostic(
                    "JSONL_OBJECT_REQUIRED",
                    f"$.inputs.{path.name}[{line_number}]",
                    "JSONL row must be an object",
                )
            )
            continue
        rows.append(row)
    if diagnostics:
        raise CloseoutError(diagnostics)
    return rows


def nested_items(payload: Any) -> Iterable[tuple[str, Any]]:
    if isinstance(payload, dict):
        for key, value in payload.items():
            yield str(key), value
            yield from nested_items(value)
    elif isinstance(payload, list):
        for value in payload:
            yield from nested_items(value)


def unsafe_status(payload: Any) -> tuple[dict[str, bool], dict[str, int], list[Diagnostic]]:
    flags = dict.fromkeys(UNSAFE_FLAG_NAMES, False)
    counters = dict.fromkeys(UNSAFE_COUNTER_NAMES, 0)
    diagnostics: list[Diagnostic] = []
    for key, value in nested_items(payload):
        if key in flags:
            if value is not False:
                flags[key] = True
                diagnostics.append(
                    Diagnostic("UNSAFE_FLAG_TRUE", f"$.{key}", f"unsafe flag must be false: {key}")
                )
        if key in counters:
            if isinstance(value, bool):
                counter_value = int(value)
            elif isinstance(value, int):
                counter_value = value
            else:
                continue
            if counter_value != 0:
                counters[key] = counter_value
                diagnostics.append(
                    Diagnostic(
                        "UNSAFE_COUNTER_NONZERO",
                        f"$.{key}",
                        f"unsafe counter must be zero: {key}={counter_value}",
                    )
                )
    return flags, counters, diagnostics


def compact_output(text: str, root: Path | None = None, limit: int = 1000) -> str:
    if not text:
        return ""
    if root is not None:
        root_text = root.resolve().as_posix()
        text = text.replace(root_text + "/", "").replace(root_text, ".")
    clean = "\n".join(line[:240] for line in text.splitlines()[:20])
    if len(clean) > limit:
        return clean[:limit] + "…"
    return clean


def git_commit_from_files(root: Path) -> str | None:
    git_dir = root / ".git"
    try:
        if git_dir.is_file():
            content = git_dir.read_text(encoding="utf-8").strip()
            if content.startswith("gitdir:"):
                git_dir = (root / content.split(":", 1)[1].strip()).resolve()
        head = (git_dir / "HEAD").read_text(encoding="utf-8").strip()
        if head.startswith("ref:"):
            ref = head.split(" ", 1)[1]
            ref_path = git_dir / ref
            if ref_path.exists():
                return ref_path.read_text(encoding="utf-8").strip()
            packed_refs = git_dir / "packed-refs"
            if packed_refs.exists():
                for line in packed_refs.read_text(encoding="utf-8", errors="replace").splitlines():
                    if line and not line.startswith("#") and line.endswith(f" {ref}"):
                        return line.split(" ", 1)[0]
            return None
        return head
    except OSError:
        return None


def validate_source_acquisition(
    corpus_dir: Path, root: Path
) -> tuple[dict[str, Any], list[Diagnostic]]:
    diagnostics: list[Diagnostic] = []
    selection_path = corpus_dir / "selection.json"
    events_path = corpus_dir / "source-acquisition-events.jsonl"
    summary_path = corpus_dir / "source-acquisition-summary.json"
    report_path = corpus_dir / "acquisition-report.md"

    selection = read_json(selection_path)
    events = read_jsonl(events_path)
    summary = read_json(summary_path)
    report_text = (
        report_path.read_text(encoding="utf-8", errors="replace") if report_path.exists() else ""
    )

    refs = selection.get("refs")
    if not isinstance(refs, list):
        diagnostics.append(
            Diagnostic(
                "SELECTION_REFS_LIST_REQUIRED", "$.selection.refs", "selection refs must be a list"
            )
        )
        refs = []
    ref_ids = [
        str(ref.get("ref_id")) for ref in refs if isinstance(ref, dict) and ref.get("ref_id")
    ]
    normalized = [
        str(ref.get("normalized_identity"))
        for ref in refs
        if isinstance(ref, dict) and ref.get("normalized_identity")
    ]
    expansion_refs = [ref_id for ref_id in ref_ids if ref_id in EXPECTED_EXPANSION_REFS]

    if len(refs) != EXPECTED_URL_REFS:
        diagnostics.append(
            Diagnostic(
                "URL_REF_COUNT_MISMATCH",
                "$.selection.refs",
                f"expected {EXPECTED_URL_REFS} URL refs, found {len(refs)}",
            )
        )
    if len(set(normalized)) != EXPECTED_NORMALIZED_IDENTITIES:
        diagnostics.append(
            Diagnostic(
                "NORMALIZED_IDENTITY_COUNT_MISMATCH",
                "$.selection.refs",
                f"expected {EXPECTED_NORMALIZED_IDENTITIES} normalized identities, found {len(set(normalized))}",
            )
        )
    if sorted(expansion_refs) != EXPECTED_EXPANSION_REFS:
        diagnostics.append(
            Diagnostic(
                "EXPANSION_REFS_MISMATCH", "$.selection.refs", "expected R15-R21 expansion refs"
            )
        )
    if normalized.count(EXPECTED_DUPLICATE_IDENTITY) != 2:
        diagnostics.append(
            Diagnostic(
                "DUPLICATE_IDENTITY_MISMATCH",
                "$.selection.refs",
                f"expected duplicate {EXPECTED_DUPLICATE_IDENTITY}",
            )
        )

    terminal_events = [event for event in events if event.get("terminal") is True]
    if len(terminal_events) != EXPECTED_TERMINAL_EVENTS:
        diagnostics.append(
            Diagnostic(
                "TERMINAL_EVENT_COUNT_MISMATCH",
                "$.source_acquisition_events",
                f"expected {EXPECTED_TERMINAL_EVENTS} terminal events, found {len(terminal_events)}",
            )
        )
    if len(events) != EXPECTED_URL_REFS:
        diagnostics.append(
            Diagnostic(
                "ACQUISITION_EVENT_COUNT_MISMATCH",
                "$.source_acquisition_events",
                f"expected {EXPECTED_URL_REFS} acquisition events, found {len(events)}",
            )
        )

    for index, event in enumerate(events):
        if event.get("status") != "captured":
            diagnostics.append(
                Diagnostic(
                    "ACQUISITION_NOT_CAPTURED",
                    f"$.source_acquisition_events[{index}].status",
                    "source acquisition event must be captured",
                )
            )
        artifact_path, artifact_diag = safe_repo_relative(event.get("artifact_path"), root)
        if artifact_diag is not None:
            diagnostics.append(
                Diagnostic(
                    artifact_diag.code,
                    f"$.source_acquisition_events[{index}].artifact_path",
                    artifact_diag.message,
                )
            )
            continue
        assert artifact_path is not None
        expected_bytes = event.get("byte_count")
        expected_sha = event.get("sha256")
        if expected_bytes != artifact_path.stat().st_size:
            diagnostics.append(
                Diagnostic(
                    "ARTIFACT_BYTE_COUNT_MISMATCH",
                    f"$.source_acquisition_events[{index}].byte_count",
                    "artifact byte count does not match disk",
                )
            )
        if expected_sha != sha256_file(artifact_path):
            diagnostics.append(
                Diagnostic(
                    "ARTIFACT_SHA256_MISMATCH",
                    f"$.source_acquisition_events[{index}].sha256",
                    "artifact SHA-256 does not match disk",
                )
            )
        _, _, unsafe_diags = unsafe_status(event)
        diagnostics.extend(
            Diagnostic(d.code, f"$.source_acquisition_events[{index}]{d.json_path[1:]}", d.message)
            for d in unsafe_diags
        )

    for label, payload in (("selection", selection), ("source_acquisition_summary", summary)):
        _, _, unsafe_diags = unsafe_status(payload)
        diagnostics.extend(
            Diagnostic(d.code, f"$.{label}{d.json_path[1:]}", d.message) for d in unsafe_diags
        )

    lower_report = report_text.lower()
    for marker in FORBIDDEN_REPORT_MARKERS:
        if marker in lower_report:
            diagnostics.append(
                Diagnostic(
                    "REPORT_FORBIDDEN_MARKER",
                    "$.acquisition_report",
                    f"metadata-only report contains forbidden marker: {marker}",
                )
            )
    if not report_path.exists():
        diagnostics.append(
            Diagnostic(
                "ACQUISITION_REPORT_MISSING",
                "$.acquisition_report",
                "acquisition report is required",
            )
        )

    preflight = {
        "status": "pass" if not diagnostics else "fail",
        "selection": file_fingerprint(selection_path, root),
        "source_acquisition_events": file_fingerprint(events_path, root),
        "source_acquisition_summary": file_fingerprint(summary_path, root),
        "acquisition_report": file_fingerprint(report_path, root),
        "url_ref_count": len(refs),
        "normalized_identity_count": len(set(normalized)),
        "terminal_event_count": len(terminal_events),
        "expansion_refs": expansion_refs,
        "duplicate_identity": EXPECTED_DUPLICATE_IDENTITY,
        "duplicate_identity_ref_count": normalized.count(EXPECTED_DUPLICATE_IDENTITY),
        "safety_flags": dict.fromkeys(UNSAFE_FLAG_NAMES, False),
        "unsafe_counters": dict.fromkeys(UNSAFE_COUNTER_NAMES, 0),
        "diagnostics": [diagnostic.as_dict() for diagnostic in diagnostics],
    }
    return preflight, diagnostics


def stage_event(
    stage: str,
    status: str,
    root: Path,
    git_commit: str | None,
    argv: Sequence[str],
    input_paths: Sequence[Path],
    output_paths: Sequence[Path],
    started_monotonic: float,
    started_at: str,
    exit_code: int,
    stdout: str = "",
    stderr: str = "",
    diagnostics: Sequence[Diagnostic] = (),
) -> dict[str, Any]:
    return {
        "schema_version": "m028.smoke-closeout-stage-event.v1",
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "stage": stage,
        "status": status,
        "command": [str(part) for part in argv],
        "cwd": ".",
        "git_commit": git_commit,
        "started_at": started_at,
        "finished_at": utc_now_iso(),
        "duration_ms": int((time.monotonic() - started_monotonic) * 1000),
        "exit_code": exit_code,
        "input_hashes": [file_fingerprint(path, root) for path in input_paths],
        "output_hashes": [file_fingerprint(path, root) for path in output_paths],
        "safety_flags": {
            "graph_write_attempted": False,
            "kg_readiness_claimed": False,
            "parser_readiness_claimed": False,
            "production_write_attempted": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "model_call_attempted": False,
            "crawler_attempted": False,
            "network_fetch_attempted": False,
            "raw_payload_embedded": False,
        },
        "diagnostics": [diagnostic.as_dict() for diagnostic in diagnostics],
        "stdout_excerpt": compact_output(stdout, root),
        "stderr_excerpt": compact_output(stderr, root),
    }


def build_stages(corpus_dir: Path, replay_dir: Path, root: Path) -> list[Stage]:
    selection = corpus_dir / "selection.json"
    source_acquisition_events = corpus_dir / "source-acquisition-events.jsonl"
    source_metadata_events = replay_dir / "source-metadata-events.jsonl"
    source_metadata_summary = replay_dir / "source-metadata-summary.json"
    pdf_events = replay_dir / "pdf-acquisition-events.jsonl"
    pdf_summary = replay_dir / "pdf-acquisition-summary.json"
    bundles = replay_dir / "universal-loader-evidence-bundles.jsonl"
    bundle_summary = replay_dir / "universal-loader-evidence-summary.json"
    digest = replay_dir / "hermes-digest-projection.json"

    def arg_path(path: Path) -> str:
        return repo_relative(path, root)

    return [
        Stage(
            "S02_build_source_metadata_adapters",
            [
                PYTHON_CMD,
                "scripts/build_m028_source_metadata_adapters.py",
                "--selection",
                arg_path(selection),
                "--acquisition-events",
                arg_path(source_acquisition_events),
                "--out-dir",
                arg_path(replay_dir),
            ],
            [selection, source_acquisition_events],
            [source_metadata_events, source_metadata_summary],
        ),
        Stage(
            "S02_verify_source_metadata_adapters",
            [
                PYTHON_CMD,
                "scripts/verify_m028_source_metadata_adapters.py",
                "--selection",
                arg_path(selection),
                "--acquisition-events",
                arg_path(source_acquisition_events),
                "--metadata-events",
                arg_path(source_metadata_events),
                "--summary",
                arg_path(source_metadata_summary),
                "--reject-unsafe-claims",
            ],
            [selection, source_acquisition_events, source_metadata_events, source_metadata_summary],
            [source_metadata_events, source_metadata_summary],
        ),
        Stage(
            "S03_build_pdf_acquisition_diagnostics",
            [
                PYTHON_CMD,
                "scripts/build_m028_pdf_acquisition_diagnostics.py",
                "--selection",
                arg_path(selection),
                "--acquisition-events",
                arg_path(source_acquisition_events),
                "--metadata-events",
                arg_path(source_metadata_events),
                "--metadata-summary",
                arg_path(source_metadata_summary),
                "--out-dir",
                arg_path(replay_dir),
            ],
            [selection, source_acquisition_events, source_metadata_events, source_metadata_summary],
            [pdf_events, pdf_summary, replay_dir / "pdf-acquisition-report.md"],
        ),
        Stage(
            "S03_verify_pdf_acquisition_diagnostics",
            [
                PYTHON_CMD,
                "scripts/verify_m028_pdf_acquisition_diagnostics.py",
                "--selection",
                arg_path(selection),
                "--events",
                arg_path(pdf_events),
                "--summary",
                arg_path(pdf_summary),
                "--reject-unsafe-claims",
            ],
            [selection, pdf_events, pdf_summary],
            [pdf_events, pdf_summary],
        ),
        Stage(
            "S04_build_universal_loader_evidence_bundles",
            [
                PYTHON_CMD,
                "scripts/build_m028_universal_loader_evidence_bundles.py",
                "--selection",
                arg_path(selection),
                "--source-acquisition-events",
                arg_path(source_acquisition_events),
                "--metadata-events",
                arg_path(source_metadata_events),
                "--metadata-summary",
                arg_path(source_metadata_summary),
                "--pdf-events",
                arg_path(pdf_events),
                "--pdf-summary",
                arg_path(pdf_summary),
                "--out-dir",
                arg_path(replay_dir),
            ],
            [
                selection,
                source_acquisition_events,
                source_metadata_events,
                source_metadata_summary,
                pdf_events,
                pdf_summary,
            ],
            [bundles, bundle_summary, replay_dir / "universal-loader-evidence-report.md"],
        ),
        Stage(
            "S04_verify_universal_loader_evidence_bundles",
            [
                PYTHON_CMD,
                "scripts/verify_m028_universal_loader_evidence_bundles.py",
                "--selection",
                arg_path(selection),
                "--metadata-events",
                arg_path(source_metadata_events),
                "--pdf-events",
                arg_path(pdf_events),
                "--bundles",
                arg_path(bundles),
                "--summary",
                arg_path(bundle_summary),
                "--reject-unsafe-claims",
            ],
            [selection, source_metadata_events, pdf_events, bundles, bundle_summary],
            [bundles, bundle_summary],
        ),
        Stage(
            "S05_build_hermes_digest_projection",
            [
                PYTHON_CMD,
                "scripts/build_m028_hermes_digest_projection.py",
                "--bundles",
                arg_path(bundles),
                "--summary",
                arg_path(bundle_summary),
                "--out-dir",
                arg_path(replay_dir),
            ],
            [bundles, bundle_summary],
            [digest, replay_dir / "hermes-digest-projection-report.md"],
        ),
        Stage(
            "S05_verify_hermes_digest_projection",
            [
                PYTHON_CMD,
                "scripts/verify_m028_hermes_digest_projection.py",
                "--bundles",
                arg_path(bundles),
                "--summary",
                arg_path(bundle_summary),
                "--digest",
                arg_path(digest),
                "--report",
                arg_path(replay_dir / "hermes-digest-projection-report.md"),
                "--reject-unsafe-claims",
            ],
            [bundles, bundle_summary, digest, replay_dir / "hermes-digest-projection-report.md"],
            [digest, replay_dir / "hermes-digest-projection-report.md"],
        ),
    ]


def run_stage(
    stage: Stage, root: Path, git_commit: str | None, timeout_seconds: int
) -> dict[str, Any]:
    started = time.monotonic()
    started_at = utc_now_iso()
    try:
        completed = subprocess.run(
            stage.argv,
            cwd=root,
            text=True,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
        )
        status = "pass" if completed.returncode == 0 else "fail"
        diagnostics = (
            []
            if completed.returncode == 0
            else [
                Diagnostic(
                    "STAGE_EXIT_NONZERO",
                    f"$.stages.{stage.name}.exit_code",
                    f"stage exited {completed.returncode}",
                )
            ]
        )
        return stage_event(
            stage.name,
            status,
            root,
            git_commit,
            stage.argv,
            stage.input_paths,
            stage.output_paths,
            started,
            started_at,
            completed.returncode,
            completed.stdout,
            completed.stderr,
            diagnostics,
        )
    except subprocess.TimeoutExpired as exc:
        return stage_event(
            stage.name,
            "fail",
            root,
            git_commit,
            stage.argv,
            stage.input_paths,
            stage.output_paths,
            started,
            started_at,
            124,
            exc.stdout if isinstance(exc.stdout, str) else "",
            exc.stderr if isinstance(exc.stderr, str) else "",
            [
                Diagnostic(
                    "STAGE_TIMEOUT",
                    f"$.stages.{stage.name}",
                    f"stage exceeded {timeout_seconds}s timeout",
                )
            ],
        )


def render_report(summary: dict[str, Any]) -> str:
    stage_rows = "\n".join(
        f"| {event['stage']} | {event['status']} | {event['exit_code']} | {event['duration_ms']} |"
        for event in summary["stage_events"]
    )
    diagnostics = summary.get("diagnostics", [])
    diagnostic_lines = (
        "\n".join(f"- `{d['code']}` at `{d['json_path']}`: {d['message']}" for d in diagnostics)
        or "- None."
    )
    return f"""# M028 Smoke Replay Closeout

- Milestone: `{MILESTONE_ID}`
- Slice: `{SLICE_ID}`
- Status: `{summary["status"]}`
- Corpus: `{summary["corpus_dir"]}`
- Replay artifacts: `{summary["replay_artifacts_dir"]}`
- Git commit: `{summary.get("git_commit") or "unavailable"}`

## Metadata-only Boundary

This closeout replays only local metadata/provenance stages S02-S05. It does not perform network fetches, live acquisition, parser or chunker calls, graph imports, LadybugDB writes, model calls, crawler calls, or production writes.

## Source Acquisition Preflight

- URL refs: {summary["source_acquisition_preflight"]["url_ref_count"]}
- Normalized identities: {summary["source_acquisition_preflight"]["normalized_identity_count"]}
- Terminal captured events: {summary["source_acquisition_preflight"]["terminal_event_count"]}
- Expansion refs: {", ".join(summary["source_acquisition_preflight"]["expansion_refs"])}
- Duplicate identity: `{summary["source_acquisition_preflight"]["duplicate_identity"]}` ({summary["source_acquisition_preflight"]["duplicate_identity_ref_count"]} refs)

## Stage Replay

| Stage | Status | Exit Code | Duration ms |
|---|---:|---:|---:|
{stage_rows}

## Safety Flags

All closeout safety flags remain fail-closed: graph/import/write/model/crawler/parser/chunker behavior is false and unsafe counters are zero.

## Failure Modes

- Filesystem inputs: missing or malformed JSON/JSONL fails before replay with stable diagnostics (`INPUT_MISSING`, `JSON_MALFORMED`, `JSONL_MALFORMED`).
- Local artifact provenance: unsafe, missing, byte-count-mismatched, or SHA-256-mismatched artifact paths fail preflight.
- Subprocess stages: timeout or nonzero exit records the failed stage, command, cwd, exit code, bounded output excerpts, and stops subsequent replay.
- Network/API dependencies: live fetch, crawler, model, graph, LadybugDB, and production-write paths are intentionally not invoked; any unsafe flag/counter in inputs or verifier output fails closed.

## Load Profile

Expected load is exactly 21 URL refs and 20 normalized identities. At 10x, local filesystem hashing/subprocess replay would saturate first; this runner protects the boundary with exact-count checks rather than expanding into batch ingestion.

## Negative Tests

- `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_absolute_or_escaping_artifact_path` covers unsafe repo-relative artifact paths.
- `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_nonzero_unsafe_counter_and_flag` covers unsafe graph/write flags and counters.
- `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_payload_bearing_key` and `tests/test_m028_smoke_replay_closeout.py::test_verifier_rejects_raw_payload_marker` cover payload-bearing keys and raw payload markers.

## Diagnostics

{diagnostic_lines}
"""


def write_outputs(out_dir: Path, summary: dict[str, Any]) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    events_path = out_dir / EVENTS_FILENAME
    summary_path = out_dir / SUMMARY_FILENAME
    report_path = out_dir / REPORT_FILENAME
    events_path.write_text(
        "\n".join(json.dumps(event, sort_keys=True) for event in summary["stage_events"]) + "\n",
        encoding="utf-8",
    )
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    report_path.write_text(render_report(summary), encoding="utf-8")


def run_closeout(
    corpus_dir: Path, out_dir: Path, *, timeout_seconds: int = 120, clean: bool = True
) -> dict[str, Any]:
    root = repo_root()
    corpus_dir = corpus_dir if corpus_dir.is_absolute() else root / corpus_dir
    out_dir = out_dir if out_dir.is_absolute() else root / out_dir
    if not corpus_dir.resolve().is_relative_to(root.resolve()):
        raise CloseoutError(
            [
                Diagnostic(
                    "CORPUS_DIR_OUTSIDE_REPO",
                    "$.corpus_dir",
                    "corpus dir must be inside repository",
                )
            ]
        )
    if not out_dir.resolve().is_relative_to(root.resolve()):
        raise CloseoutError(
            [
                Diagnostic(
                    "OUT_DIR_OUTSIDE_REPO", "$.out_dir", "output dir must be inside repository"
                )
            ]
        )

    replay_dir = out_dir / REPLAY_ARTIFACTS_DIRNAME
    if clean and replay_dir.exists():
        shutil.rmtree(replay_dir)
    replay_dir.mkdir(parents=True, exist_ok=True)
    out_dir.mkdir(parents=True, exist_ok=True)

    git_commit = git_commit_from_files(root)
    preflight, preflight_diagnostics = validate_source_acquisition(corpus_dir, root)
    stage_events: list[dict[str, Any]] = []
    all_diagnostics = [diagnostic.as_dict() for diagnostic in preflight_diagnostics]

    if not preflight_diagnostics:
        for stage in build_stages(corpus_dir, replay_dir, root):
            event = run_stage(stage, root, git_commit, timeout_seconds)
            stage_events.append(event)
            all_diagnostics.extend(event["diagnostics"])
            if event["status"] != "pass":
                break

    status = (
        "pass"
        if not all_diagnostics
        and stage_events
        and all(event["status"] == "pass" for event in stage_events)
        else "fail"
    )
    summary = {
        "schema_version": "m028.smoke-closeout-summary.v1",
        "milestone_id": MILESTONE_ID,
        "slice_id": SLICE_ID,
        "status": status,
        "created_at": utc_now_iso(),
        "cwd": ".",
        "git_commit": git_commit,
        "corpus_dir": repo_relative(corpus_dir, root),
        "out_dir": repo_relative(out_dir, root),
        "replay_artifacts_dir": repo_relative(replay_dir, root),
        "source_acquisition_preflight": preflight,
        "stage_events": stage_events,
        "diagnostics": all_diagnostics,
        "safety_flags": {
            "graph_write_attempted": False,
            "kg_readiness_claimed": False,
            "parser_readiness_claimed": False,
            "production_write_attempted": False,
            "production_import_attempted": False,
            "ladybugdb_written": False,
            "model_call_attempted": False,
            "crawler_attempted": False,
            "network_fetch_attempted": False,
            "raw_payload_embedded": False,
        },
        "unsafe_counters": {
            "import_eligible_count": 0,
            "promoted_to_fact_count": 0,
            "graph_write_count": 0,
            "ladybugdb_write_count": 0,
            "model_call_count": 0,
            "crawler_call_count": 0,
            "network_fetch_count": 0,
        },
        "failure_modes": [
            "missing or malformed filesystem input fails before replay",
            "unsafe acquisition provenance fails preflight",
            "subprocess timeout or nonzero exit records failed stage and stops",
            "live network/model/crawler/graph/write work is out of scope and fail-closed by safety flags",
        ],
        "load_profile": {
            "expected_url_refs": EXPECTED_URL_REFS,
            "ten_x_url_refs": EXPECTED_URL_REFS * 10,
            "first_saturating_resource": "local filesystem hashing and serial subprocess replay",
            "protection": "exact expected-count checks fail closed instead of widening into batch ingestion",
        },
        "negative_tests": [
            "missing artifact path rejected",
            "unsafe graph/write flag rejected",
            "malformed JSONL rejected",
        ],
    }
    write_outputs(out_dir, summary)
    return summary


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-dir", type=Path, default=DEFAULT_CORPUS_DIR)
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_CORPUS_DIR / DEFAULT_OUT_DIRNAME)
    parser.add_argument("--timeout-seconds", type=int, default=120)
    parser.add_argument(
        "--no-clean",
        action="store_true",
        help="do not remove previous isolated replay artifacts before running",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        summary = run_closeout(
            args.corpus_dir,
            args.out_dir,
            timeout_seconds=args.timeout_seconds,
            clean=not args.no_clean,
        )
    except CloseoutError as exc:
        root = repo_root()
        out_dir = args.out_dir if args.out_dir.is_absolute() else root / args.out_dir
        summary = {
            "schema_version": "m028.smoke-closeout-summary.v1",
            "milestone_id": MILESTONE_ID,
            "slice_id": SLICE_ID,
            "status": "fail",
            "created_at": utc_now_iso(),
            "cwd": ".",
            "git_commit": git_commit_from_files(root),
            "corpus_dir": args.corpus_dir.as_posix(),
            "out_dir": args.out_dir.as_posix(),
            "replay_artifacts_dir": (args.out_dir / REPLAY_ARTIFACTS_DIRNAME).as_posix(),
            "source_acquisition_preflight": {
                "status": "fail",
                "diagnostics": [diagnostic.as_dict() for diagnostic in exc.diagnostics],
            },
            "stage_events": [],
            "diagnostics": [diagnostic.as_dict() for diagnostic in exc.diagnostics],
            "safety_flags": {
                "graph_write_attempted": False,
                "production_write_attempted": False,
                "ladybugdb_written": False,
                "model_call_attempted": False,
                "crawler_attempted": False,
            },
            "unsafe_counters": {"import_eligible_count": 0, "promoted_to_fact_count": 0},
        }
        write_outputs(out_dir, summary)
        sys.stderr.write(str(exc) + "\n")
        return 1

    sys.stdout.write(
        f"closeout_summary={args.out_dir / SUMMARY_FILENAME} "
        f"closeout_events={args.out_dir / EVENTS_FILENAME} "
        f"closeout_report={args.out_dir / REPORT_FILENAME} status={summary['status']}\n"
    )
    return 0 if summary["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
