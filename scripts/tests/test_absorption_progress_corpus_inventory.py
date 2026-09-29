"""M514-1lss2s/S02/T04 — local corpus inventory, licence provenance and the external byte boundary.

T01 froze the product scope and the two-corpus split as DATA, T02 froze the
thresholds and bound them to the carrier revision, and T03 published a derived
per-document census. This module is the executable guard for the companion
*corpus inventory record*
(``doc/review/grobid-absorption-s02-corpus-inventory.md``): it re-derives the
counts and the disjointness of the two local corpora from the frozen tracked
sources on every run, so the recorded facts cannot rot under a stale prose row.

What it re-derives, and from where:

* the evaluation population — ``data/parse/holdout_manifests.yaml``: the 610
  assigned identities, their declared ``bytes_present`` state, and the per
  document byte pin (``sha256`` + ``size_bytes``);
* a second, independently produced frozen source for the same 610 identities —
  ``artifacts/parse_compat/external_holdout_release_probe.json``
  (``reference_population.byte_pins``), plus the probe's own filesystem census
  (``production_catalog_overlap``) and its binding to the live manifest bytes;
* the tuning population — ``data/gold_standard/*.gold.json`` (200 identities,
  each file's ``paper_id`` equal to its file identity);
* the disjointness of the two — by arXiv document identity, never by prompt hash;
* the production-catalog overlap — re-derived from the live tree and asserted
  against the declared ``0``;
* the external byte boundary — the probe band and the contract's
  ``independent_corpus.external`` block, which must agree with each other;
* the D491 boundary — re-derived from ``data/parse/research_authorization.yaml``
  and ``data/parse/license_inventory.yaml``, never from this repository's prose.

Boundary (stated, not implied): the corpus PDFs under ``data/sources/arxiv/`` are
gitignored, so this guard binds the *tracked declarations of presence* — the
manifest's ``status: bytes_present`` for all 610, the per-document byte pin, and
the probe's ``declared_pdf_present_count: 610`` /
``absent_from_every_locator_count: 0``. The byte-level measurement of the local
tree (every declared PDF present with a matching size) is recorded as a measured
fact in the report; it cannot be a CI-stable executable check, and pretending
otherwise would make this suite depend on a gitignored 2.78 GB local corpus.

The record itself is prose and is deliberately not parsed here: the guard's value
is that any change to a tracked source changes what the record must say, so a
stale record is detected by its reader rather than by a regex over its wording.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

import pytest
import yaml

_REPO_ROOT = Path(__file__).resolve().parents[2]

MANIFEST_PATH = "data/parse/holdout_manifests.yaml"
GOLD_DIR = "data/gold_standard"
GOLD_SUFFIX = ".gold.json"
PROBE_PATH = "artifacts/parse_compat/external_holdout_release_probe.json"
SCHEMA_PATH = "data/parse/absorption_progress.schema.yaml"
LICENSE_INVENTORY_PATH = "data/parse/license_inventory.yaml"
RESEARCH_AUTHORIZATION_PATH = "data/parse/research_authorization.yaml"
REPORT_PATH = "doc/review/grobid-absorption-s02-corpus-inventory.md"
CANONICAL_CATALOG_ROOT = "data/article_catalog/article_catalog/arxiv"

# The frozen numbers the record states. Written as literals so a re-cut corpus is
# a diff here rather than a silently accepted new population.
CENSUS = {
    "evaluation_documents": 610,
    "tuning_documents": 200,
    "measurable": 531,
    "excluded": 79,
    "production_catalog_overlap": 0,
}

_SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
_HEX64_RE = re.compile(r"^[0-9a-f]{64}$")


# ── loaders (fail closed on absence or malformed structure) ────────────────


def _load_json(relative_path: str):
    return json.loads((_REPO_ROOT / relative_path).read_text(encoding="utf-8"))


def _load_yaml(relative_path: str):
    return yaml.safe_load((_REPO_ROOT / relative_path).read_text(encoding="utf-8"))


def _sha256(relative_path: str) -> str:
    return (
        "sha256:"
        + hashlib.sha256((_REPO_ROOT / relative_path).read_bytes()).hexdigest()
    )


def _require_list(document, key: str, where: str) -> list:
    """The non-empty list at ``key``, or a hard error.

    Returning an empty list for a missing or malformed block would let a
    truncated carrier read as a clean, empty population; a derivation that
    cannot see its input must fail, not silently shrink the corpus.
    """
    if not isinstance(document, dict):
        raise ValueError(f"{where}: expected a mapping")
    value = document.get(key)
    if not isinstance(value, list) or not value:
        raise ValueError(f"{where}: {key} must be a non-empty list")
    return value


def _wellformed_sha256(value) -> bool:
    return isinstance(value, str) and bool(_SHA256_RE.match(value))


def _wellformed_hex64(value) -> bool:
    """A bare 64 hex digest, the form the probe's per-pin identity digest uses."""
    return isinstance(value, str) and bool(_HEX64_RE.match(value))


