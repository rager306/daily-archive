## Co-Designing Graph-based Approximate Nearest Neighbor Search at Billion Scale for Processing-in-Memory

Sitian Chen1, Yusen Li2, Yao Chen3, Minwen Deng4, Jintao Meng5, Amelie Chi Zhou1

1Hong Kong Baptist University, 2Nankai University, 3Huazhong University of Science and Technology 4Tencent, 5Shenzhen Institutes of Advanced Technology

# arXiv:2605.25522v1[cs.AR]25May2026

Abstract—Approximate Nearest Neighbor Search (ANNS) is a core primitive in modern AI systems, and graph-based methods currently offer the best accuracy–efficiency trade-off at scale. The workload is fundamentally memory-bound: graph traversal produces frequent, irregular memory accesses that cap CPU throughput at main-memory bandwidth, while GPUs lack the high-bandwidth memory capacity to host billion-scale indexes. Processing-in-Memory (PIM) is a natural candidate, as placing computation next to data unlocks the abundant internal bandwidth that such bandwidth-starved workloads demand. Porting graph-based ANNS to PIM, however, exposes several architectural mismatches: each processing unit has only a small local memory, inter-unit communication is costly, host coordination adds overhead, and in-memory compute units are relatively weak—limitations that have forced prior PIM-based ANNS designs to fall back on cluster-based indexing, whose recall ceiling is far below that of graph methods. This paper presents an algorithm–architecture co-design that overcomes these obstacles through three components: a compacted index layout that shrinks the PIM-resident memory footprint by 14.5×; an asynchronous pipelined scheduler that keeps the host-to-PIM interconnect saturated; and a multiplication-free distance kernel that loses under 0.08% recall. Across three billion-scale benchmarks, the proposed design achieves up to 20× and 17.1× higher throughput than CPU and GPU baselines, respectively, outperforms prior PIM accelerators by 129× in the high-recall regime, and scales gracefully across multi-node deployments and emerging PIM architectures.

I. INTRODUCTION

Approximate Nearest Neighbor Search (ANNS) is a foundational primitive for large-scale AI services, including retrievalaugmented generation (RAG) [7], [11], [36], and recommendation systems [53], [55]. Given a high-dimensional query vector, ANNS retrieves the top-k most similar vectors from a massive database while trading a small amount of accuracy for orders-of-magnitude efficiency gains. Among the major ANNS families (hash-based [18], [62], cluster-based [4], [8], [25], [42], graph-based [5], [15], [19], [54], [56], [63]), graph-based methods have received extensive attention in both academia and industry due to their superior expected search accuracy and efficiency trade-offs.

Limitation of Existing Architectures. Despite algorithmic advances, graph-based ANNS on conventional architectures are fundamentally bottlenecked by the “memory wall.” To quantify this effect, we perform a roofline analysis of representative graph-based ANNS methods using the SIFT dataset and a CPU platform (2× Intel Xeon Gold 6330). As

Corresponding author: Amelie Chi Zhou.

Performance(GFLOPS)

PIM-HBM (Samsung) A100 UPMEM AiM (SK Hynix) CPU

105

PIMCQG

103

NDSearch

101

HNSWlib SymphonyQG

10 1

10 4 10 3 10 2 10 1 100 101 102 Arithmetic Intensity (FLOP/Byte)

Fig. 1: Roofline Analysis of graph-based ANNS on SIFT.

shown in Figure 1, the results indicate that these state-of-theart methods, including HNSWlib [35], NDSearch [56], and SymphonyQG [19], lie firmly in the memory-bound region, struggling to effectively utilize available compute resources. CPUs are constrained by limited memory bandwidth (e.g., 375 GB/s on CPUs), while GPUs, despite offering higher bandwidth (e.g., 2.0 TB/s on an A100), are severely limited by global memory capacity—a full SymphonyQG index for billion-scale datasets exceeds 1.25 TB, far beyond what GPU HBM can accommodate.

Opportunities of PIM. To bridge this gap, Processingin-Memory (PIM) has emerged as a promising architecture. Modern PIM systems (e.g., UPMEM PIM [17], Samsung PIM-HBM [32], and SK Hynix AiM [33]) embed lightweight processing units (PUs) directly inside memory banks. By colocating computation with data, PIM enables massive internal memory bandwidth (e.g., 2 TB/s aggregate on UPMEM) and minimizes costly data movement across the memory hierarchy, making it particularly attractive for memory-bound, irregular workloads like graph traversal. Unlike custom accelerator designs [30], [50], [58], [60], commodity PIM modules ship as standard DIMM form-factor devices, enabling practical deployment without system redesign. However, existing PIMbased ANNS accelerators [4], [8], [57] predominantly rely on simpler, cluster-based algorithms (e.g., IVF-PQ), which inherently suffer from lower search quality and hit a capability ceiling at low recall levels (e.g., ∼61% on SIFT1B).

Challenges. While PIM’s massive internal bandwidth makes it appear well suited for memory-bound workloads, directly mapping graph-based ANNS onto PIM exposes a fundamental algorithm–architecture mismatch that manifests as four tightly coupled challenges (detailed analysis in Section II-C):

1 Extreme Local Memory Capacity: PU-private memory is severely limited (e.g., 64 MB per UPMEM DPU), yet billion-scale graph indexes exceed 1.25 TB, forcing aggressive partitioning. 2 Inter-PU Communication: Fine-grained partitioning causes frequent cross-PU traversals over external bandwidth paths that are >10× slower than internal band-

![image 1](<original_images/imageFile1.png>)

width. 3 Coordination Overhead and Load Imbalance: Existing batch-synchronous [3], [4], [8], [9] and per-query [57] scheduling strategies either waste PU cycles on synchronization barriers or fragment communication bandwidth. 4

###### …

Query

###### Current Node: 4

|1|
|---|


3

Search Beam set:

4

Search Neighbor

2

|4|
|---|


Result Beam set:

5 1 3

###### Current Node: 5

Search Beam set:

|4|
|---|


1 3

Search Neighbor

Entry Point

Restricted PU Compute: PIM cores feature weak arithmetic capabilities and in some cases lack hardware multipliers (e.g., UPMEM), resulting in disproportionate cost for the remaining floating-point operations in quantization-based distance kernels. Overcoming these coupled challenges requires a holistic algorithm–hardware co-design.

0

|5|
|---|


Result Beam set:

###### Current Node: 0

|V0 V1 V2 V3 V4 V5<br><br>Neighbors<br><br>→ V1 → V3<br><br>x → V5 x Distance<br><br>Calculation<br><br>D1 2 D3 3 D5 1|
|---|


|5|
|---|


1 3

Search Beam set:

Search Neighbor

entry

|0|
|---|


Result Beam set:

Fig. 2: Query processing with greedy beam search. II. BACKGROUND AND MOTIVATION A. Graph-based ANNS Algorithms

Innovations. To overcome these architectural constraints, we propose PIMCQG, a holistic algorithm–hardware codesign framework that enables high-performance, high-recall graph-based ANNS on commodity PIM platforms. Rather than applying isolated software patches, PIMCQG realigns the entire system stack—data layout, scheduling, and computationwith PIM architectural constraints through three synergistic innovations: 1 PIM-Friendly Compact Index: We eliminate redundant per-edge quantization metadata via IVF-style clustering and offload exact similarity reranking to the host CPU. This reduces the PIM-resident index footprint by up to 14.5× (e.g., from 2,385 GB to 164 GB on SSN1B dataset [12]), dramatically easing capacity limits (Challenge 1) and crossPU communication pressure (Challenge 2) (Section IV-A).

Graph-based ANNS approaches model the dataset as a proximity graph, enabling a greedy beam search [14], [15], [24], [43] to navigate the structure. Starting from an entry point, the search maintains a candidate set (the beam) of the current nb closest vectors and iteratively expands promising nodes by exploring their neighbors until convergence. Figure 2 illustrates this process for nb = 3 and k=1. The search begins at the entry node (node 0), fetches its neighbors, computes their distances to the query, and updates both the search and result beams accordingly. Each iteration therefore consists of three tightly coupled actions: (1) pointer-based graph traversal to access adjacency lists and candidate vectors, (2) distance computation and ranking, and (3) state updates to the beam and visited set. This decomposition highlights a fundamental performance characteristic of graph-based ANNS: its efficiency depends on both irregular memory access from graph traversal and arithmetic cost from distance evaluation.

2 Asynchronous Pipelined Scheduling: We decouple hostside query dispatch and post-processing from in-PIM search through dynamic mini-batching and FIFO-based asynchronous execution. By overlapping communication, search, and reranking, this design mitigates coordination overhead and load imbalance (Challenge 3) while fully utilizing host–PIM bandwidth (Section IV-B). 3 Multiplication-Free Distance Computation: On observing that the quantization-error scaling factor cos(θ) is empirically stable within each IVF cluster, we replace the expensive per-node floating-point multiplication with a cluster-wide constant approximated via bitwise shifts and additions, achieving a fully multiplication-free inPIM distance kernel with <0.08% recall loss (Challenge 4) (Section IV-C).

Early designs were primarily bottlenecked by distance computation, motivating a large body of work on accelerating or approximating distance evaluation [1], [35], [59], [64]. Among them, RabitQ [16] stands out as a state-of-the-art (SOTA) quantization technique, offering strong theoretical error bounds while maintaining high practical accuracy. It has been widely used in real-world systems and included in the FAISS library [13], [48]. Built upon this, SymphonyQG [19] represents a SOTA graph-based ANNS design that tightly integrates RabitQ quantization with graph traversal.

