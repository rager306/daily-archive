"""Application use cases for controlled pilot writes (M205 S03–S09).

Orchestrates authorized GraphDBPort pilot writes, receipts, read-back,
idempotent replay, rollback classification, batch gating, and export/restore
verdicts. Does not import infrastructure adapters directly in production code
paths beyond typing — composition injects GraphDBPort-like adapters.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from typing import Any, Literal, Protocol

from research_graph.application.graph.pilot_write_authorization import PilotWriteAuthorization
from research_graph.domain.navigation import PageIndexDocument
from research_graph.domain.schema import ExtractionPatch
from research_graph.domain.semantic_chunks import EvidencePath, SemanticChunk
from research_graph.domain.universal_kb.contracts import SafetyFlags

ReceiptStatus = Literal["success", "failed", "replay_noop", "rolled_back"]
BatchVerdict = Literal["proceed", "repair", "stop"]
ActivationVerdict = Literal["proceed", "repair", "stop"]


class PilotGraphWriter(Protocol):
    """Minimal write surface used by the use case (GraphDBPort subset + read_back)."""

    def init_schema(self) -> None: ...

    def upsert_scientific_kg(
        self,
        document: PageIndexDocument,
        chunks: list[SemanticChunk],
        evidence_paths: list[EvidencePath],
        patch: ExtractionPatch,
    ) -> None: ...

    def read_back_paper(self, paper_id: str) -> dict[str, Any]: ...


@dataclass(frozen=True, slots=True)
class PilotWriteReceipt:
    """Separate pilot write receipt — not production import eligibility."""

    receipt_id: str
    auth_id: str
    candidate_id: str
    paper_id: str
    packet_hash: str
    status: ReceiptStatus
    classification: str
    node_count: int = 0
    edge_count: int = 0
    evidence_path_ids: tuple[str, ...] = ()
    error: str | None = None
    production_activation: bool = False
    production_safety_flags: SafetyFlags = field(default_factory=SafetyFlags)
    diagnostics: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        self.production_safety_flags.assert_no_write()
        if self.production_activation:
            raise ValueError("receipt cannot enable production activation")

    def to_dict(self) -> dict[str, Any]:
        return {
            "receipt_id": self.receipt_id,
            "auth_id": self.auth_id,
            "candidate_id": self.candidate_id,
            "paper_id": self.paper_id,
            "packet_hash": self.packet_hash,
            "status": self.status,
            "classification": self.classification,
            "node_count": self.node_count,
            "edge_count": self.edge_count,
            "evidence_path_ids": list(self.evidence_path_ids),
            "error": self.error,
            "production_activation": self.production_activation,
            "production_safety_flags": self.production_safety_flags.to_dict(),
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class HumanBatchApproval:
    """Fresh human gate before multi-paper batch (S07)."""

    approval_token: str
    paper_ids: tuple[str, ...]
    environment: str
    rollback_plan: tuple[str, ...]
    max_papers: int = 5
    approved: bool = False

    def __post_init__(self) -> None:
        if len(self.paper_ids) > self.max_papers:
            raise ValueError(f"batch exceeds max_papers={self.max_papers}")
        if self.approved and not self.approval_token.strip():
            raise ValueError("approved batch requires approval_token")


@dataclass(frozen=True, slots=True)
class PilotBatchReport:
    receipts: tuple[PilotWriteReceipt, ...]
    paper_count: int
    verdict: BatchVerdict
    production_activation: bool = False
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "receipts": [r.to_dict() for r in self.receipts],
            "paper_count": self.paper_count,
            "verdict": self.verdict,
            "production_activation": self.production_activation,
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True, slots=True)
class ExportRestoreVerdict:
    verdict: ActivationVerdict
    export_hash: str
    restore_ok: bool
    production_activation: bool = False
    reasons: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return {
            "verdict": self.verdict,
            "export_hash": self.export_hash,
            "restore_ok": self.restore_ok,
            "production_activation": self.production_activation,
            "reasons": list(self.reasons),
        }


def _receipt_id(auth_id: str, paper_id: str, status: str) -> str:
    raw = f"{auth_id}|{paper_id}|{status}"
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:16]


def execute_authorized_pilot_write(
    writer: PilotGraphWriter,
    authorization: PilotWriteAuthorization,
    *,
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
    evidence_paths: list[EvidencePath],
    patch: ExtractionPatch,
    init_schema: bool = True,
) -> PilotWriteReceipt:
    """Execute one authorized pilot upsert and return PilotWriteReceipt."""
    authorization.assert_production_flags_closed()
    paper_id = str(document.paper_id)
    if not authorization.authorized or authorization.status != "authorized":
        return PilotWriteReceipt(
            receipt_id=_receipt_id(authorization.auth_id, paper_id, "failed"),
            auth_id=authorization.auth_id,
            candidate_id=authorization.candidate_id,
            paper_id=paper_id,
            packet_hash=authorization.packet_hash,
            status="failed",
            classification="authorization_denied",
            error="not_authorized",
            diagnostics=("write_blocked_no_auth",),
        )
    try:
        if init_schema:
            writer.init_schema()
        writer.upsert_scientific_kg(document, chunks, evidence_paths, patch)
        read_back = writer.read_back_paper(paper_id)
        if not read_back.get("found"):
            return PilotWriteReceipt(
                receipt_id=_receipt_id(authorization.auth_id, paper_id, "failed"),
                auth_id=authorization.auth_id,
                candidate_id=authorization.candidate_id,
                paper_id=paper_id,
                packet_hash=authorization.packet_hash,
                status="failed",
                classification="read_back_missing",
                error="read_back_not_found",
                diagnostics=("write_committed_but_read_back_failed",),
            )
        evidence_ids = tuple(str(x) for x in (read_back.get("evidence_path_ids") or []))
        return PilotWriteReceipt(
            receipt_id=_receipt_id(authorization.auth_id, paper_id, "success"),
            auth_id=authorization.auth_id,
            candidate_id=authorization.candidate_id,
            paper_id=paper_id,
            packet_hash=authorization.packet_hash,
            status="success",
            classification="pilot_write_ok",
            node_count=int(read_back.get("node_count") or 0),
            edge_count=int(read_back.get("edge_count") or 0),
            evidence_path_ids=evidence_ids,
            diagnostics=("pilot_write_success", "import_eligible_false"),
        )
    except Exception as exc:  # noqa: BLE001 - receipt captures failure
        return PilotWriteReceipt(
            receipt_id=_receipt_id(authorization.auth_id, paper_id, "failed"),
            auth_id=authorization.auth_id,
            candidate_id=authorization.candidate_id,
            paper_id=paper_id,
            packet_hash=authorization.packet_hash,
            status="failed",
            classification="write_exception",
            error=f"{type(exc).__name__}:{exc}",
            diagnostics=("pilot_write_failed", "rolled_back_or_aborted"),
        )


def verify_read_back(
    writer: PilotGraphWriter,
    *,
    paper_id: str,
    expected_packet_hash: str,
    expected_evidence_ids: Sequence[str] = (),
) -> dict[str, Any]:
    """S04: verify written graph resolves EvidencePaths and packet hash."""
    data = writer.read_back_paper(paper_id)
    ok = bool(data.get("found")) and data.get("packet_hash") == expected_packet_hash
    if expected_evidence_ids:
        got = set(data.get("evidence_path_ids") or [])
        ok = ok and set(expected_evidence_ids).issubset(got)
    ok = ok and data.get("import_eligible") is False
    return {
        "ok": ok,
        "paper_id": paper_id,
        "packet_hash_match": data.get("packet_hash") == expected_packet_hash,
        "import_eligible": data.get("import_eligible", False),
        "evidence_path_ids": list(data.get("evidence_path_ids") or []),
        "found": data.get("found", False),
    }


def replay_authorized_pilot_write(
    writer: PilotGraphWriter,
    authorization: PilotWriteAuthorization,
    *,
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
    evidence_paths: list[EvidencePath],
    patch: ExtractionPatch,
    prior_receipt: PilotWriteReceipt,
) -> PilotWriteReceipt:
    """S05: idempotent replay — no duplicates, stable classification."""
    paper_id = str(document.paper_id)
    before = writer.read_back_paper(paper_id)
    before_nodes = int(before.get("node_count") or 0)
    receipt = execute_authorized_pilot_write(
        writer,
        authorization,
        document=document,
        chunks=chunks,
        evidence_paths=evidence_paths,
        patch=patch,
        init_schema=False,
    )
    after = writer.read_back_paper(paper_id)
    after_nodes = int(after.get("node_count") or 0)
    if receipt.status == "success" and after_nodes == before_nodes and before.get("found"):
        return PilotWriteReceipt(
            receipt_id=_receipt_id(authorization.auth_id, paper_id, "replay_noop"),
            auth_id=authorization.auth_id,
            candidate_id=authorization.candidate_id,
            paper_id=paper_id,
            packet_hash=authorization.packet_hash,
            status="replay_noop",
            classification="idempotent_replay",
            node_count=after_nodes,
            edge_count=int(after.get("edge_count") or 0),
            evidence_path_ids=tuple(str(x) for x in (after.get("evidence_path_ids") or [])),
            diagnostics=("replay_no_duplicates", f"prior:{prior_receipt.receipt_id}"),
        )
    return receipt


def execute_with_injected_failure(
    writer: PilotGraphWriter,
    authorization: PilotWriteAuthorization,
    *,
    document: PageIndexDocument,
    chunks: list[SemanticChunk],
    evidence_paths: list[EvidencePath],
    patch: ExtractionPatch,
    fail_after_begin: Callable[[], None],
) -> PilotWriteReceipt:
    """S06: mid-write failure path via injected callback on writer upsert."""
    paper_id = str(document.paper_id)
    authorization.assert_production_flags_closed()

    # Monkey-patch style: wrap upsert to fail
    original = writer.upsert_scientific_kg

    def _failing_upsert(*args: Any, **kwargs: Any) -> None:
        fail_after_begin()
        raise RuntimeError("injected_mid_write_failure")

    # type: ignore[method-assign]
    writer.upsert_scientific_kg = _failing_upsert  # type: ignore[method-assign]
    try:
        receipt = execute_authorized_pilot_write(
            writer,
            authorization,
            document=document,
            chunks=chunks,
            evidence_paths=evidence_paths,
            patch=patch,
            init_schema=True,
        )
    finally:
        writer.upsert_scientific_kg = original  # type: ignore[method-assign]

    if receipt.status == "failed":
        return PilotWriteReceipt(
            receipt_id=_receipt_id(authorization.auth_id, paper_id, "rolled_back"),
            auth_id=authorization.auth_id,
            candidate_id=authorization.candidate_id,
            paper_id=paper_id,
            packet_hash=authorization.packet_hash,
            status="rolled_back",
            classification="mid_write_failure_no_partial_trusted",
            error=receipt.error,
            diagnostics=("injected_failure", "no_partial_trusted_graph"),
        )
    return receipt


def require_fresh_human_batch_approval(
    approval: HumanBatchApproval,
    *,
    expected_environment: str,
) -> HumanBatchApproval:
    """S07: gate batch until fresh human approval matches environment."""
    if not approval.approved:
        raise PermissionError("batch requires fresh human approval")
    if approval.environment != expected_environment:
        raise PermissionError("approval environment mismatch")
    if not approval.approval_token.strip():
        raise PermissionError("approval token required")
    if len(approval.paper_ids) == 0:
        raise ValueError("empty batch")
    if len(approval.paper_ids) > approval.max_papers:
        raise ValueError("batch too large")
    return approval


def run_controlled_pilot_batch(
    writer: PilotGraphWriter,
    authorization_factory: Callable[[str], PilotWriteAuthorization],
    papers: Sequence[
        tuple[PageIndexDocument, list[SemanticChunk], list[EvidencePath], ExtractionPatch]
    ],
    approval: HumanBatchApproval,
    *,
    expected_environment: str,
) -> PilotBatchReport:
    """S08: max five independently authorized papers under human gate."""
    require_fresh_human_batch_approval(approval, expected_environment=expected_environment)
    if len(papers) > approval.max_papers:
        raise ValueError("paper count exceeds approval max")
    receipts: list[PilotWriteReceipt] = []
    for document, chunks, evidence_paths, patch in papers:
        paper_id = str(document.paper_id)
        if paper_id not in approval.paper_ids:
            receipts.append(
                PilotWriteReceipt(
                    receipt_id=_receipt_id("none", paper_id, "failed"),
                    auth_id="none",
                    candidate_id="none",
                    paper_id=paper_id,
                    packet_hash="",
                    status="failed",
                    classification="paper_not_in_approval_scope",
                    error="out_of_scope",
                    diagnostics=("batch_scope_violation",),
                )
            )
            continue
        auth = authorization_factory(paper_id)
        receipts.append(
            execute_authorized_pilot_write(
                writer,
                auth,
                document=document,
                chunks=chunks,
                evidence_paths=evidence_paths,
                patch=patch,
                init_schema=True,
            )
        )
    successes = sum(1 for r in receipts if r.status in {"success", "replay_noop"})
    if successes == len(receipts) and receipts:
        verdict: BatchVerdict = "proceed"
    elif successes == 0:
        verdict = "stop"
    else:
        verdict = "repair"
    return PilotBatchReport(
        receipts=tuple(receipts),
        paper_count=len(receipts),
        verdict=verdict,
        production_activation=False,
        diagnostics=(f"successes:{successes}", "production_activation:false"),
    )


def export_restore_activation_verdict(
    *,
    export_snapshot: dict[str, Any],
    restore_snapshot_fn: Callable[[dict[str, Any]], None],
    read_export_fn: Callable[[], dict[str, Any]],
    batch_verdict: BatchVerdict,
) -> ExportRestoreVerdict:
    """S09: export/restore to fresh isolation; never production activation."""
    payload = json.dumps(export_snapshot, sort_keys=True, default=str)
    export_hash = hashlib.sha256(payload.encode("utf-8")).hexdigest()
    try:
        restore_snapshot_fn(export_snapshot)
        restored = read_export_fn()
        restore_ok = (
            len(restored.get("nodes", [])) == len(export_snapshot.get("nodes", []))
            and len(restored.get("edges", [])) == len(export_snapshot.get("edges", []))
        )
    except Exception:
        restore_ok = False
    if batch_verdict == "proceed" and restore_ok:
        verdict: ActivationVerdict = "proceed"
        reasons = ("export_restore_ok", "production_activation_false")
    elif restore_ok:
        verdict = "repair"
        reasons = (f"batch:{batch_verdict}", "restore_ok")
    else:
        verdict = "stop"
        reasons = ("restore_failed", f"batch:{batch_verdict}")
    return ExportRestoreVerdict(
        verdict=verdict,
        export_hash=export_hash,
        restore_ok=restore_ok,
        production_activation=False,
        reasons=reasons,
    )


__all__ = [
    "ActivationVerdict",
    "BatchVerdict",
    "ExportRestoreVerdict",
    "HumanBatchApproval",
    "PilotBatchReport",
    "PilotGraphWriter",
    "PilotWriteReceipt",
    "ReceiptStatus",
    "execute_authorized_pilot_write",
    "execute_with_injected_failure",
    "export_restore_activation_verdict",
    "replay_authorized_pilot_write",
    "require_fresh_human_batch_approval",
    "run_controlled_pilot_batch",
    "verify_read_back",
]
