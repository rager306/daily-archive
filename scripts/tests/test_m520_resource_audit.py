#!/usr/bin/env python3
"""M520 S01/T02 — governance contract for the native-candidate resource audit.

This contract has **one fail-closed admission boundary**. Nothing declared by
the audit manifest is opened, hashed or otherwise touched through a
manifest-selected path until the *entire* manifest has passed closed-schema
validation, exact independent identity binding, closed metadata/rights/caps
checks and path/credential/trusted-root rules. The admission entry returns
immediately on any structural failure, so a rejected manifest cannot reach a
read.

Two separated axes (collapsing them is how a blocked resource becomes a false
"ready"):

* **historical observation** — what the audit recorded on its own host
  (``historical_observed_state``). Committed metadata; never a verification
  result.
* **current availability** — what this verification actually finds in *this*
  checkout, computed at verification time, never stored in the manifest.

A missing git-ignored PDF, an absent protected map or unreachable diagnostic
bytes report ``unavailable``; they are never silently accepted as a match,
never downgraded into a pass, and never acquired. A historical "present"
observation does not imply current source-bound verification.

What this contract proves: **identity** (which bytes), **path safety** and
**honest polarity**. It does not prove scholarly success, a rights approval or
a positive fixture. Negative controls use only inert bytes in a temp directory.
"""

from __future__ import annotations

import copy
import hashlib
import json
import os
import re
import tempfile

REPO_ROOT = os.path.realpath(os.path.join(os.path.dirname(__file__), "..", ".."))
MANIFEST_REL = "artifacts/parse_compat/native_candidate_m520/resource_audit.json"
CONTRACT_REL = "artifacts/parse_compat/native_candidate_m520/source_contract.json"

# --------------------------------------------------------------------------
# Trusted host roots. A manifest may only name *aliases*; the alias -> host
# root mapping lives here, so a manifest can never select a filesystem root and
# an unreachable root degrades to "unavailable", never to a fetch.
# --------------------------------------------------------------------------
TRUSTED_ROOTS = {
    "repo": REPO_ROOT,
    "grobid-home": os.environ.get(
        "M520_GROBID_HOME", "/root/vendor-source/grobid/grobid-home"
    ),
    "rust-grobid-useful-pdf": os.environ.get(
        "M520_USEFUL_PDF_ROOT", "/tmp/rust-grobid-useful-pdf"
    ),
    "rust-grobid-integration": os.environ.get(
        "M520_INTEGRATION_ROOT", "/tmp/rust-grobid-integration"
    ),
}

# --------------------------------------------------------------------------
# Independent literal pins. NONE of these is read from the manifest under test.
# --------------------------------------------------------------------------
EXPECTED_SCHEMA_VERSION = "native-candidate-resource-audit.v1"
EXPECTED_MILESTONE = "M520-zxj5e9"
EXPECTED_SLICE = "S01"
EXPECTED_TASK = "T01"

# id -> exact identity tuple. `kind`/`format`/`stage` are bound to the identity
# here, not merely admitted as "a string from some allowed enum".
PINNED_RESOURCES = {
    "pdf_arx_1203_2295": {
        "kind": "pdf_source",
        "root_alias": "repo",
        "relative_path": "data/sources/arxiv/math-oc/1203.2295/source/1203.2295.pdf",
        "size_bytes": 383369,
        "sha256": "f4594edaccccad936def8b1b4673d6d4796f624e9952f1e94a650cfc3ea94608",
        "format": "pdf",
        "stage": "source",
    },
    "grobid_model_header": {
        "kind": "crf_model_weights",
        "root_alias": "grobid-home",
        "relative_path": "models/header/model.wapiti",
        "size_bytes": 24034608,
        "sha256": "9966a973faa662e0c966774c64586aff07e64d96bde520bb7252a656219e491a",
        "format": "wapiti",
        "stage": "header",
    },
    "grobid_model_reference_segmenter": {
        "kind": "crf_model_weights",
        "root_alias": "grobid-home",
        "relative_path": "models/reference-segmenter/model.wapiti",
        "size_bytes": 12167531,
        "sha256": "cf51216fc62acbfd0dacff568c5f9bc99a9689e3c6e01af0307b939bf4d1fe8c",
        "format": "wapiti",
        "stage": "reference-segmenter",
    },
    "grobid_model_citation": {
        "kind": "crf_model_weights",
        "root_alias": "grobid-home",
        "relative_path": "models/citation/model.wapiti",
        "size_bytes": 22103761,
        "sha256": "5f17697840c78fa9595119aa7f579f5e9576fbc879477c47507a5f9a6bc264f5",
        "format": "wapiti",
        "stage": "citation",
    },
    "grobid_lexicon_places_location": {
        "kind": "gazetteer_lexicon",
        "root_alias": "grobid-home",
        "relative_path": "lexicon/places/location.txt",
        "size_bytes": 10302767,
        "sha256": "74cd4d6bbf38643a6f1dcfbf342c45a7a786cd10bd9db817fa37b0fa65cf51c8",
        "format": "plain_text_lines",
        "stage": "location",
    },
}

RESOURCE_IDENTITY_FIELDS = (
    "id",
    "kind",
    "root_alias",
    "relative_path",
    "size_bytes",
    "sha256",
)
RESOURCE_BINDING_FIELDS = (
    tuple(field for field in RESOURCE_IDENTITY_FIELDS if field != "id")
    + ("format", "stage")
)

PINNED_RESOURCE_IDENTITIES = [
    {
        "id": rid,
        "kind": pin["kind"],
        "root_alias": pin["root_alias"],
        "relative_path": pin["relative_path"],
        "size_bytes": pin["size_bytes"],
        "sha256": pin["sha256"],
    }
    for rid, pin in PINNED_RESOURCES.items()
]

# Exact three negative diagnostic identities, read off the six pre-existing
# evidence files — NOT copied from the manifest under test.
PINNED_DIAGNOSTICS = [
    {
        "document_id": "1203.2295",
        "root_alias": "rust-grobid-integration",
        "report_relative_path": "smoke-post-boundary-1203.2295/report.json",
        "report_size_bytes": 3717981,
        "report_sha256": "b91f09886c37eacddea8d50287b510d9a4910ad78b35c705676638d4057c29c5",
        "tei_relative_path": "smoke-post-boundary-1203.2295/candidate.tei.xml",
        "tei_size_bytes": 14588,
        "tei_sha256": "25ee98d6590234d336e7c22ef4b59bb7fddf47923ebb3a82bb2c3efe1cbddfbc",
        "outcome": "diagnostic_negative",
        "outcome_code": "header_zero_font_refusal_and_reference_lines_unadmitted",
    },
    {
        "document_id": "1506.02438",
        "root_alias": "rust-grobid-integration",
        "report_relative_path": "smoke-post-boundary-1506.02438/report.json",
        "report_size_bytes": 6439357,
        "report_sha256": "1680968c32c379ca8f90366f2c963a8ad417e7acb7c089a8d1ba20001d73dd5f",
        "tei_relative_path": "smoke-post-boundary-1506.02438/candidate.tei.xml",
        "tei_size_bytes": 45012,
        "tei_sha256": "485d0ce54c4748e9e70e88d254f2b1d3d8859a87125b554a2e582701433ae674",
        "outcome": "diagnostic_negative",
        "outcome_code": "header_font_mean_refusal_and_references_region_absent",
    },
    {
        "document_id": "1409.0473",
        "root_alias": "rust-grobid-integration",
        "report_relative_path": "smoke-post-boundary-1409.0473/report.json",
        "report_size_bytes": 6564296,
        "report_sha256": "c945ceca6ceb64035b01012e512d385607e8bda551cccaa07760a3ccf60ca3e9",
        "tei_relative_path": "smoke-post-boundary-1409.0473/candidate.tei.xml",
        "tei_size_bytes": 51380,
        "tei_sha256": "0126507ae4e9f21b242fd06940fcbe977bcfe000cfe51824610db9f0d36809ec",
        "outcome": "diagnostic_negative",
        "outcome_code": "header_font_mean_refusal_and_references_region_absent",
    },
]
DIAGNOSTIC_IDS = [entry["document_id"] for entry in PINNED_DIAGNOSTICS]

PINNED_PROTECTED_MAP = {
    "root_alias": "rust-grobid-useful-pdf",
    "relative_path": "authorized-cli-clean-before-bindings.json",
    "size_bytes": 5724,
    "sha256": "e1d61c9ae05f8c4e998e3a21e4104135a9bb7b42236fffffd8b252ea3f5abe10",
    "entry_count": 25,
}

PINNED_REFERENCE_PLAN = {
    "root_alias": "rust-grobid-integration",
    "relative_path": "gsd-first-pdf-resource-plan.md",
    "size_bytes": 35809,
    "sha256": "4531a05f1e8f6f13b70274032ce9f35a668a53402dcdae028d42692f4f2873cd",
}

PINNED_SYNTHETIC_DIAGNOSTIC = {
    "root_alias": "rust-grobid-integration",
    "relative_path": "synthetic-diagnostic-lexicon.yaml",
    "size_bytes": 220,
    "sha256": "88aa7e02047ee0c8492bef723e71c10ae4061d32f4057511b8fe6943a0e69209",
}

# The independent review that rejected the T01 output; the corrective scope is
# bound to these bytes, so the manifest cannot silently re-point at a rewrite.
PINNED_INDEPENDENT_REVIEW = {
    "root_alias": "rust-grobid-useful-pdf",
    "relative_path": "native-t01-output-independent-review.md",
    "size_bytes": 6435,
    "sha256": "1ab7b8fc3d7a6284f3c3747b9d75b365981db34578d4aa6eb754f37b38df6f14",
}

PINNED_DECISION_REFS = ["D127", "D449", "D491", "ADR-076"]

# Closed vocabularies derived once from the pinned resources and reused by the
# audit schema *and* the source contract, so the two artifacts cannot drift
# apart into parallel lists. These are the admitted *vocabulary*; the exact
# per-identity binding is enforced separately in ``structural_errors``.
RESOURCE_KINDS = sorted({pin["kind"] for pin in PINNED_RESOURCES.values()})
RESOURCE_FORMATS = sorted({pin["format"] for pin in PINNED_RESOURCES.values()})
RESOURCE_STAGES = sorted({pin["stage"] for pin in PINNED_RESOURCES.values()})

CLOSED_BLOCKER_CODES = [
    "lexicon_export_rights_blocked",
    "gazetteer_exceeds_lane_cap",
    "bundle_cap_exceeded",
    "no_lexicon_exporter_available",
    "source_version_drift_0_9_1_vs_0_9_0",
    "java_rust_feature_abi_discrepancy",
    "font_mean_one_ulp_defect",
    "zero_font_annotation_contamination",
    "region_assignment_no_body_transition",
    "reference_inventory_incomplete",
    "visual_layout_not_independently_reviewed",
    "no_independent_positive_fixture",
    "other_channels_unknown_unready",
]

POLICY_LOCK_KEYS = (
    "import_eligible",
    "deployed",
    "export_permitted",
    "native_use_permitted",
    "promotion_permitted",
    "limits_raised",
    "synthetic_as_real_resource",
)

# The lexicon's line census as an independent observation *of the byte-pinned
# file* (620 618 newline-terminated physical lines, 535 792 unique trimmed
# non-blank lines, no blank or whitespace-only lines). It pins the last
# non-snippet field that would otherwise accept any number, and it is a
# measurement of those bytes rather than a product limit.
PINNED_LEXICON_COUNTS = {
    "raw_lines": 620618,
    "raw_lines_basis": "newline_terminated_physical_lines",
    "blank_lines": 0,
    "whitespace_only_lines": 0,
    "unique_trimmed_nonblank_lines": 535792,
    "normalization": "trim_ascii_whitespace_then_drop_empty",
    "dedup": "byte_exact_after_trim",
    "current_loader_applicability": "exceeds_current_caps",
}

PINNED_CORRECTIVE_REVISION = {
    "task_id": "T02",
    "scope": "single_fail_closed_pre_open_admission_and_closed_content",
    "corrected_review_findings": [
        "actual_availability_integrated",
        "closed_non_snippet_content",
        "exact_three_diagnostic_identities",
        "single_fail_closed_pre_open_boundary",
    ],
    "rewrites_t01_history": False,
}

EXPECTED_CAPS = {
    "bundle_entries_per_channel": 100000,
    "gazetteer_entries_per_lane": 65536,
    "serialized_loader_bytes": 16777216,
}

EXPECTED_RIGHTS_SOURCE_FILE = "data/parse/grobid_model_rights.yaml"
EXPECTED_RIGHTS_SOURCE_SHA256 = (
    "e0c77b4b92eb717fa82b951dd46f0d33f0ad0c7311f88c723673c3babe05ef2f"
)

