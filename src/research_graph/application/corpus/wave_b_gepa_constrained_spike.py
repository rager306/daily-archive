"""Wave B GEPA-shaped constrained spike (offline, fail-closed).

Implements a duck-typed GEPA adapter surface over the constrained pilot:

- multi-component text candidate:
    entity_select_instruction / relation_link_instruction
- evaluate → entity/relation F1 via evaluate_records (never invents free labels)
- make_reflective_dataset → ASI records (coverage gaps, type misses, drops)
- offline_reflective_spike → local reflective mutation without importing gepa
- optional try_gepa_optimize when the ``gepa`` package is installed

Never DSPy. Never import_eligible. Never graph writes.
GEPA optimizes **instruction text**, not model weights and not candidate coverage.
If gold labels are missing from candidates, reflection can only report candidate_gap.
"""

from __future__ import annotations

import hashlib
import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

from research_graph.application.corpus.wave_b_gold_hybrid_constrained_pilot import (
    build_body_candidates,
    build_constrained_prediction_record,
    score_gold_hybrid_constrained_pilot,
    surface_in_body,
)
from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    ALLOWED_ENTITY_TYPES,
    ALLOWED_RELATION_TYPES,
    truncate_body_for_pilot,
)
from research_graph.application.extraction_benchmark import evaluate_records

SCHEMA_VERSION = "wave-b-gepa-constrained-spike.v1"

COMPONENT_ENTITY = "entity_select_instruction"
COMPONENT_RELATION = "relation_link_instruction"
DEFAULT_CANDIDATE: dict[str, str] = {
    COMPONENT_ENTITY: (
        "Select grounded multiword candidates from the list. "
        "Assign closed types: Field, Task, Method, Dataset, Model, Metric.\n"
        "SELECT_MAX: 4\n"
        "# TYPE_HINT: <surface> -> <Type>\n"
    ),
    COMPONENT_RELATION: (
        "Link selected entities with closed relations: "
        "APPLIED_TO, EVALUATED_ON, USES, PART_OF, OUTPERFORMS.\n"
        "# RELATION_HINT: <TypeA> <REL> <TypeB>\n"
    ),
}

_TYPE_HINT_RE = re.compile(
    r"TYPE_HINT\s*:\s*(.+?)\s*->\s*([A-Za-z_]+)",
    re.IGNORECASE,
)
_REL_HINT_RE = re.compile(
    r"RELATION_HINT\s*:\s*([A-Za-z_]+)\s+([A-Za-z_]+)\s+([A-Za-z_]+)",
    re.IGNORECASE,
)
_SELECT_MAX_RE = re.compile(r"SELECT_MAX\s*:\s*(\d+)", re.IGNORECASE)


def _normalize(value: str) -> str:
    return " ".join(value.casefold().strip().split())


def parse_entity_type_hints(instruction: str) -> dict[str, str]:
    """Parse TYPE_HINT lines from entity instruction text."""
    out: dict[str, str] = {}
    for match in _TYPE_HINT_RE.finditer(instruction or ""):
        surface = match.group(1).strip()
        etype = match.group(2).strip()
        if etype.islower():
            etype = etype[:1].upper() + etype[1:]
        if etype in ALLOWED_ENTITY_TYPES and surface:
            out[_normalize(surface)] = etype
    return out


def parse_relation_hints(instruction: str) -> list[tuple[str, str, str]]:
    """Parse RELATION_HINT lines: TypeA REL TypeB."""
    out: list[tuple[str, str, str]] = []
    for match in _REL_HINT_RE.finditer(instruction or ""):
        src_t = match.group(1).strip()
        rel = match.group(2).strip().upper()
        tgt_t = match.group(3).strip()
        if src_t.islower():
            src_t = src_t[:1].upper() + src_t[1:]
        if tgt_t.islower():
            tgt_t = tgt_t[:1].upper() + tgt_t[1:]
        if (
            src_t in ALLOWED_ENTITY_TYPES
            and tgt_t in ALLOWED_ENTITY_TYPES
            and rel in ALLOWED_RELATION_TYPES
        ):
            out.append((src_t, rel, tgt_t))
    return out


def parse_select_max(instruction: str, *, default: int = 4) -> int:
    match = _SELECT_MAX_RE.search(instruction or "")
    if not match:
        return default
    try:
        value = int(match.group(1))
    except ValueError:
        return default
    return max(1, min(value, 16))


