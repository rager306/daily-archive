"""Continuous hybrid-body chunk quality gate (M273).

Samples real hybrid bodies and scores scholarly body quality + lightweight
structure signals (headings/sections). Closes the continuity gap
``real_corpus_chunk_quality_not_continuously_gated`` when pass rate meets
threshold. Never authorizes import.
"""

from __future__ import annotations

import re
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

from research_graph.application.corpus.body_quality import assess_body_quality

SCHEMA_VERSION = "structure-chunk-quality-gate.v1"
CONTINUITY_GAP_CODE = "real_corpus_chunk_quality_not_continuously_gated"

GateSignal = Literal["blocked", "partial", "pass"]

_HEADING_RE = re.compile(r"(?m)^(#{1,3}\s+\S+|Abstract|Introduction|Conclusion|References)\b")


@dataclass(frozen=True, slots=True)
class StructureChunkQualityGatePackage:
    schema_version: str
    gate_signal: GateSignal
    sampled: int
    passed: int
    soft_signal: int
    low_quality: int
    pass_rate: float
    min_pass_rate: float
    min_sample: int
    continuity_gap_cleared: bool
    sample_diagnostics: tuple[str, ...]
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("chunk quality gate cannot authorize import/writes")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "gate_signal": self.gate_signal,
            "sampled": self.sampled,
            "passed": self.passed,
            "soft_signal": self.soft_signal,
            "low_quality": self.low_quality,
            "pass_rate": self.pass_rate,
            "min_pass_rate": self.min_pass_rate,
            "min_sample": self.min_sample,
            "continuity_gap_cleared": self.continuity_gap_cleared,
            "sample_diagnostics": list(self.sample_diagnostics),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "gap_code": CONTINUITY_GAP_CODE,
            "note": (
                "Continuous sample of hybrid bodies via scholarly body_quality + "
                "heading/section signals. Clears continuity structure gap when "
                "pass_rate>=min_pass_rate and sampled>=min_sample. Never import."
            ),
        }


def _structure_signal_ok(body: str) -> bool:
    if not body or len(body.strip()) < 200:
        return False
    headings = len(_HEADING_RE.findall(body))
    # hybrid bodies often have markdown headings or section words
    newlines = body.count("\n")
    return headings >= 1 or newlines >= 8


def discover_hybrid_body_paths(
    body_roots: Sequence[Path],
    *,
    limit: int = 40,
) -> list[Path]:
    """Deterministic sample of hybrid body files under roots."""
    found: list[Path] = []
    for root in body_roots:
        root_p = Path(root)
        if not root_p.is_dir():
            continue
        for path in sorted(root_p.rglob("*.hybrid.body.md")):
            if path.is_file():
                found.append(path)
            if len(found) >= max(1, int(limit) * 3):
                break
        if len(found) >= max(1, int(limit) * 3):
            break
    # stable subsample: every k-th to spread roots
    if len(found) <= limit:
        return found
    step = max(1, len(found) // int(limit))
    return found[::step][: int(limit)]


def evaluate_structure_chunk_quality_gate(
    body_roots: Sequence[Path],
    *,
    sample_limit: int = 40,
    min_sample: int = 10,
    min_pass_rate: float = 0.55,
) -> StructureChunkQualityGatePackage:
    """Run continuous quality gate over hybrid body sample (read-only)."""
    paths = discover_hybrid_body_paths(body_roots, limit=sample_limit)
    passed = 0
    soft = 0
    low = 0
    sample_diag: list[str] = []
    for path in paths:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            low += 1
            sample_diag.append(f"unreadable:{path.name}")
            continue
        report = assess_body_quality(text, profile="scholarly")
        struct_ok = _structure_signal_ok(text)
        # Hybrid markdown often trips soft_signal via heading symbols; with structure
        # present treat as continuous-gate pass (still not import).
        if report.status == "ok" and struct_ok:
            passed += 1
            status = "ok"
        elif report.status == "soft_signal" and struct_ok:
            passed += 1
            soft += 1
            status = "soft_pass_structure"
        elif report.status == "soft_signal":
            soft += 1
            status = "soft_no_structure"
        else:
            low += 1
            status = report.status
        if len(sample_diag) < 12:
            sample_diag.append(
                f"{path.name}:{status}:words={report.word_count}:struct={struct_ok}"
            )

    sampled = len(paths)
    pass_rate = round(passed / sampled, 4) if sampled else 0.0
    min_rate = float(min_pass_rate)
    min_n = int(min_sample)

    if sampled < min_n:
        signal: GateSignal = "blocked" if sampled == 0 else "partial"
        cleared = False
    elif pass_rate >= min_rate:
        signal = "pass"
        cleared = True
    elif pass_rate >= min_rate * 0.7:
        signal = "partial"
        cleared = False
    else:
        signal = "blocked"
        cleared = False

    diagnostics = (
        f"sampled:{sampled}",
        f"passed:{passed}",
        f"soft_signal:{soft}",
        f"low_quality:{low}",
        f"pass_rate:{pass_rate}",
        f"min_pass_rate:{min_rate}",
        f"min_sample:{min_n}",
        f"gate_signal:{signal}",
        f"continuity_gap_cleared:{cleared}",
        "import_write_fail_closed",
        "structure_chunk_quality_gate_only",
    )
    return StructureChunkQualityGatePackage(
        schema_version=SCHEMA_VERSION,
        gate_signal=signal,
        sampled=sampled,
        passed=passed,
        soft_signal=soft,
        low_quality=low,
        pass_rate=pass_rate,
        min_pass_rate=min_rate,
        min_sample=min_n,
        continuity_gap_cleared=cleared,
        sample_diagnostics=tuple(sample_diag),
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "CONTINUITY_GAP_CODE",
    "GateSignal",
    "StructureChunkQualityGatePackage",
    "discover_hybrid_body_paths",
    "evaluate_structure_chunk_quality_gate",
]
