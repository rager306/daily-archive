"""M514-1lss2s/S06/T02 — the native-route boundary, endpoint/consolidation scope
and the *prospective* operational limit freeze.

The S06 route is the first PDF->native path that ends in one typed document
result on the frozen 3-document slice with no Java and no prior TEI. Before any
benchmark may run, three things have to exist as DATA rather than be implied by
whatever the benchmark happened to do: the boundary of the route, the scholarly
endpoints it must stay compatible with (and the consolidation scope it runs),
and the ceilings it will be measured against (D509 — a ceiling chosen after the
numbers is not a ceiling).

This file proves that half is honest rather than optimistic:

* the Governor block declares the boundary *by pointer*, so it cannot drift from
  `data/parse/layout_bakeoff.yaml#java_free_vertical_slice` (ADR-077 §3) or from
  `defaults.cascade_stages` in `data/parse/grobid_compatibility.yaml`;
* the block restates neither the stage ids nor the document ids, and every one
  of its scholarly endpoints resolves to a real ledger entry by pointer with no
  URL copied into the rule;
* the consolidation scope is the structural profile with both flags off, the
  forbidden neighbour is `rich` (1/1) and the fingerprint field order equals the
  live `configuration_fingerprint` literal in `crates/da-parse/src/profile.rs`;
* the corpus counts recompute from the pinned documents (3 / 202) and agree with
  the manifest's own `corpus_n` / `page_denominator`;
* the limit policy is `frozen`, prospective (`tuned_after_scoring: forbidden`),
  carries a composite sha256 revision that *recomputes* from the pinned carrier
  bytes, and closes its metric and direction dictionaries;
* the diagnostic claim scope is declared (`claim_scope: diagnostic_only`,
  `product_coverage_effect: none`, `thresholds: not_accepted`), so no metric may
  claim a threshold while the policy says the threshold is not accepted;
* five named mutations — `freeze_after_scoring`, `unknown_metric`,
  `missing_freeze_revision`, `restated_corpus_count` and
  `threshold_claimed_while_not_accepted` — plus five more (restated stage list,
  unknown endpoint, shifted consolidation flag, second contract file, stale
  revision) each fail by name;
* the half adds no dimension and no severity of its own (D492).

The composite-arithmetic half is owned here because the Rust contract test at
`crates/da-parse/tests/native_route_boundary_contract.rs` checks the *shape* of
the revision and defers the recomputation to this gate. Nothing here executes a
subprocess, reads a network resource, or writes to the tree.
"""

from __future__ import annotations

import copy
import hashlib
import sys
from functools import lru_cache
from pathlib import Path
from typing import Any, Callable

import pytest
import yaml

_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
_REPO_ROOT = _SCRIPTS_DIR.parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from governor.adapters.absorption_progress_reader import (  # noqa: E402
    _UNRESOLVED,
    resolve_pointer,
)

RULE_PATH = "scripts/governor/checks/grobid_absorption_progress.yaml"
BLOCK = "native_route_e2e"
DECLARATION_MANIFEST = "data/parse/layout_bakeoff.yaml"
STAGE_ID_MANIFEST = "data/parse/grobid_compatibility.yaml"
LEDGER_MANIFEST = "data/parse/api_compatibility_ledger.yaml"
CEILING_MANIFEST = "data/parse/comparison_contract.yaml"
PROFILE_SURFACE = "crates/da-parse/src/profile.rs"
DIMENSIONS_PATH = "scripts/governor/checks/dimensions.yaml"
SEVERITY_PATH = "scripts/governor/checks/assurance_severity.yaml"

SCHEMA_VERSION = "native-route-e2e-rule.v1"
ENDPOINT_STATE_VOCABULARY = frozenset({"required", "not_required", "deferred"})
LIMIT_DIRECTION_VOCABULARY = frozenset({"floor", "ceiling"})
FREEZE_LINE_FORMAT = "<path>:<sha256-hex>"

