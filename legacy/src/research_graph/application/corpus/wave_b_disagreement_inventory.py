"""Wave B disagreement inventory over reviewed extraction fixtures.

Rolls up DisagreementEvidence from score_reviewed_split without loading raw
article text, without DSPy, without import.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from research_graph.application.reviewed_extraction_fixtures import (
    load_jsonl_records,
    load_reviewed_extraction_split,
)
from research_graph.application.reviewed_extraction_metrics import (
    score_reviewed_split,
)

SCHEMA_VERSION = "wave-b-disagreement-inventory.v1"


@dataclass(frozen=True, slots=True)
class WaveBDisagreementInventoryPackage:
    schema_version: str
    train_case_count: int
    validation_case_count: int
    train_disagreement_count: int
    validation_disagreement_count: int
    disagreement_kind_counts: dict[str, int]
    train_entity_f1: float
    validation_entity_f1: float
    leakage_clean: bool
    samples: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("disagreement inventory cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("disagreement inventory cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "train_case_count": self.train_case_count,
            "validation_case_count": self.validation_case_count,
            "train_disagreement_count": self.train_disagreement_count,
            "validation_disagreement_count": self.validation_disagreement_count,
            "disagreement_kind_counts": dict(self.disagreement_kind_counts),
            "train_entity_f1": self.train_entity_f1,
            "validation_entity_f1": self.validation_entity_f1,
            "leakage_clean": self.leakage_clean,
            "samples": list(self.samples),
            "diagnostics": list(self.diagnostics),
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave B disagreement rollup on reviewed extraction fixtures; "
                "not DSPy; not import; metadata only"
            ),
        }


def inventory_reviewed_extraction_disagreements(
    *,
    fixtures_root: Path | None = None,
    sample_limit: int = 20,
) -> WaveBDisagreementInventoryPackage:
    """Score M072 train+validation and aggregate disagreement kinds."""
    if fixtures_root is not None:
        root = Path(fixtures_root)
        train_gold = load_jsonl_records(root / "train-gold.jsonl")
        train_pred = load_jsonl_records(root / "train-baseline-predictions.jsonl")
        val_gold = load_jsonl_records(root / "validation-gold.jsonl")
        val_pred = load_jsonl_records(root / "validation-baseline-predictions.jsonl")
    else:
        train_gold, train_pred = load_reviewed_extraction_split("train")
        val_gold, val_pred = load_reviewed_extraction_split("validation")

    train = score_reviewed_split(train_gold, train_pred, split_name="train")
    val = score_reviewed_split(val_gold, val_pred, split_name="validation")
    kinds: Counter[str] = Counter()
    samples: list[dict[str, Any]] = []
    for d in (*train.disagreements, *val.disagreements):
        kinds[d.kind] += 1
        if len(samples) < sample_limit:
            samples.append(
                {
                    "case_id": d.case_id,
                    "kind": d.kind,
                    "detail": d.detail,
                    "import_eligible": False,
                }
            )

    diagnostics = (
        f"train_cases:{train.case_count}",
        f"val_cases:{val.case_count}",
        f"train_disagreements:{len(train.disagreements)}",
        f"val_disagreements:{len(val.disagreements)}",
        f"kinds:{dict(kinds)}",
        "dspy:false",
        "import_write_fail_closed",
        "wave_b_disagreement_inventory_only",
        "reviewed_harness_reuse",
    )
    return WaveBDisagreementInventoryPackage(
        schema_version=SCHEMA_VERSION,
        train_case_count=train.case_count,
        validation_case_count=val.case_count,
        train_disagreement_count=len(train.disagreements),
        validation_disagreement_count=len(val.disagreements),
        disagreement_kind_counts=dict(sorted(kinds.items())),
        train_entity_f1=float(train.metrics.get("entity_f1") or 0.0),
        validation_entity_f1=float(val.metrics.get("entity_f1") or 0.0),
        leakage_clean=bool(train.leakage_clean and val.leakage_clean),
        samples=tuple(samples),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "WaveBDisagreementInventoryPackage",
    "inventory_reviewed_extraction_disagreements",
]