def instruction_rule_select(
    body_text: str,
    case_id: str,
    candidates: Sequence[Mapping[str, Any]],
    *,
    entity_instruction: str,
    relation_instruction: str,
) -> dict[str, Any]:
    """Deterministic selector driven by structured instruction hints.

    Used as GEPA task program stand-in (no LLM). Hints must name surfaces that
    already exist in the candidate set; ungrounded / missing surfaces are dropped.
    """
    del case_id
    type_hints = parse_entity_type_hints(entity_instruction)
    select_max = parse_select_max(entity_instruction)
    by_norm = {
        str(c.get("surface_norm") or _normalize(str(c.get("surface") or ""))): c
        for c in candidates
        if isinstance(c, Mapping) and c.get("surface")
    }

    entities: list[dict[str, Any]] = []
    # Prefer explicit TYPE_HINT matches (order of instruction appearance via dict).
    for norm, etype in type_hints.items():
        cand = by_norm.get(norm)
        if cand is None:
            continue
        surface = str(cand.get("surface") or "")
        if not surface_in_body(surface, body_text):
            continue
        entities.append(
            {"candidate_id": str(cand.get("candidate_id")), "type": etype}
        )
        if len(entities) >= select_max:
            break

    # If no hints matched, keep empty entities (honest weak seed) — GEPA must learn.
    selected_types = {
        str(e.get("type")): str(e.get("candidate_id")) for e in entities
    }
    # Also map candidate_id -> type for relation building
    cid_to_type = {str(e.get("candidate_id")): str(e.get("type")) for e in entities}

    relations: list[dict[str, Any]] = []
    for src_t, rel, tgt_t in parse_relation_hints(relation_instruction):
        # find one selected entity of each type
        src_ids = [cid for cid, t in cid_to_type.items() if t == src_t]
        tgt_ids = [cid for cid, t in cid_to_type.items() if t == tgt_t]
        if not src_ids or not tgt_ids:
            continue
        src_id, tgt_id = src_ids[0], tgt_ids[0]
        if src_id == tgt_id:
            continue
        relations.append(
            {"type": rel, "source_id": src_id, "target_id": tgt_id}
        )

    del selected_types
    return {"entities": entities, "relations": relations, "json_valid": True}


def make_select_fn_from_candidate(
    candidate: Mapping[str, str],
) -> Callable[[str, str, Sequence[Mapping[str, Any]]], Mapping[str, Any]]:
    entity_i = str(candidate.get(COMPONENT_ENTITY) or "")
    relation_i = str(candidate.get(COMPONENT_RELATION) or "")

    def _select(
        body_text: str,
        case_id: str,
        candidates: Sequence[Mapping[str, Any]],
    ) -> Mapping[str, Any]:
        return instruction_rule_select(
            body_text,
            case_id,
            candidates,
            entity_instruction=entity_i,
            relation_instruction=relation_i,
        )

    return _select


@dataclass(frozen=True, slots=True)
class EvaluationBatch:
    """Duck-type of gepa.core.adapter.EvaluationBatch."""

    outputs: list[Any]
    scores: list[float]
    trajectories: list[dict[str, Any]] | None = None
    objective_scores: list[dict[str, float]] | None = None


@dataclass(frozen=True, slots=True)
class WaveBGEPASpikePackage:
    schema_version: str
    mode: str
    case_count: int
    train_count: int
    val_count: int
    seed_metrics: dict[str, Any]
    best_metrics: dict[str, Any]
    floor_metrics: dict[str, Any] | None
    seed_candidate: dict[str, str]
    best_candidate: dict[str, str]
    iterations: tuple[dict[str, Any], ...]
    reflective_samples: tuple[dict[str, Any], ...]
    diagnostics: tuple[str, ...]
    gepa_package_available: bool = False
    gepa_ran: bool = False
    llm_used: bool = False
    dspy_optimizer_enabled: bool = False
    import_eligible: bool = False
    graph_writes_allowed: bool = False
    oracle_ceiling_metrics: dict[str, Any] | None = None
    coverage_summary: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("gepa spike cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("gepa spike cannot enable DSPy")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "wave": "B",
            "mode": self.mode,
            "case_count": self.case_count,
            "train_count": self.train_count,
            "val_count": self.val_count,
            "seed_metrics": dict(self.seed_metrics),
            "best_metrics": dict(self.best_metrics),
            "floor_metrics": dict(self.floor_metrics) if self.floor_metrics else None,
            "oracle_ceiling_metrics": (
                dict(self.oracle_ceiling_metrics) if self.oracle_ceiling_metrics else None
            ),
            "seed_candidate": dict(self.seed_candidate),
            "best_candidate": dict(self.best_candidate),
            "iterations": list(self.iterations),
            "reflective_samples": list(self.reflective_samples),
            "coverage_summary": dict(self.coverage_summary),
            "diagnostics": list(self.diagnostics),
            "gepa_package_available": self.gepa_package_available,
            "gepa_ran": self.gepa_ran,
            "llm_used": self.llm_used,
            "dspy_optimizer_enabled": False,
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Offline GEPA-shaped reflective spike over constrained candidates. "
                "Not production path. Not DSPy. Not import. "
                "Raises quality only when gold surfaces exist in candidates."
            ),
        }