# Sanctioned ``data/parse`` ledger names — independent literals transcribed from
# the existing Rust contract in ``crates/da-parse/tests/data_contracts.rs``.
EXPECTED_DATA_PARSE_NAMES = [
    "SCHEMA.yaml",
    "absorption_progress.schema.yaml",
    "acceptance_manifest.yaml",
    "api_compatibility_ledger.yaml",
    "callout_matching_semantics.yaml",
    "comparison_contract.yaml",
    "consolidation_specialized_semantics.yaml",
    "document_segmentation.yaml",
    "external_holdout_results.yaml",
    "feature_tagger_training_parity.yaml",
    "fulltext_block_semantics.yaml",
    "grobid_compatibility.yaml",
    "grobid_model_rights.yaml",
    "holdout_manifests.yaml",
    "layout_bakeoff.yaml",
    "layout_lattice_parity.yaml",
    "license_inventory.yaml",
    "native_body_accounting.yaml",
    "native_fulltext_composition.yaml",
    "native_visual_families.yaml",
    "native_visual_heads.yaml",
    "p5_cross_stage.yaml",
    "p5_shadow_join.yaml",
    "regression_cases.yaml",
    "research_authorization.yaml",
    "rest_batch_asset_parity.yaml",
]

EXPECTED_DATA_PARSE_PINS = {
    "SCHEMA.yaml": (2268, "733dba7d34d9fbe67f40fe39093344d1cd5e6a5328bfd9e3245ba0200777afaa"),
    "absorption_progress.schema.yaml": (27345, "4ed78e34cb5863cf0fcbf6c09fed1351e1bb722659c73b881a36b1a16317641a"),
    "acceptance_manifest.yaml": (155745, "4d1c55ea9dd873b4aada48aa964609395f59e8a726bdf42c65d2b0f36865ca06"),
    "api_compatibility_ledger.yaml": (46549, "fcce2f3e54780ba6385935aa111b733eb1a3dd25510e0907fa9baa0279092c28"),
    "callout_matching_semantics.yaml": (6130, "b9473399840845ed67e840163907b7bdb78088236b02532666f1682d78402290"),
    "comparison_contract.yaml": (51392, "9e4ace58568f9aa043bb9e721544b684e5238f526c35eb71c6ce83ed4fde20e0"),
    "consolidation_specialized_semantics.yaml": (4528, "6bc65ca991dbcbb9f452c660d484d9eff75765132220c8e521563b861dd9e740"),
    "document_segmentation.yaml": (7739, "2bc5d8b9fe03e01eae94f7373d4777dec1318f7eacd59feb3c8237b48fd5e0ae"),
    "external_holdout_results.yaml": (1538, "8194a76e03106f300032d381edf1b1d9d2bb986f10803947c124c7f453f71705"),
    "feature_tagger_training_parity.yaml": (8242, "67be4ce8e63add107cfc07265df0f74c3e85006bb2bdb961ca75b44bd7e88e14"),
    "fulltext_block_semantics.yaml": (5647, "b39736cf64e3914c7fe9f59f43be7e8ef0b5c7621d6541aeb1195cce50302989"),
    "grobid_compatibility.yaml": (145161, "0065ca553b8b4f647831bec7f372680c4d6c0f9d31701517883d5b17c9d8f4ec"),
    "grobid_model_rights.yaml": (3184, EXPECTED_RIGHTS_SOURCE_SHA256),
    "holdout_manifests.yaml": (139069, "ef91d3c25c7d91dda2654471140fd41fcae8e1ed87e7094ad24c31fef2b64a39"),
    "layout_bakeoff.yaml": (19598, "a98d1772e68da35b9deb99c62e4c9ab413e53f0d1ddf918e017ac2242c0d80cc"),
    "layout_lattice_parity.yaml": (3609, "6e10a02eaeab1d1a39191451874d3b4a14b80c74dc9da45bb8f35a7b329d5f0f"),
    "license_inventory.yaml": (18869, "4f860c17a7963570b7233988034a7005cde3ed0f3cde6785b96d23fb53caba66"),
    "native_body_accounting.yaml": (7296, "611b243c3bff686256e3f1ea620bee76681f87a9359b6a48abfff7551435ff58"),
    "native_fulltext_composition.yaml": (31459, "266b68b413f26c905cd430db5b555ead7037701a0905f4260b77cb84fa85e141"),
    "native_visual_families.yaml": (7453, "00a535c7ca2ead6e47b96657b4f8508a8c0f5bd19eebc8752021c74f36058f99"),
    "native_visual_heads.yaml": (6107, "d4b15517801bec5c90d62f65485563921e14c4b495f70743b6c39a35301190d1"),
    "p5_cross_stage.yaml": (1451, "75765ca7e6fa18460820ecd07a383d72a59b03cf9932e10fc53b46d3c8d0bd00"),
    "p5_shadow_join.yaml": (1204, "3ca8e19358d8689b9aeb1ff1d5e3cbbd3163ff72af0788d43d75bd10b95c6ab9"),
    "regression_cases.yaml": (11831, "cd7a35fb089b6facb33c0caf70f89522ac7bf2c664182ad81ed6d84532c628fd"),
    "research_authorization.yaml": (5889, "caa0c5c3baf6e4156375f74cb2d3c691530b01cb2567ce698e431e07be7a7a41"),
    "rest_batch_asset_parity.yaml": (6225, "332dc94e2eae2c18ea4e1946e457d161bb8ec24c927a471eefb73c7c4367a583"),
}

PROVENANCE = "independent_source_pdf_text"
PINNED_TITLE = {
    "snippet": "TECHNIQUES FOR SOLVING SUDOKU PUZZLES",
    "chars": 37,
    "physical_page": 1,
    "printed_page": 1,
    "provenance": PROVENANCE,
    "independent_review": False,
}
PINNED_REFERENCES = [
    {
        "label": 11,
        "snippet": "[11] K. Lange, Optimization, Springer-Verlag, New York, 2004.",
        "chars": 61,
        "physical_page": 10,
        "printed_page": 10,
        "provenance": PROVENANCE,
        "independent_review": False,
    },
    {
        "label": 18,
        "snippet": "[18] S. S. Skiena, The Algorithm Design Manual, Springer-Verlag, 2nd ed., 2008.",
        "chars": 79,
        "physical_page": 11,
        "printed_page": 11,
        "provenance": PROVENANCE,
        "independent_review": False,
    },
]
PINNED_CALLOUTS = [
    {
        "target_label": 11,
        "snippet": "[11]. We will denote the projection operator taking y to x by PC (y) = x.",
        "chars": 73,
        "physical_page": 4,
        "printed_page": 4,
        "provenance": PROVENANCE,
        "independent_review": False,
    },
    {
        "target_label": 18,
        "snippet": "it becomes a full solution or violates a constraint [18]. In the latter case it backtracks",
        "chars": 90,
        "physical_page": 2,
        "printed_page": 2,
        "provenance": PROVENANCE,
        "independent_review": False,
    },
]
PINNED_SNIPPET_BOUND = 90  # longest actual observed snippet, not a product limit
PINNED_PDF_NUMERICS = {
    "document_id": "1203.2295",
    "extraction_mode": "independent_poppler_pdftotext_text",
    "page_count": 11,
    "physical_printed_page_aligned": True,
    "visual_layout_reviewed": False,
    "semantic_truth_asserted": False,
    "substring_scan_is_proof": False,
    "snippet_char_bound": PINNED_SNIPPET_BOUND,
    "reference_list_total_labels": 22,
    "target_reference_labels": [11, 18],
    "wrapped_or_incomplete_refs_unresolved": 22,
    "complete_bibliography_established": False,
    "linking_established": False,
    "two_selected_refs_establish_complete_bibliography": False,
}

# Closed rejection-code vocabulary for the admission report.
CODE_SCHEMA = "schema_or_closed_content_invalid"
CODE_DECISION_REFS = "authority_decision_refs_not_exact"
CODE_PATH_UNSAFE = "path_unsafe_or_credential_like"
CODE_RESOURCE_IDENTITY = "resource_identity_not_bound"
CODE_DIAGNOSTIC_MAP = "diagnostic_mapping_not_exact"
CODE_PROTECTED_MAP = "protected_map_pin_invalid"
CODE_REFERENCE_PLAN = "reference_plan_pin_invalid"
CODE_SYNTHETIC = "synthetic_diagnostic_pin_invalid"
CODE_INDEPENDENT_REVIEW = "independent_review_pin_invalid"
CODE_CORRECTIVE_REVISION = "corrective_revision_pin_invalid"
CODE_HISTORICAL_SCOPE = "historical_vs_current_scope_invalid"
CODE_PDF_EVIDENCE = "pdf_snippet_evidence_invalid"
CODE_LIVE_TREE = "data_parse_tree_pins_invalid"
CODE_RIGHTS_SOURCE = "rights_boundary_pin_invalid"
CODE_BLOCKERS = "blocker_set_or_readiness_not_blocked"

CODE_DECLARED_BOUND = "declared_source_bound_not_corroborated"
CODE_REQUIRED_UNAVAILABLE = "required_identity_currently_unavailable"
CODE_REQUIRED_MISMATCH = "required_identity_currently_mismatch"

REJECTION_CODES = frozenset(
    {
        CODE_SCHEMA,
        CODE_DECISION_REFS,
        CODE_PATH_UNSAFE,
        CODE_RESOURCE_IDENTITY,
        CODE_DIAGNOSTIC_MAP,
        CODE_PROTECTED_MAP,
        CODE_REFERENCE_PLAN,
        CODE_SYNTHETIC,
        CODE_INDEPENDENT_REVIEW,
        CODE_CORRECTIVE_REVISION,
        CODE_HISTORICAL_SCOPE,
        CODE_PDF_EVIDENCE,
        CODE_LIVE_TREE,
        CODE_RIGHTS_SOURCE,
        CODE_BLOCKERS,
    }
)

CREDENTIAL_LIKE = {
    "netrc",
    "credentials",
    "credential",
    "secret",
    "secrets",
    "password",
    "passwd",
    "token",
    "tokens",
    "apikey",
    "api_key",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    "env",
    "kubeconfig",
}
CREDENTIAL_SUFFIXES = (".pem", ".key", ".p12", ".pfx")
HEX64 = re.compile(r"^[0-9a-f]{64}$")

# --------------------------------------------------------------------------
# Loaders
# --------------------------------------------------------------------------
def manifest_path() -> str:
    return os.path.join(REPO_ROOT, MANIFEST_REL)


def contract_path() -> str:
    return os.path.join(REPO_ROOT, CONTRACT_REL)


def load_manifest() -> dict:
    with open(manifest_path(), "r", encoding="utf-8") as handle:
        return json.load(handle)


def load_source_contract() -> dict:
    with open(contract_path(), "r", encoding="utf-8") as handle:
        return json.load(handle)


# --------------------------------------------------------------------------
# Closed-schema walker (one implementation, used by both artifacts)
# --------------------------------------------------------------------------
def _validate(spec, value, path, errs):
    t = spec.get("t")
    if t == "obj":
        if not isinstance(value, dict):
            errs.append(f"{path}: expected object, got {type(value).__name__}")
            return
        fields = spec.get("fields", {})
        if spec.get("closed", True):
            for key in value:
                if key not in fields:
                    errs.append(f"{path}: unknown key {key!r} (closed vocabulary)")
        for key, sub in fields.items():
            if key not in value:
                if sub.get("opt"):
                    continue
                errs.append(f"{path}: missing required key {key!r}")
                continue
            _validate(sub, value[key], f"{path}.{key}", errs)
    elif t == "arr":
        if not isinstance(value, list):
            errs.append(f"{path}: expected array, got {type(value).__name__}")
            return
        if "len" in spec and len(value) != spec["len"]:
            errs.append(f"{path}: expected length {spec['len']}, got {len(value)}")
        if "min" in spec and len(value) < spec["min"]:
            errs.append(f"{path}: expected at least {spec['min']}, got {len(value)}")
        if "max" in spec and len(value) > spec["max"]:
            errs.append(f"{path}: expected at most {spec['max']}, got {len(value)}")
        for index, item in enumerate(value):
            _validate(spec["item"], item, f"{path}[{index}]", errs)
    elif t == "map":
        if not isinstance(value, dict):
            errs.append(f"{path}: expected map, got {type(value).__name__}")
            return
        for key, item in value.items():
            if "keys" in spec and key not in spec["keys"]:
                errs.append(f"{path}: unknown map key {key!r}")
            _validate(spec["value"], item, f"{path}.{key}", errs)
    elif t == "enum":
        if value not in spec["values"]:
            errs.append(f"{path}: {value!r} not in closed vocabulary {spec['values']}")
    elif t == "snippet":
        if not isinstance(value, str):
            errs.append(f"{path}: expected snippet string, got {type(value).__name__}")
    elif t == "hex64":
        if not isinstance(value, str) or not HEX64.match(value):
            errs.append(f"{path}: expected 64-char lowercase hex digest")
    elif t == "int":
        if not isinstance(value, int) or isinstance(value, bool):
            errs.append(f"{path}: expected int, got {type(value).__name__}")
            return
        if "min" in spec and value < spec["min"]:
            errs.append(f"{path}: {value} below min {spec['min']}")
        if "max" in spec and value > spec["max"]:
            errs.append(f"{path}: {value} above max {spec['max']}")
    elif t == "str":
        if not isinstance(value, str):
            errs.append(f"{path}: expected string, got {type(value).__name__}")
    elif t == "bool":
        if not isinstance(value, bool):
            errs.append(f"{path}: expected bool, got {type(value).__name__}")
            return
        if "value" in spec and value is not spec["value"]:
            errs.append(f"{path}: must stay {spec['value']}, got {value}")
    elif t == "null":
        if value is not None:
            errs.append(f"{path}: must stay null, got {type(value).__name__}")
    else:
        errs.append(f"{path}: unknown spec type {t!r}")