Evaluation. We evaluate PIMCQG on a real-world UPMEM PIM server across three industry-standard billion-scale datasets. Our results demonstrate three key advantages over state-of-the-art baselines: 1) Performance: PIMCQG delivers up to 20× and 17.1× throughput speedups over CPU-based SymphonyQG and GPU-based GGNN, respectively. Furthermore, unlike prior PIM-based ANNS solutions that hit a capability ceiling at low recall levels, PIMCQG successfully scales to high-recall targets, achieving up to 129× speedup at comparable recall boundaries. 2) Energy Efficiency: PIMCQG provides up to 6.5× and 30.8× improvement in energy efficiency (QPS/Watt) compared to CPU and GPU baselines, demonstrating the power of PIM for large-scale ANNS. 3) Scalability: PIMCQG maintains its performance advantages when scaling to multi-node configurations and different commodity PIM platforms (Samsung PIM-HBM, SK Hynix AiM), showing its generality and practical deployment potential.

SymphonyQG consists of two tightly coupled components, offline index construction and online graph-based search, both of which rely on RabitQ [16] for quantization-aware distance estimation: (1) The index construction stage builds a proximity graph over the dataset. For each node, it encodes its neighbors using RabitQ with respect to the current node, generating quantized codes and associated scaling factors. This design enables efficient approximate distance computation during search, but also introduces additional metadata per edge (details in Figure 5(a)). (2) At query time, SymphonyQG performs greedy beam search over the graph as described in Figure 2. Specifically, it computes approximate distances using the quantized representations generated by RabitQ, hence greatly reducing the computation cost. To maintain high recall, a small set of candidates (size of nb) is periodically re-ranked using full-precision vectors. In this work, we focus

PIM DIMM

and tightly coupled to host coordination. As a result, mapping modern graph-based ANNS onto PIM surfaces four tightly coupled challenges, which reflect the core reasons why existing PIM-based ANNS solutions have been limited to simpler, cluster-based algorithms [4], [8].

C1: Small Local Memory C2: Inter-PU Communication

PU Local Memory Data Bus

| | | |
|---|---|---|
|PIM-HB|M| |
| | | |


###### Main Memory

Host Processor

AiM

- C1: Extreme Local Memory Capacity Constraints. Each PU in a PIM system is equipped with only a small private memory bank (Table I), yet modern graph-based ANNS indexes are memory intensive. Using SymphonyQG as an example, the index must store graph topology, quantization codes, and auxiliary metadata per node. For billion-scale datasets such as SIFT1B (n = 109, D = 128, graph degree R = 32), the index footprint exceeds 1.25 TB, which would require partitioning the graph across tens of thousands of PUs. This extreme partitioning is not merely an engineering inconvenience; it fundamentally reshapes the execution behavior of graph traversal and directly exacerbates the next challenge.
- C2: Graph Partitioning vs. Inter-PU Communication. The fine-grained partitioning forced by C1 splits the proximity graph across many PUs. During query processing, graph traversal frequently crosses partition boundaries. Any graph edge that crosses a boundary becomes a “remote” access that must traverse the slow external bandwidth path, which, as shown in Table I, can be over an order of magnitude slower than the internal bandwidth. As a result, the benefits of PIM’s high internal bandwidth are nullified by communication overhead unless the index layout and traversal behavior are codesigned to minimize cross-PU interactions.
- C3: Coordination Overhead and Load Imbalance. Communication overhead is further compounded by the highly data-dependent execution of graph-based ANNS. Because the proximity graph is statically partitioned across PUs, queries that traverse dense or frequently accessed partitions concentrate work on a small subset of PUs, while others remain underutilized. Existing PIM systems [3], [4], [8], [9], [37] typically rely on rigid batch-synchronous execution models to amortize communication overheads, but these global barriers force fast PUs to idle while waiting for the slowest PU to complete a batch. Alternative fine-grained dispatching strategies (e.g., PIMANN [57]) reduce idle time but fragment communication, leaving aggregate bandwidth underutilized. Effectively addressing this challenge therefore requires rethinking how queries are scheduled and how host-side coordination is overlapped with in-PIM execution.
- C4: Restricted PU Compute Capability. Even after memory, communication, and coordination issues are addressed, PIM processing units still provide orders of magnitude less compute throughput than GPUs, and in some designs lack hardware multipliers altogether (e.g. UPMEM). This constraint directly conflicts with the arithmetic patterns of graph-based ANNS, even in optimized methods like SymphonyQG, where approximate distance estimation still relies on multiplicationheavy inner products. Under such constraints, each arithmetic operation carries disproportionate cost, making it necessary to redesign the distance kernel.


PIM Chip

PIM Chip

…

UPMEM PIM

C4: Restricted Compute Capability C3: Two-level Collaboration

Fig. 3: Commodity PIM architecture.

on optimizing the query execution path, as it dominates endto-end ANNS performance, while treating index construction as an offline preprocessing step.

By aggressively reducing the cost of distance computation, modern graph-based ANNS algorithms shift their performance bottleneck toward memory access and graph traversal, as confirmed by the roofline analysis in Figure 1. Motivated by this observation, this paper focuses on addressing the memory-bound nature of modern graph-based ANNS. We choose SymphonyQG as a representative algorithm to study and resolve the memory-related challenges.

- B. Processing-In-Memory Architectures

Processing-In-Memory (PIM) architectures represent a class of memory-centric systems designed to mitigate the longstanding memory wall by embedding lightweight processing units (PUs) directly inside or adjacent to memory banks. By co-locating computation with data, PIM enables high internal memory bandwidth and reduces costly data movement across the memory hierarchy. Figure 3 illustrates a representative commodity PIM architecture, capturing common design principles shared by modern PIM systems (e.g., UPMEM DRAMPIM [17], Samsung PIM-HBM [32] and SK Hynix AiM [33]).

A PIM system consists of a large number of memory chips, each augmented with multiple PUs. Every PU is tightly coupled to a private local memory bank and executes a lightweight instruction set, while a conventional host CPU remains responsible for query dispatch, synchronization, and result aggregation. Table I further quantifies these architectural characteristics by comparing PIM systems with conventional CPU and GPU platforms. While PIM exposes significantly higher aggregate internal memory bandwidth through massive parallelism across thousands of PUs, each PU operates with limited local memory capacity and relatively modest compute capability compared to modern processors.

- C. Co-Design Challenges for ANNS on PIM


Although the massive internal bandwidth of PIM architectures makes them appear well suited for memory-bound workloads, applying PIM directly to graph-based ANNS exposes a fundamental algorithm–architecture mismatch. As discussed in Section II-A, graph-based ANNS is inherently irregular and data-dependent, relying on dynamic graph traversal, evolving beams, and frequent state updates. At the same time, PIM architectures are distributed (data partitioned across thousands of PUs), resource-constrained (limited PU-private memory),

These four challenges are deeply interrelated. Limited PU-

TABLE I: Comparison of representative CPU, GPU, and PIM platforms. The CPU, GPU, and UPMEM configurations are based on real hardware and are scaled to the same power budget for fair comparison. Specifications of PIM-HBM and AiM are derived from vendor technical reports, as no publicly available hardware is currently accessible.

Architecture Type Power (TDP) Peak TFLOPS External BW Internal BW # PUs / Cores PU-Private Memory Power-Comparable Systems (scaled to power budget of ∼400W)

UPMEM PIM PIM ∼400W 14.0 (TOPS) 150 GB/s 2.8 TB/s 3584 DPUs 64 MB (MRAM) (28 DIMMs) Intel Xeon Gold 6330 CPU 410W 7.2 375 GB/s - 56 Cores N/A (Shared DRAM) (Dual-Socket) NVIDIA A100 GPU 400W 312 2.0 TB/s - 6912 (CUDA) N/A (Shared HBM) (SXM4) 432 (Tensor)

###### Hypothetical / Component-Level Systems

PIM-HBM (Samsung) PIM Not Stated 1.2 307 GB/s 1.2 TB/s 128 PUs 16 MB AiM (SK Hynix) PIM Not Stated 1.0 64 GB/s 1.0 TB/s 32 PUs 32 MB

PIM-friendly Index construct (Sec. Ⅳ.A)

Low-bit vector

- PU 1

- PU 2


- Vector 1
- Vector 2 Vector n


| |
|---|


| |
|---|


C3

| |
|---|


| |
|---|


…

| |
|---|


| |
|---|


…

C1

C1

PU n

C2

Raw vectors Clustering the vectors

Graph Building

RabitQ quantization

Cluster dispatching

Offline

###### Online

###### Local Memory

Query Dispatcher Search Engine

Trigger Send

Received Mini-Batch Graph Search

Graph index structure

Query In Cluster Filtering

Add to Buffer

Query Scheduling (Sec. Ⅳ.B)

|Q1|Q2|
|---|---|


Q3 Q4

Post-processor

Size: EF Multiplication-Free Opt. (Sec. Ⅳ.C)

EF

Query Results

Res. Out Final Reranking Fetch Result Send Back Store Candidates Calculate Dist. PIM

Host

Topk

Fig. 4: Overview of PIMCQG

O3: Multiplication-Free Distance Computation. Even after memory and scheduling bottlenecks are mitigated, C4 remains a key obstacle. To address this, we redesign the distance computation kernel, and replace expensive, PIMhostile floating-point multiplication operations with a sequence of highly efficient bitwise shift and addition operations. This better matches the strengths of lightweight PIM cores and enables efficient in-memory execution of graph-based ANNS without sacrificing search quality. (Details in Section IV-C.)

local memory capacity (C1) forces aggressive graph partitioning, which in turn amplifies inter-PU communication during traversal (C2). High communication overhead exacerbates coordination costs and load imbalance across PUs (C3), while restricted PU compute capability (C4) further constrains the choice of distance computation and pruning strategies that could otherwise mitigate these effects. Overcoming them requires a holistic algorithm–hardware co-design.

III. PIMCQG: A CO-DESIGN FRAMEWORK FOR ANNS ON PIM

Figure 4 provides a high-level overview of PIMCQG’s end-to-end execution flow and illustrates how the proposed optimizations interact across the host and PIM. Conceptually, query processing is organized into three modules: a hostside Query Dispatcher, an in-PIM Search Engine, and a host-side Post-processor. O1 defines the compact graph index traversed by the Search Engine, O2 governs the asynchronous interaction among modules, and O3 optimizes the distance computation used during candidate evaluation. The detailed design of each optimization is presented in Section IV.

