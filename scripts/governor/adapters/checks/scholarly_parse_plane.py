"""Fail-closed governance check for the staged scholarly parse plane (S19)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

import yaml

from governor.domain.trajectory import DriftFlag


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _requirements(contract: dict[str, Any]) -> str:
    anchor = str(contract.get("anchor", ""))
    return ", ".join(part.strip() for part in anchor.split(",") if part.strip())


def _check_negative_violation(label: str, record: dict[str, Any], contract: dict[str, Any]) -> bool:
    """Evaluate one mutation without scanning arbitrary source text."""
    value = record.get("record") or {}
    plane = next(
        (name for name, spec in (contract.get("planes") or {}).items()
         if label in (spec.get("violations") or []) or label in (contract.get("closed_taxonomy") or {}).get("violations", [])),
        "",
    )
    if label == "forbidden_dependency":
        return bool(value.get("forbidden_dependencies"))
    if label == "authoritative_redb":
        return value.get("authoritative_store") == "redb"
    if label == "memory_only_rvf":
        return value.get("rvf_role") == "authoritative_memory_only"
    if label == "hardcoded_physical_default":
        return bool(value.get("physical_defaults"))
    if label == "cache_fingerprint_collision":
        return value.get("collision_policy") == "allowed"
    if label == "missing_rollback":
        return value.get("rollback") is None
    if label == "auto_promote":
        return value.get("auto_promote") is True
    if label == "deployed":
        return value.get("deployed") is True
    if label == "promotion_receipt":
        return bool(value.get("promotion_receipt"))
    if label == "production_route":
        return bool(value.get("production_route"))
    if label == "graph_write":
        return value.get("graph_write") is True
    if label == "import_leak":
        return value.get("import_eligible") is True
    if label == "schema_vocabulary_drift":
        return value.get("disposition") not in set((contract.get("closed_taxonomy") or {}).get("dispositions", []))
    if label == "missing_integration_evidence":
        return not value.get("integration_evidence")
    if label in {"acceptance_evidence_missing", "acceptance_evidence_substitution", "acceptance_role_drift", "acceptance_same_n_drift", "acceptance_lock_erosion"}:
        return bool(value.get("acceptance_mutation"))
    if label == "ledger_surface":
        surface = str(value.get("surface") or "").strip().lower()
        return value.get("state") in {"proxy", "shadow"} and (
            not surface or surface == "none" or surface.startswith("none")
        )
    if label == "backend_identity":
        return "health_state" in value and (
            value.get("health_state") != "proxy" or value.get("version_state") != "proxy"
        )
    if label == "policy_stamp":
        if "closed_sources" not in value and "production_consult" not in value:
            return False
        sources = set(value.get("closed_sources") or [])
        return "none" not in sources or value.get("production_consult") != "parse-pdf"
    if label == "silent_loss":
        return value.get("verdict") == "PASS" and value.get("no_silent_loss") is not True
    if label == "lexical_visual_mix":
        kinds = {str(item) for item in (value.get("lexical_token_kinds") or [])}
        return bool(kinds & {"Image", "Formula", "image", "formula"})
    if label == "region_coverage":
        if value.get("status") not in {"pass", "success"}:
            return False
        return value.get("admitted_tokens") != value.get("assigned_tokens")
    if label == "header_sequence":
        receipts = [str(item) for item in (value.get("receipts") or [])]
        return receipts != ["Date", "Names", "Affiliations"] and bool(receipts)
    if label == "invented_author":
        return value.get("name_outcome") in {"empty", "malformed"} and int(value.get("authors") or 0) > 0
    if label == "bibliography_sequence":
        receipts = [str(item) for item in (value.get("receipts") or [])]
        return receipts != ["References", "CitationFields", "Anchors"] and bool(receipts)
    if label == "lost_references":
        return int(value.get("recognized") or 0) != int(value.get("entries") or 0) and str(value.get("status") or "") in {"accepted", "pass"}
    if label == "body_java_fallback":
        selectable = [str(item) for item in (value.get("independently_selectable") or [])]
        return selectable != ["header", "references"] and bool(selectable)
    if label == "reading_order_change":
        return value.get("body_overlap") is True and str(value.get("status") or "") in {"accepted", "pass"}
    if label == "typed_visual_absence":
        if "absent_signal_not_empty_success" not in value:
            return False
        return str(value.get("absent_signal_not_empty_success") or "") not in {"required", "true"}
    if label == "zero_count_from_absence":
        if "outcome" not in value and "receipts" not in value:
            return False
        return str(value.get("outcome") or "").lower() in {"figure", "table", "formula"} and int(value.get("receipts") or 0) == 0 and str(value.get("status") or "") in {"pass", "accepted"}
    if label == "family_promote_cap":
        if "non_candidate_families" not in value and "deployed" not in value:
            return False
        return int(value.get("non_candidate_families") or 0) > 1 or value.get("deployed") is True
    if label == "worker_identity_leak":
        fields = {str(item).lower() for item in (value.get("identity_fields") or [])}
        return bool(fields & {"worker", "workers", "smp", "concurrency", "thread"})
    return bool(plane) and bool(value)


def _check_scholarly_parse_plane(root: Path) -> list[DriftFlag]:
    """Validate the source-bound scholarly parse policy and its fixtures.

    This is intentionally evidence-oriented: the checker reads only declared
    paths and typed JSON fields, never broad substring-scans repository text.
    """
    policy_path = root / "scripts/governor/checks/scholarly_parse_plane.yaml"
    if not policy_path.is_file():
        return []
    try:
        contract = yaml.safe_load(policy_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        return [DriftFlag("scholarly_parse_plane_invalid", "high", f"scholarly_parse_plane: policy={policy_path}; expected=valid YAML; observed={error}", "scholarly_parse_plane")]

    dimension = str(contract.get("dimension", "scholarly_parse_plane"))
    severity = str(contract.get("severity", "high"))
    requirements = _requirements(contract)
    flags: list[DriftFlag] = []

    def flag(code: str, plane: str, evidence: Path | str, expected: Any, observed: Any) -> None:
        flags.append(DriftFlag(
            flag=f"scholarly_parse_plane_{code}",
            severity=severity,
            dimension=dimension,
            evidence=(f"plane={plane}; violation={code}; evidence={evidence}; "
                      f"requirements={requirements}; expected={expected!r}; observed={observed!r}"),
        ))

    bindings = list((contract.get("bindings") or {}).get("source_data", []))
    bindings += list((contract.get("bindings") or {}).get("canonical_artifacts", []))
    for binding in bindings:
        relative = str(binding.get("path", ""))
        path = root / relative
        expected = binding.get("sha256")
        if not path.is_file():
            flag("evidence_missing", "integration", relative, "tracked file", "missing")
        else:
            try:
                observed = _sha256(path)
            except OSError as error:
                flag("evidence_unreadable", "integration", relative, expected, str(error))
            else:
                if observed != expected:
                    flag("evidence_substitution", "integration", relative, expected, observed)

    # S20 acceptance is a separate typed surface: the manifest records
    # official identities and missingness, while the report records only the
    # bounded local proxy. Never infer execution from a catalogue row.
    manifest_path = root / "data/parse/acceptance_manifest.yaml"
    report_path = root / "artifacts/parse_compat/acceptance_evidence.json"
    try:
        manifest = yaml.safe_load(manifest_path.read_text(encoding="utf-8")) or {}
        acceptance = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, ValueError, yaml.YAMLError) as error:
        flag("acceptance_evidence_missing", "integration", "artifacts/parse_compat/acceptance_evidence.json", "manifest and report", str(error))
    else:
        classes = {"publisher_acceptance", "layout_omnidocbench"}
        manifest_classes = manifest.get("classes") or {}
        report_classes = acceptance.get("classes") or {}
        if manifest.get("schema_version") != "parse-acceptance.v1" or acceptance.get("schema_version") != "parse-acceptance-evidence.v1" or set(manifest_classes) != classes or set(report_classes) != classes:
            flag("acceptance_role_drift", "integration", report_path.relative_to(root), sorted(classes), {"manifest": sorted(manifest_classes), "report": sorted(report_classes)})
        for name in sorted(classes):
            spec = manifest_classes.get(name) or {}
            observed = report_classes.get(name) or {}
            rows = [row for row in spec.get("rows", []) if isinstance(row, dict)]
            proxy_rows = [row for row in (spec.get("local_proxy") or {}).get("rows", []) if isinstance(row, dict)]
            expected = {"bound_n": sum(row.get("state") == "bound" for row in rows), "catalogued_n": sum(row.get("state") == "catalogued" for row in rows), "bytes_absent_n": sum(row.get("state") == "bytes_absent" for row in rows)}
            actual = {key: observed.get(key) for key in expected}
            expected_execution = len(proxy_rows)
            if actual != expected or observed.get("same_n") is not True or observed.get("executed_n") != expected_execution:
                flag("acceptance_same_n_drift", name, report_path.relative_to(root), {**expected, "same_n": True, "executed_n": expected_execution}, {**actual, "same_n": observed.get("same_n"), "executed_n": observed.get("executed_n")})
            if observed.get("comparison_kind") != spec.get("comparison_kind") or (name == "layout_omnidocbench" and acceptance.get("conservation", {}).get("tei_oracle") not in {None, "java_grobid_compatibility"}):
                flag("acceptance_role_drift", name, report_path.relative_to(root), spec.get("comparison_kind"), observed.get("comparison_kind"))
        if acceptance.get("manifest_id") != manifest.get("manifest_id") or acceptance.get("locks", {}).get("import_eligible") is not False or acceptance.get("locks", {}).get("auto_promote") != "forbidden" or acceptance.get("locks", {}).get("production_route") != "forbidden":
            flag("acceptance_lock_erosion", "promotion_quarantine", report_path.relative_to(root), "candidate-only locks", acceptance.get("locks"))
        conservation = acceptance.get("conservation") or {}
        if acceptance.get("verdict") == "PASS" and conservation.get("no_silent_loss") is not True:
            flag("silent_loss", "integration", report_path.relative_to(root), {"verdict": "PASS", "no_silent_loss": True}, {"verdict": acceptance.get("verdict"), "no_silent_loss": conservation.get("no_silent_loss")})

    registry_path = root / "data/parse/grobid_compatibility.yaml"
    try:
        registry = yaml.safe_load(registry_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("backend_identity", "integration", "data/parse/grobid_compatibility.yaml", "valid YAML", str(error))
        registry = {}
    caps = [c for c in (registry.get("capabilities") or []) if isinstance(c, dict)]
    by_id = {str(c.get("id")): c for c in caps}
    for cap_id in ("rest.health", "rest.version"):
        cap = by_id.get(cap_id) or {}
        impl = cap.get("implementation") or {}
        surface = str(impl.get("surface") or "")
        if cap.get("state") != "proxy" or surface in {"", "none"}:
            flag("backend_identity", "integration", f"data/parse/grobid_compatibility.yaml#{cap_id}", {"state": "proxy", "surface": "http.rs"}, {"state": cap.get("state"), "surface": surface})
    for cap in caps:
        state = str(cap.get("state") or "")
        surface = str((cap.get("implementation") or {}).get("surface") or "").strip()
        surface_l = surface.lower()
        if state in {"proxy", "shadow"} and (
            not surface or surface_l == "none" or surface_l.startswith("none")
        ):
            flag("ledger_surface", "integration", f"data/parse/grobid_compatibility.yaml#{cap.get('id')}", {"state": state, "surface": "crate path"}, {"state": state, "surface": surface})
    header_ids = (
        "rest.processDate",
        "rest.processHeaderNames",
        "rest.processAffiliations",
        "stage.header_composition",
    )
    expected_surfaces = {
        "rest.processDate": "crates/da-parse/src/native_date.rs",
        "rest.processHeaderNames": "crates/da-parse/src/native_personal_name.rs",
        "rest.processAffiliations": "crates/da-parse/src/native_affiliation.rs",
        "stage.header_composition": "crates/da-parse/src/native_composition.rs",
    }
    for cap_id in header_ids:
        cap = by_id.get(cap_id) or {}
        impl = cap.get("implementation") or {}
        surface = str(impl.get("surface") or "").strip()
        if cap.get("state") != "shadow" or not surface.startswith("crates/da-parse/src/"):
            flag(
                "header_sequence",
                "integration",
                f"data/parse/grobid_compatibility.yaml#{cap_id}",
                {"state": "shadow", "surface": expected_surfaces.get(cap_id)},
                {"state": cap.get("state"), "surface": surface},
            )
        budget = cap.get("invention_budget")
        if cap_id == "rest.processHeaderNames" and budget not in {0, "0", None}:
            flag(
                "invented_author",
                "integration",
                f"data/parse/grobid_compatibility.yaml#{cap_id}",
                {"invention_budget": 0},
                {"invention_budget": budget},
            )

    bibliography_ids = (
        "rest.processCitationNames",
        "rest.processCitation",
        "rest.processCitationListTEI",
        "stage.reference_segmentation",
    )
    expected_biblio = {
        "rest.processCitationNames": "crates/da-parse/src/native_personal_name.rs",
        "rest.processCitation": "crates/da-parse/src/native_citation.rs",
        "rest.processCitationListTEI": "crates/da-parse/src/native_citation.rs",
        "stage.reference_segmentation": "crates/da-parse/src/native_composition.rs",
    }
    for cap_id in bibliography_ids:
        cap = by_id.get(cap_id) or {}
        impl = cap.get("implementation") or {}
        surface = str(impl.get("surface") or "").strip()
        if cap.get("state") != "shadow" or not surface.startswith("crates/da-parse/src/"):
            flag(
                "bibliography_sequence",
                "integration",
                f"data/parse/grobid_compatibility.yaml#{cap_id}",
                {"state": "shadow", "surface": expected_biblio.get(cap_id)},
                {"state": cap.get("state"), "surface": surface},
            )
    consolidation = by_id.get("stage.bibliographic_consolidation") or {}
    cons_surface = str((consolidation.get("implementation") or {}).get("surface") or "")
    if consolidation.get("state") not in {"fallback", "proxy"} or "profile.rs" not in cons_surface:
        flag(
            "bibliography_sequence",
            "integration",
            "data/parse/grobid_compatibility.yaml#stage.bibliographic_consolidation",
            {"state": "fallback", "surface": "profile.rs"},
            {"state": consolidation.get("state"), "surface": cons_surface},
        )

    fulltext_path = root / "data/parse/native_fulltext_composition.yaml"
    try:
        fulltext = yaml.safe_load(fulltext_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("body_java_fallback", "integration", "data/parse/native_fulltext_composition.yaml", "valid YAML", str(error))
        fulltext = {}
    admission = fulltext.get("admission") or {}
    selectable = [str(item) for item in (admission.get("independently_selectable") or [])]
    if selectable != ["header", "references"]:
        flag(
            "body_java_fallback",
            "integration",
            "data/parse/native_fulltext_composition.yaml#admission.independently_selectable",
            ["header", "references"],
            selectable,
        )
    matrix = admission.get("current_matrix") or {}
    for family in ("document_segmentation", "figure", "table", "formula"):
        if str(matrix.get(family) or "") == "passed":
            flag(
                "body_java_fallback",
                "integration",
                f"data/parse/native_fulltext_composition.yaml#admission.current_matrix.{family}",
                "fallback",
                matrix.get(family),
            )

    visual = by_id.get("stage.figure_table_formula") or {}
    visual_surface = str((visual.get("implementation") or {}).get("surface") or "").strip()
    if visual.get("state") != "shadow" or "native_visual.rs" not in visual_surface:
        flag(
            "typed_visual_absence",
            "integration",
            "data/parse/grobid_compatibility.yaml#stage.figure_table_formula",
            {"state": "shadow", "surface": "crates/da-parse/src/native_visual.rs"},
            {"state": visual.get("state"), "surface": visual_surface},
        )
    visual_heads_path = root / "data/parse/native_visual_heads.yaml"
    try:
        visual_heads = yaml.safe_load(visual_heads_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("typed_visual_absence", "integration", "data/parse/native_visual_heads.yaml", "valid YAML", str(error))
        visual_heads = {}
    replay = visual_heads.get("replay") or {}
    if replay.get("absent_signal_not_empty_success") not in {True, "required", "true"}:
        flag(
            "typed_visual_absence",
            "integration",
            "data/parse/native_visual_heads.yaml#replay.absent_signal_not_empty_success",
            "required",
            replay.get("absent_signal_not_empty_success"),
        )
    if str((visual_heads.get("gates") or {}).get("fulltext_composition") or "") not in {"forbidden", "false"}:
        flag(
            "zero_count_from_absence",
            "integration",
            "data/parse/native_visual_heads.yaml#gates.fulltext_composition",
            "forbidden",
            (visual_heads.get("gates") or {}).get("fulltext_composition"),
        )

    promotion = (registry.get("defaults") or {}).get("promotion") or {}
    non_candidate = [
        cap.get("id")
        for cap in caps
        if str(cap.get("promotion_state") or "candidate") not in {"candidate", ""}
    ]
    deployed_caps = [
        cap.get("id")
        for cap in caps
        if cap.get("deployed") is True or str(cap.get("state") or "") == "deployed"
    ]
    wave = promotion.get("this_wave_promotes")
    wave_ok = wave in {None, "none"} or wave == []
    if (
        int(promotion.get("max_non_candidate_families") or 1) < 1
        or promotion.get("deployed") is True
        or promotion.get("auto_promote") not in {None, "forbidden", False}
        or not wave_ok
        or len(non_candidate) > 1
        or deployed_caps
    ):
        flag(
            "family_promote_cap",
            "promotion_quarantine",
            "data/parse/grobid_compatibility.yaml#defaults.promotion",
            {"max_non_candidate_families": 1, "this_wave_promotes": "none", "deployed": False},
            {
                "non_candidate": non_candidate,
                "deployed_caps": deployed_caps,
                "promotion": promotion,
            },
        )
    envelope_path = root / "crates/da-parse/src/envelope.rs"
    try:
        envelope = envelope_path.read_text(encoding="utf-8")
    except OSError:
        envelope = ""
    if envelope:
        identity_block = envelope.split("pub struct TeiIdentity", 1)[-1].split("impl TeiIdentity", 1)[0]
        leaked = [token for token in ("worker", "workers", "smp", "concurrency", "thread") if token in identity_block.lower()]
        if leaked:
            flag(
                "worker_identity_leak",
                "cache_identity",
                "crates/da-parse/src/envelope.rs#TeiIdentity",
                "no worker/smp fields",
                leaked,
            )

    stamp = ((registry.get("defaults") or {}).get("policy_stamp") or {})
    sources = set(stamp.get("closed_sources") or [])
    if "none" not in sources or "wasm" not in sources or "native_fallback" not in sources or stamp.get("production_consult") != "parse-pdf":
        flag("policy_stamp", "integration", "data/parse/grobid_compatibility.yaml#defaults.policy_stamp", {"closed_sources": ["wasm", "native_fallback", "none"], "production_consult": "parse-pdf"}, stamp)

    segmentation_path = root / "data/parse/document_segmentation.yaml"
    try:
        segmentation = yaml.safe_load(segmentation_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("lexical_visual_mix", "integration", "data/parse/document_segmentation.yaml", "valid YAML", str(error))
        segmentation = {}
    lexical_kinds = {
        str(item) for item in ((segmentation.get("regions") or {}).get("lexical_token_kinds") or [])
    }
    visual_in_lexical = lexical_kinds & {"Image", "Formula", "image", "formula"}
    if visual_in_lexical or not lexical_kinds:
        flag(
            "lexical_visual_mix",
            "integration",
            "data/parse/document_segmentation.yaml#regions.lexical_token_kinds",
            {"lexical_token_kinds": ["Word", "Whitespace", "Punctuation", "Symbol"]},
            {"lexical_token_kinds": sorted(lexical_kinds), "visual_mix": sorted(visual_in_lexical)},
        )

    shadow_path = root / "artifacts/parse_compat/document_segmentation_shadow.json"
    try:
        shadow = json.loads(shadow_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        # Typed absence: no successful outcome to audit. Unsupported lanes stay silent.
        shadow = {}
    for outcome in shadow.get("outcomes") or []:
        if not isinstance(outcome, dict):
            continue
        status = str(outcome.get("status") or "")
        if status not in {"pass", "success"}:
            continue
        admitted = outcome.get("admitted_tokens")
        assigned = outcome.get("assigned_tokens")
        if admitted != assigned:
            flag(
                "region_coverage",
                "integration",
                f"artifacts/parse_compat/document_segmentation_shadow.json#{outcome.get('document_id')}:{outcome.get('lane')}",
                {"assigned_tokens": admitted},
                {"status": status, "admitted_tokens": admitted, "assigned_tokens": assigned},
            )

    shared = root / str((contract.get("bindings") or {}).get("shared_promotion_contract", ""))
    if not shared.is_file():
        flag("promotion_contract_missing", "promotion_quarantine", shared.relative_to(root) if shared.is_absolute() and shared.is_relative_to(root) else shared, "shared promotion contract", "missing")

    fixtures = contract.get("fixtures") or {}
    positive_spec = fixtures.get("positive") or {}
    positive_path = root / str(positive_spec.get("path", ""))
    try:
        positive = json.loads(positive_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        flag("fixture_invalid", "integration", positive_spec.get("path", ""), "JSON object", str(error))
        positive = None
    if isinstance(positive, dict):
        if positive.get("schema_version") != contract.get("schema_version"):
            flag("schema_version_drift", "integration", positive_path.relative_to(root), contract.get("schema_version"), positive.get("schema_version"))
        if positive.get("policy_id") != contract.get("policy_id"):
            flag("policy_identity_drift", "integration", positive_path.relative_to(root), contract.get("policy_id"), positive.get("policy_id"))
        expected_planes = set(contract.get("planes", {}))
        observed_planes = set(positive.get("planes") or {})
        if observed_planes != expected_planes:
            flag("plane_coverage_missing", "integration", positive_path.relative_to(root), sorted(expected_planes), sorted(observed_planes))
        promotion = positive.get("promotion") or {}
        locks = contract.get("locks") or {}
        for field, expected in locks.items():
            if field not in {"import_eligible", "production_route", "graph_write", "auto_promote", "deployed", "promotion_receipt"}:
                continue
            observed_key = "receipt" if field == "promotion_receipt" else field
            if promotion.get(observed_key) != expected:
                flag(field, "promotion_quarantine", positive_path.relative_to(root), expected, promotion.get(observed_key))
        if (positive.get("rollback") or {}).get("target") != (contract.get("bindings") or {}).get("rollback_target"):
            flag("missing_rollback", "promotion_quarantine", positive_path.relative_to(root), (contract.get("bindings") or {}).get("rollback_target"), (positive.get("rollback") or {}).get("target"))
        required = {(item.get("path"), item.get("identity")) for item in (contract.get("bindings") or {}).get("integration_evidence", [])}
        observed = {(item.get("path"), item.get("identity")) for item in positive.get("integration_evidence", []) if isinstance(item, dict)}
        if not required.issubset(observed):
            flag("missing_integration_evidence", "integration", positive_path.relative_to(root), sorted(required), sorted(observed))

    negative_spec = fixtures.get("negative") or {}
    negative_path = root / str(negative_spec.get("path", ""))
    try:
        records = json.loads(negative_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        flag("fixture_invalid", "integration", negative_spec.get("path", ""), "JSON array", str(error))
        records = None
    if isinstance(records, list):
        expected_labels = set((negative_spec.get("expect") or {}).get("require_violations", []))
        observed_labels = {r.get("violation") for r in records if isinstance(r, dict)}
        for label in sorted(expected_labels - observed_labels):
            flag(label, "integration", negative_path.relative_to(root), "label present", "missing")
        for record in records:
            if not isinstance(record, dict):
                flag("fixture_invalid", "integration", negative_path.relative_to(root), "typed record", record)
                continue
            label = str(record.get("violation", ""))
            if label and not _check_negative_violation(label, record, contract):
                flag(label, str(record.get("plane", "unknown")), negative_path.relative_to(root), "isolated mutation triggers violation", record.get("record"))
    return flags