def _snippet_block_spec():
    return {
        "t": "obj",
        "closed": True,
        "fields": {
            "snippet": {"t": "snippet"},
            "chars": {"t": "int", "min": 1, "max": PINNED_SNIPPET_BOUND},
            "physical_page": {"t": "int", "min": 1},
            "printed_page": {"t": "int", "min": 1},
            "provenance": {"t": "enum", "values": [PROVENANCE]},
            "independent_review": {"t": "bool", "value": False},
        },
    }


def _reference_block_spec():
    block = _snippet_block_spec()
    block["fields"]["label"] = {"t": "int", "min": 1}
    return block


def _callout_block_spec():
    block = _snippet_block_spec()
    block["fields"]["target_label"] = {"t": "int", "min": 1}
    return block


def build_manifest_schema():
    resource_spec = {
        "t": "obj",
        "closed": True,
        "fields": {
            "id": {"t": "enum", "values": sorted(PINNED_RESOURCES)},
            "kind": {"t": "enum", "values": RESOURCE_KINDS},
            "root_alias": {"t": "enum", "values": sorted(TRUSTED_ROOTS)},
            "relative_path": {"t": "str"},
            "size_bytes": {"t": "int", "min": 1},
            "sha256": {"t": "hex64"},
            "historical_observed_state": {"t": "enum", "values": ["present", "unavailable"]},
            "git_tracked": {"t": "bool", "value": False},
            "disposition": {
                "t": "enum",
                "values": ["read_only_local_evidence", "external_service_only_evidence"],
            },
            # closed here as well as bound per identity below: the vocabulary is
            # declared, not merely "some string"
            "format": {"t": "enum", "values": RESOURCE_FORMATS},
            "stage": {"t": "enum", "values": RESOURCE_STAGES},
            "reuse": {"t": "enum", "values": ["external_service_only"], "opt": True},
            "redistributable": {"t": "bool", "value": False, "opt": True},
            "rights_status": {"t": "enum", "values": ["blocked"], "opt": True},
            "pdf_redistribution": {"t": "enum", "values": ["not_performed"], "opt": True},
            "pdf_rights_assessment": {
                "t": "enum",
                "values": ["not_performed_in_this_audit"],
                "opt": True,
            },
            "counts": {
                "t": "obj",
                "closed": True,
                "opt": True,
                "fields": {
                    "raw_lines": {"t": "int", "min": 0},
                    "raw_lines_basis": {
                        "t": "enum",
                        "values": ["newline_terminated_physical_lines"],
                    },
                    "blank_lines": {"t": "int", "min": 0},
                    "whitespace_only_lines": {"t": "int", "min": 0},
                    "unique_trimmed_nonblank_lines": {"t": "int", "min": 0},
                    "normalization": {
                        "t": "enum",
                        "values": ["trim_ascii_whitespace_then_drop_empty"],
                    },
                    "dedup": {"t": "enum", "values": ["byte_exact_after_trim"]},
                    "current_loader_applicability": {
                        "t": "enum",
                        "values": ["exceeds_current_caps", "within_current_caps"],
                    },
                },
            },
        },
    }
    diagnostic_spec = {
        "t": "obj",
        "closed": True,
        "fields": {
            "document_id": {"t": "enum", "values": DIAGNOSTIC_IDS},
            "root_alias": {"t": "enum", "values": ["rust-grobid-integration"]},
            "report_relative_path": {"t": "str"},
            "report_size_bytes": {"t": "int", "min": 1},
            "report_sha256": {"t": "hex64"},
            "tei_relative_path": {"t": "str"},
            "tei_size_bytes": {"t": "int", "min": 1},
            "tei_sha256": {"t": "hex64"},
            "outcome": {"t": "enum", "values": ["diagnostic_negative"]},
            "outcome_code": {
                "t": "enum",
                "values": [entry["outcome_code"] for entry in PINNED_DIAGNOSTICS],
            },
        },
    }
    blocker_codes = CLOSED_BLOCKER_CODES
    bool_locks = POLICY_LOCK_KEYS
    return {
        "t": "obj",
        "closed": True,
        "fields": {
            "schema_version": {"t": "enum", "values": [EXPECTED_SCHEMA_VERSION]},
            "audit_kind": {"t": "enum", "values": ["metadata_only_read_only_research_audit"]},
            "milestone_id": {"t": "enum", "values": [EXPECTED_MILESTONE]},
            "slice_id": {"t": "enum", "values": [EXPECTED_SLICE]},
            "task_id": {"t": "enum", "values": [EXPECTED_TASK]},
            "inventory_scope": {"t": "enum", "values": ["partial_3_weights_1_gazetteer"]},
            "inventory_is_complete": {"t": "bool", "value": False},
            "genuine_export": {"t": "bool", "value": False},
            "product_or_scientific_acceptance": {"t": "bool", "value": False},
            "historical_observations": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "scope": {
                        "t": "enum",
                        "values": ["historical_metadata_audit_not_current_verification"],
                    },
                    "current_availability_source": {
                        "t": "enum",
                        "values": ["computed_by_governance_contract_at_verification_time"],
                    },
                    "current_availability_stored_in_manifest": {"t": "bool", "value": False},
                },
            },
            "authority": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "decision_refs": {
                        "t": "arr",
                        "len": len(PINNED_DECISION_REFS),
                        "item": {"t": "enum", "values": PINNED_DECISION_REFS},
                    },
                    "d449_scope": {"t": "enum", "values": ["M512"]},
                    "d449_is_blanket_resource_permission": {"t": "bool", "value": False},
                    "d491_effect": {
                        "t": "enum",
                        "values": [
                            "c3_independent_rights_review_not_prerequisite_for_hobby_research"
                        ],
                    },
                    "fresh_per_asset_legal_census_required": {"t": "bool", "value": False},
                },
            },
            "policy_locks": {
                "t": "obj",
                "closed": True,
                "fields": dict(
                    [
                        (key, {"t": "bool", "value": False})
                        for key in bool_locks
                    ]
                    + [
                        ("auto_promote", {"t": "enum", "values": ["forbidden"]}),
                        ("graph_write", {"t": "enum", "values": ["forbidden"]}),
                    ]
                ),
            },
            "roots": {
                "t": "arr",
                "len": len(TRUSTED_ROOTS),
                "item": {
                    "t": "obj",
                    "closed": True,
                    "fields": {
                        "alias": {"t": "enum", "values": sorted(TRUSTED_ROOTS)},
                        "kind": {
                            "t": "enum",
                            "values": [
                                "repository_root",
                                "trusted_local_grobid_resource_root",
                                "trusted_external_evidence_root",
                            ],
                        },
                    },
                },
            },
            "closed_vocabularies": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "observed_states": {
                        "t": "arr",
                        "item": {"t": "enum", "values": ["present", "unavailable"]},
                    },
                    "resource_kinds": {
                        "t": "arr",
                        "item": {"t": "enum", "values": RESOURCE_KINDS},
                    },
                    "rights_reuse": {
                        "t": "arr",
                        "item": {"t": "enum", "values": ["external_service_only"]},
                    },
                    "rights_status": {
                        "t": "arr",
                        "item": {"t": "enum", "values": ["blocked"]},
                    },
                    "pdf_redistribution": {
                        "t": "arr",
                        "item": {"t": "enum", "values": ["not_performed"]},
                    },
                    "pdf_rights_assessment": {
                        "t": "arr",
                        "item": {"t": "enum", "values": ["not_performed_in_this_audit"]},
                    },
                    "dispositions": {
                        "t": "arr",
                        "item": {
                            "t": "enum",
                            "values": [
                                "read_only_local_evidence",
                                "external_service_only_evidence",
                            ],
                        },
                    },
                    "blocker_effects": {
                        "t": "arr",
                        "item": {
                            "t": "enum",
                            "values": [
                                "resource_not_ready",
                                "capability_absent",
                                "identity_unverified",
                                "quality_unverified",
                                "positive_fixture_absent",
                                "abi_mismatch",
                                "version_drift",
                            ],
                        },
                    },
                    "diagnostic_outcome_codes": {
                        "t": "arr",
                        "item": {
                            "t": "enum",
                            "values": [entry["outcome_code"] for entry in PINNED_DIAGNOSTICS],
                        },
                    },
                    "blocker_codes": {
                        "t": "arr",
                        "item": {"t": "enum", "values": blocker_codes},
                    },
                },
            },
            "resources": {
                "t": "arr",
                "len": len(PINNED_RESOURCES),
                "item": resource_spec,
            },
            "caps": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "bundle_entries_per_channel": {"t": "int", "min": 1},
                    "gazetteer_entries_per_lane": {"t": "int", "min": 1},
                    "serialized_loader_bytes": {"t": "int", "min": 1},
                    "caps_unchanged": {"t": "bool", "value": True},
                    "subset_permitted": {"t": "bool", "value": False},
                    "sharding_permitted": {"t": "bool", "value": False},
                    "alias_key_permitted": {"t": "bool", "value": False},
                    "cap_raise_permitted": {"t": "bool", "value": False},
                },
            },
            "rights_boundary": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "source_file": {"t": "enum", "values": [EXPECTED_RIGHTS_SOURCE_FILE]},
                    "source_sha256": {"t": "hex64"},
                    "source_must_remain_byte_identical": {"t": "bool", "value": True},
                    "reuse": {"t": "enum", "values": ["external_service_only"]},
                    "redistributable": {"t": "bool", "value": False},
                    "rights_status": {"t": "enum", "values": ["blocked"]},
                    "missing_or_ambiguous_rights": {"t": "enum", "values": ["fail_closed"]},
                    "export_permitted": {"t": "bool", "value": False},
                    "native_use_permitted": {"t": "bool", "value": False},
                    "promotion_permitted": {"t": "bool", "value": False},
                    "model_bytes_committed": {"t": "bool", "value": False},
                    "sidecar_authority": {"t": "bool", "value": False},
                },
            },
            "pdf_evidence": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "document_id": {"t": "enum", "values": ["1203.2295"]},
                    "extraction_mode": {
                        "t": "enum",
                        "values": ["independent_poppler_pdftotext_text"],
                    },
                    "page_count": {"t": "int", "min": 1},
                    "physical_printed_page_aligned": {"t": "bool", "value": True},
                    "visual_layout_reviewed": {"t": "bool", "value": False},
                    "semantic_truth_asserted": {"t": "bool", "value": False},
                    "substring_scan_is_proof": {"t": "bool", "value": False},
                    "snippet_char_bound": {"t": "int", "min": 1},
                    "title": _snippet_block_spec(),
                    "references": {"t": "arr", "len": 2, "item": _reference_block_spec()},
                    "callouts": {"t": "arr", "len": 2, "item": _callout_block_spec()},
                    "reference_list_total_labels": {"t": "int", "min": 0},
                    "target_reference_labels": {
                        "t": "arr",
                        "item": {"t": "int", "min": 1},
                    },
                    "wrapped_or_incomplete_refs_unresolved": {"t": "int", "min": 0},
                    "complete_bibliography_established": {"t": "bool", "value": False},
                    "linking_established": {"t": "bool", "value": False},
                    "two_selected_refs_establish_complete_bibliography": {
                        "t": "bool",
                        "value": False,
                    },
                },
            },
            "diagnostic_negatives": {"t": "arr", "len": 3, "item": diagnostic_spec},
            "synthetic_diagnostic": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "root_alias": {"t": "enum", "values": ["rust-grobid-integration"]},
                    "relative_path": {"t": "str"},
                    "size_bytes": {"t": "int", "min": 1},
                    "sha256": {"t": "hex64"},
                    "is_synthetic": {"t": "bool", "value": True},
                    "scholarly_lexicon": {"t": "bool", "value": False},
                    "copied_to_carrier": {"t": "bool", "value": False},
                    "usable_as_real_resource": {"t": "bool", "value": False},
                },
            },
            "protected_map": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "root_alias": {"t": "enum", "values": ["rust-grobid-useful-pdf"]},
                    "relative_path": {"t": "str"},
                    "size_bytes": {"t": "int", "min": 1},
                    "sha256": {"t": "hex64"},
                    "entry_count": {"t": "int", "min": 1},
                    "historical_observed_state": {
                        "t": "enum",
                        "values": ["present", "unavailable"],
                    },
                    "replacement_or_trim_permitted": {"t": "bool", "value": False},
                    "contents_committed": {"t": "bool", "value": False},
                },
            },
            "reference_plan": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "root_alias": {"t": "enum", "values": ["rust-grobid-integration"]},
                    "relative_path": {"t": "str"},
                    "size_bytes": {"t": "int", "min": 1},
                    "sha256": {"t": "hex64"},
                    "consulted": {"t": "bool", "value": True},
                    "authority": {"t": "enum", "values": ["leads_only"]},
                    "authority_over_d491_or_current_manifest": {"t": "bool", "value": False},
                },
            },
            "corrective_revision": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "task_id": {"t": "enum", "values": ["T02"]},
                    "scope": {
                        "t": "enum",
                        "values": [
                            "single_fail_closed_pre_open_admission_and_closed_content"
                        ],
                    },
                    "corrected_review_findings": {
                        "t": "arr",
                        "len": 4,
                        "item": {
                            "t": "enum",
                            "values": PINNED_CORRECTIVE_REVISION[
                                "corrected_review_findings"
                            ],
                        },
                    },
                    "independent_review": {
                        "t": "obj",
                        "closed": True,
                        "fields": {
                            "root_alias": {
                                "t": "enum",
                                "values": ["rust-grobid-useful-pdf"],
                            },
                            "relative_path": {"t": "str"},
                            "size_bytes": {"t": "int", "min": 1},
                            "sha256": {"t": "hex64"},
                        },
                    },
                    "rewrites_t01_history": {"t": "bool", "value": False},
                },
            },
            "data_parse_tree": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "directory": {"t": "enum", "values": ["data/parse"]},
                    "expected_entry_count": {"t": "int", "min": 1},
                    "sanctioned_names": {
                        "t": "arr",
                        "item": {"t": "enum", "values": EXPECTED_DATA_PARSE_NAMES},
                    },
                    "bytepins": {
                        "t": "map",
                        "keys": EXPECTED_DATA_PARSE_NAMES,
                        "value": {
                            "t": "obj",
                            "closed": True,
                            "fields": {
                                "size_bytes": {"t": "int", "min": 1},
                                "sha256": {"t": "hex64"},
                            },
                        },
                    },
                    "directories_permitted": {"t": "bool", "value": False},
                    "parallel_ledger_tree_permitted": {"t": "bool", "value": False},
                },
            },
            "blockers": {
                "t": "arr",
                "min": 1,
                "item": {
                    "t": "obj",
                    "closed": True,
                    "fields": {
                        "code": {"t": "enum", "values": blocker_codes},
                        "resource_id": {"t": "enum", "values": sorted(PINNED_RESOURCES)},
                        "effect": {
                            "t": "enum",
                            "values": [
                                "resource_not_ready",
                                "capability_absent",
                                "identity_unverified",
                                "quality_unverified",
                                "positive_fixture_absent",
                                "abi_mismatch",
                                "version_drift",
                            ],
                        },
                    },
                },
            },
            "readiness": {
                "t": "obj",
                "closed": True,
                "fields": {
                    "first_pdf_source_bound": {"t": "bool", "value": True},
                    "source_bound_requires_current_byte_corroboration": {
                        "t": "bool",
                        "value": True,
                    },
                    "positive_fixture_ready": {"t": "bool", "value": False},
                    "complete_bibliography_established": {"t": "bool", "value": False},
                    "callouts_linked_established": {"t": "bool", "value": False},
                    "lexicon_export_ready": {"t": "bool", "value": False},
                    "native_access_ready": {"t": "bool", "value": False},
                    "visual_layout_reviewed": {"t": "bool", "value": False},
                    "other_channels": {"t": "enum", "values": ["unknown_not_ready"]},
                    "rights_approval_claimed": {"t": "bool", "value": False},
                    "verdict": {
                        "t": "enum",
                        "values": ["blocked_pending_resources_and_positive_fixture"],
                    },
                },
            },
        },
    }


