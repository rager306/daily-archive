from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import pytest

sys.path.insert(0, str(Path("scripts").resolve()))
sys.path.insert(0, str(Path("src").resolve()))

# pyrefly: ignore [missing-import]
from augment_m073_evidence_paths import augment  # noqa: E402  # ty:ignore[unresolved-import]

from research_graph.application.extraction_benchmark import evaluate_files  # noqa: E402

ROOT = Path("artifacts/m073-parser-evidence-benchmark")
M072_FIXTURES = Path("artifacts/m072-reviewed-extraction-benchmark/fixtures")
M073_FIXTURES = ROOT / "fixtures"

FORBIDDEN_KEYS = {
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
FORBIDDEN_TEXT = (
    "raw_text",
    "raw_pdf_text",
    "model_payload",
    "graph_write_payload",
    "prompt:",
    "embedding:",
    "vector:",
)


def _load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def _walk_keys(value: Any) -> set[str]:
    if isinstance(value, dict):
        keys = set(value)
        for child in value.values():
            keys |= _walk_keys(child)
        return keys  # ty:ignore[invalid-return-type]
    if isinstance(value, list):
        keys: set[str] = set()
        for child in value:
            keys |= _walk_keys(child)
        return keys
    return set()


def test_m073_augmentation_is_deterministic(tmp_path: Path) -> None:
    coverage = augment(
        ROOT / "source-evidence-audit.json",
        M072_FIXTURES,
        tmp_path,
    )

    assert coverage == json.loads((M073_FIXTURES / "evidence-coverage.json").read_text())
    assert (tmp_path / "train-gold-evidence.jsonl").read_text() == (
        M073_FIXTURES / "train-gold-evidence.jsonl"
    ).read_text()
    assert (tmp_path / "validation-gold-evidence.jsonl").read_text() == (
        M073_FIXTURES / "validation-gold-evidence.jsonl"
    ).read_text()


def test_m073_each_case_has_evidence_refs_or_missing_diagnostics() -> None:
    for split in ("train", "validation"):
        for row in _load_jsonl(M073_FIXTURES / f"{split}-gold-evidence.jsonl"):
            diagnostics = row["evidence_path_diagnostics"]
            assert row["evidence_path_refs"] or diagnostics["missing_reasons"]
            assert diagnostics["evidence_status"] in {
                "parser_manifest_available",
                "canonical_pdf_only",
                "missing_canonical_pdf_and_parser_manifest",
            }
            assert diagnostics["evidence_ref_count"] == len(row["evidence_path_refs"])


def test_m073_outputs_remain_metadata_only() -> None:
    for path in sorted(M073_FIXTURES.glob("*.json*")):
        payload_text = path.read_text()
        for term in FORBIDDEN_TEXT:
            assert term not in payload_text
        if path.suffix == ".jsonl":
            records = _load_jsonl(path)
        else:
            records = [json.loads(payload_text)]
        for record in records:
            assert not (_walk_keys(record) & FORBIDDEN_KEYS)


def test_m073_augmented_gold_preserves_m072_metrics() -> None:
    expected = json.loads((M072_FIXTURES / "expected-metrics.json").read_text())

    for split in ("train", "validation"):
        metrics = evaluate_files(
            M073_FIXTURES / f"{split}-gold-evidence.jsonl",
            M072_FIXTURES / f"{split}-baseline-predictions.jsonl",
        )
        for key, expected_value in expected[split].items():
            if isinstance(expected_value, float):
                assert metrics[key] == pytest.approx(expected_value)
            else:
                assert metrics[key] == expected_value