This paper presents PIMCQG, a co-design framework that realigns state-of-the-art graph-based ANNS [19] with the architectural realities of commodity PIM systems. PIMCQG consists of three synergistic optimizations that jointly address the coupled challenges discussed earlier.

### O1: PIM-Friendly Compact Index. To address C1 and C2,

we propose a compact index structure that removes redundant quantization metadata and offloads exact reranking to the host. This substantially reduces the memory footprint of the graph index, enabling massive datasets to fit within distributed PIM memories while also reducing the partitioning pressure that would otherwise amplify remote traversal and communication overhead. (Details in Section IV-A.)

IV. DESIGN DETAILS OF PIMCQG A. PIM-friendly Compact Index

We first revisit the internal organization of the SymphonyQG index. As illustrated in Figure 5(a), the index stores four parts per node, including the original vector, neighbor codes, neighbor factors and neighbor IDs. The “neighbor code” and “neighbor factor” components are the core of RabitQ quantization, enabling the replacement of expensive, full-precision distance calculations with highly efficient approximate ones. This design, while effective for enabling accurate quantizationaware distance estimation, introduces substantial redundancy and memory overhead. Therefore, directly porting the SymphonyQG index to PIM would not only exceed memory

### O2: Asynchronous Pipelined Scheduling. For C3, we

propose an asynchronous pipelined scheduling architecture. It decouples host-side query dispatch and post-processing from in-PIM search through dynamic mini-batching and FIFObased asynchronous execution. By overlapping communication, search, and post-processing, this design reduces synchronization stalls, better utilizes host–PIM bandwidth, and improves throughput under highly data-dependent workloads. (Details in Section IV-B.)

high recall. However, storing raw vectors directly on PIM is impractical due to their large footprint (DIM∗32 bits per node).

Redundant representation of Node 3

Redundant representation of Node 3

| |
|---|
|Node 4 code<br><br>Node 4 code|
| |


| | |
|---|---|
|Node 4 Factor<br><br>Node 4 Factor| |
| | |


Node 3 code

Node 3 Factor

Node 3 code

Node 3 Factor

5

- 3

- 1

- 2


- 4


5

- 3

- 1

- 2


- 4


PIMCQG addresses this issue by restructuring the ranking pipeline to decouple approximate search from exact reranking. This design is motivated by two observations: 1) RabitQ-based distance estimation already provides sufficiently accurate ordering for traversal and pruning; and 2) exact distances are only required for a small candidate set near convergence. Therefore, PIMCQG removes raw vectors from the PIM-resident index and retains them exclusively on the host. This enables approximate-only traversal on PIM. The query execution path is hence redesigned as follows:

| |
|---|
|Node 1 code<br><br>Node 1|
|code|


| | |
|---|---|
|Node 1 Factor<br><br>Node 1 Factor| |
| | |


Node 3 code

Node 3 Factor

Node 3 code

Node 3 Factor

|Node 2|
|---|
|code<br><br>Node 2|
|code|


|Node 2| |
|---|---|
|Factor<br><br>Node 2 Factor| |
| | |


Node 3 code

Node 3 Factor

Node 3 code

Node 3 Factor

Original Code Neighbor code Neighbor Factor Neighbor IDs DIM * 32bits DIM *#degree bit #degree* 96bits #degree * 32bits

One vector:

Original Code Neighbor code Neighbor Factor Neighbor IDs DIM * 32bits DIM *#degree bit #degree* 96bits #degree * 32bits

One vector:

x N vectors

…

###### (a) SymphonyQG index structure (per-vector)

x N vectors

…

###### (a) SymphonyQG index structure (per-vector)

(a) SymphonyQG (per-vector)

- 3

- 1

- 2


- 4 5


…

…

x C clusters

###### …

- 3

- 1

- 2


- 4 5


…

…

x C clusters

###### …

|Code 1:| | |Node 1 Factor| |
|---|---|---|---|---|
|RabitQ code| | |RabitQ Factor| |


- Code 1:
- Code 2:
- Code 3:


1 2

- Node 1 Factor
- Node 2 Factor
- Node 3 Factor


Node 3:

- Code 2:
- Code 3:


1 2

- Node 2 Factor
- Node 3 Factor


Node 3:

1 3

- Node 1:
- Node 2:


1 3

- Node 1:
- Node 2:


2 3

2 3

centroid Neighbor IDs

One cluster:

centroid Neighbor IDs

RabitQ code

RabitQ Factor

One cluster:

- 1. (Host-side) Filters the target clusters for the query and dispatches search requests to corresponding PUs.
- 2. (PIM-side) Traverses the graph and computes approximate distances using canonical quantized representations.
- 3. (PIM-side) Generates the candidate set with size EF based on approximate distances.
- 4. (Host-side) Fetches the candidate set from PIM and performs a full-precision reranking to get the final top-k.


DIM * Nc bits

32 * Nc bits

#D * Nc * 32bits

PIM Storage

DIM * Nc bits

32 * Nc bits

#D * Nc * 32bits

PIM Storage

###### Host Storage

Original Code DIM * 32 * N bits

Centroid Vector DIM * 32 * C bits

All clusters:

(Main Memory or Disk)

###### Host Storage

Original Code DIM * 32 * N bits

Centroid Vector DIM * 32 * C bits

All clusters:

(Main Memory or Disk)

(b) PIMCQG index structure (per-cluster)

(b) PIMCQG (per-cluster)

(b) PIMCQG index structure (per-cluster)

Fig. 5: Index structure of SymphonyQG and PIMCQG

capacity, but also negate the benefits of near-data processing due to excessive communication overhead. To address these issues, we redesign the SymphonyQG index structure with two complementary techniques as described below.

Accuracy tradeoff. Compared to the SymphonyQG baseline, PIMCQG quantizes vectors to their respective centroids, other than to the current searched node. This sacrifices locality and may introduce accuracy loss to the quantized representations. To preserve search accuracy, we adopt an over-fetching strategy that uses EF > nb to enlarge the candidate set returned by PIM. The value of EF is selected empirically based on the study in Section V-D to trade off performance with accuracy.

1) Eliminating Index Redundancy: The redundancy is a direct result of SymphonyQG’s quantization logic, where a neighbor’s quantized code o is computed relative to the current node’s vector v (i.e., o = o

r−v

||or−v||), causing the same neighbor (e.g., “Node 3” in Figure 5(a)) to store different, redundant codes and factors for each incoming edge.

- A naive approach to eliminating this redundancy would be


B. Asynchronous Pipelined Query Scheduling

to quantize all vectors using a single global reference. However, such a design fails to capture local data distributions and significantly degrades quantization accuracy. Instead, we adopt an Inverted File (IVF)-style clustering strategy: we partition the dataset into clusters, assign each node to a cluster, and use the cluster centroid c as the shared reference for quantization within the cluster (i.e., o = o

Our compact index design makes the IVF cluster the fundamental unit of deployment: each cluster contains a selfcontained search structure (i.e., neighbor IDs and RabitQ code/factor arrays) in PU-local memory, while full-precision vectors remain on the host for final reranking. This organization avoids fine-grained graph partitioning across PUs and turns query execution into a cluster-aware scheduling problem. Based on this organization, PIMCQG adopts a two-level scheduling strategy consisting of offline cluster placement and online mini-batch pipeline execution.

r−c

||or−c||). Under this design, each node is encoded once relative to its assigned centroid, producing a single canonical RabitQ code and scaling factor that can be reused by all incoming edges.

Figure 5(b) shows the resulting PIM-aware index layout. The graph adjacency lists stored in PIM now contain only neighbor IDs, while the corresponding canonical RabitQ codes and scaling factors are stored once in shared PIM-resident arrays. During traversal, a PU follows a neighbor ID and resolves it to the node’s canonical code/factor entry, rather than edge-specific code/factor. Thus, multiple edges pointing to the same node reuse the same quantized representation, eliminating redundancy while preserving quantization accuracy.

1) Scheduling Challenges on Commodity PIM: Before online execution, PIMCQG first places compact-index clusters onto PUs using a greedy load-balancing policy based on estimated or profiled access frequency [4], [8]. This step mitigates persistent hot spots caused by skewed cluster popularity and improves utilization across the PU array. Because the compact index substantially reduces the memory footprint of each cluster, the scheduler has more flexibility to balance load while respecting the PU-local memory budget. In this sense, the data-layout optimization of O1 directly enables the scheduling flexibility required by O2.

### 2) Eliminating Raw Vectors via Approximate Ranking:

After eliminating edge-specific quantization redundancy, the remaining dominant contributor to index size in SymphonyQG is the storage of full-precision raw vectors. As described in Section II-A, SymphonyQG retains full-precision vectors to support exact distance computation during search. Although not used at every traversal step, these vectors are accessed periodically to rerank candidates and refine the beam, ensuring

After cluster placement, the primary runtime bottleneck becomes host–PIM coordination, due to the significant gap between external and internal bandwidth on PIM. Figure 6 characterizes the communication cost between the host and PIM across different transfer sizes. The key observation is

TransferTime(s)

TransferTime(s)

| | | | | | | | | | | | | | | |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | | | | | | |
| | | | |H PI|ost M| |PI Ho|M st| | | | | | |
| | | | | | | | | | | | | | | |
| | | | | | | | | | | | | | | |
| | | | | | | | | | | | | | | |
| | | | | | | | | | | | | | | |


| | | | | | | | | | | | | | | |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | | | | | | |
| | | | |H PI|ost M| |PI Ho|M st| | | | | | |
| | | | | | | | | | | | | | | |
| | | | | | | | | | | | | | | |
| | | | | | | | | | | | | | | |
| | | | | | | | | | | | | | | |


①Prepare ②Host->PIM ③PIM Process ④PIM->Host ⑤Post-process

- 0.0e+00
- 1.0e-06
- 2.0e-06


Query Batch

1.5e-01

(a) Batch-synchronous Model

- 0.0e+00

