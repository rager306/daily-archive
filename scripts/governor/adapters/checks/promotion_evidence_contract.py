"""promotion_evidence_contract contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_promotion_evidence_contract(root) -> list[DriftFlag]:
    """Evaluate the data-driven promotion evidence contract (PIR-015).

    Reads `scripts/governor/checks/promotion_evidence_contract.yaml`
    (DATA) and validates the committed PromotionDecision fixtures against
    the authority-neutral promotion protocol implemented in
    `da_domain::promotion`: every positive decision must be clean across
    all five states; every negative record must trigger exactly its
    labelled violation (missing evidence, D127 import leak,
    self-confirmation, non-independent reviewer/promoter, transition
    mismatch, invalid timestamp, double promotion, terminal mutation) and
    every required violation must appear in the fixture; the domain FSM
    and the application commit service must keep the typed guard
    markers. The Rust domain tests and proptests
    (`crates/da-domain/src/promotion.rs`) plus the fail-closed contract
    test (`crates/da-application/tests/promotion_contract_test.rs`)
    implement the same guards, so a broken contract cannot silently
    survive.
    """
    path = root / "scripts/governor/checks/promotion_evidence_contract.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="promotion_evidence_contract_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="candidate_promotion",
        )]
    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "candidate_promotion"))
    severity = str(contract.get("severity", "high"))

    def _flag(name: str, evidence: str) -> None:
        flags.append(DriftFlag(
            flag=name, severity=severity, evidence=evidence, dimension=dimension,
        ))

    # Positive fixture: every committed decision must be clean.
    positive = (contract.get("fixtures") or {}).get("positive") or {}
    positive_path = root / str(positive.get("path", ""))
    if not positive_path.is_file():
        _flag("promotion_evidence_contract_fixture_missing",
              f"positive fixture missing: {positive_path}")
    else:
        try:
            decisions = json.loads(positive_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag("promotion_evidence_contract_fixture_invalid",
                  f"positive fixture unreadable: {error}")
        else:
            if not isinstance(decisions, list) or not decisions:
                _flag("promotion_evidence_contract_fixture_invalid",
                      "positive fixture must be a non-empty JSON array")
            else:
                broken = []
                for decision in decisions:
                    violations = _promotion_decision_violations(decision, contract)
                    if violations:
                        broken.append((
                            decision.get("decision_id"),
                            sorted(violations),
                        ))
                if broken:
                    _flag("promotion_evidence_contract_broken",
                          f"positive fixture {positive.get('id')} must be clean, "
                          f"got violations: {broken}")

    # Negative fixture: each record must trigger its labelled violation
    # and every required violation must appear somewhere.
    negative = (contract.get("fixtures") or {}).get("negative") or {}
    negative_path = root / str(negative.get("path", ""))
    if not negative_path.is_file():
        _flag("promotion_evidence_contract_fixture_missing",
              f"negative fixture missing: {negative_path}")
    else:
        try:
            records = json.loads(negative_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag("promotion_evidence_contract_fixture_invalid",
                  f"negative fixture unreadable: {error}")
        else:
            if not isinstance(records, list) or not records:
                _flag("promotion_evidence_contract_fixture_invalid",
                      "negative fixture must be a non-empty JSON array")
            else:
                observed: set[str] = set()
                mismatches = []
                for record in records:
                    label = str(record.get("violation", "unnamed"))
                    if "decision" in record:
                        violations = _promotion_decision_violations(
                            record.get("decision") or {}, contract)
                    elif "journal" in record:
                        violations = _promotion_journal_violations(
                            record.get("journal") or [], contract)
                    else:
                        violations = {"missing_record_shape"}
                    observed.update(violations)
                    if label not in violations:
                        mismatches.append(
                            f"{label}: expected to trigger {label}, "
                            f"got {sorted(violations)}"
                        )
                if mismatches:
                    _flag("promotion_evidence_contract_broken",
                          f"negative fixture {negative.get('id')} records do not "
                          f"trigger their labelled violations: {'; '.join(mismatches)}")
                require = list(
                    (negative.get("expect") or {}).get("require_violations", [])
                )
                missing_required = [v for v in require if v not in observed]
                if missing_required:
                    _flag("promotion_evidence_contract_broken",
                          f"negative fixture {negative.get('id')} missing required "
                          f"violation coverage: {missing_required}")

    # Source markers: the Rust enforcement points must stay present.
    for marker_key, entry in (contract.get("source_markers") or {}).items():
        marker_path = root / str(entry.get("path", ""))
        if not marker_path.is_file():
            _flag("promotion_evidence_contract_source_marker_missing",
                  f"{marker_key} source missing: {marker_path}")
            continue
        text = marker_path.read_text(errors="replace")
        missing = [p for p in entry.get("required_patterns", []) if p not in text]
        if missing:
            _flag("promotion_evidence_contract_source_marker_missing",
                  f"{marker_key} source missing enforcement markers: {missing}")
    return flags

