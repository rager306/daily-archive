"""Adapter: declaration synchronisation gap axis.

The second axis of the M516 scorecard answers a single question with a number:
*how many things does the DATA declare that the code does not enforce?* The
number's goal is zero, so a regression is an increase, and every gap is named
individually rather than aggregated, so the figure is actionable.

Three independent declaration surfaces are reconciled against their
enforcement counterparts:

1. **Hard metrics without a disposition.** ``scripts/governor/checks/
   article_context_assurance.yaml`` declares ``metric_definitions`` and marks
   some ``kind: hard``, plus a ``metric_dispositions.records`` block that covers
   every declared hard metric (CONTEXT G2). A ``computed`` record is covered
   when its ``computation`` names a symbol that occurs as a whole token in
   production Rust sources (``crates/*/src``); a ``downgraded`` record is
   covered when it carries a non-empty ``owner`` and ``reason``. A declared hard
   metric with no record, or a malformed one, is a gap: the policy asserts a 1.0
   gate that no code produces and no owner has downgraded. The block is a
   required input, so a missing or malformed one raises rather than letting the
   axis report a vacuous zero.

2. **Stage hard invariants without a disposition.**
   ``data/ontology/pipeline_stage_contracts.yaml`` attaches ``hard_invariants``
   to each stage and carries a ``stage_invariant_dispositions.records`` block
   that covers every declared name (CONTEXT G3). A distinct invariant name is
   covered when its record is well formed: ``checked`` with a non-empty
   ``check_id``, or ``downgraded`` with a non-empty ``owner`` and ``reason``.
   A declared name with no record, or a record shaped neither way, is a gap.
   The block is a required input, so a missing or malformed one raises rather
   than letting the axis report a vacuous zero.

3. **Prompt call forms without a resolvable registry entry.**
   ``data/ontology/prompt_tasks.yaml`` is the prompt-task registry. Its
   ``prompt_contract.task_rule_bindings`` keys are prompt-bearing task forms;
   ``crates/da-adapters/src/prompt_task_registry.rs`` validates that each
   binding *value* resolves to a rule, but never that each binding *key*
   resolves to a ``tasks[]`` entry. A binding key with no registered task is a
   gap (CONTEXT G5 — ``critic``).

Whole-token matching is deliberate: a substring scan would count
``exact_lineage`` inside the test name ``exact_lineage_and_policy_locks`` as a
mapping, hiding exactly the decorative names this axis exists to expose.

The three declaration files are required inputs. A missing one raises, so the
axis can never report a vacuous ``0`` green by scanning nothing, and a missing
``crates/`` directory raises for the same reason.
"""

from __future__ import annotations

import glob
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

import yaml

__all__ = [
    "DeclarationGap",
    "DeclarationSyncGap",
    "build_declaration_sync_gap",
    "observe_declaration_sync_gap",
]

#: Declaration sources, relative to the repository root.
_METRIC_DECLARATION = "scripts/governor/checks/article_context_assurance.yaml"
_STAGE_DECLARATION = "data/ontology/pipeline_stage_contracts.yaml"
_PROMPT_REGISTRY = "data/ontology/prompt_tasks.yaml"

#: Where a hard metric's production computation would live.
_METRIC_COMPUTATION_SURFACES: tuple[str, ...] = ("crates/*/src/**/*.rs",)

#: Gap class identifiers, stable for the machine-readable payload.
HARD_METRIC_WITHOUT_COMPUTATION = "hard_metric_without_computation"
STAGE_INVARIANT_WITHOUT_CHECK = "stage_invariant_without_check"
PROMPT_FORM_WITHOUT_REGISTRY_ENTRY = "prompt_form_without_registry_entry"

_GAP_CLASS_ORDER = (
    HARD_METRIC_WITHOUT_COMPUTATION,
    STAGE_INVARIANT_WITHOUT_CHECK,
    PROMPT_FORM_WITHOUT_REGISTRY_ENTRY,
)


def _require_file(root: Path, rel: str) -> Path:
    path = root / rel
    if not path.is_file():
        raise FileNotFoundError(
            f"{rel} is missing under {root}; refusing to report a vacuous zero-gap axis"
        )
    return path


def _load_yaml(root: Path, rel: str) -> dict:
    path = _require_file(root, rel)
    try:
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as exc:  # pragma: no cover - exercised by malformed fixtures
        raise ValueError(f"{rel} is not valid YAML: {exc}") from exc
    if not isinstance(data, dict):
        raise ValueError(f"{rel} must parse to a mapping, got {type(data).__name__}")
    return data