class WaveBConstrainedGEPAAdapter:
    """GEPAAdapter-compatible surface for constrained gold-hybrid pilot.

    Does not import gepa. Callers may pass instances to gepa.optimize if installed.
    """

    def __init__(
        self,
        cases: Sequence[Mapping[str, Any]],
        *,
        max_body_chars: int = 8000,
        score_key: str = "entity_f1",
    ) -> None:
        self.cases = [dict(c) for c in cases]
        self.max_body_chars = int(max_body_chars)
        self.score_key = score_key
        # Precompute candidates per case for ASI / reflection
        self._prepared: list[dict[str, Any]] = []
        for case in self.cases:
            gold = dict(case.get("gold") or {})
            body = str(case.get("body_text") or "")
            case_id = str(case.get("case_id") or gold.get("case_id") or "unknown")
            paper_id = str(case.get("paper_id") or gold.get("paper_id") or "")
            window = truncate_body_for_pilot(body, max_chars=self.max_body_chars)
            candidates = build_body_candidates(window, paper_id=paper_id)
            gold_norms = {
                _normalize(str(e.get("label") or ""))
                for e in (gold.get("entities") or [])
                if isinstance(e, Mapping) and e.get("label")
            }
            cand_norms = {
                str(c.get("surface_norm") or "")
                for c in candidates
                if isinstance(c, Mapping)
            }
            self._prepared.append(
                {
                    "case": case,
                    "gold": gold,
                    "case_id": case_id,
                    "paper_id": paper_id,
                    "window": window,
                    "candidates": candidates,
                    "gold_norms": gold_norms,
                    "cand_norms": cand_norms,
                    "coverage": len(gold_norms & cand_norms),
                    "missing_gold": sorted(gold_norms - cand_norms),
                }
            )

    def coverage_summary(self) -> dict[str, Any]:
        total_gold = sum(len(p["gold_norms"]) for p in self._prepared)
        covered = sum(p["coverage"] for p in self._prepared)
        return {
            "case_count": len(self._prepared),
            "gold_entity_total": total_gold,
            "gold_labels_in_candidates": covered,
            "coverage_ratio": (covered / total_gold) if total_gold else 0.0,
            "per_case": [
                {
                    "case_id": p["case_id"],
                    "coverage": p["coverage"],
                    "gold_entity_count": len(p["gold_norms"]),
                    "missing_gold": list(p["missing_gold"]),
                    "candidate_count": len(p["candidates"]),
                }
                for p in self._prepared
            ],
        }

    def evaluate(
        self,
        batch: Sequence[Mapping[str, Any]] | None,
        candidate: Mapping[str, str],
        capture_traces: bool = False,
    ) -> EvaluationBatch:
        """Score candidate instructions on a batch of cases (or full set if batch is None)."""
        prepared = self._select_prepared(batch)
        select_fn = make_select_fn_from_candidate(candidate)
        scores: list[float] = []
        outputs: list[Any] = []
        trajectories: list[dict[str, Any]] = []
        objective_scores: list[dict[str, float]] = []

        for prep in prepared:
            gold = dict(prep["gold"])
            case_id = prep["case_id"]
            paper_id = prep["paper_id"]
            window = prep["window"]
            candidates = prep["candidates"]
            gold.setdefault("case_id", case_id)
            gold.setdefault("paper_id", paper_id)
            gold.setdefault("source_artifact_refs", ["artifact:catalog-unknown"])
            gold.setdefault("schema_valid", True)
            gold.setdefault("json_valid", True)
            gold.setdefault(
                "operational",
                {"cost_estimate": 0.0, "latency_ms": 0, "retry_count": 0},
            )
            gold.setdefault("entities", [])
            gold.setdefault("relations", [])

            try:
                selection = dict(select_fn(window, case_id, candidates))
                json_valid = bool(selection.get("json_valid", True))
            except Exception as exc:  # noqa: BLE001 — GEPA contract: score 0, don't raise
                selection = {"entities": [], "relations": [], "json_valid": False}
                json_valid = False
                err = type(exc).__name__
            else:
                err = ""

            pred = build_constrained_prediction_record(
                case_id=case_id,
                paper_id=paper_id,
                body_text=window,
                candidates=candidates,
                selection=selection,
                source_artifact_refs=gold.get("source_artifact_refs"),
            )
            metrics = evaluate_records([gold], [pred])
            entity_f1 = float(metrics.get("entity_f1") or 0.0)
            relation_f1 = float(metrics.get("relation_f1") or 0.0)
            # GEPA per-example score: primary metric (entity_f1 by default)
            score = entity_f1 if self.score_key == "entity_f1" else relation_f1
            if not json_valid:
                score = 0.0
            scores.append(score)
            outputs.append(
                {
                    "case_id": case_id,
                    "prediction": pred,
                    "metrics": metrics,
                    "json_valid": json_valid,
                }
            )
            objective_scores.append(
                {
                    "entity_f1": entity_f1,
                    "relation_f1": relation_f1,
                    "entity_recall": float(metrics.get("entity_recall") or 0.0),
                    "entity_precision": float(metrics.get("entity_precision") or 0.0),
                }
            )
            if capture_traces:
                pred_labels = {
                    _normalize(str(e.get("label") or ""))
                    for e in (pred.get("entities") or [])
                    if isinstance(e, Mapping)
                }
                gold_by_norm = {
                    _normalize(str(e.get("label") or "")): e
                    for e in (gold.get("entities") or [])
                    if isinstance(e, Mapping) and e.get("label")
                }
                gold_labels = set(gold_by_norm)
                gold_entities_asi = [
                    {
                        "label": str(g.get("label") or ""),
                        "type": str(g.get("type") or ""),
                        "in_candidates": _normalize(str(g.get("label") or ""))
                        in prep["cand_norms"],
                    }
                    for g in gold_by_norm.values()
                ]
                type_mismatches: list[dict[str, str]] = []
                for e in pred.get("entities") or []:
                    if not isinstance(e, Mapping):
                        continue
                    g = gold_by_norm.get(_normalize(str(e.get("label") or "")))
                    if g is None:
                        continue
                    if str(e.get("type")) != str(g.get("type")):
                        type_mismatches.append(
                            {
                                "label": str(e.get("label")),
                                "pred_type": str(e.get("type")),
                                "gold_type": str(g.get("type")),
                            }
                        )
                trajectories.append(
                    {
                        "case_id": case_id,
                        "entity_instruction": str(
                            candidate.get(COMPONENT_ENTITY) or ""
                        ),
                        "relation_instruction": str(
                            candidate.get(COMPONENT_RELATION) or ""
                        ),
                        "candidate_count": len(candidates),
                        "gold_label_coverage": prep["coverage"],
                        "missing_gold_in_candidates": list(prep["missing_gold"]),
                        "pred_labels": sorted(pred_labels),
                        "gold_labels": sorted(gold_labels),
                        "gold_entities": gold_entities_asi,
                        "missed_but_in_candidates": sorted(
                            (gold_labels - pred_labels) & prep["cand_norms"]
                        ),
                        "false_positives": sorted(pred_labels - gold_labels),
                        "type_mismatches": type_mismatches,
                        "entity_f1": entity_f1,
                        "relation_f1": relation_f1,
                        "json_valid": json_valid,
                        "error": err,
                        "selection": selection,
                    }
                )

        return EvaluationBatch(
            outputs=outputs,
            scores=scores,
            trajectories=trajectories if capture_traces else None,
            objective_scores=objective_scores,
        )

    def make_reflective_dataset(
        self,
        candidate: Mapping[str, str],
        eval_batch: EvaluationBatch,
        components_to_update: Sequence[str],
    ) -> dict[str, list[dict[str, Any]]]:
        """Build ASI-style reflective records per component (GEPA contract)."""
        del candidate
        components = list(components_to_update) or [
            COMPONENT_ENTITY,
            COMPONENT_RELATION,
        ]
        trajectories = eval_batch.trajectories or []
        dataset: dict[str, list[dict[str, Any]]] = {c: [] for c in components}

        for traj in trajectories:
            if not isinstance(traj, Mapping):
                continue
            feedback_bits: list[str] = []
            missing = list(traj.get("missing_gold_in_candidates") or [])
            if missing:
                feedback_bits.append(
                    "candidate_gap: gold labels not in candidate set: "
                    + ", ".join(missing)
                )
            missed = list(traj.get("missed_but_in_candidates") or [])
            if missed:
                feedback_bits.append(
                    "missed_available: gold labels present in candidates but not selected: "
                    + ", ".join(missed)
                    + " — add TYPE_HINT lines"
                )
            for mm in traj.get("type_mismatches") or []:
                if isinstance(mm, Mapping):
                    feedback_bits.append(
                        f"type_mismatch: pred={mm.get('pred_type')} "
                        f"gold={mm.get('gold_type')} for '{mm.get('label')}'"
                    )
            fps = list(traj.get("false_positives") or [])
            if fps:
                feedback_bits.append(
                    "false_positives: " + ", ".join(fps) + " — remove wrong TYPE_HINT"
                )
            if not traj.get("json_valid", True):
                feedback_bits.append(
                    f"json_valid=false error={traj.get('error') or 'unknown'}"
                )
            if float(traj.get("entity_f1") or 0.0) >= 0.99:
                feedback_bits.append("ok: entity extraction matches gold for this case")

            feedback = "; ".join(feedback_bits) if feedback_bits else "no issues"

            record_base = {
                "Inputs": {
                    "case_id": traj.get("case_id"),
                    "gold_labels": traj.get("gold_labels"),
                    "gold_entities": traj.get("gold_entities"),
                    "candidate_count": traj.get("candidate_count"),
                    "gold_label_coverage": traj.get("gold_label_coverage"),
                },
                "Generated Outputs": {
                    "pred_labels": traj.get("pred_labels"),
                    "entity_f1": traj.get("entity_f1"),
                    "relation_f1": traj.get("relation_f1"),
                    "selection": traj.get("selection"),
                },
                "Feedback": feedback,
            }
            if COMPONENT_ENTITY in dataset:
                dataset[COMPONENT_ENTITY].append(
                    {
                        **record_base,
                        "Component": COMPONENT_ENTITY,
                        "Current Instruction": traj.get("entity_instruction"),
                    }
                )
            if COMPONENT_RELATION in dataset:
                # relation-focused feedback
                rel_bits = [
                    b
                    for b in feedback_bits
                    if "relation" in b.casefold() or "type_mismatch" in b.casefold()
                ]
                if float(traj.get("relation_f1") or 0.0) < 0.99:
                    rel_bits.append(
                        "relation_gap: ensure RELATION_HINT matches selected entity types "
                        "(e.g. Field APPLIED_TO Task)"
                    )
                dataset[COMPONENT_RELATION].append(
                    {
                        **record_base,
                        "Component": COMPONENT_RELATION,
                        "Current Instruction": traj.get("relation_instruction"),
                        "Feedback": (
                            "; ".join(rel_bits) if rel_bits else feedback
                        ),
                    }
                )
        return dataset

    def _select_prepared(
        self, batch: Sequence[Mapping[str, Any]] | None
    ) -> list[dict[str, Any]]:
        if batch is None:
            return list(self._prepared)
        wanted = {
            str(b.get("case_id") or (b.get("gold") or {}).get("case_id") or "")
            for b in batch
            if isinstance(b, Mapping)
        }
        if not wanted or wanted == {""}:
            return list(self._prepared)
        return [p for p in self._prepared if p["case_id"] in wanted]



