---
source_key: vasiliev2026knowledge-graph-memory-graph
canonical_url: https://sergeyvasiliev.substack.com/p/knowledge-graph-memory-graph-and
captured_from: source/article.html
note: derived markdown sidecar for git; HTML is gitignored (data/**/*.html) and remains local with sha256 in identity.json
---

Knowledge Graph, Memory Graph and Context Graph: Three Roles in One Architecture

### What they have in common, where they differ and when each one is useful
 Sergey Vasiliev Aug 17, 2026 6 1 Share 
## Table of Contents 
 
- Introduction

- The Three Roles at a Glance

- Knowledge Graph: Governed Meaning

- Memory Graph: Retained Experience

- Context Graph: The Decision-Time View

- How the Three Roles Work Together

- One Graph Database or Several Data Stores?

- When Agent Memory Becomes an Enterprise Capability

- Summary

- References

 
# 1. Introduction 
 Enterprise AI architecture is acquiring graphs at an impressive rate.

 We already had knowledge graphs. We now have memory graphs, context graphs, reasoning graphs and decision graphs. Put them into one architecture diagram and it can look as though every AI agent needs several new graph databases before it can answer a useful question.

 From a Labelled Property Graph, LPG, practitioner’s point of view, this is the wrong place to start.

 Knowledge graph, memory graph and context graph should first be understood as three architectural roles:

> A knowledge graph represents governed enterprise knowledge. 

 A memory graph represents retained experience around that knowledge. 

 A context graph represents the information selected for one task, actor, purpose and point in time. 

 The three roles share connected entities, relationships, time and provenance. They differ in authority, write path, lifecycle, scope and persistence.

 Agent memory is also broader than a memory graph. It may include session state, source events, documents, vector indexes, workflow checkpoints, procedures and executable skills. A memory graph is the connected representation used when identity, relationships, time and provenance materially affect retrieval or action.

 This article develops several ideas from my previous publications. A Practical Architecture for LLM Memory examined the role of an LPG in long-term agent memory [1]. Enterprise Knowledge Graphs vs Applied Knowledge Graphs separated reusable enterprise meaning from application-level decision support [2]. The Ontology Trap argued that generated structure should remain a proposal until it has passed validation and governance [3]. 

 The purpose here is narrower. It is to explain what the three graph roles have in common, where they differ and how to use them without turning three useful terms into three unnecessary platforms.

# 2. The Three Roles at a Glance 
 “Context graph” does not yet have one stable industry meaning. It has been used for context-enriched knowledge graphs, operational event graphs and graphs assembled for runtime decisions [9].

 In this article, a context graph is a task-specific runtime projection of knowledge, memory, live state and policy . It is a structured decision context, not another enterprise source of truth. 

 All three roles operate over connected entities and relationships. Each depends on some combination of identity, time, provenance and controlled meaning. Their main differences concern who writes the information, how much authority it carries, how long it remains and how widely it is reused.

 
- 
## What they have in common 
 The same canonical entity can appear in governed knowledge, retained experience and the current context without being recreated three times.

 For example, one LegalEntity node may be connected to: 

 accepted ownership relationships in the knowledge graph

- previous screening claims and analyst decisions in the memory graph

- the evidence selected for the current review in the context graph

 The shared foundation includes more than the LPG model itself. It also includes governance:

 
- canonical identity

- controlled relationship meaning

- provenance

- temporal interpretation

- access rules

- lifecycle management

 The LPG provides nodes, relationships and properties. It does not decide whether an assertion is governed, unreviewed, rejected or merely relevant to the current task. Those distinctions must be designed into the architecture.

# 3. Knowledge Graph: Governed Meaning 
 Within this architecture, the knowledge graph holds governed enterprise meaning.

 Governed meaning includes the identities, definitions, relationships and rules that the organisation has accepted for reuse under explicit ownership and lifecycle controls.

 That is an assigned architectural role, not a universal definition of every knowledge graph. Some knowledge graphs also contain extracted or disputed information. The approach used here keeps those less certain assertions in the memory graph until a defined governance process accepts them.

 The LPG fragments in this article are illustrative patterns rather than complete production schemas.

 A simplified beneficial ownership model might contain:

 (:LegalEntity)-[:REGISTERED_IN]->(:Jurisdiction)
(:LegalEntity)-[:HAS_BENEFICIAL_OWNER]->(:Person)
(:EntityType)-[:SUBJECT_TO]->(:ComplianceObligation)
(:ComplianceObligation)-[:DEFINED_BY]->(:RegulatorySource) This structure can support questions such as:

 
- Which canonical entity does this identifier represent?