#: Keys that would turn a diagnostic ceiling into a claimed threshold.
THRESHOLD_CLAIM_KEYS = frozenset(
    {
        "threshold",
        "threshold_ms",
        "threshold_value",
        "acceptance",
        "accepted",
        "attained",
    }
)

# The closed dictionary of reasons this half may report. A code that is emitted
# must be registered here, and every registered code is exercised by a named
# mutation below, so a silently dropped invariant is visible in this file.
V_BLOCK_MISSING = "native_route_block_missing"
V_SCHEMA = "native_route_schema_version_unregistered"
V_DECLARATION = "native_route_declaration_unbound"
V_AUTHORITY = "native_route_authority_not_sole_source"
V_STAGE_SOURCE = "native_route_stage_source_unbound"
V_STAGE_RESTATED = "native_route_stage_ids_restated"
V_CONSOLIDATION = "native_route_consolidation_scope_drifted"
V_FINGERPRINT = "native_route_fingerprint_fields_drifted"
V_ENDPOINT_VOCABULARY = "native_route_endpoint_vocabulary_open"
V_ENDPOINT_UNBOUND = "native_route_endpoint_unbound"
V_ENDPOINT_URL = "native_route_endpoint_url_restated"
V_CORPUS = "native_route_corpus_count_mismatch"
V_CORPUS_RESTATED = "native_route_document_ids_restated"
V_FREEZE_STATE = "native_route_freeze_not_prospective_frozen"
V_CLAIM_SCOPE = "native_route_claim_scope_widened"
V_FREEZE_REVISION = "native_route_freeze_revision_mismatch"
V_METRIC_UNKNOWN = "native_route_metric_unknown"
V_METRIC_DIRECTION = "native_route_metric_direction_unregistered"
V_THRESHOLD_CLAIMED = "native_route_threshold_claimed_while_not_accepted"
V_FREEZE_AFTER_SCORING = "native_route_tuned_after_scoring"
V_SILENT_CLAMP = "native_route_silent_clamp_allowed"
V_ZERO_MEASURED = "native_route_zero_is_a_measurement"
V_PASS_GATE = "native_route_pass_without_frozen_threshold"
V_CLAIM_WITHOUT_FREEZE = "native_route_claim_without_freeze"
V_COMPETING_DIMENSION = "native_route_competing_dimension"

NATIVE_ROUTE_VIOLATION_LABELS = frozenset(
    {
        V_BLOCK_MISSING,
        V_SCHEMA,
        V_DECLARATION,
        V_AUTHORITY,
        V_STAGE_SOURCE,
        V_STAGE_RESTATED,
        V_CONSOLIDATION,
        V_FINGERPRINT,
        V_ENDPOINT_VOCABULARY,
        V_ENDPOINT_UNBOUND,
        V_ENDPOINT_URL,
        V_CORPUS,
        V_CORPUS_RESTATED,
        V_FREEZE_STATE,
        V_CLAIM_SCOPE,
        V_FREEZE_REVISION,
        V_METRIC_UNKNOWN,
        V_METRIC_DIRECTION,
        V_THRESHOLD_CLAIMED,
        V_FREEZE_AFTER_SCORING,
        V_SILENT_CLAMP,
        V_ZERO_MEASURED,
        V_PASS_GATE,
        V_CLAIM_WITHOUT_FREEZE,
        V_COMPETING_DIMENSION,
    }
)


@lru_cache(maxsize=None)
def _load_yaml(rel: str) -> Any:
    return yaml.safe_load((_REPO_ROOT / rel).read_text(encoding="utf-8"))


@lru_cache(maxsize=1)
def _profile_src() -> str:
    return (_REPO_ROOT / PROFILE_SURFACE).read_text(encoding="utf-8")


