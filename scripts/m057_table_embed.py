#!/usr/bin/env python3
# DEPRECATED: compatibility shim for historical M057 imports.
# Use arxiv_archive.embedder.Embedder for new fd embedding calls.
# fd v2 env vars (TEI_URL, FD_API_KEY, MODEL_ID, REDIS_HOST, REDIS_PORT)
# are resolved by scripts/legacy/m057_table_embed.py.

from legacy.m057_table_embed import *  # noqa: F403