- Which person is accepted as a beneficial owner?

- Which obligation applies to this type of entity?

- Which regulatory source defines that obligation?

- During which period was the ownership relationship valid?

 The value does not come from representing documents as nodes. It comes from connecting identity, relationships, evidence and applicable rules in a form that several workflows can reuse.

## The domain semantic contract 
 A knowledge graph needs a domain semantic contract . 

 That contract governs the wider graph model. It may define:

 
- canonical identifiers

- approved labels and relationship types

- valid source and target patterns

- required provenance

- temporal expectations

- ownership

- validation requirements

- permitted lifecycle transitions

 The Minimum Semantic Contract is narrower. It defines the smaller subset of labels, relationships, evidence conditions and temporal rules that a particular runtime is allowed to trust [4]. 

 The distinction is practical.

 The domain semantic contract may describe everything the graph can represent. The Minimum Semantic Contract describes what one agent or application may safely use.

 Database constraints can enforce part of the model. The ISO GQL standard defines a language for property graphs, while graph products provide implementation-specific constraints and privileges [10][11]. These features help to enforce structure, but they do not decide what the enterprise accepts as true or suitable for use.

## Domain rules, runtime policy and access control 
 Three different forms of control should remain separate.

 A domain rule describes subject-matter logic, such as the evidence needed to establish beneficial ownership. 

 A runtime policy controls how an agent may retrieve, combine or act on information. 

 An access control determines who may read or write particular information. 

 A fourth concern, purpose limitation , determines what authorised information may influence a particular task. 

 An analyst may be allowed to view a document but still be prohibited from using it as the sole basis for an automated ownership decision.

## An extracted claim is not governed knowledge 
 Suppose an extraction agent processes a filing and proposes that a person controls a legal entity.

 The document contains a similar name and a shared address. That may justify further investigation, but it does not yet justify this governed relationship:

 (:LegalEntity)-[:HAS_BENEFICIAL_OWNER]->(:Person) The initial result should remain a claim linked to its source and review:

 (:OwnershipClaim)-[:SUBJECT]->(:LegalEntity)
(:OwnershipClaim)-[:OBJECT]->(:Person)
(:OwnershipClaim)-[:WAS_DERIVED_FROM]->(:SourceSpan)
(:AnalystDecision)-[:REVIEWS]->(:OwnershipClaim) An observation records what a source presented. A claim is a proposition extracted or inferred from that observation. Evidence supports or challenges the claim. Only after validation and governance should it become an accepted enterprise assertion.

 The accepted claim may remain as the evidence-bearing record. A derived edge can then provide a simpler path for operational traversal.

 This avoids a common mistake. An agent-generated relationship and a governed relationship may look identical in an LPG unless their different authority and lifecycle are made explicit.

 The operating principle remains:

> AI proposes. Systems validate. Analysts decide. The graph records the decision. 

## Time belongs to the assertion 
 A relationship may be governed and still be valid only for a defined period.

 A legal entity can exist for decades, while one ownership relationship remains valid for six months. Dates therefore belong to the relationship or claim they describe, not only to the entity.

 At minimum, the model should distinguish:

 
- valid time , when the assertion was true or applicable in the domain 

- transaction time , when that version was held by the system 

 This allows the graph to answer both current and historical questions without mixing incompatible states [6].

# 4. Memory Graph: Retained Experience 
 A memory graph records experience around governed entities and relationships.

 It may connect:

 (:Episode)
(:Observation)
(:Claim)
(:Retrieval)
(:ToolCall)
(:AnalystDecision)
(:Action)
(:Outcome)
(:Feedback) In the beneficial ownership example, the graph might contain:

 (:ReviewCase)-[:CONCERNS]->(:LegalEntity)
(:Retrieval)-[:RETURNED]->(:SourceVersion)
(:OwnershipClaim)-[:WAS_DERIVED_FROM]->(:SourceSpan)
(:AnalystDecision)-[:REVIEWS]->(:OwnershipClaim)
(:Outcome)-[:FOLLOWS]->(:AnalystDecision) The knowledge graph can show the accepted ownership structure and the obligation that applies.

 The memory graph can show which source version was retrieved, what claim the agent produced, why the analyst accepted or rejected it, and what happened afterwards.

 That additional history helps the system avoid repeating failed or misleading patterns.