def _resolve(document: Any, pointer: Any) -> Any:
    """Resolve a pointer in either dialect this DATA uses.

    The engine's own grammar (`resolve_pointer`) is slash-separated, so it is
    tried first; the layout-bakeoff/compatibility authority writes dot-separated
    pointers, so that form is the fallback. A pointer holding a selector such as
    ``entries[id=parse.endpoint.x]`` resolves on the first try and never reaches
    the dot fallback, so the dots inside the selector survive.
    """
    if not isinstance(pointer, str) or not pointer:
        return _UNRESOLVED
    resolved = resolve_pointer(document, pointer)
    if resolved is not _UNRESOLVED:
        return resolved
    if "." in pointer:
        return resolve_pointer(document, pointer.replace(".", "/"))
    return _UNRESOLVED


def _tokens(text: str) -> set[str]:
    found: set[str] = set()
    current = ""
    for char in text:
        if char.isalnum():
            current += char
        elif current:
            found.add(current)
            current = ""
    if current:
        found.add(current)
    return found


def _block_text(block: Any) -> str:
    return yaml.safe_dump(block, sort_keys=True)


def _fingerprint_field_order(profile_src: str) -> list[str]:
    """The field order of the frozen D204 fingerprint, read out of the live
    ``configuration_fingerprint`` literal rather than a second copy of the list."""
    marker = "pub fn configuration_fingerprint"
    start = profile_src.index(marker)
    rest = profile_src[start:]
    after = rest[rest.index("format!(") + len("format!(") :]
    literal = after[after.index('"') + 1 :]
    literal = literal[: literal.index('"')]
    fields: list[str] = []
    for piece in literal.split(";"):
        name = piece.split("=")[0].strip()
        if name:
            fields.append(name)
    return fields


def _recompute_freeze_revision(revision: Any) -> str | None:
    """Recompute ``sha256(join("<path>:<sha256-hex>", "\\n"))`` over the ordered
    carriers, or ``None`` when the declaration is not shaped for that."""
    if not isinstance(revision, dict):
        return None
    if revision.get("line_format") != FREEZE_LINE_FORMAT:
        return None
    carriers = revision.get("carriers")
    if not isinstance(carriers, list) or not carriers:
        return None
    lines: list[str] = []
    for carrier in carriers:
        if not isinstance(carrier, dict):
            return None
        path = carrier.get("path")
        if not isinstance(path, str) or not path or path.startswith("/"):
            return None
        target = _REPO_ROOT / path
        if not target.is_file():
            return None
        lines.append(f"{path}:{hashlib.sha256(target.read_bytes()).hexdigest()}")
    return "sha256:" + hashlib.sha256("\n".join(lines).encode("utf-8")).hexdigest()


def _restates_url(declaration: Any) -> bool:
    if not isinstance(declaration, dict):
        return False
    for key, value in declaration.items():
        if isinstance(key, str) and (
            key == "url" or key == "route" or key.startswith("http")
        ):
            return True
        if isinstance(value, str) and "http" in value:
            return True
    return False


def _metric_entries(metrics: Any) -> list[tuple[Any, dict | None]]:
    entries: list[tuple[Any, dict | None]] = []
    if not isinstance(metrics, list):
        return entries
    for raw in metrics:
        if isinstance(raw, str):
            entries.append((raw, None))
        elif isinstance(raw, dict):
            entries.append((raw.get("id"), raw))
        else:
            entries.append((None, None))
    return entries


class Fixtures:
    """The live rule + the live carriers it points at.

    ``rule`` and ``layout`` are copied so a mutation cannot leak into the module
    cache; the remaining carriers are read-only for every check here.
    """

    def __init__(self) -> None:
        self.rule = copy.deepcopy(_load_yaml(RULE_PATH))
        self.layout = copy.deepcopy(_load_yaml(DECLARATION_MANIFEST))
        self.compatibility = _load_yaml(STAGE_ID_MANIFEST)
        self.ledger = _load_yaml(LEDGER_MANIFEST)
        self.ceilings = _load_yaml(CEILING_MANIFEST)
        self.profile_src = _profile_src()

    @property
    def block(self) -> Any:
        return self.rule.get(BLOCK)

    @property
    def limit(self) -> Any:
        return (self.block or {}).get("limit_policy")