def _surface_files(root: Path, patterns: Iterable[str]) -> list[Path]:
    """Every file matching ``patterns`` under ``root``, deduplicated and sorted."""
    seen: dict[str, Path] = {}
    for pattern in patterns:
        for match in glob.glob(str(root / pattern), recursive=True):
            seen[str(Path(match))] = Path(match)
    return [seen[key] for key in sorted(seen)]


def _read_surfaces(files: Sequence[Path]) -> list[tuple[Path, str]]:
    out: list[tuple[Path, str]] = []
    for path in files:
        try:
            out.append((path, path.read_text(encoding="utf-8")))
        except (OSError, UnicodeDecodeError):
            # A surface we cannot read is not a mapping; skipping keeps the axis
            # from counting a file that does not exist as enforcement.
            continue
    return out


def _token_re(token: str) -> re.Pattern[str]:
    """Whole-token pattern: ``_`` is a word character, so ``exact_lineage`` does
    not match inside ``exact_lineage_and_policy_locks``."""
    return re.compile(r"(?<![A-Za-z0-9_])" + re.escape(token) + r"(?![A-Za-z0-9_])")


def _contains_token(surfaces: Sequence[tuple[Path, str]], token: str) -> bool:
    pattern = _token_re(token)
    return any(pattern.search(text) is not None for _, text in surfaces)


# ── gap classes ──────────────────────────────────────────────────────────────


@dataclass(frozen=True)
class DeclarationGap:
    """One declaration the code does not enforce, named individually."""

    gap_class: str
    name: str
    declared_in: str
    detail: str

    def to_dict(self) -> dict:
        return {
            "gap_class": self.gap_class,
            "name": self.name,
            "declared_in": self.declared_in,
            "detail": self.detail,
        }


def _hard_metric_names(root: Path) -> list[str]:
    metric_defs = _load_yaml(root, _METRIC_DECLARATION).get("metric_definitions")
    if not isinstance(metric_defs, dict):
        raise ValueError(f"{_METRIC_DECLARATION} has no metric_definitions mapping")
    names: list[str] = []
    for name, spec in metric_defs.items():
        if isinstance(spec, dict) and spec.get("kind") == "hard":
            names.append(str(name))
    return names


def _metric_dispositions(root: Path) -> dict:
    """The ``metric_dispositions.records`` mapping, required to exist.

    Refusing a missing or non-mapping block keeps the axis fail-closed: without
    it a fresh declaration file would silently read as zero gaps.
    """
    block = _load_yaml(root, _METRIC_DECLARATION).get("metric_dispositions")
    if not isinstance(block, dict):
        raise ValueError(
            f"{_METRIC_DECLARATION} has no metric_dispositions mapping; every "
            "declared hard metric must carry a computed or downgraded disposition"
        )
    records = block.get("records")
    if not isinstance(records, dict):
        raise ValueError(
            f"{_METRIC_DECLARATION} metric_dispositions.records must be a mapping"
        )
    return records


def _metric_disposition_gap_reason(
    record: object, surfaces: Sequence[tuple[Path, str]]
) -> str | None:
    """``None`` when ``record`` well-formedly covers its metric, else the cause."""
    if not isinstance(record, dict):
        return f"metric disposition record is not a mapping, got {type(record).__name__}"
    disposition = record.get("disposition")
    if disposition == "computed":
        computation = record.get("computation")
        if not isinstance(computation, str) or not computation.strip():
            return "disposition 'computed' carries no non-empty computation"
        symbol = computation.rsplit("::", 1)[-1].strip()
        if not symbol:
            return f"computation {computation!r} has no trailing '::' symbol"
        if not _contains_token(surfaces, symbol):
            return (
                f"disposition 'computed' names {symbol!r}, but it occurs in no "
                "production Rust source (crates/*/src)"
            )
        return None
    if disposition == "downgraded":
        owner = record.get("owner")
        reason = record.get("reason")
        if (
            isinstance(owner, str)
            and owner.strip()
            and isinstance(reason, str)
            and reason.strip()
        ):
            return None
        return "disposition 'downgraded' carries no non-empty owner and reason"
    return f"disposition {disposition!r} is neither 'computed' nor 'downgraded'"