## Agent memory is broader than the graph 
 A memory graph is one component of agent memory.

 The broader capability may also use:

 
- session state

- the source event record

- lexical and vector indexes

- documents

- workflow checkpoints

- controlled procedures

- executable skills

 Graph structure becomes useful when relationships between events, entities, claims, decisions and outcomes affect later retrieval or action.

 A preference such as “show the result as a table” may need only a profile record. A history of disputed ownership claims, source versions and analyst decisions has a much stronger graph shape.

## A technical log is not automatically memory 
 A log may show that a search tool returned ten documents.

 Useful memory connects that event to its purpose and outcome:

 
- The graph adds value when these connections improve later retrieval, reconstruction or explanation.

 Recent research treats agent memory as a lifecycle that includes writing, storing, retrieving, updating and forgetting [13]. Systems such as APEX-MEM and Zep use temporal graph structures to connect episodes, claims and changing information [14][15].

 These systems provide useful patterns, but they do not show that every memory problem needs a graph database.

## The source event record 
 The original interaction history should normally remain available in a source event record . 

 This record preserves:

 what the agent received

- what a tool returned

- which action was requested

- what the analyst accepted or rejected

- what outcome was observed

 It is authoritative for the interaction itself, not automatically for the underlying domain assertion.

 A record showing that an ownership claim was retrieved proves that the claim was retrieved. It does not prove that the person was a beneficial owner.

 Keeping the source event record independently of the derived memory graph allows the graph to be rebuilt when extraction logic, identity resolution or the semantic contract changes.

## A proposed minimum memory contract 
 There is no universal schema that fits every observation, claim, action and outcome.

 A practical proposal would give each durable memory:

 
- a stable identifier

- a controlled memory type

- authenticated origin

- provenance

- user, case, tenant or application scope

- relevant time information

- lifecycle and validation state

- source authority

- retention rules

- the contract version under which it was created

 Different memory types would then add their own fields.

 An inferred ownership claim may require confidence, source spans and an extractor version. A directly recorded tool call may not need confidence. An outcome may concern several entities rather than one subject.

 The proposal should remain extensible rather than forcing every memory into one flat record.

 Lifecycle, validation, source authority, permitted use and retention should also remain separate. A memory can be current but unreviewed, rejected but retained for audit, or historically valid but no longer applicable to the present case.

 Compressing all these meanings into one status property usually creates ambiguity. 

## Remembering rejection and failure 
 Useful memory includes more than successful actions.

 It should preserve that:

 
- a claim was rejected

- a source version was superseded

- an extraction pattern produced a false match

- a decision required analyst review

- an action was prohibited for the stated purpose

 I have described this category as Negative Semantic Knowledge [5]. 

 It is different from missing information. The system explicitly knows that a claim, source or action should not be used in a particular way.

 The retrieval layer must enforce that knowledge. Recording a rejection while continuing to return the rejected claim as current information preserves history but does not create reliable memory.

## Quality starts at the write boundary 
 Persistent memory creates a feedback loop. A weak claim stored today may influence many later decisions.

 Research has shown that agents often follow retrieved experience even when it is incorrect or only superficially similar [16]. Other work has demonstrated memory-poisoning and privacy risks [17][18].

 A controlled write path should therefore preserve source origin, authenticate the writer, distinguish observation from inference and quarantine unreviewed claims.

 Repeated retrieval does not turn an uncertain claim into trusted knowledge.

# 5. Context Graph: The Decision-Time View 
 A context graph is the structured decision context assembled for the current task.

 It combines selected parts of:

 
- the knowledge graph

- the memory graph

- live operational state

- runtime policy

- access and purpose restrictions

 Live operational state means information that may change during the workflow, such as the current case status, the documents already reviewed, the latest screening result or the actions already attempted.

 For a beneficial ownership review, the complete environment may contain many entities, filings, claims, previous decisions and regulatory obligations.

 The context graph should include only what is:

 
- relevant to the entity under review

- valid at the review date

- applicable to the jurisdiction and entity type

- available to the analyst

- permitted for the current purpose

 It may therefore contain the accepted ownership path, one unresolved claim, the source versions supporting each assertion, a previous rejection reason and the obligation currently in force.

## The context graph is not the prompt 
 The context graph is a structured decision context.

 It may be delivered to an agent as:

 
- selected graph paths

- structured records

- source excerpts

- JSON

- citations

