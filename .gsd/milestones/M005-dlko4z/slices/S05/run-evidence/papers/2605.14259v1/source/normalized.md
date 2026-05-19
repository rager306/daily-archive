## Hypergraph Enterprise Agentic Reasoner over Heterogeneous Business Systems

Ling Wang, Songnan Liu, Jianan Wang, Cheng Cheng, Xin Liu, Yihan Zhu, Enyu Li, Yu Xiao, Jiangyong Xie, Duogong Yan, Jiangyi Chen

SUPCON, Hangzhou, China

{wangling3,liusongnan,wangjianan,chengcheng1,liuxin4,

zhuyihan,lienyu,xiaoyu2,xiejiangyong,yanduogong,chenjiangyi}@supcon.com

Applying Large Language Models (LLMs) to heterogeneous enterprise systems is hindered by hallucinations and failures in multi-hop, n-ary reasoning. Existing paradigms (e.g., GraphRAG, NL2SQL) lack the semantic grounding and auditable execution required for these complex environments. We introduce HEAR, an enterprise agentic reasoner built on a Stratified Hypergraph Ontology. Its base Graph Layer virtualizes provenance-aware data interfaces, while the Hyperedge Layer encodes n-ary business rules and procedural protocols. Operating an evidence-driven reasoning loop, HEAR dynamically orchestrates ontology tools for structured multi-hop analysis without requiring LLM retraining. Evaluations on supply-chain tasks, including order fulfillment blockage root cause analysis (RCA), show HEAR achieves up to 94.7% accuracy. Crucially, HEAR demonstrates adaptive efficiency: utilizing procedural hyperedges to minimize token costs, while leveraging topological exploration for rigorous correctness on complex queries. By matching proprietary model performance with open-weight backbones and automating manual diagnostics, HEAR establishes a scalable, auditable foundation for enterprise intelligence.

Correspondence: Ling Wang(wangling3@supcon.com)

## 1 Introduction

Modern businesses rely on heterogeneous systems-including Enterprise Resource Planning (ERP) [34], Supplier Relationship Management (SRM) [54], Warehouse Management Systems (WMS) [5], and Business Process Management (BPM) [18]-to manage end-to-end workflows like order fulfillment (Figure 1). While providing robust transactional capabilities, they often lack semantic interoperability due to severe data fragmentation. Real-world tabular data is notoriously noisy and misaligned, exhibiting schema ambiguity, inconsistent identifiers and formats, and opaque statistical metrics. For instance, custom development on a German SAP system within a Chinese enterprise often yields schema ambiguities, confusing original abbreviations with localized Pinyin acronyms.

Beyond data-level misalignment, these systems are governed by complex business constraints that span multiple platforms. These constraints dictate conditional workflow branching (e.g., routing in-house stock to delivery requests on ERP and then outbound delivery on WMS, versus directing direct-shipment physical items to SRM for procurement as shown in Figure 1), status transitions, and cross-system synchronization (e.g., aligning BPM delivery requests with ERP material document generation). Together, these factors make routine analytical tasks such as fulfillment diagnosis and delay tracing a coupled challenge of cross-system evidence acquisition and business-logic reasoning, where relevant facts are distributed, only partially aligned, and constrained by domain-specific rules [3, 42].

Although Large Language Models (LLMs) excel at natural language reasoning, they are severely hampered by this lack of semantic interoperability when navigating heterogeneous multi-hop tasks and complex business constraints. If forced to rely merely on misaligned pre-trained knowledge, they frequently suffer from severe factual hallucinations and 'reasoning stalls' [13, 67]. Standard Retrieval- Augmented Generation (RAG) paradigms seem to be a promising solution, but they fail to capture the complex n -ary relational facts required for such decision support [36, 77]. Even GraphRAG [27, 49] is limited by binary structures that cannot adequately model n -ary constraints. Hyper-topology RAG [42, 65, 23] triggers index explosion, while NL2SQL [37, 35] and general agents [6, 75] lack the simultaneous semantic grounding, topology awareness, and auditable execution required for productiongrade enterprise intelligence.

Figure 1: Order fulfillment workflow across four heterogeneous business systems: BPM, ERP, SRM, and WMSin SUPCON.

<!-- image -->

To bridge this gap, we propose HEAR ( H ypergraph E nterprise A gentic R easoner), an enterprise agentic reasoning system grounded in a Stratified Hypergraph Ontology . By representing complex, n -ary relationships as hyperedges, HEAR provides a unified framework for multi-hop reasoning that seamlessly spans heterogeneous systems. In this architecture, the Graph Layer standardizes binary connections across heterogeneous data sources, providing a virtualized, provenance-aware factual substrate. The Hyperedge Layer functions as an n -ary ontology whose hyperedges bind subsets of cross-domain graph nodes. Instead of requiring rigid symbolic representations, HEAR uses human- and machineinterpretable natural-language soft axioms in declarative hyperedges which ground admissible business states, cross-system relationships, and semantic interpretation in established graph nodes, while procedural hyperedges build on these anchors to construct detailed execution protocols and reusable reasoning workflows for routine tasks. An orchestration LLM acts as the reasoning engine, dynamically activating these hyperedges and calling tools to navigate the underlying data topology while adhering to business constraints and validated protocols. Importantly, the system evolves through the continuous, human-inthe-loop construction of these hyperedges, securely adapting to new business logic without retraining the underlying LLM.

Empirically, on real-world supply-chain tasks, this hypergraph-grounded design yields up to 94.7% accuracy on complex blockage analysis and 88.7% on general questions. Crucially, HEAR exhibits highly adaptive execution efficiency. On bounded diagnostic tasks, predefined procedural hyperedges actively constrain exploration, drastically reducing token consumption compared to baselines. Conversely, when forced into autonomous multi-hop exploration for diverse queries, HEAR deliberately scales its computational investment to secure indispensable correctness, significantly widening its strict accuracy gap over baseline methods that severely degrade. Additionally, deploying HEAR with cost-efficient open-weight models achieves proprietary-level performance and translates minimal ontology maintenance into a scalable paradigm shift that automates recurring manual diagnostics.

The main contributions of this work are summarized as follows:

- We propose a Stratified Hypergraph Ontology for enterprise intelligence. The Graph Layer registers provenance-aware data interfaces over heterogeneous systems, while the Hyperedge Layer serves as an n -ary ontology whose declarative hyperedges bind localized soft axioms to graph nodes and whose procedural hyperedges construct reusable execution protocols over those grounded anchors.
- We design HEAR , an enterprise agentic reasoner with an evidence-driven agentic reasoning loop on top of the stratified hypergraph ontology. The LLM acts as the reasoning engine that performs semantic planning and invokes ontology tools for verifiable evidence acquisition and ontology building, supporting both procedural hyperedge-guided reuse and topology-guided generalization for long-tail tasks.
- We provide a real-world empirical evaluation on enterprise supply-chain tasks. Results demonstrate that explicit hypergraph semantics drive exceptional robustness and strict accuracy as reasoning spans increase. Notably, HEAR exhibits adaptive execution efficiency: it minimizes token costs via predefined procedural hyperedges on standard tasks, while deliberately trading raw compute for indispensable correctness during multi-hop exploration. Furthermore, it achieves proprietary-level accuracy using cost-efficient open-weight models, automating cross-system manual diagnostics with minimal maintenance overhead.

## 2 Related Work

The transition toward enterprise-grade intelligent systems intersects with several research areas aimed at enhancing LLMs for enterprise applications.

Hallucination Mitigation and the Need for Grounding. LLM hallucinations hinder enterprise adoption [14, 32, 2], particularly within complex, out-of-distribution business logic [59, 43]. Treating LLMs as monolithic reasoners causes fragile logical chains, 'reasoning stalls,' and factual fabrication [73, 70, 66]. While semantic entropy detects uncertainties [21], it fails to resolve underlying knowledge conflicts. To effectively mitigate this, production-grade systems demand both informational grounding for factual accuracy and structural grounding : explicit topological constraints, verifiable tool-use, and auditable protocols [24, 51, 19]. Consequently, recent research explores retrieval- and graph-augmented architectures to achieve this, despite distinct bottlenecks.

RAG and Graph-Augmented Methods. While RAG grounds LLMs [36], standard single-hop retrieval struggles with multi-hop relationships across heterogeneous enterprise systems [13, 80]. GraphRAG and KBQA improve multi-hop reasoning [49, 45, 68, 39] but incur high maintenance costs for graph indexing as data evolves [49, 27]. Conversely, Virtual Knowledge Graphs (VKGs) enable query access without physical materialization [63, 8, 50]. Other methods aggregate outputs via auxiliary graphs [28]. Crucially, most graph systems assume binary triples, losing vital information when modeling n -ary enterprise events [16, 76, 3].

Hyper-Relational Topology in RAG. Binary graph structures often cause 'contextual fragmentation' in multi-entity relationships [27, 3]. Works like HyperGraphRAG and HypKG mitigate this by representing n -ary facts via hyperedges, preserving information density and beyond-pairwise correlations [42, 65, 23]. Recent advances further optimize hypergraph navigation: PRoH [74] dynamically plans multi-hop reasoning paths over knowledge hypergraphs, while Cog-RAG [30] utilizes dual-hypergraphs to achieve both global thematic and local entity alignment. However, applying these instance-level methods directly to enterprise tabular environments requires instantiating millions of tabular rows into a hypergraph, which could trigger index explosion, rendering them unsuitable for industrial environments.

LLM-Driven Ontology and Constraint Modeling. Ontologies are often constructed using formal semantic standards (e.g., RDF [15] and OWL [29]) alongside rigorous cross-system mappings. To streamline this modeling process, recent works explore LLM-driven generation: NeOn-GPT [22] and OntoEKG [47] automate ontology creation; AutoQG [57] and AttributeForge [31] scale schema generation via multi-agent orchestration; LLM-CG [38] extracts relational constraints; and Matchmaker [55] and ConStruM [9] enable zero-shot schema matching. Inspired by advances like OG-RAG [56], HEAR provides an alternative to these formal structural methods, utilizing natural language to explicitly represent n -ary axioms among nodes. Because this format is natively interpretable by both humans and LLMs, it establishes lightweight solutions that enable the agile, on-demand construction of semantic hypergraphs.