5.0e-02

- 1.0e-01


- Mini Batch A ①
- Mini Batch B
- Mini Batch C


② ③ ④ ⑤

① ② ③ ④ ⑤

Saving Time

816326412825651210242048409681921638432768

816326412825651210242048409681921638432768

① ② ③ ④ ⑤

Data Size (bytes)

Data Size (bytes)

…

(b) Host-PU collaboration Model

(a) UPMEM PIM

(b) PIM-HBM (Simulated)

Fig. 8: Comparison of Host-PU collaboration strategy.

Fig. 6: Host-PIM communication overhead. We vary the size of data transferred from/to PIM and measure the latency.

the Search Engine on each PU reads a mini-batch, traverses the compact PIM-resident graph index, and computes approximate distances using the multiplication-free kernel described in the next subsection. Third, the Post-processor on the host continuously fetches returned candidate sets, retrieves the corresponding raw vectors, and performs exact reranking to produce the final top-k results. This execution flow is consistent with the end-to-end workflow shown in Figure 4.

Synchronization

| | |
|---|---|
| | |


![image 2](<original_images/imageFile2.png>)

+. Query

√ Result

Router PU PU PU

(a) Batch-synchronous model

+. Query

√ Result

Router

PU PU PU

(b) Per-Query Dispatching (Only use 1/3 bandwidth)

Trigger Batch send

Mini-Batching Buffer (Host)

Although the logical execution consists of three components, the end-to-end processing is executed as five overlapped stages: 1 host-side query preparation, 2 host-to-PIM transfer,

- 4/4

- 5/4


√ Result

+. Query +. Query +. Query

√ Result

Router

7/4

√ Result

3 in-PIM query processing, 4 PIM-to-host result return, and 5 host-side reranking. Figure 8 highlights the key benefit of

(c) Mini-Batching Query Dispatching (Fully use Host-PU Comm.)

Fig. 7: Comparison of different scheduling strategies.

this design: unlike batch-synchronous execution, communication, in-PIM search, and host-side reranking proceed concurrently rather than serially. As a result, the host no longer waits for all PUs to finish before beginning post-processing, and the PUs no longer stall until the host completes reranking for an entire batch. This is particularly important for graph-based ANNS, where traversal varies significantly across queries.

that transfer latency is not linear in practice: very small transfers under-utilize bandwidth, while large transfers incur much higher latency. Therefore, an effective scheduling policy must strike a balance between these two extremes.

Unfortunately, existing mainstream scheduling strategies fail to navigate this trade-off effectively. The widely adopted batch-synchronous model [3], [4], [8], [9], [37] aggregates a large batch of queries, dispatches them to PUs, and blocks until every PU finishes before collecting results (Figure 7(a)). This introduces rigid global barriers: the host remains idle during in-PIM search, and the PUs later remain idle while the host performs post-processing and prepares the next batch. At the opposite extreme, per-query dispatching removes the global barrier by sending each query immediately (Figure 7(b)). However, this strategy fragments communication into transfers that are too small to efficiently utilize the host–PU bandwidth, and fails to exploit system-wide transfer parallelism when the host serializes many tiny requests.

To maintain stable execution, PIMCQG uses FIFO queues to decouple the host and PIM stages and applies lightweight flow control to bound the number of in-flight mini-batches. This avoids queue overflow without reintroducing coarse-grained synchronization barriers.

The effectiveness of this design also depends on choosing a good mini-batch size. Let Tpre(NB), Tproc(NB), and Tpost(NB) denote the host-side dispatch time, in-PIM processing time, and host-side reranking time for a mini-batch of size NB, respectively. Since these stages execute in parallel, the average processing time per query is determined by the slowest stage:

2) Dynamic Mini-Batch Pipeline Design: To balance communication efficiency and execution overlap, PIMCQG adopts dynamic mini-batching, as illustrated in Figure 7(c). The host maintains a input buffer per-PU, and each incoming query is appended to the buffers of the PUs that store its relevant clusters after cluster filtering. A mini-batch is dispatched when either the workload reaches a target threshold (e.g., 4 in Figure 7(c)) or the oldest buffered query exceeds a waitingtime limit. This policy allows the system to aggregate enough work to amortize communication overhead under heavy load, while still being responsive when the arrival rate is low.

max (Tpre(NB), Tproc(NB), Tpost(NB)) NB

(1)

T(NB) =

Therefore, the optimal mini-batch size is chosen as

N∗ = arg min

T(NB).

NB

Based on the real hardware limits observed in Figure 6, we tune N∗ to the point where Tproc ≈ max Tpre,Tpost . This ensures that the data size stays within the fast communicating range (e.g., under 8KB), keeping the execution pipeline fully balanced and efficient.

Based on this dispatch policy, PIMCQG organizes execution

C. Multiplication-Free Distance Computation

- as an asynchronous pipeline spanning the host and PIM. Logically, the pipeline contains three components. First, the Query Dispatcher on the host performs cluster filtering, fills per-PU buffers, and sends ready mini-batches to PIM. Second,


After the compact index and asynchronous pipeline are in place, the efficiency of the PU-side search engine is determined primarily by its inner-loop distance computation.

PIMCQG inherits the distance computation kernel from RabitQ [16], where the approximate distance between the query vector q and the candidate vector o is computed as:

√

∥o∥ ⟨¯,o⟩

dappro = ∥o∥2 + 2 ·

+∥q∥2

· ⟨c,¯o⟩ ·

D

Query-independent term

∥o∥ ⟨¯,o⟩

−

#### · 2 · ⟨¯,q⟩ − sumq · 2

Reduced to additions via RabitQ

Outer scaling

(2)

where ¯ is the vector reconstructed from the quantization code, c is the cluster centroid, and sumq is the sum of query values.

Conceptually, this formula consists of three logical components: (1) a query-independent term that can be precomputed, (2) an inner-product term heavily optimized into simple additions, and (3) an outer scaling operation that involves floatingpoint multiplication and division. Thus, although RabitQ successfully removes multiplication from the inner product itself, the remaining outer scalar is still poorly matched to commodity PIM cores and becomes a major source of latency inside the Search Engine.

To minimize runtime arithmetic, we first isolate all query-independent terms and precompute them as a single RabitQFactor during index construction. Since ∥q∥2 is identical for all candidates within a query, it can be omitted without affecting relative ranking. In addition, all candidate vectors are normalized in advance, so ∥o∥=1. Under these conditions, the static portion of the approximate-distance expression can be absorbed into a per-node constant stored in the compact index. The primary remaining computational bottleneck is therefore the outer scalar factor derived from ⟨¯,o⟩.

Because ¯ is reconstructed through a random orthogonal transformation, its norm is also 1, and the inner product ⟨¯,o⟩ reduces to cos(θ), where θ is the angle between the original vector o and its reconstruction ¯. In the original RabitQ formulation, this term is node-specific, forcing the PU to repeatedly apply a distinct floating-point scaling factor for each candidate. Our key observation is that this quantization-error term is sufficiently stable within an IVF cluster. Because vectors in the same cluster are encoded relative to a shared centroid, their quantization-error distribution is also empirically stable

- at the cluster level. This makes a cluster-wise approximation natural. Therefore, instead of maintaining a separate cos(θ) for each node, PIMCQG replaces it with a cluster-wide constant α, yielding the simplified approximate-distance formulation:


1 α · (RabitQ result) (3)

dappro = RabitQFactor −

where RabitQ result denotes the lookup-based term already computed using additions.

To fully eliminate multiplication from the online search path, we further approximate the inverse factor 1/α using only bit shifts and additions. Empirical analysis indicates that for typical feature dimensions (e.g., 102 to 106), cos(θ) concentrates around 0.8 [16]. Setting α= 0.8 gives 1/α = 1.25. This

cos( ) 0.304 0.137 0.157 0.080 0.012 -0.040 -0.080

100

Recall(%)

90

fixed

0.259

80

-0.041

70

60

-0.065

50

10 20 30 40 50 60 70 80 90 100

EF

Fig. 9: Recall of PIMCQG using node-specific cos(θ) or fixed α on SIFT.

value has a convenient binary representation, 1.012, which translates directly into efficient bitwise operations: x · 1.25 ≈ x+(x >> 2). When a specific dataset or cluster deviates from this default, α is calibrated during index construction to the nearest hardware-friendly binary-shift equivalent. As a result, the expensive floating-point scaling is removed from online search and absorbed into lightweight offline preprocessing.

Finally, we validate that this simplification does not materially degrade search quality. Figure 9 compares the original node-specific cos(θ) formulation against the fixed-α version across different EF settings on the SIFT dataset. The results show that using a fixed α = 0.8 incurs negligible accuracy loss, with a maximum recall drop below 0.08%. Therefore, PIMCQG achieves a multiplication-free in-PIM distance kernel while preserving the ranking quality for high-recall graph traversal.

V. EXPERIMENTAL EVALUATION

We evaluate PIMCQG on a real-world commodity PIM platform to answer three key questions: (1) Can PIMCQG deliver high-throughput graph-based ANNS at high recall, (2) how do its algorithm–hardware co-design choices contribute to performance and energy efficiency, and (3) does the design scale across datasets and system configurations.

A. Experimental Setup

- 1) Hardware Setup: We conduct evaluations on three types

of hardware platforms, including PIM, CPU and GPU. The PIM platform contains a real-world UPMEM PIM server [10] equipped with a dual-socket Intel Xeon Silver 4110 host processor (2.10GHz), 256GB DDR4 host memory, and 20 PIM modules providing up to 2,560 DPUs operating at 350 MHz. The total system power is approximately 450W, including ∼170W for the host CPU and ∼14W per activated PIM module. For future-looking scalability analysis, we additionally use the vendor-provided open-source simulators for Samsung PIM-HBM [47] and SK Hynix AiM [49], since these platforms are not yet publicly accessible as commercial systems. The CPU platform contains a dual-socket Intel Xeon Gold 6330 platform (2.0GHz, 112 threads) with 512GB of host memory and a total system power of ∼410W. The GPU platform attaches an NVIDIA A100-SMX4 (80GB) to the CPU platform, resulting in an estimated total system power of ∼810W. For multi-GPU comparison, we expand the system to include up to eight A100 GPUs with the same configurations.

