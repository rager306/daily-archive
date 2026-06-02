#!/usr/bin/env python3
"""Build metadata-only source adapter outputs for M028 S02.

The adapter consumes the accepted URL selection plus terminal acquisition events and
emits per-ref metadata/provenance records.  It never serializes source bodies,
PDF bytes, chunks, graph writes, parser readiness, model output, or production
persistence claims.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
import sys
from collections import Counter, defaultdict
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

SUMMARY_SCHEMA_VERSION = "m028.source-metadata-summary.v1"
EVENT_SCHEMA_VERSION = "m028.source-metadata-event.v1"

EVENTS_FILENAME = "source-metadata-events.jsonl"
SUMMARY_FILENAME = "source-metadata-summary.json"

ARXIV_URL_RE = re.compile(r"arxiv\.org/(?:abs|pdf)/(\d{4}\.\d{4,5})(v\d+)?(?:\.pdf)?", re.I)
META_TAG_RE = re.compile(r"<meta\s+([^>]*?)>", re.I | re.S)
ATTR_RE = re.compile(r"([:\w-]+)\s*=\s*(['\"])(.*?)\2", re.I | re.S)
TITLE_RE = re.compile(r"<title[^>]*>(.*?)</title>", re.I | re.S)
TAG_RE = re.compile(r"<[^>]+>")
WHITESPACE_RE = re.compile(r"\s+")

OPTIONAL_FIELDS = (
    "title",
    "authors",
    "published_date",
    "updated_date",
    "doi",
    "artifact_arxiv_id",
    "pdf_url",
)

FAIL_CLOSED_FLAGS = {
    "graph_write_attempted": False,
    "production_persistence_attempted": False,
    "parser_readiness_claimed": False,
    "kg_readiness_claimed": False,
    "dspy_attempted": False,
    "rlm_attempted": False,
    "minimax_attempted": False,
    "source_payload_embedded": False,
    "binary_payload_embedded": False,
    "chunk_payload_embedded": False,
    "model_output_embedded": False,
}


class AdapterInputError(ValueError):
    """Raised when metadata adapter inputs are malformed or unsafe."""


def utc_now_iso() -> str:
    return datetime.now(UTC).isoformat()


def read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise AdapterInputError(f"input_missing:{path}") from exc
    except json.JSONDecodeError as exc:
        raise AdapterInputError(f"json_malformed:{path}:{exc.lineno}:{exc.colno}") from exc
    if not isinstance(payload, dict):
        raise AdapterInputError(f"json_object_required:{path}")
    return payload


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise AdapterInputError(f"input_missing:{path}") from exc

    events: list[dict[str, Any]] = []
    for index, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as exc:
            raise AdapterInputError(f"jsonl_malformed:{path}:{index}:{exc.colno}") from exc
        if not isinstance(event, dict):
            raise AdapterInputError(f"jsonl_object_required:{path}:{index}")
        events.append(event)
    return events


def validate_selection(selection: dict[str, Any]) -> list[dict[str, Any]]:
    refs = selection.get("refs")
    if not isinstance(refs, list) or not refs:
        raise AdapterInputError("selection_refs_required")
    seen_ref_ids: set[str] = set()
    validated: list[dict[str, Any]] = []
    for index, ref in enumerate(refs):
        if not isinstance(ref, dict):
            raise AdapterInputError(f"selection_ref_object_required:{index}")
        ref_id = ref.get("ref_id")
        url = ref.get("url")
        source_kind = ref.get("source_kind")
        normalized_identity = ref.get("normalized_identity")
        canonical_url = ref.get("canonical_url")
        if not all(isinstance(value, str) and value for value in (ref_id, url, source_kind, normalized_identity, canonical_url)):
            raise AdapterInputError(f"selection_ref_required_fields:{index}")
        if ref_id in seen_ref_ids:
            raise AdapterInputError(f"selection_ref_duplicate:{ref_id}")
        seen_ref_ids.add(ref_id)
        validated.append(ref)
    return validated


def acquisition_by_ref(events: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    by_ref: dict[str, dict[str, Any]] = {}
    for index, event in enumerate(events):
        ref_id = event.get("ref_id")
        if not isinstance(ref_id, str) or not ref_id:
            raise AdapterInputError(f"acquisition_ref_id_required:{index}")
        if ref_id in by_ref:
            raise AdapterInputError(f"acquisition_ref_duplicate:{ref_id}")
        by_ref[ref_id] = event
    return by_ref


def arxiv_id_from_url(url: str) -> str | None:
    match = ARXIV_URL_RE.search(url)
    if not match:
        return None
    return match.group(1) + (match.group(2) or "")


def unversioned_arxiv_id(arxiv_id: str | None) -> str | None:
    if arxiv_id is None:
        return None
    return re.sub(r"v\d+$", "", arxiv_id)


def canonical_url_for_ref(ref: dict[str, Any]) -> str:
    source_kind = str(ref.get("source_kind"))
    if source_kind.startswith("arxiv_"):
        arxiv_id = unversioned_arxiv_id(str(ref.get("arxiv_unversioned_id") or ref.get("arxiv_id") or ""))
        if not arxiv_id:
            arxiv_id = unversioned_arxiv_id(arxiv_id_from_url(str(ref["url"])))
        if arxiv_id:
            return f"https://arxiv.org/abs/{arxiv_id}"
    return str(ref.get("canonical_url") or ref["url"])


def source_family(source_kind: str) -> str:
    if source_kind.startswith("arxiv_"):
        return "arxiv"
    if source_kind == "company_blog_url":
        return "company_blog"
    if source_kind == "nature_article_url":
        return "nature"
    return "unknown"


def classify_variant(url: str, source_kind: str) -> str:
    if source_kind == "arxiv_pdf_url" or "/pdf/" in url:
        return "pdf_url"
    if source_kind == "arxiv_abs_url" or "/abs/" in url:
        return "abs_url"
    if source_kind == "company_blog_url":
        return "company_blog_url"
    if source_kind == "nature_article_url":
        return "nature_article_url"
    return "unknown_url"


def build_identity_groups(refs: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for ref in refs:
        grouped[str(ref["normalized_identity"])].append(ref)
    identity_groups: dict[str, dict[str, Any]] = {}
    for normalized_identity, group_refs in grouped.items():
        ref_ids = [str(ref["ref_id"]) for ref in group_refs]
        identity_groups[normalized_identity] = {
            "group_id": f"identity:{normalized_identity}",
            "normalized_identity": normalized_identity,
            "ref_ids": ref_ids,
            "url_ref_count": len(ref_ids),
            "has_url_variants": len(ref_ids) > 1,
            "url_variants": [classify_variant(str(ref["url"]), str(ref["source_kind"])) for ref in group_refs],
        }
    return identity_groups


def clean_metadata_value(value: str | None) -> str | None:
    if value is None:
        return None
    value = html.unescape(value)
    value = TAG_RE.sub(" ", value)
    value = WHITESPACE_RE.sub(" ", value).strip()
    return value or None


def html_metadata(path: Path) -> dict[str, list[str]]:
    document = path.read_text(encoding="utf-8", errors="replace")
    metadata: dict[str, list[str]] = defaultdict(list)
    for tag_match in META_TAG_RE.finditer(document):
        attrs = {name.lower(): clean_metadata_value(value) for name, _, value in ATTR_RE.findall(tag_match.group(1))}
        key = attrs.get("name") or attrs.get("property") or attrs.get("itemprop")
        content = attrs.get("content")
        if key and content:
            metadata[key.lower()].append(content)
    title_match = TITLE_RE.search(document)
    if title_match:
        title = clean_metadata_value(title_match.group(1))
        if title:
            metadata["html:title"].append(title)
    return dict(metadata)


def first_metadata_value(metadata: dict[str, list[str]], keys: tuple[str, ...]) -> tuple[str | None, str | None]:
    for key in keys:
        values = metadata.get(key.lower()) or []
        for value in values:
            cleaned = clean_metadata_value(value)
            if cleaned:
                return cleaned, key
    return None, None


def metadata_list(metadata: dict[str, list[str]], keys: tuple[str, ...], *, limit: int = 50) -> tuple[list[str], str | None]:
    for key in keys:
        values = [value for value in (clean_metadata_value(item) for item in metadata.get(key.lower(), [])) if value]
        if values:
            return values[:limit], key
    return [], None


def optional_value(value: Any, source: str | None, *, reason: str = "not_found") -> dict[str, Any]:
    present = bool(value) if not isinstance(value, list) else bool(value)
    return {
        "status": "present" if present else "missing",
        "value": value if present else None,
        "source": source if present else None,
        "missing_reason": None if present else reason,
    }


def derive_optional_metadata(ref: dict[str, Any], artifact_path: Path | None, source_kind: str) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    metadata: dict[str, list[str]] = {}
    if artifact_path is not None and artifact_path.exists() and artifact_path.suffix.lower() in {".html", ".htm"}:
        try:
            metadata = html_metadata(artifact_path)
        except UnicodeDecodeError:
            diagnostics.append(diagnostic("artifact_metadata_decode_failed", ref["ref_id"], "warning", "artifact"))

    family = source_family(source_kind)
    title_keys = ("citation_title", "og:title", "twitter:title", "dc.title", "html:title")
    author_keys = ("citation_author", "author", "article:author", "dc.creator")
    published_keys = ("citation_date", "citation_publication_date", "article:published_time", "date", "dc.date")
    updated_keys = ("article:modified_time", "citation_online_date", "lastmod", "dc.modified")
    doi_keys = ("citation_doi", "dc.identifier", "prism.doi")
    pdf_keys = ("citation_pdf_url",)

    title, title_source = first_metadata_value(metadata, title_keys)
    authors, authors_source = metadata_list(metadata, author_keys)
    published, published_source = first_metadata_value(metadata, published_keys)
    updated, updated_source = first_metadata_value(metadata, updated_keys)
    doi, doi_source = first_metadata_value(metadata, doi_keys)
    pdf_url, pdf_source = first_metadata_value(metadata, pdf_keys)

    artifact_arxiv_id = None
    artifact_arxiv_source = None
    if family == "arxiv":
        artifact_arxiv_id, artifact_arxiv_source = first_metadata_value(metadata, ("citation_arxiv_id",))
        artifact_arxiv_id = unversioned_arxiv_id(artifact_arxiv_id or str(ref.get("arxiv_unversioned_id") or ref.get("arxiv_id") or ""))
        artifact_arxiv_source = artifact_arxiv_source or "selection"

    optional_metadata = {
        "title": optional_value(title, title_source),
        "authors": optional_value(authors, authors_source),
        "published_date": optional_value(published, published_source),
        "updated_date": optional_value(updated, updated_source),
        "doi": optional_value(doi, doi_source, reason="not_applicable" if family in {"arxiv", "company_blog"} else "not_found"),
        "artifact_arxiv_id": optional_value(
            artifact_arxiv_id,
            artifact_arxiv_source,
            reason="not_applicable" if family != "arxiv" else "not_found",
        ),
        "pdf_url": optional_value(pdf_url, pdf_source, reason="not_applicable" if family != "arxiv" else "not_found"),
    }

    for field_name, field_value in optional_metadata.items():
        if field_value["status"] == "missing":
            diagnostics.append(
                diagnostic(
                    "optional_metadata_missing",
                    str(ref["ref_id"]),
                    "info",
                    f"optional_metadata.{field_name}",
                    details={"field": field_name, "reason": field_value["missing_reason"]},
                )
            )
    return optional_metadata, diagnostics


def diagnostic(
    code: str,
    ref_id: str,
    severity: str,
    json_path: str,
    *,
    details: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "code": code,
        "severity": severity,
        "ref_id": ref_id,
        "json_path": json_path,
        "message": "Metadata adapter diagnostic; inspect code, severity, and JSON path.",
        "details": details or {},
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def artifact_record(ref: dict[str, Any], acquisition: dict[str, Any] | None, repo_root: Path) -> tuple[dict[str, Any], Path | None, list[dict[str, Any]]]:
    diagnostics: list[dict[str, Any]] = []
    if acquisition is None:
        diagnostics.append(diagnostic("missing_acquisition_event", ref["ref_id"], "error", "acquisition"))
        return {
            "path": None,
            "exists": False,
            "content_type": None,
            "byte_count": None,
            "sha256": None,
            "checksum_verified": False,
            "payload_embedded": False,
        }, None, diagnostics

    path_value = acquisition.get("artifact_path")
    artifact_path = repo_root / path_value if isinstance(path_value, str) and path_value else None
    exists = artifact_path.exists() if artifact_path else False
    checksum_verified = False
    actual_byte_count: int | None = None

    if artifact_path is None:
        diagnostics.append(diagnostic("artifact_path_missing", ref["ref_id"], "error", "artifact.path"))
    elif not exists:
        diagnostics.append(diagnostic("artifact_file_missing", ref["ref_id"], "error", "artifact.path"))
    else:
        actual_byte_count = artifact_path.stat().st_size
        expected_byte_count = acquisition.get("byte_count")
        if isinstance(expected_byte_count, int) and expected_byte_count != actual_byte_count:
            diagnostics.append(
                diagnostic(
                    "artifact_byte_count_mismatch",
                    ref["ref_id"],
                    "warning",
                    "artifact.byte_count",
                    details={"expected": expected_byte_count, "actual": actual_byte_count},
                )
            )
        expected_sha256 = acquisition.get("sha256")
        actual_sha256 = sha256_file(artifact_path)
        checksum_verified = isinstance(expected_sha256, str) and expected_sha256 == actual_sha256
        if not checksum_verified:
            diagnostics.append(
                diagnostic(
                    "artifact_checksum_mismatch",
                    ref["ref_id"],
                    "warning",
                    "artifact.sha256",
                )
            )

    return {
        "path": path_value if isinstance(path_value, str) else None,
        "exists": exists,
        "content_type": acquisition.get("content_type"),
        "byte_count": actual_byte_count if actual_byte_count is not None else acquisition.get("byte_count"),
        "sha256": acquisition.get("sha256"),
        "checksum_verified": checksum_verified,
        "payload_embedded": False,
    }, artifact_path, diagnostics


def build_event(
    ref: dict[str, Any],
    acquisition: dict[str, Any] | None,
    identity_groups: dict[str, dict[str, Any]],
    repo_root: Path,
) -> dict[str, Any]:
    source_kind = str(ref["source_kind"])
    normalized_identity = str(ref["normalized_identity"])
    canonical_url = canonical_url_for_ref(ref)
    diagnostics: list[dict[str, Any]] = []

    if acquisition is not None:
        if acquisition.get("source_kind") != source_kind:
            diagnostics.append(diagnostic("source_kind_drift", ref["ref_id"], "error", "source_kind"))
        if acquisition.get("normalized_identity") != normalized_identity:
            diagnostics.append(diagnostic("normalized_identity_drift", ref["ref_id"], "error", "normalized_identity"))
        if acquisition.get("terminal") is not True:
            diagnostics.append(diagnostic("acquisition_not_terminal", ref["ref_id"], "error", "acquisition.terminal"))
        if acquisition.get("status") != "captured":
            diagnostics.append(diagnostic("acquisition_not_captured", ref["ref_id"], "warning", "acquisition.status"))

    artifact, artifact_path, artifact_diagnostics = artifact_record(ref, acquisition, repo_root)
    optional_metadata, metadata_diagnostics = derive_optional_metadata(ref, artifact_path, source_kind)
    diagnostics.extend(artifact_diagnostics)
    diagnostics.extend(metadata_diagnostics)

    if source_kind.startswith("arxiv_"):
        selected_arxiv_id = unversioned_arxiv_id(str(ref.get("arxiv_unversioned_id") or ref.get("arxiv_id") or ""))
        url_arxiv_id = unversioned_arxiv_id(arxiv_id_from_url(str(ref["url"])))
        if selected_arxiv_id and url_arxiv_id and selected_arxiv_id != url_arxiv_id:
            diagnostics.append(diagnostic("arxiv_url_id_drift", ref["ref_id"], "error", "url"))

    blocking_codes = {"missing_acquisition_event", "artifact_path_missing", "artifact_file_missing", "source_kind_drift", "normalized_identity_drift", "acquisition_not_terminal"}
    has_blocking = any(item["code"] in blocking_codes for item in diagnostics)
    has_warning = any(item["severity"] == "warning" for item in diagnostics)

    metadata_status = "metadata_available"
    if has_blocking:
        metadata_status = "blocked"
    elif has_warning:
        metadata_status = "metadata_available_with_diagnostics"

    return {
        "schema_version": EVENT_SCHEMA_VERSION,
        "ref_id": ref["ref_id"],
        "url": ref["url"],
        "url_variant": classify_variant(str(ref["url"]), source_kind),
        "canonical_url": canonical_url,
        "source_kind": source_kind,
        "source_family": source_family(source_kind),
        "normalized_identity": normalized_identity,
        "identity_group": identity_groups[normalized_identity],
        "acquisition": {
            "status": acquisition.get("status") if acquisition else None,
            "terminal": acquisition.get("terminal") if acquisition else False,
            "http_status": acquisition.get("http_status") if acquisition else None,
            "failure_code": acquisition.get("failure_code") if acquisition else "missing_acquisition_event",
            "captured": bool(acquisition and acquisition.get("status") == "captured"),
        },
        "artifact": artifact,
        "normalized": {
            "arxiv_id": unversioned_arxiv_id(str(ref.get("arxiv_unversioned_id") or ref.get("arxiv_id") or "")) if source_kind.startswith("arxiv_") else None,
            "canonical_url": canonical_url,
            "identity": normalized_identity,
        },
        "optional_metadata": optional_metadata,
        "optional_metadata_gaps": [
            {"field": field, "reason": value["missing_reason"]}
            for field, value in optional_metadata.items()
            if value["status"] == "missing"
        ],
        "metadata_status": metadata_status,
        "safety_flags": dict(FAIL_CLOSED_FLAGS),
        "diagnostics": diagnostics,
    }


def summarize_events(events: list[dict[str, Any]], identity_groups: dict[str, dict[str, Any]]) -> dict[str, Any]:
    source_kind_counts = Counter(str(event["source_kind"]) for event in events)
    source_family_counts = Counter(str(event["source_family"]) for event in events)
    acquisition_status_counts = Counter(str(event["acquisition"].get("status")) for event in events)
    metadata_status_counts = Counter(str(event["metadata_status"]) for event in events)
    optional_gap_counts: Counter[str] = Counter()
    diagnostic_counts: Counter[str] = Counter()
    for event in events:
        for gap in event["optional_metadata_gaps"]:
            optional_gap_counts[str(gap["field"])] += 1
        for item in event["diagnostics"]:
            diagnostic_counts[str(item["code"])] += 1

    duplicate_groups = [group for group in identity_groups.values() if group["url_ref_count"] > 1]
    return {
        "schema_version": SUMMARY_SCHEMA_VERSION,
        "generated_at": utc_now_iso(),
        "url_ref_count": len(events),
        "ref_count": len(events),
        "normalized_identity_count": len(identity_groups),
        "duplicate_identity_group_count": len(duplicate_groups),
        "duplicate_identity_groups": duplicate_groups,
        "source_kind_counts": dict(sorted(source_kind_counts.items())),
        "source_family_counts": dict(sorted(source_family_counts.items())),
        "acquisition_status_counts": dict(sorted(acquisition_status_counts.items())),
        "metadata_status_counts": dict(sorted(metadata_status_counts.items())),
        "optional_metadata_gap_counts": dict(sorted(optional_gap_counts.items())),
        "diagnostic_counts": dict(sorted(diagnostic_counts.items())),
        "ref_ids": [str(event["ref_id"]) for event in events],
        "identity_groups": list(identity_groups.values()),
        "safety_flags": dict(FAIL_CLOSED_FLAGS),
        "unsafe_claim_counts": {
            "graph_write_attempted": 0,
            "production_persistence_attempted": 0,
            "parser_readiness_claimed": 0,
            "kg_readiness_claimed": 0,
            "dspy_attempted": 0,
            "rlm_attempted": 0,
            "minimax_attempted": 0,
        },
        "load_profile": {
            "expected_url_refs": 21,
            "ten_x_url_refs": 210,
            "first_saturating_resource": "sequential filesystem reads and checksum hashing of captured artifacts",
            "protection": "single-pass streaming checksum, no network calls, no parser invocation, no body serialization, deterministic per-ref iteration",
        },
        "failure_modes": [
            {"dependency": "selection JSON", "failure_path": "missing or malformed input raises AdapterInputError before writing partial outputs"},
            {"dependency": "acquisition JSONL", "failure_path": "missing, malformed, or duplicate ref events raise AdapterInputError before writing partial outputs"},
            {"dependency": "captured artifact filesystem", "failure_path": "missing artifact emits blocked per-ref diagnostic without embedding payloads"},
            {"dependency": "artifact checksum", "failure_path": "byte/hash drift emits per-ref warning diagnostics and summary counts"},
        ],
        "negative_tests": [
            "tests/test_m028_source_metadata_adapters.py::test_missing_acquisition_event_is_blocked_not_silent",
            "tests/test_m028_source_metadata_adapters.py::test_checksum_mismatch_records_diagnostic",
            "tests/test_m028_source_metadata_adapters.py::test_rejects_malformed_selection",
        ],
    }


def build_metadata_outputs(selection_path: Path, acquisition_events_path: Path, out_dir: Path, *, repo_root: Path | None = None) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    repo_root = repo_root or Path.cwd()
    selection = read_json(selection_path)
    refs = validate_selection(selection)
    acquisition_events = read_jsonl(acquisition_events_path)
    acquisition_events_by_ref = acquisition_by_ref(acquisition_events)
    identity_groups = build_identity_groups(refs)

    events = [build_event(ref, acquisition_events_by_ref.get(str(ref["ref_id"])), identity_groups, repo_root) for ref in refs]
    summary = summarize_events(events, identity_groups)

    out_dir.mkdir(parents=True, exist_ok=True)
    events_path = out_dir / EVENTS_FILENAME
    summary_path = out_dir / SUMMARY_FILENAME
    events_path.write_text("\n".join(json.dumps(event, sort_keys=True) for event in events) + "\n", encoding="utf-8")
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return events, summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--acquisition-events", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        events, summary = build_metadata_outputs(args.selection, args.acquisition_events, args.out_dir)
    except AdapterInputError as exc:
        raise SystemExit(str(exc)) from exc
    sys.stdout.write(
        "wrote metadata adapter outputs: "
        f"refs={summary['url_ref_count']} identities={summary['normalized_identity_count']} "
        f"events={args.out_dir / EVENTS_FILENAME} summary={args.out_dir / SUMMARY_FILENAME}\n"
    )
    if any(event["metadata_status"] == "blocked" for event in events):
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