def native_route_freeze_violations(fixtures: Fixtures) -> list[str]:
    """The pure oracle for the native-route boundary / endpoint / freeze half."""
    violations: list[str] = []
    block = fixtures.block
    if not isinstance(block, dict):
        return [V_BLOCK_MISSING]

    if block.get("schema_version") != SCHEMA_VERSION:
        violations.append(V_SCHEMA)

    # 1. Boundary binding by pointer, and the slice stays the sole source.
    declaration = block.get("declaration") or {}
    if (
        declaration.get("manifest") != DECLARATION_MANIFEST
        or declaration.get("pointer") != "java_free_vertical_slice"
        or not isinstance(
            _resolve(fixtures.layout, declaration.get("pointer")), dict
        )
    ):
        violations.append(V_DECLARATION)
    authority = _resolve(fixtures.layout, "java_free_vertical_slice.authority")
    if (
        not isinstance(authority, dict)
        or authority.get("declaration_manifest") != DECLARATION_MANIFEST
        or authority.get("declaration_pointer") != "java_free_vertical_slice"
        or authority.get("sole_source") is not True
        or authority.get("second_contract_file") != "forbidden"
    ):
        violations.append(V_AUTHORITY)

    # 2. Stage ids resolve by pointer and are never restated inside the block.
    stage_source = block.get("stage_id_source") or {}
    cascade = _resolve(fixtures.compatibility, stage_source.get("pointer"))
    if (
        stage_source.get("manifest") != STAGE_ID_MANIFEST
        or stage_source.get("pointer") != "defaults.cascade_stages"
        or not isinstance(cascade, dict)
        or not cascade
    ):
        violations.append(V_STAGE_SOURCE)
        cascade_ids: set[str] = set()
    else:
        cascade_ids = {str(key) for key in cascade}
    if cascade_ids & _tokens(_block_text(block)):
        violations.append(V_STAGE_RESTATED)

    # 3. Consolidation scope: structural, both flags off, fingerprint order live.
    consolidation = block.get("consolidation") or {}
    resolved_profile = _resolve(fixtures.compatibility, consolidation.get("pointer"))
    if (
        consolidation.get("manifest") != STAGE_ID_MANIFEST
        or consolidation.get("pointer") != "defaults.profiles.structural"
        or consolidation.get("profile") != "structural"
        or consolidation.get("consolidateHeader") != 0
        or consolidation.get("consolidateCitations") != 0
        or not isinstance(resolved_profile, dict)
        or resolved_profile.get("consolidateHeader") != 0
        or resolved_profile.get("consolidateCitations") != 0
    ):
        violations.append(V_CONSOLIDATION)
    if list(consolidation.get("fingerprint_fields") or []) != _fingerprint_field_order(
        fixtures.profile_src
    ):
        violations.append(V_FINGERPRINT)
    rich = _resolve(fixtures.compatibility, "defaults.profiles.rich")
    if (
        consolidation.get("forbidden_profile") != "rich"
        or not isinstance(rich, dict)
        or rich.get("consolidateHeader") != 1
        or rich.get("consolidateCitations") != 1
    ):
        violations.append(V_CONSOLIDATION)

    # 4. Endpoints: closed vocabulary, pointer-bound, no URL restatement.
    endpoints = block.get("scholarly_endpoints") or {}
    vocabulary = set(endpoints.get("state_vocabulary") or [])
    if vocabulary != set(ENDPOINT_STATE_VOCABULARY):
        violations.append(V_ENDPOINT_VOCABULARY)
    seen_endpoints: set[Any] = set()
    for entry in endpoints.get("declarations") or []:
        if not isinstance(entry, dict):
            violations.append(V_ENDPOINT_UNBOUND)
            continue
        endpoint_id = entry.get("endpoint_id")
        if endpoint_id in seen_endpoints:
            violations.append(V_ENDPOINT_UNBOUND)
        seen_endpoints.add(endpoint_id)
        if entry.get("state") not in vocabulary:
            violations.append(V_ENDPOINT_VOCABULARY)
        if not str(entry.get("reason") or "").strip():
            violations.append(V_ENDPOINT_UNBOUND)
        manifest = entry.get("manifest")
        if manifest != LEDGER_MANIFEST or not (_REPO_ROOT / str(manifest)).is_file():
            violations.append(V_ENDPOINT_UNBOUND)
            continue
        resolved = _resolve(fixtures.ledger, entry.get("pointer"))
        if not isinstance(resolved, dict) or resolved.get("id") != endpoint_id:
            violations.append(V_ENDPOINT_UNBOUND)
        if _restates_url(entry):
            violations.append(V_ENDPOINT_URL)

    # 5. Corpus counts recompute from the pinned documents; ids are not restated.
    corpus = block.get("corpus") or {}
    documents = _resolve(fixtures.layout, corpus.get("pointer"))
    if isinstance(documents, list):
        count = len(documents)
        pages = sum(
            doc.get("pages", 0) for doc in documents if isinstance(doc, dict)
        )
    else:
        count = pages = None
    if (
        corpus.get("manifest") != DECLARATION_MANIFEST
        or corpus.get("pointer") != "native_route_corpus.documents"
        or count != corpus.get("document_count")
        or pages != corpus.get("page_denominator")
        or _resolve(fixtures.layout, corpus.get("count_pointer"))
        != corpus.get("document_count")
        or _resolve(fixtures.layout, corpus.get("page_denominator_pointer"))
        != corpus.get("page_denominator")
    ):
        violations.append(V_CORPUS)
    document_ids = {
        doc.get("id") for doc in documents or [] if isinstance(doc, dict)
    }
    if any(doc_id and doc_id in _block_text(block) for doc_id in document_ids):
        violations.append(V_CORPUS_RESTATED)

    # 6. The prospective, diagnostic freeze.
    limit = block.get("limit_policy") or {}
    if (
        limit.get("state") != "frozen"
        or not str(limit.get("frozen_at") or "").strip()
        or limit.get("freeze_owner") != "S06"
        or limit.get("freeze_scope") != "prospective_diagnostic_ceiling"
    ):
        violations.append(V_FREEZE_STATE)
    if (
        limit.get("claim_scope") != "diagnostic_only"
        or limit.get("product_coverage_effect") != "none"
        or limit.get("thresholds") != "not_accepted"
    ):
        violations.append(V_CLAIM_SCOPE)
    revision = limit.get("freeze_revision")
    if (
        not isinstance(revision, dict)
        or revision.get("kind") != "composite_sha256"
        or revision.get("rule") != "sha256_over_ordered_carrier_lines"
        or _recompute_freeze_revision(revision) != revision.get("value")
    ):
        violations.append(V_FREEZE_REVISION)

    ceiling_source = limit.get("ceiling_metric_source") or {}
    ceiling_metrics = _resolve(fixtures.ceilings, ceiling_source.get("pointer"))
    if (
        ceiling_source.get("manifest") != CEILING_MANIFEST
        or not isinstance(ceiling_metrics, list)
        or not ceiling_metrics
    ):
        violations.append(V_METRIC_UNKNOWN)
        ceiling_metrics = []
    route_metrics = limit.get("route_metrics") or []
    route_ids = {
        entry.get("id") for entry in route_metrics if isinstance(entry, dict)
    }
    metric_entries = _metric_entries(limit.get("metrics"))
    metric_ids = {metric_id for metric_id, _ in metric_entries}
    allowed = set(ceiling_metrics) | route_ids
    for metric_id, payload in metric_entries:
        if metric_id not in allowed:
            violations.append(V_METRIC_UNKNOWN)
        if payload and set(payload) & THRESHOLD_CLAIM_KEYS:
            violations.append(V_THRESHOLD_CLAIMED)
    # Every ceiling name is reused and every route counter is declared: a dropped
    # name would silently shrink the frozen vocabulary.
    if not set(ceiling_metrics) <= metric_ids:
        violations.append(V_METRIC_UNKNOWN)
    direction_vocabulary = set(limit.get("direction_vocabulary") or [])
    if direction_vocabulary != set(LIMIT_DIRECTION_VOCABULARY):
        violations.append(V_METRIC_DIRECTION)
    for entry in route_metrics:
        if not isinstance(entry, dict):
            violations.append(V_METRIC_DIRECTION)
            continue
        if entry.get("id") not in metric_ids:
            violations.append(V_METRIC_UNKNOWN)
        if entry.get("direction") not in direction_vocabulary:
            violations.append(V_METRIC_DIRECTION)

    if limit.get("tuned_after_scoring") != "forbidden":
        violations.append(V_FREEZE_AFTER_SCORING)
    if limit.get("silent_clamp") != "forbidden":
        violations.append(V_SILENT_CLAMP)
    if limit.get("zero_is_not_a_measurement") is not True:
        violations.append(V_ZERO_MEASURED)
    if limit.get("pass_requires_frozen_threshold") is not True:
        violations.append(V_PASS_GATE)
    if limit.get("claim_without_freeze") != "forbidden":
        violations.append(V_CLAIM_WITHOUT_FREEZE)

    if "dimension" in block or "severity" in block:
        violations.append(V_COMPETING_DIMENSION)

    return violations


