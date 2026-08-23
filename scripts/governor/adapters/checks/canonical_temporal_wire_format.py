"""canonical_temporal_wire_format contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_canonical_temporal_wire_format(root) -> list[DriftFlag]:
    """Evaluate the data-driven canonical temporal wire contract (PIR-008).

    Reads `scripts/governor/checks/canonical_temporal_wire_format.yaml`
    (DATA). The ontology DATA file (`data/ontology/node_schemas.yaml`)
    must carry the `temporal_wire_contract` markers; the positive wire
    fixture must contain only values the write validator accepts
    (canonical RFC3339 UTC strings, OPEN, null); the negative fixture
    must contain only values the write validator rejects (integer
    timestamps, non-UTC offsets, non-RFC3339 strings, wrong types). The
    Rust test `temporal_wire_fixtures_match_committed_governor_data` in
    `crates/kg-ontology/src/temporal.rs` pins the fixtures to the actual
    validator, so a broken contract cannot silently survive a
    regenerate. The classification predicate `_wire_value_accepted`
    mirrors `kg_ontology::temporal::validate_datetime_wire`.
    """
    path = root / "scripts/governor/checks/canonical_temporal_wire_format.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="canonical_temporal_wire_format_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="temporal_ast",
        )]
    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "temporal_ast"))
    severity = str(contract.get("severity", "high"))

    def _flag(name: str, evidence: str) -> None:
        flags.append(DriftFlag(
            flag=name, severity=severity, evidence=evidence, dimension=dimension,
        ))

    onto = contract.get("ontology_data") or {}
    onto_path = root / str(onto.get("path", ""))
    if not onto_path.is_file():
        _flag("canonical_temporal_wire_format_ontology_data_missing",
              f"ontology DATA missing: {onto_path}")
    else:
        text = onto_path.read_text(errors="replace")
        missing = [m for m in onto.get("required_markers", []) if m not in text]
        if missing:
            _flag("canonical_temporal_wire_format_ontology_marker_missing",
                  f"ontology DATA missing contract markers: {missing}")

    for polarity, expected in (("positive", True), ("negative", False)):
        fixture = (contract.get("wire_fixtures") or {}).get(polarity) or {}
        fixture_path = root / str(fixture.get("path", ""))
        if not fixture_path.is_file():
            _flag("canonical_temporal_wire_format_fixture_missing",
                  f"{polarity} fixture missing: {fixture_path}")
            continue
        try:
            values = json.loads(fixture_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag("canonical_temporal_wire_format_fixture_invalid",
                  f"{polarity} fixture unreadable: {error}")
            continue
        if not isinstance(values, list) or not values:
            _flag("canonical_temporal_wire_format_fixture_invalid",
                  f"{polarity} fixture must be a non-empty JSON array")
            continue
        bad = [v for v in values if _wire_value_accepted(v) != expected]
        if bad:
            _flag("canonical_temporal_wire_format_broken",
                  f"{polarity} fixture {fixture.get('id')} violates the wire contract: "
                  f"{bad}")
    return flags

