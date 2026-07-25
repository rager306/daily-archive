"""Reviewed extraction fixture loaders (application-pure).

Loads gold + baseline prediction JSONL splits for the reviewed extraction
benchmark. Default on-disk root is a legacy artifact path; callers may pass
an explicit ``fixtures_root``.

No LLM, no DSPy, no import authorization.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

#: Default fixture root (historical artifact tree; not a public API name).
DEFAULT_REVIEWED_EXTRACTION_FIXTURES_ROOT = Path(
    "artifacts/m072-reviewed-extraction-benchmark/fixtures"
)


def load_jsonl_records(path: Path) -> list[dict[str, Any]]:
    """Load a JSONL file into a list of object records."""
    rows: list[dict[str, Any]] = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        obj = json.loads(line)
        if isinstance(obj, dict):
            rows.append(obj)
    return rows


def load_reviewed_extraction_split(
    split: str = "train",
    *,
    fixtures_root: Path | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Load reviewed gold + baseline predictions for one split.

    Expected files under ``fixtures_root``:
      ``{split}-gold.jsonl``
      ``{split}-baseline-predictions.jsonl``
    """
    root = Path(fixtures_root) if fixtures_root is not None else DEFAULT_REVIEWED_EXTRACTION_FIXTURES_ROOT
    gold = load_jsonl_records(root / f"{split}-gold.jsonl")
    pred = load_jsonl_records(root / f"{split}-baseline-predictions.jsonl")
    return gold, pred


def load_reviewed_extraction_splits(
    *,
    fixtures_root: Path | None = None,
    splits: tuple[str, ...] = ("train", "validation"),
) -> dict[str, tuple[list[dict[str, Any]], list[dict[str, Any]]]]:
    """Load multiple reviewed splits keyed by split name."""
    return {
        name: load_reviewed_extraction_split(name, fixtures_root=fixtures_root)
        for name in splits
    }


__all__ = [
    "DEFAULT_REVIEWED_EXTRACTION_FIXTURES_ROOT",
    "load_jsonl_records",
    "load_reviewed_extraction_split",
    "load_reviewed_extraction_splits",
]