# ──────────────────────────────── tests ────────────────────────────────────


def test_live_block_is_clean_and_bound_to_the_live_carriers() -> None:
    fixtures = Fixtures()
    assert fixtures.block is not None
    assert native_route_freeze_violations(fixtures) == []

    documents = _resolve(fixtures.layout, fixtures.block["corpus"]["pointer"])
    assert isinstance(documents, list) and len(documents) == 12
    assert sum(doc["pages"] for doc in documents) == 381
    assert {doc["id"] for doc in documents} == {
        "hub-2507.19457",
        "rlm-2512.24601",
        "reca-2605.26525v1",
        "1409.0473",
        "1410.4615",
        "1410.5401",
        "1506.02438",
        "2402.04268",
        "2402.10980",
        "2406.13163",
        "1203.2295",
        "2502.05199",
    }


def test_every_block_pointer_resolves_against_its_declared_manifest() -> None:
    fixtures = Fixtures()
    block = fixtures.block
    assert _resolve(fixtures.layout, block["declaration"]["pointer"]) is not _UNRESOLVED
    assert (
        _resolve(fixtures.compatibility, block["stage_id_source"]["pointer"])
        is not _UNRESOLVED
    )
    assert (
        _resolve(fixtures.compatibility, block["consolidation"]["pointer"])
        is not _UNRESOLVED
    )
    corpus = block["corpus"]
    for pointer in (
        corpus["pointer"],
        corpus["count_pointer"],
        corpus["page_denominator_pointer"],
    ):
        assert _resolve(fixtures.layout, pointer) is not _UNRESOLVED, pointer
    assert (
        _resolve(fixtures.ceilings, block["limit_policy"]["ceiling_metric_source"]["pointer"])
        is not _UNRESOLVED
    )
    for entry in block["scholarly_endpoints"]["declarations"]:
        resolved = _resolve(fixtures.ledger, entry["pointer"])
        assert isinstance(resolved, dict), entry["endpoint_id"]
        assert resolved["id"] == entry["endpoint_id"]