Memory Systems and Agent Frameworks. Memory systems extend context via structured representations [48, 79, 61, 12, 69], but this passive factual recall often causes memory fragmentation by failing to enforce n -ary business constraints. Concurrently, unconstrained long interaction chains in agentic frameworks inevitably introduce semantic drift [52, 62, 19, 44]. HEAR overcomes both bottlenecks via its Stratified Hypergraph Ontology. Instead of retrieving fragmented past interactions, HEAR encapsulates semantics and task-specific execution protocols directly into hyperedges, providing topology-constrained pathways that actively guide tool invocation and prevent logical drift.

Table-Aware Retrieval and Enterprise Querying. NL2SQL techniques effectively bridge users and databases [37] but struggle with the n -ary constraints and massive schemas typical of enterprise reasoning [71, 35]. To handle heterogeneous tabular data, parallel table-centric RAG methods [81, 72] and schemagraph frameworks like CSR-RAG [58] attempt to map schemas for retrieval; however, they often falter on cross-system queries lacking explicit column-name linkage, which the hyperedges in HEAR can supplement. Other approaches aim to improve robustness via interactive feedback [46], trace-reuse memory [6, 75], or mapping schemas to retrievable text nodes (e.g., LlamaIndex [40]). More critically, these isolated optimizations lack an iterative ReAct paradigm [70] to enhance LLMs, so they remain unable to dynamically refine execution plans based on intermediate execution results-a structural gap that HEAR's agentic reasoning loop explicitly addresses.

Case Study: Supply-Chain Root-Cause Analysis. Supply-chain root-cause analysis (RCA) inherently requires tracing multi-hop dependencies across highly heterogeneous enterprise systems (e.g., ERP, SRM, WMS, BPM) [34, 54, 5, 33]. To diagnose such complex anomalies, recent works integrate LLMs with knowledge graphs [17], utilize ontology-guided extraction for risk monitoring [78, 20], and explore hypergraph structures to model failure propagation [64]. Addressing this systemic fragmentation, HEAR specifically tackles order fulfillment blockage RCA by localizing stalled contracts through the composition of cross-system evidence. This concrete industrial application directly motivates our Stratified Hypergraph Ontology, providing the necessary structural foundation for topology-constrained retrieval and logical analysis.

## 3 Problem Formulation

To achieve intelligent operational decision analysis across heterogeneous enterprise systems, extremely stringent requirements [13] must be satisfied. To prevent the LLM from hallucinating, losing track of multi-hop dependencies, or stalling, the system must address three core challenges:

1. Data Fragmentation and Implicit Anomalies: Enterprise knowledge is heavily fragmented across heterogeneous systems (e.g., ERP, WMS, SRM, BPM), where real-world tabular data is notoriously noisy and misaligned. To orchestrate reasoning, the engine must overcome severe semantic interoperability barriers [11, 63]. It must dynamically resolve a myriad of implicit data and process anomalies-such as semantic ambiguity in schemas, field-level equivocation, opaque statistical metrics, and cross-system identifier inconsistencies.
2. Complex n -ary Business Constraints: Workflows adhere to conditional logic involving multiple entities simultaneously. Capturing these multifaceted constraints requires representations beyond binary schema mappings to provide clear semantic and executive guidelines.
3. Verifiable Multi-Hop Reasoning: Navigating topologies requires iterative reasoning to correct early hypothesis drift [4]. To ensure audit compliance, every intermediate step and the final conclusion must be grounded in traceable data provenance.

Formal Definition. Given a natural language query q representing the task, a heterogeneous data environment D = { D 1 , . . . , D m } , and a set of soft axioms R encapsulating complex n -ary business constraints, the task is to orchestrate an LLM to dynamically generate a sequence of executable reasoning steps π = ( a 1 , . . . , a n ) . Each step a i may retrieve verifiable evidence from D , apply relevant axioms in R , or think about subsequent actions based on the current context. The ultimate objective is to synthesize a definitive response r , grounded in a completely auditable trace of data provenance and logical reasoning.

## 4 Methodology

To address the multifaceted challenges of enterprise intelligence formulated in Section 3, we propose HEAR , an agentic reasoner built on a Stratified Hypergraph Ontology . As illustrated in Figure 2, the HEAR architecture operates as an end-to-end stack across three integrated layers. At the top, the Orchestration layer hosts an LLM acting as the reasoning engine within an Agentic Reasoning Loop , which iteratively drives context initialization, semantic planning, evidence acquisition, and auditable termination for a given query q . This loop is structurally grounded by the middle Stratified Hypergraph Ontology , which separates a base Graph G = ( V , E ) of virtualized, provenance-aware data interfaces as nodes from an upper Hyperedge layer H that binds nodes via declarative n -ary soft axioms and procedural protocols. The reasoning engine interacts with this substrate via a dedicated Ontology Tools suite (enabling hyperedge retrieval, topology exploration, graph data access, and auxiliary executions), ensuring that reasoning plans are executed through verifiable operations. Finally, the foundational Data layer encompasses heterogeneous enterprise systems (e.g., WMS, SAP, BPM, SRM) in-place. The following subsections formalize this ontology and the evidence-driven reasoning loop that operates upon it.

## 4.1 Architecture of the Stratified Hypergraph Ontology

Unlike flat knowledge graphs that force complex relationships into binary triples and thereby fragment context, HEAR adopts a Stratified Hypergraph Ontology to achieve a unified representation of enterprise data and business logic. This stratified design decouples stable data entities from evolving semantics, supporting iterative ontology refinement without requiring expensive pipeline engineering or physical schema changes.

Figure 2: Overview of HEAR . The system combines an Agentic Reasoning Loop with a Stratified Hypergraph Ontology: the Graph Layer registers virtualized, provenance-aware data interfaces and binary relations over heterogeneous business systems, while the Hyperedge Layer organizes n -ary semantic units on top of the base graph. An LLM acts as the reasoning engine and interacts with ontology tools over heterogeneous enterprise data.

<!-- image -->

Graph Layer (Virtual Entity-Relational Substrate). The Graph Layer consists of Nodes V and Edges E representing binary relational structures. Each node v ∈ V maps to a curated entity proxy (e.g., a semantic view of a database table or API endpoint) rather than storing raw data. This design implements the data virtualization paradigm from Virtual Knowledge Graphs (VKG) [63], where data remains in-place and is accessed through declarative mappings. Core node attributes include: id , name , description , and schema (enriched with field-level annotations to guide the LLM reasoning engine). Edges E represent deterministic binary associations, such as cross-system foreign-key relationships, to support multi-hop data acquisition.

Hyperedge Layer ( n -ary Semantic Ontology). Ahyperedge e acts as a higher-order container binding graph node subsets V e ⊆ V to localized soft (natural language) axioms R e ⊆ R . Implemented within the semantic\_details fi eld, these axioms express business conditions, anomaly triggers, and tool-use boundaries in a unified format. Functionally, Declarative Hyperedges ground admissible business states and cross-system relationships within the factual topology of the Graph Layer. Drawing inspiration from modular agentic skills [60, 53], Procedural Hyperedges build upon these declarative anchors to encapsulate reusable reasoning workflows and execution protocols. Ultimately, this humanand machine-interpretable representation provides the explicit structural scaffolding required for the orchestration LLM to navigate complex branching logic and multi-step validation with high fidelity.

Beyond these connecting nodes and soft axioms, a hyperedge e is structurally defined by its title and description , which will be used later for retrieval. In addition, the related\_hyperedges and attachments define ontological dependencies and paths to executable artifacts (e.g., Python scripts).

Hyperedges are also partitioned by governance into Global and Scoped types, ensuring architectural extensibility where role-specific protocols can be introduced without altering the underlying shared graph substrate.

Unified Heterogeneous Graph Structure. Following the principle of bipartite incidence representations [7], we model hyperedges as first-class nodes within the architecture. This transformation consolidates the stratified ontology into a unified heterogeneous graph, formally parameterized by a tuple of five elements: Base Graph Nodes V and Binary Edges E representing entity proxies and their structural links; Hyperedge Nodes V H that model the hyperedges themselves; Incidence Edges E V , H that define the membership connections between base graph nodes and hyperedge nodes; and Inter-Hyperedge Edges E H connecting hyperedge nodes via related\_hyperedges fi elds into a cognitive navigational network. This provides a standardized search space for both high-level semantic exploration and precise data retrieval.

## 4.2 Ontology Tools for Access, Management, and Execution

To bridge the LLM Reasoning Engine with physical data execution, HEAR provides several core tool tiers-Graph Data Access, Topology Exploration, Hyperedge Retrieval, and Hyperedge Building-as well as auxiliary execution primitives. Together, these tools support transitions between localized evidence access, topology-guided generalization, and ontological-unit management, while ensuring that intermediate reasoning traces are provenance-aware and auditable.

Graph Data Access Tools. This tool extracts physical evidence from the data substrate 1 . To strictly enforce enterprise governance, all operations are wrapped in a permission hook that validates user privileges and applies data-level access controls (e.g., truncating unauthorized fields or rejecting requests). Authorized access utilizes two modes:

(1) Direct Access : Executes LLM-generated structured queries (e.g., SQL, RESTful APIs) for deterministic retrieval.

(2) Topology-Driven Access : Operationalizes multi-hop gathering across data nodes. Given an input graph node subset and constraints (e.g., SalesOrder.id=123 ), it first verifies graph-layer connectivity to preclude hallucinated retrieval paths. Then the tool implements federated retrieval by sequentially propagating join keys along the validated subgraph, orchestrating localized queries to seamlessly bridge heterogeneous systems.

