"""quarantine_semantic_boundary check — GAR-012 quarantine is isolation, not quality.

Reads `scripts/governor/checks/quarantine_semantic_boundary.yaml` (DATA) and
enforces that a quarantine outcome stays a containment statement:

- `boundary_role` is `isolation_only` and the declared containment claims
  (`establishes`) and semantic non-claims (`does_not_establish`) are
  non-empty and disjoint;
- every non-claim has a `claim_routing` entry whose evidence kind is declared,
  whose evidence surface exists and still carries its pinned identity, and
  which never routes back to the quarantine scanner (`quarantine_scan_routes`),
  so a semantic usefulness claim is answered by executed/evaluated evidence
  and an admissibility claim by the licence/provenance register;
- `reused_definitions` resolves the existing quarantine definitions in the
  scholarly parse plane DATA (no second copy of the token/boundary lists):
  each scope must exist, must carry the `semantic_boundary` pointer back to
  this contract and declare `isolation_only: true`;
- a quarantine definition scope that carries a `semantic_claim_keys` key is
  `..._claim_leak` — a quarantine outcome must not be readable as a utility,
  relevance, quality, parity or admissibility statement;
- the contract must not restate quarantine definitions itself
  (`..._rival_copy`), and the finding must still be present in the review
  registry (`..._finding_missing`).

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

DEFAULT_CONTRACT = "scripts/governor/checks/quarantine_semantic_boundary.yaml"
DEFAULT_DIMENSION = "scholarly_parse_plane"

# Quarantine-definition keys the boundary contract must never carry: copying
# `tokens`, `boundaries` or `source_bindings` here would create a second,
# divergent home for the quarantine definitions the scholarly plane owns.
DEFAULT_RIVAL_DEFINITION_KEYS = frozenset(
    {
        "tokens",
        "boundaries",
        "forbidden_tokens",
        "source_bindings",
        "source_artifacts",
        "sanctioned_endpoints",
        "closed_taxonomy",
        "quarantine_definitions",
        "planes",
        "defaults",
    }
)

def _flag(name: str, evidence: str, severity: str, dimension: str) -> DriftFlag:
    return DriftFlag(flag=name, severity=severity, evidence=evidence, dimension=dimension)

def _load_contract(contract_path: Path) -> Mapping[str, Any] | str:
    """Return the parsed mapping, or an error string the caller turns into a flag."""
    try:
        loaded = yaml.safe_load(contract_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return f"cannot load {contract_path}: {error}"
    if not isinstance(loaded, Mapping):
        return f"contract must be a YAML mapping, got {type(loaded).__name__}: {contract_path}"
    return loaded

def _iter_keys(node: Any, out: set[str]) -> None:
    """Collect every mapping key reachable from `node` (recursive)."""
    if isinstance(node, Mapping):
        for key, value in node.items():
            out.add(str(key))
            _iter_keys(value, out)
    elif isinstance(node, list):
        for item in node:
            _iter_keys(item, out)

def _resolve_scope(document: Any, scope: str) -> list[tuple[str, Any]]:
    """Resolve a dotted DATA scope, supporting a trailing `*_quarantine` glob.

    Returns labelled `(scope_label, node)` pairs. A missing intermediate key
    simply yields no nodes, which the caller reports as a missing scope.
    """
    parts = [part for part in str(scope).split(".") if part]
    nodes: list[tuple[str, Any]] = [("", document)]
    for part in parts:
        next_nodes: list[tuple[str, Any]] = []
        for label, node in nodes:
            if not isinstance(node, Mapping):
                continue
            if part.startswith("*") and part.endswith("_quarantine"):
                for key, value in node.items():
                    if str(key).endswith("_quarantine"):
                        next_nodes.append((f"{label}.{key}" if label else str(key), value))
            elif part in node:
                next_nodes.append((f"{label}.{part}" if label else part, node[part]))
        nodes = next_nodes
    return nodes

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

def check_quarantine_semantic_boundary(
    root: Path, contract_rel: str | None = None
) -> list[DriftFlag]:
    """Validate the GAR-012 quarantine semantic boundary (isolation only)."""
    requested = contract_rel or DEFAULT_CONTRACT
    candidate = Path(requested)
    contract_path = candidate if candidate.is_absolute() else root / candidate
    if not contract_path.is_file():
        return []

    loaded = _load_contract(contract_path)
    if isinstance(loaded, str):
        return [
            _flag(
                "quarantine_semantic_boundary_invalid", loaded, "high", DEFAULT_DIMENSION
            )
        ]
    contract = loaded

    dimension = str(contract.get("dimension", DEFAULT_DIMENSION))
    severity = str(contract.get("severity", "medium"))
    flags: list[DriftFlag] = []

    def broken(evidence: str, flag_severity: str | None = None) -> None:
        flags.append(
            _flag(
                "quarantine_semantic_boundary_broken",
                evidence,
                flag_severity or severity,
                dimension,
            )
        )

    def high(name: str, evidence: str) -> None:
        flags.append(_flag(name, evidence, "high", dimension))

    # --- boundary role and the two claim lists ----------------------------
    if contract.get("boundary_role") != "isolation_only":
        broken(
            f"boundary_role must be isolation_only, got {contract.get('boundary_role')!r}: "
            "quarantine establishes containment and nothing else"
        )
    canonical = str(contract.get("canonical_contract", "")).strip()
    if not canonical:
        broken("canonical_contract must name the canonical boundary contract path")
    annotation_key = str(contract.get("boundary_annotation_key", "")).strip()
    if not annotation_key:
        broken("boundary_annotation_key must name the pointer key the reused scopes carry")

    establishes_raw = contract.get("establishes")
    establishes = [
        str(item) for item in establishes_raw if str(item).strip()
    ] if isinstance(establishes_raw, list) else []
    if not establishes:
        broken("establishes must be a non-empty list of containment claims")

    non_claims_raw = contract.get("does_not_establish")
    non_claims = [
        str(item) for item in non_claims_raw if str(item).strip()
    ] if isinstance(non_claims_raw, list) else []
    if not non_claims:
        broken("does_not_establish must be a non-empty list of semantic non-claims")
    overlap = sorted(set(establishes) & set(non_claims))
    if overlap:
        broken(
            f"claims {overlap} appear in both establishes and does_not_establish; "
            "a quarantine outcome cannot both prove and not prove a claim"
        )

    # --- claim routing -----------------------------------------------------
    kinds_raw = contract.get("routing_evidence_kinds")
    kinds = [str(item) for item in kinds_raw if str(item).strip()] if isinstance(kinds_raw, list) else []
    if not kinds:
        broken("routing_evidence_kinds must declare the closed evidence kinds")
    scan_routes_raw = contract.get("quarantine_scan_routes")
    forbidden_routes = (
        {str(item) for item in scan_routes_raw if str(item).strip()}
        if isinstance(scan_routes_raw, list)
        else set()
    )
    if not forbidden_routes:
        broken("quarantine_scan_routes must name the routes a non-claim may never take")

    routing_raw = contract.get("claim_routing")
    if not isinstance(routing_raw, Mapping) or not routing_raw:
        broken("claim_routing must be a non-empty mapping of non-claim -> evidence surface")
        routing: Mapping[str, Any] = {}
    else:
        routing = routing_raw
    unknown_routes = sorted(set(str(key) for key in routing) - set(non_claims))
    if unknown_routes:
        broken(
            f"claim_routing decides undeclared claims {unknown_routes}; "
            "every routed term must appear in does_not_establish"
        )
    for term in non_claims:
        entry = routing.get(term)
        if not isinstance(entry, Mapping):
            high(
                "quarantine_semantic_boundary_routing_missing",
                f"non-claim {term!r} has no claim_routing entry: a semantic usefulness "
                "claim must route to executed or evaluated evidence",
            )
            continue
        routed_to = str(entry.get("routed_to", "")).strip()
        evidence_kind = str(entry.get("evidence_kind", "")).strip()
        if not routed_to or evidence_kind not in kinds:
            broken(
                f"claim_routing.{term} must declare a routed_to surface and a declared "
                f"evidence_kind (got routed_to={routed_to!r}, evidence_kind={evidence_kind!r})"
            )
            continue
        if routed_to in forbidden_routes:
            high(
                "quarantine_semantic_boundary_routing_forbidden",
                f"claim_routing.{term} routes to the quarantine scanner {routed_to!r}; "
                "isolation can never decide a semantic claim",
            )
            continue
        evidence_rel = str(entry.get("path", "")).strip()
        if not evidence_rel:
            broken(f"claim_routing.{term} is missing the routed evidence path")
            continue
        evidence_path = root / evidence_rel
        if not evidence_path.is_file():
            high(
                "quarantine_semantic_boundary_evidence_missing",
                f"claim_routing.{term} routes to missing evidence surface {evidence_rel}",
            )
            continue
        try:
            evidence_text = evidence_path.read_text(encoding="utf-8", errors="replace")
        except OSError as error:
            high(
                "quarantine_semantic_boundary_evidence_missing",
                f"claim_routing.{term} evidence {evidence_rel} unreadable: {error}",
            )
            continue
        required_patterns = entry.get("required_patterns")
        if not isinstance(required_patterns, list) or not required_patterns:
            broken(f"claim_routing.{term} must pin required_patterns on its evidence surface")
            continue
        missing = [str(p) for p in required_patterns if str(p) not in evidence_text]
        if missing:
            high(
                "quarantine_semantic_boundary_evidence_missing",
                f"routed evidence {evidence_rel} lost the pinned identity {missing}; "
                f"a semantic claim must be decided by live evidence",
            )

    # --- reused quarantine definitions ------------------------------------
    reused_raw = contract.get("reused_definitions")
    if not isinstance(reused_raw, list) or not reused_raw:
        broken("reused_definitions must be a non-empty list of quarantine bindings")
        reused: list[Any] = []
    else:
        reused = reused_raw
    semantic_claim_keys = (
        {str(item) for item in contract.get("semantic_claim_keys")}
        if isinstance(contract.get("semantic_claim_keys"), list)
        else set()
    )
    if not semantic_claim_keys:
        broken("semantic_claim_keys must declare the keys a quarantine definition may never carry")
    rival_keys_raw = contract.get("rival_definition_keys")
    rival_keys = (
        {str(item) for item in rival_keys_raw}
        if isinstance(rival_keys_raw, list)
        else set(DEFAULT_RIVAL_DEFINITION_KEYS)
    )

    for index, binding in enumerate(reused):
        where = f"reused_definitions[{index}]"
        if not isinstance(binding, Mapping):
            broken(f"{where} must be a mapping")
            continue
        reused_rel = str(binding.get("path", "")).strip()
        scope = str(binding.get("scope", "")).strip()
        if not reused_rel or not scope:
            broken(f"{where} must declare path and scope")
            continue
        used_for = binding.get("used_for_scope")
        if not isinstance(used_for, list) or not used_for:
            broken(f"{where} must declare used_for_scope")
        elif not {str(item) for item in used_for} <= set(establishes):
            broken(
                f"{where} maps to undeclared containment claims "
                f"{sorted({str(item) for item in used_for} - set(establishes))}"
            )
        scope_path = root / reused_rel
        if not scope_path.is_file():
            high(
                "quarantine_semantic_boundary_scope_missing",
                f"{where} quarantine definition missing: {reused_rel}",
            )
            continue
        scope_loaded = _load_contract(scope_path)
        if isinstance(scope_loaded, str):
            high(
                "quarantine_semantic_boundary_scope_missing",
                f"{where} quarantine definition unreadable {reused_rel}: {scope_loaded}",
            )
            continue
        nodes = _resolve_scope(scope_loaded, scope)
        if not nodes:
            high(
                "quarantine_semantic_boundary_scope_missing",
                f"{where} scope {scope!r} resolves to nothing in {reused_rel}: "
                "the reused quarantine definition moved",
            )
            continue
        annotations = binding.get("required_scope_annotations")
        for label, node in nodes:
            claim_keys: set[str] = set()
            _iter_keys(node, claim_keys)
            leaked = sorted(claim_keys & semantic_claim_keys)
            if leaked:
                high(
                    "quarantine_semantic_boundary_claim_leak",
                    f"{reused_rel}#{label} carries semantic claim keys {leaked}: "
                    "a quarantine outcome is containment, never a utility or "
                    "relevance statement",
                )
            if isinstance(annotations, Mapping) and annotations:
                if not isinstance(node, Mapping):
                    high(
                        "quarantine_semantic_boundary_scope_missing",
                        f"{reused_rel}#{label} is not a mapping; cannot carry the "
                        "isolation annotation",
                    )
                    continue
                for key, expected in annotations.items():
                    key_name = str(key)
                    if node.get(key_name) != expected:
                        # The pointer back to this contract is the separation
                        # anchor: a quarantine scope must name the canonical
                        # boundary, not another (rival) document.
                        if key_name == annotation_key and canonical and node.get(key_name) is not None:
                            high(
                                "quarantine_semantic_boundary_scope_missing",
                                f"{reused_rel}#{label}.{key_name} points at "
                                f"{node.get(key_name)!r}, not the canonical boundary "
                                f"{canonical!r}",
                            )
                            continue
                        high(
                            "quarantine_semantic_boundary_scope_missing",
                            f"{reused_rel}#{label}.{key_name} must be {expected!r}, "
                            f"got {node.get(key_name)!r}: the quarantine is not scoped "
                            "to isolation",
                        )
        required_patterns = binding.get("required_patterns")
        if isinstance(required_patterns, list) and required_patterns:
            try:
                scope_text = scope_path.read_text(encoding="utf-8", errors="replace")
            except OSError as error:
                high(
                    "quarantine_semantic_boundary_scope_missing",
                    f"{where} quarantine definition unreadable {reused_rel}: {error}",
                )
                continue
            missing = [str(p) for p in required_patterns if str(p) not in scope_text]
            if missing:
                high(
                    "quarantine_semantic_boundary_scope_missing",
                    f"{reused_rel} no longer carries the reused quarantine markers {missing}",
                )

    # --- the contract must not become a second quarantine home ------------
    for surface_name, surface in [("contract", contract)] + [
        (f"reused_definitions[{i}]", b) for i, b in enumerate(reused) if isinstance(b, Mapping)
    ]:
        hits = sorted(set(str(key) for key in surface) & rival_keys)
        if hits:
            high(
                "quarantine_semantic_boundary_rival_copy",
                f"{surface_name} declares quarantine-definition keys {hits}; the "
                "reused quarantine definitions stay in the scholarly parse plane "
                "DATA, not in a second copy",
            )

    # --- the boundary must name the finding it remediates -----------------
    finding = str(contract.get("finding", "")).strip()
    if not finding:
        broken("finding must name the review finding this boundary remediates")
    registry_rel = str(contract.get("finding_registry", "")).strip()
    if not registry_rel:
        broken("finding_registry must name the review registry")
    elif finding:
        registry_path = root / registry_rel
        if registry_path.is_file():
            registry_ids = _registry_finding_ids(registry_path)
            if registry_ids and finding not in registry_ids:
                high(
                    "quarantine_semantic_boundary_finding_missing",
                    f"{finding} is not present in {registry_rel}",
                )

    return flags

def summarize_quarantine_semantic_boundary(
    root: Path, contract_rel: str | None = None
) -> dict[str, Any]:
    """Read-only summary of the boundary for diagnostics (never a flag).

    Returns the containment claims, the semantic non-claims and their routed
    evidence surfaces, the reused quarantine scopes and any non-claim that is
    not routed. A missing or malformed contract yields `{"present": False, ...}`
    rather than raising.
    """
    requested = contract_rel or DEFAULT_CONTRACT
    candidate = Path(requested)
    contract_path = candidate if candidate.is_absolute() else root / candidate
    if not contract_path.is_file():
        return {"present": False, "contract": requested}
    loaded = _load_contract(contract_path)
    if isinstance(loaded, str):
        return {"present": True, "contract": requested, "error": loaded}
    non_claims = [
        str(item) for item in (loaded.get("does_not_establish") or []) if str(item).strip()
    ]
    routing = loaded.get("claim_routing") if isinstance(loaded.get("claim_routing"), Mapping) else {}
    routed = {term: str((routing.get(term) or {}).get("routed_to", "")) for term in non_claims}
    scopes: list[str] = []
    for binding in loaded.get("reused_definitions") or []:
        if isinstance(binding, Mapping):
            scopes.append(f"{binding.get('path')}#{binding.get('scope')}")
    return {
        "present": True,
        "contract": requested,
        "finding": loaded.get("finding"),
        "boundary_role": loaded.get("boundary_role"),
        "establishes": [str(item) for item in (loaded.get("establishes") or [])],
        "does_not_establish": non_claims,
        "routing": routed,
        "reused_scopes": scopes,
        "unrouted": [term for term in non_claims if not routed.get(term)],
    }