def test_freeze_revision_recomputes_from_the_pinned_carrier_bytes() -> None:
    fixtures = Fixtures()
    revision = fixtures.block["limit_policy"]["freeze_revision"]
    assert revision["kind"] == "composite_sha256"
    assert revision["rule"] == "sha256_over_ordered_carrier_lines"
    assert revision["line_format"] == FREEZE_LINE_FORMAT
    assert [carrier["path"] for carrier in revision["carriers"]] == [
        "data/parse/layout_bakeoff.yaml",
        "data/parse/comparison_contract.yaml",
        "data/parse/native_fulltext_composition.yaml",
        "crates/da-parse/tests/fixtures/preserved/provenance.yaml",
    ]
    assert _recompute_freeze_revision(revision) == revision["value"]


def test_the_block_restates_neither_stage_ids_nor_document_ids() -> None:
    fixtures = Fixtures()
    block = fixtures.block
    cascade = _resolve(fixtures.compatibility, block["stage_id_source"]["pointer"])
    assert set(cascade) == {f"P{i}" for i in range(10)}
    assert not (set(cascade) & _tokens(_block_text(block)))

    documents = _resolve(fixtures.layout, block["corpus"]["pointer"])
    text = _block_text(block)
    for document in documents:
        assert document["id"] not in text
        assert str(document["sha256"]) not in text


