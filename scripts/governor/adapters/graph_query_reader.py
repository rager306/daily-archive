"""Graph semantic query reader — validates .sgsnap content quality.

Reads a gzip-compressed NDJSON Samyama snapshot (format `sgsnap`, version 2)
and computes semantic metrics STRUCTURALLY, not by text heuristics:

  Line 1  : header {format, version, node_count, edge_count, labels,
                    edge_types, created_at, ...}
  t=="n"  : {"id", "labels": [...], "props": {..., "vid": hex}}
  t=="e"  : {"src": <node id>, "tgt": <node id>, "type": "EDGE_TYPE"}

Metrics:
  - distinct node count per label (vid-keyed, never string/2 heuristics)
  - avg raw_text across Section nodes only
  - enrichment coverage (Author/Venue/Topic nodes per Paper) — only when the
    snapshot vocabulary actually carries those labels
  - entity dedup integrity (duplicate vids among Entity-labeled nodes)
  - revision binding coverage via HAS_REVISION edges resolved to Paper nodes

Failure mode: a snapshot that cannot be parsed structurally raises; the
caller surfaces ERROR rather than silently reporting zero.
"""

from __future__ import annotations

import gzip
import json
from pathlib import Path
from typing import Any

from governor.domain.trajectory import DriftFlag

# Labels whose presence gates the enrichment rule (era-aware).
_ENRICHMENT_LABELS = ("Author", "Venue", "Topic")
_SECTION_LABEL = "Section"
_ENTITY_LABEL = "Entity"
_PAPER_LABEL = "Paper"
_REVISION_EDGE = "HAS_REVISION"


class _SnapshotModel:
    """Structurally parsed sgsnap v2 content."""

    def __init__(self) -> None:
        self.header: dict[str, Any] = {}
        self.label_vids: dict[str, set[str]] = {}
        self.id_labels: dict[int, set[str]] = {}
        self.id_vids: dict[int, str] = {}
        self.section_raw_text_lens: list[int] = []
        self.entity_vid_seen: set[str] = set()
        self.entity_vid_dupes = 0
        self.revision_edge_papers: set[str] = set()
        self.node_lines = 0
        self.edge_lines = 0


def _parse_snapshot(snapshot: Path) -> _SnapshotModel:
    model = _SnapshotModel()
    setattr(model, "_path", snapshot)
    with gzip.open(snapshot, "rt", encoding="utf-8", errors="replace") as fh:
        header_line = fh.readline()
        if not header_line.strip():
            raise ValueError(f"empty sgsnap: {snapshot}")
        header = json.loads(header_line)
        if header.get("format") != "sgsnap":
            raise ValueError(
                f"unexpected format {header.get('format')!r} in {snapshot}"
            )
        model.header = header

        for line in fh:
            line = line.strip()
            if not line:
                continue
            rec = json.loads(line)
            rtype = rec.get("t")
            if rtype == "n":
                model.node_lines += 1
                nid = rec.get("id")
                labels = frozenset(rec.get("labels") or [])
                vid = str((rec.get("props") or {}).get("vid") or "")
                if nid is not None:
                    model.id_labels[nid] = set(labels)
                    if vid:
                        model.id_vids[nid] = vid
                if vid:
                    for label in labels:
                        model.label_vids.setdefault(label, set()).add(vid)
                if _SECTION_LABEL in labels:
                    # Section text prop: production ingest writes "text"
                    # (ingest.rs:812); "raw_text" is the legacy name kept
                    # for older snapshots.
                    raw = (rec.get("props") or {}).get("text")
                    if not isinstance(raw, str):
                        raw = (rec.get("props") or {}).get("raw_text")
                    if isinstance(raw, str):
                        model.section_raw_text_lens.append(len(raw))
                if _ENTITY_LABEL in labels and vid:
                    if vid in model.entity_vid_seen:
                        model.entity_vid_dupes += 1
                    else:
                        model.entity_vid_seen.add(vid)
            elif rtype == "e":
                model.edge_lines += 1
    return model