Hyperedge Retrieval Tools. To balance low-latency execution with flexible semantic search, HEAR manages hyperedges via two mechanisms that enforce lazy context expansion:

(1) Passive Activation : For queries q containing exact hyperedge titles or aliases, HEAR uses multipattern string matching (e.g., Trie or Aho-Corasick [25, 1]) for zero-latency detection. It injects a lightweight hyperedge summary ( title and description ) into the initial context C , deferring full semantic\_details to lazy loading.

(2) Active Retrieval : For complex queries lacking direct matches, the LLM generates searching queries q search = LLM plan ( q ) based on the task goal. A hybrid retrieval strategy utilizes q search and hyperedge summaries, combining dense embeddings (e.g., BGE [10]) and sparse BM25 signals to rank candidates. Hyperedge summaries whose similarities exceed a composite threshold τ are returned to the LLM.

1 Results are returned directly if small, or persisted as referenced artifacts if large.

The LLM then autonomously decides whether to invoke a detail-read operation for full semantic\_details and attachments . Crucially, this retrieval is not a single-pass operation; the reasoning engine can invoke the tool iteratively, dynamically pulling additional hyperedges across multiple reasoning steps as new intermediate findings or requirements emerge.

Topology Exploration Tools. To navigate the stratified ontology beyond initially retrieved hyperedges while preventing context bloat, this tool offers two targeted traversal modes: (1) Local Adjacency Inspection : Restricted to base graph nodes, this mode retrieves semantic descriptions and immediate binary relationships for specified entities. This enables safe, single-hop context expansion without uncontrolled recursive materialization. (2) Bounded Path Discovery : Operating across all node types (both base graph nodes and hyperedges), this mode executes constrained traversal (e.g., breadth-first search) to autonomously discover viable multi-hop structural correlations between disjoint entities. By orchestrating both modes, the LLM dynamically traces cross-system logic and valid graph data access queries-for example, peeking at a Sales Order 's binary neighbors before invoking path discovery through intermediate nodes (e.g., Delivery Request ) to locate a final Order Fulfillment record.

Hyperedge Building Tool. To support secure, continuous evolution without model retraining or expensive ontology building pipelines, HEAR employs an adaptive construction framework enabling human-AI collaboration. While some declarative hyperedges are auto-instantiated directly from graph nodes during initial system setup, this tool empowers domain experts to dynamically create and refine both hyperedge types on demand. Users can iteratively adjust declarative constraints in semantic\_details . For procedural hyperedges, the tool facilitates LLM-assisted co-creation: the LLM drafts execution scripts from natural language intents, which experts then validate and finalize. This human-in-the-loop approach ensures scalable, verifiable ontology expansion.

This establishes a self-improving cognitive loop . While novel queries rely on zero-shot topology exploration, successful multi-hop reasoning traces for high-frequency tasks can be distilled into reusable procedural hyperedges. This progressively shifts the system from flexible exploration to highly efficient, deterministic protocol reuse. Finally, to ensure structural integrity, construction strictly enforces rolebased access control : global hyperedges require root-level review, whereas tenant-scoped hyperedges isolate local logic, requiring explicit approval for any cross-scope dependencies.

Terminal and Code Execution Tools. These tools provide execution primitives for the operational steps referenced within semantic\_details or encoded in attachments for procedural hyperedges. They serve as the critical bridge that translates the soft protocols of a procedural hyperedge into actionable system operations, enabling the dynamic integration of external workflows and real-time automation. To ensure enterprise-grade security and prevent malicious system-level intrusions [41], all script executions and file system interactions are strictly confined within isolated sandbox environments.

## 4.3 Agentic Reasoning Loop on the Stratified Hypergraph Ontology

Having established the Stratified Hypergraph Ontology, we now detail how HEAR employs an LLM as its core engine to orchestrate a dynamic, evidence-driven agentic reasoning loop. Given an input query q , the system initializes an evolving context state C . By structurally grounding the LLM's reasoning process within the ontology, each tool invocation explicitly enriches C with verifiable factual anchors. This mechanism guarantees adaptive planning and end-to-end auditability as the reasoning engine iteratively executes the following phases:

- Context Initialization: A lightweight matching layer scans q to detect relevant hyperedges (detailed in Section 4.2) or domain terms. These compact semantic hints are injected into the initial LLM context C , deliberately avoiding the eager loading of full hyperedge details to conserve token space.
- LLMReasoning and Tool Selection: Conditioned on the current context state C and historical tool outputs, the LLM decomposes the current objective into immediate actionable sub-tasks, formulating a local execution plan for the current iteration. Conceptually, the agent dynamically selects from the specialized tool tiers (defined in Section 4.2) to: (i) invoke the Hyperedge Retrieval Tool to actively search for relevant hyperedge summaries, and subsequently make autonomous decisions to lazily load full semantic\_details for promising candidates; (ii) utilize the Graph Data Access Tool to extract physical evidence through direct queries or topologydriven federated retrieval; (iii) employ the Topology Exploration Tool to inspect graph nodes' adjacency or discover unmapped reasoning pathways when retrieved hyperedges are insufficient; or (iv) execute auxiliary primitives (e.g., safe terminal operations) through the Terminal and Code Execution Tools . These invocations can be generated simultaneously or fluidly interleaved.
- Evidence Acquisition and Context Revision: Upon receiving the execution results, HEAR appends the newly acquired verifiable evidence directly to the context state C 2 . This linearly expanded state then feeds directly back into the reasoning phase, ensuring that the agent's subsequent tool selections strictly adhere to the latest findings and the business constraints encoded within the active hyperedges.
- Auditable Termination: The iterative loop terminates when the LLM determines that the accumulated evidence in C is sufficient to definitively resolve the input query q . The final response r is directly synthesized from, and firmly grounded in, this transparent and auditable evidence trace.

Typical Structured Multi-hop Reasoning Paths. While the core reasoning loop is unified, HEAR realizes structured multi-hop reasoning through two primary execution paths, supported by intra- and interhyperedge navigation. Path A (Procedural Hyperedge-Guided) relies on intra-hyperedge composition: the LLM actively retrieves and lazily loads a relevant procedural hyperedge whose semantic\_details encapsulate a reusable multi-hop protocol over graph nodes and the declarative anchors attached to them. For example, a procedural hyperedge for order fulfillment blockage root-cause analysis can prescribe an ordered evidence-acquisition program spanning Contract Signing (BPM), Delivery Request (ERP), Outbound Delivery (WMS), and supplier-side Purchase Order traces (SRM), strictly adhering to the conditional branches defined by the active soft axioms. Conversely, Path B (Topological Ontology Exploration) serves as a fallback for long-tail queries lacking procedural hyperedges. Anchoring to declarative hyperedges, the LLM orchestrates the dual-mode Topology Exploration Tool-dynamically alternating between local adjacency inspection and bounded path discovery-to autonomously navigate the underlying hypergraph and resolve novel reasoning pathways. Alongside utilizing the Topology Exploration Tool for navigation, the LLM could also leverage the related\_hyperedges fi eld to query semantic neighbors, recursively load relevant successors, and assemble an interpretable high-level chain (e.g., e 1 → e 2 →··· → e k ) tailored to the dynamically evolving query context.

## 5 Experiments

2 When the length of context C exceeds a predefined threshold, a system-level context compaction step is triggered to retain essential factual traces.

## 5.1 Experimental Setup

## 5.1.1 Datasets

We extracted real-world business data from the Order Fulfillment (OF) process of SUPCON. To prevent irreproducible results caused by continuous production updates, we synchronized static snapshots of 20 tables to local databases. Spanning heterogeneous systems (BPM, ERP, SRM, and WMS), this provides a high-fidelity foundation for evaluating multi-hop analysis tasks. We construct two datasets sharing the same database snapshots but differing in queries and ground-truth generation: (1) Order Fulfillment Blockage Root-Cause Analysis (OF-BRCA): 95 standardized questions following the template 'Why has contract [X] not completed fulfillment?'. Answering requires chaining evidence across all systems to produce a grounded diagnosis. As this reflects an actual operational bottleneck, the ground truth was rigorously established by domain experts who manually investigated each contract across the heterogeneous systems. (2) Order Fulfillment Generalization (OF-General): 160 general and diverse questions categorized by required reasoning span. Span is operationalized by the number of distinct data nodes involved, measured as k = | nodes | ∈ { 0 , 1 , 2 , 3 , &gt; 3 } . To construct the ground truth for these diverse scenarios, domain experts engaged in multi-turn interactions with an AI assistant, actively verifying retrieved evidence, correcting reasoning trajectories, and refining the final conclusions to align with accepted enterprise statistical standards and operational definitions.

## 5.1.2 Evaluation Metrics &amp; Configuration

We evaluate three core metrics: Accuracy , Tool turns (iterations executed), and total Tokens consumed. To validate accuracy, we adopt the LLM-as-a-judge framework [26] followed by human expert verification, evaluating the final conclusion alongside its supporting tool evidence. For OF-General, we apply a threeway label ( correct / partially correct / incorrect ) to accommodate definitional nuances, reporting correct and correct + partially correct accuracies. For OF-BRCA, accuracy is strictly binary ( correct vs. incorrect ).

HEAR runs with a maximum budget of 50 tool turns. Unless stated otherwise, we use GPT-5 as the default LLM backbone, but also evaluate DeepSeek-3.2, Qwen-3.5-27B, and Qwen-Next 80B to quantify model-capability effects. Declarative hyperedges are auto-extracted and refined to bind soft business axioms to established graph nodes. Crucially, we authored a dedicated procedural hyperedge for OF-BRCA to construct the validated fulfillment-diagnosis protocol, explicitly aligning it with Path A (Procedural Hyperedge-Guided) execution. Lacking such predefined protocols, OF-General naturally defaults to Path B (Topological Ontology Exploration) for autonomous multi-hop discovery.