def _identity_overlap(left, right) -> set:
    return set(left) & set(right)


def _canonical_catalog_matches(root: Path, identities) -> list:
    """Holdout identities present in the canonical production catalog.

    Mirrors the probe's own locator rule: an identity is present when
    ``<root>/<category>/<id>/source/<id>.pdf`` exists for some category. A root
    that does not exist yields no matches, which is the honest answer — the
    canonical catalog currently holds none of the 610.
    """
    if not root.is_dir():
        return []
    categories = [entry.name for entry in root.iterdir() if entry.is_dir()]
    return sorted(
        identity
        for identity in identities
        if any(
            (root / category / identity / "source" / f"{identity}.pdf").is_file()
            for category in categories
        )
    )


# ── live-tree derivations ──────────────────────────────────────────────────


def _manifest() -> dict:
    return _load_yaml(MANIFEST_PATH)


def _articles() -> list:
    return _require_list(_manifest(), "articles", MANIFEST_PATH)


def _evaluation_identities() -> list:
    return [article["article_id"] for article in _articles()]


def _gold_identity(path: Path) -> str:
    """The document identity of one gold file, or a hard error on a mismatch."""
    identity = path.name[: -len(GOLD_SUFFIX)]
    record = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(record, dict) or record.get("paper_id") != identity:
        raise ValueError(f"{path.name}: file identity is not the document identity")
    return identity


def _gold_identities() -> list:
    return [
        _gold_identity(path)
        for path in sorted((_REPO_ROOT / GOLD_DIR).glob(f"*{GOLD_SUFFIX}"))
    ]


def _probe() -> dict:
    return _load_json(PROBE_PATH)


def _contract() -> dict:
    return _load_yaml(SCHEMA_PATH)


# ── measured facts: the evaluation corpus ──────────────────────────────────


def test_evaluation_population_is_610_unique_identities_all_bytes_present() -> None:
    articles = _articles()
    identities = [article["article_id"] for article in articles]
    assert len(identities) == CENSUS["evaluation_documents"]
    assert len(set(identities)) == len(identities), "a duplicate would inflate the census"
    population = _contract()["assigned_population"]
    assert population["declared"] == CENSUS["evaluation_documents"]
    assert population["manifest"]["path"] == MANIFEST_PATH
    present = population["manifest"]["present_value"]
    assert present == "bytes_present"
    assert all(article["status"] == present for article in articles)


def test_evaluation_identity_byte_pins_are_wellformed() -> None:
    for article in _articles():
        assert _wellformed_sha256(article["sha256"]), article["article_id"]
        assert isinstance(article["size_bytes"], int) and article["size_bytes"] > 0
        assert article["pdf_path"].startswith("data/")


def test_probe_byte_pins_name_the_same_610_identities() -> None:
    pins = _require_list(_probe()["reference_population"], "byte_pins", PROBE_PATH)
    pinned = [pin["document_id"] for pin in pins]
    assert len(pinned) == CENSUS["evaluation_documents"]
    assert len(set(pinned)) == len(pinned)
    assert set(pinned) == set(_evaluation_identities())
    for pin in pins:
        assert _wellformed_hex64(pin["identity_sha256"]), pin["document_id"]
        assert isinstance(pin["size_bytes"], int) and pin["size_bytes"] > 0


def test_probe_binds_the_live_manifest_bytes() -> None:
    reference = _probe()["reference_population"]
    assert reference["source_manifest"] == MANIFEST_PATH
    assert reference["source_manifest_sha256"] == _sha256(MANIFEST_PATH)
    assert reference["documents"] == CENSUS["evaluation_documents"]
    assert reference["measurable_documents"] == CENSUS["measurable"]
    assert reference["excluded_documents"] == CENSUS["excluded"]


def test_probe_declares_every_assigned_pdf_present_locally() -> None:
    census = _probe()["production_catalog_overlap"]
    assert census["documents"] == CENSUS["evaluation_documents"]
    assert census["declared_pdf_present_count"] == CENSUS["evaluation_documents"]
    assert census["absent_from_every_locator_count"] == 0
    locators = _require_list(census, "locators", f"{PROBE_PATH}#production_catalog_overlap")
    assert len(locators) == CENSUS["evaluation_documents"]
    assert all(row["declared_pdf_present"] is True for row in locators)
    # The frozen probe already saw the disjointness on the filesystem.
    assert census["gold_standard_present_count"] == 0


# ── measured facts: the tuning corpus and the split ────────────────────────


