"""fsm_interaction_map check — GAR-014 bounded cross-automata invalidation map.

Reads `scripts/governor/checks/fsm_interaction_map.yaml` (DATA) and enforces
that the published map stays a *map*:

- every automaton named in `automata` has at least one invalidating input
  recorded in `invalidation_rules` (the GAR-014 acceptance condition);
- every automaton records its local transitions, retry preconditions and the
  states that authorize nothing;
- every invalidation rule names a known automaton, a declared trigger class and
  a declared action, and declares whether re-derivation is required;
- every trigger class (source / policy / model / input) is represented;
- every evidence binding resolves to a real file that still carries the pinned
  symbols, so the map cannot drift away from the automata it describes;
- no executive controller key appears: the map describes transitions and must
  never execute, schedule or dispatch them;
- a pointer file under `data/policies/` does not grow a second, rival map.

A missing contract is silent (checks are opt-in per project), matching
`feature_matrix_gate`. Every flag returned here reaches the trajectory report,
so a violation is a drift risk, never an informational note.
"""
from __future__ import annotations

import json
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import yaml

from governor.domain.trajectory import DriftFlag

DEFAULT_CONTRACT = "scripts/governor/checks/fsm_interaction_map.yaml"
DEFAULT_DIMENSION = "scholarly_parse_plane"

# Keys a descriptive map must never carry. Any of these at the top level, on an
# automaton, on a transition/precondition entry or on an invalidation rule is a
# controller leaking into what must stay DATA.
DEFAULT_FORBIDDEN_CONTROLLER_KEYS = frozenset(
    {
        "controller",
        "executor",
        "runner",
        "dispatch",
        "orchestrate",
        "execute_transition",
        "tick",
        "advance",
        "step",
        "command",
        "apply",
    }
)

_RIVAL_MAP_KEYS = ("automata", "invalidation_rules")


def _flag(name: str, evidence: str, severity: str, dimension: str) -> DriftFlag:
    return DriftFlag(flag=name, severity=severity, evidence=evidence, dimension=dimension)


