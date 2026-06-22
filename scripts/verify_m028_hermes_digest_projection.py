#!/usr/bin/env python3
"""Verify M028 S05 Hermes digest projection fail-closed.

The verifier treats S04 loader evidence bundles, S04 summary, the Hermes digest,
and the markdown report as untrusted local inputs. It recomputes the digest
scope from S04 evidence, verifies linkage/fingerprints/report sections, and
rejects raw payload leakage, unsafe paths, parser/chunker/model/graph/KG/import
readiness claims, and production-write claims without fetching sources or
writing graph state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from build_m028_hermes_digest_projection import (  # noqa: E402
    EXPANDED_SCOPE_REF_IDS,
    EXPECTED_DUPLICATE_GROUP,
    EXPECTED_IDENTITY_COUNT,
    EXPECTED_REF_COUNT,
    EXPECTED_REF_IDS,
    EXPECTED_SOURCE_KIND_COUNTS,
    FORBIDDEN_KEYS,
    FORBIDDEN_PAYLOAD_MARKERS,
    SCHEMA_NAME,
    SCHEMA_VERSION,
    UNSAFE_COUNTER_KEYS,
    UNSAFE_SAFETY_FLAG_KEYS,
    count_unsafe_claims,
)

REQUIRED_REPORT_SECTIONS: dict[str, tuple[str, ...]] = {
    "Scope": ("Scope",),
    "Source References": ("Source References",),
    "Summary": ("Summary",),
    "Quality Warnings": ("Quality Warnings", "Warnings"),
    "Skipped/Not Acquired Diagnostics": ("Skipped/Not Acquired Diagnostics", "Skipped Diagnostics"),
    "Safety and Redaction": ("Safety and Redaction", "Safety"),
    "Failure Modes": ("Failure Modes",),
    "Load Profile": ("Load Profile",),
    "Negative Tests": ("Negative Tests",),
    "Observability Impact": ("Observability Impact",),
}

UNSAFE_TRUE_FIELDS = {
    "network_calls_attempted",
    "parser_attempted",
    "chunker_attempted",
    "model_attempted",
    "graph_write_attempted",
    "production_write_attempted",
    "production_persistence_attempted",
    "hermes_digest_ready",
    "parser_output_available",
    "kg_import_eligible",
    "production_import_eligible",
    "raw_article_text_embedded",
    "html_source_embedded",
    "raw_pdf_bytes_embedded",
    "source_payload_embedded",
    "binary_payload_embedded",
    "chunk_content_embedded",
    "chunk_payload_embedded",
    "model_output_embedded",
    "local_absolute_paths_embedded",
    "graph_or_kg_claims_embedded",
}

UNSAFE_STRING_MARKERS = tuple(
    sorted(
        set(FORBIDDEN_PAYLOAD_MARKERS)
        | {
            "trusted_fact",
            "graph_ready",
            "kg_ready",
            "parser_ready",
            "chunker_ready",
            "model_ready",
            "import_ready",
            "production_ready",
            "promoted_to_fact",
            "ladybugdb_written=true",
            "kg_import_eligible=true",
            "production_import_eligible=true",
        }
    )
)


@dataclass(frozen=True)
class Diagnostic:
    """Stable verifier diagnostic for agent-inspectable failures."""

    code: str
    path: str
    message: str

    def render(self) -> str:
        return f"{self.code}\t{self.path}\t{self.message}"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path, diagnostics: list[Diagnostic], label: str) -> dict[str, Any] | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        diagnostics.append(
            Diagnostic("INPUT_MISSING", f"$.inputs.{label}", f"missing input: {path.as_posix()}")
        )
        return None
    except json.JSONDecodeError as exc:
        diagnostics.append(
            Diagnostic(
                "JSON_MALFORMED", f"$.inputs.{label}", f"malformed JSON at {exc.lineno}:{exc.colno}"
            )
        )
        return None
    if not isinstance(payload, dict):
        diagnostics.append(
            Diagnostic(
                "JSON_OBJECT_REQUIRED",
                f"$.inputs.{label}",
                "top-level JSON value must be an object",
            )
        )
        return None
    return payload


def read_jsonl(
    path: Path, diagnostics: list[Diagnostic], label: str
) -> list[dict[str, Any]] | None:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        diagnostics.append(
            Diagnostic("INPUT_MISSING", f"$.inputs.{label}", f"missing input: {path.as_posix()}")
        )
        return None
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as exc:
            diagnostics.append(
                Diagnostic(
                    "JSONL_MALFORMED",
                    f"$.inputs.{label}[{line_number}]",
                    f"malformed JSONL at column {exc.colno}",
                )
            )
            continue
        if not isinstance(row, dict):
            diagnostics.append(
                Diagnostic(
                    "JSONL_OBJECT_REQUIRED",
                    f"$.inputs.{label}[{line_number}]",
                    "JSONL row must be an object",
                )
            )
            continue
        rows.append(row)
    return rows


def read_report(path: Path, diagnostics: list[Diagnostic]) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        diagnostics.append(
            Diagnostic("INPUT_MISSING", "$.inputs.report", f"missing input: {path.as_posix()}")
        )
        return None


def json_path_join(prefix: str, part: str | int) -> str:
    if isinstance(part, int):
        return f"{prefix}[{part}]"
    return f"{prefix}.{part}" if prefix != "$" else f"$.{part}"


def safe_relative_path(path_value: Any) -> bool:
    if path_value is None:
        return True
    if not isinstance(path_value, str) or not path_value.strip() or "://" in path_value:
        return False
    normalized = PurePosixPath(path_value.replace("\\", "/"))
    return (
        not normalized.is_absolute()
        and ".." not in normalized.parts
        and all(part for part in normalized.parts)
    )


def walk_forbidden(
    payload: Any, diagnostics: list[Diagnostic], path: str = "$", *, scan_strings: bool = True
) -> None:
    if isinstance(payload, dict):
        for key, value in payload.items():
            child_path = json_path_join(path, str(key))
            if str(key) in FORBIDDEN_KEYS:
                diagnostics.append(
                    Diagnostic(
                        "FORBIDDEN_KEY_PRESENT", child_path, f"forbidden payload-bearing key {key}"
                    )
                )
            if str(key) in UNSAFE_TRUE_FIELDS and value is True:
                diagnostics.append(
                    Diagnostic("UNSAFE_BOOLEAN_TRUE", child_path, f"{key} must not be true")
                )
            walk_forbidden(value, diagnostics, child_path, scan_strings=scan_strings)
    elif isinstance(payload, list):
        for index, value in enumerate(payload):
            walk_forbidden(
                value, diagnostics, json_path_join(path, index), scan_strings=scan_strings
            )
    elif scan_strings and isinstance(payload, str):
        lower = payload.lower()
        for marker in UNSAFE_STRING_MARKERS:
            if marker in lower:
                diagnostics.append(
                    Diagnostic("FORBIDDEN_MARKER_PRESENT", path, f"forbidden marker {marker}")
                )


def rows_by_ref(
    rows: list[dict[str, Any]], diagnostics: list[Diagnostic], label: str
) -> dict[str, dict[str, Any]]:
    by_ref: dict[str, dict[str, Any]] = {}
    for index, row in enumerate(rows):
        ref_id = row.get("ref_id")
        if not isinstance(ref_id, str) or not ref_id:
            diagnostics.append(
                Diagnostic("REF_ID_REQUIRED", f"$.{label}[{index}].ref_id", "row is missing ref_id")
            )
            continue
        if ref_id in by_ref:
            diagnostics.append(
                Diagnostic(
                    "REF_ID_DUPLICATE", f"$.{label}[{index}].ref_id", f"duplicate ref_id {ref_id}"
                )
            )
            continue
        by_ref[ref_id] = row
    return by_ref


def expected_summary_from_bundles(
    bundles: list[dict[str, Any]], upstream_summary: dict[str, Any]
) -> dict[str, Any]:
    diagnostic_counts: Counter[str] = Counter()
    duplicate_identity_groups = []
    seen_groups: set[str] = set()
    for bundle in bundles:
        diagnostics = (
            bundle.get("diagnostics") if isinstance(bundle.get("diagnostics"), list) else []
        )
        for diagnostic in diagnostics:
            if isinstance(diagnostic, dict):
                diagnostic_counts[str(diagnostic.get("code", "diagnostic"))] += 1
        group = (
            bundle.get("identity_group") if isinstance(bundle.get("identity_group"), dict) else None
        )
        if (
            group
            and int(group.get("url_ref_count", 0)) > 1
            and str(group.get("group_id")) not in seen_groups
        ):
            duplicate_identity_groups.append(group)
            seen_groups.add(str(group.get("group_id")))
    return {
        "url_ref_count": len(bundles),
        "normalized_identity_count": len(
            {
                str(bundle.get("normalized_identity"))
                for bundle in bundles
                if isinstance(bundle.get("normalized_identity"), str)
            }
        ),
        "ref_ids": [
            str(bundle.get("ref_id")) for bundle in bundles if isinstance(bundle.get("ref_id"), str)
        ],
        "expanded_scope_ref_ids": EXPANDED_SCOPE_REF_IDS,
        "source_kind_counts": dict(
            sorted(
                Counter(
                    str(bundle.get("source_kind"))
                    for bundle in bundles
                    if isinstance(bundle.get("source_kind"), str)
                ).items()
            )
        ),
        "source_family_counts": dict(
            sorted(
                Counter(
                    str(bundle.get("source_family"))
                    for bundle in bundles
                    if isinstance(bundle.get("source_family"), str)
                ).items()
            )
        ),
        "pdf_status_counts": dict(
            sorted(
                Counter(
                    str((bundle.get("pdf_diagnostic") or {}).get("status"))
                    for bundle in bundles
                    if isinstance(bundle.get("pdf_diagnostic"), dict)
                ).items()
            )
        ),
        "source_quality_status_counts": dict(
            sorted(
                Counter(
                    str((bundle.get("loader_evidence") or {}).get("source_quality_status"))
                    for bundle in bundles
                    if isinstance(bundle.get("loader_evidence"), dict)
                ).items()
            )
        ),
        "diagnostic_counts": dict(sorted(diagnostic_counts.items())),
        "duplicate_identity_group_count": len(duplicate_identity_groups),
        "duplicate_identity_groups": duplicate_identity_groups,
        "unsafe_counter_total": 0,
    }


def validate_scope_and_duplicates(
    bundles: list[dict[str, Any]], diagnostics: list[Diagnostic]
) -> None:
    if len(bundles) != EXPECTED_REF_COUNT:
        diagnostics.append(
            Diagnostic(
                "SCOPE_REF_COUNT_MISMATCH",
                "$.bundles",
                f"expected {EXPECTED_REF_COUNT} bundles, found {len(bundles)}",
            )
        )
    ref_ids = [
        str(bundle.get("ref_id")) for bundle in bundles if isinstance(bundle.get("ref_id"), str)
    ]
    if ref_ids != EXPECTED_REF_IDS:
        diagnostics.append(
            Diagnostic(
                "SCOPE_REF_IDS_MISMATCH",
                "$.bundles[*].ref_id",
                f"expected ordered refs {EXPECTED_REF_IDS}, found {ref_ids}",
            )
        )
    missing_expanded = sorted(set(EXPANDED_SCOPE_REF_IDS) - set(ref_ids))
    if missing_expanded:
        diagnostics.append(
            Diagnostic(
                "EXPANDED_SCOPE_REFS_MISSING",
                "$.bundles[*].ref_id",
                f"missing expanded refs {missing_expanded}",
            )
        )
    identities = [
        str(bundle.get("normalized_identity"))
        for bundle in bundles
        if isinstance(bundle.get("normalized_identity"), str)
    ]
    if len(set(identities)) != EXPECTED_IDENTITY_COUNT:
        diagnostics.append(
            Diagnostic(
                "SCOPE_IDENTITY_COUNT_MISMATCH",
                "$.bundles[*].normalized_identity",
                f"expected {EXPECTED_IDENTITY_COUNT} identities, found {len(set(identities))}",
            )
        )
    source_kind_counts = dict(
        sorted(
            Counter(
                str(bundle.get("source_kind"))
                for bundle in bundles
                if isinstance(bundle.get("source_kind"), str)
            ).items()
        )
    )
    if source_kind_counts != dict(sorted(EXPECTED_SOURCE_KIND_COUNTS.items())):
        diagnostics.append(
            Diagnostic(
                "SCOPE_SOURCE_KIND_COUNTS_MISMATCH",
                "$.bundles[*].source_kind",
                f"expected {EXPECTED_SOURCE_KIND_COUNTS}, found {source_kind_counts}",
            )
        )
    duplicate_groups = sorted(
        sorted(
            str(bundle.get("ref_id"))
            for bundle in bundles
            if bundle.get("normalized_identity") == identity
        )
        for identity in sorted(set(identities))
        if sum(1 for bundle in bundles if bundle.get("normalized_identity") == identity) > 1
    )
    if duplicate_groups != [EXPECTED_DUPLICATE_GROUP]:
        diagnostics.append(
            Diagnostic(
                "DUPLICATE_IDENTITY_DRIFT",
                "$.bundles[*].normalized_identity",
                f"expected duplicate group {EXPECTED_DUPLICATE_GROUP}, found {duplicate_groups}",
            )
        )


def validate_source_refs(
    digest: dict[str, Any],
    summary: dict[str, Any],
    bundles_path: Path,
    summary_path: Path,
    diagnostics: list[Diagnostic],
) -> None:
    source_refs = digest.get("source_refs") if isinstance(digest.get("source_refs"), dict) else {}
    expected = {
        "loader_bundle": {"path": bundles_path.as_posix(), "sha256": sha256_file(bundles_path)},
        "loader_summary": {"path": summary_path.as_posix(), "sha256": sha256_file(summary_path)},
    }
    for label, expected_ref in expected.items():
        actual = source_refs.get(label) if isinstance(source_refs.get(label), dict) else {}
        for key, expected_value in expected_ref.items():
            if actual.get(key) != expected_value:
                diagnostics.append(
                    Diagnostic(
                        "SOURCE_REF_MISMATCH",
                        f"$.digest.source_refs.{label}.{key}",
                        f"expected {expected_value!r}, found {actual.get(key)!r}",
                    )
                )
        if not safe_relative_path(actual.get("path")):
            diagnostics.append(
                Diagnostic(
                    "SOURCE_REF_PATH_UNSAFE",
                    f"$.digest.source_refs.{label}.path",
                    "source ref path must be safe repo-relative",
                )
            )
    selection_expected = (
        summary.get("input_fingerprints", {}).get("selection")
        if isinstance(summary.get("input_fingerprints"), dict)
        else None
    )
    selection_actual = (
        source_refs.get("selection_ref")
        if isinstance(source_refs.get("selection_ref"), dict)
        else None
    )
    if isinstance(selection_expected, dict):
        for key in ("path", "sha256"):
            if selection_actual is None or selection_actual.get(key) != selection_expected.get(key):
                diagnostics.append(
                    Diagnostic(
                        "SELECTION_FINGERPRINT_MISMATCH",
                        f"$.digest.source_refs.selection_ref.{key}",
                        "selection fingerprint drift from S04 summary",
                    )
                )
        if selection_actual is not None and not safe_relative_path(selection_actual.get("path")):
            diagnostics.append(
                Diagnostic(
                    "SOURCE_REF_PATH_UNSAFE",
                    "$.digest.source_refs.selection_ref.path",
                    "selection ref path must be safe repo-relative",
                )
            )


def validate_digest_summary(
    digest: dict[str, Any],
    bundles: list[dict[str, Any]],
    upstream_summary: dict[str, Any],
    diagnostics: list[Diagnostic],
) -> None:
    summary = digest.get("summary") if isinstance(digest.get("summary"), dict) else None
    if summary is None:
        diagnostics.append(
            Diagnostic(
                "DIGEST_SUMMARY_REQUIRED", "$.digest.summary", "digest summary must be an object"
            )
        )
        return
    unsafe_counts = count_unsafe_claims(bundles, upstream_summary)
    expected = expected_summary_from_bundles(bundles, upstream_summary)
    expected["unsafe_counter_total"] = sum(unsafe_counts.values())
    for key, expected_value in expected.items():
        if summary.get(key) != expected_value:
            diagnostics.append(
                Diagnostic(
                    "DIGEST_SUMMARY_MISMATCH",
                    f"$.digest.summary.{key}",
                    f"expected recomputed {key}={expected_value!r}, found {summary.get(key)!r}",
                )
            )
    if summary.get("url_ref_count") != EXPECTED_REF_COUNT:
        diagnostics.append(
            Diagnostic(
                "DIGEST_SCOPE_REF_COUNT_MISMATCH",
                "$.digest.summary.url_ref_count",
                "digest URL ref count drifted",
            )
        )
    if summary.get("normalized_identity_count") != EXPECTED_IDENTITY_COUNT:
        diagnostics.append(
            Diagnostic(
                "DIGEST_SCOPE_IDENTITY_COUNT_MISMATCH",
                "$.digest.summary.normalized_identity_count",
                "digest identity count drifted",
            )
        )


def validate_items(
    digest: dict[str, Any], bundles: list[dict[str, Any]], diagnostics: list[Diagnostic]
) -> None:
    items = digest.get("items")
    if not isinstance(items, list):
        diagnostics.append(
            Diagnostic("DIGEST_ITEMS_REQUIRED", "$.digest.items", "digest items must be a list")
        )
        return
    if len(items) != len(bundles):
        diagnostics.append(
            Diagnostic(
                "DIGEST_ITEM_COUNT_MISMATCH",
                "$.digest.items",
                f"expected {len(bundles)} items, found {len(items)}",
            )
        )
    bundle_by_ref = rows_by_ref(bundles, diagnostics, "bundles")
    item_by_ref = rows_by_ref(
        [item for item in items if isinstance(item, dict)], diagnostics, "digest.items"
    )
    if set(item_by_ref) != set(bundle_by_ref):
        diagnostics.append(
            Diagnostic(
                "DIGEST_ITEM_REF_SET_MISMATCH",
                "$.digest.items[*].ref_id",
                f"missing={sorted(set(bundle_by_ref) - set(item_by_ref))} extra={sorted(set(item_by_ref) - set(bundle_by_ref))}",
            )
        )
    for index, item in enumerate(items):
        item_path = f"$.digest.items[{index}]"
        if not isinstance(item, dict):
            diagnostics.append(
                Diagnostic(
                    "DIGEST_ITEM_OBJECT_REQUIRED", item_path, "digest item must be an object"
                )
            )
            continue
        ref_id = item.get("ref_id")
        if not isinstance(ref_id, str) or ref_id not in bundle_by_ref:
            continue
        bundle = bundle_by_ref[ref_id]
        for key in (
            "canonical_url",
            "normalized_identity",
            "source_kind",
            "source_family",
            "url_variant",
            "identity_group",
            "loader_evidence",
            "pdf_diagnostic",
        ):
            if item.get(key) != bundle.get(key):
                diagnostics.append(
                    Diagnostic(
                        "DIGEST_ITEM_BUNDLE_MISMATCH",
                        f"{item_path}.{key}",
                        f"item {ref_id} drift from S04 bundle {key}",
                    )
                )
        item_artifacts = (
            item.get("artifact_refs") if isinstance(item.get("artifact_refs"), dict) else {}
        )
        bundle_artifacts = (
            bundle.get("artifact_refs") if isinstance(bundle.get("artifact_refs"), dict) else {}
        )
        for artifact_name in ("source_artifact", "metadata_artifact", "pdf_artifact"):
            actual = (
                item_artifacts.get(artifact_name)
                if isinstance(item_artifacts.get(artifact_name), dict)
                else {}
            )
            expected = (
                bundle_artifacts.get(artifact_name)
                if isinstance(bundle_artifacts.get(artifact_name), dict)
                else {}
            )
            for key in ("path", "sha256", "byte_count", "content_type"):
                actual_value = actual.get(key)
                expected_value = (
                    expected.get(key) if isinstance(expected.get(key), (str, int)) else None
                )
                if actual_value != expected_value:
                    diagnostics.append(
                        Diagnostic(
                            "DIGEST_ARTIFACT_LINKAGE_MISMATCH",
                            f"{item_path}.artifact_refs.{artifact_name}.{key}",
                            f"artifact {key} drift for {ref_id}",
                        )
                    )
            if actual.get("payload_embedded") is not False:
                diagnostics.append(
                    Diagnostic(
                        "ARTIFACT_PAYLOAD_FLAG_UNSAFE",
                        f"{item_path}.artifact_refs.{artifact_name}.payload_embedded",
                        "artifact payload_embedded must be false",
                    )
                )
            if not safe_relative_path(actual.get("path")):
                diagnostics.append(
                    Diagnostic(
                        "ARTIFACT_PATH_UNSAFE",
                        f"{item_path}.artifact_refs.{artifact_name}.path",
                        "artifact path must be null or safe repo-relative path",
                    )
                )
        evidence = (
            item.get("loader_evidence") if isinstance(item.get("loader_evidence"), dict) else {}
        )
        if evidence.get("outcome") == "promoted_to_fact":
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_OUTCOME_REJECTED",
                    f"{item_path}.loader_evidence.outcome",
                    "digest item must not claim promoted facts",
                )
            )


def validate_unsafe_claims(
    digest: dict[str, Any],
    bundles: list[dict[str, Any]],
    upstream_summary: dict[str, Any],
    diagnostics: list[Diagnostic],
    reject_unsafe_claims: bool,
) -> None:
    unsafe_counts = (
        digest.get("unsafe_counters") if isinstance(digest.get("unsafe_counters"), dict) else {}
    )
    computed_counts = count_unsafe_claims(bundles, upstream_summary)
    for key in UNSAFE_COUNTER_KEYS:
        if key not in unsafe_counts:
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_COUNTER_MISSING",
                    f"$.digest.unsafe_counters.{key}",
                    "unsafe counter missing from digest",
                )
            )
        if unsafe_counts.get(key) != computed_counts.get(key, 0):
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_COUNTER_MISMATCH",
                    f"$.digest.unsafe_counters.{key}",
                    f"expected {computed_counts.get(key, 0)}, found {unsafe_counts.get(key)!r}",
                )
            )
        if reject_unsafe_claims and unsafe_counts.get(key) != 0:
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_CLAIM_REJECTED",
                    f"$.digest.unsafe_counters.{key}",
                    f"unsafe counter {key}={unsafe_counts.get(key)!r}",
                )
            )
    for key in UNSAFE_SAFETY_FLAG_KEYS:
        if computed_counts.get(key, 0) != 0:
            diagnostics.append(
                Diagnostic(
                    "UNSAFE_CLAIM_IN_BUNDLE",
                    f"$.bundles[*].safety_flags.{key}",
                    f"computed unsafe flag {key}={computed_counts.get(key, 0)}",
                )
            )
    redaction_flags = (
        digest.get("redaction_flags") if isinstance(digest.get("redaction_flags"), dict) else {}
    )
    for key, value in redaction_flags.items():
        if value is not False:
            diagnostics.append(
                Diagnostic(
                    "REDACTION_FLAG_UNSAFE",
                    f"$.digest.redaction_flags.{key}",
                    "redaction flags must remain false",
                )
            )
    generator = digest.get("generator") if isinstance(digest.get("generator"), dict) else {}
    for key in (
        "network_calls_attempted",
        "parser_attempted",
        "chunker_attempted",
        "model_attempted",
        "graph_write_attempted",
        "production_write_attempted",
    ):
        if generator.get(key) is not False:
            diagnostics.append(
                Diagnostic(
                    "GENERATOR_UNSAFE_CLAIM", f"$.digest.generator.{key}", f"{key} must be false"
                )
            )


def validate_report(report: str, diagnostics: list[Diagnostic]) -> None:
    headings = set(re.findall(r"^##\s+(.+?)\s*$", report, flags=re.MULTILINE))
    for canonical, aliases in REQUIRED_REPORT_SECTIONS.items():
        if not any(alias in headings for alias in aliases):
            diagnostics.append(
                Diagnostic(
                    "REPORT_SECTION_MISSING",
                    f"$.report.sections.{canonical}",
                    f"missing required report section {canonical}",
                )
            )


def validate_contract(
    bundles_path: Path,
    summary_path: Path,
    digest_path: Path,
    report_path: Path,
    *,
    reject_unsafe_claims: bool = False,
) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    bundles = read_jsonl(bundles_path, diagnostics, "bundles")
    summary = read_json(summary_path, diagnostics, "summary")
    digest = read_json(digest_path, diagnostics, "digest")
    report = read_report(report_path, diagnostics)
    if diagnostics:
        return diagnostics
    assert bundles is not None and summary is not None and digest is not None and report is not None

    walk_forbidden(bundles, diagnostics, "$.bundles")
    walk_forbidden(summary, diagnostics, "$.summary")
    # Scan the structured digest for payload/readiness markers. Report prose is
    # section-checked separately to avoid false positives from required negative-test prose.
    walk_forbidden(digest, diagnostics, "$.digest")

    if digest.get("schema_name") != SCHEMA_NAME:
        diagnostics.append(
            Diagnostic(
                "DIGEST_SCHEMA_NAME_MISMATCH", "$.digest.schema_name", f"expected {SCHEMA_NAME}"
            )
        )
    if digest.get("schema_version") != SCHEMA_VERSION:
        diagnostics.append(
            Diagnostic(
                "DIGEST_SCHEMA_VERSION_MISMATCH",
                "$.digest.schema_version",
                f"expected {SCHEMA_VERSION}",
            )
        )

    validate_scope_and_duplicates(bundles, diagnostics)
    validate_source_refs(digest, summary, bundles_path, summary_path, diagnostics)
    validate_digest_summary(digest, bundles, summary, diagnostics)
    validate_items(digest, bundles, diagnostics)
    validate_unsafe_claims(digest, bundles, summary, diagnostics, reject_unsafe_claims)
    validate_report(report, diagnostics)
    return diagnostics


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundles", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--digest", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument(
        "--reject-unsafe-claims",
        action="store_true",
        help="fail if any unsafe readiness/import/model/production claim is present",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    diagnostics = validate_contract(
        args.bundles,
        args.summary,
        args.digest,
        args.report,
        reject_unsafe_claims=args.reject_unsafe_claims,
    )
    if diagnostics:
        sys.stderr.write("M028 Hermes digest projection verification failed\n")
        for item in diagnostics:
            sys.stderr.write(item.render() + "\n")
        return 1
    sys.stdout.write(
        "M028 Hermes digest projection verification passed: "
        f"refs={EXPECTED_REF_COUNT} identities={EXPECTED_IDENTITY_COUNT} "
        f"expanded_refs={','.join(EXPANDED_SCOPE_REF_IDS)} reject_unsafe_claims={args.reject_unsafe_claims}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
