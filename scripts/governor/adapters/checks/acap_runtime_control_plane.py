"""acap_runtime_control_plane contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_acap_runtime_control_plane(root) -> list[DriftFlag]:
    """Evaluate the data-driven ACAP runtime control-plane contract.

    Reads `scripts/governor/checks/acap_runtime_control_plane.yaml`
    (DATA) and validates the committed positive/negative fixtures for
    runtime admission (PIR-014), promotion authority (PIR-015), durable
    scheduler recovery (PIR-016) and replay idempotence, mirroring the
    same-process proof in `acap_control_plane_test.rs` (M399/S04 T01):
    every positive record must be clean across all four planes; every
    negative record must trigger exactly its labelled violation and every
    required violation must appear in the fixture (a future edit that
    removes a negative case is flagged); the Rust enforcement points must
    keep the named typed guards. The admission FSM transition table is
    read from `article_context_assurance.yaml` (the canonical ACAP
    policy), so the runtime gate mirrors `ArticleContextState::
    can_transition_to` by construction.
    """
    path = root / "scripts/governor/checks/acap_runtime_control_plane.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="acap_runtime_control_plane_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="acap_runtime_control_plane",
        )]
    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "acap_runtime_control_plane"))
    severity = str(contract.get("severity", "high"))

    def _flag(name: str, evidence: str) -> None:
        flags.append(DriftFlag(
            flag=name, severity=severity, evidence=evidence, dimension=dimension,
        ))

    # Admission legality depends on the canonical ACAP FSM transition
    # table. Fail closed: an unreadable policy means no transition is
    # declared, so every from/to pair is illegal.
    transitions: set = set()
    fsm_policy = str((contract.get("admission") or {}).get("fsm_policy", ""))
    if fsm_policy:
        fsm_path = root / fsm_policy
        if fsm_path.is_file():
            try:
                policy = yaml.safe_load(fsm_path.read_text(encoding="utf-8")) or {}
                for transition in ((policy.get("fsm") or {}).get("transitions") or []):
                    if isinstance(transition, list) and len(transition) == 2:
                        transitions.add((str(transition[0]), str(transition[1])))
            except (OSError, yaml.YAMLError):
                pass

    planes = [
        ("admission", _acap_admission_violations, transitions),
        ("promotion", _acap_promotion_authority_violations, None),
        ("scheduler", _acap_scheduler_violations, None),
        ("replay", _acap_replay_violations, None),
    ]
    for plane_key, fn, extra in planes:
        section = contract.get(plane_key) or {}
        fixtures = section.get("fixtures") or {}
        for kind in ("positive", "negative"):
            spec = fixtures.get(kind) or {}
            fixture_path = root / str(spec.get("path", ""))
            if not fixture_path.is_file():
                _flag("acap_runtime_control_plane_fixture_missing",
                      f"{plane_key}/{kind} fixture missing: {fixture_path}")
                continue
            try:
                records = json.loads(fixture_path.read_text(encoding="utf-8"))
            except (ValueError, OSError) as error:
                _flag("acap_runtime_control_plane_fixture_invalid",
                      f"{plane_key}/{kind} fixture unreadable: {error}")
                continue
            if not isinstance(records, list) or not records:
                _flag("acap_runtime_control_plane_fixture_invalid",
                      f"{plane_key}/{kind} fixture must be a non-empty JSON array")
                continue
            if kind == "positive":
                broken = []
                for record in records:
                    if not isinstance(record, dict):
                        broken.append(("<non-object>", ["missing_record_fields"]))
                        continue
                    violations = fn(record, contract) if extra is None else fn(record, contract, extra)
                    if violations:
                        identity = record.get(
                            "stage_id",
                            record.get("decision_id", record.get("task_vid", record.get("scenario", "?"))),
                        )
                        broken.append((identity, sorted(violations)))
                if broken:
                    _flag("acap_runtime_control_plane_broken",
                          f"{plane_key} positive fixture {spec.get('id')} must be clean, "
                          f"got violations: {broken}")
            else:
                observed: set[str] = set()
                mismatches = []
                for record in records:
                    if not isinstance(record, dict):
                        mismatches.append("<non-object>: not a labelled record")
                        continue
                    label = str(record.get("violation", "unnamed"))
                    payload = record.get("record") or {}
                    violations = fn(payload, contract) if extra is None else fn(payload, contract, extra)
                    observed.update(violations)
                    if label not in violations:
                        mismatches.append(
                            f"{label}: expected to trigger {label}, "
                            f"got {sorted(violations)}"
                        )
                if mismatches:
                    _flag("acap_runtime_control_plane_broken",
                          f"{plane_key} negative fixture {spec.get('id')} records do not "
                          f"trigger their labelled violations: {'; '.join(mismatches)}")
                require = list(
                    (spec.get("expect") or {}).get("require_violations", [])
                )
                missing_required = [v for v in require if v not in observed]
                if missing_required:
                    _flag("acap_runtime_control_plane_broken",
                          f"{plane_key} negative fixture {spec.get('id')} missing required "
                          f"violation coverage: {missing_required}")

    # Source markers: the Rust enforcement points must stay present.
    for marker_key, entry in (contract.get("source_markers") or {}).items():
        marker_path = root / str(entry.get("path", ""))
        if not marker_path.is_file():
            _flag("acap_runtime_control_plane_source_marker_missing",
                  f"{marker_key} source missing: {marker_path}")
            continue
        text = marker_path.read_text(errors="replace")
        missing = [p for p in entry.get("required_patterns", []) if p not in text]
        if missing:
            _flag("acap_runtime_control_plane_source_marker_missing",
                  f"{marker_key} source missing enforcement markers: {missing}")
        forbidden_hits = [p for p in entry.get("forbidden_patterns", []) if p in text]
        if forbidden_hits:
            _flag("acap_runtime_control_plane_source_marker_missing",
                  f"{marker_key} source still contains forbidden markers: {forbidden_hits}")

    # Optional DATA map: present only on the production contract. Isolated
    # fixture tests omit it so they do not need a full pipeline ledger.
    ledger_layers = contract.get("ledger_layers") or {}
    if ledger_layers:
        ledger_path = root / str(ledger_layers.get("path", ""))
        if not ledger_path.is_file():
            _flag("acap_runtime_control_plane_ledger_layers_missing",
                  f"pipeline ledger missing: {ledger_path}")
        else:
            try:
                ledger = yaml.safe_load(ledger_path.read_text(encoding="utf-8")) or {}
            except (OSError, yaml.YAMLError) as error:
                _flag("acap_runtime_control_plane_ledger_layers_missing",
                      f"cannot load pipeline ledger {ledger_path}: {error}")
            else:
                stages = {
                    str(stage.get("id")): stage
                    for stage in (ledger.get("stages") or [])
                    if isinstance(stage, dict) and stage.get("id")
                }
                for stage_id, expected in (ledger_layers.get("required") or {}).items():
                    actual = (stages.get(str(stage_id)) or {}).get("acap_layer")
                    if actual != expected:
                        _flag(
                            "acap_runtime_control_plane_ledger_layers_missing",
                            f"stage {stage_id} acap_layer={actual!r} expected {expected!r}",
                        )
                for stage_id in ledger_layers.get("forbidden") or []:
                    if (stages.get(str(stage_id)) or {}).get("acap_layer") is not None:
                        _flag(
                            "acap_runtime_control_plane_ledger_layers_missing",
                            f"residual stage {stage_id} must not declare acap_layer",
                        )
    return flags

