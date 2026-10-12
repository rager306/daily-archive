"""Adapter: milestone-closeout criterion disposition oracle (M516-th0oby/S13).

Milestone success criterion SC7 clause 1 is *"governor preflight reports no high
severity flag at closeout"*. The criterion is only machine-checkable once every
high-capable flag CLASS carries a recorded disposition. That disposition is DATA
(``scripts/governor/checks/closeout_criterion_dispositions.yaml``, S13/T01); this
module is the pure, independently testable reader that turns
``(registry, live high flags)`` into findings.

Three findings fall out of the reconciliation:

  - ``undisposed`` -- a live high-severity flag CLASS has NO record in the
    registry. The criterion is not satisfied for that class.
  - ``contradiction`` -- a live high-severity flag CLASS has a record whose
    disposition is ``measured``. ``measured`` claims the class is absent from
    the governor's own live report, so a live class with a ``measured`` record is
    a stale clear, not a pass.
  - ``malformed`` -- a record that cannot be trusted either way: not a mapping,
    missing or empty required field, a disposition outside the closed
    vocabulary, or a reason outside the registry's ``reason_vocabulary``.

The registry is a REQUIRED input. A missing or malformed registry RAISES rather
than reporting a vacuous "all disposed" zero, so a deleted registry can never
read as a green criterion -- the same fail-closed contract the S08 axis
``declaration_sync_gap.py`` applies to its declaration surfaces.

Keying is by flag CLASS: the flag id before any parameterized ``@`` suffix
(``artifact_file_missing@M514`` and ``artifact_file_missing`` share the class
``artifact_file_missing``), so a per-milestone parameter never needs a
per-milestone entry. Only severities in the registry's
``required_for_severities`` are evaluated; a ``medium`` flag is not failed
closed.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

import yaml

__all__ = [
    "CLOSEOUT_CRITERION_UNDISPOSED",
    "DISPOSITION_VOCABULARY",
    "REQUIRED_RECORD_FIELDS",
    "CloseoutCriterionDisposition",
    "DispositionFinding",
    "build_closeout_criterion_disposition",
    "observe_closeout_criterion_disposition",
]

#: The DATA registry this oracle reads, relative to the repository root.
_REGISTRY_RELATIVE = "scripts/governor/checks/closeout_criterion_dispositions.yaml"

#: The finding class a consumer publishes for every unresolved class. It is also
#: the class this audit seeds itself with (T03), so the registry must declare it
#: or the audit self-triggers -- self-consistency with no special case.
CLOSEOUT_CRITERION_UNDISPOSED = "closeout_criterion_undisposed"

#: The closed set of dispositions. ``measured`` is a live claim (the class is
#: absent); ``downgraded`` is a standing, owned downgrade.
DISPOSITION_VOCABULARY = frozenset({"measured", "downgraded"})

#: Every record must carry these fields, each a non-empty string. Enforced by
#: the oracle independently of what the registry declares, so a registry that
#: quietly drops a field cannot weaken the contract.
REQUIRED_RECORD_FIELDS: tuple[str, ...] = (
    "disposition",
    "owner",
    "reason",
    "decision_ref",
    "rationale",
)

#: Fail-closed default when the registry cannot be trusted to declare it.
_DEFAULT_REQUIRED_FOR_SEVERITIES: tuple[str, ...] = ("high",)

def _require_file(root: Path, rel: str) -> Path:
    path = Path(root) / rel
    if not path.is_file():
        raise FileNotFoundError(
            f"{rel} is missing under {root}; refusing to report a vacuous "
            "all-disposed closeout criterion"
        )
    return path

def _load_registry(root: Path) -> dict:
    """Load the registry, raising when it cannot be trusted.

    Raises ``FileNotFoundError`` when the file is absent and ``ValueError`` when
    the YAML is not a mapping, when ``disposition_vocabulary`` is not a list, or
    when the ``records`` block is not a mapping. The registry is a REQUIRED
    input: a consumer must never read its absence as "everything is disposed".
    """
    path = _require_file(root, _REGISTRY_RELATIVE)
    try:
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as exc:
        raise ValueError(f"{_REGISTRY_RELATIVE} is not valid YAML: {exc}") from exc
    if not isinstance(data, dict):
        raise ValueError(
            f"{_REGISTRY_RELATIVE} must parse to a mapping, got "
            f"{type(data).__name__}"
        )
    if not isinstance(data.get("disposition_vocabulary"), list):
        raise ValueError(
            f"{_REGISTRY_RELATIVE} disposition_vocabulary must be a list"
        )
    if not isinstance(data.get("records"), dict):
        raise ValueError(f"{_REGISTRY_RELATIVE} records must be a mapping")
    return data

def _required_for_severities(document: dict) -> frozenset[str]:
    """The severities held to the criterion, fail-closed to ``{"high"}``."""
    declared = document.get("required_for_severities")
    if isinstance(declared, list):
        severities = frozenset(
            severity for severity in declared if isinstance(severity, str)
        )
        if severities:
            return severities
    return frozenset(_DEFAULT_REQUIRED_FOR_SEVERITIES)

def _reason_vocabulary(document: dict) -> frozenset[str]:
    """The registry's closed reason vocabulary; empty means every reason fails."""
    declared = document.get("reason_vocabulary")
    if not isinstance(declared, list):
        return frozenset()
    return frozenset(reason for reason in declared if isinstance(reason, str))

def _flag_class(flag_id: object) -> str:
    """The flag CLASS: the id before any parameterized ``@`` suffix."""
    return str(flag_id).split("@", 1)[0]

def _flag_id(flag: object) -> str:
    """A live flag's id, from a ``(id, severity)`` pair or an object's ``.flag``."""
    if isinstance(flag, (tuple, list)):
        return str(flag[0])
    return str(getattr(flag, "flag"))