def _bind_revision_papers(model: _SnapshotModel) -> None:
    """Resolve HAS_REVISION edges to Paper-side vids.

    Needs a second pass because edges may precede their endpoint nodes.
    """
    with gzip.open(snapshot_path_of(model), "rt", encoding="utf-8",
                   errors="replace") as fh:
        fh.readline()  # header
        edge_needed = _REVISION_EDGE not in (
            model.header.get("edge_types") or []
        )
        if edge_needed:
            return
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if rec.get("t") != "e" or rec.get("type") != _REVISION_EDGE:
                continue
            for endpoint in (rec.get("src"), rec.get("tgt")):
                labels = model.id_labels.get(endpoint, set())
                if _PAPER_LABEL in labels:
                    vid = model.id_vids.get(endpoint)
                    if vid:
                        model.revision_edge_papers.add(vid)


def snapshot_path_of(model: _SnapshotModel) -> Path:
    """Path recorded on the model at parse time."""
    path = getattr(model, "_path", None)
    if path is None:  # pragma: no cover - programming error
        raise RuntimeError("model has no source path")
    return path

class GraphQueryReader:
    """Extract semantic metrics from an .sgnap snapshot."""

    def __init__(self, root: Path):
        self.root = root

    def find_snapshots(self) -> tuple[list[Path], list[Path]]:
        """Find .sgsnap files split by priority root.

        Returns (store_snapshots, auxiliary_snapshots):
          - store: data/graphs/*.sgsnap — materialized graph stores
          - aux:   artifacts/**/*.sgsnap — per-session/load views
        """
        store: list[Path] = list(self.root.glob("data/graphs/*.sgsnap"))
        seen = {p.resolve() for p in store}
        aux: list[Path] = []
        for p in self.root.glob("artifacts/**/*.sgsnap"):
            if p.resolve() not in seen:
                aux.append(p)
                seen.add(p.resolve())
        return sorted(store), sorted(aux)

    def _created_at(self, snapshot: Path) -> tuple[str, int]:
        """(created_at from header, size) — newest-first selection key."""
        size = snapshot.stat().st_size
        try:
            with gzip.open(snapshot, "rt", encoding="utf-8",
                           errors="replace") as fh:
                header = json.loads(fh.readline())
                created = str(header.get("created_at") or "")
                return (created, size)
        except (OSError, ValueError, json.JSONDecodeError):
            return ("", size)

    def select_primary(self, snapshots: list[Path]) -> Path | None:
        """Newest materialization by header created_at; size tie-break."""
        if not snapshots:
            return None
        return max(snapshots, key=lambda p: self._created_at(p))

    def compute_metrics(self, snapshot: Path) -> dict[str, Any]:
        """Compute all semantic metrics for one snapshot."""
        model = _parse_snapshot(snapshot)
        vocabulary = set(model.header.get("labels") or [])
        _bind_revision_papers(model)

        def n(label: str) -> int:
            return len(model.label_vids.get(label, ()))

        sections_n = len(model.section_raw_text_lens)
        avg_section_text = (
            sum(model.section_raw_text_lens) / sections_n if sections_n else 0.0
        )
        enrichment_present = any(l in vocabulary for l in _ENRICHMENT_LABELS)
        revision_possible = _REVISION_EDGE in (
            model.header.get("edge_types") or []
        )

        metrics: dict[str, Any] = {
            "snapshot": str(snapshot.relative_to(self.root)),
            "created_at": model.header.get("created_at"),
            "total_nodes": model.header.get("node_count", model.node_lines),
            "total_edges": model.header.get("edge_count", model.edge_lines),
            "vocabulary": sorted(vocabulary),
            "enrichment_labels_in_vocab": enrichment_present,
            "revision_edges_declared": revision_possible,
            "section_count": n(_SECTION_LABEL),
            "paper_count": n(_PAPER_LABEL),
            "author_count": n("Author"),
            "venue_count": n("Venue"),
            "topic_count": n("Topic"),
            "entity_count": n(_ENTITY_LABEL),
            "avg_raw_text_len": avg_section_text,
            "section_raw_text_scanned": sections_n,
            "duplicate_entity_vids": model.entity_vid_dupes,
            "revision_bound_papers": len(model.revision_edge_papers),
        }
        return metrics