def _load_contract(contract_path: Path) -> Mapping[str, Any] | str:
    """Return the parsed mapping, or an error string the caller turns into a flag."""
    try:
        loaded = yaml.safe_load(contract_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return f"cannot load {contract_path}: {error}"
    if not isinstance(loaded, Mapping):
        return (
            f"contract must be a YAML mapping, got {type(loaded).__name__}: {contract_path}"
        )
    return loaded


def check_fsm_interaction_map(
    root: Path, contract_rel: str | None = None
) -> list[DriftFlag]:
    """Validate the bounded cross-automata invalidation map (GAR-014)."""
    requested = contract_rel or DEFAULT_CONTRACT
    candidate = Path(requested)
    contract_path = candidate if candidate.is_absolute() else root / candidate
    if not contract_path.is_file():
        return []

    loaded = _load_contract(contract_path)
    if isinstance(loaded, str):
        return [_flag("fsm_interaction_map_invalid", loaded, "high", DEFAULT_DIMENSION)]
    contract = loaded

    dimension = str(contract.get("dimension", DEFAULT_DIMENSION))
    severity = str(contract.get("severity", "medium"))
    flags: list[DriftFlag] = []

    def broken(evidence: str, flag_severity: str | None = None) -> None:
        flags.append(
            _flag("fsm_interaction_map_broken", evidence, flag_severity or severity, dimension)
        )

    # --- the map must not be a controller ---------------------------------
    if contract.get("map_role") != "descriptive_only":
        flags.append(
            _flag(
                "fsm_interaction_map_controller_leak",
                f"map_role must be descriptive_only, got {contract.get('map_role')!r}: "
                "the interaction map describes invalidations and must not execute transitions",
                "high",
                dimension,
            )
        )
    declared_forbidden = contract.get("forbidden_controller_keys")
    forbidden = (
        {str(key) for key in declared_forbidden}
        if isinstance(declared_forbidden, list) and declared_forbidden
        else set(DEFAULT_FORBIDDEN_CONTROLLER_KEYS)
    )

    # --- automata ---------------------------------------------------------
    automata_raw = contract.get("automata")
    if not isinstance(automata_raw, Mapping) or not automata_raw:
        automata: dict[str, Mapping[str, Any]] = {}
        broken("automata must be a non-empty mapping of automaton id -> local contract")
    else:
        automata = {str(name): entry for name, entry in automata_raw.items()}

    surfaces: list[tuple[str, Mapping[str, Any]]] = [("contract", contract)]
    for name, entry in automata.items():
        if not isinstance(entry, Mapping):
            broken(f"automaton {name} must be a mapping, got {type(entry).__name__}")
            continue
        surfaces.append((f"automata.{name}", entry))
        for field in ("authority", "evidence_source"):
            if not str(entry.get(field, "")).strip():
                broken(f"automaton {name} is missing {field}")
        if entry.get("grants_production_authority") is not False:
            flags.append(
                _flag(
                    "fsm_interaction_map_controller_leak",
                    f"automaton {name} must declare grants_production_authority: false; "
                    "no automaton in this map may authorize production or import",
                    "high",
                    dimension,
                )
            )
        for field in ("local_transitions", "retry_preconditions", "non_authorizing_states"):
            value = entry.get(field)
            if not isinstance(value, list) or not value:
                broken(f"automaton {name} records no {field}")
        transitions = entry.get("local_transitions")
        if isinstance(transitions, list):
            for index, transition in enumerate(transitions):
                where = f"automaton {name} local_transitions[{index}]"
                if not isinstance(transition, Mapping):
                    broken(f"{where} must be a mapping")
                    continue
                surfaces.append((where, transition))
                if not str(transition.get("id", "")).strip():
                    broken(f"{where} is missing id")
                if transition.get("stays_local") is not True:
                    broken(f"{where} must declare stays_local: true")
        preconditions = entry.get("retry_preconditions")
        if isinstance(preconditions, list):
            for index, precondition in enumerate(preconditions):
                where = f"automaton {name} retry_preconditions[{index}]"
                if not isinstance(precondition, Mapping):
                    broken(f"{where} must be a mapping")
                    continue
                surfaces.append((where, precondition))
                if not str(precondition.get("id", "")).strip():
                    broken(f"{where} is missing id")
                if not isinstance(precondition.get("requires_rederivation"), bool):
                    broken(f"{where} must declare requires_rederivation as a boolean")
                for field in ("allowed_when", "forbidden_when"):
                    if not isinstance(precondition.get(field), list):
                        broken(f"{where} must declare {field} as a list")
        states = entry.get("non_authorizing_states")
        if isinstance(states, list):
            for index, state in enumerate(states):
                if not str(state).strip():
                    broken(f"automaton {name} non_authorizing_states[{index}] is not a state name")

    # --- invalidation rules ----------------------------------------------
    trigger_classes = [
        str(item) for item in (contract.get("trigger_classes") or []) if str(item).strip()
    ]
    actions = [str(item) for item in (contract.get("actions") or []) if str(item).strip()]
    if not trigger_classes:
        broken("trigger_classes must declare the closed trigger classes")
    if not actions:
        broken("actions must declare the closed invalidation actions")

    rules_raw = contract.get("invalidation_rules")
    if not isinstance(rules_raw, list) or not rules_raw:
        broken("invalidation_rules must be a non-empty list")
        rules: list[Any] = []
    else:
        rules = rules_raw

    invalidated: set[str] = set()
    seen_classes: set[str] = set()
    for index, rule in enumerate(rules):
        where = f"invalidation_rules[{index}]"
        if not isinstance(rule, Mapping):
            broken(f"{where} must be a mapping, got {type(rule).__name__}")
            continue
        surfaces.append((where, rule))
        if not str(rule.get("trigger", "")).strip():
            broken(f"{where} is missing trigger")
        trigger_class = str(rule.get("trigger_class", ""))
        if trigger_class not in trigger_classes:
            broken(
                f"{where} has unknown trigger_class {trigger_class!r}; "
                f"expected one of {trigger_classes}"
            )
        else:
            seen_classes.add(trigger_class)
        action = str(rule.get("action", ""))
        if action not in actions:
            broken(f"{where} has unknown action {action!r}; expected one of {actions}")
        targets = rule.get("invalidates")
        if not isinstance(targets, list) or not targets:
            broken(f"{where} invalidates nothing: an invalidation rule must name an automaton")
        else:
            for target in targets:
                target_id = str(target)
                if target_id not in automata:
                    flags.append(
                        _flag(
                            "fsm_interaction_map_unknown_automaton",
                            f"{where} invalidates undeclared automaton {target_id!r}",
                            "high",
                            dimension,
                        )
                    )
                else:
                    invalidated.add(target_id)
        requires = rule.get("requires_rederivation")
        if not isinstance(requires, bool):
            broken(f"{where} must declare requires_rederivation as a boolean")
        elif action == "none" and requires is True:
            broken(f"{where} action none cannot require re-derivation")
        elif action in actions and action != "none" and requires is not True:
            broken(
                f"{where} action {action} must require re-derivation: "
                "a retry may not bypass the invalidation it follows"
            )

    for name in automata:
        if name not in invalidated:
            flags.append(
                _flag(
                    "fsm_interaction_map_unmapped_automaton",
                    f"automaton {name} has no invalidating input recorded in invalidation_rules",
                    "high",
                    dimension,
                )
            )
    for trigger_class in trigger_classes:
        if trigger_class not in seen_classes:
            flags.append(
                _flag(
                    "fsm_interaction_map_trigger_class_missing",
                    f"no invalidation rule records a {trigger_class} change",
                    "high",
                    dimension,
                )
            )

    # --- controller key scan ---------------------------------------------
    for surface_name, surface in surfaces:
        hits = sorted(set(surface) & forbidden)
        if hits:
            flags.append(
                _flag(
                    "fsm_interaction_map_controller_leak",
                    f"{surface_name} declares executive controller keys {hits}; "
                    "the map must not execute, schedule or dispatch transitions",
                    "high",
                    dimension,
                )
            )

    # --- non-authorizing invariants --------------------------------------
    invariants = contract.get("non_authorizing_invariants")
    if not isinstance(invariants, list) or not invariants:
        broken("non_authorizing_invariants must be a non-empty list")
    else:
        for index, invariant in enumerate(invariants):
            where = f"non_authorizing_invariants[{index}]"
            if not isinstance(invariant, Mapping):
                broken(f"{where} must be a mapping")
                continue
            if not str(invariant.get("id", "")).strip():
                broken(f"{where} is missing id")
            if not str(invariant.get("statement", "")).strip():
                broken(f"{where} is missing statement")

    # --- evidence bindings ------------------------------------------------
    bindings = contract.get("evidence_bindings")
    if not isinstance(bindings, list) or not bindings:
        broken("evidence_bindings must be a non-empty list")
    else:
        for index, binding in enumerate(bindings):
            where = f"evidence_bindings[{index}]"
            if not isinstance(binding, Mapping):
                broken(f"{where} must be a mapping")
                continue
            rel = str(binding.get("path", "")).strip()
            if not rel:
                broken(f"{where} is missing path")
                continue
            target = root / rel
            if not target.is_file():
                flags.append(
                    _flag(
                        "fsm_interaction_map_evidence_missing",
                        f"{where} evidence path missing: {rel}",
                        "high",
                        dimension,
                    )
                )
                continue
            try:
                text = target.read_text(encoding="utf-8", errors="replace")
            except OSError as error:
                flags.append(
                    _flag(
                        "fsm_interaction_map_evidence_missing",
                        f"{where} evidence path unreadable {rel}: {error}",
                        "high",
                        dimension,
                    )
                )
                continue
            patterns = binding.get("required_patterns")
            if not isinstance(patterns, list) or not patterns:
                broken(f"{where} must declare required_patterns")
                continue
            missing = [str(pattern) for pattern in patterns if str(pattern) not in text]
            if missing:
                flags.append(
                    _flag(
                        "fsm_interaction_map_evidence_drift",
                        f"{rel} no longer carries the pinned symbols {missing}: "
                        "the map describes an automaton that moved",
                        "high",
                        dimension,
                    )
                )

    # --- pointer must not become a rival map ------------------------------
    pointer_sources = contract.get("pointer_sources")
    if isinstance(pointer_sources, list):
        for rel in pointer_sources:
            pointer_path = root / str(rel)
            if not pointer_path.is_file():
                continue
            pointer_loaded = _load_contract(pointer_path)
            if isinstance(pointer_loaded, str):
                flags.append(
                    _flag(
                        "fsm_interaction_map_duplicate_map",
                        f"pointer {rel} is unreadable: {pointer_loaded}",
                        "high",
                        dimension,
                    )
                )
                continue
            rival = [key for key in _RIVAL_MAP_KEYS if pointer_loaded.get(key)]
            if rival:
                flags.append(
                    _flag(
                        "fsm_interaction_map_duplicate_map",
                        f"pointer {rel} declares {rival}: the canonical map is {requested}; "
                        "a second maintained copy would let invalidation reasoning diverge",
                        "high",
                        dimension,
                    )
                )
            canonical = pointer_loaded.get("canonical_map")
            if canonical is not None and str(canonical) != str(requested):
                flags.append(
                    _flag(
                        "fsm_interaction_map_duplicate_map",
                        f"pointer {rel} points at {canonical!r} instead of the canonical map "
                        f"{requested!r}",
                        "high",
                        dimension,
                    )
                )

    # --- map must name the finding it remediates --------------------------
    finding = str(contract.get("finding", "")).strip()
    if not finding:
        broken("finding must name the review finding this map remediates")
    registry_rel = str(contract.get("finding_registry", "")).strip()
    if finding and registry_rel:
        registry_path = root / registry_rel
        if registry_path.is_file():
            registry_ids = _registry_finding_ids(registry_path)
            if registry_ids and finding not in registry_ids:
                flags.append(
                    _flag(
                        "fsm_interaction_map_finding_missing",
                        f"{finding} is not present in {registry_rel}",
                        "high",
                        dimension,
                    )
                )

    return flags


def _registry_finding_ids(registry_path: Path) -> set[str]:
    """Collect `finding_id` values from a JSONL review registry (tolerant read)."""
    try:
        lines = registry_path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return set()
    ids: set[str] = set()
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        try:
            record = json.loads(stripped)
        except ValueError:
            continue
        if isinstance(record, Mapping) and record.get("finding_id"):
            ids.add(str(record["finding_id"]))
    return ids


def summarize_fsm_interaction_map(root: Path, contract_rel: str | None = None) -> dict[str, Any]:
    """Read-only summary of the published map for diagnostics (never a flag).

    Returns the automata, the recorded triggers per automaton and the trigger
    classes represented. A missing or malformed contract yields
    `{"present": False, ...}` rather than raising, because this helper is only
    used to inspect state after the check has already reported.
    """
    requested = contract_rel or DEFAULT_CONTRACT
    candidate = Path(requested)
    contract_path = candidate if candidate.is_absolute() else root / candidate
    if not contract_path.is_file():
        return {"present": False, "contract": requested}
    loaded = _load_contract(contract_path)
    if isinstance(loaded, str):
        return {"present": True, "contract": requested, "error": loaded}
    automata = loaded.get("automata")
    automata_ids = sorted(str(name) for name in automata) if isinstance(automata, Mapping) else []
    triggers: dict[str, list[str]] = {name: [] for name in automata_ids}
    classes: set[str] = set()
    for rule in loaded.get("invalidation_rules") or []:
        if not isinstance(rule, Mapping):
            continue
        trigger = str(rule.get("trigger", ""))
        classes.add(str(rule.get("trigger_class", "")))
        for target in rule.get("invalidates") or []:
            triggers.setdefault(str(target), []).append(trigger)
    return {
        "present": True,
        "contract": requested,
        "finding": loaded.get("finding"),
        "map_role": loaded.get("map_role"),
        "automata": automata_ids,
        "triggers": triggers,
        "trigger_classes": sorted(name for name in classes if name),
        "unmapped": [name for name in automata_ids if not triggers.get(name)],
    }
