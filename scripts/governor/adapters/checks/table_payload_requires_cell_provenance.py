"""table_payload_requires_cell_provenance contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_table_payload_requires_cell_provenance(root) -> list[DriftFlag]:
    """Evaluate the data-driven table cell provenance contract (PIR-019).

    Reads `scripts/governor/checks/table_payload_requires_cell_provenance.yaml`
    (DATA) and validates the committed TableCandidate fixtures against
    the revision-grounded table contract in `da_domain::table_candidate`
    + `da_domain::table_fidelity` (M400 S02, ADR-051): every positive
    candidate must be clean — TEI grounding (R087), per-cell
    row/column/digest/span provenance, deterministic cell identities,
    typed conflict kinds only, authority/import locks false (D127 /
    ADR-053); every negative record must trigger exactly its labelled
    violation and every required violation must appear in the fixture;
    the Rust domain/fidelity and the snapshot table fidelity suite must
    keep the named enforcement markers.
    """
    path = root / "scripts/governor/checks/table_payload_requires_cell_provenance.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="table_payload_requires_cell_provenance_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="table_cell_provenance",
        )]
    return _candidate_contract_flags(
        contract, root, "table_cell_provenance",
        "table_payload_requires_cell_provenance",
        _table_candidate_violations,
    )