- 2) Datasets: We experiment on three industry-standard


billion-scale benchmarks: SPACEV1B (D=100) [44], SIFT1B (D=128) [22], and SimSearchNet++ (SSN1B, D=256) [12]. Their uncompressed raw-vector footprints are 95GB, 123GB,

SymphonyQG (CPU) GGNN (GPU)

PIMCQG

| |
|---|


| |
|---|


15k

30k

30k

10k

20k

20k

10k

5k

10k

6k

- 0

- 1k

- 2k

- 3k


4k

QPS

4k

2k

2k

0

0

62% 70% 76% 80% 84% 86% 88% 90% 92% 94%

67% 72% 76% 78% 80% 82% 84% 86% 88% 90%

64% 68% 72% 76% 78% 80% 82% 84% 86% 88%

Recall (%)

Recall (%)

Recall (%) (c) SSN1B

(a) SIFT1B

(b) SPACEV1B

Fig. 10: QPS vs. recall@10 of compared baselines. Each point is obtained by varying the search-cluster count and EF.

SymphonyQG (CPU) GGNN (GPU)

PIMCQG

| |
|---|


| |
|---|


75

75

30

50

50

20

25

25

10

QPS/W

| | | |
|---|---|---|
| | | |


| | | |
|---|---|---|
| | | |


| | | |
|---|---|---|
| | | |


12

12

4

8

8

2

4

4

0

0

0

62% 70% 76% 80% 84% 86% 88% 90% 92% 94%

67% 72% 76% 78% 80% 82% 84% 86% 88% 90%

64% 68% 72% 76% 78% 80% 82% 84% 86% 88%

Recall (%)

Recall (%)

Recall (%)

(a) SIFT1B

(b) SPACEV1B

(c) SSN1B

Fig. 11: Energy efficiency (QPS/W) vs. recall@10 for the compared baselines across three datasets.

and 239GB, respectively. For each dataset, we use the default public query set and compute the ground-truth nearest neighbors using exact brute-force search.

PIMCQG

A100 *4

A100 *8

| |
|---|


| |
|---|


| |
|---|


75

100K

###### QPS/W

50

QPS

- 3) Baselines: We compare PIMCQG against four representative baselines spanning CPU, GPU, and PIM platforms.

- • SymphonyQG [19] serves as the primary CPU baseline and the SOTA graph-based ANNS method. Because the full SymphonyQG index exceeds the memory capacity of the CPU platform, we apply the same IVF partitioning used by PIMCQG and load only the query-relevant clusters into host memory; disk I/O is excluded from all reported timings to isolate search performance.
- • UpANNS [8] and PIMANN [57] serve as the two PIM baselines, representing batch-synchronous and per-query scheduling strategies, respectively, on UPMEM.
- • GGNN [20], a popular GPU-based acceleration for billionscale graph-based ANNS, is adopted for comparison since SymphonyQG lacks GPU support. We select GGNN over newer GPU-based systems (e.g., CAGRA [45], PathWeaver [29]), which target million-scale datasets and do not scale to billion-scale workloads. GGNN utilizes a kNN graph index, similar to SymphonyQG and PIMCQG. For fairness on billion-scale datasets, GGNN is configured with 16 data shards in our experiments.


- 4) Metrics: We report four primary metrics: throughput (QPS), recall@10, index size (GB), and energy efficiency (QPS/W). Unless otherwise specified, our default setting uses SIFT1B, builds an IVF partitioning with 8,192 clusters (limited by the 64 MB per-DPU memory budget), probes 8 clusters per query, constructs the graph index with node degree 32, and sets the over-fetched candidate size EF to 40.


50K

25

3.21×

3.96×

6.69×

9.18×

0

0

SIFT1B SPACEV1B

SIFT1B SPACEV1B

Fig. 12: Comparing PIMCQG with GGNN on 4 and 8 GPUs.

Across most recall targets, PIMCQG achieves the highest throughput among all three platforms. Compared with SymphonyQG, PIMCQG delivers up to 7.1×, 7.4×, and 20× higher QPS on SIFT1B, SPACEV1B, and SSN1B, respectively. Compared with single-GPU GGNN, PIMCQG achieves up to 17.1×, 16.7×, and 15.8× higher throughput on the same datasets within the practically relevant recall range of approximately 0.60–0.84. At very high recall targets, GGNN becomes more competitive, especially on SPACEV1B. This behavior is expected because the single A100 GPU cannot fully hold the full graph index, forcing CPU–GPU data swapping that becomes a persistent overhead regardless of search strictness. Even so, PIMCQG maintains a clear advantage throughout the main operating region used in practice.

PIMCQG also provides substantially higher energy efficiency than both baselines. On SIFT1B, it achieves 4–76 QPS/W, compared with 1.5–11 QPS/W for SymphonyQG and about 2.5 QPS/W for GGNN, corresponding to improvements of up to 6.5× over CPU and 30.8× over GPU. Similar trends hold on SPACEV1B and SSN1B, confirming that the throughput gains of PIMCQG are not achieved through disproportionate power consumption, but instead through a more energy-proportional execution model.

To isolate the effect of GPU memory capacity, we additionally evaluate GGNN on 4× and 8× A100 configurations in which the full index fits in GPU memory. As shown in Figure 12, multi-GPU GGNN significantly increases raw throughput and can surpass PIMCQG in absolute QPS. However, this comes at a much higher system power cost. As a result, PIMCQG still retains a 3.2×–9.1× advantage in QPS/W over the 4× and 8× A100 configurations on SIFT1B and SPACEV1B. These results show that while aggressively scaled

- B. End-to-End Performance Across Hardware


We first evaluate whether PIMCQG improves the end-toend throughput and energy efficiency of graph-based ANNS relative to representative CPU and GPU baselines. Specifically, we compare PIMCQG against SymphonyQG on CPU and GGNN on GPU, and vary the search-cluster count and EF to cover a broad range of recall targets. Figures 10 and 11 report the resulting throughput and energy-efficiency trade-offs.

UpANNS PIMANN PIMCQG

10K

10K

QPS

~48×

~129×

| |
|---|


Reach saturation (~61.4%)

1K

| |
|---|


| | |
|---|---|
| | |


Reach saturation (~66.7%)

1K

55 60 65 70 75 80 85 90 95

60 65 70 75 80 85 90

Recall (%)

Recall (%)

(a) SIFT1B (b) SPACEV1B

Fig. 13: QPS vs. recall@10 for PIMCQG and prior PIM-based ANNS systems.

TABLE II: Index footprint of PIMCQG and SymphonyQG.

SIFT1B SPACEV1B SSN1B

SymphonyQG 1423 GB 1327 GB 2385 GB PIMCQG 138 GB 138 GB 164 GB Reduction Ratio 10.31x 9.62x 14.54x

GPU systems can deliver high peak throughput, PIMCQG offers a substantially more energy-efficient solution for billionscale graph-based ANNS.

- C. Comparison with Existing PIM-based ANNS Systems

We next compare PIMCQG with prior PIM-based ANNS systems to assess whether the proposed co-design closes the capability gap between PIM and high-recall graph-based search. We use UpANNS and PIMANN as representative baselines, both of which implement IVFPQ-based ANNS [4], [8] on the UPMEM platform. Since their original studies used a relaxed recall definition, we re-evaluate both methods using the standard recall@10 metric to ensure a fair comparison. Figure 13 reports the resulting recall-throughput curves.

Under this standardized evaluation protocol, PIMCQG shows a much stronger recall-throughput trade-off than both prior PIM solutions. A key observation is that UpANNS and PIMANN reach a clear capability ceiling at relatively low recall levels (e.g., 61.4% on SIFT1B and 66.7% on SPACEV1B), after which throughput drops sharply. In contrast, PIMCQG maintains high throughput well beyond these saturation points.

When compared at recall levels near the saturation boundaries of the prior PIM baselines, PIMCQG achieves up to 48× higher throughput on SIFT1B and 129× higher throughput on SPACEV1B. These results show that the contribution of PIMCQG is not merely incremental acceleration over previous PIM designs; rather, it enables a qualitatively different operating regime by supporting the higher recall levels expected from modern graph-based ANNS workloads.

- D. Component-wise Analysis and Ablation


To understand where the gains of PIMCQG come from, we next evaluate the individual effects of its three co-designed optimizations and analyze the remaining system bottlenecks.

1) Compact Index Footprint: We first examine the memory footprint of the compact index introduced in Section IV-A. Table II compares the index size of PIMCQG with SymphonyQG across the three billion-scale datasets. By removing raw vectors from PIM and replacing per-edge metadata with a compact cluster-aware structure, PIMCQG reduces index size by 10×–14× (e.g., from 1423GB to 138GB on SIFT1B). This reduction is critical for making billion-scale graph-based

Prepare

CPU DPU

DPU search

DPU CPU

Postprocessor

Others

| |
|---|


| |
|---|


| |
|---|


| |
|---|


| |
|---|


Total Time Query Dispatch

DPU search Post-processor Others

0.0 0.5 1.0

0.0 0.5 1.0

0.0 0.5 1.0

SIFT1B

SPACEV1B

SSN1B

Fig. 14: Performance breakdown. The overall execution time is dictated by the slowest pipeline stage.

12

12

- 0.75

- 1


- 0.75

- 1


NormalizedRecall

NormalizedRecall

NormalizedQPS

NormalizedQPS

Norm. QPS

Norm. QPS

10

10

Norm. Recall

Norm. Recall

8

8

6

6

0.50

0.50

4

4

0.25

0.25

- 0

- 1

- 2


- 0

- 1

- 2


0

0

30 50 70 80 100 150 200 300

30 50 70 80 100 150 200 300

EF

EF

(a) SIFT1B

