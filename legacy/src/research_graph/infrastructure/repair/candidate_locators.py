# Formerly: src/arxiv_archive/candidate_locators.py
"""Compatibility shim for candidate locator staging helpers.

Implementation ownership moved to :mod:`research_graph.infrastructure.staging.graph_candidates`.
This module re-exports the same public names so existing public imports
continue to work.
"""

from __future__ import annotations

from research_graph.infrastructure.staging.graph_candidates import (
    ALLOWED_CANDIDATE_TYPES,
    ALLOWED_COORDINATE_SPACES,
    ALLOWED_REVIEW_QUEUE_REASONS,
    ALLOWED_ROUTES,
    ALLOWED_STATES,
    ALLOWED_SUPPORT_LEVELS,
    ALLOWED_UNCERTAINTY_LABELS,
    ALLOWED_USES,
    CANDIDATE_LOCATOR_PROTOCOL_VERSION,
    DEFAULT_ROUTE_SPECS,
    EXCLUDED_USES,
    FORBIDDEN_PAYLOAD_KEYS,
    LocatorRouteSpec,
    LocatorSource,
    SourceReadResult,
    build_candidate_locator_artifact,
    build_candidate_locator_batch_from_targets,
    default_safety_flags,
    find_forbidden_payload_keys,
    validate_candidate_locator_artifact,
    write_candidate_locator_artifact,
)

__all__ = [
    "ALLOWED_CANDIDATE_TYPES",
    "ALLOWED_COORDINATE_SPACES",
    "ALLOWED_REVIEW_QUEUE_REASONS",
    "ALLOWED_ROUTES",
    "ALLOWED_STATES",
    "ALLOWED_SUPPORT_LEVELS",
    "ALLOWED_UNCERTAINTY_LABELS",
    "ALLOWED_USES",
    "CANDIDATE_LOCATOR_PROTOCOL_VERSION",
    "DEFAULT_ROUTE_SPECS",
    "EXCLUDED_USES",
    "FORBIDDEN_PAYLOAD_KEYS",
    "LocatorRouteSpec",
    "LocatorSource",
    "SourceReadResult",
    "build_candidate_locator_artifact",
    "build_candidate_locator_batch_from_targets",
    "default_safety_flags",
    "find_forbidden_payload_keys",
    "validate_candidate_locator_artifact",
    "write_candidate_locator_artifact",
]
