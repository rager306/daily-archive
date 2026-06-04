#!/usr/bin/env python3
"""Validate the M030 requested-ref intake contract.

The verifier is intentionally local-only. It checks the bounded user-requested
loading/analysis intake without fetching sources and without claiming source
acquisition, parser readiness, chunk readiness, graph readiness, or production
persistence.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

EXPECTED_SELECTION_ID = "m029-pipeline-architecture-audit-v1"
EXPECTED_SCHEMA_VERSION = "article-corpus-selection.v00.02"
EXPECTED_URLS = {
    "https://arxiv.org/abs/2507.19457": {
        "identity": "arxiv:2507.19457",
        "catalog_status": "already_cataloged",
        "prior_selection_status": "not_in_m028_selection",
    },
    "https://web.stanford.edu/class/cs224n/readings/gradient-notes.pdf": {
        "identity": "stanford:cs224n:gradient-notes",
        "catalog_status": "missing_from_article_catalog",
        "prior_selection_status": "not_in_m028_selection",
    },
    "https://arxiv.org/abs/2605.29548": {
        "identity": "arxiv:2605.29548",
        "catalog_status": "missing_from_article_catalog",
        "prior_selection_status": "not_in_m028_selection",
    },
    "https://arxiv.org/abs/2605.26099": {
        "identity": "arxiv:2605.26099",
        "catalog_status": "already_cataloged",
        "prior_selection_status": "already_in_m028_selection",
    },
}
EXPECTED_COUNTS = {
    "url_refs": 4,
    "unique_normalized_identities": 4,
    "already_in_article_catalog": 2,
    "already_in_m028_selection": 1,
    "missing_from_article_catalog": 2,
    "new_to_m028_selection": 3,
}
FALSE_SELECTION_FLAGS = {
    "loader_owns_selection",
    "source_acquisition_completed",
    "raw_article_text_embedded",
    "binary_payload_embedded",
    "parser_ready_claimed",
    "chunk_ready_claimed",
    "kg_readiness_claimed",
    "graph_write_attempted",
    "production_persistence_attempted",
}
FALSE_REF_CLAIMS = {
    "source_acquired_now",
    "parser_ready_claimed",
    "chunk_ready_claimed",
    "graph_ready_claimed",
}


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"{path}: invalid JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValueError(f"{path}: expected a JSON object")
    return payload


def _catalog_identities(index: dict[str, Any]) -> set[str]:
    identities: set[str] = set()
    for article in index.get("articles", []):
        if not isinstance(article, dict):
            continue
        source = article.get("source_code")
        key = article.get("article_key")
        if not isinstance(source, str) or not isinstance(key, str):
            continue
        if source == "arxiv":
            identities.add(f"arxiv:{key.removesuffix('v1')}")
        elif source == "nature":
            identities.add(f"nature:articles_{key}")
        elif source == "company_blog":
            identities.add(f"company_blog:{key}")
        else:
            identities.add(f"{source}:{key}")
    return identities


def _m028_identities(selection: dict[str, Any]) -> set[str]:
    identities: set[str] = set()
    for ref in selection.get("refs", []):
        if isinstance(ref, dict) and isinstance(ref.get("normalized_identity"), str):
            identities.add(ref["normalized_identity"])
    return identities


def validate_selection(selection: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if selection.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        errors.append("M030_INTAKE_SCHEMA: unexpected schema_version")
    if selection.get("selection_id") != EXPECTED_SELECTION_ID:
        errors.append("M030_INTAKE_ID: unexpected selection_id")

    counts = selection.get("counts")
    if not isinstance(counts, dict):
        errors.append("M030_INTAKE_COUNTS: counts must be an object")
    else:
        for key, expected in EXPECTED_COUNTS.items():
            if counts.get(key) != expected:
                errors.append(f"M030_INTAKE_COUNTS: {key} expected {expected}, got {counts.get(key)!r}")

    refs = selection.get("refs")
    if not isinstance(refs, list):
        errors.append("M030_INTAKE_REFS: refs must be a list")
        refs = []
    if len(refs) != EXPECTED_COUNTS["url_refs"]:
        errors.append(f"M030_INTAKE_REFS: expected 4 refs, got {len(refs)}")

    seen_urls: set[str] = set()
    seen_identities: set[str] = set()
    for index, ref in enumerate(refs):
        if not isinstance(ref, dict):
            errors.append(f"M030_INTAKE_REF_SHAPE: refs[{index}] must be an object")
            continue
        url = ref.get("url")
        identity = ref.get("normalized_identity")
        if not isinstance(url, str) or url not in EXPECTED_URLS:
            errors.append(f"M030_INTAKE_URL: unexpected url at refs[{index}]: {url!r}")
            continue
        expected = EXPECTED_URLS[url]
        seen_urls.add(url)
        if identity != expected["identity"]:
            errors.append(f"M030_INTAKE_IDENTITY: {url} expected {expected['identity']}, got {identity!r}")
        if isinstance(identity, str):
            seen_identities.add(identity)
        if ref.get("catalog_status") != expected["catalog_status"]:
            errors.append(f"M030_INTAKE_CATALOG_STATUS: {url} expected {expected['catalog_status']}")
        if ref.get("prior_selection_status") != expected["prior_selection_status"]:
            errors.append(f"M030_INTAKE_PRIOR_SELECTION_STATUS: {url} expected {expected['prior_selection_status']}")
        if ref.get("reachability_status") != "available_http_200":
            errors.append(f"M030_INTAKE_REACHABILITY: {url} must preserve available_http_200")
        claims = ref.get("unsafe_claims")
        if not isinstance(claims, dict):
            errors.append(f"M030_INTAKE_UNSAFE_CLAIMS: {url} unsafe_claims must be an object")
        else:
            for claim in FALSE_REF_CLAIMS:
                if claims.get(claim) is not False:
                    errors.append(f"M030_INTAKE_UNSAFE_CLAIMS: {url} {claim} must be false")

    missing_urls = sorted(set(EXPECTED_URLS) - seen_urls)
    if missing_urls:
        errors.append(f"M030_INTAKE_MISSING_URLS: missing {missing_urls}")
    if len(seen_identities) != EXPECTED_COUNTS["unique_normalized_identities"]:
        errors.append("M030_INTAKE_IDENTITIES: expected 4 unique normalized identities")

    flags = selection.get("safety_flags")
    if not isinstance(flags, dict):
        errors.append("M030_INTAKE_FLAGS: safety_flags must be an object")
    else:
        if flags.get("network_availability_checked") is not True:
            errors.append("M030_INTAKE_FLAGS: network_availability_checked must be true")
        for flag in FALSE_SELECTION_FLAGS:
            if flags.get(flag) is not False:
                errors.append(f"M030_INTAKE_FLAGS: {flag} must be false")
    return errors


def validate_report(report_path: Path, selection: dict[str, Any]) -> list[str]:
    text = report_path.read_text(encoding="utf-8")
    errors: list[str] = []
    for url, expected in EXPECTED_URLS.items():
        if url not in text:
            errors.append(f"M030_INTAKE_REPORT_URL: report missing {url}")
        status = expected["catalog_status"]
        expected_phrase = "already in `article_catalog`" if status == "already_cataloged" else "missing from `article_catalog`"
        if expected_phrase not in text:
            errors.append(f"M030_INTAKE_REPORT_STATUS: report missing phrase {expected_phrase!r}")
    required_phrases = [
        "two identities are now represented in `article_catalog`",
        "does not claim source acquisition",
        "parser readiness",
        "chunk readiness",
        "graph readiness",
        "stale missing-status drift",
    ]
    for phrase in required_phrases:
        if phrase not in text:
            errors.append(f"M030_INTAKE_REPORT_PHRASE: report missing {phrase!r}")
    # Keep this dependency explicit so future edits cannot silently point the
    # report at a different selection while leaving counts unchanged.
    if str(selection.get("selection_id")) not in text and "M029 Pipeline Architecture Audit Intake" not in text:
        errors.append("M030_INTAKE_REPORT_LINKAGE: report does not identify the intake selection")
    return errors


def validate_catalog_status(selection: dict[str, Any], catalog_index: dict[str, Any]) -> list[str]:
    catalog_ids = _catalog_identities(catalog_index)
    errors: list[str] = []
    for ref in selection.get("refs", []):
        if not isinstance(ref, dict):
            continue
        identity = ref.get("normalized_identity")
        status = ref.get("catalog_status")
        if status == "already_cataloged" and identity not in catalog_ids:
            errors.append(f"M030_INTAKE_CATALOG_LINK: {identity} marked cataloged but absent from catalog index")
        if status == "missing_from_article_catalog" and identity in catalog_ids:
            errors.append(f"M030_INTAKE_CATALOG_LINK: {identity} marked missing but present in catalog index")
    return errors


def validate_m028_status(selection: dict[str, Any], m028_selection: dict[str, Any]) -> list[str]:
    m028_ids = _m028_identities(m028_selection)
    errors: list[str] = []
    for ref in selection.get("refs", []):
        if not isinstance(ref, dict):
            continue
        identity = ref.get("normalized_identity")
        status = ref.get("prior_selection_status")
        if status == "already_in_m028_selection" and identity not in m028_ids:
            errors.append(f"M030_INTAKE_M028_LINK: {identity} marked in M028 but absent from M028 selection")
        if status == "not_in_m028_selection" and identity in m028_ids:
            errors.append(f"M030_INTAKE_M028_LINK: {identity} marked absent from M028 but present in M028 selection")
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selection", required=True, type=Path)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--catalog-index", type=Path)
    parser.add_argument("--m028-selection", type=Path)
    parser.add_argument("--validate-only", action="store_true", help="Validate existing local artifacts without fetching or writing.")
    args = parser.parse_args(argv)
    if not args.validate_only:
        parser.error("only --validate-only is supported; this verifier must not fetch or write")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    errors: list[str] = []
    try:
        selection = _load_json(args.selection)
        errors.extend(validate_selection(selection))
        if args.report:
            errors.extend(validate_report(args.report, selection))
        if args.catalog_index:
            errors.extend(validate_catalog_status(selection, _load_json(args.catalog_index)))
        if args.m028_selection:
            errors.extend(validate_m028_status(selection, _load_json(args.m028_selection)))
    except (OSError, ValueError) as exc:
        errors.append(f"M030_INTAKE_IO: {exc}")

    if errors:
        sys.stderr.write("M030 requested-ref intake validation failed:\n")
        for error in errors:
            sys.stderr.write(f"- {error}\n")
        return 1

    sys.stdout.write(
        "M030 requested-ref intake validation passed: "
        "4 refs, 2 cataloged, 2 missing from catalog, graph/import claims fail-closed.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