(b) SPACEV1B

Fig. 15: The impact of overfetching-reranking strategy

ANNS feasible on commodity PIM and directly validates the effectiveness of the compact-index design.

- 2) Pipeline Bottleneck Analysis: We then analyze the run-

time breakdown of PIMCQG using Figure 14. Since the asynchronous pipelined execution overlaps query dispatch, DPU search, and post-processing, the end-to-end latency is determined by the slowest pipeline stage. Across all datasets, the actual DPU search contributes only a relatively small fraction (≤50%), while the post-processing stage, including DPU-to-CPU result transfer and host-side exact distance recomputation, dominates the total execution time. Note that this overhead is attributed to two inherent factors: offloading raw vectors to the host to reduce PIM memory footprint, and the limited host-PIM bandwidth of current UPMEM hardware. It also suggests that PIMCQG can benefit substantially from future PIM systems with higher host-PIM bandwidth.

- 3) Overfetching-Reranking: PIMCQG relies on a host-side


reranking stage over an overfetched candidate set (size of EF) to preserve high accuracy. To study the impact of this strategy, we vary the value of EF and normalize the QPS and recall results of PIMCQG against the results of SymphonyQG with the candidate set size nb=30.

Figure 15 clearly shows the trade-off between throughput and accuracy. Without overfetching (EF = nb), PIMCQG achieves very high throughput (10×-10.4× that of SymphonyQG), but achieves only 81%–89% of the baseline recall. When increasing the overfetch size (to 150 for SIFT1B and 100 for SPACEV1B), PIMCQG achieves the same recall level as SymphonyQG, while still preserving 4×-6x higher QPS. This result confirms that overfetching-reranking is an effective mechanism for preserving accuracy without sacrificing the throughput advantage of the compact PIM index.

4) Asynchronous Pipelined Scheduling: We next isolate the contribution of the scheduling design in Section IV-B by comparing PIMCQG against three alternatives: per-query dispatching, batch-synchronous scheduling, and PIMCQG 1, a pipeline variant with mini-batch size fixed to one.

Figure 16 shows that PIMCQG outperforms naive per-query dispatching by 70×–155× across all three datasets, demonstrating that fine-grained dispatch fails to utilize host–DPU com-

w/o MF w/ MF

40K Per-Query

PIMCQG_1

| |
|---|


0.15

Batch Sync

PIMCQG

| |
|---|


30K

-49.6%

-60.4%

0.10

Time(s)

QPS

-60.8%

20K

0.05

10K

600

400

200

0

0.00

SIFT1B SPACEV1B SSN1B

SIFT1B SPACEV1B SSN1B

Fig. 16: Throughput comparison of different scheduling strategies

Fig. 17: DPU search time w/ and w/o multiplicationfree distance computation

- 100

- 101


QPS

QPS/W

vs CPU

vs GPU

| |
|---|


| |
|---|


- 100

- 101

- 102


Speedup

Speedup

1 2 4 8 16 32

UPMEM PIM-HBM AiM

Number of nodes

Fig. 18: Multi-node scalability of PIMCQG on SIFT1B.

Fig. 19: Speedup of PIMCQG on different PIM architectures.

munication parallelism. Compared with batch-synchronous execution, the full asynchronous pipeline yields a ∼1.5× throughput improvement by overlapping communication and computation across query batches. Finally, relative to PIMCQG 1, using an appropriately sized mini-batch provides a further 1.7×–2.4× improvement, confirming that dispatch granularity is critical for approaching peak effective bandwidth.

5) Multiplication-free Distance Computation: Finally, we evaluate the effect of our multiplication-free distance kernel. Figure 17 compares the DPU search phase of PIMCQG with and without the shift-add reformulation enabled. Across the three datasets, the optimized kernel reduces DPU search time by 49.6%–60.8%. This confirms that even after data movement and scheduling are optimized, arithmetic simplification remains essential for commodity PIM, and that removing floating-point multiplication from the PU-side critical path is a major contributor to the overall performance of PIMCQG.

- E. Scalability


We finally evaluate the scalability of PIMCQG in two dimensions: scale-out across multiple nodes and portability to emerging PIM hardware.

- 1) Multi-node Scalability: To evaluate distributed scaling,

we simulate a multi-node deployment of PIMCQG using a 400 Gbps InfiniBand network model in which communication cost scales with data transfer size. All speedups are normalized to GGNN on 4× A100 GPU for SIFT1B. Figure 18 shows that scaling from one node to two nodes initially causes a performance drop due to inter-node communication overhead. However, as the system scales from 2 to 32 nodes, PIMCQG exhibits near-linear speedup. At that point, the large amount of query-level parallelism dominates the inter-node overhead, indicating that PIMCQG can effectively exploit scale-out execution at the cluster level.

- 2) PIM-architecture Scalability: We also project PIMCQG


onto two emerging PIM architectures, Samsung PIM-HBM and SK Hynix AiM, both of which provide substantially higher operating frequency and host–PIM bandwidth than current

UPMEM systems. Using the vendor-provided simulators, we model the search time of PIMCQG with a GEMV kernel that matches the computational complexity of the optimized distance computation and scale it by the empirically observed average number of graph-search hops per query. Figure 19 shows that on these future platforms, PIMCQG is projected to achieve 100×–137× speedup over the CPU baseline and 6.3×–8.7× speedup over the 4× A100 GPU baseline on SIFT1B. These projections suggest that the algorithmic structure of PIMCQG is well aligned with future high-bandwidth PIM systems and can benefit directly as the underlying hardware matures.

VI. RELATED WORK

We categorize related works into two main areas: hardware acceleration for ANNS and the broader adoption of PIM.

Hardware Acceleration for ANNS. Conventional CPUs and GPUs remain common choices for ANNS acceleration, but their performance is fundamentally constrained by memory bottlenecks. CPU-based methods [5], [16], [19], [54] accelerate distance computation via SIMD but remain memory bandwidth-bound. Conversely, GPUs offer superior memory bandwidth [27], [29], [45]. However, GPU acceleration is severely limited by small global memory capacities.

To bypass these limits, various heterogeneous and specialized systems have been proposed. Heterogeneous storage systems [6], [24], [38], [52] utilize multi-tier memory hierarchies, but they introduce complex data management and I/O synchronization overheads. Similarly, specialized hardware solutions, including FPGA [25], [39], [50], [60], SmartSSD [28], [51], CXL-based memory pools [23], [30], and NAND flash [56], [58], have been developed to push performance limits. These approaches are orthogonal to our PIM-centric design, which leverages commodity hardware without custom fabrication.

PIM-Based ANNS and General Applications. PIM has emerged as a proven paradigm for overcoming memory bottlenecks, demonstrating significant success across dataintensive domains like databases [3], [9], [31], large language models [21], [40], [46], [61], and recommendation systems [2], [26], [34], [41]. However, in the context of ANNS, existing PIM accelerators [4], [8], [57] target cluster-based algorithms (e.g., IVF-PQ), which provide lower search quality and throughput compared to graph-based methods. Our work bridges this gap by explicitly mapping the high-performance graph-based ANNS algorithm to real-world PIM hardware.

VII. CONCLUSION

We presented PIMCQG, an algorithm–hardware co-design framework that enables high-recall graph-based ANNS on commodity PIM by jointly redesigning data layout, scheduling, and distance computation. The compact index reduces PIM-resident footprint by up to 14.5×, making billion-scale graphs feasible on distributed PU memories. The asynchronous pipeline overlaps host–PIM execution while fully utilizing host–PIM bandwidth. The multiplication-free kernel removes all floating-point multiplications, cutting DPU search time by up to 60.8%. Our evaluation shows that PIMCQG delivers up to 20×/17.1× throughput over CPU/GPU baselines, 129×

over prior PIM systems at high recall, and maintains strong scalability across multi-node configurations and emerging PIM architectures. Our bottleneck analysis shows that host– PIM bandwidth remains the primary constraint, suggesting that PIMCQG will benefit directly from emerging higherbandwidth PIM architectures.

REFERENCES

