"""Fail-closed governance check for the staged scholarly parse plane (S19)."""

from __future__ import annotations

import hashlib
import json
import re
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
    if label in {"acceptance_evidence_missing", "acceptance_evidence_substitution", "acceptance_role_drift", "acceptance_lock_erosion"}:
        return bool(value.get("acceptance_mutation"))
    if label == "acceptance_same_n_drift":
        if value.get("acceptance_mutation"):
            return True
        if (
            "documents" not in value
            and "unique_source_sha256" not in value
            and "identity_kind" not in value
            and "java_header_authors_equals" not in value
            and "citation_names_not_biblstruct" not in value
            and "pooling_accepted_with_raw" not in value
        ):
            return False
        documents = int(value.get("documents") or 0)
        unique = int(value.get("unique_source_sha256") or 0)
        kind = value.get("identity_kind")
        if documents > 1 and unique == 1 and kind != "synthetic_clone":
            return True
        if kind == "preserved_three_sha" and unique != 3:
            return True
        if value.get("java_header_authors_equals") == "header_accepted_total":
            return True
        if value.get("citation_names_not_biblstruct") is False:
            return True
        if value.get("pooling_accepted_with_raw") == "allowed":
            return True
        if value.get("pooling_citation_names_with_references") == "allowed":
            return True
        return False
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
        if (
            "closed_sources" not in value
            and "production_consult" not in value
            and "profile_route_default" not in value
            and "bare_from_env_parse" not in value
            and "parse_pdf_consults" not in value
            and "empty_native_unknown_point" not in value
        ):
            return False
        if value.get("bare_from_env_parse") is True:
            return True
        sources = set(value.get("closed_sources") or [])
        if "profile_route_default" in value and value.get("profile_route_default") != "structural":
            return True
        consults = [str(item) for item in (value.get("parse_pdf_consults") or [])]
        empty_native = [str(item) for item in (value.get("empty_native_unknown_point") or [])]
        if set(consults) & set(empty_native):
            return True
        if "parse_pdf_consults" in value and consults != ["grammar_route", "profile_route"]:
            return True
        if "empty_native_unknown_point" in value and empty_native != [
            "concurrency_tune",
            "retry_policy",
            "summary_depth",
        ]:
            return True
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


_FROM_ENV = re.compile(r"GrobidParser::from_env\s*\(\s*\)")


def _from_env_role(lines: list[str], index: int, relative: str) -> str:
    """Classify one GrobidParser::from_env() site. Missing roots are skipped by the caller."""
    stripped = lines[index].strip()
    if stripped.startswith(("//", "///", "//!", "*")):
        return "comment"
    window = "\n".join(lines[max(0, index - 2) : min(len(lines), index + 8)])
    prev = "\n".join(lines[max(0, index - 8) : index + 4])
    if "fn corpus_grobid_parser" in prev:
        return "consult_helper"
    if ".with_profile" in window:
        return "with_profile"
    if any(token in window for token in ("is_alive", "probe_health", "probe_version", "GROBID:")):
        return "health_probe"
    if "mod tests" in "\n".join(lines[: index + 1]) or relative.endswith("/tests.rs") or "/tests/" in relative:
        return "test"
    if "parse_pdf" in window:
        return "PARSE_BARE"
    return "PARSE_BARE"


