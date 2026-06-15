# M067 Distribution Model Assumption

## Decision scope

M067 assumes daily-archive is a self-hosted research project: a local scientific knowledge graph for a single-user workflow, operated by the project owner rather than offered as a hosted database service to third parties.

## Current daily-archive state

- Research KG for article ingestion, graph-vector retrieval, and local analysis.
- Self-hosted operation with no third-party hosted graph service exposure.
- Single-user workflow today.
- Production graph import is not authorized during this benchmark work.
- Real database connections are disabled by default in the benchmark harness.

## FalkorDB SSPLv1 interpretation for M067

FalkorDB is licensed under SSPLv1. Under the M067 distribution model, FalkorDB is viable because self-hosted use does not require source disclosure. The same is true for internal proprietary applications when FalkorDB itself is not exposed as a hosted service to third parties.

FalkorDB is OK for:

- Self-hosted daily-archive research use.
- Internal deployment and internal proprietary applications.
- Evaluation, prototyping, and internal testing.
- Closed-source proprietary application use when the application is not offering FalkorDB as a service to third parties.

Disclosure or commercial licensing is triggered when:

- FalkorDB is provided as SaaS.
- FalkorDB is exposed as a hosted database service to third parties.
- The deployment model makes FalkorDB functionality available as a managed third-party service.

In those cases, SSPLv1 Section 13 disclosure obligations apply unless daily-archive obtains a commercial FalkorDB license.

## Optional FalkorDB Cloud pricing context

FalkorDB Cloud is optional and cloud-only:

- Free: limited tier.
- Startup: $73 per 1GB-month.
- Pro: $350 per 8GB-month.
- Enterprise: tailored.

Self-hosting remains fully supported through Docker, Kubernetes, and standalone deployment modes. FalkorDB requires Redis 8.0+.

## Future-state uncertainty and mitigation

The future daily-archive distribution model is uncertain. If the project evolves into SaaS or a hosted service for third parties, the mitigation is to either contact FalkorDB for a commercial license or migrate to a permissive fallback such as Apache AGE.

## M067 consequence

Under the current self-hosted research assumption, FalkorDB's license score is **4/5** and its corrected total is **70/90**. FalkorDB is the preferred self-hosted candidate ahead of Apache AGE at **64/90** and LadybugDB at **62/90**, while Neo4j remains the highest total scorer at **76/90** but is not selected because AGPLv3 is viral for self-hosted distribution.
