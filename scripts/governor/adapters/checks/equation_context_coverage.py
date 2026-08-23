"""equation_context_coverage contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_equation_context_coverage(root) -> list[DriftFlag]:
    """Evaluate the data-driven equation context coverage contract (PIR-020).

    Reads `scripts/governor/checks/equation_context_coverage.yaml` (DATA)
    and validates the committed EquationCandidate fixtures against the
    revision-grounded equation contract in `da_domain::equation_candidate`
    (M400 S03, ADR-051): every positive candidate must be clean — formula
    identity, revision/section grounding, typed representation, source
    span, deterministic identity, locks false (D127 / ADR-053); every
    negative record must trigger exactly its labelled violation and every
    required violation must appear in the fixture; the Rust domain and
    the snapshot equation candidate suite must keep the named enforcement
    markers.
    """
    path = root / "scripts/governor/checks/equation_context_coverage.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="equation_context_coverage_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="equation_context_coverage",
        )]
    return _candidate_contract_flags(
        contract, root, "equation_context_coverage",
        "equation_context_coverage",
        _equation_candidate_violations,
    )

