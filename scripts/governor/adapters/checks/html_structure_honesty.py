"""html_structure_honesty contract check — extracted from trajectory_service (Governor v2)."""
from governor.domain.trajectory import DriftFlag


def _make_flag(flag, evidence, severity="high", dimension="unknown"):
    return DriftFlag(flag=flag, severity=severity, evidence=evidence, dimension=dimension)


def _check_html_structure_honesty(root) -> list[DriftFlag]:
    """Evaluate the data-driven HTML structure honesty contract (PIR-009/PIR-010).

    Reads `scripts/governor/checks/html_structure_honesty.yaml` (DATA).
    The positive fixture is a DocumentContextProjection rendered by
    `da_application::DocumentContextStage` from HTML sections that carry
    the full derived identity: paths must be unique, every section must
    have a `path_id`, each declared parent must be the dotted parent of
    that path (dot-free `~N` restart suffixes never become dotted
    parents), and prev/next chains must follow document order. The
    negative fixture models the fail-closed outcome for flat HTML:
    ok=false, readiness stays `not_claimed`/`blocked`, a typed reason
    names the missing identity, and `import_eligible` stays false (D127).
    The Rust tests
    `document_context_stage_fixtures_match_committed_governor_data` and
    `document_context_stage_negative_fixture_models_blocked_html_readiness`
    pin the fixtures to the current stage behavior, so a broken contract
    cannot silently survive a regenerate. The readiness surface
    (`article_load.rs`) must keep stamping HTML/textbook sources as
    `graph_readiness = "not_claimed"`.
    """
    path = root / "scripts/governor/checks/html_structure_honesty.yaml"
    if not path.exists():
        return []
    try:
        contract = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag(
            flag="html_structure_honesty_invalid",
            severity="high",
            evidence=f"cannot load {path}: {error}",
            dimension="html_structure_ast",
        )]
    flags: list[DriftFlag] = []
    dimension = str(contract.get("dimension", "html_structure_ast"))
    severity = str(contract.get("severity", "high"))

    def _flag(name: str, evidence: str) -> None:
        flags.append(DriftFlag(
            flag=name, severity=severity, evidence=evidence, dimension=dimension,
        ))

    report_contract = contract.get("report_contract") or {}
    required_fields = list(report_contract.get("required_fields", []))
    section_fields = list(report_contract.get("section_required_fields", []))

    for polarity in ("positive", "negative"):
        fixture = (contract.get("fixtures") or {}).get(polarity) or {}
        fixture_path = root / str(fixture.get("path", ""))
        if not fixture_path.is_file():
            _flag("html_structure_honesty_fixture_missing",
                  f"{polarity} fixture missing: {fixture_path}")
            continue
        try:
            payload = json.loads(fixture_path.read_text(encoding="utf-8"))
        except (ValueError, OSError) as error:
            _flag("html_structure_honesty_fixture_invalid",
                  f"{polarity} fixture unreadable: {error}")
            continue
        if polarity == "positive":
            problems = _validate_html_projection(payload, required_fields, section_fields)
        else:
            problems = _validate_html_blocked_record(payload)
        if problems:
            _flag("html_structure_honesty_broken",
                  f"{polarity} fixture {fixture.get('id')} violates the contract: "
                  f"; ".join(problems))

    markers = contract.get("readiness_markers") or {}
    markers_path = root / str(markers.get("path", ""))
    if not markers_path.is_file():
        _flag("html_structure_honesty_readiness_marker_missing",
              f"readiness surface missing: {markers_path}")
    else:
        text = markers_path.read_text(errors="replace")
        missing = [m for m in markers.get("required_patterns", []) if m not in text]
        if missing:
            _flag("html_structure_honesty_readiness_marker_missing",
                  f"readiness surface missing honesty markers: {missing}")
    return flags