def _metric_gaps(root: Path, surfaces: Sequence[tuple[Path, str]]) -> tuple[list[str], list[DeclarationGap]]:
    declared = _hard_metric_names(root)
    records = _metric_dispositions(root)
    gaps: list[DeclarationGap] = []
    for name in declared:
        record = records.get(name)
        if record is None:
            gaps.append(
                DeclarationGap(
                    HARD_METRIC_WITHOUT_COMPUTATION,
                    name,
                    _METRIC_DECLARATION,
                    f"hard metric {name!r} is declared but has no "
                    "metric_dispositions record, so nothing computes or "
                    "downgrades it",
                )
            )
            continue
        cause = _metric_disposition_gap_reason(record, surfaces)
        if cause is not None:
            gaps.append(
                DeclarationGap(
                    HARD_METRIC_WITHOUT_COMPUTATION,
                    name,
                    _METRIC_DECLARATION,
                    f"hard metric {name!r} has a malformed metric disposition "
                    f"record: {cause}",
                )
            )
    return declared, gaps


def _distinct_invariants(root: Path) -> list[str]:
    stages = _load_yaml(root, _STAGE_DECLARATION).get("stages")
    if not isinstance(stages, list):
        raise ValueError(f"{_STAGE_DECLARATION} has no stages list")
    names: set[str] = set()
    for stage in stages:
        if not isinstance(stage, dict):
            continue
        for invariant in stage.get("hard_invariants") or []:
            names.add(str(invariant))
    return sorted(names)


def _invariant_dispositions(root: Path) -> dict:
    """The ``stage_invariant_dispositions.records`` mapping, required to exist.

    Refusing a missing or non-mapping block keeps the axis fail-closed: without
    it a fresh declaration file would silently read as zero gaps.
    """
    block = _load_yaml(root, _STAGE_DECLARATION).get("stage_invariant_dispositions")
    if not isinstance(block, dict):
        raise ValueError(
            f"{_STAGE_DECLARATION} has no stage_invariant_dispositions mapping; "
            "every declared hard invariant must carry a checked or downgraded disposition"
        )
    records = block.get("records")
    if not isinstance(records, dict):
        raise ValueError(
            f"{_STAGE_DECLARATION} stage_invariant_dispositions.records must be a mapping"
        )
    return records


def _disposition_gap_reason(record: object) -> str | None:
    """``None`` when ``record`` well-formedly covers its invariant, else the cause."""
    if not isinstance(record, dict):
        return f"disposition record is not a mapping, got {type(record).__name__}"
    disposition = record.get("disposition")
    if disposition == "checked":
        check_id = record.get("check_id")
        if isinstance(check_id, str) and check_id.strip():
            return None
        return "disposition 'checked' carries no non-empty check_id"
    if disposition == "downgraded":
        owner = record.get("owner")
        reason = record.get("reason")
        if (
            isinstance(owner, str)
            and owner.strip()
            and isinstance(reason, str)
            and reason.strip()
        ):
            return None
        return "disposition 'downgraded' carries no non-empty owner and reason"
    return f"disposition {disposition!r} is neither 'checked' nor 'downgraded'"


def _invariant_gaps(root: Path) -> tuple[list[str], list[DeclarationGap]]:
    declared = _distinct_invariants(root)
    records = _invariant_dispositions(root)
    gaps: list[DeclarationGap] = []
    for name in declared:
        record = records.get(name)
        if record is None:
            gaps.append(
                DeclarationGap(
                    STAGE_INVARIANT_WITHOUT_CHECK,
                    name,
                    _STAGE_DECLARATION,
                    f"stage hard invariant {name!r} is declared but has no "
                    "stage_invariant_dispositions record, so nothing checks or "
                    "downgrades it",
                )
            )
            continue
        cause = _disposition_gap_reason(record)
        if cause is not None:
            gaps.append(
                DeclarationGap(
                    STAGE_INVARIANT_WITHOUT_CHECK,
                    name,
                    _STAGE_DECLARATION,
                    f"stage hard invariant {name!r} has a malformed disposition "
                    f"record: {cause}",
                )
            )
    return declared, gaps


def _prompt_gaps(root: Path) -> tuple[list[str], list[DeclarationGap]]:
    registry = _load_yaml(root, _PROMPT_REGISTRY)
    tasks = registry.get("tasks") or []
    task_ids = {
        str(task.get("task_id"))
        for task in tasks
        if isinstance(task, dict) and task.get("task_id")
    }
    contract = registry.get("prompt_contract") or {}
    bindings = contract.get("task_rule_bindings") or {}
    if not isinstance(bindings, dict):
        raise ValueError(f"{_PROMPT_REGISTRY} task_rule_bindings must be a mapping")
    declared = sorted(str(key) for key in bindings)
    gaps = [
        DeclarationGap(
            PROMPT_FORM_WITHOUT_REGISTRY_ENTRY,
            name,
            _PROMPT_REGISTRY,
            f"prompt call form {name!r} is bound in task_rule_bindings but has no "
            "tasks[] entry, so the registry cannot resolve it",
        )
        for name in declared
        if name not in task_ids
    ]
    return declared, gaps


