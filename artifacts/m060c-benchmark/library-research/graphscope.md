# graphscope library research

## Architecture summary

The GraphScope source is vendored at `/root/vendor-source/graphscope`, but GitNexus repo `graphscope` is not available yet; `gitnexus context Graph --repo graphscope` and the matching query both returned repository-not-found. Local source inspection still shows analytical graph applications and distributed graph processing surfaces.

GraphScope is a distributed graph system, which is a mismatch for the current M060b intermediate layer unless we outgrow single-process analysis. Graph writes are not authorized; production import is not authorized; fact promotion is not authorized; external network default is disabled; LLM calls default is disabled.

## Algorithm support table

| Algorithm | Support | Evidence |
|---|---:|---|
| BFS | Yes | Local vendored hits include `flex/Performance.md` and graph analytical design docs. |
| PageRank | Yes | Local vendored hits include GraphScope data-scientist docs and performance docs. |
| shortest_path | Yes | Local vendored hits include `python/graphscope/analytical/app/sssp.py`. |
| community | Partial | Local vendored hits include community-adjacent docs; exact fit needs GitNexus indexing completion. |

## Our use case fit

Poor near-term fit for 10k-100k edge local benchmarks: distributed runtime and operational complexity are likely overkill. It becomes interesting only if later corpus scale or multi-tenant graph analytics exceeds single-process limits.

## Decision

**DEFER**. Rationale: wait for GitNexus indexing completion and only revisit if scale demands distributed graph processing.
