# Continue — M421 S01+S02 implemented (pending GSD seal)

## Last action

M421-ujpf0o S01+S02 implemented (code green; GSD skip-delivered closeout next):

- S01: `DA_REMOTE_GRAPH_ENDPOINT` canonical, `DA_GRAPH_ENDPOINT` explicit
  alias (conflict fails closed). `GraphModeError::{RemoteWritesUnsupported,
  RemoteEndpointMissing}` replace the lie that the RESP client is unshipped.
  `open_for_mode(Remote)` never returns a Samyama HOT store.
- S02: `da query` / `graph-stats` open `RemoteGraphStore` under
  `DA_GRAPH_MODE=remote`. `--snapshot` is embedded-only. Health copy matches
  the real read plane. Governor pins `remote_writes_unsupported`,
  `remote_endpoint_alias`, `remote_query_read_plane`.

## Next action (priority)

1. GSD skip-delivered closeout for M421 (UAT + sha256 validate + complete).
2. Then Wave 2-b pdf-inspector (ToUnicode/page-level missingness) **or**
   leave remote writes deferred (honest: RESP plane has no DirectGraphStore).

## Do not

Unlock D127. Commit `.env`. Fake a HOT DirectGraphStore over RESP.
Soak-wakeup loops. Feature-gated code must pass default-build clippy.
GSD BROWSER_REQUIREMENT_RE treats hyphen as a word boundary
(`no-fail-open` + `zero-page` → false browser-required); UAT
`commandOrTool` must contain `gsd_uat_exec` for adopted structuredOnly.
