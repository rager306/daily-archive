"""read_only_agent_contract contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_read_only_agent_contract(root) -> list[DriftFlag]:
    """Evaluate the data-driven read-only agent plane contract (PIR-017/021).

    Reads `scripts/governor/checks/read_only_agent_contract.yaml` (DATA)
    and validates the committed genuine positive/negative v1 envelopes
    produced by `da agent-run verify` (M401 S05 T01): Python checks
    envelope *structure* only (D275 / D273 / MEM885) — format, required
    fields, terminal-state vocabulary, snapshot/identity/budget/evidence
    blocks, result_records 1:1 with evidence, and the D127 lock
    (`import_eligible` absent) — while digest math (plan -> identity ->
    evidence -> claims -> verification -> result_ref) stays in Rust and
    is pinned by
    `crates/da-cli/tests/read_only_agent_contract_verify_test.rs` (exit
    0 PASS / exit 3 FAIL). The negative fixture is a *structurally
    valid* v1 envelope with a tampered raw row, so Python must stay
    clean and only the Rust verifier FAILs (exit 3).
    `_candidate_contract_flags` is deliberately NOT reused: it expects a
    JSON *array* of labelled candidate records, while each fixture here
    is a single envelope *object*. The Rust enforcement points must keep
    the named verify / evidence-chain / read-plane / serve-graph / FSM
    markers.
    """
    path = root / "scripts/governor/checks/read_only_agent_contract.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="read_only_agent_contract_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="agent_read_plane",
        )]
    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "agent_read_plane"))
    severity = str(contract.get("severity", "high"))

    def _flag(name: str, evidence: str) -> None:
        flags.append(DriftFlag(
            flag=name, severity=severity, evidence=evidence, dimension=dimension,
        ))

    # Fixtures: BOTH the positive (genuine, Rust PASS exit 0) and the
    # negative (tampered raw row, Rust FAIL exit 3) envelope must be
    # structurally clean v1 objects. `_candidate_contract_flags` is NOT
    # reused — it expects a JSON *array* of labelled candidate records,
    # while each fixture here is a single envelope *object*.
    for kind, spec in ((contract.get("fixtures") or {}).items()):
        fixture_path = root / str(spec.get("path", ""))
        if not fixture_path.is_file():
            _flag("read_only_agent_contract_fixture_missing",
                  f"{kind} fixture missing: {fixture_path}")
            continue
        try:
            envelope = json.loads(fixture_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag("read_only_agent_contract_fixture_invalid",
                  f"{kind} fixture unreadable: {error}")
            continue
        if not isinstance(envelope, dict):
            _flag("read_only_agent_contract_fixture_invalid",
                  f"{kind} fixture must be a single envelope object "
                  f"(not an array): {type(envelope).__name__}")
            continue
        violations = _read_only_agent_envelope_violations(envelope, contract)
        # D127 string-level lock, mirroring the Rust verifier guard
        # `!json.contains("import_eligible")` (da-cli/src/agent_run.rs).
        if "import_eligible" in fixture_path.read_text(encoding="utf-8"):
            violations.add("import_eligible_present")
        if violations:
            _flag("read_only_agent_contract_broken",
                  f"{kind} fixture {spec.get('id')} must be structurally clean, "
                  f"got violations: {sorted(violations)}")

    # Source markers: the Rust enforcement points must stay present.
    for marker_key, entry in (contract.get("source_markers") or {}).items():
        marker_path = root / str(entry.get("path", ""))
        if not marker_path.is_file():
            _flag("read_only_agent_contract_source_marker_missing",
                  f"{marker_key} source missing: {marker_path}")
            continue
        text = marker_path.read_text(errors="replace")
        missing = [p for p in entry.get("required_patterns", []) if p not in text]
        if missing:
            _flag("read_only_agent_contract_source_marker_missing",
                  f"{marker_key} source missing enforcement markers: {missing}")
    return flags