- a concise evidence summary

 The prompt or tool response is the delivery format.

 GraphRAG creates value through better selection and reconstruction, not by placing a larger graph-shaped payload into the model context [7][8].

## Context is usually temporary 
 Most context graphs exist for one task or run.

 Their contents change when the entity, analyst, purpose, time or policy changes. Permanent RELEVANT_TO relationships are therefore often misleading. 

 A source relevant to one review date may be outdated for another. Information available to one analyst may be restricted for another. A previously useful claim may have been superseded.

 Where audit or replay is required, the system can retain a Context Snapshot . 

 A useful snapshot records:

 
- the exact source versions and graph assertions selected

- the relevant valid and transaction times

- the runtime policy and Minimum Semantic Contract versions

- the access and purpose decisions

- the retrieval and filtering parameters

- the actor and task

 A list of mutable node identifiers is not enough. Historical reconstruction requires versioned or otherwise stable content.

# 6. How the Three Roles Work Together 
 The three roles form a lifecycle rather than a simple hierarchy.

 Four inputs contribute to context assembly:

 
- Governed sources contribute accepted knowledge. Agent interactions and analyst decisions contribute experience. Live state shows what is happening in the current workflow. Runtime policy and access controls determine what may be retrieved and how it may be used.

 Context assembly brings these inputs together without treating them as equally authoritative.

 Consider an analyst reviewing the ownership of a legal entity.

 The knowledge graph contains an accepted owner and the relevant compliance obligation. The memory graph records that an earlier agent proposed another owner based only on a shared service-provider address. An analyst rejected that claim and recorded the reason.

 For the current task, the context graph includes the accepted ownership path, the rejected claim, its supporting source and the previous rejection reason. The agent can then avoid repeating the same weak inference.

 The decision creates new experience, which enters the source event record and may update the memory graph.

 Most experience remains memory. Some repeated and validated experience may justify a change to governed knowledge.

 Suppose several rejected claims show that the same extraction pattern repeatedly treats a service-provider address as evidence of control. After review, the organisation may add a governed rule stating that the address alone is insufficient.

 The memory graph reveals the pattern. The knowledge graph records the accepted rule.

# 7. One Graph Database or Several Data Stores? 
 Knowledge graph, memory graph and context graph are logical roles. They are not procurement instructions.

 A knowledge graph and memory graph can share one graph database when they have:

 common canonical identifiers

- compatible access controls

- manageable write volumes

- similar availability requirements

- compatible retention rules

- clear ownership

 One database might contain:

 Knowledge:
LegalEntity, Person, Jurisdiction, ComplianceObligation

Memory:
Episode, OwnershipClaim, AnalystDecision, Outcome

Audit:
Run, ContextSnapshot Labels, relationship types and service APIs can preserve modelling boundaries. They do not create security boundaries on their own.

 A label such as :RestrictedClaim does not prevent a database user from reading the node. Security requires explicit authentication, authorisation and graph privileges [12]. 

 Separate data stores become reasonable when operational requirements differ. Memory may have a much higher write volume than governed knowledge. It may require shorter retention, stronger tenant isolation or more frequent deletion. Generated claims may also need to remain quarantined from accepted assertions.

 Physical separation does not remove the need for shared identity. Each store should use the same durable business identifiers even when it maintains its own local nodes.

 A context graph rarely needs its own database. It is normally assembled for a task and discarded afterwards. When persistence is required, a Context Snapshot can be retained without creating a new enterprise context master.

 Some agent memory does not need a graph at all.

 A preferred output format may belong in a profile record. A workflow stage may belong in a transactional table. A controlled procedure may remain a versioned document. Conversation history may be stored in an event stream. Lexical or vector retrieval may be sufficient when relationships do not materially change the result.

 The presence of an AI agent does not make every persistence problem graph-shaped.

# 8. When Agent Memory Becomes an Enterprise Capability 
 Agent memory is becoming an important enterprise capability.

 Long-running agents need to preserve observations, recognise corrections, reuse relevant experience and maintain continuity across sessions. They also need controlled ways to write, update and forget memory.

 This does not always require an enterprise memory service.

 Three decisions should be made independently.

 
- A shared enterprise memory service becomes justified when several agents need the same experience, identity resolution must remain consistent, retention and deletion require common governance, and decisions need cross-application audit.

 For one assistant remembering a small set of display preferences, this would be excessive.

 A sensible progression is:

 session state

- simple durable memory

- application-scoped memory graph

