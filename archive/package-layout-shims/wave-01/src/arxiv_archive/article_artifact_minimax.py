"""Compatibility shim for article artifact MiniMax boundary.

The canonical import path is now ``arxiv_archive.artifacts.minimax_boundary``.
This module remains to preserve imports created before M085.
"""

from arxiv_archive.artifacts.minimax_boundary import (
    MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION,
    MINIMAX_ARTIFACT_HELPER_TOOL_NAME,
    MINIMAX_ARTIFACT_HELPER_DETECTOR,
    REQUEST_MODE,
    DEFAULT_ARTICLE_ARTIFACT_BINDING,
    MiniMaxArtifactHelperRequest,
    MiniMaxArtifactHelperResult,
    ArticleArtifactWorkRequest,
    request_article_artifact_classification,
    build_article_artifact_minimax_request,
    validate_article_artifact_minimax_response,
    article_artifact_minimax_hint_schema,
)

__all__ = [
    'MINIMAX_ARTIFACT_HELPER_SCHEMA_VERSION',
    'MINIMAX_ARTIFACT_HELPER_TOOL_NAME',
    'MINIMAX_ARTIFACT_HELPER_DETECTOR',
    'REQUEST_MODE',
    'DEFAULT_ARTICLE_ARTIFACT_BINDING',
    'MiniMaxArtifactHelperRequest',
    'MiniMaxArtifactHelperResult',
    'ArticleArtifactWorkRequest',
    'request_article_artifact_classification',
    'build_article_artifact_minimax_request',
    'validate_article_artifact_minimax_response',
    'article_artifact_minimax_hint_schema',
]