def _flag_severity(flag: object) -> object:
    """A live flag's severity, from a ``(id, severity)`` pair or ``.severity``."""
    if isinstance(flag, (tuple, list)):
        return flag[1] if len(flag) > 1 else None
    return getattr(flag, "severity", None)

def _malformed_causes(
    record: object, reason_vocabulary: frozenset[str]
) -> list[str]:
    """Every way ``record`` cannot be trusted, in a stable order.

    An empty list means the record is well formed: a mapping carrying all
    required fields non-empty, a disposition in the closed vocabulary, and a
    reason in the registry's ``reason_vocabulary``.
    """
    if not isinstance(record, dict):
        return [f"record is not a mapping, got {type(record).__name__}"]
    causes: list[str] = []
    for field in REQUIRED_RECORD_FIELDS:
        value = record.get(field)
        if not (isinstance(value, str) and value.strip()):
            causes.append(f"missing or empty `{field}`")
    disposition = record.get("disposition")
    if disposition not in DISPOSITION_VOCABULARY:
        causes.append(
            f"disposition {disposition!r} is outside "
            f"{sorted(DISPOSITION_VOCABULARY)}"
        )
    reason = record.get("reason")
    if reason not in reason_vocabulary:
        causes.append(
            f"reason {reason!r} is outside the registry reason vocabulary"
        )
    return causes

# ── the findings ─────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class DispositionFinding:
    """One unresolved flag class, named individually and by kind."""

    flag_class: str
    kind: str
    detail: str

    def to_dict(self) -> dict:
        return {
            "flag_class": self.flag_class,
            "kind": self.kind,
            "detail": self.detail,
        }

@dataclass(frozen=True)
class CloseoutCriterionDisposition:
    """The criterion's reconciliation: undisposed, contradictions, malformed.

    ``findings`` concatenates the three groups in a stable order (undisposed,
    then contradictions, then malformed) so a consumer serializes a
    deterministic list; ``is_silent`` is true exactly when the criterion is
    satisfied for every live high-severity class.
    """

    undisposed: tuple[DispositionFinding, ...]
    contradictions: tuple[DispositionFinding, ...]
    malformed: tuple[DispositionFinding, ...]

    @property
    def findings(self) -> tuple[DispositionFinding, ...]:
        return self.undisposed + self.contradictions + self.malformed

    @property
    def is_silent(self) -> bool:
        return not self.findings

    @property
    def count(self) -> int:
        return len(self.findings)

    def to_dict(self) -> dict:
        return {
            "undisposed": [f.to_dict() for f in self.undisposed],
            "contradictions": [f.to_dict() for f in self.contradictions],
            "malformed": [f.to_dict() for f in self.malformed],
            "undisposed_count": len(self.undisposed),
            "contradiction_count": len(self.contradictions),
            "malformed_count": len(self.malformed),
            "finding_count": self.count,
            "is_silent": self.is_silent,
        }

def build_closeout_criterion_disposition(
    root: Path | str, live_high_flags: Sequence[object]
) -> CloseoutCriterionDisposition:
    """Reconcile the disposition registry against the live high-severity flags.

    ``live_high_flags`` is a sequence of ``(flag_id, severity)`` pairs or
    objects exposing ``.flag``/``.severity`` (e.g. ``DriftFlag``). Each live flag
    whose severity is in the registry's ``required_for_severities`` is resolved
    by CLASS; the inputs are never mutated and the result is deterministic
    (classes sorted).

    Raises ``FileNotFoundError`` when the registry is missing and ``ValueError``
    when it is malformed -- never a vacuous all-disposed zero.
    """
    root = Path(root)
    document = _load_registry(root)
    records = document["records"]
    required = _required_for_severities(document)
    reason_vocabulary = _reason_vocabulary(document)

    malformed: list[DispositionFinding] = []
    for class_key in sorted(records, key=str):
        causes = _malformed_causes(records[class_key], reason_vocabulary)
        if causes:
            malformed.append(
                DispositionFinding(
                    flag_class=str(class_key),
                    kind="malformed",
                    detail=(
                        f"record {class_key!r} is malformed: " + "; ".join(causes)
                    ),
                )
            )

    live_classes = sorted(
        {
            _flag_class(_flag_id(flag))
            for flag in live_high_flags
            if _flag_severity(flag) in required
        }
    )

    undisposed: list[DispositionFinding] = []
    contradictions: list[DispositionFinding] = []
    for flag_class in live_classes:
        if flag_class not in records:
            undisposed.append(
                DispositionFinding(
                    flag_class=flag_class,
                    kind="undisposed",
                    detail=(
                        f"live high-severity flag class {flag_class!r} has no "
                        f"record in {_REGISTRY_RELATIVE}, so the closeout "
                        "criterion is not satisfied for it"
                    ),
                )
            )
            continue
        record = records[flag_class]
        if isinstance(record, dict) and record.get("disposition") == "measured":
            contradictions.append(
                DispositionFinding(
                    flag_class=flag_class,
                    kind="contradiction",
                    detail=(
                        f"live high-severity flag class {flag_class!r} carries a "
                        "`measured` record, but the class is live: a stale "
                        "clear, not a pass"
                    ),
                )
            )

    return CloseoutCriterionDisposition(
        undisposed=tuple(undisposed),
        contradictions=tuple(contradictions),
        malformed=tuple(malformed),
    )

def observe_closeout_criterion_disposition(
    root: Path | str | None = None,
    live_high_flags: Sequence[object] = (),
) -> CloseoutCriterionDisposition:
    """Observe the criterion on the real tree (or a given root)."""
    if root is None:
        root = Path(__file__).resolve().parent.parent.parent.parent
    return build_closeout_criterion_disposition(Path(root), live_high_flags)
