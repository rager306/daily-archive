"""section_navigation_contract contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_section_navigation_contract(root) -> list[DriftFlag]:
    """Evaluate the data-driven section navigation report contract (PIR-006).

    Reads `scripts/governor/checks/section_navigation_contract.yaml` (DATA)
    and validates the committed NavigationReport fixtures: the positive
    complete-tree fixture must be `ok: true` with zero diagnostics; the
    negative broken-tree fixture must be `ok: false` and report every
    required NAV-* code. The graph-task template expressing the same
    contract must also stay present. The Rust test
    `navigation_report_fixtures_match_committed_governor_data` pins the
    fixtures to the current validator output, so a broken contract cannot
    silently survive a regenerate.
    """
    path = root / "scripts/governor/checks/section_navigation_contract.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="section_navigation_contract_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="temporal_ast_navigation",
        )]
    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "temporal_ast_navigation"))
    severity = str(contract.get("severity", "high"))
    required_fields = list((contract.get("report_contract") or {}).get("required_fields", []))

    def _flag(name: str, evidence: str) -> None:
        flags.append(DriftFlag(
            flag=name, severity=severity, evidence=evidence, dimension=dimension,
        ))

    for polarity in ("positive", "negative"):
        fixture = (contract.get("fixtures") or {}).get(polarity) or {}
        fixture_path = root / str(fixture.get("path", ""))
        if not fixture_path.is_file():
            _flag("section_navigation_contract_fixture_missing",
                  f"{polarity} fixture missing: {fixture_path}")
            continue
        try:
            report = json.loads(fixture_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag("section_navigation_contract_fixture_invalid",
                  f"{polarity} fixture unreadable: {error}")
            continue
        missing = [f for f in required_fields if f not in report]
        if missing:
            _flag("section_navigation_contract_broken",
                  f"{polarity} fixture {fixture.get('id')} missing required fields: {missing}")
            continue
        expect = fixture.get("expect") or {}
        if polarity == "positive":
            if report.get("ok") is not True:
                _flag("section_navigation_contract_broken",
                      f"positive complete-tree fixture must be ok=true")
            if report.get("diagnostic_count") != 0:
                _flag("section_navigation_contract_broken",
                      f"positive complete-tree fixture must have zero diagnostics, "
                      f"got diagnostic_count={report.get('diagnostic_count')}")
        else:
            if report.get("ok") is not False:
                _flag("section_navigation_contract_broken",
                      f"negative broken-tree fixture must be ok=false")
            min_diags = int(expect.get("diagnostic_count_min", 1) or 1)
            if int(report.get("diagnostic_count") or 0) < min_diags:
                _flag("section_navigation_contract_broken",
                      f"negative broken-tree fixture must report at least {min_diags} "
                      f"diagnostic(s), got {report.get('diagnostic_count')}")
            counts = report.get("counts") or {}
            missing_codes = [
                code for code in expect.get("require_codes", [])
                if int(counts.get(code, 0)) <= 0
            ]
            if missing_codes:
                _flag("section_navigation_contract_broken",
                      f"negative broken-tree fixture missing required diagnostic codes: "
                      f"{missing_codes}")

    suite = contract.get("graph_task_suite") or {}
    suite_path = root / str(suite.get("path", ""))
    if not suite_path.is_file():
        _flag("section_navigation_contract_suite_missing",
              f"navigation graph task suite missing: {suite_path}")
    else:
        text = suite_path.read_text(errors="replace")
        missing = [p for p in suite.get("required_patterns", []) if p not in text]
        if missing:
            _flag("section_navigation_contract_suite_broken",
                  f"navigation graph task suite missing required markers: {missing}")
    return flags

