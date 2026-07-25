"""Continuous hybrid-body chunk quality gate (M273 + M277 v2).

Samples real hybrid bodies and scores scholarly body quality + structure.
M277: prefer CanonicalDocument IR and markdown headings; demote newline-only
to soft_legacy (tracked, not hard structure). Never authorizes import.
"""

from __future__ import annotations

import json
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal

from research_graph.application.corpus.body_quality import assess_body_quality

SCHEMA_VERSION = "structure-chunk-quality-gate.v2"
CONTINUITY_GAP_CODE = "real_corpus_chunk_quality_not_continuously_gated"

GateSignal = Literal["blocked", "partial", "pass"]
StructureMode = Literal[
    "canonical_ir",
    "markdown_heading",
    "soft_legacy_newline",
    "none",
]

_HEADING_RE = re.compile(
    r"(?m)^(#{1,3}\s+\S+|Abstract|Introduction|Conclusion|References)\b"
)


@dataclass(frozen=True, slots=True)
class StructureScore:
    """Per-body structure score (M277)."""

    hard_ok: bool
    soft_legacy_ok: bool
    mode: StructureMode
    heading_count: int = 0
    section_count: int = 0
    grounded_blocks: int = 0
    table_or_figure: int = 0
    newline_count: int = 0
    newline_demoted: bool = False

    @property
    def structure_ok(self) -> bool:
        """True if hard IR/heading or soft legacy newline (body present signal)."""
        return self.hard_ok or self.soft_legacy_ok

    def to_dict(self) -> dict[str, Any]:
        return {
            "hard_ok": self.hard_ok,
            "soft_legacy_ok": self.soft_legacy_ok,
            "mode": self.mode,
            "heading_count": self.heading_count,
            "section_count": self.section_count,
            "grounded_blocks": self.grounded_blocks,
            "table_or_figure": self.table_or_figure,
            "newline_count": self.newline_count,
            "newline_demoted": self.newline_demoted,
        }


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
    # M277 v2 metrics (defaults keep older constructors rare)
    ir_hard_count: int = 0
    markdown_heading_count: int = 0
    newline_demoted_count: int = 0
    soft_legacy_count: int = 0

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
            "ir_hard_count": self.ir_hard_count,
            "markdown_heading_count": self.markdown_heading_count,
            "newline_demoted_count": self.newline_demoted_count,
            "soft_legacy_count": self.soft_legacy_count,
            "sample_diagnostics": list(self.sample_diagnostics),
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "gap_code": CONTINUITY_GAP_CODE,
            "note": (
                "v2: scholarly body_quality + IR/heading hard structure; "
                "newline-only demoted to soft_legacy. Clears continuity gap when "
                "pass_rate>=min_pass_rate and sampled>=min_sample. Never import. "
                "Operators: ir_hard_count==0 with pass means weak structure signal."
            ),
        }


def _count_ir_signals(canonical: Mapping[str, Any]) -> tuple[int, int, int, int]:
    """Return (sections, headings, grounded_blocks, table_or_figure)."""
    sections = canonical.get("sections")
    blocks = canonical.get("blocks")
    section_count = len(sections) if isinstance(sections, list) else 0
    heading = 0
    grounded = 0
    table_fig = 0

    def walk_blocks(items: Any) -> None:
        nonlocal heading, grounded, table_fig
        if not isinstance(items, list):
            return
        for b in items:
            if not isinstance(b, Mapping):
                continue
            kind = str(b.get("kind") or "").casefold()
            if kind in {"heading", "section"}:
                heading += 1
            if kind in {"table", "figure"}:
                table_fig += 1
            spans = b.get("spans") or []
            if isinstance(spans, list):
                for sp in spans:
                    if not isinstance(sp, Mapping):
                        continue
                    if sp.get("page") is not None or sp.get("bbox") is not None:
                        grounded += 1
                        break

    if isinstance(sections, list):
        for sec in sections:
            if not isinstance(sec, Mapping):
                continue
            walk_blocks(sec.get("blocks"))
            children = sec.get("children")
            if isinstance(children, list):
                for ch in children:
                    if isinstance(ch, Mapping):
                        walk_blocks(ch.get("blocks"))
    walk_blocks(blocks)
    # also honor diagnostics counter if present
    for d in canonical.get("diagnostics") or ():
        if isinstance(d, str) and d.startswith("blocks_with_page_or_bbox:"):
            try:
                n = int(d.split(":", 1)[1])
                grounded = max(grounded, n)
            except ValueError:
                pass
    return section_count, heading, grounded, table_fig


