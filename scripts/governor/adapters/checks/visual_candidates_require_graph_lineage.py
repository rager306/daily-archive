"""visual_candidates_require_graph_lineage contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_visual_candidates_require_graph_lineage(root) -> list[DriftFlag]:
    """Evaluate the data-driven visual candidate lineage contract (PIR-018).

    Reads `scripts/governor/checks/visual_candidates_require_graph_lineage.yaml`
    (DATA) and validates the committed FigureCandidate fixtures against
    the revision-grounded candidate contract in `da_domain::figure_candidate`
    (M400 S01, ADR-051): every positive candidate must be clean — full
    revision/section lineage, valid SHA-256 digests, deterministic
    revision-bound identity (VID matches the recomputed lineage), and
    authority/import locks false (D127 / ADR-053); every negative record
    must trigger exactly its labelled violation and every required
    violation must appear in the fixture (a future edit that removes a
    negative case is flagged); the Rust domain and the snapshot graph
    task suite must keep the named enforcement markers.
    """
    path = root / "scripts/governor/checks/visual_candidates_require_graph_lineage.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="visual_candidates_require_graph_lineage_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="visual_candidate_lineage",
        )]
    return _candidate_contract_flags(
        contract, root, "visual_candidate_lineage",
        "visual_candidates_require_graph_lineage",
        _visual_candidate_violations,
    )