MANIFEST_SCHEMA = build_manifest_schema()


def manifest_schema_errors(doc) -> list:
    errs: list = []
    _validate(MANIFEST_SCHEMA, doc, "$", errs)
    return errs


# --------------------------------------------------------------------------
# Path safety (pure)
# --------------------------------------------------------------------------
def path_safety_errors(rel: str) -> list:
    errs = []
    if not isinstance(rel, str) or not rel:
        return ["path must be a non-empty string"]
    if os.path.isabs(rel) or rel.startswith("\\") or re.match(r"^[A-Za-z]:", rel):
        errs.append(f"absolute path is forbidden: {rel!r}")
    parts = rel.split("/")
    for part in parts:
        if part in ("", ".", ".."):
            errs.append(f"empty or traversal segment {part!r} in {rel!r}")
    if "\\" in rel:
        errs.append(f"backslash separator is forbidden: {rel!r}")
    for part in parts:
        low = part.lower()
        stem = low.split(".", 1)[0]
        dotless = low.lstrip(".")
        if low in CREDENTIAL_LIKE or stem in CREDENTIAL_LIKE or dotless in CREDENTIAL_LIKE:
            errs.append(f"credential-like path segment {part!r} in {rel!r}")
        if low.endswith(CREDENTIAL_SUFFIXES):
            errs.append(f"credential-like suffix in {rel!r}")
    return errs


def _declared_paths(manifest: dict):
    """Every manifest-supplied relative path, with a location label."""
    for entry in manifest.get("resources") or []:
        if isinstance(entry, dict):
            yield f"resources[{entry.get('id')}]", entry.get("relative_path")
    for entry in manifest.get("diagnostic_negatives") or []:
        if isinstance(entry, dict):
            label = entry.get("document_id")
            yield f"diagnostic_negatives[{label}].report", entry.get("report_relative_path")
            yield f"diagnostic_negatives[{label}].tei", entry.get("tei_relative_path")
    for key in ("synthetic_diagnostic", "protected_map", "reference_plan"):
        entry = manifest.get(key)
        if isinstance(entry, dict):
            yield key, entry.get("relative_path")
    revision = manifest.get("corrective_revision")
    if isinstance(revision, dict):
        review = revision.get("independent_review")
        if isinstance(review, dict):
            yield "corrective_revision.independent_review", review.get("relative_path")


# --------------------------------------------------------------------------
# Structural admission (PURE: never touches the filesystem)
# --------------------------------------------------------------------------
def structural_errors(manifest) -> list:
    """All admission checks that need no filesystem access.

    Returns ``[(code, detail), ...]``. Because this is pure, a manifest that
    fails here can never reach an open/hash: the admission entry returns early.
    """
    errs: list = []

    def add(code, detail):
        errs.append((code, detail))

    if not isinstance(manifest, dict):
        return [(CODE_SCHEMA, f"manifest is not an object: {type(manifest).__name__}")]

    # 1. closed schema / key+type vocabulary
    for message in manifest_schema_errors(manifest):
        add(CODE_SCHEMA, message)

    # 2. every manifest-declared path: no absolute/traversal/credential shapes
    for where, rel in _declared_paths(manifest):
        for message in path_safety_errors(rel):
            add(CODE_PATH_UNSAFE, f"{where}: {message}")

    # 3. authority decision references close to the exact approved set
    authority = manifest.get("authority")
    refs = authority.get("decision_refs") if isinstance(authority, dict) else None
    if refs != PINNED_DECISION_REFS:
        add(CODE_DECISION_REFS, f"decision_refs {refs!r} != {PINNED_DECISION_REFS!r}")

    # 4. resource identity: kind/format/stage bound to the exact identity
    resources = manifest.get("resources")
    by_id: dict = {}
    if isinstance(resources, list):
        for entry in resources:
            if not isinstance(entry, dict):
                continue
            rid = entry.get("id")
            if rid in by_id:
                add(CODE_RESOURCE_IDENTITY, f"duplicate resource id {rid!r}")
            by_id[rid] = entry
    if set(by_id) != set(PINNED_RESOURCES):
        add(
            CODE_RESOURCE_IDENTITY,
            f"resource id set {sorted(by_id)} != {sorted(PINNED_RESOURCES)}",
        )
    for rid, pinned in PINNED_RESOURCES.items():
        entry = by_id.get(rid)
        if not isinstance(entry, dict):
            continue
        for field in RESOURCE_BINDING_FIELDS:
            if entry.get(field) != pinned[field]:
                add(
                    CODE_RESOURCE_IDENTITY,
                    f"{rid}.{field} {entry.get(field)!r} != {pinned[field]!r}",
                )
        counts = entry.get("counts")
        if rid == "grobid_lexicon_places_location":
            if counts != PINNED_LEXICON_COUNTS:
                add(
                    CODE_RESOURCE_IDENTITY,
                    f"{rid}.counts != the independently observed lexicon counts",
                )
        elif counts is not None:
            add(CODE_RESOURCE_IDENTITY, f"{rid}.counts is not a declared observation")

    # 5. exact three negative diagnostic identities (whole mapping compared)
    diagnostics = manifest.get("diagnostic_negatives")
    if diagnostics != PINNED_DIAGNOSTICS:
        add(CODE_DIAGNOSTIC_MAP, "diagnostic_negatives != independent pin table")
        got: dict = {}
        if isinstance(diagnostics, list):
            for entry in diagnostics:
                if not isinstance(entry, dict):
                    add(CODE_DIAGNOSTIC_MAP, f"malformed diagnostic entry {entry!r}")
                    continue
                doc = entry.get("document_id")
                if doc in got:
                    add(CODE_DIAGNOSTIC_MAP, f"duplicate diagnostic document_id {doc!r}")
                got[doc] = entry
        for pinned_entry in PINNED_DIAGNOSTICS:
            doc = pinned_entry["document_id"]
            if doc not in got:
                add(CODE_DIAGNOSTIC_MAP, f"diagnostic {doc} is missing")
            elif got[doc] != pinned_entry:
                add(CODE_DIAGNOSTIC_MAP, f"diagnostic {doc} mapping is substituted")
        for doc in got:
            if doc not in DIAGNOSTIC_IDS:
                add(CODE_DIAGNOSTIC_MAP, f"unexpected diagnostic {doc!r}")

    # 6. protected map / reference plan / synthetic / independent review pins
    for code, key, pinned in (
        (CODE_PROTECTED_MAP, "protected_map", PINNED_PROTECTED_MAP),
        (CODE_REFERENCE_PLAN, "reference_plan", PINNED_REFERENCE_PLAN),
        (CODE_SYNTHETIC, "synthetic_diagnostic", PINNED_SYNTHETIC_DIAGNOSTIC),
    ):
        entry = manifest.get(key)
        if not isinstance(entry, dict):
            add(code, f"{key} missing")
            continue
        for field, value in pinned.items():
            if entry.get(field) != value:
                add(code, f"{key}.{field} {entry.get(field)!r} != {value!r}")

    revision = manifest.get("corrective_revision")
    if not isinstance(revision, dict):
        add(CODE_CORRECTIVE_REVISION, "corrective_revision missing")
    else:
        for field, value in PINNED_CORRECTIVE_REVISION.items():
            if revision.get(field) != value:
                add(
                    CODE_CORRECTIVE_REVISION,
                    f"corrective_revision.{field} {revision.get(field)!r} != {value!r}",
                )
        review = revision.get("independent_review")
        if not isinstance(review, dict):
            add(CODE_INDEPENDENT_REVIEW, "corrective_revision.independent_review missing")
        else:
            for field, value in PINNED_INDEPENDENT_REVIEW.items():
                if review.get(field) != value:
                    add(
                        CODE_INDEPENDENT_REVIEW,
                        f"independent_review.{field} {review.get(field)!r} != {value!r}",
                    )

    # 7. historical observation scoped as history, never as current availability
    historical = manifest.get("historical_observations")
    if not isinstance(historical, dict) or historical.get("current_availability_stored_in_manifest") is not False:
        add(CODE_HISTORICAL_SCOPE, "current availability must not be stored in the manifest")

    # 8. PDF snippets are exact observed literals inside their measured bound
    pdf = manifest.get("pdf_evidence")
    if isinstance(pdf, dict):
        for field, value in PINNED_PDF_NUMERICS.items():
            if pdf.get(field) != value:
                add(CODE_PDF_EVIDENCE, f"pdf_evidence.{field} {pdf.get(field)!r} != {value!r}")
        for key, value in (
            ("title", PINNED_TITLE),
            ("references", PINNED_REFERENCES),
            ("callouts", PINNED_CALLOUTS),
        ):
            if pdf.get(key) != value:
                add(CODE_PDF_EVIDENCE, f"pdf_evidence.{key} != independent observed literals")

    # 9. data/parse ledger tree pins
    tree = manifest.get("data_parse_tree")
    if not isinstance(tree, dict):
        add(CODE_LIVE_TREE, "data_parse_tree missing")
    else:
        if tree.get("sanctioned_names") != EXPECTED_DATA_PARSE_NAMES:
            add(CODE_LIVE_TREE, "sanctioned_names != independent 26-name list")
        if tree.get("expected_entry_count") != len(EXPECTED_DATA_PARSE_NAMES):
            add(CODE_LIVE_TREE, "expected_entry_count != 26")
        declared_pins = tree.get("bytepins")
        if not isinstance(declared_pins, dict) or set(declared_pins) != set(EXPECTED_DATA_PARSE_PINS):
            add(CODE_LIVE_TREE, "bytepins key set != independent 26-name list")
        else:
            for name, (size, sha) in EXPECTED_DATA_PARSE_PINS.items():
                pin = declared_pins.get(name)
                if not isinstance(pin, dict) or pin.get("size_bytes") != size or pin.get("sha256") != sha:
                    add(CODE_LIVE_TREE, f"bytepins[{name}] drifted from the independent pin")

    # 10. rights boundary pin
    rights = manifest.get("rights_boundary")
    if isinstance(rights, dict) and rights.get("source_sha256") != EXPECTED_RIGHTS_SOURCE_SHA256:
        add(CODE_RIGHTS_SOURCE, "rights_boundary.source_sha256 != pinned rights source")

    # 11. caps are unchanged and the blocker/readiness state stays blocked
    caps = manifest.get("caps")
    if isinstance(caps, dict):
        for key, value in EXPECTED_CAPS.items():
            if caps.get(key) != value:
                add(CODE_SCHEMA, f"caps.{key} {caps.get(key)!r} != {value!r}")

    vocab_block = manifest.get("closed_vocabularies")
    vocab = vocab_block.get("blocker_codes") if isinstance(vocab_block, dict) else None
    blockers = manifest.get("blockers")
    if isinstance(vocab, list) and isinstance(blockers, list):
        codes = [b.get("code") for b in blockers if isinstance(b, dict)]
        if len(set(codes)) != len(codes):
            add(CODE_BLOCKERS, "duplicate blocker code")
        if set(codes) != set(vocab):
            add(CODE_BLOCKERS, f"blocker codes {sorted(set(codes))} != closed vocabulary")
    readiness = manifest.get("readiness")
    if isinstance(readiness, dict):
        if readiness.get("verdict") != "blocked_pending_resources_and_positive_fixture":
            add(CODE_BLOCKERS, "readiness verdict is not the blocked verdict")
        if readiness.get("positive_fixture_ready") is not False:
            add(CODE_BLOCKERS, "positive_fixture_ready must stay false")

    return errs