def stable_train_val_split(
    cases: Sequence[Mapping[str, Any]],
    *,
    train_ratio: float = 0.67,
    seed: int = 0,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Deterministic train/val split by case_id hash (not first-N order).

    Avoids train-prefix overfitting when fixtures are ordered by id.
    """
    cases_list = [dict(c) for c in cases]
    n = len(cases_list)
    if n <= 1:
        return cases_list, list(cases_list)
    # stable sort then hash-bucket for train preference
    ordered = sorted(
        cases_list,
        key=lambda c: str(c.get("case_id") or c.get("paper_id") or ""),
    )
    n_train = max(1, min(n - 1, int(round(n * float(train_ratio)))))
    scored: list[tuple[int, dict[str, Any]]] = []
    for c in ordered:
        key = f"{seed}:{c.get('case_id') or c.get('paper_id') or id(c)}"
        h = int(hashlib.md5(key.encode("utf-8")).hexdigest()[:8], 16)
        scored.append((h, c))
    scored.sort(key=lambda x: (x[0], str(x[1].get("case_id") or "")))
    train = [c for _, c in scored[:n_train]]
    val = [c for _, c in scored[n_train:]]
    if not val:
        val = [train[-1]]
    return train, val


def propose_instruction_from_reflection(
    current: Mapping[str, str],
    reflective: Mapping[str, Sequence[Mapping[str, Any]]],
    *,
    min_support: int = 1,
    max_type_hints: int = 12,
    max_new_hints: int = 3,
) -> dict[str, str]:
    """Deterministic offline reflection (stand-in for reflection LM).

    Mines Feedback for missed_available / type_mismatch / relation_gap and
    appends structured TYPE_HINT / RELATION_HINT lines. Does not invent surfaces
    that never appear as missed_available (respects candidate coverage).

    min_support: require a surface/type pair to appear in at least this many
    reflective rows before adding a new TYPE_HINT (reduces paper-id overfit).
    max_type_hints: hard cap on TYPE_HINT lines in the entity instruction.
    """
    entity_lines = [str(current.get(COMPONENT_ENTITY) or "").rstrip()]
    relation_lines = [str(current.get(COMPONENT_RELATION) or "").rstrip()]
    existing_entity = parse_entity_type_hints("\n".join(entity_lines))
    existing_rel = set(parse_relation_hints("\n".join(relation_lines)))

    # Aggregate surface->type votes across reflective rows (anti-overfit).
    # Count each (surface, type) at most once per reflective row so a single
    # paper cannot satisfy min_support via dual gold+feedback channels.
    votes: dict[tuple[str, str], int] = {}
    surface_display: dict[str, str] = {}
    for rec in reflective.get(COMPONENT_ENTITY) or []:
        if not isinstance(rec, Mapping):
            continue
        feedback = str(rec.get("Feedback") or "")
        inputs_raw = rec.get("Inputs")
        inputs: Mapping[str, Any] = (
            inputs_raw if isinstance(inputs_raw, Mapping) else {}
        )
        row_votes: set[tuple[str, str]] = set()
        gold_type_by_norm: dict[str, str] = {}
        for ge in inputs.get("gold_entities") or []:
            if isinstance(ge, Mapping) and ge.get("label") and ge.get("type"):
                lab = _normalize(str(ge.get("label")))
                etype = str(ge.get("type"))
                gold_type_by_norm[lab] = etype
                if ge.get("in_candidates") and etype in ALLOWED_ENTITY_TYPES:
                    row_votes.add((lab, etype))
                    surface_display.setdefault(lab, str(ge.get("label") or lab))
        if "missed_available:" in feedback:
            after = feedback.split("missed_available:", 1)[1]
            labels_part = after.split("—")[0].split(";")[0]
            labels = [
                _normalize(x)
                for x in labels_part.replace(
                    "gold labels present in candidates but not selected:", ""
                ).split(",")
                if _normalize(x)
            ]
            for lab in labels:
                etype = gold_type_by_norm.get(lab, "Method")
                if etype not in ALLOWED_ENTITY_TYPES:
                    etype = "Method"
                row_votes.add((lab, etype))
                surface_display.setdefault(lab, lab)
        for piece in feedback.split(";"):
            piece = piece.strip()
            if piece.startswith("type_mismatch:"):
                m = re.search(
                    r"gold=([A-Za-z_]+)\s+for\s+'([^']+)'",
                    piece,
                )
                if m:
                    etype, label = m.group(1), m.group(2)
                    if etype in ALLOWED_ENTITY_TYPES:
                        lab = _normalize(label)
                        row_votes.add((lab, etype))
                        surface_display.setdefault(lab, label)
        for key in row_votes:
            votes[key] = votes.get(key, 0) + 1

    # Keep existing hints first; add only high-support new ones up to cap.
    # Incremental: at most max_new_hints per proposal (val-aware needs gradual steps).
    current_hint_count = len(existing_entity)
    added_new = 0
    ranked = sorted(votes.items(), key=lambda kv: (-kv[1], kv[0][0], kv[0][1]))
    for (lab, etype), support in ranked:
        if current_hint_count >= max(1, int(max_type_hints)):
            break
        if added_new >= max(0, int(max_new_hints)):
            break
        if support < max(1, int(min_support)):
            continue
        if lab in existing_entity and existing_entity[lab] == etype:
            continue
        surface = surface_display.get(lab, lab)
        entity_lines.append(f"TYPE_HINT: {surface} -> {etype}")
        existing_entity[lab] = etype
        current_hint_count += 1
        added_new += 1

    for rec in reflective.get(COMPONENT_RELATION) or []:
        if not isinstance(rec, Mapping):
            continue
        feedback = str(rec.get("Feedback") or "")
        if "relation_gap" in feedback.casefold():
            gen_raw = rec.get("Generated Outputs")
            gen: Mapping[str, Any] = gen_raw if isinstance(gen_raw, Mapping) else {}
            inputs_raw = rec.get("Inputs")
            inputs: Mapping[str, Any] = (
                inputs_raw if isinstance(inputs_raw, Mapping) else {}
            )
            sel_raw = gen.get("selection")
            sel = sel_raw if isinstance(sel_raw, Mapping) else None
            types: list[str] = []
            if isinstance(sel, Mapping):
                for e in sel.get("entities") or []:
                    if isinstance(e, Mapping) and e.get("type"):
                        types.append(str(e.get("type")))
            if not types:
                for ge in inputs.get("gold_entities") or []:
                    if not isinstance(ge, Mapping):
                        continue
                    if not ge.get("in_candidates"):
                        continue
                    et = str(ge.get("type") or "")
                    if et in ALLOWED_ENTITY_TYPES:
                        types.append(et)
            if "Field" in types and "Task" in types:
                hint = ("Field", "APPLIED_TO", "Task")
                if hint not in existing_rel:
                    relation_lines.append("RELATION_HINT: Field APPLIED_TO Task")
                    existing_rel.add(hint)
            elif len(types) >= 2:
                hint = (types[0], "APPLIED_TO", types[1])
                if hint[0] in ALLOWED_ENTITY_TYPES and hint[2] in ALLOWED_ENTITY_TYPES:
                    if hint not in existing_rel:
                        relation_lines.append(
                            f"RELATION_HINT: {hint[0]} APPLIED_TO {hint[2]}"
                        )
                        existing_rel.add(hint)

    return {
        COMPONENT_ENTITY: "\n".join(entity_lines) + "\n",
        COMPONENT_RELATION: "\n".join(relation_lines) + "\n",
    }


def offline_reflective_spike(
    *,
    cases: Sequence[Mapping[str, Any]],
    seed_candidate: Mapping[str, str] | None = None,
    max_iterations: int = 4,
    max_body_chars: int = 8000,
    floor_metrics: Mapping[str, Any] | None = None,
    train_ratio: float = 0.67,
    acceptance: str = "val_aware",
    min_support: int = 1,
    max_type_hints: int = 12,
    max_new_hints: int = 3,
    max_val_gap: float = 0.35,
    split_seed: int = 0,
    train_blend: float = 0.2,
) -> WaveBGEPASpikePackage:
    """Run a local reflective mutation loop (no gepa package, no LLM).

    Demonstrates GEPA method shape: evaluate → ASI → mutate instructions → accept if improved.

    acceptance:
      - "train": accept when train score sum improves (legacy; overfits)
      - "val_aware": accept when val score does not degrade and train improves,
        or val strictly improves; best_candidate selected by val then train
    min_support / max_type_hints: anti-overfit controls for TYPE_HINT mining.
    """
    cases_list = [dict(c) for c in cases]
    n = len(cases_list)
    train, val = stable_train_val_split(
        cases_list, train_ratio=train_ratio, seed=split_seed
    )
    # single-case unit tests: train==val is fine
    if n == 1:
        train = cases_list
        val = cases_list

    # For tiny multi-case suites, min_support=2 can starve learning; allow 1 when n small
    effective_min_support = int(min_support)
    if n < 3:
        effective_min_support = 1

    adapter = WaveBConstrainedGEPAAdapter(train, max_body_chars=max_body_chars)
    val_adapter = WaveBConstrainedGEPAAdapter(val, max_body_chars=max_body_chars)
    seed = dict(seed_candidate or DEFAULT_CANDIDATE)
    coverage = adapter.coverage_summary()
    val_cov = val_adapter.coverage_summary()

    seed_eval = val_adapter.evaluate(None, seed, capture_traces=True)
    seed_metrics = _aggregate_objective(seed_eval)

    oracle_pkg = score_gold_hybrid_constrained_pilot(
        cases=cases_list,
        use_lexical_oracle=True,
        floor_metrics=floor_metrics,
        max_body_chars=max_body_chars,
        llm_used=False,
    )

    best = dict(seed)
    seed_train_eval = adapter.evaluate(None, seed, capture_traces=False)
    seed_val_sum = sum(seed_eval.scores)
    best_train_sum = sum(seed_train_eval.scores)
    best_val_sum = seed_val_sum
    best_metrics = dict(seed_metrics)
    best_train_metrics = _aggregate_objective(seed_train_eval)
    iterations: list[dict[str, Any]] = []
    reflective_samples: list[dict[str, Any]] = []

    current = dict(seed)
    current_val_sum = seed_val_sum
    for i in range(max(1, max_iterations)):
        train_eval = adapter.evaluate(None, current, capture_traces=True)
        reflective = adapter.make_reflective_dataset(
            current,
            train_eval,
            [COMPONENT_ENTITY, COMPONENT_RELATION],
        )
        for comp, rows in reflective.items():
            for row in rows[:2]:
                reflective_samples.append(
                    {
                        "iteration": i,
                        "component": comp,
                        "feedback": row.get("Feedback"),
                        "case_id": (row.get("Inputs") or {}).get("case_id"),
                    }
                )
        proposed = propose_instruction_from_reflection(
            current,
            reflective,
            min_support=effective_min_support,
            max_type_hints=max_type_hints,
            max_new_hints=max_new_hints,
        )
        before_sum = sum(train_eval.scores)
        after_eval = adapter.evaluate(None, proposed, capture_traces=False)
        after_sum = sum(after_eval.scores)
        val_after = val_adapter.evaluate(None, proposed, capture_traces=True)
        val_after_sum = sum(val_after.scores)
        val_metrics = _aggregate_objective(val_after)
        train_metrics = _aggregate_objective(after_eval)

        mode = (acceptance or "val_aware").strip().lower()
        blend = max(0.0, min(1.0, float(train_blend)))
        # composite: prioritize val, blend some train signal (current_val_sum tracked)
        before_composite = current_val_sum + blend * before_sum
        after_composite = val_after_sum + blend * after_sum
        if mode == "train":
            accepted = after_sum > before_sum
        else:
            # val_aware: accept if composite improves and val is not strictly worse
            train_improved = after_sum > before_sum
            val_improved = val_after_sum > current_val_sum + 1e-9
            val_not_worse = val_after_sum + 1e-9 >= current_val_sum
            composite_improved = after_composite > before_composite + 1e-9
            accepted = (composite_improved and val_not_worse) or val_improved

        te = float(train_metrics.get("entity_f1") or 0.0)
        ve = float(val_metrics.get("entity_f1") or 0.0)
        gap = te - ve
        gap_reject = False
        if mode != "train" and n >= 3 and gap > float(max_val_gap):
            # reject when gap exceeds budget unless val strictly improved
            if accepted and not (val_after_sum > current_val_sum + 1e-9):
                accepted = False
                gap_reject = True

        iterations.append(
            {
                "iteration": i,
                "accepted": accepted,
                "acceptance_mode": mode,
                "train_score_sum_before": before_sum,
                "train_score_sum_after": after_sum,
                "val_score_sum_before": current_val_sum,
                "val_score_sum_after": val_after_sum,
                "train_entity_f1": train_metrics.get("entity_f1"),
                "val_entity_f1": val_metrics.get("entity_f1"),
                "val_relation_f1": val_metrics.get("relation_f1"),
                "val_gap": round(gap, 6),
                "gap_reject": gap_reject,
                "type_hint_count": str(proposed.get(COMPONENT_ENTITY) or "").count(
                    "TYPE_HINT:"
                ),
            }
        )
        if accepted:
            current = proposed
            current_val_sum = val_after_sum
            # Select best by val first, then train (D128 val-aware)
            better_val = val_after_sum > best_val_sum + 1e-9
            equal_val_better_train = (
                abs(val_after_sum - best_val_sum) <= 1e-9 and after_sum >= best_train_sum
            )
            if better_val or equal_val_better_train or mode == "train":
                if mode == "train":
                    if after_sum >= best_train_sum:
                        best = dict(proposed)
                        best_train_sum = after_sum
                        best_val_sum = val_after_sum
                        best_train_metrics = dict(train_metrics)
                        best_metrics = dict(val_metrics)
                else:
                    best = dict(proposed)
                    best_train_sum = after_sum
                    best_val_sum = val_after_sum
                    best_train_metrics = dict(train_metrics)
                    best_metrics = dict(val_metrics)

    full_adapter = WaveBConstrainedGEPAAdapter(
        cases_list, max_body_chars=max_body_chars
    )
    full_best = full_adapter.evaluate(None, best, capture_traces=False)
    full_metrics = _aggregate_objective(full_best)
    best_metrics = {
        **full_metrics,
        "train_entity_f1": best_train_metrics.get("entity_f1"),
        "val_entity_f1": best_metrics.get("entity_f1"),
        "val_relation_f1": best_metrics.get("relation_f1"),
        "acceptance": acceptance,
        "min_support": effective_min_support,
        "max_type_hints": max_type_hints,
        "max_new_hints": max_new_hints,
        "max_val_gap": max_val_gap,
        "train_blend": train_blend,
        "val_gap": round(float(best_train_metrics.get("entity_f1") or 0.0) - float(best_metrics.get("entity_f1") or 0.0), 6),
    }

    import importlib.util

    gepa_available = importlib.util.find_spec("gepa") is not None
    train_e = float(best_metrics.get("train_entity_f1") or 0.0)
    val_e = float(best_metrics.get("val_entity_f1") or 0.0)
    val_gap = train_e - val_e

    diagnostics = (
        f"case_count:{n}",
        f"train:{len(train)}",
        f"val:{len(val)}",
        f"seed_entity_f1:{seed_metrics.get('entity_f1')}",
        f"best_entity_f1:{best_metrics.get('entity_f1')}",
        f"oracle_entity_f1:{oracle_pkg.metrics.get('entity_f1')}",
        f"coverage_ratio:{coverage.get('coverage_ratio')}",
        f"iterations:{len(iterations)}",
        f"accepted_any:{any(x.get('accepted') for x in iterations)}",
        f"acceptance:{acceptance}",
        f"min_support:{effective_min_support}",
        f"max_type_hints:{max_type_hints}",
        f"val_gap:{round(val_gap, 4)}",
        f"type_hints:{str(best.get(COMPONENT_ENTITY) or '').count('TYPE_HINT:')}",
        f"gepa_package:{str(gepa_available).lower()}",
        "mode:offline_reflective_spike",
        "dspy:false",
        "import_write_fail_closed",
        "llm:false",
    )
    return WaveBGEPASpikePackage(
        schema_version=SCHEMA_VERSION,
        mode="offline_reflective_spike",
        case_count=n,
        train_count=len(train),
        val_count=len(val),
        seed_metrics=seed_metrics,
        best_metrics=best_metrics,
        floor_metrics=dict(floor_metrics) if floor_metrics else None,
        seed_candidate=seed,
        best_candidate=best,
        iterations=tuple(iterations),
        reflective_samples=tuple(reflective_samples[:24]),
        diagnostics=diagnostics,
        gepa_package_available=gepa_available,
        gepa_ran=False,
        llm_used=False,
        dspy_optimizer_enabled=False,
        import_eligible=False,
        graph_writes_allowed=False,
        oracle_ceiling_metrics=dict(oracle_pkg.metrics),
        coverage_summary={
            "train": coverage,
            "val": val_cov,
        },
    )


def try_gepa_optimize(
    *,
    cases: Sequence[Mapping[str, Any]],
    seed_candidate: Mapping[str, str] | None = None,
    max_metric_calls: int = 30,
    max_body_chars: int = 8000,
    reflection_lm: Callable[[str], str] | None = None,
) -> dict[str, Any]:
    """Optional real gepa.optimize call. Returns status dict; never raises import.

    Requires optional dependency ``gepa``. Reflection LM must be provided by caller
    (injectible); default skips with reason if missing.
    """
    import importlib.util

    if importlib.util.find_spec("gepa") is None:
        return {
            "ran": False,
            "reason": "gepa_package_not_installed",
            "import_eligible": False,
        }
    if reflection_lm is None:
        return {
            "ran": False,
            "reason": "reflection_lm_not_provided",
            "import_eligible": False,
            "hint": "Pass injectible reflection_lm; do not hardcode secrets",
        }
    import gepa  # type: ignore

    adapter = WaveBConstrainedGEPAAdapter(cases, max_body_chars=max_body_chars)
    seed = dict(seed_candidate or DEFAULT_CANDIDATE)
    # gepa.optimize expects trainset as list of examples adapter understands.
    # Our adapter reads case_id from batch items matching prepared cases.
    trainset = [{"case_id": str(c.get("case_id") or "")} for c in cases]
    try:
        result = gepa.optimize(
            seed_candidate=seed,
            trainset=trainset,
            adapter=adapter,
            reflection_lm=reflection_lm,
            max_metric_calls=max_metric_calls,
        )
    except TypeError:
        # API shape may differ across gepa versions — capture and fail closed
        return {
            "ran": False,
            "reason": "gepa_optimize_signature_mismatch",
            "import_eligible": False,
        }
    except Exception as exc:  # noqa: BLE001
        return {
            "ran": False,
            "reason": f"gepa_optimize_error:{type(exc).__name__}",
            "import_eligible": False,
        }
    best = getattr(result, "best_candidate", None) or getattr(
        result, "best_program", None
    )
    return {
        "ran": True,
        "reason": "ok",
        "best_candidate": dict(best) if isinstance(best, Mapping) else best,
        "import_eligible": False,
        "dspy_optimizer_enabled": False,
    }


def _aggregate_objective(batch: EvaluationBatch) -> dict[str, float]:
    if not batch.objective_scores:
        mean_score = (
            sum(batch.scores) / len(batch.scores) if batch.scores else 0.0
        )
        return {
            "entity_f1": mean_score,
            "relation_f1": 0.0,
            "entity_recall": 0.0,
            "entity_precision": 0.0,
        }
    keys = ("entity_f1", "relation_f1", "entity_recall", "entity_precision")
    out: dict[str, float] = {}
    for k in keys:
        vals = [float(o.get(k) or 0.0) for o in batch.objective_scores]
        out[k] = sum(vals) / len(vals) if vals else 0.0
    return out


def gepa_package_available() -> bool:
    import importlib.util

    return importlib.util.find_spec("gepa") is not None


__all__ = [
    "SCHEMA_VERSION",
    "COMPONENT_ENTITY",
    "COMPONENT_RELATION",
    "DEFAULT_CANDIDATE",
    "EvaluationBatch",
    "WaveBConstrainedGEPAAdapter",
    "WaveBGEPASpikePackage",
    "gepa_package_available",
    "instruction_rule_select",
    "make_select_fn_from_candidate",
    "offline_reflective_spike",
    "parse_entity_type_hints",
    "parse_relation_hints",
    "propose_instruction_from_reflection",
    "try_gepa_optimize",
]
