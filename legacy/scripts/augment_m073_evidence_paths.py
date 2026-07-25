#!/usr/bin/env python3
"""Augment M072 benchmark fixtures with M073 evidence path references.

Local-only deterministic utility. It copies fixture records and adds path-like
evidence references plus explicit missing diagnostics. It does not read PDF body
content, parser body content, model payloads, or graph data.
"""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path
from typing import Any

SPLIT_FILES = {
    "train": "train-gold.jsonl",
    "validation": "validation-gold.jsonl",
}

FORBIDDEN_OUTPUT_KEYS = {
    "body",
    "completion",
    "embedding",
    "embeddings",
    "graph_write_payload",
    "model_payload",
    "prompt",
    "prompts",
    "raw_pdf_text",
    "raw_text",
    "secret",
    "vector",
    "vectors",
}


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def _write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows) + "\n")


def _paper_id(row: dict[str, Any]) -> str:
    paper_id = row.get("paper_id", "")
    if not paper_id.startswith("arxiv:"):
        raise ValueError(f"unsupported paper_id: {paper_id!r}")
    return paper_id.split(":", 1)[1]


def _audit_by_id(audit: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {record["arxiv_id"]: record for record in audit["records"]}


def _evidence_payload(record: dict[str, Any]) -> dict[str, Any]:
    arxiv_id = record["arxiv_id"]
    refs: list[str] = []
    missing: list[str] = []

    parser_artifacts = record.get("parser_artifacts") or []
    for manifest in parser_artifacts:
        path = manifest.get("path")
        if path:
            refs.append(f"artifact:m061-parser-manifest:{arxiv_id}:{path}")

    if record.get("canonical_pdf"):
        refs.append(f"artifact:canonical-pdf:{arxiv_id}:{record['canonical_pdf']}")
    else:
        missing.append(f"missing:canonical_pdf:{arxiv_id}")

    if not parser_artifacts:
        missing.append(f"missing:parser_manifest:{arxiv_id}")

    return {
        "evidence_path_refs": refs,
        "evidence_path_diagnostics": {
            "canonical_pdf_exists": bool(record.get("canonical_pdf_exists")),
            "evidence_ref_count": len(refs),
            "evidence_status": record.get("evidence_status"),
            "missing_reasons": missing,
            "parser_manifest_count": int(record.get("parser_artifact_count") or 0),
        },
    }


def _reject_forbidden_keys(value: Any, path: str = "$.") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key in FORBIDDEN_OUTPUT_KEYS:
                raise ValueError(f"forbidden output key {path}{key}")
            _reject_forbidden_keys(child, f"{path}{key}.")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_forbidden_keys(child, f"{path}[{index}].")


def augment(audit_path: Path, fixture_dir: Path, output_dir: Path) -> dict[str, Any]:
    audit = json.loads(audit_path.read_text())
    records = _audit_by_id(audit)
    output_dir.mkdir(parents=True, exist_ok=True)

    coverage: dict[str, Any] = {
        "milestone": "M073-2c89cc",
        "source_milestone": "M072-wqjtfv",
        "splits": {},
    }

    for split, filename in SPLIT_FILES.items():
        rows = _load_jsonl(fixture_dir / filename)
        augmented_rows: list[dict[str, Any]] = []
        split_counts = defaultdict(int)

        for row in rows:
            arxiv_id = _paper_id(row)
            if arxiv_id not in records:
                raise KeyError(f"audit record missing for {arxiv_id}")
            payload = _evidence_payload(records[arxiv_id])
            augmented = dict(row)
            augmented.update(payload)
            _reject_forbidden_keys(augmented)
            augmented_rows.append(augmented)

            split_counts["case_count"] += 1
            if payload["evidence_path_diagnostics"]["parser_manifest_count"] > 0:
                split_counts["parser_manifest_available"] += 1
            if payload["evidence_path_diagnostics"]["canonical_pdf_exists"]:
                split_counts["canonical_pdf_available"] += 1
            if payload["evidence_path_diagnostics"]["missing_reasons"]:
                split_counts["cases_with_missing_diagnostics"] += 1

        _write_jsonl(output_dir / f"{split}-gold-evidence.jsonl", augmented_rows)
        case_count = split_counts["case_count"] or 1
        coverage["splits"][split] = {
            "case_count": split_counts["case_count"],
            "canonical_pdf_available": split_counts["canonical_pdf_available"],
            "canonical_pdf_coverage": split_counts["canonical_pdf_available"] / case_count,
            "parser_manifest_available": split_counts["parser_manifest_available"],
            "parser_manifest_coverage": split_counts["parser_manifest_available"] / case_count,
            "cases_with_missing_diagnostics": split_counts["cases_with_missing_diagnostics"],
        }

    output = output_dir / "evidence-coverage.json"
    output.write_text(json.dumps(coverage, indent=2, sort_keys=True) + "\n")
    return coverage


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--audit", required=True, type=Path)
    parser.add_argument("--fixture-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    coverage = augment(args.audit, args.fixture_dir, args.output_dir)
    print(json.dumps({"status": "PASS", "splits": coverage["splits"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