## 5.1.3 Ablation and Baselines

To isolate our hypergraph representation's advantages, we compare HEAR against ablations and baselines. For fair evaluation, all baselines receive the identical business axioms R (Section 3) as prompts in their initial context.

- HEAR (Complete) : The proposed method with both declarative anchors and procedural protocols available.
- HEAR (Declarative) : Only declarative hyperedges are available; procedural hyperedges are removed.
- Table-RAG : Hyperedges removed. Uses a highly scalable, multi-granularity index over schema metadata via LlamaIndex [40] to generate and execute SQL.

Table 1: Comparison on OF-BRCA to evaluate the importance of hyperedges. Here HEAR (Complete) exposes both declarative and procedural hyperedges, whereas HEAR (Declarative) keeps only declarative hyperedges. Accuracy is binary (correct vs. incorrect).

| Category          | Method     | Hyperedges   |   Accuracy (%) | Tool turns       | Tokens ( × 10 3 )   |
|-------------------|------------|--------------|----------------|------------------|---------------------|
|                   | HEAR       | Complete     |           94.7 | 6.3 ± 0.2 14.9 ± | 38.1 ± 1.3          |
|                   | HEAR       | Declarative  |           74.2 | 0.4              | 179.3 ± 7.1         |
| Internal Baseline | Table-RAG  | No           |           52.6 | 10.8 ± 0.5       | 78.5 ± 5.2          |
| Internal Baseline | Table list | No           |           51.5 | 4.9 ± 0.3        | 63.1 ± 5.0          |
| External Baseline | CSR-RAG    | -            |           11.3 | 3.0 ± 0.0        | 16.7 ± 0.7          |

Figure 3: Accuracy and efficiency on OF-BRCA across LLM backbones when both declarative and procedural hyperedges are available.

<!-- image -->

- Table list : Hyperedges removed. Relies autonomously on a flat, semantically processed list of table identifiers and interface descriptions in the initial context.
- CSR-RAG : A state-of-the-art multi-table retrieval architecture [58] that constructs a schema hypergraph to generate SQL, entirely outside the HEAR agentic loop.

## 5.2 Evaluation on OF-BRCA Dataset

Evaluating HEAR on OF-BRCA (Table 1), HEAR (Complete) achieves a dominant 94.7% accuracy. Ablation reveals procedural hyperedges critically constrain exploration: removing them drops accuracy by 20.5 points (to 74.2% for HEAR Declarative) and drastically inflates search costs (from 6.3 turns/38.1K tokens to 14.9 turns/179.3K tokens). Yet, this declarative-only variant still significantly outperforms Table-RAG (52.6%), Table list (51.5%), and CSR-RAG (11.3%), proving axiomatic structure alone establishes a superior foundation. Error analysis reinforces this: lacking global state tracking, Table-RAG conflates disconnected evidence, while CSR-RAG's collapse confirms static pipelines outside an iterative loop inherently fail at complex multi-hop reasoning.

Comparing LLM backbones under complete HEAR (Figure 3), the proprietary GPT-5 (94.7%) and DeepSeek-3.2 (93.8%) narrowly lead the open-weight Qwen-3.5-27B (92.7%), while Qwen-Next-80B collapses (45.4%). Although GPT-5 is the most efficient (6.29 turns, 38.1K tokens), deploying Qwen-3.527B demonstrates that an open-weight model can achieve proprietary-level accuracy with exceptional costefficiency. In contrast, DeepSeek-3.2 incurs materially higher costs (84.7K tokens) due to a conservative, verification-heavy style. Finally, residual errors across the top models cluster around undocumented enterprise anomalies (e.g., supplier dropship logic), indicating that long-tail workflow ambiguity-not generic reasoning capacity-is the primary bottleneck.

Table 2: Comparison on OF-General to evaluate the importance of hyperedges. Under Accuracy (%), Correct reports fully correct answers only, and +Partial reports fully correct plus partially correct answers.

| Category          | Method     | Hyperedges   | Accuracy (%)   | Accuracy (%)   | Tool turns   | Tokens ( × 10 3 )   |
|-------------------|------------|--------------|----------------|----------------|--------------|---------------------|
|                   |            |              | Correct        | +Partial       |              |                     |
|                   | HEAR       | Complete     | 37.1           | 88.7           | 7.3 ± 0.4    | 81.6 ± 8.5          |
|                   |            | Declarative  | 35.8           | 86.2           | 7.4 ± 0.3    | 76.7 ± 5.8          |
| Internal Baseline | Table-RAG  | No           | 29.6           | 85.5           | 6.7 ± 0.3    | 40.8 ± 3.7          |
|                   | Table list | No           | 28.9           | 88.7           | 4.5 ± 0.2    | 35.1 ± 3.0          |
| External Baseline | CSR-RAG    | -            | 6.3            | 72.2           | -            | 8.1 ± 0.4           |

Figure 4: Comparison on OF-General by node-count bucket (defined by k = | nodes | as a proxy for required reasoning span). The stacked bars report Correct and +Partial accuracies.

<!-- image -->

## 5.3 Evaluation on OF-General Dataset

We next evaluate on OF-General (Table 2). Unlike OF-BRCA, this diverse dataset lacks procedural hyperedges targeted at specific questions, forcing Path B (Topological Ontology Exploration) for multihop reasoning. While baselines appear competitive under the lenient Correct+Partial metric (both HEAR Complete and Table list hit 88.7%), the strict Correct metric shows clear separation. HEAR (Complete) peaks at 37.1%. Notably, HEAR (Declarative) also performs strongly (35.8% fully correct), proving the value of declarative axioms even without predefined protocols. Both outperform Table-RAG (29.6%), while CSR-RAG performs poorest (6.3%) because its lack of an agentic reasoning loop severely limits multi-hop evidence gathering. HEAR pays a higher token cost than Table-RAG (81.6K vs. 40.8K tokens) for this capability, a reasonable trade-off when prioritizing correctness.

By-complexity results (Figure 4) further demonstrate this exploration dynamic under the Correct+Partial metric. As reasoning spans grow and predefined routes vanish, Path B execution remains competitive across all buckets: HEAR ties or leads the top scores at k ≤ 2 (95.1% and 84.8%), leads clearly at k =3 (89.5%), and remains tied at k&gt; 3 (70.8%). The clearest separation appears in the mid-complexity k =3 bucket, where HEAR strictly outperforms Table list (84.2%) and Table-RAG (63.2%). Constructing longer reasoning chains naturally raises execution costs: turns increase from 5.52 ( k =1 ) to 11.84 ( k =3 ), while tokens rise from 41.7K to 201.1K. Although baselines like Table-RAG use fewer tokens on harder buckets, their weaker accuracy reinforces our trade-off: HEAR accepts moderate overhead for reliable multi-hop reasoning.

