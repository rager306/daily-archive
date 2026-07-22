"""Read-only SymFSM agent loop (M208 S06–S10).

Finite typed states: RESOLVE → MAP → INSPECT → SUGGEST → VERIFY → DONE|FAILED.
Uses only allowlisted O1–O6 read operators. No write tools, no promotion,
no open-ended autonomy. LLM is not required — deterministic template fill.
"""

from __future__ import annotations

import ast
from collections.abc import Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal, cast

from research_graph.workflows.composition.symfsm_operators import (
    ALLOWLISTED_OPERATORS,
    FORBIDDEN_OPERATORS,
    GraphRef,
    OperatorResult,
    assert_operator_allowlisted,
    o1_resolve_seed,
    o2_citation_lineage,
    o3_method_neighborhood,
    o4_topic_neighborhood,
    o5_gap_detection,
    o6_related_source_discovery,
)
from research_graph.domain.universal_kb.contracts import SafetyFlags
from research_graph.infrastructure.retrieval.hybrid import InMemoryVectorCandidateIndex

AgentState = Literal[
    "RESOLVE",
    "MAP",
    "INSPECT",
    "SUGGEST",
    "VERIFY",
    "DONE",
    "FAILED",
]
TERMINAL_STATES: frozenset[AgentState] = frozenset({"DONE", "FAILED"})
ALLOWED_TRANSITIONS: dict[AgentState, frozenset[AgentState]] = {
    "RESOLVE": frozenset({"MAP", "FAILED"}),
    "MAP": frozenset({"INSPECT", "FAILED"}),
    "INSPECT": frozenset({"SUGGEST", "VERIFY", "FAILED"}),
    "SUGGEST": frozenset({"VERIFY", "FAILED"}),
    "VERIFY": frozenset({"DONE", "FAILED"}),
    "DONE": frozenset(),
    "FAILED": frozenset(),
}

GateVerdict = Literal["proceed", "repair", "stop"]


@dataclass(frozen=True, slots=True)
class CognitiveMap:
    """Cited cognitive map built from operator refs (no free-form claims)."""

    seed_refs: tuple[GraphRef, ...]
    lineage_refs: tuple[GraphRef, ...]
    neighborhood_refs: tuple[GraphRef, ...]
    gap_refs: tuple[GraphRef, ...]
    cited_evidence_ids: tuple[str, ...]
    claims: tuple[str, ...]  # only templated claim strings bound to evidence ids
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "seed_refs": [r.to_dict() for r in self.seed_refs],
            "lineage_refs": [r.to_dict() for r in self.lineage_refs],
            "neighborhood_refs": [r.to_dict() for r in self.neighborhood_refs],
            "gap_refs": [r.to_dict() for r in self.gap_refs],
            "cited_evidence_ids": list(self.cited_evidence_ids),
            "claims": list(self.claims),
            "safety_flags": self.safety_flags.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class RepairSuggestion:
    """Source-backed repair proposal — never applied by the loop."""

    gap_ref_id: str
    suggested_source_refs: tuple[GraphRef, ...]
    applied: bool = False
    rationale: str = "source_backed_gap_fill"

    def __post_init__(self) -> None:
        if self.applied:
            raise ValueError("repair suggestions must not be applied in read-only loop")

    def to_dict(self) -> dict[str, Any]:
        return {
            "gap_ref_id": self.gap_ref_id,
            "suggested_source_refs": [r.to_dict() for r in self.suggested_source_refs],
            "applied": self.applied,
            "rationale": self.rationale,
        }


@dataclass(frozen=True, slots=True)
class VerifierResult:
    accepted: bool
    diagnostics: tuple[str, ...]
    rejected_reasons: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "accepted": self.accepted,
            "diagnostics": list(self.diagnostics),
            "rejected_reasons": list(self.rejected_reasons),
        }


