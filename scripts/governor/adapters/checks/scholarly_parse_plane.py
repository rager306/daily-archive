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
            and "citation_names_total" not in value
            and "pooling_citation_names_with_references" not in value
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
        # M495/S01: citation_names_total=2198 — отдельный именованный
        # знаменатель. Подмена на 207 (references_total / biblStruct) — пул,
        # не отдельный флаг, и даёт тот же acceptance_same_n_drift.
        if "citation_names_total" in value and value.get("citation_names_total") not in {2198, "2198"}:
            return True
        if "references_total" in value and value.get("references_total") not in {207, "207"}:
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
_HEALTH_TOKENS = ("is_alive", "probe_health", "probe_version", "GROBID:")
_HEALTH_METHODS = {"is_alive", "probe_health", "probe_version"}
_FN_DEF = re.compile(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+[A-Za-z_]")
_LET_FROM_ENV = re.compile(
    r"let\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?::[^=;{}]+)?=\s*[^;{}]*"
    r"GrobidParser::from_env\s*\(\s*\)"
)
_COMMENT_OR_STRING = re.compile(r'"(?:[^"\\]|\\.)*"')


def _strip_line_noise(line: str) -> str:
    """Drop // comments and string literals so identifier scans see code only."""
    return _COMMENT_OR_STRING.sub('""', line.split("//", 1)[0])


def _bound_parser_name(lines: list[str], index: int) -> str | None:
    """Return the `let` binding holding a bare from_env construction, if any."""
    chunk = "\n".join(lines[max(0, index - 3) : index + 1])
    match = _LET_FROM_ENV.search(chunk)
    return match.group(1) if match else None


def _enclosing_fn_scope(lines: list[str], index: int) -> tuple[int, int]:
    """Return [start, end) line bounds of the enclosing fn body (whole file fallback)."""
    start = 0
    for pos in range(index, -1, -1):
        if _FN_DEF.match(lines[pos]):
            start = pos
            break
    end = len(lines)
    for pos in range(index + 1, len(lines)):
        if _FN_DEF.match(lines[pos]):
            end = pos
            break
    return start, end


def _parser_binding_use(lines: list[str], index: int, name: str) -> str:
    """Classify how a bare-constructed binding is used in its fn scope.

    Returns "parse" (real work: parse_pdf, any other non-health method call,
    or a moved/consumed bare use such as Box::new(parser)), "health_only"
    (exclusively liveness probing), or "unused" (fail-closed by the caller).
    """
    start, end = _enclosing_fn_scope(lines, index)
    use = re.compile(rf"\b{re.escape(name)}\b(?:\s*\.\s*([A-Za-z_][A-Za-z0-9_]*))?")
    skip_top = max(0, index - 3)
    saw_health = False
    for pos in range(start, end):
        if skip_top <= pos <= index:
            continue
        for match in use.finditer(_strip_line_noise(lines[pos])):
            method = match.group(1)
            if method is None:
                return "parse"
            if method == "parse_pdf":
                return "parse"
            if method in _HEALTH_METHODS:
                saw_health = True
            else:
                return "parse"
    return "health_only" if saw_health else "unused"


def _from_env_role(lines: list[str], index: int, relative: str) -> str:
    """Classify one GrobidParser::from_env() site. Missing roots are skipped by the caller.

    Fail-closed lifetime rule: a bare from_env binding used for anything beyond
    liveness probing in its enclosing fn scope is PARSE_BARE, even when a health
    probe (is_alive) sits nearby. Pure health-only, with_profile, helper, test,
    and comment roles stay allowed.
    """
    stripped = lines[index].strip()
    if stripped.startswith(("//", "///", "//!", "*")):
        return "comment"
    window = "\n".join(lines[max(0, index - 2) : min(len(lines), index + 8)])
    prev = "\n".join(lines[max(0, index - 8) : index + 4])
    if "fn corpus_grobid_parser" in prev:
        return "consult_helper"
    if ".with_profile" in window:
        return "with_profile"
    if "mod tests" in "\n".join(lines[: index + 1]) or relative.endswith("/tests.rs") or "/tests/" in relative:
        return "test"
    name = _bound_parser_name(lines, index)
    if name is not None:
        if _parser_binding_use(lines, index, name) == "health_only":
            return "health_probe"
        return "PARSE_BARE"
    if "parse_pdf" in window:
        return "PARSE_BARE"
    if any(token in window for token in _HEALTH_TOKENS):
        return "health_probe"
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


def _cascade_observe_tokens(stages: Any) -> tuple[list[str], list[str]]:
    """Derive required/forbidden observe-stamp tokens from cascade DATA.

    Convention: stage ``P7`` maps to token ``stamp_p7_`` (trailing underscore
    avoids a future P10 collision). Stages with
    ``observe_mode == accountant_later`` are forbidden (must not be stamped
    as live); every other stage — including production_stamp — is required
    (fail-closed).
    """
    if not isinstance(stages, dict):
        return [], []
    required: list[str] = []
    forbidden: list[str] = []
    for sid, row in stages.items():
        token = f"stamp_{str(sid).lower()}_"
        mode = str((row if isinstance(row, dict) else {}).get("observe_mode") or "")
        if mode == "accountant_later":
            forbidden.append(token)
        else:
            required.append(token)
    return required, forbidden


def _empty_native_consult_tokens(consult: Any) -> list[tuple[str, str]]:
    """Derive (snake, PascalCase) tokens from DATA consult_status.empty_native_unknown_point.

    Intentionally conservative substring scan is limited to a single file
    (crates/da-cli/src/source_cli.rs) and to this DATA set, comments and
    string literals included. No hardcoded DecisionPoint list and no invented
    fallback on malformed/missing DATA: the exact-list DATA-shape check owns
    that surface and this helper returns [] there.
    """
    if not isinstance(consult, dict):
        return []
    raw = consult.get("empty_native_unknown_point")
    if not isinstance(raw, list):
        return []
    pairs: list[tuple[str, str]] = []
    for item in raw:
        if not isinstance(item, str):
            continue
        snake = item.strip()
        if not snake:
            continue
        pascal = "".join(part[:1].upper() + part[1:].lower() for part in snake.split("_") if part)
        if not pascal:
            continue
        pairs.append((snake, pascal))
    return pairs


def _native_rules_not_consulted_tokens(consult: Any) -> list[tuple[str, str]]:
    """Derive (snake, PascalCase) tokens from DATA consult_status.native_rules_not_consulted.

    Sibling of the S03 empty-native helper: scan is limited to a single file
    (crates/da-cli/src/source_cli.rs) and to this DATA set, comments and
    string literals included. No hardcoded DecisionPoint list and no invented
    fallback on malformed/missing DATA: the exact-list DATA-shape check owns
    that surface and this helper returns [] there.
    """
    if not isinstance(consult, dict):
        return []
    raw = consult.get("native_rules_not_consulted")
    if not isinstance(raw, list):
        return []
    pairs: list[tuple[str, str]] = []
    for item in raw:
        if not isinstance(item, str):
            continue
        snake = item.strip()
        if not snake:
            continue
        pascal = "".join(part[:1].upper() + part[1:].lower() for part in snake.split("_") if part)
        if not pascal:
            continue
        pairs.append((snake, pascal))
    return pairs


def _body_accounting_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive native body accountant atoms and boundaries from ledger DATA."""
    if not isinstance(defaults, dict):
        return [], []
    spec = defaults.get("body_accounting_quarantine")
    if not isinstance(spec, dict):
        return [], []
    tokens = [str(item).strip() for item in (spec.get("tokens") or []) if isinstance(item, str) and str(item).strip()]
    boundaries = [str(item).strip() for item in (spec.get("boundaries") or []) if isinstance(item, str) and str(item).strip()]
    return tokens, boundaries


def _fulltext_blocks_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive S03 semantic companion atoms and production boundaries from DATA."""
    if not isinstance(defaults, dict):
        return [], []
    spec = defaults.get("fulltext_blocks_quarantine")
    if not isinstance(spec, dict):
        return [], []
    tokens = [str(item).strip() for item in (spec.get("tokens") or []) if isinstance(item, str) and str(item).strip()]
    boundaries = [str(item).strip() for item in (spec.get("boundaries") or []) if isinstance(item, str) and str(item).strip()]
    return tokens, boundaries

def _fulltext_blocks_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    """Find S03 type/call atoms while ignoring comments and string literals."""
    code = _strip_header_quarantine_comments(code)
    hits = []
    for token in tokens:
        pattern = rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*(?:\(|\b)" if "(" not in token else re.escape(token)
        if re.search(pattern, code):
            hits.append(token)
    return hits

def _consolidation_specialized_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive the S05 specialized companion atoms and production boundaries."""
    if not isinstance(defaults, dict):
        return [], []
    spec = defaults.get("consolidation_specialized_quarantine")
    if not isinstance(spec, dict):
        return [], []
    tokens = [str(item).strip() for item in (spec.get("tokens") or []) if isinstance(item, str) and str(item).strip()]
    boundaries = [str(item).strip() for item in (spec.get("boundaries") or []) if isinstance(item, str) and str(item).strip()]
    return tokens, boundaries


def _consolidation_specialized_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    """Find S05 names in production boundaries while ignoring comments."""
    code = _strip_header_quarantine_comments(code)
    return [token for token in tokens if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code)]


def _callout_matching_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    if not isinstance(defaults, dict):
        return [], []
    spec = defaults.get("callout_matching_quarantine")
    if not isinstance(spec, dict):
        return [], []
    tokens = [str(item).strip() for item in (spec.get("tokens") or []) if isinstance(item, str) and str(item).strip()]
    boundaries = [str(item).strip() for item in (spec.get("boundaries") or []) if isinstance(item, str) and str(item).strip()]
    return tokens, boundaries

def _callout_matching_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    """Find S04 matcher atoms in production boundaries, excluding comments."""
    code = _strip_header_quarantine_comments(code)
    return [token for token in tokens if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code)]

def _feature_tagger_training_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive S06 feature/tagger/training atoms and boundaries from ledger DATA."""
    if not isinstance(defaults, dict):
        return [], []
    spec = defaults.get("feature_tagger_training_quarantine")
    if not isinstance(spec, dict):
        return [], []
    tokens = [str(item).strip() for item in (spec.get("tokens") or []) if isinstance(item, str) and str(item).strip()]
    boundaries = [str(item).strip() for item in (spec.get("boundaries") or []) if isinstance(item, str) and str(item).strip()]
    return tokens, boundaries

def _feature_tagger_training_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    """Find S06 typed atoms in production boundaries, excluding comments."""
    code = _strip_header_quarantine_comments(code)
    return [token for token in tokens if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code)]

def _body_accounting_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    """Find typed names and call-sites in code, excluding line comments."""
    code = _strip_header_quarantine_comments(code)
    hits: list[str] = []
    for token in tokens:
        if "_" in token:
            pattern = rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*\("
        else:
            pattern = rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])"
        if re.search(pattern, code):
            hits.append(token)
    return hits


def _java_absent_endpoint_tokens(defaults: Any) -> list[str]:
    """Derive the independent Java absent-endpoint set from ledger DATA.

    Returns only non-empty strings from the ``java_absent_endpoints`` list.
    Missing or malformed DATA yields ``[]`` with no hardcoded fallback: the
    forbidden set must never be invented by the checker. The existing
    SHA/evidence_missing/load signals own the missing-ledger surface; this
    helper stays silent there.
    """
    if not isinstance(defaults, dict):
        return []
    raw = defaults.get("java_absent_endpoints")
    if not isinstance(raw, list):
        return []
    tokens: list[str] = []
    for item in raw:
        if not isinstance(item, str):
            continue
        token = item.strip()
        if token:
            tokens.append(token)
    return tokens


def _find_java_absent_capability(caps: list[dict[str, Any]], by_id: dict[str, dict[str, Any]], token: str) -> dict[str, Any] | None:
    """Locate one capability for a DATA token by exact typed match only.

    Match order: exact suffix capability id (``rest.{token}``), exact
    ``official.grobid_path_constant`` is not a token match by itself (the
    constant differs from the wire token), so the typed matches are: exact
    suffix id, exact ``official.identity`` containing the token as an exact
    ``/api/{token}`` route, or identity ending with ``/api/{token}``. Raw
    substring scan of the YAML is forbidden.
    """
    suffix_id = f"rest.{token}"
    direct = by_id.get(suffix_id)
    if isinstance(direct, dict):
        return direct
    route = f"/api/{token}"
    for cap in caps:
        if not isinstance(cap, dict):
            continue
        official = cap.get("official") if isinstance(cap.get("official"), dict) else {}
        identity = str(official.get("identity") or "")
        if route in identity:
            # Accept exact identity match or an identity ending with the
            # exact route (method prefix like "POST " is allowed).
            tail = identity.strip().split()[-1] if identity.strip() else ""
            if tail == route or identity.strip().endswith(route):
                return cap
    return None


def _header_sequence_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive the native header quarantine (tokens, boundaries) from ledger DATA.

    Returns only non-empty strings from ``header_sequence_quarantine.tokens``
    and ``header_sequence_quarantine.boundaries``. Missing or malformed DATA
    yields ``([], [])`` with no hardcoded fallback: the forbidden set and the
    boundary set must never be invented by the checker. The existing
    SHA/evidence_missing/load signals own the missing-ledger surface; this
    helper stays silent there.
    """
    if not isinstance(defaults, dict):
        return [], []
    raw = defaults.get("header_sequence_quarantine")
    if not isinstance(raw, dict):
        return [], []
    tokens: list[str] = []
    for item in raw.get("tokens") if isinstance(raw.get("tokens"), list) else []:
        if not isinstance(item, str):
            continue
        token = item.strip()
        if token:
            tokens.append(token)
    boundaries: list[str] = []
    for item in raw.get("boundaries") if isinstance(raw.get("boundaries"), list) else []:
        if not isinstance(item, str):
            continue
        boundary = item.strip()
        if boundary:
            boundaries.append(boundary)
    return tokens, boundaries


def _strip_header_quarantine_comments(text: str) -> str:
    kept: list[str] = []
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            kept.append("")
            continue
        cut = line.find("//")
        kept.append(line if cut < 0 else line[:cut])
    return "\n".join(kept)


def _header_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    hits: list[str] = []
    for token in tokens:
        if "_" in token:
            # snake_case: exact code-position call-site, whitespace allowed before `(`.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*\(", code):
                hits.append(token)
        else:
            # CamelCase type name: word boundary, no call paren required.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code):
                hits.append(token)
    return hits


def _check_header_quarantine(root: Path, registry: dict[str, Any], registry_path: Path, flag: Any) -> None:
    if not isinstance(registry, dict) or not registry or not registry_path.is_file():
        return
    defaults = registry.get("defaults") if isinstance(registry, dict) else None
    tokens, boundaries = _header_sequence_quarantine_spec(defaults)
    if not tokens or not boundaries:
        return
    for relative in boundaries:
        path = root / relative
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            flag("policy_stamp", "integration", f"{relative}#header_quarantine", "readable boundary", str(error))
            continue
        code = _strip_header_quarantine_comments(text)
        for token in _header_quarantine_tokens_present(code, tokens):
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#header_quarantine.{token}",
                "header atoms stay shadow-only (no parse-pdf source_cli or ParserPort splice)",
                f"header atom {token} in {relative}",
            )

_BIBLIOGRAPHY_QUARANTINE_TOKENS = ("compose_references", "run_reference_sequence")
_BIBLIOGRAPHY_QUARANTINE_BOUNDARIES = (
    "crates/da-cli/src/source_cli.rs",
    "crates/da-ports/src/parser.rs",
)


def _strip_bibliography_quarantine_comments(text: str) -> str:
    kept: list[str] = []
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            kept.append("")
            continue
        cut = line.find("//")
        kept.append(line if cut < 0 else line[:cut])
    return "\n".join(kept)


def _bibliography_quarantine_tokens_present(code: str) -> list[str]:
    hits: list[str] = []
    for token in _BIBLIOGRAPHY_QUARANTINE_TOKENS:
        if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*\(", code):
            hits.append(token)
    return hits


def _check_bibliography_quarantine(root: Path, registry: dict[str, Any], registry_path: Path, flag: Any) -> None:
    if not isinstance(registry, dict) or not registry or not registry_path.is_file():
        return
    for relative in _BIBLIOGRAPHY_QUARANTINE_BOUNDARIES:
        path = root / relative
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            flag("policy_stamp", "integration", f"{relative}#bibliography_quarantine", "readable boundary", str(error))
            continue
        code = _strip_bibliography_quarantine_comments(text)
        for token in _bibliography_quarantine_tokens_present(code):
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#bibliography_quarantine.{token}",
                "bibliography atoms stay shadow-only (no parse-pdf source_cli or ParserPort splice)",
                f"bibliography atom {token} in {relative}",
            )


def _bibliography_sequence_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive the native bibliography/callout quarantine from ledger DATA.

    Returns only non-empty strings from ``bibliography_sequence_quarantine.tokens``
    and ``bibliography_sequence_quarantine.boundaries``. Missing or malformed DATA
    yields ``([], [])`` with no hardcoded fallback: the forbidden set and the
    boundary set must never be invented by the checker. The existing
    SHA/evidence_missing/load signals own the missing-ledger surface; this helper
    stays silent there.

    This is additive to ``_BIBLIOGRAPHY_QUARANTINE_TOKENS`` (MEM1115 / D362):
    that tuple keeps only the snake_case call-site atoms ``compose_references``
    and ``run_reference_sequence``, while this DATA key carries the S04 lane's
    own atoms. The ParserPort type/method prohibition stays on the Rust side
    (``crates/da-parse/tests/architecture_contract.rs``).
    """
    if not isinstance(defaults, dict):
        return [], []
    raw = defaults.get("bibliography_sequence_quarantine")
    if not isinstance(raw, dict):
        return [], []
    tokens: list[str] = []
    for item in raw.get("tokens") if isinstance(raw.get("tokens"), list) else []:
        if not isinstance(item, str):
            continue
        token = item.strip()
        if token:
            tokens.append(token)
    boundaries: list[str] = []
    for item in raw.get("boundaries") if isinstance(raw.get("boundaries"), list) else []:
        if not isinstance(item, str):
            continue
        boundary = item.strip()
        if boundary:
            boundaries.append(boundary)
    return tokens, boundaries


def _bibliography_sequence_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    hits: list[str] = []
    for token in tokens:
        if "_" in token:
            # snake_case: exact code-position call-site, whitespace allowed before `(`.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*\(", code):
                hits.append(token)
        else:
            # CamelCase type name: word boundary, no call paren required.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code):
                hits.append(token)
    return hits


def _check_bibliography_sequence_quarantine(
    root: Path, registry: dict[str, Any], registry_path: Path, flag: Any
) -> None:
    if not isinstance(registry, dict) or not registry or not registry_path.is_file():
        return
    defaults = registry.get("defaults") if isinstance(registry, dict) else None
    tokens, boundaries = _bibliography_sequence_quarantine_spec(defaults)
    if not tokens or not boundaries:
        return
    for relative in boundaries:
        path = root / relative
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#bibliography_sequence_quarantine",
                "readable boundary",
                str(error),
            )
            continue
        code = _strip_bibliography_quarantine_comments(text)
        for token in _bibliography_sequence_quarantine_tokens_present(code, tokens):
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#bibliography_sequence_quarantine.{token}",
                "native bibliography atoms stay shadow-only (no parse-pdf source_cli or ParserPort splice)",
                f"native bibliography atom {token} in {relative}",
            )