- shared enterprise memory service

 The architecture should move to the next stage only when the earlier one can no longer meet the requirement.

## The graph break-even point 
 A graph becomes more credible when several conditions are present:

 
- Correct retrieval or reconstruction requires multi-hop relationships.

- Identities vary across systems or sessions.

- Relevant relationships change over time.

- Provenance affects whether information can be trusted.

- Previous actions and outcomes affect the next decision.

- Several applications reuse the connected experience.

 This is a design test rather than a benchmark.

 The economic question remains:

> Does the improvement justify identity resolution, graph modelling, extraction, governance, security, reprocessing and operational cost? 

# 9. Summary 
 Knowledge graph, memory graph and context graph use similar connected-data foundations, but they serve different roles.

 The knowledge graph represents governed enterprise knowledge. It connects canonical identities, accepted assertions, relationships and domain rules. 

 The memory graph represents retained experience. It connects observations, claims, actions, decisions, outcomes and feedback. It is one part of the wider agent-memory capability. 

 The context graph represents the structured decision context selected for one task, actor, purpose and point in time. 

 They can share canonical identifiers and graph technology. They may even share one database. Their authority, lifecycle and scope should remain distinct.

 Five practical rules follow:

 
- Keep governed knowledge separate from agent-generated claims. 

- Link memory to canonical identities instead of remastering them. 

- Build context for a task instead of storing permanent relevance. 

- Use separate data stores only when security, workload, retention or ownership requires them. 

- Introduce graph structure only when relationships, time and provenance materially improve the result. 

 Most organisations do not need three new graph platforms.

 They need governed knowledge, trustworthy experience and a context assembly process that knows the difference.

# 10. References 

## Previous publications 
 
- Sergey Vasiliev. A Practical Architecture for LLM Memory: LPG as the Adaptive Knowledge Cache . 29 November 2025. 

- Sergey Vasiliev. Enterprise Knowledge Graphs vs Applied Knowledge Graphs: Same Vision, Different Operational Path . 3 December 2025. 

- Sergey Vasiliev. The Ontology Trap: When AI Scales Unvalidated Meaning . 11 June 2026. 

- Sergey Vasiliev. From LPG to GraphRAG: The Minimum Semantic Contract . 10 July 2026. 

- Sergey Vasiliev. The Missing Half of Enterprise Semantics: What the Agent Must Not Do . 31 July 2026. 

- Sergey Vasiliev. Topological and Temporal Aspects of Graphs . 19 January 2026. 

- Sergey Vasiliev. RAG vs GraphRAG vs KAG: A Graph Practitioner’s Perspective . 11 February 2026. 

- Sergey Vasiliev. Why Most GraphRAG Evaluations Are Wrong . 17 April 2026. 

 
## Technical references 
 
- Chengjin Xu et al. Context Graph . arXiv preprint, 2024. DOI: 10.48550/arXiv.2406.11160. 

- ISO/IEC 39075:2024. Information Technology: Database Languages: GQL . First edition, April 2024. 

- Neo4j. Graph Database Concepts and Cypher Documentation . Accessed 16 August 2026. 

- Neo4j. Role-Based Access Control and Graph Privileges . Accessed 16 August 2026. 

- Chang Yang et al. Graph-Based Agent Memory: Taxonomy, Techniques, and Applications . arXiv preprint, 2026. DOI: 10.48550/arXiv.2602.05665. 

- Pratyay Banerjee et al. APEX-MEM: Agentic Semi-Structured Memory with Temporal Reasoning for Long-Term Conversational AI . Proceedings of ACL 2026. DOI: 10.18653/v1/2026.acl-long.749. 

- Preston Rasmussen et al. Zep: A Temporal Knowledge Graph Architecture for Agent Memory . Company-authored arXiv preprint, 2025. DOI: 10.48550/arXiv.2501.13956. 

- Zidi Xiong et al. How Memory Management Impacts LLM Agents: An Empirical Study of Experience-Following Behavior . Proceedings of ACL 2026. DOI: 10.18653/v1/2026.acl-long.27. 

- Jiachen Qian. Visual Inception: Compromising Long-Term Planning in Agentic Recommenders via Multimodal Memory Poisoning . Proceedings of ACL 2026. DOI: 10.18653/v1/2026.acl-long.954. 

- Bo Wang et al. Unveiling Privacy Risks in LLM Agent Memory . Proceedings of ACL 2025. DOI: 10.18653/v1/2025.acl-long.1227. 

 

 6 1 Share
