"""Wave B extraction quality baseline.

Composes reviewed extraction harness APIs over reviewed fixtures:
``load_reviewed_extraction_split`` → staged scores → ``decide_gate_verdict``.

Never enables DSPy optimizers, never authorizes import/graph writes.
Optional human_go stamp is a durable authorization record for Wave B work
(not import authorization).
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal

from research_graph.application.extraction_ablations import (
    decide_gate_verdict,
    run_staged_reviewed_run,
)
from research_graph.application.reviewed_extraction_fixtures import (
    load_jsonl_records,
    load_reviewed_extraction_split,
)

SCHEMA_VERSION = "wave-b-extraction-baseline.v1"
DEFAULT_HUMAN_GO_STAMP = Path("artifacts/wave-b/human_go.json")

GateVerdict = Literal["proceed", "repair", "stop"]


@dataclass(frozen=True, slots=True)
class WaveBExtractionBaselinePackage:
    schema_version: str
    train_case_count: int
    validation_case_count: int
    train_metrics: dict[str, Any]
    validation_metrics: dict[str, Any]
    gate_verdict: GateVerdict
    gate_reasons: tuple[str, ...]
    leakage_clean: bool
    diagnostics: tuple[str, ...]
    human_go: bool = True
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("wave B extraction baseline cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("wave B baseline cannot enable DSPy optimizers")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "train_case_count": self.train_case_count,
            "validation_case_count": self.validation_case_count,
            "train_metrics": dict(self.train_metrics),
            "validation_metrics": dict(self.validation_metrics),
            "gate_verdict": self.gate_verdict,
            "gate_reasons": list(self.gate_reasons),
            "leakage_clean": self.leakage_clean,
            "diagnostics": list(self.diagnostics),
            "human_go": self.human_go,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Wave B extraction quality baseline on reviewed extraction fixtures; "
                "reviewed harness only; not DSPy optimizer; not graph import"
            ),
        }


def write_human_go_stamp(
    path: Path,
    *,
    authorized_by: str,
    decision_ref: str,
    note: str = "Wave B extraction quality authorized",
    force_rewrite: bool = False,
) -> dict[str, Any]:
    """Persist Wave B human go stamp (never import authorization).

    Refuses to mutate an existing valid stamp unless force_rewrite=True.
    Prevents accidental authorized_at bumps and silent re-auth.
    """
    path = Path(path)
    existing: dict[str, Any] | None = None
    if path.is_file():
        try:
            raw = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            raw = None
        if isinstance(raw, dict) and raw.get("human_go") is True:
            if raw.get("import_eligible") is True or raw.get("graph_writes_allowed") is True:
                existing = None
            else:
                existing = raw
    if existing is not None and not force_rewrite:
        return dict(existing)
    payload: dict[str, Any] = {
        "schema_version": "wave-b-human-go.v1",
        "human_go": True,
        "authorized_by": authorized_by,
        "decision_ref": decision_ref,
        "authorized_at": datetime.now(UTC).isoformat(),
        "note": note,
        "import_eligible": False,
        "graph_writes_allowed": False,
        "dspy_optimizer_enabled": False,
    }
    if force_rewrite and existing is not None:
        payload["note"] = f"{note} (force_rewrite of prior stamp)"
        payload["prior_authorized_at"] = existing.get("authorized_at")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return payload

def read_human_go_stamp(path: Path) -> dict[str, Any] | None:
    """Read human go stamp if present and valid."""
    path = Path(path)
    if not path.is_file():
        return None
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    if not isinstance(data, dict):
        return None
    if data.get("human_go") is not True:
        return None
    # hard fail-closed: stamp must never claim import
    if data.get("import_eligible") is True or data.get("graph_writes_allowed") is True:
        return None
    return data


def build_wave_b_extraction_baseline(
    *,
    human_go: bool = True,
    fixtures_root: Path | None = None,
) -> WaveBExtractionBaselinePackage:
    """Score M072 train+validation baseline predictions (reviewed harness)."""
    # load_reviewed_extraction_split defaults to reviewed fixture root; cwd is repo root in tests/ops
    if fixtures_root is not None:
        # temporary: monkey via chdir not needed if we load manually when root given
        root = Path(fixtures_root)
        train_gold = load_jsonl_records(root / "train-gold.jsonl")
        train_pred = load_jsonl_records(root / "train-baseline-predictions.jsonl")
        val_gold = load_jsonl_records(root / "validation-gold.jsonl")
        val_pred = load_jsonl_records(root / "validation-baseline-predictions.jsonl")
    else:
        train_gold, train_pred = load_reviewed_extraction_split("train")
        val_gold, val_pred = load_reviewed_extraction_split("validation")

    train_staged = run_staged_reviewed_run(
        train_gold,
        train_pred,
        target_count=max(len(train_gold), 1),
        split_name="train",
    )
    val_staged = run_staged_reviewed_run(
        val_gold,
        val_pred,
        target_count=max(len(val_gold), 1),
        split_name="validation",
    )
    # Gate on train metrics (larger split); validation reported separately
    gate = decide_gate_verdict(
        train_staged.metrics,
        paper_count=train_staged.paper_count,
    )
    diagnostics = (
        f"train_cases:{len(train_gold)}",
        f"validation_cases:{len(val_gold)}",
        f"gate_verdict:{gate.verdict}",
        f"train_entity_f1:{train_staged.metrics.get('entity_f1')}",
        f"train_relation_f1:{train_staged.metrics.get('relation_f1')}",
        f"val_entity_f1:{val_staged.metrics.get('entity_f1')}",
        "dspy:false",
        "import_write_fail_closed",
        "wave_b_extraction_baseline_only",
        "reviewed_harness_reuse",
    )
    return WaveBExtractionBaselinePackage(
        schema_version=SCHEMA_VERSION,
        train_case_count=len(train_gold),
        validation_case_count=len(val_gold),
        train_metrics=dict(train_staged.metrics),
        validation_metrics=dict(val_staged.metrics),
        gate_verdict=gate.verdict,  # type: ignore[arg-type]
        gate_reasons=tuple(gate.reasons),
        leakage_clean=bool(
            train_staged.leakage_clean and val_staged.leakage_clean and gate.leakage_clean
        ),
        diagnostics=diagnostics,
        human_go=human_go,
    )


__all__ = [
    "DEFAULT_HUMAN_GO_STAMP",
    "SCHEMA_VERSION",
    "WaveBExtractionBaselinePackage",
    "build_wave_b_extraction_baseline",
    "read_human_go_stamp",
    "write_human_go_stamp",
]