- [1] C. Aguerrebere, I. Bhati, M. Hildebrand, M. Tepper, and T. Willke, “Similarity search in the blink of an eye with compressed indices,” arXiv preprint arXiv:2304.04759, 2023. [Online]. Available: https://doi.org/10.48550/arXiv.2304.04759
- [2] B. Asgari, R. Hadidi, J. Cao, D. E. Shim, S.-K. Lim, and H. Kim, “Fafnir: Accelerating sparse gathering by using efficient near-memory intelligent reduction,” in 2021 IEEE International Symposium on High-Performance Computer Architecture (HPCA). IEEE, 2021, pp. 908–920. [Online]. Available: https://doi.org/10.1109/HPCA51647. 2021.00080
- [3] S. Cai, B. Tian, H. Zhang, and M. Gao, “Pimpam: Efficient graph pattern matching on real processing-in-memory hardware,” Proceedings of the ACM on Management of Data, vol. 2, no. 3, pp. 1–25, 2024. [Online]. Available: https://doi.org/10.1145/3654964
- [4] M. Chen, T. Han, C. Liu, S. Liang, K. Yu, L. Dai, Z. Yuan, Y. Wang, L. Zhang, H. Li et al., “Drim-ann: An approximate nearest neighbor search engine based on commercial dram-pims,” in Proceedings of the International Conference for High Performance Computing, Networking, Storage and Analysis, 2025, pp. 820–836. [Online]. Available: https://doi.org/10.1145/3712285.3759801
- [5] P. Chen, W.-C. Chang, J.-Y. Jiang, H.-F. Yu, I. Dhillon, and C.-J. Hsieh, “Finger: Fast inference for graph-based approximate nearest neighbor search,” in Proceedings of the ACM Web Conference 2023, 2023, pp. 3225–3235. [Online]. Available: https://doi.org/10.1145/3543507. 3583318
- [6] Q. Chen, B. Zhao, H. Wang, M. Li, C. Liu, Z. Li, M. Yang, and J. Wang, “Spann: Highly-efficient billion-scale approximate nearest neighborhood search,” Advances in Neural Information Processing Systems, vol. 34, pp. 5199–5212, 2021. [Online]. Available: https://doi.org/10.5555/3540261.3540659
- [7] R. Chen, B. Liu, H. Zhu, Y. Wang, Q. Li, B. Ma, Q. Hua, J. Jiang, Y. Xu, H. Deng et al., “Approximate nearest neighbor search under neural similarity metric for large-scale recommendation,” in Proceedings of the 31st ACM International Conference on Information & Knowledge Management, 2022, pp. 3013–3022. [Online]. Available: https://doi.org/10.1145/3511808.3557098
- [8] S. Chen, A. C. Zhou, Y. Shi, Y. Li, and X. Yao, “Upanns: Enhancing billion-scale anns efficiency with real-world pim architecture,” in SC25: International Conference for High Performance Computing, Networking, Storage and Analysis. IEEE, 2025, pp. 1–11. [Online]. Available: https://doi.org/10.1145/3712285.3759777
- [9] L. Cui, K. Yang, Y. Li, G. Wang, and X. Liu, “{PIMLex}: A {High-Performance} learned index with {Processing-in-Memory},” in 23rd USENIX Conference on File and Storage Technologies (FAST 25), 2025, pp. 287–303. [Online]. Available: https://www.usenix.org/ conference/fast25/presentation/cui
- [10] F. Devaux, “The true processing in memory accelerator,” in 2019 IEEE Hot Chips 31 Symposium (HCS). IEEE Computer Society, 2019, pp. 1–24. [Online]. Available: https://doi.org/10.1109/HOTCHIPS.2019. 8875680
- [11] X. L. Dong, “The journey to a knowledgeable assistant with retrieval-augmented generation (rag),” in Proceedings of the 17th ACM International Conference on Web Search and Data Mining, 2024, pp. 4–4. [Online]. Available: https://doi.org/10.1145/3616855.3638207
- [12] Facebook, “Facebook SimSearchNet++,” https://dl.fbaipublicfiles.com/ billion-scale-ann-benchmarks/FB ssnpp database.u8bin, 2026.

- [13] Facebook AI Research, “Faiss,” https://github.com/facebookresearch/ faiss.
- [14] C. Fu, C. Wang, and D. Cai, “High dimensional similarity search with satellite system graph: Efficiency, scalability, and unindexed query compatibility,” IEEE Transactions on Pattern Analysis and Machine Intelligence, vol. 44, no. 8, pp. 4139–4150, 2021. [Online]. Available: https://doi.org/10.1109/TPAMI.2021.3067706


- [15] C. Fu, C. Xiang, C. Wang, and D. Cai, “Fast approximate nearest neighbor search with the navigating spreading-out graph,” arXiv preprint arXiv:1707.00143, 2017. [Online]. Available: https: //doi.org/10.48550/arXiv.1707.00143
- [16] J. Gao and C. Long, “Rabitq: Quantizing high-dimensional vectors with a theoretical error bound for approximate nearest neighbor search,” Proceedings of the ACM on Management of Data, vol. 2, no. 3, pp. 1–27, 2024. [Online]. Available: https://doi.org/10.1145/3654970
- [17] J. G´omez-Luna, I. El Hajj, I. Fernandez, C. Giannoula, G. F. Oliveira, and O. Mutlu, “Benchmarking a new paradigm: Experimental analysis and characterization of a real processing-in-memory system,” IEEE Access, vol. 10, pp. 52565–52608, 2022. [Online]. Available: https://doi.org/10.1109/ACCESS.2022.3174101
- [18] L. Gong, H. Wang, M. Ogihara, and J. Xu, “idec: indexable distance estimating codes for approximate nearest neighbor search,” Proceedings of the VLDB Endowment, vol. 13, no. 9, 2020. [Online]. Available: https://doi.org/10.14778/3397230.3397243
- [19] Y. Gou, J. Gao, Y. Xu, and C. Long, “Symphonyqg: Towards symphonious integration of quantization and graph for approximate nearest neighbor search,” Proceedings of the ACM on Management of Data, vol. 3, no. 1, pp. 1–26, 2025. [Online]. Available: https://doi.org/10.1145/3709730
- [20] F. Groh, L. Ruppert, P. Wieschollek, and H. P. Lensch, “Ggnn: Graph-based gpu nearest neighbor search,” IEEE Transactions on Big Data, vol. 9, no. 01, pp. 267–279, 2023. [Online]. Available: https://doi.org/10.1109/TBDATA.2022.3161156
- [21] Y. Gu, A. Khadem, S. Umesh, N. Liang, X. Servot, O. Mutlu, R. Iyer, and R. Das, “Pim is all you need: A cxl-enabled gpu-free system for large language model inference,” in Proceedings of the 30th ACM International Conference on Architectural Support for Programming Languages and Operating Systems, Volume 2, 2025, pp. 862–881. [Online]. Available: https://doi.org/10.1145/3676641.3716267
- [22] INRIA, “SIFT1B,” http://corpus-texmex.irisa.fr/, 2026.
- [23] J. Jang, H. Choi, H. Bae, S. Lee, M. Kwon, and M. Jung, “{CXLANNS}:{Software-Hardware} collaborative memory disaggregation and computation for {Billion-Scale} approximate nearest neighbor search,” in 2023 USENIX Annual Technical Conference (USENIX ATC 23), 2023, pp. 585–600. [Online]. Available: https://www.usenix.org/ conference/atc23/presentation/jang
- [24] S. Jayaram Subramanya, F. Devvrit, H. V. Simhadri, R. Krishnawamy, and R. Kadekodi, “Diskann: Fast accurate billion-point nearest neighbor search on a single node,” Advances in neural information processing Systems, vol. 32, 2019. [Online]. Available: https: //dl.acm.org/doi/abs/10.5555/3454287.3455520
- [25] W. Jiang, S. Li, Y. Zhu, J. de Fine Licht, Z. He, R. Shi, C. Renggli, S. Zhang, T. Rekatsinas, T. Hoefler et al., “Codesign hardware and algorithm for vector search,” in Proceedings of the International Conference for High Performance Computing, Networking, Storage and Analysis, 2023, pp. 1–15. [Online]. Available: https://doi.org/10.1145/3581784.3607045
- [26] L. Ke, X. Zhang, J. So, J.-G. Lee, S.-H. Kang, S. Lee, S. Han, Y. Cho, J. H. Kim, Y. Kwon et al., “Near-memory processing in action: Accelerating personalized recommendation with axdimm,” IEEE Micro, vol. 42, no. 1, pp. 116–127, 2021. [Online]. Available: https://doi.org/10.1109/MM.2021.3097700
- [27] S. Khan, S. Singh, H. V. Simhadri, J. Vedurada et al., “Bang: Billion-scale approximate nearest neighbor search using a single gpu,” arXiv preprint arXiv:2401.11324, 2024. [Online]. Available: https://doi.org/10.48550/arXiv.2401.11324
- [28] J.-H. Kim, Y.-R. Park, J. Do, S.-Y. Ji, and J.-Y. Kim, “Accelerating largescale graph-based nearest neighbor search on a computational storage platform,” IEEE Transactions on Computers, vol. 72, no. 1, pp. 278–290,

2022. [Online]. Available: https://doi.org/10.1109/TC.2022.3155956

- [29] S. Kim, S. Park, S. U. Noh, J. Hong, T. Kwon, H. Lim, and J. Lee, “{PathWeaver}: A {High-Throughput}{Multi-GPU} system for {Graph-Based} approximate nearest neighbor search,” in 2025 USENIX Annual Technical Conference (USENIX ATC 25), 2025, pp. 1501–1517. [Online]. Available: https://www.usenix.org/conference/ atc25/presentation/kim
- [30] S. Ko, H. Shim, W. Doh, S. Yun, J. So, Y. Kwon, S.-S. Park, S.-D. Roh, M. Yoon, T. Song et al., “Cosmos: A cxl-based full in-memory system for approximate nearest neighbor search,” IEEE Computer Architecture Letters, 2025. [Online]. Available: https://doi.org/10.1109/LCA.2025.3570235


- [31] W. Kong, S. Zheng, Y. Hua, R. Ma, Y. Wen, G. Wang, C. Zhou, and L. Huang, “Pimbeam: Efficient regular path queries over graph database using processing-in-memory,” IEEE Transactions on Parallel and Distributed Systems, 2025. [Online]. Available: https://doi.org/10.1109/TPDS.2025.3547365
- [32] Y. Kwon, K. Vladimir, N. Kim, W. Shin, J. Won, M. Lee, H. Joo, H. Choi, G. Kim, B. An et al., “System architecture and software stack for gddr6-aim,” in 2022 IEEE Hot Chips 34 Symposium (HCS). IEEE, 2022, pp. 1–25. [Online]. Available: https://doi.org/10.1109/HCS55958.2022.9895629
- [33] Y.-C. Kwon, S. H. Lee, J. Lee, S.-H. Kwon, J. M. Ryu, J.-P. Son, O. Seongil, H.-S. Yu, H. Lee, S. Y. Kim et al., “25.4 a 20nm 6gb function-in-memory dram, based on hbm2 with a 1.2 tflops programmable computing unit using bank-level parallelism, for machine learning applications,” in 2021 IEEE International Solid-State Circuits Conference (ISSCC), vol. 64. IEEE, 2021, pp. 350–352. [Online]. Available: https://doi.org/10.1109/ISSCC42613.2021.9365862
- [34] H. Lee, G. Kim, D. Yun, I. Kim, Y. Kwon, and E. Lim, “Costeffective llm accelerator using processing in memory technology,” in 2024 IEEE Symposium on VLSI Technology and Circuits (VLSI Technology and Circuits). IEEE, 2024, pp. 1–2. [Online]. Available: https://doi.org/10.1109/VLSITechnologyandCir46783.2024.10631397
- [35] Leonid Boytsov Yury Malkov., “Hnswlib - fast approximate nearest neighbor search.” https://github.com/nmslib/hnswlib.
- [36] P. Lewis, E. Perez, A. Piktus, F. Petroni, V. Karpukhin, N. Goyal, H. K¨uttler, M. Lewis, W.-t. Yih, T. Rockt¨aschel et al., “Retrievalaugmented generation for knowledge-intensive nlp tasks,” Advances in neural information processing systems, vol. 33, pp. 9459–9474,