# --------------------------------------------------------------------------
# Filesystem verification primitives (independent descriptors only)
# --------------------------------------------------------------------------
def default_reader(path: str):
    """One streaming content open -> (byte length, sha256)."""
    digest = hashlib.sha256()
    size = 0
    with open(path, "rb") as handle:
        while True:
            chunk = handle.read(1 << 20)
            if not chunk:
                break
            size += len(chunk)
            digest.update(chunk)
    return size, digest.hexdigest()


class RecordingOpener:
    """Records every path handed to a content read.

    Used to prove fail-closed behaviour: for a rejected manifest, or for a
    target that escapes its trusted root, ``calls`` must stay empty.
    """

    def __init__(self, delegate=None):
        self.calls: list = []
        self._delegate = delegate or default_reader

    def __call__(self, path: str):
        self.calls.append(path)
        return self._delegate(path)


def _contained(base: str, rel: str) -> bool:
    real_base = os.path.realpath(base)
    real_path = os.path.realpath(os.path.join(base, rel))
    return real_path == real_base or real_path.startswith(real_base + os.sep)


def read_bytes_identity(opener, path: str):
    """Read one file's (byte length, sha256), or (None, None) when unreadable.

    A read failure is a typed absence, never a match: an unreadable file cannot
    silently satisfy an identity requirement.
    """
    try:
        return opener(path)
    except OSError:
        return None, None


def verify_descriptor(opener, base: str, rel: str, size: int, sha: str):
    """Verify ONE independently pinned descriptor. Returns (status, detail).

    Containment is decided by ``realpath`` *before* any read, so a traversal or
    symlink escape is rejected without opening the target. Absence is
    ``unavailable`` and is never turned into a match.
    """
    if not isinstance(base, str) or not os.path.isdir(base):
        return "unavailable", "trusted root unavailable or unconfigured"
    if not _contained(base, rel):
        return "mismatch", "path escapes the trusted root (symlink or traversal)"
    path = os.path.join(base, rel)
    if os.path.islink(path):
        return "mismatch", "symlinked identity is not admitted"
    if not os.path.exists(path):
        return "unavailable", "absent"
    if not os.path.isfile(path):
        return "mismatch", "not a regular file"
    actual_size, actual_sha = read_bytes_identity(opener, path)
    if actual_size is None:
        return "unavailable", "unreadable (read error; never a match)"
    if actual_size != size:
        return "mismatch", f"size {actual_size} != {size}"
    if actual_sha != sha:
        return "mismatch", f"sha256 {actual_sha} != {sha}"
    return "match", ""


def protected25_errors(map_doc) -> list:
    """Every pinned repo file in the protected map must still match."""
    errs = []
    if not isinstance(map_doc, dict):
        return ["protected map is not an object"]
    current = map_doc.get("current25")
    if not isinstance(current, dict):
        return ["protected map lost its current25 block"]
    for rel, pin in current.items():
        path = os.path.join(REPO_ROOT, rel)
        if not os.path.exists(path):
            errs.append(f"protected pin target missing: {rel}")
            continue
        try:
            with open(path, "rb") as handle:
                digest = hashlib.sha256(handle.read()).hexdigest()
            size = os.path.getsize(path)
        except OSError as exc:
            errs.append(f"protected pin unreadable: {rel} ({exc.__class__.__name__})")
            continue
        if size != pin.get("size_bytes") or digest != pin.get("sha256"):
            errs.append(f"protected pin altered: {rel}")
    return errs


# --------------------------------------------------------------------------
# THE ADMISSION ENTRY
# --------------------------------------------------------------------------
REQUIRED_KEYS = (
    "protected_map",
    "synthetic_diagnostic",
    "reference_plan",
    "independent_review",
    "live_tree",
    "rights_source",
)


def admit_and_verify(manifest, roots=None, reader=None) -> dict:
    """The single fail-closed admission boundary for the audit manifest.

    Phase 1 — pure structural admission (schema, closed content, exact
    independent identities, path/credential rules). On any failure the entry
    returns *before* touching the filesystem, so no manifest-supplied path can
    reach an open or a hash.

    Phase 2 — current availability and byte identity, driven only by the
    independent literal descriptors (never by re-read manifest entries). The
    typed report keeps the historical observation separate from the current
    result and refuses to turn "unavailable" into a pass.
    """
    roots = dict(TRUSTED_ROOTS) if roots is None else dict(roots)
    opener = RecordingOpener() if reader is None else reader

    errors = structural_errors(manifest)
    readiness = manifest.get("readiness") if isinstance(manifest, dict) else None
    declared_source_bound = (
        bool(readiness.get("first_pdf_source_bound")) if isinstance(readiness, dict) else False
    )
    report = {
        "structural_admission_passed": not errors,
        "rejection_codes": sorted({code for code, _ in errors}),
        "rejection_details": [f"{code}: {detail}" for code, detail in errors],
        "open_targets": [],
        "resources": {},
        "diagnostics": {},
        "live_tree": {},
        "rights_source": {},
        "declared_source_bound": declared_source_bound,
        "current_source_bound": False,
        "consistency_codes": [],
        "required_current_status": {},
        "verdict": "rejected_fail_closed",
    }

    if errors:
        report["open_targets"] = list(opener.calls)
        return report

    # ---- Phase 2: filesystem identity from independent literals only ----
    def base_for(alias):
        return roots.get(alias)

    for rid, pinned in PINNED_RESOURCES.items():
        status, detail = verify_descriptor(
            opener,
            base_for(pinned["root_alias"]),
            pinned["relative_path"],
            pinned["size_bytes"],
            pinned["sha256"],
        )
        report["resources"][rid] = {
            "historical_observed_state": _resource_historical_state(manifest, rid),
            "current_status": status,
            "current_is_present": status == "match",
            "detail": detail,
        }

    for pinned in PINNED_DIAGNOSTICS:
        doc = pinned["document_id"]
        entry = {}
        for label, rel_key, size_key, sha_key in (
            ("report", "report_relative_path", "report_size_bytes", "report_sha256"),
            ("tei", "tei_relative_path", "tei_size_bytes", "tei_sha256"),
        ):
            status, detail = verify_descriptor(
                opener,
                base_for(pinned["root_alias"]),
                pinned[rel_key],
                pinned[size_key],
                pinned[sha_key],
            )
            entry[label] = {"current_status": status, "detail": detail}
        report["diagnostics"][doc] = entry

    report["protected_map"] = _verify_protected_map(opener, base_for)
    report["synthetic_diagnostic"] = _verify_pin(opener, base_for, PINNED_SYNTHETIC_DIAGNOSTIC)
    report["reference_plan"] = _verify_pin(opener, base_for, PINNED_REFERENCE_PLAN)
    report["independent_review"] = _verify_pin(opener, base_for, PINNED_INDEPENDENT_REVIEW)
    report["live_tree"] = _verify_live_tree(opener)
    report["rights_source"] = _verify_rights_source(opener)

    # ---- typed current status per required key ----
    required = {}
    for rid, entry in report["resources"].items():
        required[f"resource:{rid}"] = entry["current_status"]
    for doc, entry in report["diagnostics"].items():
        for label, value in entry.items():
            required[f"diagnostic:{doc}:{label}"] = value["current_status"]
    for key in REQUIRED_KEYS:
        required[key] = report[key]["current_status"]
    report["required_current_status"] = required

    report["current_source_bound"] = (
        report["resources"]["pdf_arx_1203_2295"]["current_status"] == "match"
    )
    consistency = []
    unavailable = sorted(k for k, s in required.items() if s == "unavailable")
    mismatched = sorted(k for k, s in required.items() if s == "mismatch")
    if unavailable:
        consistency.append(f"{CODE_REQUIRED_UNAVAILABLE}: {', '.join(unavailable)}")
    if mismatched:
        consistency.append(f"{CODE_REQUIRED_MISMATCH}: {', '.join(mismatched)}")
    if declared_source_bound and not report["current_source_bound"]:
        consistency.append(
            f"{CODE_DECLARED_BOUND}: readiness.first_pdf_source_bound=true but the "
            f"source PDF is {report['resources']['pdf_arx_1203_2295']['current_status']}"
        )
    report["consistency_codes"] = consistency
    report["verdict"] = (
        "admitted_verified" if not consistency else "admitted_unverified"
    )
    report["open_targets"] = list(opener.calls)
    return report


def _resource_historical_state(manifest: dict, rid: str):
    for entry in manifest.get("resources") or []:
        if isinstance(entry, dict) and entry.get("id") == rid:
            return entry.get("historical_observed_state")
    return None


def _verify_pin(opener, base_for, pinned) -> dict:
    status, detail = verify_descriptor(
        opener,
        base_for(pinned["root_alias"]),
        pinned["relative_path"],
        pinned["size_bytes"],
        pinned["sha256"],
    )
    return {"current_status": status, "detail": detail}


def _verify_protected_map(opener, base_for) -> dict:
    pinned = PINNED_PROTECTED_MAP
    base = base_for(pinned["root_alias"])
    status, detail = verify_descriptor(
        opener,
        base,
        pinned["relative_path"],
        pinned["size_bytes"],
        pinned["sha256"],
    )
    result = {"current_status": status, "detail": detail, "entry_count_ok": None}
    if status == "match":
        try:
            with open(os.path.join(base, pinned["relative_path"]), "r", encoding="utf-8") as handle:
                map_doc = json.load(handle)
        except (OSError, ValueError) as exc:
            result["current_status"] = "mismatch"
            result["detail"] = f"protected map unreadable or malformed ({exc.__class__.__name__})"
            return result
        result["entry_count_ok"] = len(map_doc.get("current25") or {}) == pinned["entry_count"]
        result["current25_pins_ok"] = not protected25_errors(map_doc)
    return result