def test_consolidation_scope_matches_the_live_profile_surface() -> None:
    fixtures = Fixtures()
    consolidation = fixtures.block["consolidation"]
    assert consolidation["profile"] == "structural"
    assert consolidation["forbidden_profile"] == "rich"
    assert _fingerprint_field_order(fixtures.profile_src) == [
        "profile",
        "consolidateHeader",
        "consolidateCitations",
        "teiCoordinates",
    ]
    assert consolidation["fingerprint_fields"] == _fingerprint_field_order(
        fixtures.profile_src
    )


def test_closed_dictionaries_are_closed() -> None:
    fixtures = Fixtures()
    endpoints = fixtures.block["scholarly_endpoints"]
    assert set(endpoints["state_vocabulary"]) == set(ENDPOINT_STATE_VOCABULARY)
    limit = fixtures.block["limit_policy"]
    assert set(limit["direction_vocabulary"]) == set(LIMIT_DIRECTION_VOCABULARY)
    # the frozen metric vocabulary is exactly the ceiling dictionary (resolved by
    # pointer from comparison_contract.yaml) plus the five route counters: a name
    # outside it would be an unfrozen metric, a dropped name an unfrozen silence.
    ceilings = _resolve(fixtures.ceilings, limit["ceiling_metric_source"]["pointer"])
    route_ids = {entry["id"] for entry in limit["route_metrics"]}
    assert set(limit["metrics"]) == set(ceilings) | route_ids
    assert native_route_freeze_violations(fixtures) == []


def test_the_freeze_half_registers_no_competing_dimension() -> None:
    fixtures = Fixtures()
    assert "dimension" not in fixtures.block
    assert "severity" not in fixtures.block
    for rel in (DIMENSIONS_PATH, SEVERITY_PATH):
        text = (_REPO_ROOT / rel).read_text(encoding="utf-8")
        assert "native_route" not in text, rel


def test_claim_scope_stays_diagnostic() -> None:
    limit = Fixtures().block["limit_policy"]
    assert limit["claim_scope"] == "diagnostic_only"
    assert limit["product_coverage_effect"] == "none"
    assert limit["thresholds"] == "not_accepted"
    assert limit["pass_requires_frozen_threshold"] is True
    assert limit["claim_without_freeze"] == "forbidden"


# ── named negative mutations ───────────────────────────────────────────────


def _mut_freeze_after_scoring(fixtures: Fixtures) -> None:
    fixtures.limit["tuned_after_scoring"] = "allowed"


def _mut_unknown_metric(fixtures: Fixtures) -> None:
    fixtures.limit["metrics"].append("native_route_unknown_metric")