Comparing LLM backbones under complete HEAR (Figure 5), GPT-5 (88.7%) narrowly leads Qwen3.5-27B (83.0%). Echoing OF-BRCA results, Qwen stands out as a highly competitive open-weight alternative that reaches near-frontier accuracy. DeepSeek-3.2 is weaker in both effectiveness (78.0%) and efficiency (22.4 turns, 224.1K tokens vs. GPT-5's 7.3 turns, 81.6K tokens) due to a verification-heavy style. Overall, robust hypergraph grounding makes Qwen-3.5-27B a highly practical open-weight choice, while GPT-5 retains the strongest overall profile on complex questions.

Figure 5: LLM backbone comparison on OF-General.

<!-- image -->

## 5.4 Maintenance Overhead and Practicality

The Stratified Hypergraph Ontology minimizes knowledge maintenance overhead. Across 20 tables, declarative hyperedges are instantiated in seconds via automated metadata extraction, requiring only hours for experts to refine semantic\_details . Constructing procedural hyperedges demands targeted upfront investment; e.g., formalizing 'order fulfillment blockage analysis' requires 1-2 days of human-AI co-creation.

However, this yields compounding gains. Conventionally, diagnosing 200 blockages takes 20 persondays manually. Grounded by procedural hyperedges, HEAR executes this workload in under 10 hours with verified correctness. By interfacing with RPA endpoints to trigger corrective actions, HEAR translates minimal maintenance into a paradigm shift: from reactive diagnostics to proactive automation.

## 6 Conclusion and Future Work

In this work, we introduced HEAR , a Hypergraph Enterprise Agentic Reasoner that grounds LLMs within a Stratified Hypergraph Ontology. By resolving heterogeneous data dependencies and modeling n -ary business constraints to enforce traceable multi-hop reasoning, HEAR enables high-fidelity, audit-compliant execution for complex enterprise tasks. Evaluations on real-world supply-chain data demonstrate that explicit hypergraph semantics secure strict accuracy and adaptive execution efficiency.

Future research will focus on scaling and hardening this cognitive infrastructure through: (1) automated hypergraph entropy control to prune redundancy and retrieval noise; (2) graph-enhanced embeddings for more robust semantic hyperedge retrieval; and (3) seamless alignment with established ontology standards (e.g., RDF, OWL) to leverage standardized knowledge schemas and enhance cross-system interoperability. Ultimately, we aim to provide an auditable, scalable, and self-improving foundation for next-generation enterprise intelligence.

## Acknowledgment

We thank Yao Yao, Yongfeng Zhang, Teng Wang, and Ao Sun for their assistance with database setup, hyperedge semantic drafting, and dataset construction. We also express our gratitude to the Order Management Department for supplying domain-specific business constraints and verifying the system in real-world production.

## Declaration of Use of Generative AI

Gemini, GPT, and Kimi were utilized to generate sections of this work, including underlying source code, bibliographic metadata, and drafted prose for the abstract, introduction, and experiments. The core intellectual contributions, system architecture, and experimental design were authored entirely by the human researchers. These models functioned strictly as assistants; the authors critically reviewed, edited, and verified all AI-generated content, assuming full responsibility for the accuracy, originality, and integrity of the entire work.

## Supplemental Material Statement

Due to strict proprietary policies and non-disclosure agreements (NDAs) with the industrial partner, neither the raw datasets (OF-BRCA and OF-General) nor the integrated source code can be publicly released. The datasets contain sensitive production data from core enterprise manufacturing systems (ERP, SRM, etc.), and the source code involves proprietary production interfaces. However, to ensure reproducibility and facilitate the verification of our results, this paper is designed to be fully self-contained. Extended details regarding the proposed methodology and further experimental configurations and findings are thoroughly documented in the Annexes of this submission.

## References

- [1] Alfred V Aho and Margaret J Corasick. Efficient string matching: an aid to bibliographic search. Communications of the ACM , 18(6):333-340, 1975.
- [2] Aisha Alansari and Hamzah Luqman. Large language models hallucination: A comprehensive survey. arXiv preprint arXiv:2510.06265 , 2025.
- [3] Alessia Antelmi, Pasquale De Meo, Fabiana Amato, Giovanni Ponti, and Domenico Ursino. A survey on hypergraph representation learning. ACM Computing Surveys (CSUR) , 2023.
- [4] Mahdi Astaraki, Mohammad Arshi Saloot, Ali Shiraee Kasmaee, Hamidreza Mahyar, and Soheila Samiee. When iterative rag beats ideal evidence: A diagnostic study in scientific multi-hop question answering. arXiv preprint arXiv:2601.19827 , 2026. Accessed: March 6, 2026.
- [5] John J. Bartholdi, III and Steven T. Hackman. Warehouse and Distribution Science: Release 0.97 . The Supply Chain and Logistics Institute, 2016.
- [6] Asim Biswal, Chuan Lei, Xiao Qin, Aodong Li, Balakrishnan Narayanaswamy, and Tim Kraska. Agentsm: Semantic memory for agentic text-to-sql. arXiv preprint arXiv:2601.15709 , 2026.
- [7] Alain Bretto. Hypergraph Theory: An Introduction . Mathematical Engineering. Springer Science &amp; Business Media, 2013.

- [8] Diego Calvanese, Benjamin Cogrel, Sarah Komla-Ebri, Roman Kontchakov, Davide Lanti, Martín Rezk, Mariano Rodriguez-Muro, and Guohui Xiao. Ontop: Answering sparql queries over relational databases. Semantic Web , 8(3):471-487, 2017.
- [9] Houming Chen, Zhe Zhang, and H. V. Jagadish. Construm: A structure-guided llm framework for context-aware schema matching, 2026.
- [10] Jianlv Chen, Shitao Xiao, Peitian Zhang, Kun Luo, Defu Lian, and Zheng Liu. M3-embedding: Multi-linguality, multi-functionality, multi-granularity text embeddings through self-knowledge distillation, 2025.
- [11] Mengyuan Chen, Chengjun Dai, Xinyang Dong, Chengzhe Feng, Kewei Fu, Jianshe Li, Zhihan Peng, Yongqi Tong, Junshao Zhang, and Hong Zhu. Dingtalk-deepresearch: A unified multi-agent framework for adaptive intelligence in enterprise environments. arXiv preprint arXiv:2510.24760 , 2025. Accessed: March 6, 2026.
- [12] Prateek Chhikara, Dev Khant, Saket Aryan, Taranjeet Singh, and Deshraj Yadav. Mem0: Building production-ready ai agents with scalable long-term memory. arXiv preprint arXiv:2504.19413 , 2025.
- [13] Prafulla Kumar Choubey, Xiangyu Peng, Shilpa Bhagavath, Kung-Hsiang Huang, Caiming Xiong, and Chien-Sheng Wu. Benchmarking deep search over heterogeneous enterprise data. arXiv preprint arXiv:2506.23139 , 2025. Accessed: March 6, 2026.
- [14] Manuel Cossio. A comprehensive taxonomy of hallucinations in large language models. arXiv preprint arXiv:2508.01781 , 2025.
- [15] Richard Cyganiak, David Wood, and Markus Lanthaler. RDF 1.1 concepts and abstract syntax. W3C recommendation, World Wide Web Consortium, 2014.
- [16] Rajarshi Das, Ameya Godbole, Ankita Naik, Elliot Tower, Robin Jia, Manzil Zaheer, Hannaneh Hajishirzi, and Andrew McCallum. Knowledge base question answering by case-based reasoning over subgraphs. In International Conference on Machine Learning (ICML) , 2022.
- [17] Maulik Divakar Dixit. Transforming root cause analysis for a global pharmaceutical leader with a knowledge graph and GenAI-enabled solution. Tredence Industrial AI Case Studies , 2025.
- [18] Marlon Dumas, Marcello La Rosa, Jan Mendling, and Hajo A. Reijers. Fundamentals of Business Process Management, Second Edition . Springer, 2018.
- [19] Sujan Dutta, Junior Francisco Garcia Ayala, Pranav Pujar, Sai Sree Harsha, Dan Luo, Nikhil Vasudeva, Bikas Saha, Pritom Baruah, and Yunyao Li. Pager: Proactive monitoring agent for enterprise ai assistant. In Proceedings of the AAAI Conference on Artificial Intelligence (Demonstration Track) , 2026.
- [20] Shikha Duttyal. Knowledge graph-enhanced llms for supply chain intelligence: Automating network visibility through public data mining. TIJER - International Research Journal , 2025.
- [21] Sebastian Farquhar, Jannik Kossen, Lorenz Kuhn, and Yarin Gal. Detecting hallucinations in llms using semantic entropy. Nature , 2024.
- [22] Nadeen Fathallah, Arunav Das, Stefano De Giorgis, Andrea Poltronieri, Peter Haase, and Liubov Kovriguina. Neon-gpt: A large language model-powered pipeline for ontology learning. In The Semantic Web: 21st International Conference, ESWC 2024 . Springer, 2024.

- [23] Yifan Feng, Hao Hu, Xingliang Hou, Shiquan Liu, Shihui Ying, Shaoyi Du, Han Hu, and Yue Gao. Hyper-RAG: Combating llm hallucinations using hypergraph-driven retrieval-augmented generation. arXiv preprint arXiv:2504.08758 , 2025.
- [24] Matthew Fitzpatrick. How to make ai work in your enterprise through integration and not silos. World Economic Forum , 2026.
- [25] Edward Fredkin. Trie memory. Communications of the ACM , 3(9):490-499, 1960.
- [26] Jiawei Gu, Xuhui Jiang, Zhichao Shi, Hexiang Tan, Xuehao Zhai, Chengjin Xu, Wei Li, Yinghan Shen, Shengjie Ma, Honghao Liu, Yuanzhuo Wang, and Jian Guo. A survey on llm-as-a-judge. arXiv preprint arXiv:2411.15594 , 2024.
- [27] Haoyu Han, Yu Wang, Harry Shomer, Kai Guo, Jiayuan Ding, Yongjia Lei, Mahantesh Halappanavar, Ryan A. Rossi, Subhabrata Mukherjee, Xianfeng Tang, Qi He, Zhigang Hua, Bo Long, Tong Zhao, Neil Shah, Amin Javari, Yinglong Xia, and Jiliang Tang. Retrieval-augmented generation with graphs (GraphRAG). arXiv preprint arXiv:2501.00309 , 2025.
- [28] Xinrui He, Yikun Ban, Jiaru Zou, Tianxin Wei, Curtiss B. Cook, and Jingrui He. Llm-forest: Ensemble learning of llms with graph-augmented prompts for data imputation. In Findings of the Association for Computational Linguistics: ACL 2025 , 2025.
- [29] Pascal Hitzler, Markus Krötzsch, Bijan Parsia, Peter F Patel-Schneider, and Sebastian Rudolph. OWL2web ontology language document overview (second edition). W3C recommendation, World Wide Web Consortium, 2012.
- [30] Hao Hu, Yifan Feng, Ruoxue Li, Rundong Xue, Xingliang Hou, Zhiqiang Tian, Yue Gao, and Shaoyi Du. Cog-RAG: Cognitive-inspired dual-hypergraph with theme alignment retrieval-augmented generation. In Proceedings of the AAAI Conference on Artificial Intelligence , 2026.
- [31] Yunhan Huang, Klevis Ramo, Andrea Iovine, Melvin Monteiro, Sedat Gokalp, Arjun Bakshi, Hasan Turalic, Arsh Kumar, Jona Neumeier, Ripley Yates, Rejaul Monir, Simon Hartmann, Tushar Manglik, and Mohamed Yakout. AttributeForge: An agentic LLM framework for automated product schema modeling. In Proceedings of the 2025 Conference on Empirical Methods in Natural Language Processing: Industry Track , pages 2106-2121. Association for Computational Linguistics, November 2025.
- [32] Ziwei Ji, Nayeon Lee, Rita Frieske, Tiezheng Yu, Dan Su, Yan Xu, Etsuko Ishii, Yejin Bang, Delong Chen, Wenliang Dai, Ho Shu Chan, Andrea Madotto, and Pascale Fung. Survey of hallucination in natural language generation. ACM Computing Surveys , 2023.
- [33] Jürgen Kletti. Manufacturing Execution Systems (MES) . Springer, 2007.
- [34] Karl E. Kurbel. Enterprise Resource Planning and Supply Chain Management . Springer, 2013.
- [35] Fangyu Lei, Jixuan Chen, Yuxiao Ye, Ruisheng Cao, Dong Chan Shin, Hongjin Su, Zhaoqi Suo, Hongwei Gao, Wenpeng Hu, Pengcheng Yin, Sida I. Wang, and Victor Zhong. Spider 2.0: Evaluating language models on real-world enterprise text-to-sql workflows. arXiv preprint arXiv:2411.07763 , 2024.
- [36] Patrick Lewis, Ethan Perez, Aleksandra Piktus, Fabio Petroni, Vladimir Karpukhin, Naman Goyal, Heinrich Küttler, Mike Lewis, Wen tau Yih, Tim Rocktäschel, Sebastian Riedel, and Douwe Kiela. Retrieval-augmented generation for knowledge-intensive nlp tasks. In Proceedings of the 34th Conference on Neural Information Processing Systems (NeurIPS) , 2020.

- [37] Yunyao Li, Dragomir Radev, and Davood Rafiei. Natural Language Interfaces to Databases . Springer Nature, November 2023.
- [38] Bao Liu and Guilin Qi. Llm-cg: Large language model-enhanced constraint graph for distantly supervised relation extraction. Neurocomputing , 655:131426, 2025.
- [39] Runxuan Liu, Bei Luo, Jiaqi Li, Baoxin Wang, Ming Liu, Dayong Wu, Shijin Wang, and Bing Qin. Ontology-guided reverse thinking makes large language models stronger on knowledge graph question answering. In Proceedings of the 63rd Annual Meeting of the Association for Computational Linguistics (ACL) , 2025.
- [40] LlamaIndex. Text-to-sql guide (query engine + retriever). https://developers. llamaindex.ai/python/examples/index\_structs/struct\_indices/ sqlindexdemo/ , 2026. Accessed: 2026-04-09.
- [41] Chengda Lu, Xiaoyu Fan, Yu Huang, Rongwu Xu, Jijie Li, and Wei Xu. Does chain-of-thought reasoning really reduce harmfulness from jailbreaking? In Wanxiang Che, Joyce Nabende, Ekaterina Shutova, and Mohammad Taher Pilehvar, editors, Findings of the Association for Computational Linguistics: ACL 2025 , pages 6523-6546, Vienna, Austria, July 2025. Association for Computational Linguistics.
- [42] Haoran Luo, Haihong E, Guanting Chen, Yandan Zheng, Xiaobao Wu, Yikai Guo, Qika Lin, Yu Feng, Zemin Kuang, Meina Song, Yifan Zhu, and Luu Anh Tuan. HyperGraphRAG: Retrievalaugmented generation via hypergraph-structured knowledge representation. In Advances in Neural Information Processing Systems (NeurIPS) , 2025.
- [43] Chuangtao Ma, Yongrui Chen, Tianxing Wu, Arijit Khan, and Haofen Wang. Large language models meet knowledge graphs for question answering: Synthesis and opportunities. In Proceedings of the 2025 Conference on Empirical Methods in Natural Language Processing (EMNLP) , 2025.
- [44] Akash V. Maharaj, David Arbour, Daniel Lee, Uttaran Bhattacharya, Anup Rao, Austin Zane, Avi Feller, Kun Qian, and Yunyao Li. Evaluation and incident prevention in an enterprise ai assistant. In Innovative Applications of Artificial Intelligence (IAAI) , 2025.
- [45] Costas Mavromatis and George Karypis. GNN-RAG: Graph neural retrieval for efficient large language model reasoning on knowledge graphs. In Wanxiang Che, Joyce Nabende, Ekaterina Shutova, and Mohammad Taher Pilehvar, editors, Findings of the Association for Computational Linguistics: ACL 2025 , pages 16682-16699, Vienna, Austria, July 2025. Association for Computational Linguistics.
- [46] Rakesh R. Menon, Kun Qian, Liqun Chen, Ishika Joshi, Daniel Pandyan, Jordyn Harrison, Shashank Srivastava, and Yunyao Li. Fisql: Enhancing text-to-sql systems with rich interactive feedback. In Proceedings of the 28th International Conference on Extending Database Technology (EDBT) , 2025.
- [47] Abdulsobur Oyewale and Tommaso Soru. Llm-driven ontology construction for enterprise knowledge graphs, 2026.
- [48] Charles Packer, Sarah Wooders, Kevin Lin, Vivian Fang, Shishir G. Patil, Ion Stoica, and Joseph E. Gonzalez. Memgpt: Towards llms as operating systems. arXiv preprint arXiv:2310.08560 , 2023.

- [49] Boci Peng, Yun Zhu, Yongchao Liu, Xiaohe Bo, Haizhou Shi, Chuntao Hong, Yan Zhang, and Siliang Tang. Graph retrieval-augmented generation: A survey. ACM Trans. Inf. Syst. , 44(2), December 2025.
- [50] Disha Purohit, Yashrajsinh Chudasama, and Maria-Esther Vidal. Capturing symbolic knowledge of constraints and incompleteness to guide inductive learning in neuro-symbolic knowledge graph completion. In Proceedings of the 13th International Conference on Knowledge Capture (K-CAP) , 2025.
- [51] Abhishek Rath. Challenges in integrating ai with existing enterprise data governance structures. Enterprise AI Journal , 2025.
- [52] Abhishek Rath. Agent drift: Quantifying behavioral degradation in multi-agent llm systems over extended interactions. arXiv preprint arXiv:2601.04170 , 2026.
- [53] Timo Schick, Jane Dwivedi-Yu, Roberto Dessì, Roberta Raileanu, Maria Lomeli, Luke Zettlemoyer, Nicola Cancedda, and Thomas Scialom. Toolformer: Language models can teach themselves to use tools. In Advances in Neural Information Processing Systems (NeurIPS) , volume 36, 2024.
- [54] Christian Schuh, Michael F. Strohmer, Stephen Easton, Michael Hales, and Alenka Triplat. Supplier Relationship Management: How to Maximize Vendor Value and Opportunity . Apress, 2014.
- [55] Nabeel Seedat and Mihaela van der Schaar. Matchmaker: Schema matching with self-improving compositional llm programs. In The Thirteenth International Conference on Learning Representations (ICLR) , 2025.
- [56] Kartik Sharma, Peeyush Kumar, and Yunqing Li. OG-RAG: Ontology-grounded retrieval-augmented generation for large language models. In Christos Christodoulopoulos, Tanmoy Chakraborty, Carolyn Rose, and Violet Peng, editors, Proceedings of the 2025 Conference on Empirical Methods in Natural Language Processing , pages 32962-32981, Suzhou, China, November 2025. Association for Computational Linguistics.
- [57] Xiaoli Shen, Guilin Qi, Peng Zhang, Dingsheng Yang, Yang Yang, Muyan Yang, Li Fangyanhao, HaoWen Guo, HuiKang Hu, Jiazhen Kang, Yan Wang, Yuchen Lu, Zheyu Xin, and Jun Wang. Autoqg: An automated framework for evidence-traceable question generation via ontology-guided knowledge graph construction. Preprint (Submitted to ICLR 2026) , 2026.
- [58] Rajpreet Singh, Novak Boškov, Lawrence Drabeck, Aditya Gudal, and Manzoor A. Khan. CSR-RAG: An efficient retrieval system for text-to-sql on the enterprise scale. arXiv preprint arXiv:2601.06564 , 2026.
- [59] Yiming Tan, Dehai Min, Yu Li, Wenbo Li, Nan Hu, Yongrui Chen, and Guilin Qi. Can chatgpt replace traditional kbqa models? an in-depth analysis of the question answering performance of the gpt llm family. In International Semantic Web Conference (ISWC) , 2023.
- [60] Guanzhi Wang, Yuqi Xie, Yunfan Jiang, Ajay Mandlekar, Chaowei Xiao, Yuke Zhu, Linxi Fan, and Anima Anandkumar. Voyager: An open-ended embodied agent with large language models. In Advances in Neural Information Processing Systems (NeurIPS) , volume 36, 2023.
- [61] Yu Wang and Xi Chen. Mirix: Multi-agent memory system for llm-based agents. arXiv preprint arXiv:2507.07957 , 2025.

- [62] Gregor Herbert Wegener. Sort-ai: Agentic system stability in large-scale ai systems-structural causes of cost, instability, and non-determinism in multi-agent workflows. Preprints.org , 2026.
- [63] Guohui Xiao, Linfang Ding, Benjamin Cogrel, and Diego Calvanese. Virtual knowledge graphs: An overview of systems and use cases. Data Intelligence , 1(3):201-223, 2019.
- [64] Shuaiyu Xie, Hanbin He, Jian Wang, and Bing Li. Root cause analysis for microservice systems via cascaded conditional learning with hypergraphs, 2025.
- [65] Yuzhang Xie, Xu Han, Ran Xu, Xiao Hu, Jiaying Lu, and Carl Yang. Hypkg: Hypergraph-based knowledge graph contextualization for precision healthcare. arXiv preprint arXiv:2507.19726 , 2025.
- [66] Rongwu Xu, Brian Lin, Shujian Yang, Tianqi Zhang, Weiyan Shi, Tianwei Zhang, Zhixuan Fang, Wei Xu, and Han Qiu. The earth is flat because...: Investigating LLMs' belief towards misinformation via persuasive conversation. In Lun-Wei Ku, Andre Martins, and Vivek Srikumar, editors, Proceedings of the 62nd Annual Meeting of the Association for Computational Linguistics (Volume 1: Long Papers) , pages 16259-16303, Bangkok, Thailand, August 2024. Association for Computational Linguistics.
- [67] Rongwu Xu, Zehan Qi, Zhijiang Guo, Cunxiang Wang, Hongru Wang, Yue Zhang, and Wei Xu. Knowledge conflicts for LLMs: A survey. In Yaser Al-Onaizan, Mohit Bansal, and Yun-Nung Chen, editors, Proceedings of the 2024 Conference on Empirical Methods in Natural Language Processing , pages 8541-8565, Miami, Florida, USA, November 2024. Association for Computational Linguistics.
- [68] Yao Xu, Shizhu He, Jiabei Chen, Zihao Wang, Yangqiu Song, Hanghang Tong, Kang Liu, and Jun Zhao. Generate-on-graph: Treat llm as both agent and kg in incomplete knowledge graph question answering. In Proceedings of the 2024 Conference on Empirical Methods in Natural Language Processing (EMNLP) , 2024.
- [69] Chang Yang, Chuang Zhou, Yilin Xiao, Su Dong, Luyao Zhuang, Yujing Zhang, Zhu Wang, Zijin Hong, Zheng Yuan, Zhishang Xiang, Shengyuan Chen, Huachi Zhou, Qinggang Zhang, Ninghao Liu, Jinsong Su, Xinrun Wang, Yi Chang, and Xiao Huang. Graph-based agent memory: Taxonomy, techniques, and applications. arXiv preprint arXiv:2602.05665 , 2026.
- [70] Shunyu Yao, Jeffrey Zhao, Dian Yu, Nan Du, Izhak Shafran, Karthik Narasimhan, and Yuan Cao. ReAct: Synergizing reasoning and acting in language models. In International Conference on Learning Representations (ICLR) , 2023.
- [71] Tao Yu, Rui Zhang, Kai Yang, Michihiro Yasunaga, Dongxu Wang, Zifan Li, James Ma, Irene Li, Qingning Yao, Shanelle Roman, Zilin Zhang, and Dragomir Radev. Spider: A large-scale human-labeled dataset for complex and cross-domain semantic parsing and text-to-SQL task. In Proceedings of the 2018 Conference on Empirical Methods in Natural Language Processing , pages 3911-3921, Brussels, Belgium, 2018. Association for Computational Linguistics.
- [72] Xiaohan Yu, Pu Jian, and Chong Chen. Tablerag: A retrieval augmented generation framework for heterogeneous document reasoning. In Proceedings of the 2025 Conference on Empirical Methods in Natural Language Processing , pages 14063-14082, Suzhou, China, 2025. Association for Computational Linguistics. Also available as arXiv:2506.10380.
- [73] Matei Zaharia, Omar Khattab, Keshav Santhanam, Omar Paranjape, et al. The shift from models to compound AI systems. Berkeley Artificial Intelligence Research (BAIR) , 2024.

- [74] Xiangjun Zai, Xingyu Tan, Xiaoyang Wang, Qing Liu, Xiwei Xu, and Wenjie Zhang. PRoH: Dynamic planning and reasoning over knowledge hypergraphs for retrieval-augmented generation. In Proceedings of the ACM Web Conference (WWW) , 2026.
- [75] Bohan Zhai, Canwen Xu, Yuxiong He, and Zhewei Yao. Optimizing reasoning for text-to-sql with execution feedback. In Findings of the Association for Computational Linguistics (ACL) , 2025.
- [76] Jing Zhang, Xiaokang Zhang, Jifan Yu, Jian Tang, Jie Tang, Cuiping Li, and Hong Chen. Subgraph retrieval enhanced model for multi-hop knowledge base question answering. In Proceedings of the 60th Annual Meeting of the Association for Computational Linguistics (ACL) , 2022.
- [77] Siyun Zhao, Yuqing Yang, Zilong Wang, Zhiyuan He, Luna K. Qiu, and Lili Qiu. Retrieval augmented generation (rag) and beyond: A comprehensive survey on how to make your llms use external data more wisely, 2024.
- [78] Shuhan Zheng, Keita Mizushina, and Ken Naono. Empowering supply chain risk monitoring with ontology-guided knowledge graph extraction by llms. In International Workshop on the Semantic Web , 2025.
- [79] Wanjun Zhong, Lianghong Guo, Qiqi Gao, He Ye, and Yanlin Wang. Memorybank: Enhancing large language models with long-term memory. Proceedings of the AAAI Conference on Artificial Intelligence , 2023.
- [80] Rui Zhu, Cogan Shimizu, Shirly Stephen, Colby K. Fisher, Thomas Thelen, Kitty Currier, Krzysztof Janowicz, Pascal Hitzler, Mark Schildhauer, Wenwen Li, Dean Rehberger, Adrita Barua, Antrea Christou, Ling Cai, Abhilekha Dalal, Anthony D'Onofrio, Andrew Eells, Mitchell Faulk, Zilong Liu, Gengchen Mai, Mohammad Saeid Mahdavinejad, Bryce Mecum, Sanaz Saki Norouzi, and Meilin Shi. The knowwheregraph: A large-scale geo-knowledge graph for interdisciplinary knowledge discovery and geo-enrichment. arXiv preprint arXiv:2502.13874 , 2025.
- [81] Jiaru Zou, Dongqi Fu, Sirui Chen, Xinrui He, Zihao Li, Yada Zhu, Jiawei Han, and Jingrui He. Rag over tables: Hierarchical memory index, multi-stage retrieval, and benchmarking. 2025.

## A Extended Problem Formulation Details

## A.1 Semantic Interoperability Issues in Enterprise Data Integration

In real-world production environments, achieving semantic interoperability is severely bottlenecked by a myriad of implicit data and process anomalies. We identify the following common issues that plague enterprise data integration:

- Semantic Ambiguity in Schemas : Table and field names often lack clear semantics (e.g., tables using German abbreviations or Pinyin acronyms).
- Field-Level Equivocation : Similar field names obscure distinct business meanings (e.g., multiple 'effective dates' coexisting without explicit definitions).
- Opaque Statistical Metrics : Calculation logic and aggregation rules are frequently undocumented, relying heavily on tribal knowledge among employees.

- Cross-Table Identifier Inconsistency : The same business entity may be labeled differently across tables (e.g., a contract identifier named contract\_no in one table and contract\_code in another).
- Format and Encoding Discrepancies : Cross-system identifiers often suffer from inconsistent formatting, such as material codes being zero-padded in one system but not in another.

## B Extended Method Details

## B.1 Graph Layer Node Attributes

Each node v ∈ V in the Graph Layer possesses core attributes designed to secure channels and align semantics during LLM planning:

- name &amp; id : Globally unique identifiers (e.g., 'table:wms\_inventory').
- description : Natural language descriptions capturing the node's precise business meaning.
- source : The physical topological origin of the data source (e.g., WMS).
- schema : Structured schema definitions (e.g., simplified SQL DDL). To optimize the LLM's token context window, we utilize field cleaning and high-level semantic annotations rather than enumerative full-field outputs.
- query\_template : Parameterized query templates (e.g., SQL queries or REST API payloads) for securely retrieving instance data.

## B.2 Hyperedge Attributes

Key attributes of a hyperedge e ∈ H include:

- title : A unique identifier and natural language specification of the hyperedge.
- description : A summary of the hyperedge's task and core logic.
- nodes V e : The subset of graph nodes defining the required data scope.
- semantic\_details : The core cognitive carrier binding the business constraints to the graph nodes. It specifies business conditions, multi-step reasoning, domain validity checks, anomaly triggers, and canonical tool-use procedures.
- related\_hyperedges : References to other hyperedges expressing dependencies, reasoning fallbacks, or next-step actions.
- attachments : Optional executable scripts (e.g., Python) for automating analysis steps.

## C Extended Experimental Details

## C.1 Dataset Profiles and Reasoning Span Definitions

For the OF-General dataset, we operationalize reasoning span by the number of distinct data nodes involved in a question ( k = | nodes | ). The specific buckets and corresponding examples are defined as follows:

- k =1 : Single-node retrieval and reasoning. Examples: (1) Top 10 customers by contract volume; (2) A breakdown of the most frequently used contract types.
- k =2 and k =3 : Require retrieval and reasoning across 2-3 nodes. Examples: (1) Which suppliers are associated with order O123? (2) Count of contracts with over-receipt (site receipt &gt; delivery request) for direct-ship physical items.
- k&gt; 3 : Require retrieval, multi-hop reasoning, and calculation involving more than 3 nodes. Examples: (1) Order O123 has generated a delivery request but has not yet left the warehouse; what are the possible causes? (2) What are the top 5 pending approvers with the highest number of blocked lines for Delivery Requests under review, including their individual blocked line counts and the overall total?

## C.2 Two-Stage Verification Protocol and Label Spaces

To ensure rigorous validation of the Accuracy metric, we employ a two-stage verification protocol: an initial assessment using an LLM-as-judge approach [26], followed by a final verification conducted by human experts. The judged output includes both the final conclusion and the organized supporting evidence gathered from tool outputs, meaning correctness is evaluated on the grounded answer package rather than on a free-form claim alone.

For OF-General, we use a three-way final-correctness label space ( correct / partially correct / incorrect ) to account for acceptable definition-level differences (e.g., alternative aggregation windows or statistical standards). For OF-BRCA, the intermediate 'partially correct' outcome is negligible in our runs as a contract will basically have only one root cause of unfulfillment, so accuracy is effectively binary ( correct vs. incorrect conclusions).

## C.3 Internal Baseline Implementation Context

To rigorously ablate the contribution of the Stratified Hypergraph Ontology, we designed internal baselines that share the identical agentic loop, tool set, and turn budget as HEAR, but are stripped of hyperedge semantics:

- Table-RAG : To provide the LLM with relevant table context without triggering the index explosion typical of raw content indexing (e.g., T-RAG [81]), we implement a strong schema-aware TableRAG. Fusing these concepts with LlamaIndex [40], we build an index over schema metadata (tables, columns, types, and short descriptions) across heterogeneous databases. Retrieval yields lightweight schema pointers via a mapping function f rather than raw table content, forcing the LLM to generate and execute SQL absent structural guidance.
- Table list : The initial reasoning context provides a flat list of table identifiers and short interface descriptions. This seemingly naive injection of semantically processed names yields a highly condensed information advantage, forming a remarkably strong baseline against which the value of explicit hypergraph topology can be measured.

## C.4 OF-BRCA Error Modes and Comparative Analysis

The evaluation on the OF-BRCA dataset reveals distinct error patterns across different retrieval settings and LLM backbones, underscoring the challenges of multi-hop enterprise reasoning.

Baseline and Ablation Analysis. The critical role of the complete hypergraph structure is validated by comparing HEAR against various baselines and ablations. Without the topological guidance provided by hyperedges, the error rates increase dramatically. For the GPT-5 backbone, the number of errors rises from 5 in the HEAR (Complete) setting to 46 for Table-RAG and 86 for CSR-RAG. Similar performance degradation is observed for Qwen-3.5-27B, where errors increase from 7 to 52 and 86 in the respective baselines.

The distribution of error modes shifts significantly in these baselines. Specifically, failures related to missing Site Goods Receipts and general Order Fulfillment state confusion experience a massive surge, frequently exceeding 40-70 cases per baseline. This suggests that without hyperedge-based context, models struggle to maintain the global logic of the fulfillment chain, leading to hallucinations regarding the current state of a business object when faced with disconnected tabular evidence. For example, in one Table-RAG case, the ground-truth blockage was an upstream delivery application that had been submitted but not yet made effective, so the contract had not entered the outbound-delivery stage; the baseline instead concluded that outbound delivery had already completed and that the remaining issue was a missing site receipt. Furthermore, tracking Direct Shipment pathways becomes highly error-prone without structural guidance, jumping to over 30 errors. Ablating the hypergraph to only the declarative variant, HEAR (Declarative), also results in 30-50 errors. This proves that declarative relational links alone are insufficient for tracking long-span business processes that strictly mandate cross-system verification among BPM, ERP, SRM, and WMS.

Main OF-BRCA Errors by LLM Backbone. Under the full HEAR configuration, denoted as HEAR (Complete), top-tier models demonstrate exceptional precision. Specifically, GPT-5, DeepSeek-3.2, and Qwen-3.5-27B only encounter 5, 6, and 7 failure cases, respectively. In contrast, the performance of the Qwen-Next-80B model drops significantly with 53 recorded errors under the same setting.

The residual failures for high-performing models are not random but tightly clustered around a small subset of high-complexity business instances. Among the incorrect cases, the most frequent error modes involve missing Delivery Requests, complex Direct Shipment chains involving Vendor ASNs, and nuanced workflow state discrepancies. Qualitatively, these failures typically arise from a misinterpretation of specific blockage stages-for example, mistaking a missing upstream Delivery Request in the BPM system for a downstream synchronization failure between WMS (Outbound Delivery) and ERP (Financial Outbound). The high degree of overlap among different backbones on these residual cases (with several instances being marked incorrect by over 10 different systems) indicates that the difficulty lies in the intrinsic complexity of specific enterprise fulfillment traces across heterogeneous systems, rather than model-specific weaknesses.

## C.5 Additional Qwen-3.5-27B Baseline-Ablation Results

The main text reports the GPT-5 baseline-ablation results. For completeness, we place the corresponding Qwen-3.5-27B versions here. These results verify that the advantages of hyperedge-structured reasoning are not tied to a proprietary backbone: even under a smaller open-weight model, HEAR (Complete) consistently produces the strongest accuracy, while ablations and retrieval-only baselines either lose correctness or fall back to less disciplined exploration.

On OF-BRCA (Table 3), HEAR (Complete) reaches 92.7% accuracy. Notably, HEAR (Declarative) establishes a robust foundation by achieving 73.2% accuracy, which clearly exceeds Table-RAG (46.4%), Table list (54.2%), and CSR-RAG (9.5%). Building upon this declarative base, adding procedural hyperedges yields a further 19.5-point accuracy jump while delivering a distinct efficiency gain: average tool turns drop from 11.8 to 8.4, and token usage decreases from 105.9K to 57.3K. Although Table list is slightly cheaper than HEAR (Complete) in tokens (48.9K vs. 57.3K), its accuracy remains far lower, confirming that concise schema exposure cannot replace procedural guidance for blockage diagnosis.

Table 3: Qwen-3.5-27B baseline ablation on OF-BRCA.

| Category          | Method     | Hyperedges   |   Accuracy (%) | Tool turns       | Tokens ( × 10 3 )   |
|-------------------|------------|--------------|----------------|------------------|---------------------|
|                   | HEAR       | Complete     |           92.7 | 8.4 ± 0.3 11.8 ± | 57.3 ± 2.4          |
|                   | HEAR       | Declarative  |           73.2 | 0.5              | 105.9 ± 9.4         |
| Internal Baseline | Table-RAG  | No           |           46.4 | 13.6 ± 0.6       | 122.6 ± 10.4        |
| Internal Baseline | Table list | No           |           54.2 | 8.8 ± 0.4        | 48.9 ± 4.1          |
| External Baseline | CSR-RAG    | -            |            9.5 | 3.0 ± 0.0        | 13.6 ± 0.8          |

Table 4: Qwen-3.5-27B baseline ablation on OF-General.

| Category          | Method     | Hyperedges   | Accuracy (%)   | Accuracy (%)   | Tool turns   | Tokens ( × 10 3 )   |
|-------------------|------------|--------------|----------------|----------------|--------------|---------------------|
|                   |            |              | Correct        | +Partial       |              |                     |
|                   | HEAR       | Complete     | 34.0           | 83.0           | 9.6 ± 0.5    | 83.5 ± 8.2          |
|                   |            | Declarative  | 28.9           | 79.2           | 9.2 ± 0.5    | 79.3 ± 8.8          |
| Internal Baseline | Table-RAG  | No           | 25.2           | 79.2           | 12.7 ± 0.9   | 154.3 ± 20.2        |
|                   | Table list | No           | 22.6           | 74.2           | 8.9 ± 0.5    | 58.8 ± 7.2          |
| External Baseline | CSR-RAG    | -            | 3.2            | 57.0           | -            | 9.7 ± 0.3           |

On OF-General (Table 4), the same trend appears under the stricter fully Correct metric. HEAR (Complete) obtains 34.0% fully correct accuracy and 83.0% Correct+Partial accuracy, outperforming HEAR (Declarative) (28.9% / 79.2%), Table-RAG (25.2% / 79.2%), Table list (22.6% / 74.2%), and CSR-RAG (3.2% / 57.0%). The cost profile again reflects the nature of Path B exploration, but here tool turns and tokens should be read together: HEAR (Complete) does not uniformly minimize tool calls relative to every ablated setting, but it accepts a slightly higher overall computation-especially in token usage relative to external baselines (83.5K vs. 79.3K for HEAR (Declarative) and 58.8K for Table list)-because it continues assembling and validating multi-node evidence instead of stopping at shallow schema matches. In other words, the additional cost on OF-General is not wasted exploration, but a task-appropriate investment in better-supported answer construction, which is precisely what yields the best strict accuracy.

The by-complexity breakdown (Figure 6) further clarifies where the complete hyperedge structure matters. For simple k =1 questions, HEAR (Complete) and Table-RAG both reach 91.5%, with HEAR (Declarative) close behind at 90.2%. As the reasoning span grows, however, the benefit of complete hyperedges becomes more pronounced: at k =3 , HEAR (Complete) reaches 63.2%, compared with 57.9% for Table-RAG and 21.1% for CSR-RAG; at k&gt; 3 , HEAR (Complete) rises to 79.2%, while HEAR (Declarative), Table-RAG, and Table list all remain at 58.3%, and CSR-RAG falls to 33.3%. The cost curves show the same scaling effect. Both tool turns and token usage rise with reasoning span, with HEAR (Complete) increasing from 7.11 turns at k =1 to 17.11 at k =3 , while tokens grow from 43.4K to 216.9K before settling at 121.5K for k&gt; 3 . Thus, Qwen-3.5-27B reproduces the central conclusion of the main experiments: hyperedge-guided exploration is most valuable precisely when the question requires long-span, multi-system evidence assembly.

Taken together, the Qwen-3.5-27B results reproduce the same high-level pattern as GPT-5, but with a clearer contrast between two regimes. On OF-BRCA, procedural hyperedges improve both accuracy and efficiency, reducing both tool turns and token cost relative to the declarative ablation. On OF-General, they improve strict correctness at higher computational cost, with the extra burden appearing primarily in tokens and, for harder questions, also in longer tool-use chains. This distinction strengthens the overall claim of the paper: the value of hyperedges is not just that they increase accuracy, but that they allocate computation in a task-appropriate way.

Figure 6: Qwen-3.5-27B baseline ablation on OF-General by node-count bucket .

<!-- image -->

## C.6 Additional Backbone-Specific Findings

Beyond the headline accuracy-cost comparisons in the main text, our traces suggest two recurring backbone-specific behavioral patterns.

Why DeepSeek-3.2 uses many more tool turns. On both OF-BRCA and OF-General, DeepSeek-3.2 is noticeably more tool-intensive than the other strong backbones. In our traces, this is not mainly because it explores dramatically broader search spaces, but because it more often re-checks intermediate artifacts before committing to the next step. Typical behaviors include reopening generated files, re-reading SQL outputs, and issuing additional verification-oriented tool calls after a seemingly sufficient intermediate result has already been obtained. This verification-heavy style can sometimes slightly improve final correctness, but it also increases latency and token consumption substantially. In other words, DeepSeek3.2 behaves more conservatively inside the HEAR loop: it spends more turns validating previous steps rather than advancing with a shorter execution chain.

Why Qwen-Next-80B does not outperform Qwen-3.5-27B. Our results show that a larger nominal model does not automatically yield better tool-grounded performance. Although Qwen-Next-80B has a larger advertised scale, it underperforms Qwen-3.5-27B on both datasets. A plausible explanation is architectural: the Qwen 'A3B' naming convention is used for sparse MoE models with only a small subset of parameters activated per token, whereas the open-weight Qwen2.5 language models in the 27B/32B class are dense models according to the official Qwen release materials. Under HEAR, this means the comparison is not simply 'bigger vs. smaller,' but sparse-MoE scale versus a smaller dense backbone. In our runs, Qwen-3.5-27B more often follows concise and coherent action chains, whereas Qwen-Next-80B more frequently loses precision during tool-grounded execution. This is consistent with the broader observation that sparse MoE scale does not necessarily translate into stronger performance in tightly constrained, multi-step tool-use settings.