def test_tuning_population_is_200_gold_documents() -> None:
    identities = _gold_identities()
    assert len(identities) == CENSUS["tuning_documents"]
    assert len(set(identities)) == len(identities)
    corpus = _contract()["independent_corpus"]
    assert corpus["tuning"]["glob"] == f"{GOLD_DIR}/*{GOLD_SUFFIX}"
    assert corpus["tuning"]["corpus_id"] == "gold-standard-200"


def test_corpora_are_disjoint_by_document_identity() -> None:
    overlap = _identity_overlap(_evaluation_identities(), _gold_identities())
    assert overlap == set()
    corpus = _contract()["independent_corpus"]
    split = _contract()["split_contract"]
    assert corpus["evaluation"]["manifest"] == MANIFEST_PATH
    assert corpus["evaluation"]["bytes_state"] == "bytes_present"
    assert corpus["disjointness"]["by"] == "document_identity"
    assert split["evaluation_source"] == MANIFEST_PATH
    assert split["tuning_source"] == f"{GOLD_DIR}/*{GOLD_SUFFIX}"
    assert split["evaluation_never_tuning_input"] is True
    assert split["reuse_forbidden"] is True


def test_split_is_declared_by_document_identity_not_prompt_hash() -> None:
    corpus = _contract()["independent_corpus"]
    split = _contract()["split_contract"]
    assert corpus["disjointness"]["prompt_hash_forbidden"] is True
    assert split["prompt_hash_disjointness_forbidden"] is True
    assert split["disjointness_identity"] == corpus["disjointness"]["identity_field"]
    assert split["disjointness_identity"] == "arxiv_id"


# ── measured facts: production catalog and the external band ───────────────


def test_production_catalog_overlap_is_zero_on_the_live_tree() -> None:
    """The holdout is not part of the production catalog, re-derived from disk.

    The declared `0` is a fact about this slice's corpus: a holdout identity that
    later enters the canonical catalog makes the declaration stale (the
    independent corpus would be production material), so the guard fails loudly
    and forces a deliberate revision rather than a silent overlap.
    """
    live = _canonical_catalog_matches(
        _REPO_ROOT / CANONICAL_CATALOG_ROOT, _evaluation_identities()
    )
    assert live == []
    assert len(live) == CENSUS["production_catalog_overlap"]
    corpus = _contract()["independent_corpus"]
    assert corpus["disjointness"]["production_catalog_overlap"] == 0
    census = _probe()["production_catalog_overlap"]
    assert census["canonical_catalog_present_count"] == 0
    assert census["census_kind"] == "filesystem_presence_of_the_holdout_identities"


def test_external_holdout_band_stays_bytes_absent_and_not_evidence() -> None:
    band = _probe()["external_holdout_band"]
    assert band["external_bytes"] == "bytes_absent"
    assert band["execution_status"] == "not_executed"
    assert band["external_rows_are_evidence"] is False
    external = _contract()["independent_corpus"]["external"]
    assert external["band"] == band["external_bytes"]
    assert external["execution_status"] == band["execution_status"]
    assert external["not_evidence"] is True
    assert external["download_forbidden"] is True
    assert external["materialize_forbidden"] is True
    assert external["authorization"] == "separate_authorization"
    assert external["package_check"] == "scripts/governor/checks/external_holdout_package.yaml"


# ── unknowns are recorded, never promoted to verified ──────────────────────


def test_manifest_records_no_per_document_licence_claim() -> None:
    """The arXiv corpus carries no per-document licence record here.

    Unknown rights must stay unknown: a `license`/`rights`/`copyright` key on an
    article would be a claim this repository cannot back, so its absence is
    asserted rather than assumed.
    """
    for article in _articles():
        for key in article:
            lowered = key.lower()
            assert "licen" not in lowered
            assert "rights" not in lowered
            assert "copyright" not in lowered


def test_component_licences_are_owned_by_the_licence_inventory() -> None:
    inventory = _load_yaml(LICENSE_INVENTORY_PATH)
    policy = inventory["redistribution_policy"]
    assert policy["new_distribution"] == "forbidden"
    assert policy["graph_write"] == "forbidden"
    assert policy["import_eligible"] is False
    assert policy["auto_promote"] == "forbidden"
    assert policy["commercial_use"] is False
    assert policy["unknown_rights"] == "blocked"
    assert policy["blocked_components_must_not_enter_runtime"] is True
    statuses = set(inventory["license_status_vocabulary"])
    assert statuses == {"verified", "blocked"}
    components = {entry["component_id"]: entry for entry in inventory["components"]}
    # The two arXiv corpora this record inventories stay blocked: per-article
    # rights were never established, so neither may be written as verified.
    for component_id in ("corpus.internal_gold", "corpus.external_holdout"):
        component = components[component_id]
        assert component["license_status"] == "blocked", component_id
        assert component["redistribution"] == "blocked", component_id
        assert component["blocked_reason"], component_id