def check_graph_semantics(root: Path) -> list[DriftFlag]:
    """Evaluate graph semantic quality against DATA thresholds."""
    import yaml

    contract_path = root / "scripts/governor/checks/graph_semantics.yaml"
    if not contract_path.exists():
        return []

    contract = yaml.safe_load(contract_path.read_text()) or {}
    if not contract:
        return []

    dimension = str(contract.get("dimension", "graph_semantics"))
    severity = str(contract.get("severity", "high"))

    def _flag(code: str, evidence: str) -> DriftFlag:
        return DriftFlag(flag=code, severity=severity,
                         evidence=evidence, dimension=dimension)

    reader = GraphQueryReader(root)
    store_snapshots, aux_snapshots = reader.find_snapshots()

    if not store_snapshots and not aux_snapshots:
        return [_flag("graph_semantics_no_snapshots",
                      "No .sgsnap snapshots found in data/graphs/ or artifacts/")]

    # Materialized stores win; per-session views only as fallback.
    candidates = store_snapshots or aux_snapshots
    primary = reader.select_primary(candidates)
    assert primary is not None
    metrics = reader.compute_metrics(primary)

    flags: list[DriftFlag] = []

    for rule in contract.get("rules", []):
        rule_id = rule.get("id", "unknown")
        threshold = rule.get("threshold", 0)
        description = rule.get("description", "")

        if rule_id == "section_text_completeness":
            scanned = metrics["section_raw_text_scanned"]
            avg = metrics["avg_raw_text_len"]
            if scanned == 0:
                # No Section text anywhere in this snapshot: report as
                # incomplete only when Sections exist but carry no text at all.
                if metrics["section_count"] > 0:
                    flags.append(_flag(
                        "graph_section_text_missing",
                        f"{metrics['section_count']} Section nodes carry no "
                        f"raw_text prop. {description}",
                    ))
                continue
            if avg < threshold:
                flags.append(_flag(
                    "graph_section_text_incomplete",
                    f"avg raw_text over {scanned} Section nodes = {avg:.0f} "
                    f"chars (threshold: {threshold}). "
                    f"Sections are stubs, not full text. {description}",
                ))

        elif rule_id == "enrichment_coverage":
            if not metrics["enrichment_labels_in_vocab"]:
                # Snapshot ontology predates enrichment nodes — the rule is
                # N/A rather than violated. Do not fail closed on eras.
                continue
            papers = metrics["paper_count"]
            enriched = (metrics["author_count"] + metrics["venue_count"]
                        + metrics["topic_count"])
            coverage = enriched / max(papers * 3, 1)
            if papers > 0 and coverage < threshold:
                flags.append(_flag(
                    "graph_enrichment_missing",
                    f"enrichment coverage = {coverage:.1%} "
                    f"(threshold: {threshold:.0%}). "
                    f"Authors={metrics['author_count']}, "
                    f"Venues={metrics['venue_count']}, "
                    f"Topics={metrics['topic_count']} for {papers} papers. "
                    f"{description}",
                ))

        elif rule_id == "entity_dedup_integrity":
            dupes = metrics["duplicate_entity_vids"]
            if dupes > threshold:
                flags.append(_flag(
                    "graph_entity_duplication",
                    f"{dupes} duplicate Entity vids (threshold: {threshold}). "
                    f"{description}",
                ))

        elif rule_id == "revision_binding_coverage":
            if not metrics["revision_edges_declared"]:
                # Edge type absent from this materialization's vocabulary:
                # nothing to measure, era-mismatch, not a violation.
                continue
            papers = metrics["paper_count"]
            bound = metrics["revision_bound_papers"]
            coverage = bound / max(papers, 1)
            if papers > 0 and coverage < threshold:
                flags.append(_flag(
                    "graph_revision_binding_low",
                    f"revision binding = {coverage:.1%} ({bound}/{papers}, "
                    f"threshold: {threshold:.0%}). {description}",
                ))

    return flags
