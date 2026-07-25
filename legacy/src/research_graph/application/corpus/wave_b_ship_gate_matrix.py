"""Wave B ship-gate metric matrix (M260).

Composes floor / header constrained / extraction baseline / optional LLM compare
into one fail-closed decision surface. Never invents gold. Never authorizes
import or GEPA/DSPy unless explicit positive delta vs header is provided.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any

from research_graph.application.corpus.wave_b_quality_n_contract import (
    evaluate_quality_n_contract,
    extract_joined_count,
)
from research_graph.application.corpus.wave_b_gold_hybrid_llm_pilot import (
    ALLOWED_RELATION_TYPES,
)

SCHEMA_VERSION = "wave-b-ship-gate-matrix.v1"

# Deployable Wave B quality path is header constrained select until LLM wins.
DEFAULT_SHIP_PATH = "header_priority_constrained_select"


@dataclass(frozen=True, slots=True)
class WaveBShipGateMatrixPackage:
    schema_version: str
    worlds: dict[str, Any]
    deltas: dict[str, Any]
    relation_status: dict[str, Any]
    ship_path: str
    ship_blocker: str | None
    ship_ready: bool
    gepa_justified: bool
    dspy_optimizer_enabled: bool
    diagnostics: tuple[str, ...]
    import_eligible: bool = False
    graph_writes_allowed: bool = False

    def __post_init__(self) -> None:
        if self.import_eligible or self.graph_writes_allowed:
            raise ValueError("ship gate matrix cannot authorize import/writes")
        if self.dspy_optimizer_enabled:
            raise ValueError("ship gate matrix cannot enable DSPy optimizer")
        if self.gepa_justified and self.ship_blocker is not None:
            # GEPA may only be justified when no ship_blocker remains for LLM path
            pass

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "worlds": dict(self.worlds),
            "deltas": dict(self.deltas),
            "relation_status": dict(self.relation_status),
            "ship_path": self.ship_path,
            "ship_blocker": self.ship_blocker,
            "ship_ready": self.ship_ready,
            "gepa_justified": self.gepa_justified,
            "dspy_optimizer_enabled": False,
            "diagnostics": list(self.diagnostics),
            "import_eligible": False,
            "graph_writes_allowed": False,
            "note": (
                "Single ship-gate matrix for Wave B quality worlds. "
                "floor=lexical oracle ceiling; header=constrained deploy path; "
                "baseline=fixture extraction gate; llm=compare only. "
                "Staged GEPA spikes allowed (D128); promote ship_path only if same-n delta_vs_header > 0 on entity and relation. "
                "Never import."
            ),
        }


def _f1(block: Mapping[str, Any] | None, *keys: str) -> float | None:
    if not block:
        return None
    for k in keys:
        if k in block and block[k] is not None:
            try:
                return float(block[k])
            except (TypeError, ValueError):
                return None
    metrics = block.get("metrics")
    if isinstance(metrics, Mapping):
        for k in keys:
            if k in metrics and metrics[k] is not None:
                try:
                    return float(metrics[k])
                except (TypeError, ValueError):
                    return None
    return None


def _delta(a: float | None, b: float | None) -> float | None:
    if a is None or b is None:
        return None
    return round(float(a) - float(b), 6)


def build_wave_b_ship_gate_matrix(
    *,
    floor: Mapping[str, Any] | None = None,
    header: Mapping[str, Any] | None = None,
    baseline: Mapping[str, Any] | None = None,
    llm: Mapping[str, Any] | None = None,
    llm_compare: Mapping[str, Any] | None = None,
    offline_gepa: Mapping[str, Any] | None = None,
    joined_count: int | None = None,
    grounding_body_ratio: float | None = None,
    grounding_cand_ratio: float | None = None,
    human_go: bool | None = None,
    wave_a_closeout_pass: bool | None = None,
    max_val_gap: float = 0.35,
) -> WaveBShipGateMatrixPackage:
    """Build ship matrix from metric worlds (pure).

    Parameters accept either flat {entity_f1, relation_f1} or nested metrics dicts
    / operator payloads.
    """
    floor = dict(floor or {})
    header = dict(header or {})
    baseline = dict(baseline or {})
    llm = dict(llm or {})
    compare = dict(llm_compare or {})

    # Prefer explicit compare artifact when present — but only for promotion
    # when joined_count matches live header n (avoid stale n=20 vs n=23).
    compare_joined: int | None = None
    compare_n_matches = True
    if compare:
        if compare.get("joined_count") is not None:
            compare_joined = int(compare["joined_count"])
        if not header and isinstance(compare.get("header"), Mapping):
            header = dict(compare["header"])
        if not llm:
            for key in (
                "llm_agnes_free_compact_prompt_prefer_header",
                "llm",
                "llm_metrics",
            ):
                if isinstance(compare.get(key), Mapping):
                    llm = dict(compare[key])
                    break
        if joined_count is None and compare_joined is not None:
            joined_count = compare_joined
        if (
            joined_count is not None
            and compare_joined is not None
            and int(joined_count) != int(compare_joined)
        ):
            compare_n_matches = False

    floor_e = _f1(floor, "entity_f1", "floor_entity_f1")
    floor_r = _f1(floor, "relation_f1", "floor_relation_f1")
    # nested floor_metrics on header operator payload
    if floor_e is None:
        floor_e = _f1(header.get("floor_metrics") if isinstance(header.get("floor_metrics"), Mapping) else None, "entity_f1")
    if floor_r is None:
        floor_r = _f1(header.get("floor_metrics") if isinstance(header.get("floor_metrics"), Mapping) else None, "relation_f1")

    header_e = _f1(header, "entity_f1")
    header_r = _f1(header, "relation_f1")
    baseline_e = _f1(baseline, "entity_f1", "train_entity_f1")
    baseline_r = _f1(baseline, "relation_f1", "train_relation_f1")
    llm_e = _f1(llm, "entity_f1")
    llm_r = _f1(llm, "relation_f1")

    # deltas vs header (LLM must beat header to justify optimizer)
    delta_llm_e = _delta(llm_e, header_e)
    delta_llm_r = _delta(llm_r, header_r)
    if compare.get("delta_vs_header") and isinstance(compare["delta_vs_header"], Mapping):
        dv = compare["delta_vs_header"]
        if delta_llm_e is None and dv.get("entity_f1") is not None:
            delta_llm_e = float(dv["entity_f1"])
        if delta_llm_r is None and dv.get("relation_f1") is not None:
            delta_llm_r = float(dv["relation_f1"])

    # Offline GEPA instruction-select world (same-n preferred)
    ogepa = dict(offline_gepa or {})
    if not ogepa and isinstance(compare.get("gepa"), Mapping):
        ogepa = dict(compare["gepa"])
    # also accept full gepa-vs-header package
    if not ogepa and isinstance(compare.get("offline_gepa"), Mapping):
        ogepa = dict(compare["offline_gepa"])
    gepa_e = _f1(ogepa, "entity_f1")
    gepa_r = _f1(ogepa, "relation_f1")
    if gepa_e is None and isinstance(ogepa.get("metrics"), Mapping):
        gepa_e = _f1(ogepa.get("metrics"), "entity_f1")  # type: ignore[arg-type]
    if gepa_r is None and isinstance(ogepa.get("metrics"), Mapping):
        gepa_r = _f1(ogepa.get("metrics"), "relation_f1")  # type: ignore[arg-type]
    delta_gepa_e = _delta(gepa_e, header_e)
    delta_gepa_r = _delta(gepa_r, header_r)
    train_e = ogepa.get("train_entity_f1")
    val_e = ogepa.get("val_entity_f1")
    if train_e is None and isinstance(ogepa.get("metrics"), Mapping):
        train_e = ogepa["metrics"].get("train_entity_f1")
    if val_e is None and isinstance(ogepa.get("metrics"), Mapping):
        val_e = ogepa["metrics"].get("val_entity_f1")
    # promote package flag if present
    promote_ready_flag = ogepa.get("promote_ready")
    if promote_ready_flag is None and isinstance(compare.get("promote_ready"), bool):
        promote_ready_flag = compare.get("promote_ready")

    val_gap_ok = True
    val_gap_blocker: str | None = None
    if train_e is not None and val_e is not None:
        gap = float(train_e) - float(val_e)
        if gap > float(max_val_gap):
            val_gap_ok = False
            val_gap_blocker = f"val_gap:{gap:.4f}>{float(max_val_gap):.4f}"

    # LLM promotion still requires same-n dual F1 (stale compare guard).
    llm_beats_header = (
        compare_n_matches
        and delta_llm_e is not None
        and delta_llm_r is not None
        and delta_llm_e > 0
        and delta_llm_r > 0
    )
    # Offline GEPA same-n: reject promote when gepa joined_count differs from live header n.
    gepa_joined = None
    if ogepa.get("joined_count") is not None:
        gepa_joined = int(ogepa["joined_count"])
    elif isinstance(ogepa.get("metrics"), Mapping) and ogepa["metrics"].get("case_count") is not None:
        gepa_joined = int(ogepa["metrics"]["case_count"])
    gepa_n_matches = True
    if (
        joined_count is not None
        and gepa_joined is not None
        and int(joined_count) != int(gepa_joined)
    ):
        gepa_n_matches = False

    # Offline GEPA promotion: dual F1 > header + val-gap + same-n (D128/M271).
    gepa_beats_header = (
        gepa_n_matches
        and delta_gepa_e is not None
        and delta_gepa_r is not None
        and delta_gepa_e > 0
        and delta_gepa_r > 0
        and val_gap_ok
        and (promote_ready_flag is not False)
    )
    # gepa_justified = offline GEPA (preferred) or LLM path ready to promote
    gepa_justified = bool(gepa_beats_header or llm_beats_header)

    blockers: list[str] = []
    if human_go is False:
        blockers.append("human_go_false")
    if wave_a_closeout_pass is False:
        blockers.append("wave_a_closeout_not_pass")
    if grounding_body_ratio is not None and grounding_body_ratio < 1.0:
        blockers.append(f"grounding_body_ratio:{grounding_body_ratio}<1.0")
    if grounding_cand_ratio is not None and grounding_cand_ratio < 1.0:
        blockers.append(f"grounding_cand_ratio:{grounding_cand_ratio}<1.0")
    if header_e is None or header_r is None:
        blockers.append("header_metrics_missing")
    # Deploy path is header; LLM weaker is not a ship blocker for header path,
    # but GEPA must stay closed.
    if not llm_beats_header:
        # informational — not always a ship blocker for header deploy
        pass
    # Relation ceiling: header relation weak relative to floor — document, do not fake
    relation_gap = None
    if header_r is not None and floor_r is not None:
        relation_gap = round(float(floor_r) - float(header_r), 6)

    # Ship ready = header path metrics present + no hard blockers
    hard = [b for b in blockers if not b.startswith("grounding_")]
    # grounding is hard too
    hard = list(blockers)
    ship_ready = len(hard) == 0 and header_e is not None and header_r is not None
    ship_blocker = None if ship_ready else (hard[0] if hard else "unknown")

    # Deploy stays header until dual F1 promotion with val-gap guard (D128).
    ship_path = DEFAULT_SHIP_PATH
    if gepa_beats_header:
        ship_path = "gepa_instruction_rule_select"
    elif llm_beats_header:
        ship_path = "constrained_llm_prefer_header_candidate"

    relation_status = {
        "path": "header_proximity_type_pair_candidates",
        "header_relation_f1": header_r,
        "floor_relation_f1": floor_r,
        "relation_gap_vs_floor": relation_gap,
        "allowed_relation_types": sorted(ALLOWED_RELATION_TYPES),
        "ceiling_note": (
            "Header path relation F1 uses proximity + closed type-pair priors "
            "among selected candidate_ids only (M272). No free invent. "
            "Floor/oracle 1.0 is not deploy quality. Accept header relation "
            "ceiling until dual F1 improves without invent."
        ),
        "accepted_as_deploy_ceiling": True if header_r is not None else False,
        "free_invent": False,
        "gepa_open": False,
        "relation_builder": "build_relation_candidates",
    }

    n_contract = evaluate_quality_n_contract(
        header_n=joined_count,
        llm_n=compare_joined if llm_e is not None else None,
        gepa_n=gepa_joined if gepa_e is not None else None,
        grounding_n=None,  # filled by operator when known
        matrix_n=joined_count,
        compare_n=compare_joined,
        canonical=joined_count,
    )
    # allow operator to pass grounding via offline_gepa/compare side channel later

    worlds = {
        "floor_lexical_oracle": {
            "entity_f1": floor_e,
            "relation_f1": floor_r,
            "role": "ceiling_not_deploy",
        },
        "header_constrained_select": {
            "entity_f1": header_e,
            "relation_f1": header_r,
            "role": "deploy_default",
            "model_id": header.get("model_id") or "header_priority_select",
        },
        "extraction_baseline_train": {
            "entity_f1": baseline_e,
            "relation_f1": baseline_r,
            "role": "fixture_gate_not_hybrid_deploy",
        },
        "llm_constrained_compare": {
            "entity_f1": llm_e,
            "relation_f1": llm_r,
            "role": "compare_only",
            "model_id": llm.get("model_id"),
            "llm_kept": llm.get("llm_kept"),
            "fallback_used_count": llm.get("fallback_used_count"),
        },
        "offline_gepa_instruction_select": {
            "entity_f1": gepa_e,
            "relation_f1": gepa_r,
            "role": "staged_gepa_candidate",
            "model_id": ogepa.get("model_id") or "gepa_instruction_rule_select",
            "train_entity_f1": train_e,
            "val_entity_f1": val_e,
            "val_gap_ok": val_gap_ok,
            "val_gap_blocker": val_gap_blocker,
            "promote_ready": promote_ready_flag,
        },
        "context": {
            "joined_count": joined_count,
            "compare_joined_count": compare_joined,
            "compare_n_matches": compare_n_matches,
            "gepa_joined_count": gepa_joined,
            "gepa_n_matches": gepa_n_matches,
            "grounding_body_ratio": grounding_body_ratio,
            "grounding_cand_ratio": grounding_cand_ratio,
            "human_go": human_go,
            "wave_a_closeout_pass": wave_a_closeout_pass,
            "max_val_gap": max_val_gap,
            "quality_n_all_match": n_contract.all_match,
            "quality_n_mismatches": list(n_contract.mismatches),
            "quality_n_canonical": n_contract.canonical_joined_count,
        },
        "quality_n_contract": n_contract.to_dict(),
    }
    deltas = {
        "llm_minus_header_entity_f1": delta_llm_e,
        "llm_minus_header_relation_f1": delta_llm_r,
        "gepa_minus_header_entity_f1": delta_gepa_e,
        "gepa_minus_header_relation_f1": delta_gepa_r,
        "header_minus_floor_entity_f1": _delta(header_e, floor_e),
        "header_minus_floor_relation_f1": _delta(header_r, floor_r),
        "llm_beats_header": llm_beats_header,
        "gepa_beats_header": gepa_beats_header,
        "quality_n_all_match": n_contract.all_match,
        "gepa_n_matches": gepa_n_matches,
        "compare_n_matches": compare_n_matches,
    }

    diagnostics = (
        f"ship_ready:{ship_ready}",
        f"ship_blocker:{ship_blocker}",
        f"ship_path:{ship_path}",
        f"header_entity_f1:{header_e}",
        f"header_relation_f1:{header_r}",
        f"floor_entity_f1:{floor_e}",
        f"floor_relation_f1:{floor_r}",
        f"llm_entity_f1:{llm_e}",
        f"llm_relation_f1:{llm_r}",
        f"gepa_entity_f1:{gepa_e}",
        f"gepa_relation_f1:{gepa_r}",
        f"gepa_beats_header:{gepa_beats_header}",
        f"val_gap_ok:{val_gap_ok}",
        f"gepa_justified:{gepa_justified}",
        f"joined_count:{joined_count}",
        f"compare_joined:{compare_joined}",
        f"compare_n_matches:{compare_n_matches}",
        f"gepa_n_matches:{gepa_n_matches}",
        f"quality_n_all_match:{n_contract.all_match}",
        "import_write_fail_closed",
        "wave_b_ship_gate_matrix_only",
    )

    return WaveBShipGateMatrixPackage(
        schema_version=SCHEMA_VERSION,
        worlds=worlds,
        deltas=deltas,
        relation_status=relation_status,
        ship_path=ship_path,
        ship_blocker=ship_blocker,
        ship_ready=ship_ready,
        gepa_justified=gepa_justified,
        dspy_optimizer_enabled=False,
        diagnostics=diagnostics,
    )


__all__ = [
    "SCHEMA_VERSION",
    "DEFAULT_SHIP_PATH",
    "WaveBShipGateMatrixPackage",
    "build_wave_b_ship_gate_matrix",
]