def _fulltext_sequence_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive the native body/fulltext quarantine from ledger DATA.

    Returns only non-empty strings from ``fulltext_sequence_quarantine.tokens``
    and ``fulltext_sequence_quarantine.boundaries``. Missing or malformed DATA
    yields ``([], [])`` with no hardcoded fallback: the forbidden set and the
    boundary set must never be invented by the checker. The existing
    SHA/evidence_missing/load signals own the missing-ledger surface; this
    helper stays silent there.

    This key is a THIRD lane next to the untouched
    ``fulltext_blocks_quarantine`` (P2 region accounting) and
    ``body_accounting_quarantine`` (M496 P5 accountant) keys (D362): the S05
    body reading-order lane has its own token set and its own four production
    boundaries, and the three sets are never merged. The ParserPort
    type/method prohibition stays on the Rust side
    (``crates/da-parse/tests/architecture_contract.rs``).
    """
    if not isinstance(defaults, dict):
        return [], []
    raw = defaults.get("fulltext_sequence_quarantine")
    if not isinstance(raw, dict):
        return [], []
    tokens: list[str] = []
    for item in raw.get("tokens") if isinstance(raw.get("tokens"), list) else []:
        if not isinstance(item, str):
            continue
        token = item.strip()
        if token:
            tokens.append(token)
    boundaries: list[str] = []
    for item in raw.get("boundaries") if isinstance(raw.get("boundaries"), list) else []:
        if not isinstance(item, str):
            continue
        boundary = item.strip()
        if boundary:
            boundaries.append(boundary)
    return tokens, boundaries


def _strip_fulltext_sequence_quarantine_comments(text: str) -> str:
    kept: list[str] = []
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            kept.append("")
            continue
        cut = line.find("//")
        kept.append(line if cut < 0 else line[:cut])
    return "\n".join(kept)


def _fulltext_sequence_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    hits: list[str] = []
    for token in tokens:
        if "_" in token:
            # snake_case: exact code-position call-site, whitespace allowed before `(`.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*\(", code):
                hits.append(token)
        else:
            # CamelCase type name: word boundary, no call paren required.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code):
                hits.append(token)
    return hits


def _check_fulltext_sequence_quarantine(
    root: Path, registry: dict[str, Any], registry_path: Path, flag: Any
) -> None:
    if not isinstance(registry, dict) or not registry or not registry_path.is_file():
        return
    defaults = registry.get("defaults") if isinstance(registry, dict) else None
    tokens, boundaries = _fulltext_sequence_quarantine_spec(defaults)
    if not tokens or not boundaries:
        return
    for relative in boundaries:
        path = root / relative
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#fulltext_sequence_quarantine",
                "readable boundary",
                str(error),
            )
            continue
        code = _strip_fulltext_sequence_quarantine_comments(text)
        for token in _fulltext_sequence_quarantine_tokens_present(code, tokens):
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#fulltext_sequence_quarantine.{token}",
                "native body/fulltext atoms stay shadow-only (no parse-pdf source_cli, ParserPort, parser.rs or projector.rs splice)",
                f"native body/fulltext atom {token} in {relative}",
            )


def _family_sequence_quarantine_spec(defaults: Any, key: str) -> tuple[list[str], list[str]]:
    """Derive a native visual-family quarantine (tokens, boundaries) from DATA.

    Returns only non-empty strings from ``<key>.tokens`` and
    ``<key>.boundaries``. Missing or malformed DATA yields ``([], [])`` with no
    hardcoded fallback: the forbidden set and the boundary set must never be
    invented by the checker. The existing SHA/evidence_missing/load signals own
    the missing-ledger surface; this helper stays silent there.

    Both S06 keys (``visual_family_sequence_quarantine`` for the figure/table/
    formula bands and ``funding_sequence_quarantine`` for the funding band) are
    their OWN lanes (D362) next to ``visual_heads_quarantine`` (S08 geometric
    heads), ``fulltext_blocks_quarantine`` (P2 region accounting),
    ``body_accounting_quarantine`` (M496 P5 accountant) and
    ``fulltext_sequence_quarantine`` (S05 body reading order). Name adjacency is
    not inheritance: each key owns its token set and its own production
    boundaries, and the sets are never merged. The ParserPort type/method
    prohibition stays on the Rust side
    (``crates/da-parse/tests/architecture_contract.rs``).
    """
    if not isinstance(defaults, dict):
        return [], []
    raw = defaults.get(key)
    if not isinstance(raw, dict):
        return [], []
    tokens: list[str] = []
    for item in raw.get("tokens") if isinstance(raw.get("tokens"), list) else []:
        if not isinstance(item, str):
            continue
        token = item.strip()
        if token:
            tokens.append(token)
    boundaries: list[str] = []
    for item in raw.get("boundaries") if isinstance(raw.get("boundaries"), list) else []:
        if not isinstance(item, str):
            continue
        boundary = item.strip()
        if boundary:
            boundaries.append(boundary)
    return tokens, boundaries


def _visual_family_sequence_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """S06 native figure/table/formula lane (own key, own boundaries)."""
    return _family_sequence_quarantine_spec(defaults, "visual_family_sequence_quarantine")


def _funding_sequence_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """S06 native funding lane (own key, own boundaries)."""
    return _family_sequence_quarantine_spec(defaults, "funding_sequence_quarantine")


def _strip_family_sequence_quarantine_comments(text: str) -> str:
    kept: list[str] = []
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            kept.append("")
            continue
        cut = line.find("//")
        kept.append(line if cut < 0 else line[:cut])
    return "\n".join(kept)


def _family_sequence_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    """One matcher for every lane; the per-lane token list is the only variable.

    snake_case tokens are call-sites (whitespace allowed before `(`); CamelCase
    tokens are type names (word boundary). Same rule as the S05 lane.
    """
    hits: list[str] = []
    for token in tokens:
        if "_" in token:
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*\(", code):
                hits.append(token)
        else:
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code):
                hits.append(token)
    return hits


def _visual_family_sequence_quarantine_tokens_present(
    code: str, tokens: list[str]
) -> list[str]:
    return _family_sequence_quarantine_tokens_present(code, tokens)


def _funding_sequence_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    return _family_sequence_quarantine_tokens_present(code, tokens)


def _check_family_sequence_quarantine(
    root: Path,
    registry: dict[str, Any],
    registry_path: Path,
    flag: Any,
    *,
    key: str,
    spec: Any,
    reason: str,
) -> None:
    if not isinstance(registry, dict) or not registry or not registry_path.is_file():
        return
    defaults = registry.get("defaults") if isinstance(registry, dict) else None
    tokens, boundaries = spec(defaults)
    if not tokens or not boundaries:
        return
    for relative in boundaries:
        path = root / relative
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#{key}",
                "readable boundary",
                str(error),
            )
            continue
        code = _strip_family_sequence_quarantine_comments(text)
        for token in _family_sequence_quarantine_tokens_present(code, tokens):
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#{key}.{token}",
                reason,
                f"native family atom {token} in {relative}",
            )


def _check_visual_family_sequence_quarantine(
    root: Path, registry: dict[str, Any], registry_path: Path, flag: Any
) -> None:
    _check_family_sequence_quarantine(
        root,
        registry,
        registry_path,
        flag,
        key="visual_family_sequence_quarantine",
        spec=_visual_family_sequence_quarantine_spec,
        reason=(
            "native figure/table/formula atoms stay shadow-only (no parse-pdf "
            "source_cli, ParserPort, parser.rs or projector.rs splice)"
        ),
    )


def _check_funding_sequence_quarantine(
    root: Path, registry: dict[str, Any], registry_path: Path, flag: Any
) -> None:
    _check_family_sequence_quarantine(
        root,
        registry,
        registry_path,
        flag,
        key="funding_sequence_quarantine",
        spec=_funding_sequence_quarantine_spec,
        reason=(
            "native funding atoms stay shadow-only (no parse-pdf source_cli, "
            "ParserPort, parser.rs or projector.rs splice)"
        ),
    )


def _visual_heads_quarantine_spec(defaults: Any) -> tuple[list[str], list[str]]:
    """Derive the visual-head quarantine (tokens, boundaries) from ledger DATA.

    Returns only non-empty strings from ``visual_heads_quarantine.tokens``
    and ``visual_heads_quarantine.boundaries``. Missing or malformed DATA
    yields ``([], [])`` with no hardcoded fallback: the forbidden set and
    the boundary set must never be invented by the checker. The existing
    SHA/evidence_missing/load signals own the missing-ledger surface; this
    helper stays silent there.
    """
    if not isinstance(defaults, dict):
        return [], []
    raw = defaults.get("visual_heads_quarantine")
    if not isinstance(raw, dict):
        return [], []
    tokens: list[str] = []
    for item in raw.get("tokens") if isinstance(raw.get("tokens"), list) else []:
        if not isinstance(item, str):
            continue
        token = item.strip()
        if token:
            tokens.append(token)
    boundaries: list[str] = []
    for item in raw.get("boundaries") if isinstance(raw.get("boundaries"), list) else []:
        if not isinstance(item, str):
            continue
        boundary = item.strip()
        if boundary:
            boundaries.append(boundary)
    return tokens, boundaries


def _strip_visual_heads_quarantine_comments(text: str) -> str:
    kept: list[str] = []
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            kept.append("")
            continue
        cut = line.find("//")
        kept.append(line if cut < 0 else line[:cut])
    return "\n".join(kept)


def _visual_heads_quarantine_tokens_present(code: str, tokens: list[str]) -> list[str]:
    hits: list[str] = []
    for token in tokens:
        if "_" in token:
            # snake_case: exact code-position call-site, whitespace allowed before `(`.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}\s*\(", code):
                hits.append(token)
        else:
            # CamelCase type name: word boundary, no call paren required.
            if re.search(rf"(?<![A-Za-z0-9_]){re.escape(token)}(?![A-Za-z0-9_])", code):
                hits.append(token)
    return hits


def _check_visual_heads_quarantine(root: Path, registry: dict[str, Any], registry_path: Path, flag: Any) -> None:
    if not isinstance(registry, dict) or not registry or not registry_path.is_file():
        return
    defaults = registry.get("defaults") if isinstance(registry, dict) else None
    tokens, boundaries = _visual_heads_quarantine_spec(defaults)
    if not tokens or not boundaries:
        return
    for relative in boundaries:
        path = root / relative
        if not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:
            flag("policy_stamp", "integration", f"{relative}#visual_heads_quarantine", "readable boundary", str(error))
            continue
        code = _strip_visual_heads_quarantine_comments(text)
        for token in _visual_heads_quarantine_tokens_present(code, tokens):
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#visual_heads_quarantine.{token}",
                "visual heads stay shadow-only (no parse-pdf source_cli or ParserPort splice)",
                f"visual head atom {token} in {relative}",
            )


def _check_java_absent_endpoints(
    registry: dict[str, Any],
    caps: list[dict[str, Any]],
    by_id: dict[str, dict[str, Any]],
    flag: Any,
) -> None:
    """Flag any Java absent-endpoint token that is not absent with no surface.

    A Java path is not a proxy surface: for each DATA token the ledger must
    carry a capability whose state is exactly ``absent`` and whose
    ``implementation.surface`` is none/empty/null. Missing row, non-absent
    state, or a real surface all flag with canonical evidence
    ``data/parse/grobid_compatibility.yaml#java_absent_endpoints.{token}``.
    This loop is intentionally separate from the opposite-polarity
    proxy/shadow-empty-surface scan above.
    """
    defaults = registry.get("defaults") if isinstance(registry, dict) else None
    for token in _java_absent_endpoint_tokens(defaults):
        cap = _find_java_absent_capability(caps, by_id, token)
        if cap is None:
            flag(
                "ledger_surface",
                "integration",
                f"data/parse/grobid_compatibility.yaml#java_absent_endpoints.{token}",
                {"state": "absent", "surface": "none"},
                "Java path is not a proxy surface: capability row missing",
            )
            continue
        state = cap.get("state")
        impl = cap.get("implementation") if isinstance(cap.get("implementation"), dict) else {}
        surface = (impl or {}).get("surface")
        surface_text = "" if surface is None else str(surface).strip()
        if state != "absent" or (surface_text != "" and surface_text.lower() != "none"):
            flag(
                "ledger_surface",
                "integration",
                f"data/parse/grobid_compatibility.yaml#java_absent_endpoints.{token}",
                {"state": "absent", "surface": "none"},
                f"Java path is not a proxy surface: state={state!r} surface={surface_text!r}",
            )


def _check_rest_batch_asset_quarantine(root: Path, contract: dict[str, Any], flag: Any) -> None:
    spec = contract.get("rest_batch_asset_quarantine") or {}
    bindings = spec.get("source_bindings") or []
    for binding in bindings:
        relative = str(binding.get("path") or "")
        path = root / relative
        if not path.is_file():
            flag("rest_batch_asset_quarantine", "integration", relative, "tracked source binding", "missing")
            continue
        observed = _sha256(path)
        if observed != binding.get("sha256"):
            flag("rest_batch_asset_quarantine", "integration", relative, binding.get("sha256"), observed)
            continue
        try:
            value = json.loads(path.read_text(encoding="utf-8")) if path.suffix == ".json" else yaml.safe_load(path.read_text(encoding="utf-8"))
        except (OSError, ValueError, yaml.YAMLError) as error:
            flag("rest_batch_asset_quarantine", "integration", relative, binding.get("schema_version"), str(error))
            continue
        observed_schema = value.get("schema_version") if isinstance(value, dict) else None
        if binding.get("schema_version") == "ledger_version:1":
            valid_schema = isinstance(value, dict) and value.get("ledger_version") == 1
        else:
            valid_schema = observed_schema == binding.get("schema_version")
        if not valid_schema:
            flag("rest_batch_asset_quarantine", "integration", relative, binding.get("schema_version"), observed_schema if isinstance(value, dict) else type(value).__name__)

    closed = set((spec.get("dispositions") or {}).get("closed") or [])
    expected_closed = {"existing_typed_transport", "existing_local_batch_composition", "product_required_gap", "not_required_by_current_journey", "unsupported_without_source", "forbidden_this_wave", "propagated_terminal"}
    if closed != expected_closed:
        flag("rest_batch_asset_quarantine", "integration", "scholarly_parse_plane.yaml#rest_batch_asset_quarantine.dispositions", sorted(expected_closed), sorted(closed))
    if (spec.get("dispositions") or {}).get("implemented_new_endpoints") != 0:
        flag("rest_batch_asset_quarantine", "promotion_quarantine", "scholarly_parse_plane.yaml#rest_batch_asset_quarantine.dispositions", 0, (spec.get("dispositions") or {}).get("implemented_new_endpoints"))

    sanctioned = spec.get("sanctioned_endpoints") or []
    expected = [
        ("rest.processFulltextDocument", "POST", "/api/processFulltextDocument", "fulltext-v1", "fulltext=v1"),
        ("rest.processHeaderDocument", "POST", "/api/processHeaderDocument", "header-v1", "header=v1"),
        ("rest.processReferences", "POST", "/api/processReferences", "references-v1", "references=v1"),
    ]
    actual = [(row.get("id"), row.get("method"), row.get("route"), row.get("request"), row.get("result")) for row in sanctioned if isinstance(row, dict)]
    if actual != expected:
        flag("rest_batch_asset_quarantine", "integration", "scholarly_parse_plane.yaml#rest_batch_asset_quarantine.sanctioned_endpoints", expected, actual)

    policy = spec.get("policy") or {}
    expected_policy = {"rollback_target": "java-grobid-http@0.9.0-crf", "structural_parse_independent_of_enrichment": True, "decision_point": 6, "independently_selectable": ["header", "references"], "import_eligible": False, "graph_write": "forbidden", "auto_promote": "forbidden", "production_route": "forbidden"}
    for key, value in expected_policy.items():
        if policy.get(key) != value:
            flag("rest_batch_asset_quarantine", "promotion_quarantine", f"scholarly_parse_plane.yaml#rest_batch_asset_quarantine.policy.{key}", value, policy.get(key))
    if (spec.get("local_batch") or {}).get("not_endpoint") is not True:
        flag("rest_batch_asset_quarantine", "integration", "scholarly_parse_plane.yaml#rest_batch_asset_quarantine.local_batch", True, spec.get("local_batch"))

    roots = (spec.get("scan") or {}).get("roots") or []
    forbidden = (spec.get("scan") or {}).get("forbidden_tokens") or []
    route_markers = (spec.get("scan") or {}).get("forbidden_route_markers") or []
    for relative in roots:
        path = root / relative
        files = sorted(path.rglob("*.rs")) if path.is_dir() else ([path] if path.is_file() else [])
        for file_path in files:
            try:
                source = file_path.read_text(encoding="utf-8")
            except OSError as error:
                flag("rest_batch_asset_quarantine", "integration", str(file_path.relative_to(root)), "readable source", str(error))
                continue
            code = re.sub(r"//[^\n]*", "", source)
            for token in forbidden + route_markers:
                if re.search(rf"(?<![A-Za-z0-9_]){re.escape(str(token))}(?![A-Za-z0-9_])", code):
                    flag("rest_batch_asset_quarantine", "promotion_quarantine", f"{file_path.relative_to(root)}#rest_batch_asset_quarantine", "no forbidden asset/training/arbitrary-route symbol", token)


def _parity_ledger_census(rows: list[dict[str, Any]], key: str) -> dict[str, int]:
    """Census of one parity-ledger numerator column, keyed by its observed value.

    `key` is the column name (`product_scope` for `capabilities[]`, `parity` for
    `parity_ledger.rows[]`). A row whose column is absent or empty is counted
    under the empty string rather than folded into a named bucket, so a dropped
    field widens a whole numerator row instead of hiding inside one.
    """
    counts: dict[str, int] = {}
    for row in rows:
        value = str(row.get(key) or "").strip()
        counts[value] = counts.get(value, 0) + 1
    return counts


def _check_parity_ledger_denominators(
    registry: dict[str, Any], caps: list[dict[str, Any]], flag: Any
) -> None:
    """Hold the parity ledger's declared denominators against the carrier that owns them.

    `capabilities[]` is the sole capability denominator (the ledger says so in
    its own `authority` text, and `parity_ledger.rows[]` is a per-capability
    projection over it). So every counter under `parity_ledger.denominators`
    must be reproducible from the same file: a `scope` counter that no longer
    equals the `product_scope` census of `capabilities[]`, a drifted
    capability / state / parity / named-gap count, or a denominator block
    removed outright is a named flag. An absent block reports
    `parity_ledger_missing` rather than passing as a clean run — the carrier is
    the only place these numbers are held, so silence would be a claim, not an
    absence.
    """
    ledger = registry.get("parity_ledger")
    if not isinstance(ledger, dict):
        flag(
            "parity_ledger_missing",
            "integration",
            "data/parse/grobid_compatibility.yaml#parity_ledger",
            "declared parity_ledger block",
            None if ledger is None else type(ledger).__name__,
        )
        return
    denominators = ledger.get("denominators")
    if not isinstance(denominators, dict):
        flag(
            "parity_ledger_missing",
            "integration",
            "data/parse/grobid_compatibility.yaml#parity_ledger.denominators",
            "declared denominators mapping",
            None if denominators is None else type(denominators).__name__,
        )
        return

    rows = [row for row in (ledger.get("rows") or []) if isinstance(row, dict)]
    named_gaps = [gap for gap in (ledger.get("named_gaps") or []) if isinstance(gap, dict)]
    observed: dict[str, Any] = {
        "capabilities": len(caps),
        "scope": _parity_ledger_census(caps, "product_scope"),
        "implemented": sum(1 for row in rows if str(row.get("state") or "") != "absent"),
        "absent": sum(1 for row in rows if str(row.get("state") or "") == "absent"),
        "parity": _parity_ledger_census(rows, "parity"),
        "named_gaps": len(named_gaps),
    }
    for key in ("capabilities", "scope", "implemented", "absent", "parity", "named_gaps"):
        declared = denominators.get(key)
        if declared != observed[key]:
            flag(
                "parity_ledger_denominator",
                "integration",
                f"data/parse/grobid_compatibility.yaml#parity_ledger.denominators.{key}",
                observed[key],
                declared,
            )


def _probe_dotted(mapping: Any, ref: str) -> Any:
    """Read a dotted reference out of a nested mapping, None when absent."""
    node: Any = mapping
    for part in str(ref).split("."):
        if not isinstance(node, dict):
            return None
        node = node.get(part)
    return node


def _release_probe_digest(articles: list[dict[str, Any]], lane: dict[str, Any]) -> str:
    """Recompute the frozen identity digest from the declared handles.

    The lane declares its own input fields, order and separators, so the digest
    is re-derived from the frozen holdout manifest rather than trusted from the
    artifact's declared value.
    """
    fields = [str(field) for field in (lane.get("digest_input_fields") or [])]
    field_separator = str(lane.get("digest_field_separator") or "")
    record_separator = str(lane.get("digest_record_separator") or "")
    payload = record_separator.join(
        field_separator.join(str(article.get(field)) for field in fields)
        for article in articles
    )
    return "sha256:" + hashlib.sha256(payload.encode("utf-8")).hexdigest()


def _check_release_probe_lane(root: Path, contract: dict[str, Any], flag: Any) -> None:
    """Hold the frozen external release-probe lane, its counters and its citation state.

    Three failure modes stay three named flags: a lane or its artifact mapping
    that is absent, unreadable or not byte-bound is `release_probe_lane_missing`;
    a counter or identity that no longer reproduces from the frozen inputs is
    `release_probe_denominator` (with the expected and observed value); and a
    citation or authorization state that unlocks while the human release review
    is still `pending` is `release_probe_citation_state` (with the exact key).
    """
    spec = contract.get("release_probe_lane")
    if not isinstance(spec, dict):
        flag(
            "release_probe_lane_missing",
            "integration",
            "scripts/governor/checks/scholarly_parse_plane.yaml#release_probe_lane",
            "declared release_probe_lane contract",
            None if spec is None else type(spec).__name__,
        )
        return
    identity = str(spec.get("artifact_identity") or "")
    manifest_rel = str(spec.get("manifest") or "data/parse/acceptance_manifest.yaml")
    holdout_rel = str(spec.get("holdout_manifest") or "data/parse/holdout_manifests.yaml")
    lane_ref = str(spec.get("lane_ref") or "external_holdout_contract.release_probe")

    # The artifact is only held when it is declared on both binding surfaces:
    # byte-pinned in canonical_artifacts and identity-mapped in
    # integration_evidence. One without the other is an unmapped lane, not a
    # pass.
    bindings = contract.get("bindings") or {}
    canonical_paths = [
        str(item.get("path"))
        for item in (bindings.get("canonical_artifacts") or [])
        if isinstance(item, dict)
    ]
    evidence = [
        (str(item.get("path")), str(item.get("identity")))
        for item in (bindings.get("integration_evidence") or [])
        if isinstance(item, dict)
    ]
    mapped = [path for path, declared_identity in evidence if declared_identity == identity]
    if not mapped or mapped[0] not in canonical_paths:
        flag(
            "release_probe_lane_missing",
            "integration",
            f"scholarly_parse_plane.yaml#bindings.integration_evidence[{identity}]",
            {"mapped": True, "byte_bound": True},
            {"mapped": mapped, "canonical_artifacts": canonical_paths},
        )
        return
    artifact_rel = mapped[0]

    try:
        manifest = yaml.safe_load((root / manifest_rel).read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("release_probe_lane_missing", "integration", manifest_rel, "readable YAML", str(error))
        return
    lane = _probe_dotted(manifest, lane_ref)
    if not isinstance(lane, dict):
        flag(
            "release_probe_lane_missing",
            "integration",
            f"{manifest_rel}#{lane_ref}",
            "declared release probe lane",
            None if lane is None else type(lane).__name__,
        )
        return

    try:
        artifact = json.loads((root / artifact_rel).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        flag("release_probe_lane_missing", "integration", artifact_rel, "readable probe artifact", str(error))
        return
    if not isinstance(artifact, dict):
        flag("release_probe_lane_missing", "integration", artifact_rel, "typed probe artifact", type(artifact).__name__)
        return

    # Live identity: artifact and lane must both carry the declared identity, so
    # a self-consistent byte swap cannot pass as the frozen lane.
    if artifact.get("schema_version") != identity or lane.get("schema_version") != identity:
        flag(
            "release_probe_denominator",
            "integration",
            artifact_rel,
            {"artifact.schema_version": identity, "lane.schema_version": identity},
            {
                "artifact.schema_version": artifact.get("schema_version"),
                "lane.schema_version": lane.get("schema_version"),
            },
        )

    try:
        holdout = yaml.safe_load((root / holdout_rel).read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("release_probe_lane_missing", "integration", holdout_rel, "readable holdout manifest", str(error))
        return
    articles = [article for article in (holdout.get("articles") or []) if isinstance(article, dict)]
    documents = len(articles)
    digest = _release_probe_digest(articles, lane)
    manifest_sha = "sha256:" + _sha256(root / holdout_rel)

    declared_counts = {
        "reference_population.documents": _probe_dotted(artifact, "reference_population.documents"),
        "evaluated_population.documents": _probe_dotted(artifact, "evaluated_population.documents"),
        "evaluated_population.aggregate.documents": _probe_dotted(artifact, "evaluated_population.aggregate.documents"),
        "executed_population.documents": _probe_dotted(artifact, "executed_population.documents"),
        "production_catalog_overlap.documents": _probe_dotted(artifact, "production_catalog_overlap.documents"),
    }
    for key, observed in declared_counts.items():
        if observed != documents:
            flag("release_probe_denominator", "integration", f"{artifact_rel}#{key}", documents, observed)
    lane_tei = [entry for entry in (lane.get("reference_tei") or []) if isinstance(entry, dict)]
    if len(lane_tei) != documents:
        flag(
            "release_probe_denominator",
            "integration",
            f"{manifest_rel}#{lane_ref}.reference_tei",
            documents,
            len(lane_tei),
        )
    if lane.get("identity_digest") != digest or _probe_dotted(artifact, "reference_population.identity_sha256") != digest:
        flag(
            "release_probe_denominator",
            "integration",
            f"{manifest_rel}#{lane_ref}.identity_digest",
            digest,
            {
                "lane": lane.get("identity_digest"),
                "artifact": _probe_dotted(artifact, "reference_population.identity_sha256"),
            },
        )
    if (
        lane.get("source_manifest_sha256") != manifest_sha
        or _probe_dotted(artifact, "reference_population.source_manifest_sha256") != manifest_sha
    ):
        flag(
            "release_probe_denominator",
            "integration",
            f"{manifest_rel}#{lane_ref}.source_manifest_sha256",
            manifest_sha,
            {
                "lane": lane.get("source_manifest_sha256"),
                "artifact": _probe_dotted(artifact, "reference_population.source_manifest_sha256"),
            },
        )

    pins = {
        str(pin.get("document_id")): pin
        for pin in (_probe_dotted(artifact, "reference_population.byte_pins") or [])
        if isinstance(pin, dict)
    }
    for entry in lane_tei:
        article_id = str(entry.get("article_id"))
        tei_rel = str(entry.get("path"))
        declared_sha = str(entry.get("sha256") or "")
        declared_size = entry.get("size_bytes")
        tei_path = root / tei_rel
        if not tei_path.is_file():
            flag("release_probe_denominator", "integration", tei_rel, declared_sha, "missing")
            continue
        observed_sha = "sha256:" + _sha256(tei_path)
        observed_size = tei_path.stat().st_size
        if observed_sha != declared_sha or observed_size != declared_size:
            flag(
                "release_probe_denominator",
                "integration",
                f"{manifest_rel}#{lane_ref}.reference_tei[{article_id}]",
                {"sha256": declared_sha, "size_bytes": declared_size},
                {"sha256": observed_sha, "size_bytes": observed_size},
            )
        pin = pins.get(article_id) or {}
        pin_sha = declared_sha[len("sha256:") :] if declared_sha.startswith("sha256:") else declared_sha
        if pin.get("identity_sha256") != pin_sha or pin.get("size_bytes") != declared_size:
            flag(
                "release_probe_denominator",
                "integration",
                f"{artifact_rel}#reference_population.byte_pins[{article_id}]",
                {"identity_sha256": pin_sha, "size_bytes": declared_size},
                {"identity_sha256": pin.get("identity_sha256"), "size_bytes": pin.get("size_bytes")},
            )

    # Citation state: while the release decision remains with a real human,
    # every citation guard stays closed and every GAR-015 authorization stays
    # blocked. Each drift names the exact key that unlocked.
    for ref, expected in (spec.get("expected_values") or {}).items():
        observed = _probe_dotted(manifest, str(ref))
        if observed != expected:
            flag("release_probe_citation_state", "promotion_quarantine", f"{manifest_rel}#{ref}", expected, observed)
    prohibited = _probe_dotted(manifest, "release_decision_package.prohibited_authorizations")
    prohibited = prohibited if isinstance(prohibited, dict) else {}
    for key, expected in (spec.get("blocked_authorizations") or {}).items():
        observed = prohibited.get(key)
        if observed != expected:
            flag(
                "release_probe_citation_state",
                "promotion_quarantine",
                f"{manifest_rel}#release_decision_package.prohibited_authorizations.{key}",
                expected,
                observed,
            )
    for ref, expected in (spec.get("artifact_expectations") or {}).items():
        observed = _probe_dotted(artifact, str(ref))
        if observed != expected:
            flag("release_probe_denominator", "integration", f"{artifact_rel}#{ref}", expected, observed)


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
    if isinstance(registry, dict) and registry_path.is_file():
        _check_java_absent_endpoints(registry, caps, by_id, flag)
    _check_parity_ledger_denominators(
        registry if isinstance(registry, dict) else {}, caps, flag
    )
    _check_header_quarantine(root, registry, registry_path, flag)
    _check_bibliography_quarantine(root, registry, registry_path, flag)
    _check_bibliography_sequence_quarantine(root, registry, registry_path, flag)
    _check_fulltext_sequence_quarantine(root, registry, registry_path, flag)
    _check_visual_family_sequence_quarantine(root, registry, registry_path, flag)
    _check_funding_sequence_quarantine(root, registry, registry_path, flag)
    _check_visual_heads_quarantine(root, registry, registry_path, flag)
    _check_rest_batch_asset_quarantine(root, contract, flag)
    _check_release_probe_lane(root, contract, flag)
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
    denominators = (fulltext.get("source_evidence") or {}).get("s12_named_denominators") or {}
    lift = denominators.get("java_lift_same_n") or {}
    # M495/S01: citation_names_total=2198 и references_total=207 — раздельные
    # именованные знаменатели, не пул. Подмена 2198 на 207 или снятие запрета
    # pooling даёт тот же существующий флаг acceptance_same_n_drift, нового
    # violation label нет. Оба DATA-ключа сравниваются именно с int 2198/207
    # (YAML хранит их как int); отсутствие ключа тоже дрейф (fail-closed).
    if (
        denominators.get("citation_names_total") not in {2198, "2198"}
        or denominators.get("references_total") not in {207, "207"}
        or lift.get("java_header_authors_equals") != "header_raw_total"
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
                "citation_names_total": 2198,
                "references_total": 207,
                "java_header_authors_equals": "header_raw_total",
                "java_biblstruct_equals": "references_total",
                "hub_native_accepted_header": 20,
                "hub_java_header_raw": 21,
                "citation_names_not_biblstruct": True,
                "pooling_accepted_with_raw": "forbidden",
                "pooling_citation_names_with_references": "forbidden",
            },
            {"citation_names_total": denominators.get("citation_names_total"), "references_total": denominators.get("references_total"), **lift},
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
    # M494/S01: observation lanes are never independently selectable. A third
    # lane in selectable fails closed under its own evidence, even when the
    # exact-list check above already fired. strata.lanes may stay a superset.
    for family in ("document_segmentation", "figure", "table", "formula", "learning_policy"):
        if family in selectable:
            flag(
                "body_java_fallback",
                "integration",
                f"data/parse/native_fulltext_composition.yaml#admission.independently_selectable.{family}",
                "observation lane not selectable",
                selectable,
            )
    matrix = admission.get("current_matrix") or {}
    for family in ("document_segmentation", "figure", "table", "formula", "learning_policy"):
        if str(matrix.get(family) or "") == "passed":
            flag(
                "body_java_fallback",
                "integration",
                f"data/parse/native_fulltext_composition.yaml#admission.current_matrix.{family}",
                "fallback",
                matrix.get(family),
            )

    # S12/T02: composition freeze — top-level candidate/shadow/frozen locks.
    # Self-promotion, import/graph unlock or Java rollback drift on the live
    # composition DATA fails closed here, not only on fixture envelopes.
    if (
        fulltext.get("observation_mode") != "shadow"
        or fulltext.get("promotion_state") != "candidate"
        or fulltext.get("auto_promote") != "forbidden"
    ):
        flag(
            "family_promote_cap",
            "promotion_quarantine",
            "data/parse/native_fulltext_composition.yaml#candidate_freeze",
            {"observation_mode": "shadow", "promotion_state": "candidate", "auto_promote": "forbidden"},
            {"observation_mode": fulltext.get("observation_mode"), "promotion_state": fulltext.get("promotion_state"), "auto_promote": fulltext.get("auto_promote")},
        )
    if fulltext.get("import_eligible") is not False:
        flag(
            "import_leak",
            "promotion_quarantine",
            "data/parse/native_fulltext_composition.yaml#import_eligible",
            False,
            fulltext.get("import_eligible"),
        )
    if fulltext.get("graph_mutation") != "forbidden":
        flag(
            "graph_write",
            "promotion_quarantine",
            "data/parse/native_fulltext_composition.yaml#graph_mutation",
            "forbidden",
            fulltext.get("graph_mutation"),
        )
    if fulltext.get("rollback_target") != "java-grobid-http@0.9.0-crf":
        flag(
            "missing_rollback",
            "promotion_quarantine",
            "data/parse/native_fulltext_composition.yaml#rollback_target",
            "java-grobid-http@0.9.0-crf",
            fulltext.get("rollback_target"),
        )

    # S12/T02: named composition admission — the single candidate contour
    # keeps its exact identity, full four-receipt set, exact lanes and Java
    # rollback on live comparison DATA. An invented explicit decision is a
    # promotion receipt without a later user yes and fails closed as one.
    comparison_path = root / "data/parse/comparison_contract.yaml"
    try:
        comparison = yaml.safe_load(comparison_path.read_text(encoding="utf-8")) or {}
    except (OSError, yaml.YAMLError) as error:
        flag("acceptance_same_n_drift", "integration", "data/parse/comparison_contract.yaml", "valid YAML", str(error))
        comparison = {}
    shadow = comparison.get("native_header_reference_composition_shadow") or {}
    if not isinstance(shadow, dict) or not shadow:
        flag(
            "acceptance_same_n_drift",
            "integration",
            "data/parse/comparison_contract.yaml#native_header_reference_composition_shadow",
            "named candidate shadow",
            "missing",
        )
    else:
        if shadow.get("candidate_id") != "da-parse.native_header_reference_composition":
            flag(
                "family_promote_cap",
                "promotion_quarantine",
                "data/parse/comparison_contract.yaml#native_header_reference_composition_shadow.candidate_id",
                "da-parse.native_header_reference_composition",
                shadow.get("candidate_id"),
            )
        if (
            shadow.get("observation_mode") != "shadow"
            or shadow.get("promotion_state") != "candidate"
            or shadow.get("auto_promote") != "forbidden"
        ):
            flag(
                "family_promote_cap",
                "promotion_quarantine",
                "data/parse/comparison_contract.yaml#native_header_reference_composition_shadow.candidate_freeze",
                {"observation_mode": "shadow", "promotion_state": "candidate", "auto_promote": "forbidden"},
                {"observation_mode": shadow.get("observation_mode"), "promotion_state": shadow.get("promotion_state"), "auto_promote": shadow.get("auto_promote")},
            )
        if shadow.get("explicit_decision") is not None:
            flag(
                "promotion_receipt",
                "promotion_quarantine",
                "data/parse/comparison_contract.yaml#native_header_reference_composition_shadow.explicit_decision",
                None,
                shadow.get("explicit_decision"),
            )
        required = [str(item) for item in ((shadow.get("component_receipts") or {}).get("required") or [])]
        expected_required = ["native_date_shadow", "native_personal_name_shadow", "native_affiliation_shadow", "native_citation_shadow"]
        if required != expected_required:
            flag(
                "acceptance_same_n_drift",
                "integration",
                "data/parse/comparison_contract.yaml#native_header_reference_composition_shadow.component_receipts.required",
                expected_required,
                required,
            )
        lanes = [str(item) for item in (shadow.get("independently_selectable_lanes") or [])]
        if lanes != ["header", "references"]:
            flag(
                "body_java_fallback",
                "integration",
                "data/parse/comparison_contract.yaml#native_header_reference_composition_shadow.independently_selectable_lanes",
                ["header", "references"],
                lanes,
            )
        # M494/S01: observation lanes are never independently selectable, on
        # the comparison shadow either. strata.lanes may stay a superset.
        for family in ("document_segmentation", "figure", "table", "formula", "learning_policy"):
            if family in lanes:
                flag(
                    "body_java_fallback",
                    "integration",
                    f"data/parse/comparison_contract.yaml#native_header_reference_composition_shadow.independently_selectable_lanes.{family}",
                    "observation lane not selectable",
                    lanes,
                )
        # M494/S01: native fulltext composition shadow keeps the same
        # selectable allowlist so the two DATA files cannot drift. Observation
        # lanes (document_segmentation, figure, table, formula,
        # learning_policy) fail closed here; strata.lanes may stay a superset.
        fulltext_shadow = comparison.get("native_fulltext_composition_shadow") or {}
        if isinstance(fulltext_shadow, dict) and fulltext_shadow:
            # M495/S01: comparison shadow держит те же именованные знаменатели
            # и тот же java_lift_same_n, что composition DATA: citation 2198
            # не biblStruct, не пул с references 207. Тот же существующий флаг
            # acceptance_same_n_drift, нового violation label нет. Ключ
            # selectable здесь называется independently_selectable_lanes
            # (в composition — independently_selectable); оба ровно
            # [header, references] и проверяются ниже по своим именам.
            ft_denominators = (fulltext_shadow.get("source_evidence") or {}).get("s12_named_denominators") or {}
            ft_lift = ft_denominators.get("java_lift_same_n") or {}
            if (
                ft_denominators.get("citation_names_total") not in {2198, "2198"}
                or ft_denominators.get("references_total") not in {207, "207"}
                or ft_lift.get("java_header_authors_equals") != "header_raw_total"
                or ft_lift.get("java_biblstruct_equals") != "references_total"
                or ft_lift.get("hub_native_accepted_header") not in {20, "20"}
                or ft_lift.get("hub_java_header_raw") not in {21, "21"}
                or ft_lift.get("citation_names_not_biblstruct") is not True
                or ft_lift.get("pooling_accepted_with_raw") != "forbidden"
                or ft_lift.get("pooling_citation_names_with_references") != "forbidden"
            ):
                flag(
                    "acceptance_same_n_drift",
                    "integration",
                    "data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.source_evidence.s12_named_denominators.java_lift_same_n",
                    {
                        "citation_names_total": 2198,
                        "references_total": 207,
                        "java_header_authors_equals": "header_raw_total",
                        "java_biblstruct_equals": "references_total",
                        "hub_native_accepted_header": 20,
                        "hub_java_header_raw": 21,
                        "citation_names_not_biblstruct": True,
                        "pooling_accepted_with_raw": "forbidden",
                        "pooling_citation_names_with_references": "forbidden",
                    },
                    {"citation_names_total": ft_denominators.get("citation_names_total"), "references_total": ft_denominators.get("references_total"), **ft_lift},
                )
            ft_admission = fulltext_shadow.get("admission") or {}
            ft_lanes = [str(item) for item in (ft_admission.get("independently_selectable_lanes") or [])]
            if ft_lanes != ["header", "references"]:
                flag(
                    "body_java_fallback",
                    "integration",
                    "data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.admission.independently_selectable_lanes",
                    ["header", "references"],
                    ft_lanes,
                )
            for family in ("document_segmentation", "figure", "table", "formula", "learning_policy"):
                if family in ft_lanes:
                    flag(
                        "body_java_fallback",
                        "integration",
                        f"data/parse/comparison_contract.yaml#native_fulltext_composition_shadow.admission.independently_selectable_lanes.{family}",
                        "observation lane not selectable",
                        ft_lanes,
                    )
        lane_rollback = shadow.get("rollback") or {}
        for lane in ("header", "references"):
            if lane_rollback.get(lane) != "java-grobid-http@0.9.0-crf":
                flag(
                    "missing_rollback",
                    "promotion_quarantine",
                    f"data/parse/comparison_contract.yaml#native_header_reference_composition_shadow.rollback.{lane}",
                    "java-grobid-http@0.9.0-crf",
                    lane_rollback.get(lane),
                )

    # M511/S05: the composition identity surface now has two live artifacts —
    # the synthetic clone (fixture population) and the preserved-three-sha
    # corpus shadow (S05/T04). Both are byte-pinned above, so a rewritten
    # artifact is already `evidence_substitution`; these predicates catch a
    # semantically collapsed identity that keeps self-consistent-looking bytes
    # (three-sha corpus collapsing to one, or a preserved corpus re-labelled
    # synthetic_clone). The flag label always names the concrete artifact.
    preserved = {
        "985461235406db21e1c0f4d16f7c161afe817d1bd23909ae51ddade2b79e7ec4",
        "e6aa41f580d36f4996d993a64b51e132fc31938ad6562a9fe5c03b0548b285b3",
        "dac1888221aac5852654ead7f6a751f9cc5e48862a1579fe92cfeb57d0c6e040",
    }
    for composition_relative in (
        "artifacts/parse_compat/native_fulltext_composition_shadow.json",
        "artifacts/parse_compat/native_fulltext_corpus_shadow.json",
    ):
        try:
            composition_shadow = json.loads((root / composition_relative).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if not composition_shadow:
            continue
        identity = composition_shadow.get("source_identity") or {}
        outcomes = [row for row in (composition_shadow.get("outcomes") or []) if isinstance(row, dict)]
        shas = {str(row.get("source_sha256") or "") for row in outcomes}
        shas.discard("")
        if not shas and identity.get("source_sha256"):
            declared_shas = identity.get("source_sha256")
            if isinstance(declared_shas, (list, tuple)):
                shas = {str(item) for item in declared_shas if str(item)}
            else:
                shas = {str(declared_shas)}
        # The declared count is the artifact's own identity claim and is part of
        # the surface this check guards: a claim that disagrees with the measured
        # outcomes (three sha collapsing to one) is the drift shape, so the claim
        # is authoritative over the measured fallback when it is present.
        #
        # M511/S05/T07: the corpus shadow carries the GAR-006 population split,
        # so its declared document count lives in `reference_population`.  The
        # legacy `denominators` location is still read as a fallback for the
        # synthetic composition artifact (which has no denominators at all), and
        # a non-integer value degrades to 0 rather than raising: predicates 2
        # and 3 below stay the identity guards in that case.
        reference_population = composition_shadow.get("reference_population") or {}
        legacy_denominators = composition_shadow.get("denominators") or {}
        declared_documents = reference_population.get("documents")
        if not isinstance(declared_documents, int) or isinstance(declared_documents, bool):
            declared_documents = legacy_denominators.get("documents")
        documents = (
            declared_documents
            if isinstance(declared_documents, int) and not isinstance(declared_documents, bool)
            else 0
        )
        declared_unique = identity.get("unique_source_sha256")
        unique = declared_unique if isinstance(declared_unique, int) and not isinstance(declared_unique, bool) else len(shas)
        kind = identity.get("identity_kind")
        if documents > 1 and unique == 1 and kind != "synthetic_clone":
            flag(
                "acceptance_same_n_drift",
                "integration",
                f"{composition_relative}#source_identity",
                {"identity_kind": "synthetic_clone", "unique_source_sha256": 1},
                {"identity_kind": kind, "unique_source_sha256": unique, "documents": documents},
            )
        ids = " ".join(str(row.get("document_id") or "") for row in outcomes).lower()
        looks_preserved = bool(shas & preserved) or any(token in ids for token in ("hub", "rlm", "reca", "2507.19457", "2512.24601", "2605.26525"))
        if looks_preserved and kind != "preserved_three_sha":
            flag(
                "acceptance_same_n_drift",
                "integration",
                f"{composition_relative}#identity_kind",
                "preserved_three_sha",
                kind,
            )
        if kind == "preserved_three_sha" and unique != 3:
            flag(
                "acceptance_same_n_drift",
                "integration",
                f"{composition_relative}#unique_source_sha256",
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
    allowed_fsm = set(expected_parse) | {"none"}
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
                "subset of parse_pdf_consults or none",
                consults,
            )
        if set(consults) & set(expected_later):
            flag(
                "policy_stamp",
                "integration",
                f"data/parse/grobid_compatibility.yaml#defaults.cascade_stages.{sid}.fsm_consult",
                "later names must not appear as fsm_consult",
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
    p5 = stages.get("P5") or {}
    if str(p5.get("observe_mode") or "") != "production_stamp":
        flag(
            "policy_stamp",
            "integration",
            "data/parse/grobid_compatibility.yaml#defaults.cascade_stages.P5.observe_mode",
            "production_stamp",
            p5.get("observe_mode"),
        )
    # M498/S01: P5 has one production observation authority and one offline
    # native shadow surface. Keep this exact and fail closed: missing keys,
    # substituted authority, widened counters, or pooled denominators all
    # invalidate the policy without changing the generic stamp derivation.
    expected_p5 = {
        "grobid": "fulltext_body_sections",
        "production_authority": "java_http",
        "native_surface": "crates/da-parse/src/native_body.rs",
        "fsm_consult": ["none"],
        "observe_mode": "production_stamp",
        "java_observation": "production",
        "java_observation_fields": ["sections_len", "body_empty"],
        "empty_count_not_success": True,
        "native_accountant": "shadow_join_only",
        "pooling_forbidden_with": ["P2", "P6", "P7"],
    }
    if p5 != expected_p5:
        flag(
            "policy_stamp",
            "integration",
            "data/parse/grobid_compatibility.yaml#defaults.cascade_stages.P5",
            expected_p5,
            p5,
        )

    source_cli = root / "crates/da-cli/src/source_cli.rs"
    if source_cli.is_file():
        try:
            src = source_cli.read_text(encoding="utf-8")
        except OSError as error:
            src = str(error)
        for snake, pascal in _empty_native_consult_tokens(consult):
            hit_snake = snake in src
            hit_pascal = f"DecisionPoint::{pascal}" in src
            if hit_snake or hit_pascal:
                observed_hit = "DecisionPoint::" + pascal if hit_pascal else snake
                flag(
                    "policy_stamp",
                    "integration",
                    f"crates/da-cli/src/source_cli.rs#empty_native_unknown_point.{snake}",
                    f"empty-native DecisionPoint {snake} must stay UnknownPoint (no live parse-pdf consult)",
                    f"live consult {observed_hit} in parse-pdf source_cli",
                )
        for snake, pascal in _native_rules_not_consulted_tokens(consult):
            hit_snake = snake in src
            hit_pascal = f"DecisionPoint::{pascal}" in src
            if hit_snake or hit_pascal:
                observed_hit = "DecisionPoint::" + pascal if hit_pascal else snake
                flag(
                    "policy_stamp",
                    "integration",
                    f"crates/da-cli/src/source_cli.rs#native_rules_not_consulted.{snake}",
                    f"native-rules DecisionPoint {snake} is not a parse-pdf consult (sona-policy demo is not closure)",
                    f"live consult {observed_hit} in parse-pdf source_cli",
                )
        required_stamps, forbidden_stamps = _cascade_observe_tokens(stages)
        for token in required_stamps:
            if token not in src:
                flag(
                    "policy_stamp",
                    "integration",
                    f"crates/da-cli/src/source_cli.rs#{token.rstrip('_')}",
                    f"parse-pdf stamps {token.rstrip('_')} from cascade_stages observe_mode",
                    f"missing {token.rstrip('_')}",
                )
        for token in forbidden_stamps:
            if token in src:
                flag(
                    "policy_stamp",
                    "integration",
                    "crates/da-cli/src/source_cli.rs#parse-pdf",
                    "every production_stamp stage must be stamped as live",
                    f"{token.rstrip('_')} present",
                )
        if 'named_native_accepted_gap".into(), 20.into()' in src:
            flag(
                "acceptance_same_n_drift",
                "integration",
                "crates/da-cli/src/source_cli.rs#stamp_p3_header_observe",
                "named_native_accepted_gap true, hub 20 vs 21 stays in DATA",
                "named_native_accepted_gap 20",
            )
        if 'citation_names_not_biblstruct".into(), 2198.into()' in src or 'citation_names_not_biblstruct".into(),2198.into()' in src:
            flag(
                "acceptance_same_n_drift",
                "integration",
                "crates/da-cli/src/source_cli.rs#stamp_p4_references_observe",
                "citation_names_not_biblstruct true, citation-names 2198 stays off java_citation_n",
                "citation_names_not_biblstruct 2198",
            )
        if 'citation_names_not_biblstruct".into(), java_citation_n.into()' in src:
            flag(
                "acceptance_same_n_drift",
                "integration",
                "crates/da-cli/src/source_cli.rs#stamp_p4_references_observe",
                "citation_names_not_biblstruct true, citation-names stay off java_citation_n",
                "citation_names_not_biblstruct pooled with java_citation_n",
            )
        if "ConservationEngine" in src:
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/source_cli.rs#parse-pdf",
                "P8 observe must not call ConservationEngine",
                "conservation engine call",
            )
        if (
            "compose_header(" in src
            or "run_header_sequence(" in src
            or "compose_references(" in src
            or "run_reference_sequence(" in src
        ):
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/source_cli.rs#parse-pdf",
                "P2/P3/P4 observe must not call native compose",
                "native compose call",
            )
        if "NativeSegmenter" in src or "segment_document(" in src:
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/source_cli.rs#parse-pdf",
                "P2 observe must not call native segmentation",
                "native segmentation call",
            )
        if (
            "detect_figure_heads" in src
            or "detect_table_heads" in src
            or "detect_formula_heads" in src
            or "NativeVisual" in src
        ):
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/source_cli.rs#parse-pdf",
                "P6 observe must not call native visual heads",
                "native visual call",
            )
        if "LayoutProvider" in src or "LayoutDocument" in src or "pdfalto" in src:
            flag(
                "policy_stamp",
                "integration",
                "crates/da-cli/src/source_cli.rs#parse-pdf",
                "P1 observe must not call native layout",
                "native layout call",
            )

    defaults = (registry.get("defaults") or {}) if isinstance(registry, dict) else {}
    body_tokens, body_boundaries = _body_accounting_quarantine_spec(defaults)
    body_quarantine = defaults.get("body_accounting_quarantine") or {}
    if body_quarantine.get("production_authority") != "java_http" or body_quarantine.get("observe_mode") != "shadow_receipt":
        flag(
            "policy_stamp",
            "integration",
            "data/parse/grobid_compatibility.yaml#defaults.body_accounting_quarantine",
            {"production_authority": "java_http", "observe_mode": "shadow_receipt"},
            body_quarantine,
        )
    for relative in body_boundaries:
        path = root / relative
        if not path.is_file():
            continue
        try:
            boundary_src = path.read_text(encoding="utf-8")
        except OSError as error:
            boundary_src = str(error)
        hits = _body_accounting_quarantine_tokens_present(boundary_src, body_tokens)
        if hits:
            flag(
                "policy_stamp",
                "integration",
                f"{relative}#body_accounting_quarantine",
                "native body accountant remains shadow-only",
                hits,
            )

    fulltext_tokens, fulltext_boundaries = _fulltext_blocks_quarantine_spec(defaults)
    fulltext_quarantine = defaults.get("fulltext_blocks_quarantine") or {}
    if fulltext_quarantine.get("production_authority") != "java_http" or fulltext_quarantine.get("observe_mode") != "shadow_receipt":
        flag("policy_stamp", "integration", "data/parse/grobid_compatibility.yaml#defaults.fulltext_blocks_quarantine", {"production_authority": "java_http", "observe_mode": "shadow_receipt"}, fulltext_quarantine)
    for relative in fulltext_boundaries:
        path = root / relative
        files = sorted(path.rglob("*.rs")) if path.is_dir() else ([path] if path.is_file() else [])
        for file_path in files:
            try:
                boundary_src = file_path.read_text(encoding="utf-8")
            except OSError as error:
                boundary_src = str(error)
            hits = _fulltext_blocks_quarantine_tokens_present(boundary_src, fulltext_tokens)
            if hits:
                flag("policy_stamp", "integration", f"{file_path.relative_to(root)}#fulltext_blocks_quarantine", "S03 semantic companion remains shadow-only", hits)

    specialized_tokens, specialized_boundaries = _consolidation_specialized_quarantine_spec(defaults)
    specialized_quarantine = defaults.get("consolidation_specialized_quarantine") or {}
    expected_specialized = {"production_authority": "java_http", "observe_mode": "shadow_receipt", "production_route": "forbidden", "graph_write": "forbidden", "import_eligible": False, "auto_promote": "forbidden", "decision_point": 6, "independently_selectable": ["header", "references"]}
    for key, expected in expected_specialized.items():
        if specialized_quarantine.get(key) != expected:
            flag("policy_stamp", "integration", "data/parse/grobid_compatibility.yaml#defaults.consolidation_specialized_quarantine", expected, specialized_quarantine.get(key))
    for relative in specialized_boundaries:
        path = root / relative
        files = sorted(path.rglob("*.rs")) if path.is_dir() else ([path] if path.is_file() else [])
        for file_path in files:
            try:
                boundary_src = file_path.read_text(encoding="utf-8")
            except OSError as error:
                boundary_src = str(error)
            hits = _consolidation_specialized_quarantine_tokens_present(boundary_src, specialized_tokens)
            if hits:
                flag("policy_stamp", "integration", f"{file_path.relative_to(root)}#s18_consolidation_specialized_quarantine", "specialized companion remains shadow-only", hits)

    callout_tokens, callout_boundaries = _callout_matching_quarantine_spec(defaults)
    callout_quarantine = defaults.get("callout_matching_quarantine") or {}
    # The S04 matcher has its own quarantine. Do not fold it into S03: each
    # family must remain independently auditable and fail closed.
    if callout_quarantine.get("production_authority") != "java_http" or callout_quarantine.get("observe_mode") != "shadow_receipt":
        flag("policy_stamp", "integration", "data/parse/grobid_compatibility.yaml#defaults.callout_matching_quarantine", {"production_authority": "java_http", "observe_mode": "shadow_receipt"}, callout_quarantine)
    for relative in callout_boundaries:
        path = root / relative
        files = sorted(path.rglob("*.rs")) if path.is_dir() else ([path] if path.is_file() else [])
        for file_path in files:
            try:
                boundary_src = file_path.read_text(encoding="utf-8")
            except OSError as error:
                boundary_src = str(error)
            hits = _callout_matching_quarantine_tokens_present(boundary_src, callout_tokens)
            if hits:
                flag("policy_stamp", "integration", f"{file_path.relative_to(root)}#callout_matching_quarantine", "S04 matcher remains shadow-only", hits)

    feature_tokens, feature_boundaries = _feature_tagger_training_quarantine_spec(defaults)
    feature_quarantine = defaults.get("feature_tagger_training_quarantine") or {}
    expected_feature = {
        "production_authority": "java_http",
        "observe_mode": "shadow_receipt",
        "production_route": "forbidden",
        "graph_write": "forbidden",
        "import_eligible": False,
        "auto_promote": "forbidden",
        "decision_point": 6,
        "independently_selectable": ["header", "references"],
        "training_and_model_runtime": "forbidden",
        "same_n_required": True,
    }
    for key, expected in expected_feature.items():
        if feature_quarantine.get(key) != expected:
            flag("policy_stamp", "integration", "data/parse/grobid_compatibility.yaml#defaults.feature_tagger_training_quarantine", expected, feature_quarantine.get(key))
    if set(feature_quarantine.get("dispositions") or []) != {"port_structural_subset", "replace_runtime_abstraction", "defer_insufficient_evidence"}:
        flag("policy_stamp", "integration", "data/parse/grobid_compatibility.yaml#defaults.feature_tagger_training_quarantine.dispositions", "closed advisory dispositions", feature_quarantine.get("dispositions"))
    for relative in feature_boundaries:
        path = root / relative
        files = sorted(path.rglob("*.rs")) if path.is_dir() else ([path] if path.is_file() else [])
        for file_path in files:
            try:
                boundary_src = file_path.read_text(encoding="utf-8")
            except OSError as error:
                boundary_src = str(error)
            hits = _feature_tagger_training_quarantine_tokens_present(boundary_src, feature_tokens)
            if hits:
                flag("policy_stamp", "integration", f"{file_path.relative_to(root)}#feature_tagger_training_quarantine", "feature companion remains shadow-only", hits)

    real_tei = root / "crates/da-parse/src/real_tei_tests.rs"
    if real_tei.is_file():
        try:
            tei_src = real_tei.read_text(encoding="utf-8")
        except OSError as error:
            tei_src = str(error)
        if "sections.len() >= 80" in tei_src and "citations.len() >= 80" in tei_src:
            flag(
                "acceptance_same_n_drift",
                "integration",
                "crates/da-parse/src/real_tei_tests.rs#hub_sections",
                "hub Java sections 69, not biblStruct 80",
                "sections.len() >= 80 pooled with citations.len() >= 80",
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

    # M496/T02: the native body ledger is a closed, source-bound DATA
    # contract.  Do not infer labels or counts here; reject any mutation of
    # its explicit locks and independent-denominator semantics.
    ledger_relative = "data/parse/native_body_accounting.yaml"
    ledger_path = root / ledger_relative
    try:
        ledger = yaml.safe_load(ledger_path.read_text(encoding="utf-8"))
    except (OSError, yaml.YAMLError) as error:
        flag("ledger_surface", "integration", ledger_relative, "valid native-body-accounting.v1", str(error))
        ledger = None
    if isinstance(ledger, dict):
        if ledger.get("schema_version") != "native-body-accounting.v1":
            flag("schema_vocabulary_drift", "taxonomy", f"{ledger_relative}#schema_version", "native-body-accounting.v1", ledger.get("schema_version"))
        authority = (ledger.get("source_authority") or {}).get("evidence_artifact")
        if authority != "artifacts/parse_compat/java_grobid_body_taxonomy_source.json":
            flag("policy_stamp", "integration", f"{ledger_relative}#source_authority", "repo-owned Java taxonomy evidence", authority)
        locks = ledger.get("policy_locks") or {}
        expected_locks = {"observation_mode": "shadow", "promotion_state": "candidate", "auto_promote": "forbidden", "import_eligible": False, "graph_mutation": "forbidden", "raw_payloads": "forbidden", "production_authority": "java-grobid-http@0.9.0-crf"}
        for key, expected in expected_locks.items():
            if locks.get(key) != expected:
                flag("policy_stamp" if key == "production_authority" else "ledger_surface", "promotion_quarantine" if key in {"auto_promote", "import_eligible", "graph_mutation", "raw_payloads"} else "integration", f"{ledger_relative}#policy_locks.{key}", expected, locks.get(key))
        regions = (ledger.get("segmentation_regions") or {}).get("labels") or {}
        fulltext = (ledger.get("fulltext_labels") or {}).get("labels") or {}
        if (ledger.get("segmentation_regions") or {}).get("unknown_label_behavior") != "fail_closed" or (ledger.get("fulltext_labels") or {}).get("unknown_label_behavior") != "fail_closed":
            flag("schema_vocabulary_drift", "taxonomy", f"{ledger_relative}#unknown_label_behavior", "fail_closed", "open")
        families = ledger.get("families") or {}
        family_specs = [spec for name, spec in families.items() if name not in {"closed", "pooling"}]
        malformed_families = [name for name, spec in families.items() if name not in {"closed", "pooling"} and not isinstance(spec, dict)]
        if malformed_families or families.get("pooling") != "forbidden" or any(spec.get("pooling") != "forbidden" for spec in family_specs if isinstance(spec, dict)):
            flag("acceptance_same_n_drift", "integration", f"{ledger_relative}#families.pooling", "forbidden typed family specs", {"malformed": malformed_families, "pooling": families.get("pooling")})
        pending = ((ledger.get("denominators") or {}).get("pending_body_counts") or {})
        if set(pending) != set(families) - {"closed", "pooling"} or any(value != "pending_independent_oracle" for value in pending.values()):
            flag("acceptance_same_n_drift", "integration", f"{ledger_relative}#denominators.pending_body_counts", "all body families pending_independent_oracle", pending)
        if (ledger.get("exclusions") or {}).get("raw_payload_fields") and (ledger.get("exclusions") or {}).get("production_claims") != "forbidden":
            flag("ledger_surface", "integration", f"{ledger_relative}#exclusions.production_claims", "forbidden", (ledger.get("exclusions") or {}).get("production_claims"))
        if not regions or not fulltext:
            flag("schema_vocabulary_drift", "taxonomy", f"{ledger_relative}#labels", "closed label maps", "missing")

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