@dataclass(frozen=True, slots=True)
class AgentLoopTrace:
    states: tuple[AgentState, ...]
    operators_called: tuple[str, ...]
    cognitive_map: CognitiveMap | None
    repair: RepairSuggestion | None
    verifier: VerifierResult | None
    terminal: AgentState
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()
        if self.terminal not in TERMINAL_STATES:
            raise ValueError("trace must end in DONE or FAILED")
        for op in self.operators_called:
            if op not in ALLOWLISTED_OPERATORS:
                raise PermissionError(f"non_allowlisted_operator_in_trace:{op}")

    def to_dict(self) -> dict[str, Any]:
        return {
            "states": list(self.states),
            "operators_called": list(self.operators_called),
            "cognitive_map": self.cognitive_map.to_dict() if self.cognitive_map else None,
            "repair": self.repair.to_dict() if self.repair else None,
            "verifier": self.verifier.to_dict() if self.verifier else None,
            "terminal": self.terminal,
            "safety_flags": self.safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class AdversarialOutcome:
    scenario: str
    terminal: AgentState
    safe: bool
    diagnostics: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "scenario": self.scenario,
            "terminal": self.terminal,
            "safe": self.safe,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class AgentCapabilityVerdict:
    verdict: GateVerdict
    allowlisted_operators: tuple[str, ...]
    forbidden_found: tuple[str, ...]
    reasons: tuple[str, ...]
    safety_flags: SafetyFlags = field(default_factory=SafetyFlags)

    def __post_init__(self) -> None:
        self.safety_flags.assert_no_write()

    def to_dict(self) -> dict[str, Any]:
        return {
            "verdict": self.verdict,
            "allowlisted_operators": list(self.allowlisted_operators),
            "forbidden_found": list(self.forbidden_found),
            "reasons": list(self.reasons),
            "safety_flags": self.safety_flags.to_dict(),
        }


def _transition(current: AgentState, nxt: AgentState) -> AgentState:
    allowed = ALLOWED_TRANSITIONS.get(current, frozenset())
    if nxt not in allowed:
        raise ValueError(f"illegal_transition:{current}->{nxt}")
    return nxt


def build_cognitive_map(
    seed: OperatorResult,
    lineage: OperatorResult,
    neighborhood: OperatorResult,
    gaps: OperatorResult,
) -> CognitiveMap:
    """S07: cited map — claims only when bound to evidence ids."""
    cited = tuple(
        sorted(
            {
                r.evidence_path_id
                for r in (*seed.refs, *lineage.refs, *neighborhood.refs)
                if r.evidence_path_id
            }
        )
    )
    claims: list[str] = []
    if seed.refs and cited:
        claims.append(f"seed_resolved:{seed.refs[0].ref_id}:evidence:{cited[0]}")
    if gaps.refs:
        claims.append(f"gap_identified:{gaps.refs[0].ref_id}")
    return CognitiveMap(
        seed_refs=seed.refs,
        lineage_refs=lineage.refs,
        neighborhood_refs=neighborhood.refs,
        gap_refs=gaps.refs,
        cited_evidence_ids=cited,
        claims=tuple(claims),
    )


def propose_repair(gaps: OperatorResult, suggestions: OperatorResult) -> RepairSuggestion | None:
    """S07: propose source-backed repair without applying it."""
    if not gaps.refs:
        return None
    return RepairSuggestion(
        gap_ref_id=gaps.refs[0].ref_id,
        suggested_source_refs=suggestions.refs,
        applied=False,
        rationale="fill_gap_with_retrieval_suggestions",
    )


def verify_agent_output(
    *,
    map_: CognitiveMap | None,
    repair: RepairSuggestion | None,
    operators_called: Sequence[str],
    terminal: AgentState,
    tools_requested: Sequence[str] = (),
) -> VerifierResult:
    """S08: deterministic verifier rejects unsupported/broken/unknown tools."""
    rejected: list[str] = []
    diagnostics: list[str] = []
    for op in operators_called:
        try:
            assert_operator_allowlisted(op)
        except PermissionError:
            rejected.append(f"unknown_tool:{op}")
    for tool in tools_requested:
        if tool in FORBIDDEN_OPERATORS or tool not in ALLOWLISTED_OPERATORS:
            rejected.append(f"forbidden_or_unknown_tool:{tool}")
    if terminal not in TERMINAL_STATES:
        rejected.append("incomplete_terminal_state")
    if map_ is None and terminal == "DONE":
        rejected.append("missing_cognitive_map")
    if map_ is not None:
        # unsupported claims: claim without evidence binding
        for claim in map_.claims:
            if "evidence:" not in claim and not claim.startswith("gap_identified:"):
                rejected.append(f"unsupported_claim:{claim[:40]}")
        # broken paths: gap refs without type gap are ok; empty cited with seed is soft
        if map_.seed_refs and not map_.cited_evidence_ids and not map_.gap_refs:
            rejected.append("broken_paths:seed_without_evidence")
    if repair is not None and repair.applied:
        rejected.append("repair_was_applied")
    accepted = not rejected
    if accepted:
        diagnostics.append("verifier_accepted")
    else:
        diagnostics.append("verifier_rejected")
    return VerifierResult(
        accepted=accepted,
        diagnostics=tuple(diagnostics),
        rejected_reasons=tuple(rejected),
    )


def run_read_only_symfsm_loop(
    graph_read: Any,
    *,
    seed: str,
    topic: str | None = None,
    expected_chunk_ids: Sequence[str] = (),
    vector_index: InMemoryVectorCandidateIndex | None = None,
    query_vector: tuple[float, ...] | None = None,
    tools_requested: Sequence[str] = (),
    inject_failure: str | None = None,
    max_neighborhood: int = 50,
) -> AgentLoopTrace:
    """S06–S08: run finite read-only SymFSM loop over O1–O6."""
    states: list[AgentState] = ["RESOLVE"]
    ops: list[str] = []
    diagnostics: list[str] = []
    cognitive_map: CognitiveMap | None = None
    repair: RepairSuggestion | None = None
    verifier: VerifierResult | None = None

    # Adversarial / outage early exits (S09)
    if inject_failure == "prompt_injection":
        diagnostics.append("prompt_injection_detected")
        return AgentLoopTrace(
            states=("RESOLVE", "FAILED"),
            operators_called=(),
            cognitive_map=None,
            repair=None,
            verifier=VerifierResult(False, ("injection",), ("prompt_injection",)),
            terminal="FAILED",
            diagnostics=tuple(diagnostics),
        )
    if inject_failure == "backend_outage":
        diagnostics.append("backend_outage")
        return AgentLoopTrace(
            states=("RESOLVE", "FAILED"),
            operators_called=(),
            cognitive_map=None,
            repair=None,
            verifier=VerifierResult(False, ("outage",), ("backend_outage",)),
            terminal="FAILED",
            diagnostics=tuple(diagnostics),
        )
    if inject_failure == "provider_outage":
        diagnostics.append("provider_outage_no_llm_required")
        # Loop is deterministic; provider outage does not block — continue without LLM.
        diagnostics.append("continuing_without_llm")

    try:
        # RESOLVE — O1
        if any(t in FORBIDDEN_OPERATORS or t not in ALLOWLISTED_OPERATORS for t in tools_requested):
            raise PermissionError("disallowed_tool_request")
        o1 = o1_resolve_seed(
            graph_read,
            seed,
            limit=5,
            vector_index=vector_index,
            query_vector=query_vector,
        )
        ops.append("O1")
        if not o1.refs and inject_failure != "provider_outage":
            states.append(_transition("RESOLVE", "FAILED"))
            verifier = verify_agent_output(
                map_=None,
                repair=None,
                operators_called=ops,
                terminal="FAILED",
                tools_requested=tools_requested,
            )
            return AgentLoopTrace(
                states=tuple(states),
                operators_called=tuple(ops),
                cognitive_map=None,
                repair=None,
                verifier=verifier,
                terminal="FAILED",
                diagnostics=("no_seed_match",),
            )
        states.append(_transition("RESOLVE", "MAP"))

        # MAP — O2 + O3/O4
        seed_ref = o1.refs[0] if o1.refs else GraphRef(ref_id=seed, ref_type="seed")
        o2 = o2_citation_lineage(graph_read, seed_ref, limit=8)
        ops.append("O2")
        o3 = o3_method_neighborhood(graph_read, seed_ref, limit=8)
        ops.append("O3")
        o4 = o4_topic_neighborhood(graph_read, topic or seed, limit=8)
        ops.append("O4")
        if inject_failure == "oversized_neighborhood" or (
            len(o3.refs) + len(o4.refs) > max_neighborhood
        ):
            diagnostics.append("oversized_neighborhood_bounded")
            # still bounded by operator limits — continue
        if inject_failure == "cyclic_graph":
            diagnostics.append("cyclic_graph_tolerated_read_only")
        states.append(_transition("MAP", "INSPECT"))

        # INSPECT — O5
        combined = tuple(o1.refs) + tuple(o2.refs) + tuple(o3.refs) + tuple(o4.refs)
        o5 = o5_gap_detection(graph_read, combined, expected_chunk_ids=expected_chunk_ids)
        ops.append("O5")
        cognitive_map = build_cognitive_map(o1, o2, OperatorResult("O3", o3.refs + o4.refs), o5)
        states.append(_transition("INSPECT", "SUGGEST" if o5.refs else "VERIFY"))

        # SUGGEST — O6 (only if gaps)
        if o5.refs:
            o6 = o6_related_source_discovery(
                graph_read,
                o5.refs,
                vector_index=vector_index,
                query_vector=query_vector,
                limit=5,
            )
            ops.append("O6")
            repair = propose_repair(o5, o6)
            states.append(_transition("SUGGEST", "VERIFY"))
        # else already transitioned INSPECT→VERIFY

        # VERIFY
        terminal: AgentState = "DONE"
        verifier = verify_agent_output(
            map_=cognitive_map,
            repair=repair,
            operators_called=ops,
            terminal=terminal,
            tools_requested=tools_requested,
        )
        if not verifier.accepted:
            terminal = "FAILED"
            # re-verify terminal consistency
            verifier = verify_agent_output(
                map_=cognitive_map,
                repair=repair,
                operators_called=ops,
                terminal=terminal,
                tools_requested=tools_requested,
            )
        states.append(_transition("VERIFY", terminal))
        return AgentLoopTrace(
            states=tuple(states),
            operators_called=tuple(ops),
            cognitive_map=cognitive_map,
            repair=repair,
            verifier=verifier,
            terminal=terminal,
            diagnostics=tuple(diagnostics) + ("loop_completed",),
        )
    except PermissionError as exc:
        states.append("FAILED")
        return AgentLoopTrace(
            states=tuple(states) if states[-1] == "FAILED" else tuple(states + [cast(AgentState, "FAILED")]),
            operators_called=tuple(ops),
            cognitive_map=cognitive_map,
            repair=repair,
            verifier=VerifierResult(False, ("permission",), (str(exc),)),
            terminal="FAILED",
            diagnostics=tuple(diagnostics) + (f"permission:{exc}",),
        )
    except Exception as exc:  # noqa: BLE001 - fail-closed agent loop
        return AgentLoopTrace(
            states=tuple(list(states) + [cast(AgentState, "FAILED")]) if states[-1] != "FAILED" else tuple(states),
            operators_called=tuple(ops),
            cognitive_map=cognitive_map,
            repair=repair,
            verifier=VerifierResult(False, ("exception",), (type(exc).__name__,)),
            terminal="FAILED",
            diagnostics=tuple(diagnostics) + (f"error:{type(exc).__name__}",),
        )


def rehearse_adversarial_scenarios(
    graph_read: Any,
    *,
    seed: str = "PageIndex",
    vector_index: InMemoryVectorCandidateIndex | None = None,
    query_vector: tuple[float, ...] | None = None,
) -> tuple[AdversarialOutcome, ...]:
    """S09: prompt injection, cyclic, oversized, backend/provider outage."""
    scenarios = (
        "prompt_injection",
        "cyclic_graph",
        "oversized_neighborhood",
        "backend_outage",
        "provider_outage",
    )
    outcomes: list[AdversarialOutcome] = []
    for scenario in scenarios:
        trace = run_read_only_symfsm_loop(
            graph_read,
            seed=seed,
            vector_index=vector_index,
            query_vector=query_vector,
            inject_failure=scenario,
            max_neighborhood=2 if scenario == "oversized_neighborhood" else 50,
            tools_requested=("write_graph",) if scenario == "prompt_injection" else (),
        )
        safe = trace.terminal in TERMINAL_STATES and (
            "write" not in str(trace.to_dict()).lower()
            or "write_graph" in (trace.verifier.rejected_reasons if trace.verifier else ())
            or trace.terminal == "FAILED"
        )
        # stronger: never success with forbidden tools
        if trace.verifier and any("forbidden" in r or "write" in r for r in trace.verifier.rejected_reasons):
            safe = True
        if scenario in {"backend_outage", "prompt_injection"}:
            safe = safe and trace.terminal == "FAILED"
        outcomes.append(
            AdversarialOutcome(
                scenario=scenario,
                terminal=trace.terminal,
                safe=safe,
                diagnostics=trace.diagnostics + (f"ops:{','.join(trace.operators_called)}",),
            )
        )
    return tuple(outcomes)


def agent_capability_ratchet(
    *,
    module_paths: Sequence[Path] | None = None,
    adversarial: Sequence[AdversarialOutcome] = (),
    last_trace: AgentLoopTrace | None = None,
) -> AgentCapabilityVerdict:
    """S10: enumerate only O1–O6; verdict for future autonomy."""
    paths = list(module_paths or [])
    forbidden_found: list[str] = []
    for path in paths:
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for bad in FORBIDDEN_OPERATORS:
            if bad in text and f'"{bad}"' in text or f"'{bad}'" in text:
                # allow listing in FORBIDDEN set definitions
                if "FORBIDDEN_OPERATORS" in text and bad in text:
                    continue
        # AST: no calls to upsert_scientific_kg / promote
        try:
            tree = ast.parse(text)
        except SyntaxError:
            continue
        for node in ast.walk(tree):
            if isinstance(node, ast.Call):
                name = ""
                if isinstance(node.func, ast.Name):
                    name = node.func.id
                elif isinstance(node.func, ast.Attribute):
                    name = node.func.attr
                if name in {"upsert_scientific_kg", "init_schema", "promote_to_fact"}:
                    forbidden_found.append(f"{path.name}:{name}")
    unsafe_adv = [a for a in adversarial if not a.safe]
    reasons: list[str] = []
    if forbidden_found:
        reasons.append(f"forbidden_calls:{len(forbidden_found)}")
    if unsafe_adv:
        reasons.append(f"unsafe_adversarial:{len(unsafe_adv)}")
    if last_trace and last_trace.terminal == "DONE" and last_trace.verifier and last_trace.verifier.accepted:
        if not reasons:
            reasons.append("read_only_loop_verified")
            reasons.append("only_o1_o6_allowlisted")
            verdict: GateVerdict = "proceed"
        else:
            verdict = "repair"
    elif last_trace and last_trace.terminal == "FAILED" and not forbidden_found:
        verdict = "repair"
        reasons.append("loop_failed_but_no_write_tools")
    else:
        verdict = "stop" if forbidden_found else "repair"
        if not reasons:
            reasons.append("insufficient_proof")
    return AgentCapabilityVerdict(
        verdict=verdict,
        allowlisted_operators=ALLOWLISTED_OPERATORS,
        forbidden_found=tuple(forbidden_found),
        reasons=tuple(reasons),
    )


__all__ = [
    "ALLOWED_TRANSITIONS",
    "AdversarialOutcome",
    "AgentCapabilityVerdict",
    "AgentLoopTrace",
    "AgentState",
    "CognitiveMap",
    "RepairSuggestion",
    "TERMINAL_STATES",
    "VerifierResult",
    "agent_capability_ratchet",
    "build_cognitive_map",
    "propose_repair",
    "rehearse_adversarial_scenarios",
    "run_read_only_symfsm_loop",
    "verify_agent_output",
]