# ── the axis ─────────────────────────────────────────────────────────────────


@dataclass(frozen=True)
class DeclarationSyncGap:
    """The declaration synchronisation axis: declared minus enforced, as a count."""

    root: str
    gaps: tuple[DeclarationGap, ...]
    declared_totals: dict[str, int]

    @property
    def count(self) -> int:
        return len(self.gaps)

    def gaps_for(self, gap_class: str) -> tuple[DeclarationGap, ...]:
        return tuple(g for g in self.gaps if g.gap_class == gap_class)

    def declared_for(self, gap_class: str) -> int:
        return self.declared_totals.get(gap_class, 0)

    def to_dict(self) -> dict:
        return {
            "axis": "declaration_sync_gap",
            "definition": (
                "number of declarations the DATA makes that the code does not enforce: "
                "a hard metric with no computed-or-downgraded disposition, a stage "
                "hard invariant with no disposition, and a prompt call form with no "
                "resolvable registry entry. The goal is zero."
            ),
            "root": self.root,
            "gap_count": self.count,
            "declared_totals": dict(self.declared_totals),
            "gaps_by_class": {
                gap_class: {
                    "declared": self.declared_for(gap_class),
                    "gaps": len(self.gaps_for(gap_class)),
                    "names": [g.name for g in self.gaps_for(gap_class)],
                }
                for gap_class in _GAP_CLASS_ORDER
            },
            "gaps": [g.to_dict() for g in self.gaps],
        }

    def render_lines(self) -> list[str]:
        """The axis as the lines a session reads first."""
        lines = [
            f"declaration synchronisation: {self.count} gaps "
            f"(goal 0; a regression is an increase)"
        ]
        labels = {
            HARD_METRIC_WITHOUT_COMPUTATION: "hard metrics without a production computation",
            STAGE_INVARIANT_WITHOUT_CHECK: "stage invariants without a disposition",
            PROMPT_FORM_WITHOUT_REGISTRY_ENTRY: "prompt call forms without a registry entry",
        }
        for gap_class in _GAP_CLASS_ORDER:
            gaps = self.gaps_for(gap_class)
            lines.append(
                f"  {labels[gap_class]}: {len(gaps)}/{self.declared_for(gap_class)} unmapped"
            )
        for gap_class in _GAP_CLASS_ORDER:
            names = [g.name for g in self.gaps_for(gap_class)]
            if names:
                lines.append(f"    {gap_class}: {', '.join(names)}")
        return lines


def build_declaration_sync_gap(root: Path) -> DeclarationSyncGap:
    """Compute the axis for ``root`` against the three declaration surfaces.

    Raises ``FileNotFoundError`` when ``root`` has no ``crates/`` directory or a
    required declaration file is missing: an empty scan would otherwise report a
    vacuous zero-gap green and hide a bad root.
    """
    root = Path(root)
    if not (root / "crates").is_dir():
        raise FileNotFoundError(
            f"{root} has no crates/ directory; refusing to report a vacuous zero-gap axis"
        )
    metric_surfaces = _read_surfaces(_surface_files(root, _METRIC_COMPUTATION_SURFACES))

    declared_metrics, metric_gaps = _metric_gaps(root, metric_surfaces)
    declared_invariants, invariant_gaps = _invariant_gaps(root)
    declared_prompts, prompt_gaps = _prompt_gaps(root)

    gaps = tuple(metric_gaps + invariant_gaps + prompt_gaps)
    declared_totals = {
        HARD_METRIC_WITHOUT_COMPUTATION: len(declared_metrics),
        STAGE_INVARIANT_WITHOUT_CHECK: len(declared_invariants),
        PROMPT_FORM_WITHOUT_REGISTRY_ENTRY: len(declared_prompts),
    }
    return DeclarationSyncGap(root=str(root), gaps=gaps, declared_totals=declared_totals)


def observe_declaration_sync_gap(root: Path | None = None) -> DeclarationSyncGap:
    """Observe the axis on the real tree (or a given root)."""
    if root is None:
        root = Path(__file__).resolve().parent.parent.parent.parent
    return build_declaration_sync_gap(Path(root))