def _verify_live_tree(opener) -> dict:
    directory = os.path.join(REPO_ROOT, "data/parse")
    if not os.path.isdir(directory):
        return {"current_status": "unavailable", "detail": "data/parse is missing"}
    names = sorted(os.listdir(directory))
    if names != sorted(EXPECTED_DATA_PARSE_NAMES):
        return {
            "current_status": "mismatch",
            "detail": f"data/parse listing {names} != sanctioned 26 names",
        }
    drifted = []
    for name, (size, sha) in EXPECTED_DATA_PARSE_PINS.items():
        actual_size, actual_sha = read_bytes_identity(opener, os.path.join(directory, name))
        if actual_size is None or actual_size != size or actual_sha != sha:
            drifted.append(name)
    if drifted:
        return {"current_status": "mismatch", "detail": f"byte drift or unreadable: {', '.join(drifted)}"}
    return {"current_status": "match", "detail": ""}


def _verify_rights_source(opener) -> dict:
    path = os.path.join(REPO_ROOT, EXPECTED_RIGHTS_SOURCE_FILE)
    actual_size, actual_sha = read_bytes_identity(opener, path)
    if actual_size is None:
        return {"current_status": "unavailable", "detail": "rights source unreadable"}
    if actual_sha != EXPECTED_RIGHTS_SOURCE_SHA256:
        return {"current_status": "mismatch", "detail": "rights source bytes drifted"}
    if actual_size != EXPECTED_DATA_PARSE_PINS["grobid_model_rights.yaml"][0]:
        return {"current_status": "mismatch", "detail": "rights source size drifted"}
    try:
        with open(path, "r", encoding="utf-8") as handle:
            text = handle.read()
    except OSError:
        return {"current_status": "unavailable", "detail": "rights source unreadable"}
    for literal in (
        "reuse: external_service_only",
        "redistributable: false",
        "rights_status: blocked",
        "missing_or_ambiguous_rights: fail_closed",
    ):
        if literal not in text:
            return {"current_status": "mismatch", "detail": f"rights source lost {literal!r}"}
    return {"current_status": "match", "detail": ""}


# --------------------------------------------------------------------------
# SOURCE CONTRACT (T02 disposition) — closed schema + one check entry
# --------------------------------------------------------------------------
CONTRACT_UNBLOCK_CODES = [
    "lawful_or_interface_compatible_native_access_path_recorded",
    "lexicon_export_or_use_rights_cleared_with_recorded_decision",
    "gazetteer_and_bundle_cap_policy_change_publicly_reviewed",
    "grobid_version_and_feature_abi_aligned_to_pinned_baseline",
    "independent_visual_layout_review_recorded",
    "complete_labelled_bibliography_inventory_and_callout_linking_established",
    "java_renderer_comparator_rows_recorded_under_separate_identity",
]
CONTRACT_FIXTURE_ABSENCE_CODES = [
    "no_independent_positive_fixture",
    "bibliography_inventory_incomplete_and_callout_linking_not_established",
    "visual_layout_not_independently_reviewed",
]

CODE_CONTRACT_SCHEMA = "source_contract_schema_invalid"
CODE_CONTRACT_POLARITY = "source_contract_fake_permitted_or_ready_state"
CODE_CONTRACT_IDENTITY = "source_contract_identity_mismatch"

CONTRACT_BLOCKER_CODES = list(CLOSED_BLOCKER_CODES)

CONTRACT_SCHEMA = {
    "t": "obj",
    "closed": True,
    "fields": {
        "schema_version": {"t": "enum", "values": ["native-candidate-source-contract.v1"]},
        "contract_kind": {"t": "enum", "values": ["typed_source_contract_disposition"]},
        "milestone_id": {"t": "enum", "values": [EXPECTED_MILESTONE]},
        "slice_id": {"t": "enum", "values": [EXPECTED_SLICE]},
        "task_id": {"t": "enum", "values": ["T02"]},
        "consumes": {
            "t": "obj",
            "closed": True,
            "fields": {
                "audit_manifest": {"t": "enum", "values": [MANIFEST_REL]},
                "audit_schema_version": {"t": "enum", "values": [EXPECTED_SCHEMA_VERSION]},
                "audit_task_id": {"t": "enum", "values": [EXPECTED_TASK]},
                "audit_verdict": {
                    "t": "enum",
                    "values": ["blocked_pending_resources_and_positive_fixture"],
                },
                "admission_boundary": {
                    "t": "enum",
                    "values": ["single_fail_closed_pre_open_admission"],
                },
            },
        },
        "disposition": {"t": "enum", "values": ["blocked"]},
        "positive_fixture_ready": {"t": "bool", "value": False},
        "expected_positive": {"t": "null"},
        "expected_positive_absent_reason_codes": {
            "t": "arr",
            "min": 1,
            "item": {"t": "enum", "values": CONTRACT_FIXTURE_ABSENCE_CODES},
        },
        "identities": {
            "t": "obj",
            "closed": True,
            "fields": {
                "pdf": {
                    "t": "obj",
                    "closed": True,
                    "fields": {
                        "id": {"t": "enum", "values": ["pdf_arx_1203_2295"]},
                        "document_id": {"t": "enum", "values": ["1203.2295"]},
                        "kind": {"t": "enum", "values": ["pdf_source"]},
                        "root_alias": {"t": "enum", "values": sorted(TRUSTED_ROOTS)},
                        "relative_path": {"t": "str"},
                        "size_bytes": {"t": "int", "min": 1},
                        "sha256": {"t": "hex64"},
                    },
                },
                "resources": {
                    "t": "arr",
                    "len": len(PINNED_RESOURCES),
                    "item": {
                        "t": "obj",
                        "closed": True,
                        "fields": {
                            "id": {"t": "enum", "values": sorted(PINNED_RESOURCES)},
                            "kind": {"t": "enum", "values": RESOURCE_KINDS},
                            "root_alias": {"t": "enum", "values": sorted(TRUSTED_ROOTS)},
                            "relative_path": {"t": "str"},
                            "size_bytes": {"t": "int", "min": 1},
                            "sha256": {"t": "hex64"},
                        },
                    },
                },
            },
        },
        "blocker_codes": {
            "t": "arr",
            "min": 1,
            "item": {"t": "enum", "values": CONTRACT_BLOCKER_CODES},
        },
        "blocked_dimensions": {
            "t": "obj",
            "closed": True,
            "fields": {
                "resource_readiness": {
                    "t": "enum",
                    "values": ["blocked_external_service_only"],
                },
                "lexicon_export_and_use": {
                    "t": "enum",
                    "values": ["blocked_rights_and_caps"],
                },
                "native_access_and_abi": {
                    "t": "enum",
                    "values": ["blocked_version_drift_and_feature_abi_gap"],
                },
                "independent_positive_fixture": {"t": "enum", "values": ["absent"]},
                "visual_layout_review": {"t": "enum", "values": ["not_performed"]},
                "bibliography_and_linking": {"t": "enum", "values": ["not_established"]},
                "scholarly_quality": {"t": "enum", "values": ["not_claimed"]},
                "rights_boundary": {"t": "enum", "values": ["blocked_fail_closed"]},
            },
        },
        "policy_locks": {
            "t": "obj",
            "closed": True,
            "fields": dict(
                [(key, {"t": "bool", "value": False}) for key in POLICY_LOCK_KEYS]
                + [
                    ("auto_promote", {"t": "enum", "values": ["forbidden"]}),
                    ("graph_write", {"t": "enum", "values": ["forbidden"]}),
                ]
            ),
        },
        "rights_boundary": {
            "t": "obj",
            "closed": True,
            "fields": {
                "reuse": {"t": "enum", "values": ["external_service_only"]},
                "redistributable": {"t": "bool", "value": False},
                "rights_status": {"t": "enum", "values": ["blocked"]},
                "missing_or_ambiguous_rights": {"t": "enum", "values": ["fail_closed"]},
                "model_bytes_committed": {"t": "bool", "value": False},
                "rights_approval_claimed": {"t": "bool", "value": False},
            },
        },
        "no_pretend_success": {
            "t": "obj",
            "closed": True,
            "fields": {
                "expected_title": {"t": "null"},
                "expected_references": {"t": "null"},
                "expected_callouts": {"t": "null"},
                "scientific_quality_claimed": {"t": "bool", "value": False},
            },
        },
        "required_to_unblock": {
            "t": "arr",
            "min": 1,
            "item": {"t": "enum", "values": CONTRACT_UNBLOCK_CODES},
        },
        "next_action": {
            "t": "enum",
            "values": ["stop_and_public_replan_before_any_source_work"],
        },
        "independent_review": {
            "t": "obj",
            "closed": True,
            "fields": {
                "root_alias": {"t": "enum", "values": ["rust-grobid-useful-pdf"]},
                "relative_path": {"t": "str"},
                "size_bytes": {"t": "int", "min": 1},
                "sha256": {"t": "hex64"},
            },
        },
    },
}

PINNED_PDF_IDENTITY = {
    "id": "pdf_arx_1203_2295",
    "document_id": "1203.2295",
    "kind": "pdf_source",
    "root_alias": PINNED_RESOURCES["pdf_arx_1203_2295"]["root_alias"],
    "relative_path": PINNED_RESOURCES["pdf_arx_1203_2295"]["relative_path"],
    "size_bytes": PINNED_RESOURCES["pdf_arx_1203_2295"]["size_bytes"],
    "sha256": PINNED_RESOURCES["pdf_arx_1203_2295"]["sha256"],
}


def source_contract_errors(contract, manifest) -> list:
    """Check the typed disposition. Returns ``[(code, detail), ...]``."""
    errs: list = []

    def add(code, detail):
        errs.append((code, detail))

    schema_errs: list = []
    _validate(CONTRACT_SCHEMA, contract, "$", schema_errs)
    for message in schema_errs:
        add(CODE_CONTRACT_SCHEMA, message)
    if not isinstance(contract, dict):
        return errs

    # identity must equal the independently pinned audit identities. A schema
    # failure above does not skip these: the checks below are safe on any dict.
    identities = contract.get("identities")
    if not isinstance(identities, dict):
        add(CODE_CONTRACT_IDENTITY, "identities block missing")
    else:
        if identities.get("pdf") != PINNED_PDF_IDENTITY:
            add(CODE_CONTRACT_IDENTITY, "identities.pdf != pinned source PDF identity")
        resources = identities.get("resources")
        if resources != PINNED_RESOURCE_IDENTITIES:
            add(CODE_CONTRACT_IDENTITY, "identities.resources != pinned resource identities")
        if isinstance(resources, list):
            for index, entry in enumerate(resources):
                rel = entry.get("relative_path") if isinstance(entry, dict) else None
                for message in path_safety_errors(rel):
                    add(CODE_CONTRACT_IDENTITY, f"identities.resources[{index}]: {message}")
        pdf_identity = identities.get("pdf")
        if isinstance(pdf_identity, dict):
            for message in path_safety_errors(pdf_identity.get("relative_path")):
                add(CODE_CONTRACT_IDENTITY, f"identities.pdf: {message}")

    review = contract.get("independent_review")
    if review != PINNED_INDEPENDENT_REVIEW:
        add(CODE_CONTRACT_IDENTITY, "independent_review != pinned review bytes")
    elif isinstance(review, dict):
        for message in path_safety_errors(review.get("relative_path")):
            add(CODE_CONTRACT_IDENTITY, f"independent_review: {message}")

    # exact blocker codes carried from the corrected audit
    blocker_codes = contract.get("blocker_codes")
    if not isinstance(blocker_codes, list) or set(blocker_codes) != set(CONTRACT_BLOCKER_CODES):
        add(CODE_CONTRACT_IDENTITY, "blocker_codes != the audited closed blocker set")
    audit_blockers = manifest.get("blockers")
    if isinstance(audit_blockers, list):
        audit_codes = {b.get("code") for b in audit_blockers if isinstance(b, dict)}
        if not isinstance(blocker_codes, list) or set(blocker_codes) != audit_codes:
            add(CODE_CONTRACT_IDENTITY, "blocker_codes != audit manifest blocker codes")

    if contract.get("policy_locks") != manifest.get("policy_locks"):
        add(CODE_CONTRACT_POLARITY, "policy_locks differ from the audit manifest")
    audit_rights = manifest.get("rights_boundary") or {}
    contract_rights = contract.get("rights_boundary") or {}
    for key in ("reuse", "redistributable", "rights_status", "missing_or_ambiguous_rights"):
        if contract_rights.get(key) != audit_rights.get(key):
            add(CODE_CONTRACT_POLARITY, f"rights_boundary.{key} differs from the audit")
    if (
        manifest.get("genuine_export") is not False
        or manifest.get("product_or_scientific_acceptance") is not False
    ):
        add(CODE_CONTRACT_POLARITY, "consumed audit does not stay candidate-only")
    return errs