def _scan_bare_from_env_parse(root: Path, scan_roots: list[str]) -> list[str]:
    """Return PARSE_BARE sites under declared roots. Missing roots are skipped."""
    hits: list[str] = []
    for relative in scan_roots:
        path = root / relative
        if not path.exists():
            continue
        files = [path] if path.is_file() else sorted(path.rglob("*.rs"))
        for file_path in files:
            try:
                lines = file_path.read_text(encoding="utf-8").splitlines()
            except OSError:
                continue
            rel = str(file_path.relative_to(root))
            for index, line in enumerate(lines):
                if not _FROM_ENV.search(line):
                    continue
                if _from_env_role(lines, index, rel) == "PARSE_BARE":
                    hits.append(f"{rel}:{index + 1}")
    return hits


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
    lift = ((fulltext.get("source_evidence") or {}).get("s12_named_denominators") or {}).get("java_lift_same_n") or {}
    if (
        lift.get("java_header_authors_equals") != "header_raw_total"
        or lift.get("java_biblstruct_equals") != "references_total"
        or lift.get("hub_native_accepted_header") not in {20, "20"}
        or lift.get("hub_java_header_raw") not in {21, "21"}
        or lift.get("citation_names_not_biblstruct") is not True
        or lift.get("pooling_accepted_with_raw") != "forbidden"
        or lift.get("pooling_citation_names_with_references") != "forbidden"
    ):
        flag(
            "acceptance_same_n_drift",
            "integration",
            "data/parse/native_fulltext_composition.yaml#source_evidence.s12_named_denominators.java_lift_same_n",
            {
                "java_header_authors_equals": "header_raw_total",
                "java_biblstruct_equals": "references_total",
                "hub_native_accepted_header": 20,
                "hub_java_header_raw": 21,
                "citation_names_not_biblstruct": True,
                "pooling_accepted_with_raw": "forbidden",
                "pooling_citation_names_with_references": "forbidden",
            },
            lift,
        )
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

    composition_shadow_path = root / "artifacts/parse_compat/native_fulltext_composition_shadow.json"
    try:
        composition_shadow = json.loads(composition_shadow_path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        composition_shadow = {}
    if composition_shadow:
        identity = composition_shadow.get("source_identity") or {}
        outcomes = [row for row in (composition_shadow.get("outcomes") or []) if isinstance(row, dict)]
        shas = {str(row.get("source_sha256") or "") for row in outcomes}
        shas.discard("")
        if not shas and identity.get("source_sha256"):
            shas = {str(identity.get("source_sha256"))}
        documents = int((composition_shadow.get("denominators") or {}).get("documents") or 0)
        unique = len(shas)
        kind = identity.get("identity_kind")
        if documents > 1 and unique == 1 and kind != "synthetic_clone":
            flag(
                "acceptance_same_n_drift",
                "integration",
                "artifacts/parse_compat/native_fulltext_composition_shadow.json#source_identity",
                {"identity_kind": "synthetic_clone", "unique_source_sha256": 1},
                {"identity_kind": kind, "unique_source_sha256": unique, "documents": documents},
            )
        preserved = {
            "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
            "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
            "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
        }
        ids = " ".join(str(row.get("document_id") or "") for row in outcomes).lower()
        looks_preserved = bool(shas & preserved) or any(token in ids for token in ("hub", "rlm", "reca", "2507.19457", "2512.24601", "2605.26525"))
        if looks_preserved and kind != "preserved_three_sha":
            flag(
                "acceptance_same_n_drift",
                "integration",
                "artifacts/parse_compat/native_fulltext_composition_shadow.json#identity_kind",
                "preserved_three_sha",
                kind,
            )
        if kind == "preserved_three_sha" and unique != 3:
            flag(
                "acceptance_same_n_drift",
                "integration",
                "artifacts/parse_compat/native_fulltext_composition_shadow.json#unique_source_sha256",
                3,
                unique,
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

    policy_reg_path = root / "data/policies/registry.yaml"
    try:
        policy_reg = yaml.safe_load(policy_reg_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("policy_stamp", "integration", "data/policies/registry.yaml", "valid YAML", str(error))
        policy_reg = {}
    profile_native = ((policy_reg.get("decision_points") or {}).get("profile_route") or {}).get("native") or {}
    if profile_native.get("default") != "structural":
        flag(
            "policy_stamp",
            "integration",
            "data/policies/registry.yaml#decision_points.profile_route.native.default",
            "structural",
            profile_native.get("default"),
        )
    consult = policy_reg.get("consult_status") or {}
    parse_pdf = [str(item) for item in (consult.get("parse_pdf_consults") or [])]
    empty_native = [str(item) for item in (consult.get("empty_native_unknown_point") or [])]
    not_consulted = [str(item) for item in (consult.get("native_rules_not_consulted") or [])]
    later = [str(item) for item in (consult.get("later") or [])]
    expected_parse = ["grammar_route", "profile_route"]
    expected_empty = ["concurrency_tune", "retry_policy", "summary_depth"]
    expected_not = ["batch_admission"]
    expected_later = ["head_fallback", "visual_bundle", "critic_sample"]
    if (
        parse_pdf != expected_parse
        or empty_native != expected_empty
        or not_consulted != expected_not
        or later != expected_later
        or set(parse_pdf) & set(empty_native)
    ):
        flag(
            "policy_stamp",
            "integration",
            "data/policies/registry.yaml#consult_status",
            {
                "parse_pdf_consults": expected_parse,
                "empty_native_unknown_point": expected_empty,
                "native_rules_not_consulted": expected_not,
                "later": expected_later,
            },
            consult,
        )
    stamp_consults = [str(item) for item in (stamp.get("parse_pdf_consults") or [])]
    stamp_empty = [str(item) for item in (stamp.get("empty_native_unknown_point") or [])]
    if stamp_consults != expected_parse or stamp_empty != expected_empty:
        flag(
            "policy_stamp",
            "integration",
            "data/parse/grobid_compatibility.yaml#defaults.policy_stamp.parse_pdf_consults",
            {"parse_pdf_consults": expected_parse, "empty_native_unknown_point": expected_empty},
            {"parse_pdf_consults": stamp_consults, "empty_native_unknown_point": stamp_empty},
        )

    sona_path = root / "crates/da-cli/src/background_tasks.rs"
    if sona_path.is_file():
        try:
            sona_src = sona_path.read_text(encoding="utf-8")
        except OSError as error:
            sona_src = str(error)
        if (
            "demo=true" not in sona_src
            or "empty_native_unknown_point" not in sona_src
            or "refuses before consult" not in sona_src
        ):
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/background_tasks.rs#sona_policy_report",
                "demo=true and refuse empty_native_unknown_point before consult",
                "missing demo stamp or empty-native refuse",
            )

    stages = (registry.get("defaults") or {}).get("cascade_stages") or {}
    expected_ids = [f"P{i}" for i in range(10)]
    if list(stages.keys()) != expected_ids:
        flag(
            "policy_stamp",
            "integration",
            "data/parse/grobid_compatibility.yaml#defaults.cascade_stages",
            expected_ids,
            list(stages.keys()),
        )
    allowed_fsm = set(expected_parse) | {"none"} | set(expected_later)
    for sid in expected_ids:
        row = stages.get(sid) or {}
        authority = str(row.get("production_authority") or "")
        consults = row.get("fsm_consult") or []
        if isinstance(consults, str):
            consults = [consults]
        consults = [str(item) for item in consults]
        if authority == "native":
            flag(
                "policy_stamp",
                "integration",
                f"data/parse/grobid_compatibility.yaml#defaults.cascade_stages.{sid}.production_authority",
                "java_http or java_profile or tei_lift",
                authority,
            )
        if set(consults) & set(expected_empty):
            flag(
                "policy_stamp",
                "integration",
                f"data/parse/grobid_compatibility.yaml#defaults.cascade_stages.{sid}.fsm_consult",
                "subset of parse_pdf_consults or later or none",
                consults,
            )
        if consults and any(item not in allowed_fsm for item in consults):
            flag(
                "policy_stamp",
                "integration",
                f"data/parse/grobid_compatibility.yaml#defaults.cascade_stages.{sid}.fsm_consult",
                sorted(allowed_fsm),
                consults,
            )
    p0 = stages.get("P0") or {}
    p0_consults = p0.get("fsm_consult") or []
    if isinstance(p0_consults, str):
        p0_consults = [p0_consults]
    if [str(item) for item in p0_consults] != expected_parse:
        flag(
            "policy_stamp",
            "integration",
            "data/parse/grobid_compatibility.yaml#defaults.cascade_stages.P0.fsm_consult",
            expected_parse,
            p0_consults,
        )
    p3 = stages.get("P3") or {}
    if str(p3.get("observe_mode") or "") != "shadow_receipt" or str(p3.get("production_authority") or "") == "native":
        flag(
            "policy_stamp",
            "integration",
            "data/parse/grobid_compatibility.yaml#defaults.cascade_stages.P3",
            {"observe_mode": "shadow_receipt", "production_authority": "java_http"},
            p3,
        )

    source_cli = root / "crates/da-cli/src/source_cli.rs"
    if source_cli.is_file():
        try:
            src = source_cli.read_text(encoding="utf-8")
        except OSError as error:
            src = str(error)
        if "stamp_p3_header_observe" not in src:
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/source_cli.rs#stamp_p3_header_observe",
                "parse-pdf stamps P3 observe from Java header_n",
                "missing stamp_p3_header_observe",
            )
        if "compose_header(" in src or "run_header_sequence(" in src:
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/source_cli.rs#parse-pdf",
                "P3 observe must not call native compose",
                "compose_header or run_header_sequence call",
            )

    constructors = (registry.get("defaults") or {}).get("production_pdf_constructors") or {}
    scan_roots = [str(item) for item in (constructors.get("scan_roots") or [])]
    allowed = set(constructors.get("allowed_from_env_roles") or [])
    if constructors:
        if "health_probe" not in allowed or constructors.get("forbidden_role") != "PARSE_BARE":
            flag(
                "policy_stamp",
                "integration",
                "data/parse/grobid_compatibility.yaml#defaults.production_pdf_constructors",
                {"allowed_from_env_roles": ["health_probe", "with_profile", "consult_helper", "test", "comment"], "forbidden_role": "PARSE_BARE"},
                constructors,
            )
        bare = _scan_bare_from_env_parse(root, scan_roots)
        if bare:
            flag(
                "policy_stamp",
                "integration",
                "GrobidParser::from_env PDF parse constructor",
                "consult_corpus_grobid_parser or with_profile",
                bare,
            )

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