2020. [Online]. Available: https://proceedings.neurips.cc/paper files/ paper/2020/file/6b493230205f780e1bc26945df7481e5-Paper.pdf

- [37] C. Li, Z. Zhou, Y. Wang, F. Yang, T. Cao, M. Yang, Y. Liang, and G. Sun, “Pim-dl: Expanding the applicability of commodity dram-pims for deep learning via algorithm-system co-optimization,” in Proceedings of the 29th ACM International Conference on Architectural Support for Programming Languages and Operating Systems, Volume 2, 2024, pp. 879–896. [Online]. Available: https://doi.org/10.1145/3620665.3640376
- [38] Z. Li, X. Ke, Y. Zhu, B. Yu, B. Zheng, and Y. Gao, “Scalable graph indexing using gpus for approximate nearest neighbor search,” Proceedings of the ACM on Management of Data, vol. 3, no. 6, pp. 1–27, 2025. [Online]. Available: https://doi.org/10.1145/3769825
- [39] S. Liang, Y. Wang, Z. Yuan, C. Liu, H. Li, and X. Li, “Vstore: in-storage graph based vector search accelerator,” in Proceedings of the 59th ACM/IEEE Design Automation Conference, 2022, pp. 997–1002. [Online]. Available: https://doi.org/10.1145/3489517.3530560
- [40] C. Liu, H. Liu, D. Chen, Y. Huang, Y. Zhang, W. Xiao, X. Liao, and H. Jin, “Heterrag: Heterogeneous processing-in-memory acceleration for retrieval-augmented generation,” in Proceedings of the 52nd Annual International Symposium on Computer Architecture, 2025, pp. 884–898. [Online]. Available: https://doi.org/10.1145/3695053.3731089
- [41] H. Liu, L. Zheng, Y. Huang, C. Liu, X. Ye, J. Yuan, X. Liao, H. Jin, and J. Xue, “Accelerating personalized recommendation with cross-level near-memory processing,” in Proceedings of the 50th Annual International Symposium on Computer Architecture, 2023, pp. 1–13. [Online]. Available: https://doi.org/10.1145/3579371.3589101
- [42] Z. Liu, W. Ni, J. Leng, Y. Feng, C. Guo, Q. Chen, C. Li, M. Guo, and Y. Zhu, “Juno: optimizing high-dimensional approximate nearest neighbour search with sparsity-aware algorithm and ray-tracing core mapping,” in Proceedings of the 29th ACM International Conference on Architectural Support for Programming Languages and Operating Systems, Volume 2, 2024, pp. 549–565. [Online]. Available: https://doi.org/10.1145/3620665.3640360
- [43] Y. A. Malkov and D. A. Yashunin, “Efficient and robust approximate nearest neighbor search using hierarchical navigable small world graphs,” IEEE transactions on pattern analysis and machine intelligence, vol. 42, no. 4, pp. 824–836, 2018. [Online]. Available: https: //doi.org/10.1109/TPAMI.2018.2889473
- [44] Microsoft, “SPACEV1B,” https://github.com/microsoft/SPTAG/tree/ main/datasets/SPACEV1B, 2026.
- [45] H. Ootomo, A. Naruse, C. Nolet, R. Wang, T. Feher, and Y. Wang, “Cagra: Highly parallel graph construction and approximate nearest neighbor search for gpus,” in 2024 IEEE 40th International Conference on Data Engineering (ICDE). IEEE, 2024, pp. 4236–4247. [Online]. Available: https://doi.org/10.1109/icde60146.2024.00323


- [46] C. Ortega, Y. Falevoz, and R. Ayrignac, “Pim-ai: A novel architecture for high-efficiency llm inference,” arXiv preprint arXiv:2411.17309,

2024. [Online]. Available: https://doi.org/10.48550/arXiv.2411.17309

- [47] Samsung, “PIM-HBM,” https://github.com/SAITPublic/PIMSimulator.
- [48] J. Shi, J. Gao, J. Xia, T. B. Feh´er, and C. Long, “Gpu-native approximate nearest neighbor search with ivf-rabitq: Fast index build and search,” arXiv preprint arXiv:2602.23999, 2026. [Online]. Available: https://doi.org/10.48550/arXiv.2602.23999
- [49] SK Hynix, “AiM,” https://github.com/arkhadem/aim simulator.

- [50] Y. Song, C. Liu, R. Zhang, D. Zhu, and Z. Wang, “An efficient fpga implementation of approximate nearest neighbor search,” IEEE Transactions on Very Large Scale Integration (VLSI) Systems, 2025. [Online]. Available: https://doi.org/10.1109/TVLSI.2025.3544342
- [51] B. Tian, H. Liu, Z. Duan, X. Liao, H. Jin, and Y. Zhang, “Scalable billion-point approximate nearest neighbor search using {SmartSSDs},” in 2024 USENIX Annual Technical Conference (USENIX ATC 24), 2024, pp. 1135–1150. [Online]. Available: https://www.usenix.org/conference/atc24/presentation/tian
- [52] B. Tian, H. Liu, Y. Tang, S. Xiao, Z. Duan, X. Liao, X. Zhang, J. Zhu, and Y. Zhang, “Fusionanns: An efficient cpu/gpu cooperative processing architecture for billion-scale approximate nearest neighbor search,” arXiv preprint arXiv:2409.16576, 2024. [Online]. Available: https://doi.org/10.48550/arXiv.2409.16576
- [53] V. S. Vairale and S. Shukla, “Recommendation of food items for thyroid patients using content-based knn method,” in Data Science and Security: Proceedings of IDSCS 2020. Springer, 2020, pp. 71–77. [Online]. Available: https://doi.org/10.1007/978-981-15-5309-7 8

- [54] M. Wang, H. Wu, X. Ke, Y. Gao, Y. Zhu, and W. Zhou, “Accelerating graph indexing for anns on modern cpus,” Proceedings of the ACM on Management of Data, vol. 3, no. 3, pp. 1–29, 2025. [Online]. Available: https://doi.org/10.1145/3725260
- [55] Y. Wang, S. Li, Q. Zheng, A. Chang, H. Li, and Y. Chen, “Ems-i: An efficient memory system design with specialized caching mechanism for recommendation inference,” ACM Transactions on Embedded Computing Systems, vol. 22, no. 5s, pp. 1–22, 2023. [Online]. Available: https://doi.org/10.1145/3609384
- [56] Y. Wang, S. Li, Q. Zheng, L. Song, Z. Li, A. Chang, Y. Chen et al., “Ndsearch: Accelerating graph-traversal-based approximate nearest neighbor search through near data processing,” in 2024 ACM/IEEE 51st Annual International Symposium on Computer Architecture (ISCA). IEEE, 2024, pp. 368–381. [Online]. Available: https://doi.org/10.1109/ISCA59077.2024.00035
- [57] P. Wu, M. Xie, E. Zhao, D. Zhang, J. Wang, X. Liang, K. Ren, and Y. Chai, “Turbocharge {ANNS} on real {Processing-in-Memory} by enabling {Fine-Grained}{Per-PIM-Core} scheduling,” in 2025 USENIX Annual Technical Conference (USENIX ATC 25), 2025, pp. 1223–1241. [Online]. Available: https://www.usenix.org/conference/ atc25/presentation/wu-puqing
- [58] W. Xu, J. Chen, P.-K. Hsu, J. Kang, M. Zhou, S. Pinge, S. Yu, and T. Rosing, “Proxima: Near-storage acceleration for graph-based approximate nearest neighbor search in 3d nand,” IEEE Transactions on Computers, 2026. [Online]. Available: http: //doi.org/10.1109/tc.2026.3671718
- [59] Yahoo Japan, “Neighborhood Graph and Tree for Indexing Highdimensional Data.” https://github.com/yahoojapan/NGT.
- [60] S. Zeng, Z. Zhu, J. Liu, H. Zhang, G. Dai, Z. Zhou, S. Li, X. Ning, Y. Xie, H. Yang et al., “Df-gas: A distributed fpga-as-a-service architecture towards billion-scale graph-based approximate nearest neighbor search,” in Proceedings of the 56th Annual IEEE/ACM International Symposium on Microarchitecture, 2023, pp. 283–296. [Online]. Available: https://doi.org/10.1145/3613424.3614292
- [61] L. Zhao, L. Buonanno, A. Gajjar, J. Moon, A. Natarajan, S. Serebryakov, R. M. Roth, X. Sheng, Y. Zhang, P. Faraboschi et al., “Nl-dpe: An analog in-memory non-linear dot product engine for efficient cnn and llm inference,” arXiv preprint arXiv:2511.13950, 2025. [Online]. Available: https://doi.org/10.48550/arXiv.2511.13950
- [62] Y. Zheng, Q. Guo, A. K. Tung, and S. Wu, “Lazylsh: Approximate nearest neighbor search for multiple distance functions with a single index,” in Proceedings of the 2016 International Conference on Management of Data, 2016, pp. 2023–2037. [Online]. Available: https://doi.org/10.1145/2882903.2882930
- [63] Z. Zhu, J. Liu, G. Dai, S. Zeng, B. Li, H. Yang, and Y. Wang, “Processing-in-hierarchical-memory architecture for billionscale approximate nearest neighbor search,” in 2023 60th ACM/IEEE


Design Automation Conference (DAC). IEEE, 2023, pp. 1–6. [Online]. Available: https://doi.org/10.1109/DAC56929.2023.10247946

[64] Zilliz, “Pyglass - Graph Library for Approximate Similarity Search.” https://github.com/zilliztech/pyglass.