def check_source_contract(contract, manifest) -> dict:
    """Single check entry for the disposition; no filesystem access."""
    errors = source_contract_errors(contract, manifest)
    return {
        "accepted": not errors,
        "rejection_codes": sorted({code for code, _ in errors}),
        "rejection_details": [f"{code}: {detail}" for code, detail in errors],
        "disposition": contract.get("disposition") if isinstance(contract, dict) else None,
        "positive_fixture_ready": (
            contract.get("positive_fixture_ready") if isinstance(contract, dict) else None
        ),
        "expected_positive": (
            contract.get("expected_positive") if isinstance(contract, dict) else None
        ),
    }


# --------------------------------------------------------------------------
# Tests — admission accepts the real manifest and reports current bytes
# --------------------------------------------------------------------------
def test_real_manifest_admits_and_never_claims_verification_it_cannot_show():
    """The committed manifest through the one entry — on any host.

    A checkout that lacks the git-ignored PDF, the vendor models or the /tmp
    evidence root is *honest*: those identities report ``unavailable`` and the
    verdict falls to ``admitted_unverified`` with the reason codes. Forbidden on
    every host is the opposite direction — ``admitted_verified`` while a
    required byte did not match — so this asserts the biconditional instead of
    requiring one particular machine to hold the bytes.
    """
    report = admit_and_verify(load_manifest())
    assert report["structural_admission_passed"] is True, report["rejection_details"]
    assert report["rejection_codes"] == []

    # the report covers exactly the pinned identities, so nothing here is vacuous
    assert sorted(report["resources"]) == sorted(PINNED_RESOURCES)
    assert sorted(report["diagnostics"]) == sorted(DIAGNOSTIC_IDS)
    for doc in DIAGNOSTIC_IDS:
        assert sorted(report["diagnostics"][doc]) == ["report", "tei"]
    for key in REQUIRED_KEYS:
        assert key in report["required_current_status"]

    statuses = report["required_current_status"]
    non_match = sorted(key for key, status in statuses.items() if status != "match")
    assert report["verdict"] in ("admitted_verified", "admitted_unverified")
    assert (report["verdict"] == "admitted_verified") == (not non_match), (
        report["consistency_codes"]
    )
    assert report["current_source_bound"] == (
        statuses["resource:pdf_arx_1203_2295"] == "match"
    )

    if not non_match:
        assert report["consistency_codes"] == []
        assert report["current_source_bound"] is True
        return

    # every non-matching required byte must carry its own reason code
    for key in non_match:
        expected = (
            CODE_REQUIRED_MISMATCH if statuses[key] == "mismatch" else CODE_REQUIRED_UNAVAILABLE
        )
        assert any(
            code.startswith(expected) and key in code for code in report["consistency_codes"]
        ), (key, report["consistency_codes"])
    if report["declared_source_bound"] and statuses["resource:pdf_arx_1203_2295"] != "match":
        assert any(
            code.startswith(CODE_DECLARED_BOUND) for code in report["consistency_codes"]
        ), report["consistency_codes"]


# --------------------------------------------------------------------------
# Tests — CORRECTION 1: actual availability, honest polarity
# --------------------------------------------------------------------------
def test_historical_observation_is_never_treated_as_current_availability():
    manifest = load_manifest()
    with tempfile.TemporaryDirectory() as tmp:
        roots = dict(TRUSTED_ROOTS)
        roots["repo"] = tmp
        report = admit_and_verify(manifest, roots=roots)

    assert report["structural_admission_passed"] is True
    pdf = report["resources"]["pdf_arx_1203_2295"]
    assert pdf["historical_observed_state"] == "present"  # history stays history
    assert pdf["current_status"] == "unavailable"  # current truth, not a pass
    assert pdf["current_is_present"] is False
    assert report["current_source_bound"] is False
    assert report["declared_source_bound"] is True
    assert report["verdict"] == "admitted_unverified"
    joined = " | ".join(report["consistency_codes"])
    assert CODE_DECLARED_BOUND in joined, report["consistency_codes"]
    assert "resource:pdf_arx_1203_2295" in joined


def test_missing_pdf_is_never_read_and_other_required_bytes_still_verified():
    manifest = load_manifest()
    with tempfile.TemporaryDirectory() as tmp:
        roots = dict(TRUSTED_ROOTS)
        roots["repo"] = tmp
        spy = RecordingOpener()
        report = admit_and_verify(manifest, roots=roots, reader=spy)
        pdf_path = os.path.join(
            tmp, PINNED_RESOURCES["pdf_arx_1203_2295"]["relative_path"]
        )
        assert pdf_path not in spy.calls, "an absent target must not be opened"
    assert report["required_current_status"]["resource:pdf_arx_1203_2295"] == "unavailable"


def test_absent_protected_map_is_reported_unavailable_not_passed():
    manifest = load_manifest()
    with tempfile.TemporaryDirectory() as tmp:
        roots = dict(TRUSTED_ROOTS)
        roots["rust-grobid-useful-pdf"] = tmp
        spy = RecordingOpener()
        report = admit_and_verify(manifest, roots=roots, reader=spy)
        scanned = [p for p in spy.calls if p.startswith(tmp)]
        assert scanned == [], f"an unavailable trusted root must not be scanned: {scanned}"

    assert report["protected_map"]["current_status"] == "unavailable"
    assert report["protected_map"]["entry_count_ok"] is None
    assert report["verdict"] == "admitted_unverified"
    assert any("protected_map" in code for code in report["consistency_codes"])


def test_wrong_bytes_report_mismatch_and_are_never_upgraded_to_match():
    with tempfile.TemporaryDirectory() as tmp:
        inert = os.path.join(tmp, "inert.bin")
        with open(inert, "wb") as handle:
            handle.write(b"not-the-pinned-bytes")
        spy = RecordingOpener()
        status, detail = verify_descriptor(spy, tmp, "inert.bin", 4, "0" * 64)
        assert status == "mismatch"
        assert "size" in detail or "sha256" in detail
        status, _ = verify_descriptor(
            spy, tmp, "inert.bin", len(b"not-the-pinned-bytes"), "0" * 64
        )
        assert status == "mismatch"
        status, _ = verify_descriptor(spy, tmp, "absent.bin", 1, "0" * 64)
        assert status == "unavailable"


# --------------------------------------------------------------------------
# Tests — CORRECTION 2: closed non-snippet content
# --------------------------------------------------------------------------
def test_unknown_extra_or_reordered_decision_reference_is_rejected():
    for tamper in (["D127", "D449", "D491", "ADR-076", "D999"], ["D127", "D449", "D491", "D491"],
                   ["ADR-076", "D491", "D449", "D127"]):
        manifest = copy.deepcopy(load_manifest())
        manifest["authority"]["decision_refs"] = tamper
        spy = RecordingOpener()
        report = admit_and_verify(manifest, reader=spy)
        assert report["structural_admission_passed"] is False, tamper
        assert CODE_DECISION_REFS in report["rejection_codes"]
        assert spy.calls == [], "a rejected manifest must not reach the filesystem"


def test_resource_format_and_stage_are_bound_to_the_identity():
    manifest = copy.deepcopy(load_manifest())
    by_id = {entry["id"]: entry for entry in manifest["resources"]}
    by_id["grobid_model_header"]["stage"] = "citation"  # identity-inconsistent
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert CODE_RESOURCE_IDENTITY in report["rejection_codes"]

    manifest = copy.deepcopy(load_manifest())
    by_id = {entry["id"]: entry for entry in manifest["resources"]}
    by_id["grobid_lexicon_places_location"]["format"] = "arbitrary_appended_payload"
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert CODE_RESOURCE_IDENTITY in report["rejection_codes"]


def test_resource_identity_substitution_is_rejected():
    for field, value in (
        ("sha256", "0" * 64),
        ("size_bytes", 1),
        ("kind", "gazetteer_lexicon"),
        ("root_alias", "rust-grobid-integration"),
        ("relative_path", "models/citation/model.wapiti"),
    ):
        manifest = copy.deepcopy(load_manifest())
        manifest["resources"][1][field] = value
        report = admit_and_verify(manifest, reader=RecordingOpener())
        assert CODE_RESOURCE_IDENTITY in report["rejection_codes"], field


def test_arbitrary_string_cannot_hide_in_a_closed_field():
    manifest = copy.deepcopy(load_manifest())
    manifest["inventory_scope"] = "complete_full_inventory_of_every_channel"
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert report["structural_admission_passed"] is False
    assert CODE_SCHEMA in report["rejection_codes"]

    manifest = copy.deepcopy(load_manifest())
    manifest["rights_boundary"]["rights_status"] = "cleared"
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert report["structural_admission_passed"] is False


# --------------------------------------------------------------------------
# Tests — CORRECTION 3: exact three negative diagnostic identities
# --------------------------------------------------------------------------
def _diagnostic_tampers():
    yield "duplicate_document_id", lambda entries: entries.__setitem__(2, copy.deepcopy(entries[0]))
    yield "omitted_third", lambda entries: entries.pop()
    yield "swapped_report_paths", lambda entries: _swap(entries, "report_relative_path")
    yield "swapped_report_sha256", lambda entries: _swap(entries, "report_sha256")
    yield "swapped_tei_sha256", lambda entries: _swap(entries, "tei_sha256")
    yield "substituted_size", lambda entries: entries[1].__setitem__("report_size_bytes", 1)
    yield "extra_diagnostic", lambda entries: entries.append(
        {**copy.deepcopy(entries[2]), "document_id": "1203.2295", "outcome_code": "other"}
    )
    yield "wrong_outcome_for_document", lambda entries: entries[0].__setitem__(
        "outcome_code", "header_font_mean_refusal_and_references_region_absent"
    )


def _swap(entries, field):
    a, b = entries[1][field], entries[2][field]
    entries[1][field], entries[2][field] = b, a


def test_diagnostic_mapping_must_equal_the_independent_pin_table():
    # the pin table is independent by construction: the real manifest matches it
    assert load_manifest()["diagnostic_negatives"] == PINNED_DIAGNOSTICS
    for name, tamper in _diagnostic_tampers():
        manifest = copy.deepcopy(load_manifest())
        tamper(manifest["diagnostic_negatives"])
        spy = RecordingOpener()
        report = admit_and_verify(manifest, reader=spy)
        assert report["structural_admission_passed"] is False, name
        assert CODE_DIAGNOSTIC_MAP in report["rejection_codes"], name
        assert spy.calls == [], f"{name}: must reject before any open"


def test_diagnostic_identities_are_verified_from_pinned_literals_not_the_manifest():
    # a manifest that agrees with itself but not with the pins must still fail
    manifest = copy.deepcopy(load_manifest())
    for entry in manifest["diagnostic_negatives"]:
        entry["report_sha256"] = "a" * 64
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert CODE_DIAGNOSTIC_MAP in report["rejection_codes"]

    # the real manifest's three identities are checked against the pins in the
    # real tree: a drifted local byte is a hard failure, while an absent
    # evidence root is the typed "unavailable" of section 10 — never a match.
    report = admit_and_verify(load_manifest())
    for doc in DIAGNOSTIC_IDS:
        for label in ("report", "tei"):
            entry = report["diagnostics"][doc][label]
            assert entry["current_status"] in ("match", "unavailable"), (
                doc,
                label,
                entry["detail"],
            )
    codes = {entry["outcome_code"] for entry in PINNED_DIAGNOSTICS}
    assert "header_zero_font_refusal_and_reference_lines_unadmitted" in codes
    assert "header_font_mean_refusal_and_references_region_absent" in codes


# --------------------------------------------------------------------------
# Tests — CORRECTION 4: one fail-closed pre-open boundary
# --------------------------------------------------------------------------
def test_credential_like_or_unsafe_path_is_rejected_before_any_open():
    for unsafe in (
        "keys/id_rsa",
        "creds/credentials.json",
        "certs/server.pem",
        "data/.env",
        "/etc/passwd",
        "../../etc/passwd",
        "models/../../secrets.txt",
        "models\\header\\model.wapiti",
    ):
        manifest = copy.deepcopy(load_manifest())
        manifest["resources"][0]["relative_path"] = unsafe
        spy = RecordingOpener()
        report = admit_and_verify(manifest, reader=spy)
        assert report["structural_admission_passed"] is False, unsafe
        assert report["verdict"] == "rejected_fail_closed"
        assert spy.calls == [], f"{unsafe}: nothing may be opened"
        assert report["open_targets"] == []