def score_structure_signals(
    *,
    body_text: str,
    canonical: Mapping[str, Any] | None = None,
) -> StructureScore:
    """Score structure: IR hard > markdown heading hard > newline soft_legacy.

    Newline-only no longer counts as hard structure (M277 demotion).
    """
    text = body_text or ""
    newlines = text.count("\n")
    headings = len(_HEADING_RE.findall(text)) if text else 0

    if isinstance(canonical, Mapping):
        sec_n, head_n, grounded, tf = _count_ir_signals(canonical)
        # hard IR: at least one section or heading block, and some grounding or multi-block
        ir_hard = (sec_n >= 1 or head_n >= 1) and (grounded >= 1 or head_n + tf >= 2)
        if ir_hard:
            return StructureScore(
                hard_ok=True,
                soft_legacy_ok=False,
                mode="canonical_ir",
                heading_count=max(headings, head_n),
                section_count=sec_n,
                grounded_blocks=grounded,
                table_or_figure=tf,
                newline_count=newlines,
                newline_demoted=False,
            )

    if headings >= 1 and len(text.strip()) >= 200:
        return StructureScore(
            hard_ok=True,
            soft_legacy_ok=False,
            mode="markdown_heading",
            heading_count=headings,
            section_count=0,
            grounded_blocks=0,
            table_or_figure=0,
            newline_count=newlines,
            newline_demoted=False,
        )

    # soft legacy: enough body + many newlines (old heuristic, demoted)
    if len(text.strip()) >= 200 and newlines >= 8:
        return StructureScore(
            hard_ok=False,
            soft_legacy_ok=True,
            mode="soft_legacy_newline",
            heading_count=headings,
            newline_count=newlines,
            newline_demoted=True,
        )

    return StructureScore(
        hard_ok=False,
        soft_legacy_ok=False,
        mode="none",
        heading_count=headings,
        newline_count=newlines,
        newline_demoted=False,
    )


def _structure_signal_ok(body: str) -> bool:
    """Back-compat helper: hard or soft_legacy structure present."""
    return score_structure_signals(body_text=body, canonical=None).structure_ok


def load_sibling_canonical(body_path: Path) -> dict[str, Any] | None:
    """Load paper.canonical.json next to paper.hybrid.body.md if present."""
    name = body_path.name
    if name.endswith(".hybrid.body.md"):
        paper_id = name[: -len(".hybrid.body.md")]
    else:
        paper_id = body_path.stem
    candidates = [
        body_path.with_name(f"{paper_id}.canonical.json"),
        body_path.parent / f"{paper_id}.canonical.json",
    ]
    for cand in candidates:
        if not cand.is_file():
            continue
        try:
            data = json.loads(cand.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if isinstance(data, dict):
            return data
    return None


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
    ir_hard = 0
    md_heading = 0
    newline_demoted = 0
    soft_legacy = 0
    sample_diag: list[str] = []
    for path in paths:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            low += 1
            sample_diag.append(f"unreadable:{path.name}")
            continue
        report = assess_body_quality(text, profile="scholarly")
        canon = load_sibling_canonical(path)
        score = score_structure_signals(body_text=text, canonical=canon)
        struct_ok = score.structure_ok
        if score.mode == "canonical_ir" and score.hard_ok:
            ir_hard += 1
        elif score.mode == "markdown_heading" and score.hard_ok:
            md_heading += 1
        if score.newline_demoted:
            newline_demoted += 1
            soft_legacy += 1

        # Hybrid markdown often trips soft_signal via heading symbols; with structure
        # present treat as continuous-gate pass (still not import).
        if report.status == "ok" and struct_ok:
            passed += 1
            status = f"ok:{score.mode}"
        elif report.status == "soft_signal" and struct_ok:
            passed += 1
            soft += 1
            status = f"soft_pass_structure:{score.mode}"
        elif report.status == "soft_signal":
            soft += 1
            status = f"soft_no_structure:{score.mode}"
        else:
            low += 1
            status = f"{report.status}:{score.mode}"
        if len(sample_diag) < 12:
            sample_diag.append(
                f"{path.name}:{status}:words={report.word_count}:"
                f"hard={score.hard_ok}:legacy={score.soft_legacy_ok}"
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
        f"ir_hard_count:{ir_hard}",
        f"markdown_heading_count:{md_heading}",
        f"newline_demoted:{newline_demoted}",
        f"soft_legacy_count:{soft_legacy}",
        "structure_gate_v2",
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
        ir_hard_count=ir_hard,
        markdown_heading_count=md_heading,
        newline_demoted_count=newline_demoted,
        soft_legacy_count=soft_legacy,
    )


__all__ = [
    "SCHEMA_VERSION",
    "CONTINUITY_GAP_CODE",
    "GateSignal",
    "StructureScore",
    "StructureChunkQualityGatePackage",
    "score_structure_signals",
    "load_sibling_canonical",
    "discover_hybrid_body_paths",
    "evaluate_structure_chunk_quality_gate",
]