def test_research_authorization_owns_the_d491_boundary() -> None:
    authorization = _load_yaml(RESEARCH_AUTHORIZATION_PATH)
    authority = authorization["authority"]
    assert authority["kind"] == "owner_directive"
    assert authority["verdict"] == "authorized"
    assert authority["recorded_decision"] == "D491"
    # D491 waives the C3 release-decision paperwork and grants nothing else.
    assert authorization["scope"]["c3_process_gates"] == "waived"
    not_granted = authorization["not_granted"]
    assert not_granted["third_party_legal_rights"] is False
    assert not_granted["license_clearance"] is False
    assert not_granted["redistribution"] is False
    assert not_granted["graph_write"] is False
    assert not_granted["import_eligible"] is False
    assert not_granted["quality_pass"] is False
    locks = authorization["retained_locks"]
    assert locks["new_distribution"] == "forbidden"
    assert locks["graph_write"] == "forbidden"
    assert locks["import_eligible"] is False
    # The contract names this DATA carrier as the authorization authority and
    # keeps coverage separate from it.
    block = _contract()["authorization"]
    assert block["authority"] == RESEARCH_AUTHORIZATION_PATH
    assert block["decision"] == "D491"
    assert block["product_coverage_effect"] == "none"
    assert block["locks_stay_in_force"] is True


# ── the record itself ──────────────────────────────────────────────────────


def test_inventory_record_exists() -> None:
    """The prose record must exist; its wording is deliberately not parsed."""
    path = _REPO_ROOT / REPORT_PATH
    assert path.is_file(), REPORT_PATH
    assert path.stat().st_size > 0


# ── negative surface of the derivations ────────────────────────────────────


def test_require_list_fails_closed_on_a_missing_or_empty_block() -> None:
    try:
        _require_list({"articles": []}, "articles", "fixture")
    except ValueError:
        pass
    else:  # pragma: no cover - the guard exists to fail
        raise AssertionError("an empty block must not read as an empty population")
    try:
        _require_list({}, "articles", "fixture")
    except ValueError:
        pass
    else:  # pragma: no cover
        raise AssertionError("a missing block must fail closed")
    try:
        _require_list({"articles": {}}, "articles", "fixture")
    except ValueError:
        pass
    else:  # pragma: no cover
        raise AssertionError("a non-list block must fail closed")


def test_wellformed_digest_helpers_reject_malformed_pins() -> None:
    assert _wellformed_sha256("sha256:" + "a" * 64)
    assert not _wellformed_sha256("sha256:" + "a" * 63)
    assert not _wellformed_sha256("sha256:" + "A" * 64)
    assert not _wellformed_sha256("a" * 64)  # the probe pin form is not a prefixed hash
    assert not _wellformed_sha256(None)
    assert not _wellformed_sha256(1234)
    # The probe stores the per-pin identity digest bare; both forms are typed.
    assert _wellformed_hex64("a" * 64)
    assert not _wellformed_hex64("sha256:" + "a" * 64)
    assert not _wellformed_hex64("a" * 63)
    assert not _wellformed_hex64(None)


def test_gold_identity_rule_rejects_a_mismatched_paper_id(tmp_path: Path) -> None:
    matching = tmp_path / "1111.1111.gold.json"
    matching.write_text(json.dumps({"paper_id": "1111.1111"}), encoding="utf-8")
    assert _gold_identity(matching) == "1111.1111"
    mismatched = tmp_path / "1111.1111.gold.json"
    mismatched.write_text(json.dumps({"paper_id": "2222.2222"}), encoding="utf-8")
    with pytest.raises(ValueError):
        _gold_identity(mismatched)


def test_identity_overlap_detects_a_reused_document() -> None:
    assert _identity_overlap(["1203.2295"], ["1203.2295"]) == {"1203.2295"}
    assert _identity_overlap(["1203.2295"], ["1412.6980"]) == set()
    assert _identity_overlap([], ["1412.6980"]) == set()


def test_canonical_catalog_matches_reports_presence_and_absence(tmp_path: Path) -> None:
    root = tmp_path / "article_catalog" / "arxiv"
    assert _canonical_catalog_matches(root, ["1234.5678"]) == []
    source = root / "cs-cl" / "1234.5678" / "source"
    source.mkdir(parents=True)
    (source / "1234.5678.pdf").write_bytes(b"%PDF-1.4")
    assert _canonical_catalog_matches(root, ["1234.5678"]) == ["1234.5678"]
    assert _canonical_catalog_matches(root, ["1234.5678", "9999.0001"]) == ["1234.5678"]