def test_symlink_escape_is_rejected_without_reading_the_target():
    with tempfile.TemporaryDirectory() as tmp:
        outside = os.path.join(tmp, "outside")
        os.makedirs(outside)
        with open(os.path.join(outside, "real.bin"), "wb") as handle:
            handle.write(b"x")
        base = os.path.join(tmp, "base")
        os.makedirs(base)
        link = os.path.join(base, "escape.bin")
        try:
            os.symlink(os.path.join(outside, "real.bin"), link)
        except (OSError, NotImplementedError):
            return  # platform without symlink support
        spy = RecordingOpener()
        status, detail = verify_descriptor(
            spy, base, "escape.bin", 1, hashlib.sha256(b"x").hexdigest()
        )
        assert status == "mismatch"
        assert "escape" in detail
        assert spy.calls == [], "an escaping path must not be read"

        inner = os.path.join(base, "inner.bin")
        with open(inner, "wb") as handle:
            handle.write(b"x")
        inner_link = os.path.join(base, "inner-link.bin")
        try:
            os.symlink(inner, inner_link)
        except (OSError, NotImplementedError):
            return
        spy = RecordingOpener()
        status, _ = verify_descriptor(spy, base, "inner-link.bin", 1, hashlib.sha256(b"x").hexdigest())
        assert status == "mismatch"
        assert spy.calls == [], "a symlinked identity must not be read"


def test_incomplete_or_malformed_manifest_is_rejected_wholesale():
    for mutate in (
        lambda m: m.pop("readiness"),
        lambda m: m.__setitem__("extra_top_level_key", "x"),
        lambda m: m["policy_locks"].__setitem__("unexpected", True),
        lambda m: m["caps"].__setitem__("bundle_entries_per_channel", "100000"),
        lambda m: m["resources"][0].__setitem__("sha256", "not-a-digest"),
    ):
        manifest = copy.deepcopy(load_manifest())
        mutate(manifest)
        spy = RecordingOpener()
        report = admit_and_verify(manifest, reader=spy)
        assert report["structural_admission_passed"] is False
        assert report["verdict"] == "rejected_fail_closed"
        assert spy.calls == []
        assert set(report["rejection_codes"]) <= REJECTION_CODES, report["rejection_codes"]


# --------------------------------------------------------------------------
# Tests — closed blocker set, rights, caps, snippets, live tree
# --------------------------------------------------------------------------
def test_blocker_set_is_exact_and_readiness_stays_blocked():
    manifest = copy.deepcopy(load_manifest())
    manifest["blockers"] = manifest["blockers"][:-1]  # drop one blocker
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert CODE_BLOCKERS in report["rejection_codes"]

    manifest = copy.deepcopy(load_manifest())
    manifest["readiness"]["positive_fixture_ready"] = True
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert report["structural_admission_passed"] is False

    manifest = copy.deepcopy(load_manifest())
    manifest["blockers"][0]["code"] = "some_freeform_resource_string"
    report = admit_and_verify(manifest, reader=RecordingOpener())
    assert report["structural_admission_passed"] is False


def test_rights_caps_and_policy_locks_stay_blocked():
    manifest = copy.deepcopy(load_manifest())
    manifest["rights_boundary"]["redistributable"] = True
    assert admit_and_verify(manifest, reader=RecordingOpener())["structural_admission_passed"] is False

    manifest = copy.deepcopy(load_manifest())
    manifest["caps"]["cap_raise_permitted"] = True
    assert admit_and_verify(manifest, reader=RecordingOpener())["structural_admission_passed"] is False

    manifest = copy.deepcopy(load_manifest())
    for entry in manifest["resources"]:
        if entry["kind"] == "crf_model_weights":
            entry["reuse"] = "permitted"
    assert admit_and_verify(manifest, reader=RecordingOpener())["structural_admission_passed"] is False

    manifest = copy.deepcopy(load_manifest())
    manifest["rights_boundary"]["source_sha256"] = "0" * 64
    assert CODE_RIGHTS_SOURCE in admit_and_verify(manifest, reader=RecordingOpener())["rejection_codes"]


def test_snippets_are_exact_bounded_observations_and_do_not_claim_truth():
    manifest = load_manifest()
    report = admit_and_verify(manifest)
    assert report["structural_admission_passed"] is True

    over = copy.deepcopy(manifest)
    over["pdf_evidence"]["title"]["chars"] = PINNED_SNIPPET_BOUND + 1
    assert CODE_PDF_EVIDENCE in admit_and_verify(over, reader=RecordingOpener())["rejection_codes"]

    lied = copy.deepcopy(manifest)
    lied["pdf_evidence"]["title"]["independent_review"] = True
    assert admit_and_verify(lied, reader=RecordingOpener())["structural_admission_passed"] is False

    changed = copy.deepcopy(manifest)
    changed["pdf_evidence"]["references"][0]["snippet"] = "an invented reference line"
    assert CODE_PDF_EVIDENCE in admit_and_verify(changed, reader=RecordingOpener())["rejection_codes"]


def test_two_selected_refs_never_establish_a_bibliography():
    manifest = load_manifest()
    pdf = manifest["pdf_evidence"]
    assert pdf["reference_list_total_labels"] == 22
    assert pdf["target_reference_labels"] == [11, 18]
    assert pdf["wrapped_or_incomplete_refs_unresolved"] == 22
    assert pdf["complete_bibliography_established"] is False
    assert pdf["linking_established"] is False
    report = admit_and_verify(manifest)
    assert report["structural_admission_passed"] is True
    assert report["resources"]["pdf_arx_1203_2295"]["current_is_present"] == (
        report["resources"]["pdf_arx_1203_2295"]["current_status"] == "match"
    )


def test_live_data_parse_tree_and_rights_source_are_byte_pinned():
    directory = os.path.join(REPO_ROOT, "data/parse")
    names = sorted(os.listdir(directory))
    assert names == sorted(EXPECTED_DATA_PARSE_NAMES)
    assert len(names) == len(EXPECTED_DATA_PARSE_NAMES) == 26
    assert not any(os.path.isdir(os.path.join(directory, name)) for name in names)

    manifest = copy.deepcopy(load_manifest())
    manifest["data_parse_tree"]["sanctioned_names"] = EXPECTED_DATA_PARSE_NAMES[:-1]
    assert CODE_LIVE_TREE in admit_and_verify(manifest, reader=RecordingOpener())["rejection_codes"]

    manifest = copy.deepcopy(load_manifest())
    manifest["data_parse_tree"]["bytepins"]["SCHEMA.yaml"]["sha256"] = "0" * 64
    assert CODE_LIVE_TREE in admit_and_verify(manifest, reader=RecordingOpener())["rejection_codes"]

    report = admit_and_verify(load_manifest())
    assert report["live_tree"]["current_status"] == "match"
    assert report["rights_source"]["current_status"] == "match"


def test_synthetic_diagnostic_stays_synthetic_and_not_copied():
    manifest = load_manifest()
    synthetic = manifest["synthetic_diagnostic"]
    assert synthetic["is_synthetic"] is True
    assert synthetic["scholarly_lexicon"] is False
    assert synthetic["copied_to_carrier"] is False
    assert synthetic["usable_as_real_resource"] is False
    carrier = os.path.join(REPO_ROOT, os.path.dirname(MANIFEST_REL))
    for name in os.listdir(carrier):
        low = name.lower()
        assert not low.endswith((".wapiti", ".txt")), f"restricted asset in carrier: {name}"
        assert "lexicon" not in low, f"synthetic lexicon copy in carrier: {name}"


def test_lexicon_counts_are_pinned_observations():
    """The last non-snippet numeric field is a pinned observation, not free input."""
    by_id = {entry["id"]: entry for entry in load_manifest()["resources"]}
    assert by_id["grobid_lexicon_places_location"]["counts"] == PINNED_LEXICON_COUNTS

    for mutate in (
        lambda m: m["resources"][4]["counts"].__setitem__("unique_trimmed_nonblank_lines", 1),
        lambda m: m["resources"][4]["counts"].__setitem__(
            "current_loader_applicability", "within_current_caps"
        ),
        lambda m: m["resources"][0].__setitem__("counts", dict(PINNED_LEXICON_COUNTS)),
    ):
        manifest = copy.deepcopy(load_manifest())
        mutate(manifest)
        spy = RecordingOpener()
        report = admit_and_verify(manifest, reader=spy)
        assert CODE_RESOURCE_IDENTITY in report["rejection_codes"]
        assert spy.calls == []


# --------------------------------------------------------------------------
# Tests — source-contract disposition (the original T02 outcome)
# --------------------------------------------------------------------------
def test_source_contract_records_a_typed_blocked_disposition():
    manifest = load_manifest()
    contract = load_source_contract()
    result = check_source_contract(contract, manifest)
    assert result["accepted"] is True, result["rejection_details"]
    assert contract["disposition"] == "blocked"
    assert contract["positive_fixture_ready"] is False
    assert contract["expected_positive"] is None
    assert contract["no_pretend_success"] == {
        "expected_title": None,
        "expected_references": None,
        "expected_callouts": None,
        "scientific_quality_claimed": False,
    }
    assert contract["identities"]["pdf"] == PINNED_PDF_IDENTITY
    assert contract["identities"]["resources"] == PINNED_RESOURCE_IDENTITIES
    assert set(contract["blocker_codes"]) == {
        b["code"] for b in manifest["blockers"]
    }
    assert contract["next_action"] == "stop_and_public_replan_before_any_source_work"


def test_source_contract_rejects_fake_permitted_or_ready_states():
    manifest = load_manifest()
    for mutate, expected_code in (
        (lambda c: c.__setitem__("disposition", "supported"), CODE_CONTRACT_SCHEMA),
        (lambda c: c.__setitem__("positive_fixture_ready", True), CODE_CONTRACT_SCHEMA),
        (lambda c: c.__setitem__("expected_positive", {"title": "TECHNIQUES FOR SOLVING SUDOKU PUZZLES"}), CODE_CONTRACT_SCHEMA),
        (lambda c: c["no_pretend_success"].__setitem__("expected_title", "TECHNIQUES FOR SOLVING SUDOKU PUZZLES"), CODE_CONTRACT_SCHEMA),
        (lambda c: c["no_pretend_success"].__setitem__("scientific_quality_claimed", True), CODE_CONTRACT_SCHEMA),
        (lambda c: c["rights_boundary"].__setitem__("rights_status", "cleared"), CODE_CONTRACT_SCHEMA),
        (lambda c: c["policy_locks"].__setitem__("import_eligible", True), CODE_CONTRACT_SCHEMA),
        (lambda c: c["rights_boundary"].__setitem__("rights_approval_claimed", True), CODE_CONTRACT_SCHEMA),
        (lambda c: c.__setitem__("next_action", "proceed_with_native_extraction"), CODE_CONTRACT_SCHEMA),
        (lambda c: c.__setitem__("extra_permitted_state", "ready"), CODE_CONTRACT_SCHEMA),
        (lambda c: c["required_to_unblock"].append("pretend_it_is_ready"), CODE_CONTRACT_SCHEMA),
    ):
        contract = copy.deepcopy(load_source_contract())
        mutate(contract)
        result = check_source_contract(contract, manifest)
        assert result["accepted"] is False
        assert expected_code in result["rejection_codes"]


def test_source_contract_identities_and_codes_must_match_the_admitted_audit():
    manifest = load_manifest()
    for mutate, expected_code in (
        (lambda c: c["identities"]["pdf"].__setitem__("sha256", "0" * 64), CODE_CONTRACT_IDENTITY),
        (lambda c: c["identities"]["resources"][0].__setitem__("size_bytes", 1), CODE_CONTRACT_IDENTITY),
        (lambda c: c["identities"]["resources"][0].__setitem__("kind", "arbitrary_payload"), CODE_CONTRACT_IDENTITY),
        (lambda c: c["identities"]["resources"][0].__setitem__("relative_path", "keys/id_rsa"), CODE_CONTRACT_IDENTITY),
        (lambda c: c.__setitem__("blocker_codes", c["blocker_codes"][:-1]), CODE_CONTRACT_IDENTITY),
        (lambda c: c.__setitem__("blocker_codes", c["blocker_codes"] + ["invented_blocker"]), CODE_CONTRACT_IDENTITY),
        (lambda c: c["independent_review"].__setitem__("sha256", "0" * 64), CODE_CONTRACT_IDENTITY),
    ):
        contract = copy.deepcopy(load_source_contract())
        mutate(contract)
        result = check_source_contract(contract, manifest)
        assert result["accepted"] is False, contract["disposition"]
        assert expected_code in result["rejection_codes"]


def test_source_contract_schema_is_closed():
    manifest = load_manifest()
    contract = copy.deepcopy(load_source_contract())
    contract["unknown_key"] = "x"
    result = check_source_contract(contract, manifest)
    assert result["accepted"] is False
    assert CODE_CONTRACT_SCHEMA in result["rejection_codes"]
