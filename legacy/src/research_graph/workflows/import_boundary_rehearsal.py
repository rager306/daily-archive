# Formerly: src/arxiv_archive/import_boundary_rehearsal.py

"""Compatibility shim for negative import-boundary rehearsal helpers.

Implementation ownership moved to :mod:`research_graph.infrastructure.staging.import_boundary`.
Keep this module so existing public imports continue to work.
"""

from __future__ import annotations

from research_graph.infrastructure.staging.import_boundary import (
    FORBIDDEN_EMBEDDING_FIELDS,
    FORBIDDEN_OPTIMIZER_FIELDS,
    FORBIDDEN_RAW_FIELDS,
    FORBIDDEN_SECRET_FIELDS,
    FORBIDDEN_VECTOR_FIELDS,
    SCHEMA_VERSION,
    TRUSTED_IMPORT_USE,
    ImportBoundaryRehearsal,
    ImportCandidate,
    RehearsalDiagnostic,
    RehearsalValidationResult,
    build_import_boundary_rehearsal_from_benchmark,
    build_import_boundary_rehearsal,
    validate_import_boundary_rehearsal,
    write_import_boundary_rehearsal_run,
)

__all__ = [
    "SCHEMA_VERSION",
    "TRUSTED_IMPORT_USE",
    "FORBIDDEN_RAW_FIELDS",
    "FORBIDDEN_EMBEDDING_FIELDS",
    "FORBIDDEN_VECTOR_FIELDS",
    "FORBIDDEN_SECRET_FIELDS",
    "FORBIDDEN_OPTIMIZER_FIELDS",
    "RehearsalDiagnostic",
    "RehearsalValidationResult",
    "ImportCandidate",
    "ImportBoundaryRehearsal",
    "build_import_boundary_rehearsal_from_benchmark",
    "build_import_boundary_rehearsal",
    "write_import_boundary_rehearsal_run",
    "validate_import_boundary_rehearsal",
]