def _mut_missing_freeze_revision(fixtures: Fixtures) -> None:
    del fixtures.limit["freeze_revision"]


def _mut_restated_corpus_count(fixtures: Fixtures) -> None:
    fixtures.block["corpus"]["document_count"] = 5


def _mut_threshold_claimed_while_not_accepted(fixtures: Fixtures) -> None:
    fixtures.limit["metrics"][0] = {"id": "latency_p50_ms", "threshold": 5}


def _mut_restated_stage_list(fixtures: Fixtures) -> None:
    fixtures.block["stages"] = ["P1", "P2"]


def _mut_unknown_endpoint(fixtures: Fixtures) -> None:
    fixtures.block["scholarly_endpoints"]["declarations"][0]["endpoint_id"] = (
        "parse.endpoint.process_unknown"
    )


def _mut_shifted_consolidation_flag(fixtures: Fixtures) -> None:
    fixtures.block["consolidation"]["consolidateHeader"] = 1


def _mut_second_contract_file(fixtures: Fixtures) -> None:
    fixtures.layout["java_free_vertical_slice"]["authority"][
        "second_contract_file"
    ] = "allowed"


def _mut_stale_freeze_revision(fixtures: Fixtures) -> None:
    fixtures.limit["freeze_revision"]["value"] = "sha256:" + "0" * 64


MUTATIONS: list[tuple[str, Callable[[Fixtures], None], str]] = [
    ("freeze_after_scoring", _mut_freeze_after_scoring, V_FREEZE_AFTER_SCORING),
    ("unknown_metric", _mut_unknown_metric, V_METRIC_UNKNOWN),
    ("missing_freeze_revision", _mut_missing_freeze_revision, V_FREEZE_REVISION),
    ("restated_corpus_count", _mut_restated_corpus_count, V_CORPUS),
    (
        "threshold_claimed_while_not_accepted",
        _mut_threshold_claimed_while_not_accepted,
        V_THRESHOLD_CLAIMED,
    ),
    ("restated_stage_list", _mut_restated_stage_list, V_STAGE_RESTATED),
    ("unknown_endpoint", _mut_unknown_endpoint, V_ENDPOINT_UNBOUND),
    ("shifted_consolidation_flag", _mut_shifted_consolidation_flag, V_CONSOLIDATION),
    ("second_contract_file", _mut_second_contract_file, V_AUTHORITY),
    ("stale_freeze_revision", _mut_stale_freeze_revision, V_FREEZE_REVISION),
]

_MUTATION_IDS = [name for name, _, _ in MUTATIONS]


@pytest.mark.parametrize(
    ("name", "mutate", "expected"), MUTATIONS, ids=_MUTATION_IDS
)
def test_named_mutations_are_rejected_by_name(
    name: str, mutate: Callable[[Fixtures], None], expected: str
) -> None:
    fixtures = Fixtures()
    assert native_route_freeze_violations(fixtures) == [], "the control must be clean"
    mutate(fixtures)
    violations = native_route_freeze_violations(fixtures)
    assert expected in violations, (name, violations)


def test_the_oracle_emits_only_registered_labels() -> None:
    observed: set[str] = set()
    for _, mutate, _ in MUTATIONS:
        fixtures = Fixtures()
        mutate(fixtures)
        observed |= set(native_route_freeze_violations(fixtures))
    assert observed, "the mutation matrix must exercise the oracle"
    assert observed <= set(NATIVE_ROUTE_VIOLATION_LABELS)
    # Every label the matrix names is registered, and every mutation names one.
    assert {expected for _, _, expected in MUTATIONS} <= set(NATIVE_ROUTE_VIOLATION_LABELS)


def test_a_dropped_block_fails_closed() -> None:
    fixtures = Fixtures()
    del fixtures.rule[BLOCK]
    assert native_route_freeze_violations(fixtures) == [V_BLOCK_MISSING]
