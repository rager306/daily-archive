## 1 Introduction

Centrality has been a prominent measure of importance in transportation, communication, and infrastructure networks . Among the many centrality metrics proposed over the years, betweenness centrality (BC) stands out as a natural fit in applications involving shortest paths and routing . Mathematically, the BC of node $n\in N$ can be calculated as :

$$ $\mathrm{B}(n)=\sum_{s\neq n\neq t\in N}\frac{\sigma_{s,t}(n)}{\sigma_{s,t}}$ (1) $$

where $\sigma_{s,t}$ is the total number of shortest paths (also referred to as geodesics) between nodes $s$ and $t$, and $\sigma_{s,t}(n)$ is the total number of paths between the pair that pass through node $n$.

Centrality has traditionally focused on deterministic and static settings, with some recent developments in stochastic and dynamic graphs (e.g., ). In addition, nodal centrality metrics have been studied more than their group counterparts . However, in many modern applications, the network observed is neither deterministic nor static: edges can fail (e.g., flooded streets in transportation networks), capacities and other edge characteristics can change (e.g., lane closures in a transportation network), and travel times and edge costs can be random and correlated in groups (e.g., travel times in a transportation network near a car crash). Under these circumstances, the node of highest betweenness centrality is itself random and can vary substantially across different realizations of the network.

A common approach to introducing uncertainty in BC calculations is to redefine the definition of the shortest path in stochastic networks, typically via randomized or probabilistic shortest path models. We then aggregate over these paths to compute expected shortest path BC or probabilistic BC variants . The key idea is to identify candidate (in the form of expected or most probable) shortest paths between source and target pairs and measure importance by how frequently a node lies on these paths. However, this shortest path-centric approach becomes difficult once additional structural requirements are imposed, such as restricting paths to a specific subgraph or region, or enforcing other node or path level constraints .

Another widely used tool for analyzing betweenness importance is centrality based on random walk , including stationary visit measures such as PageRank , and random walk counterparts of betweenness like current flow variants .
These include walk- or flow-based betweenness within settings such as multilayer networks, dense graphs, or graphon limits, and higher order structures . Another novel approach is based on absorbing Markov chains (AMC): this moves away from the geodesic limit toward a limit where the path length is unrestricted for walkers .

However, centrality metrics that are based on random walkers quantify where a walker tends to be (or how flow spreads) under exogenously specified Markov dynamics. Consequently, high stationary visitation probability for a node, which may be heavily influenced by nodal degree or by regularization, need not align with being a node of maximum betweenness, so walk-based rankings can drift toward frequently visited nodes rather than structurally mediating ones.

Figure: Figure 1: A deterministic graph consisting of two $4$-cliques connected by a length-two bridge $1$–$5$–$6$. Nodes $\{1,2,3,4\}$ form Clique 1 and nodes $\{6,7,8,9\}$ form Clique 2; node $5$ is the unique intermediary connecting the two cliques.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/clique_example.jpg

In Figure [1](#S1.F1), the provided deterministic graph consists of two dense cliques connected only through the path $1\to 5\to 6$. Under the classical notion of BC, node $5$ is the unique maximizer since every shortest path with endpoints in different cliques has to pass through $5$. However, centrality metrics based on random walks measure long run visitation under a chosen Markov rule, and for an undirected simple random walk the stationary visit probability is proportional to node degree. Since $\deg(5)=2$ while $\deg(1)=\deg(6)=4$ (with all other clique nodes having degree $3$), node $5$ has stationary visit probability $2/28$, which is smaller than $4/28$ for nodes $1$ and $6$, so walk-based rankings favor nodes $1$ and $6$ over $5$.

Considering our context of a stochastic network, edge availability and edge weights may vary across realizations, leading to graph realizations that are disconnected. This is why centrality metrics based on random walk often rely on restarts or teleportation to remain well-defined. In a stochastic version of Figure [1](#S1.F1), the bridge edges $(1,5)$ and $(5,6)$ may be unavailable. When a bridge edge is absent, the graph becomes disconnected, and consequently paths connecting the two cliques disappear, meaning that the node of maximum centrality will vary sharply across realizations. Random walk centrality metrics then require a modeling choice. Without restarts, the walk is limited to its starting component, and no single global long-run ranking is meaningful. In the presence of restarts or teleportation, the realized connectivity and topology of the network instance is bypassed, which can blur any fragmentation effects . Moreover, computing walk- or flow-based scores over many realizations can be computationally expensive due to repeated stationary distribution calculations or Laplacian-type solves required .

### 1.1 Our contributions

We propose a unified framework for studying betweenness centrality in stochastic networks. Instead of recomputing a ranking from scratch for each realized graph, we model how the reported central node changes across these realizations. This leads to an absorbing Markov chain on the node set together with a terminal state. We call this absorbing-frequency centrality (AFC), which measures the normalized fraction of pre-absorption time spent at each node.

We define local centers (component-wise) relative to a current anchor node, addressing modeling issues that arise in possibly disconnected graphs. We introduce a component-size threshold that triggers absorption when the component including the anchor node becomes too small to support informative ranking. We also clarify why the absorbing-chain representation tracks a single reported center. In an anchor-free special case, we show that AFC admits a simple mixture representation when the post-initial center law is stationary, conditional on survival. We further characterize when this simplification breaks down under heterogeneous transition dynamics.

We further extend the framework to row-wise perturbation analysis, reward-based AFC, and Top-$k$ or structure-constrained selection, all within the same absorbing-chain representation. A row-wise Monte Carlo procedure with finite-sample guarantees makes the method implementable, and experiments on the Erdős–Rényi, Watts–Strogatz, and Les Misérables networks show that AFC identifies dominant nodes, reveals ranking sensitivity, and supports reward-aware and constrained analyses using a single estimated chain.

### 1.2 Paper roadmap

The remainder of the paper develops the necessary theory. After introducing the stochastic network model and the notions of global and component-wise betweenness centers and local Top-$k$ sets, we first construct the node-valued AMC on reported local centers and justify why this compression is mathematically well posed. Proposition [3.2](#S3.Thmtheorem2) shows that unresolved candidate correspondences do not in general induce a unique node-valued transition row, Corollary [3.4](#S3.Thmtheorem4) gives the canonical row obtained from deterministic tie-breaking, and Proposition [3.5](#S3.Thmtheorem5) explains why propagating the whole candidate set still does not remove the ambiguity unless extra structure is added. Proposition [3.8](#S3.Thmtheorem8) and Lemma [3.10](#S3.Thmtheorem10) then show that the compressed row depends only on the pushforward realized-graph law together with the absorption rule, and that it is a valid probability row. Subsection [3.3](#S3.SS3) next gives the main trajectory interpretation of AFC: Proposition [3.13](#S3.Thmtheorem13) identifies AFC with the law of a length-biased uniformly sampled pre-absorption step, and Corollary [3.15](#S3.Thmtheorem15) yields the survival-weighted decomposition that becomes the basic tool for the later sections.

Building on that representation, Section [4](#S4) studies arbitrary initial distributions in the connected anchor-free regime: Theorem [4.3](#S4.Thmtheorem3) derives the mixture formula for AFC under some assumptions, Corollary [4.5](#S4.Thmtheorem5) gives its geometric-stopping specialization, and Proposition [4.7](#S4.Thmtheorem7) provides the corresponding closed-form fundamental-matrix calculation for the canonical AMC.

Section [5](#S5) then uses this mixture formula as a benchmark, showing that row-wise perturbations typically destroy the post-initial stationarity required by Theorem [4.3](#S4.Thmtheorem3), while Remark [5.1](#S5.Thmtheorem1), the survival-weighted representation equation ([20](#S5.E20)), the first-order sensitivity expansion, and the visit-gap bounds still yield a well posed perturbation analysis and ranking-stability diagnostics.

Section [6](#S6) keeps the same AMC but changes what is rewarded: Proposition [6.1](#S6.Thmtheorem1) reduces every simulator-level reward to the same fundamental matrix formula, which is then used to develop valued local Top-$k$ summaries, pool-restricted rewards, and node and transition reward AFC without enlarging the state space. Section [7](#S7) takes the complementary viewpoint by modifying the one-step selector itself, leading to
the constrained kernels $P^{W}$ and $P^{W,\mathrm{fb}}$, the feasibility probabilities equation ([29](#S7.E29)), and the pool-mass summaries equation ([30](#S7.E30)), which extend the same AMC pipeline to report specific network structures, such as cliques or stars.

Finally, Section [8](#S8) turns the theory into an implementable procedure via Algorithms [1](#alg1) and [2](#alg2), records consistency of the row-wise Monte Carlo estimator, and gives finite-sample guarantees for estimating both the one-shot law $p$ and the full kernel $P$, together with error propagation to AFC. The full theory-to-computation pipeline and its extensions is illustrated on random graphs and on a well-studied benchmark instance in Section [9](#S9).

## 2 Problem Statement

### 2.1 Network states and central node

Let $G(V,E)$ be a base topology, with $|V|=n$. A network state is any random object $Y$ that produces a working graph $G(Y)$: $Y$ may encode edge availability $A\subseteq E$ and/or positive edge weights (e.g., representing travel times) $\tau\in\mathbb{R}_{+}^{|E|}$, possibly correlated across edges and over time.

For each realization $Y$, compute weighted shortest-path distances and classical (weighted) betweenness
$\mathrm{B}_{v}(Y)$. We also fix a deterministic tie-breaker (e.g., select the smallest-index node among ties) and then we can define the unique global central node as in equation ([2](#S2.E2)).

$$ $c(Y)\ :=\ \arg\max_{v\in V}\ \mathrm{B}_{v}(Y)\ \in V.$ (2) $$

We can also write, for convenience, that $C:=c(Y)$.

Let $H(V,E_{H},\tau)$ be a realization (working graph) with positive edge weights $\tau_{e}>0$ for each edge $e\in E$. For $s,t\in V$, let $d_{H}(s,t)$ denote the weighted shortest-path distance in $H$, with the convention $d_{H}(s,t)=+\infty$ if $t$ is unreachable from $s$. For $s\neq t$, let $\sigma_{st}(H)$ be the number of weighted shortest paths from $s$ to $t$ in $H$ (so $\sigma_{st}(H)=0$ if $d_{H}(s,t)=+\infty$), and let $\sigma_{st}(v;H)$ be the number of such shortest paths whose internal vertices include $v$ (excluding endpoints). Define the (non-normalized) weighted betweenness of $v$ on $H$ by

$$ $\mathrm{B}_{v}(H):=\sum_{\begin{subarray}{c}s,t\in V\\ s\neq t,\ s\neq v,\ t\neq v\end{subarray}}\mathbf{1}\{d_{H}(s,t)<\infty\}\,\frac{\sigma_{st}(v;H)}{\sigma_{st}(H)}.$ $$

When $d_{H}(s,t)=+\infty$, the pair $(s,t)$ contributes 0.

###### Remark 2.1 (Normalization)

We use the non-normalized betweenness definition above for ranking, hence normalization constants (e.g., division by $(n-1)(n-2)$ in the connected case, or by the number of reachable pairs) do not affect $c(H)$ as long as they do not depend on $v$.

###### Remark 2.1 (Normalization)

### 2.2 Component-wise (local) centers for disconnected realizations

The realization $H$ may be disconnected and hence comprise components. In that case, a single global argmax $c(H)$ is not representative of each component: hence, we can define a component-wise betweenness center anchored at a node.

For a realized working graph $H(V,E_{H},\tau)$, let $\mathcal{K}(H)$ denote the collection of components (maximally connected subgraphs) of $H$. For any $i\in V$, let $K(i;H)\in\mathcal{K}(H)$ be the unique component containing $i$ (so $K(i;H)=\{i\}$ if $i$ is isolated). Using the same deterministic tie-breaker as in equation ([2](#S2.E2)), define the component-wise center of $K\in\mathcal{K}(H)$ by:

$$ $c_{\mathrm{comp}}(K;H)\ :=\ \arg\max_{v\in K}\ \mathrm{B}_{v}(H)\ \in K.$ $$

Given an anchor node $i\in V$ and a realized working graph $H$, define the local center by

$$ $c_{\mathrm{loc}}(i;H)\ :=\ c_{\mathrm{comp}}(K(i;H);H)\ \in V.$ (3) $$

We can further fix a threshold $k_{\min}$ and treat realizations with $|K(i;H)|<k_{\min}$ (i.e., small enough) as having no valid continuation from anchor $i$. Motivated by , it is possible to set a component size threshold either as an absolute cutoff $k_{\min}$ or as a relative cutoff $k_{\min}=\lceil\theta|V|\rceil$ with $\theta\in\left(0,1\right)$, and treat realizations with $|K(i;H)|<k_{\min}$ as having no valid continuation from anchor $i$; this event is absorbed into $\mathcal{A}_{i}$. This avoids tie-break artifacts dominating the dynamics in highly fragmented states.

### 2.3 Multiple local centers

We may opt for a set of nodes with maximum centrality value (top-$k$ nodes), instead of a single center in each component .
For a realized working graph $H(V,E_{H},\tau)$ and a component $K\in\mathcal{K}(H)$, let $\pi_{K,H}$ denote a deterministic tie-broken ordering of the vertices in $K$ by decreasing $\mathrm{B}_{v}(H)$ (ties resolved by the same global rule, restricted to $K$). Then, the component-wise top-$k$ set is defined as

$$ $\mathrm{Top}_{k}(K;H)\ :=\ \{\pi_{K,H}(1),\dots,\pi_{K,H}(k\wedge|K|)\}\ \subseteq\ K.$ (4) $$

###### Definition 2.2 (Local top- $k$ )

Given an anchor node $i\in V$ and a realized working graph $H$, define the local top-$k$ set by

$$ $\mathrm{Top}^{\mathrm{loc}}_{k}(i;H)\ :=\ \mathrm{Top}_{k}\!\big(K(i;H);H\big)\ \subseteq\ V.$ (5) $$

When $|K(i;H)|<k_{\min}$, we treat the step as having no valid continuation from anchor $i$.

###### Remark 2.3 (Choosing $k$ vs. $k_{\min}$ )

If one component requires the top-$k$ set to contain exactly $k$ distinct nodes, it suffices to choose $k_{\min}\geq k$. Otherwise equation ([4](#S2.E4)) uses $k\wedge|K|$ and remains well-defined for any $k\geq 1$ whenever $|K|\geq k_{\min}$.

###### Definition 2.2 (Local top- k k )

###### Remark 2.3 (Choosing k k vs. k min k_{\min} )

## 3 Absorbing Markov chain on reported local centers

We summarize the stochastic network evolution by tracking only the reported local center.
The state space is $S=V\cup\{\perp\}$, where $\perp$ is an absorbing state.

If the current state is $i\in V$, we generate one new realized working graph anchored at $i$.
If this realization has no valid continuation, the chain moves to $\perp$.
Otherwise it moves to the tie-broken local center of the realized graph.

Formally, let $(\mathcal{H},\mathscr{H})$ be the measurable space of realized working graphs.
Fix the following one-step simulator throughout this section:
for each $i\in V$, there exists $(\Omega_{i},\mathcal{F}_{i},\mathbb{P}_{i}),\Pi_{i}:\Omega_{i}\to\mathcal{H},$ and $\mathcal{A}_{i}\in\mathcal{F}_{i}$.

Here $\Pi_{i}(\omega)$ is the realized working graph produced from anchor $i$, and $\mathcal{A}_{i}$ is the event of no valid continuation. In particular, $\{|K(i;\Pi_{i}(\omega))|<k_{\min}\}\subseteq\mathcal{A}_{i}$. On $\Omega_{i}\setminus\mathcal{A}_{i}$, the next reported node is $c_{\mathrm{loc}}(i;\Pi_{i}(\omega))$.

This gives an absorbing Markov chain (AMC) on $S$ with transition matrix

$$ $P=\begin{bmatrix}Q&r\\ 0&1\end{bmatrix},\qquad Q\in[0,1]^{n\times n},\quad r\in[0,1]^{n},\quad Q\mathbf{1}+r=\mathbf{1}.$ (6) $$

$Q$ is the transient block on $V$, and $r_{i}=P_{i\perp}$ is the absorption probability from $i$. Equivalently, for $i,j\in V$, $P_{ij}=\mathbb{P}_{i}\!\big(\omega\notin\mathcal{A}_{i},\ c_{\mathrm{loc}}(i;\Pi_{i}(\omega))=j\big),P_{i\perp}=\mathbb{P}_{i}(\mathcal{A}_{i})$.

The Markov property means that, conditional on the current reported node, the next step does not depend on earlier history. In the present setup, for every $t\geq 0$,

$$ $\mathbb{P}(X_{t+1}=j\mid X_{t}=i,X_{0:t-1})=\mathbb{P}_{i}\!\big(\omega\notin\mathcal{A}_{i},\ c_{\mathrm{loc}}(i;\Pi_{i}(\omega))=j\big),$ $$

and $\mathbb{P}(X_{t+1}=\perp\mid X_{t}=i,X_{0:t-1})=\mathbb{P}_{i}(\mathcal{A}_{i})$.

Assume $\rho(Q)<1$.
Then absorption occurs almost surely and the expected absorption time is finite.
Let $X_{0}\sim s\in\Delta^{n-1}$, and define the absorption time

$$ $T:=\inf\{t\geq 0:X_{t}=\perp\}.$ (7) $$

The fundamental matrix is

$$ $N:=(I-Q)^{-1}=\sum_{t\geq 0}Q^{t}.$ (8) $$

Hence the expected pre-absorption visit counts are

$$ $\mu(s):=sN,$ (9) $$

and the expected pre-absorption length is

$$ $\mathbb{E}_{s}[T]=\mu(s)\mathbf{1}.$ (10) $$

We define absorbing-frequency centrality (AFC) as in equation ([11](#S3.E11)).

$$ $b(s):=\frac{\mu(s)}{\mu(s)\mathbf{1}}\in\Delta^{n-1}.$ (11) $$

Thus $b_{v}(s)$ is the fraction of pre-absorption steps (including $t=0$) spent at node $v$.

###### Remark 3.1 (Temporal dependence and state augmentation)

If the process $(X_{t})$ is not Markov because the network has additional memory beyond the current reported node, one can enlarge the state space by adding an environment variable $Z_{t}$. The resulting process is on $(V\times\mathcal{Z})\cup\{\perp\}$, and the same occupation and absorption constructions still apply on this enlarged transient space. When $\mathcal{Z}$ is finite, the matrix formulae above remain unchanged.

###### Remark 3.1 (Temporal dependence and state augmentation)

### 3.1 Why the AMC state is a single node

The chain is node-valued, so each valid realization must produce a single next node.
Before tie-breaking, however, a realized graph may have several best candidates.

For $i\in V$ and a valid realized graph $H$ (that is, $|K(i;H)|\geq k_{\min}$), define the unresolved local co-maximizer set $\Gamma^{\max}(i;H):=\arg\max_{v\in K(i;H)}\mathrm{B}_{v}(H)\subseteq K(i;H)$.

This is the local Top-$1$ output before tie-breaking. More generally, the discussion below applies to any nonempty candidate set $\Gamma(i;H)\subseteq V$, for example $\Gamma^{\max}(i;H)$ or $\mathrm{Top}^{\mathrm{loc}}_{k}(i;H)$. Fix $i\in V$ and such a candidate correspondence $\Gamma(i;\cdot)$ on valid realizations. Assume $\Gamma(i;\cdot)$ is measurable in the sense that $\{H\in\mathcal{H}:\ u\in\Gamma(i;H)\}\in\mathscr{H}$ for every $u\in V$. An admissible selector is a measurable map $g_{i}:\mathcal{H}\to V$ such that $g_{i}(H)\in\Gamma(i;H)$ for every valid $H$. Each admissible selector induces a node-valued row by $P^{g_{i}}_{ij}:=\mathbb{P}_{i}\!\big(\omega\notin\mathcal{A}_{i},\ g_{i}(\Pi_{i}(\omega))=j\big),j\in V$, and $P^{g_{i}}_{i\perp}:=1-\sum_{j\in V}P^{g_{i}}_{ij}$.

###### Proposition 3.2 (Selector-independence and nonuniqueness)

Fix $i\in V$ and a measurable candidate correspondence $\Gamma(i;\cdot)$.

(i) If every two admissible selectors agree $\mathbb{P}_{i}$-almost surely on $\Omega_{i}\setminus\mathcal{A}_{i}$, then they induce the same node-valued row.

(ii) If $\mathbb{P}_{i}\!\big(|\Gamma(i;\Pi_{i}(\omega))|\geq 2,\ \omega\notin\mathcal{A}_{i}\big)>0$, then there exist two admissible selectors that induce different node-valued rows.

###### Proof 3.3

Proof.
Part (i) is immediate:
if $g_{i}$ and $\tilde{g}_{i}$ agree $\mathbb{P}_{i}$-almost surely on $\Omega_{i}\setminus\mathcal{A}_{i}$, then for every $j\in V$, $\mathbf{1}\{\omega\notin\mathcal{A}_{i},\ g_{i}(\Pi_{i}(\omega))=j\}=\mathbf{1}\{\omega\notin\mathcal{A}_{i},\ \tilde{g}_{i}(\Pi_{i}(\omega))=j\}$ holds $\mathbb{P}_{i}$-almost surely, so $P^{g_{i}}_{ij}=P^{\tilde{g}_{i}}_{ij}$.

For part (ii), let $E_{i}:=\{\omega\notin\mathcal{A}_{i}:\ |\Gamma(i;\Pi_{i}(\omega))|\geq 2\}$. By assumption, $\mathbb{P}_{i}(E_{i})>0$. Since $V$ is finite, there exist distinct $u,v\in V$ such that $E_{i,u,v}:=\{\omega\in E_{i}:\ u,v\in\Gamma(i;\Pi_{i}(\omega))\}$ also has positive $\mathbb{P}_{i}$-measure.

Now, fix a strict precedence order $\prec$ on $V$, and define the baseline selector $h_{i}(H):=\min_{\prec}\Gamma(i;H)$. Because $V$ is finite and the membership events $\{H:u\in\Gamma(i;H)\}$ are measurable, $h_{i}$ is also measurable. We define two admissible selectors:

$$ $g_{i}^{(u)}(H):=\begin{cases}u,&u\in\Gamma(i;H),\\ h_{i}(H),&u\notin\Gamma(i;H),\end{cases}\qquad g_{i}^{(v)}(H):=\begin{cases}v,&v\in\Gamma(i;H),\\ h_{i}(H),&v\notin\Gamma(i;H).\end{cases}$ $$

Both are measurable and satisfy $g_{i}^{(u)}(H),g_{i}^{(v)}(H)\in\Gamma(i;H)$ on valid realizations.

Consider the events

$$ $F_{u}^{(u)}:=\{\omega\notin\mathcal{A}_{i}:\ g_{i}^{(u)}(\Pi_{i}(\omega))=u\},\qquad F_{u}^{(v)}:=\{\omega\notin\mathcal{A}_{i}:\ g_{i}^{(v)}(\Pi_{i}(\omega))=u\}.$ $$

We claim that $F_{u}^{(v)}\subseteq F_{u}^{(u)}$.
Indeed, if $\omega\in F_{u}^{(v)}$, then $g_{i}^{(v)}(\Pi_{i}(\omega))=u$.
This is impossible when $v\in\Gamma(i;\Pi_{i}(\omega))$, because in that case $g_{i}^{(v)}$ would output $v$.
Hence $v\notin\Gamma(i;\Pi_{i}(\omega))$, so $g_{i}^{(v)}(\Pi_{i}(\omega))=h_{i}(\Pi_{i}(\omega))=u$.
In particular, $u\in\Gamma(i;\Pi_{i}(\omega))$, and therefore $g_{i}^{(u)}(\Pi_{i}(\omega))=u$.
So $F_{u}^{(v)}\subseteq F_{u}^{(u)}$.

Moreover, the inclusion is strict on $E_{i,u,v}$: for every $\omega\in E_{i,u,v}$, $g_{i}^{(u)}(\Pi_{i}(\omega))=u,g_{i}^{(v)}(\Pi_{i}(\omega))=v\neq u$. Thus $E_{i,u,v}\subseteq F_{u}^{(u)}\setminus F_{u}^{(v)}$, and since $\mathbb{P}_{i}(E_{i,u,v})>0$, $\mathbb{P}_{i}(F_{u}^{(u)})>\mathbb{P}_{i}(F_{u}^{(v)})$. Equivalently, $P^{g_{i}^{(u)}}_{iu}>P^{g_{i}^{(v)}}_{iu}$. So the two induced rows are different.

###### Corollary 3.4 (Deterministic tie-breaking gives a canonical row)

Fix a strict total precedence order $\prec$ on $V$ (for example, increasing node index). Then

$$ $c_{\mathrm{loc}}(i;H):=\min_{\prec}\Gamma^{\max}(i;H)$ $$

is an admissible selector. It therefore defines a canonical node-valued row, and hence a well-defined node-valued AMC on $V\cup\{\perp\}$.

###### Proposition 3.5

Let $A\subseteq V$ with $|A|\geq 2$.
If there exist $u,v\in A$ such that $P_{u\cdot}\neq P_{v\cdot}$, then $A$ does not determine a unique next-step row on $V\cup\{\perp\}$. For every $\lambda\in[0,1]$, $R^{(\lambda)}_{A\cdot}:=\lambda P_{u\cdot}+(1-\lambda)P_{v\cdot}$ is a valid probability row. Hence propagating the whole candidate set requires extra structure, such as a selector, explicit weights on the elements of $A$, or a larger state space.

###### Proof 3.6

Proof.
Each $R^{(\lambda)}_{A\cdot}$ is a convex combination of probability rows, so it is again a probability row.
Since $P_{u\cdot}\neq P_{v\cdot}$, at least one coordinate differs, and therefore $R^{(\lambda)}_{A\cdot}$ depends nontrivially on $\lambda$.

Proposition [3.5](#S3.Thmtheorem5) shows that unresolved candidate sets cannot in general be propagated within the same node-valued AMC. Unless all members of the set have identical outgoing rows, the set does not determine a unique transition law. Thus, deterministic tie-breaking is necessary: it is the mechanism that makes the compressed Markov model, and therefore AFC, well-defined.

Proposition [3.2](#S3.Thmtheorem2) does not state that only the highest-ranked node is meaningful. It only states that an unresolved candidate set does not by itself specify a unique node-valued AMC. For any fixed rank $r$, one may instead define $c_{\mathrm{loc}}^{(r)}(i;H):=\pi_{K(i;H),H}(r)$ whenever $r\leq|K(i;H)|$, and study the corresponding node-valued process. Alternatively, one may keep the full Top-$k$ output and work with a set-valued or tuple-valued state space.

###### Proposition 3.2 (Selector-independence and nonuniqueness)

###### Proof 3.3

###### Corollary 3.4 (Deterministic tie-breaking gives a canonical row)

###### Proposition 3.5

###### Proof 3.6

### 3.2 Well-posedness: order/branch invariance and row normalization

We may reach the same realized graph through a different reveal order. The compressed transition row should therefore depend only on the law of the realized graph and on the absorption rule, not on how that graph came to be generated.

###### Definition 3.7 (Pushforward of the one-step simulator)

First, fix $i\in V$. Then, the pushforward distribution of the realized graph is $\nu_{i}:=\mathbb{P}_{i}\circ\Pi_{i}^{-1}$.

Figure: Figure 2: Illustration of the pushforward compression from the branching one-step simulator to the node-valued absorbing Markov chain. For a fixed anchor $i$, each branch yields a realized working graph $H=\Pi_{i}(\omega)$. Valid realizations are mapped to the tie-broken local center $c_{\mathrm{loc}}(i;H)\in V$, while realizations in $\mathcal{A}_{i}$ are mapped to the absorbing state $\perp$. Carrying out this compression for each anchor yields the compressed AMC on $V\cup\{\perp\}$. Solid arrows denote transient transitions and dashed arrows denote absorption; only selected positive-probability transitions are shown.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/31821775245585_.pic.jpg

Figure [2](#S3.F2) gives a schematic view of the compression underlying Definition [3.7](#S3.Thmtheorem7). For a fixed current anchor $i$, the internal randomness of the one-step simulator can be represented by a branching realization tree, whose terminal branches produce realized working graphs $H=\Pi_{i}(\omega)$. The detailed branch structure is then collapsed by sending each valid realization to its reported local center $c_{\mathrm{loc}}(i;H)$, and each invalid realization in $\mathcal{A}_{i}$ to the absorbing state $\perp$. The resulting description is node-valued and forgets the reveal order itself. The next proposition makes this precise by showing that the compressed transition law is determined by the pushforward law $\nu_{i}$ of realized graphs together with the conditional absorption function $a_{i}(\cdot)$, rather than by the particular branching representation used to generate the same graph law.

###### Proposition 3.8 (Order/branch invariance of the compressed step)

For $i,j\in V$, define

$$ $P_{ij}:=\mathbb{P}_{i}\!\big(\omega\notin\mathcal{A}_{i},\ c_{\mathrm{loc}}(i;\Pi_{i}(\omega))=j\big),\qquad P_{i\perp}:=\mathbb{P}_{i}(\mathcal{A}_{i}).$ (12) $$

Let $a_{i}(H):=\mathbb{E}_{i}[\mathbf{1}_{\mathcal{A}_{i}}\mid\Pi_{i}=H]$, that is, a measurable version of the conditional absorption probability given the realized graph. Then the row $(P_{ij})_{j\in V\cup\{\perp\}}$ is determined by $\nu_{i}$ together with $a_{i}(\cdot)$. Hence any two generation procedures that induce the same $\nu_{i}$ and the same conditional absorption function $a_{i}(\cdot)$ give the same AMC row for $i$. If $\mathcal{A}_{i}\in\sigma(\Pi_{i})$, then $a_{i}(H)\in\{0,1\}$ $\nu_{i}$-almost surely, and the row depends only on $\nu_{i}$ and the graph-based absorption rule.

###### Proof 3.9

Proof.
Since $c_{\mathrm{loc}}(i;\Pi_{i}(\omega))$ depends only on the realized graph $H=\Pi_{i}(\omega)$, the tower property gives

$$ $\displaystyle P_{ij}=\mathbb{E}_{i}\!\Big[\mathbf{1}\{c_{\mathrm{loc}}(i;\Pi_{i})=j\}\,\mathbf{1}\{\omega\notin\mathcal{A}_{i}\}\Big]$ $\displaystyle=\mathbb{E}_{i}\!\Big[\mathbf{1}\{c_{\mathrm{loc}}(i;\Pi_{i})=j\}\,\mathbb{E}_{i}[\mathbf{1}_{\Omega_{i}\setminus\mathcal{A}_{i}}\mid\Pi_{i}]\Big]$ $\displaystyle=\int_{\mathcal{H}}\mathbf{1}\{c_{\mathrm{loc}}(i;H)=j\}\,\bigl(1-a_{i}(H)\bigr)\,d\nu_{i}(H),$ $$

where $a_{i}(H)=\mathbb{P}_{i}(\mathcal{A}_{i}\mid\Pi_{i}=H)\in[0,1]$. Thus $P_{ij}$ depends only on $\nu_{i}$ and $a_{i}(\cdot)$. Similarly, $P_{i\perp}=\int_{\mathcal{H}}a_{i}(H)\,d\nu_{i}(H)$.

###### Lemma 3.10 (Row normalization)

For each $i\in V$, $P_{ii}+\sum_{j\in V,\ j\neq i}P_{ij}+P_{i\perp}=1$.

###### Proof 3.11

Proof.
The events $E_{ij}:=\{\omega\notin\mathcal{A}_{i},\ c_{\mathrm{loc}}(i;\Pi_{i}(\omega))=j\},j\in V$, together with $E_{i\perp}:=\mathcal{A}_{i}$, are pairwise disjoint and their union is $\Omega_{i}$. Therefore,

$$ $1=\mathbb{P}_{i}(\Omega_{i})=\sum_{j\in V}\mathbb{P}_{i}(E_{ij})+\mathbb{P}_{i}(E_{i\perp})=\sum_{j\in V}P_{ij}+P_{i\perp}.$ $$

###### Definition 3.7 (Pushforward of the one-step simulator)

###### Proposition 3.8 (Order/branch invariance of the compressed step)

###### Proof 3.9

###### Lemma 3.10 (Row normalization)

###### Proof 3.11

### 3.3 General equivalence via uniform pre-absorption sampling

We now give a trajectory interpretation of AFC.
The idea is simple: AFC is the distribution of a uniformly chosen pre-absorption step, but under a length-biased law on sample paths.

###### Definition 3.12 (Length-biased uniform pre-absorption step)

Let $(X_{t})_{t\geq 0}$ be any process on $V\cup\{\perp\}$ with absorption time $T$ as in equation ([7](#S3.E7)). Assume that $T<\infty$ almost surely and $\mathbb{E}_{s}[T]<\infty$ under $X_{0}\sim s$.
Define $\widehat{\mathbb{P}}_{s}$ on $(\Omega\times\mathbb{Z}_{\geq 0},\,\mathcal{F}\otimes 2^{\mathbb{Z}_{\geq 0}})$ :

$$ $\widehat{\mathbb{P}}_{s}(B):=\frac{1}{\mathbb{E}_{s}[T]}\,\mathbb{E}_{s}\!\left[\sum_{t=0}^{T-1}\mathbf{1}\{(\omega,t)\in B\}\right],\qquad B\in\mathcal{F}\otimes 2^{\mathbb{Z}_{\geq 0}}.$ (13) $$

Equivalently, first sample a path with probability proportional to its pre-absorption length $T(\omega)$, and then choose $U\sim\mathrm{Unif}\{0,\dots,T(\omega)-1\}$. Under $\widehat{\mathbb{P}}_{s}$, the random node $X_{U}$ is a uniformly chosen pre-absorption step.
Moreover, $\widehat{\mathbb{P}}_{s}(U=t)=\frac{\mathbb{P}_{s}(T>t)}{\mathbb{E}_{s}[T]}$.

###### Proposition 3.13 (Uniform-step equivalence)

Let $\mu_{v}(s):=\mathbb{E}_{s}\!\Big[\sum_{t=0}^{T-1}\mathbf{1}\{X_{t}=v\}\Big],b_{v}(s):=\frac{\mu_{v}(s)}{\mathbb{E}_{s}[T]}$. Then, under $\widehat{\mathbb{P}}_{s}$ from Definition [3.12](#S3.Thmtheorem12), for every $v\in V$,

$$ $b_{v}(s)=\widehat{\mathbb{P}}_{s}(X_{U}=v)=\frac{\sum_{t\geq 0}\mathbb{P}_{s}(X_{t}=v,\ T>t)}{\sum_{t\geq 0}\mathbb{P}_{s}(T>t)}.$ (14) $$

###### Proof 3.14

Proof.
Apply Definition [3.12](#S3.Thmtheorem12) to the event $\{(\omega,t):X_{t}=v\}$. This gives

$$ $\widehat{\mathbb{P}}_{s}(X_{U}=v)=\frac{1}{\mathbb{E}_{s}[T]}\,\mathbb{E}_{s}\!\Big[\sum_{t=0}^{T-1}\mathbf{1}\{X_{t}=v\}\Big]=\frac{\mu_{v}(s)}{\mathbb{E}_{s}[T]}=b_{v}(s).$ $$

Also, $\mu_{v}(s)=\sum_{t\geq 0}\mathbb{P}_{s}(X_{t}=v,\ T>t),\mathbb{E}_{s}[T]=\sum_{t\geq 0}\mathbb{P}_{s}(T>t)$, which yields equation ([14](#S3.E14)).

###### Corollary 3.15 (Survival-weighted decomposition)

Let $w_{t}:=\frac{\mathbb{P}_{s}(T>t)}{\mathbb{E}_{s}[T]}$. Whenever $\mathbb{P}_{s}(T>t)>0$, define $\pi_{t}(v):=\mathbb{P}_{s}(X_{t}=v\mid T>t)$. When $\mathbb{P}_{s}(T>t)=0$, define $\pi_{t}$ arbitrarily.
Then $w_{t}\geq 0$, $\sum_{t\geq 0}w_{t}=1$, and

$$ $b_{v}(s)=\sum_{t\geq 0}w_{t}\,\pi_{t}(v).$ (15) $$

###### Proof 3.16

Proof.
From equation ([14](#S3.E14)), $b_{v}(s)=\frac{\sum_{t\geq 0}\mathbb{P}_{s}(T>t)\,\mathbb{P}_{s}(X_{t}=v\mid T>t)}{\mathbb{E}_{s}[T]}=\sum_{t\geq 0}w_{t}\,\pi_{t}(v)$, where terms with $\mathbb{P}_{s}(T>t)=0$ have weight $w_{t}=0$ and therefore do not matter.

###### Remark 3.17 (Within-path uniform sampling vs. length bias)

If one samples a path under the original law $\mathbb{P}_{s}$ and only then chooses $U\sim\mathrm{Unif}\{0,\dots,T-1\}$, the resulting distribution is $\mathbb{E}_{s}\!\Big[\frac{1}{T}\sum_{t=0}^{T-1}\mathbf{1}\{X_{t}=v\}\Big]$, which need not equal $b_{v}(s)$. The length bias in equation ([13](#S3.E13)) is therefore essential for equation ([14](#S3.E14)).

###### Definition 3.12 (Length-biased uniform pre-absorption step)

###### Proposition 3.13 (Uniform-step equivalence)

###### Proof 3.14

###### Corollary 3.15 (Survival-weighted decomposition)

###### Proof 3.16

###### Remark 3.17 (Within-path uniform sampling vs. length bias)

## 4 AMC under an arbitrary initial distribution

In this section, we discuss results starting from an arbitrary initial distribution. In general, under component-wise centers, the next reported center depends on the current anchor $i$ through $c_{\mathrm{loc}}(i;H)$. Therefore the transient kernel $Q$ need not have identical rows.

This section studies a simpler connected/anchor-free regime. In that regime, conditional on survival, every post-initial reported center has the same distribution $p$, independent of the current anchor. The main consequence is that AFC becomes a simple mixture of the initial distribution $s$ and this common law $p$.

Let $\mathcal{L}$ be the law of one network state $Y$ (random topology and/or positive edge weights, possibly correlated within a step). Define the one-shot center distribution by

$$ $p_{v}:=\mathbb{P}_{Y\sim\mathcal{L}}(c(Y)=v),\qquad p\in\Delta^{n-1}.$ (16) $$

Fix an initial distribution $s\in\Delta^{n-1}$. Let $(X_{t})$ be the reported-center process on $V\cup\{\perp\}$ with absorption time $T$ as in equation ([7](#S3.E7)), started from $X_{0}\sim s$. We continue to write $\pi_{t}(v):=\mathbb{P}_{s}(X_{t}=v\mid T>t)$, and $w_{t}:=\frac{\mathbb{P}_{s}(T>t)}{\mathbb{E}_{s}[T]}$, as in Corollary [3.15](#S3.Thmtheorem15). We assume:

(i)(S1) Stationary post-initial law conditional on survival. There exists $p\in\Delta^{n-1}$ such that $\pi_{t}=p$ for every $t\geq 1$.

(ii)(S2) Finite expected absorption time, which represent $T<\infty\text{ a.s.},1\leq\mathbb{E}_{s}[T]<\infty.$

###### Remark 4.1 (When does (S1) hold?)

A sufficient condition is that realized working graphs are almost surely connected, each realization has a unique center (after employing the tie-breaker), and for every $t\geq 1$ the random center $X_{t}=c(Y_{t})$ has the same law $p$. Here $p=(p_{v})_{v\in V}\in\Delta^{n-1}$ denotes the common conditional distribution of the reported center after the initial step, that is, $p_{v}=\mathbb{P}_{s}(X_{t}=v\mid T>t)$ for every $t\geq 1$ and $v\in V$. For example, this holds when $(Y_{t})_{t\geq 1}$ are i.i.d. with law $\mathcal{L}$. In that case, $p$ is exactly the one-shot distribution in equation ([16](#S4.E16)).

###### Remark 4.2 (Disconnectedness of realized graphs)

If realized graphs are disconnected, then the local center $c_{\mathrm{loc}}(i;H)=c_{\mathrm{comp}}(K(i;H);H)$ depends on the anchor through the random component containing $i$. A useful summary is the conditional continuation law

$$ $p^{\mathrm{loc}}(i)_{j}:=\mathbb{P}(X_{t+1}=j\mid X_{t}=i,\ X_{t+1}\neq\perp)=\frac{Q_{ij}}{1-P_{i\perp}},\qquad\text{whenever }P_{i\perp}<1.$ $$

In general, $p^{\mathrm{loc}}(i)$ varies with $i$. Then (S1) may fail, and the mixture formula derived need not hold.
The general survival-weighted decomposition equation ([15](#S3.E15)) still holds, and AFC is still computed from the
induced AMC through equation ([9](#S3.E9))– equation ([11](#S3.E11)).

Under (S1), the general decomposition equation ([15](#S3.E15)) simplifies because only the time-0 term depends on
the initial distribution.

###### Theorem 4.3 (Mixture formula for AFC)

Under (S1) and (S2), the absorbing-frequency centrality satisfies

$$ $b(s)=\frac{1}{\mathbb{E}_{s}[T]}\,s+\Bigl(1-\frac{1}{\mathbb{E}_{s}[T]}\Bigr)p.$ (17) $$

If, in addition, $\mathbb{E}_{s}[T]>1$ and we exclude the initial step by defining

$$ $b^{(+)}(s):=\dfrac{\mathbb{E}_{s}\!\big[\sum_{t=1}^{T-1}\mathbf{1}\{X_{t}=\cdot\}\big]}{\mathbb{E}_{s}[T-1]},$ (18) $$

then $b^{(+)}(s)=p$ for every $s$.

###### Proof 4.4

Proof.
By Corollary [3.15](#S3.Thmtheorem15), $b(s)=\sum_{t\geq 0}w_{t}\,\pi_{t}$. Under (S1), we have $\pi_{0}=s$ and $\pi_{t}=p$ for all $t\geq 1$. Since $T\geq 1$ almost surely, $w_{0}=\frac{\mathbb{P}_{s}(T>0)}{\mathbb{E}_{s}[T]}=\frac{1}{\mathbb{E}_{s}[T]}$, and $\sum_{t\geq 1}w_{t}=1-w_{0}$. Therefore,
$b(s)=w_{0}s+\Bigl(\sum_{t\geq 1}w_{t}\Bigr)p=\frac{1}{\mathbb{E}_{s}[T]}s+\Bigl(1-\frac{1}{\mathbb{E}_{s}[T]}\Bigr)p,$
which is the same as equation ([17](#S4.E17)).

For the post-initial normalization, fix $v\in V$. Then
$\mathbb{E}_{s}\!\Big[\sum_{t=1}^{T-1}\mathbf{1}\{X_{t}=v\}\Big]=\sum_{t\geq 1}\mathbb{P}_{s}(T>t)\,\pi_{t}(v)=p_{v}\sum_{t\geq 1}\mathbb{P}_{s}(T>t).$
Using
$\mathbb{E}_{s}[T-1]=\sum_{t\geq 1}\mathbb{P}_{s}(T>t),$
we obtain
$\mathbb{E}_{s}\!\Big[\sum_{t=1}^{T-1}\mathbf{1}\{X_{t}=v\}\Big]=p_{v}\,\mathbb{E}_{s}[T-1].$
Dividing by $\mathbb{E}_{s}[T-1]>0$ gives $b^{(+)}(s)=p$.

Theorem [4.3](#S4.Thmtheorem3) uses only the survival-conditional laws $(\pi_{t})$; it does not require $(X_{t})$ to be
time-homogeneous Markov. A particularly simple case is geometric stopping.

###### Corollary 4.5 (Geometric stopping)

Assume $T$ is geometric with parameter $\alpha\in(0,1)$ on $\{1,2,\dots\}$, so $\mathbb{E}_{s}[T]=\alpha^{-1}$ for every s. Under (S1), $b(s)=\alpha\,s+(1-\alpha)\,p,b^{(+)}(s)=p$.

###### Proof 4.6

Proof.
Substitute $\mathbb{E}_{s}[T]=\alpha^{-1}$ into equation ([17](#S4.E17)).

###### Proposition 4.7

Consider the *canonical* AMC in which, from every transient state, the chain is absorbed with probability
$\alpha$, and otherwise survives and moves to a fresh draw from $p$, independently of the current node. Then $Q=(1-\alpha)\,\mathbf{1}p$, and $r=\alpha\,\mathbf{1}.$ Its fundamental matrix is $N=(I-Q)^{-1}=I+\frac{1-\alpha}{\alpha}\,\mathbf{1}p$.
Consequently, $sN=s+\frac{1-\alpha}{\alpha}\,p$, and $sN\mathbf{1}=\frac{1}{\alpha}$. Hence $b(s)=\alpha\,s+(1-\alpha)\,p$.

###### Proof 4.8

Proof.
Under this construction, every transient row is the same: with probability $\alpha$ the chain is absorbed, and with probability $1-\alpha$ it survives and the next node is drawn from $p$. This gives $Q=(1-\alpha)\,\mathbf{1}p$, and $r=\alpha\,\mathbf{1}.$

Since $p\mathbf{1}=1$, we have $(\mathbf{1}p)^{2}=\mathbf{1}(p\mathbf{1})p=\mathbf{1}p$, so for every $k\geq 1$, $Q^{k}=(1-\alpha)^{k}\,\mathbf{1}p$. Therefore
$N=\sum_{k\geq 0}Q^{k}=I+\sum_{k\geq 1}(1-\alpha)^{k}\,\mathbf{1}p=I+\frac{1-\alpha}{\alpha}\,\mathbf{1}p.$

Multiplying by $s$ gives $sN=s+\frac{1-\alpha}{\alpha}\,p$, and since $p\mathbf{1}=1$, $sN\mathbf{1}=s\mathbf{1}+\frac{1-\alpha}{\alpha}\,p\mathbf{1}=1+\frac{1-\alpha}{\alpha}=\frac{1}{\alpha}.$
Finally,
$b(s)=\frac{sN}{sN\mathbf{1}}=\alpha\,s+(1-\alpha)\,p.$

###### Remark 4.9 (When the mixture formula fails)

If the survival-conditional laws $\pi_{t}$ vary with $t$, or if the stopping mechanism changes the law of $X_{t}$ given
survival, then equation ([17](#S4.E17)) need not hold. In that case one should use the general decomposition
equation ([15](#S3.E15)) and the uniform pre-absorption-step interpretation from
Proposition [3.13](#S3.Thmtheorem13).

###### Remark 4.1 (When does (S1) hold?)

###### Remark 4.2 (Disconnectedness of realized graphs)

###### Theorem 4.3 (Mixture formula for AFC)

###### Proof 4.4

###### Corollary 4.5 (Geometric stopping)

###### Proof 4.6

###### Proposition 4.7

###### Proof 4.8

###### Remark 4.9 (When the mixture formula fails)

## 5 Row-wise perturbations of the AMC kernel: perturbed AFC and comparison to the mixture formula

Subsection [4](#S4) gives us a useful benchmark: when all post-initial survival-conditional laws are equal to a common distribution $p$, AFC collapses to the simple mixture formula equation ([17](#S4.E17)). That benchmark is informative, but it relies on a strong assumption. In many settings, the induced AMC kernel is only approximate: it may come from Monte Carlo construction, empirical fitting, or anchor-dependent local-center dynamics. This makes it necessary to study how AFC behaves when the transient kernel is perturbed.

The purpose of this section is therefore twofold. First, we define AFC for every admissible row-wise perturbation of the AMC kernel and show that the absorbing construction remains well-posed. Second, we explain why such perturbations typically destroy the structure behind Theorem [4.3](#S4.Thmtheorem3), so that the full survival-weighted representation must be used in place of the simple mixture form.

### 5.1 Row-wise additive uncertainty set

Let $P^{0}$ be a nominal AMC kernel as in equation ([6](#S3.E6)), with transient block $Q^{0}$. Fix row-wise perturbation radii $\varepsilon_{ij}\geq 0$ for $i,j\in V$, together with leak lower bounds $\underline{r}_{i}\in(0,1]$. For each row $i$, define

$$ $\mathcal{U}_{i}^{\pm}:=\left\{q_{i}\in\mathbb{R}_{+}^{V}:\ q_{ij}\in\bigl[(Q^{0}_{ij}-\varepsilon_{ij})_{+},\ \min\{Q^{0}_{ij}+\varepsilon_{ij},1\}\bigr]\ \ \forall j\in V,\ \sum_{j\in V}q_{ij}\leq 1-\underline{r}_{i}\right\},$ $$

where $(x)_{+}:=\max\{x,0\}$.
Assume $\mathcal{U}_{i}^{\pm}\neq\varnothing$ for every $i$; for example, it is enough that $\sum_{j\in V}(Q^{0}_{ij}-\varepsilon_{ij})_{+}\leq 1-\underline{r}_{i}$.

Each $q_{i}\in\mathcal{U}_{i}^{\pm}$ is lifted to a probability row on $S=V\cup\{\perp\}$ by setting $P_{ij}:=q_{ij}\quad(j\in V)$, and $P_{i\perp}:=1-\sum_{j\in V}q_{ij}$. Thus $P_{i\perp}\geq\underline{r}_{i}$, so every admissible perturbation retains a strictly positive absorption probability. If $r_{i}^{0}\geq\underline{r}_{i}$ for all $i$, then the nominal kernel $P^{0}$ itself belongs to the uncertainty set.

The global uncertainty set is

$$ $\mathcal{U}^{\pm}:=\left\{P=\begin{bmatrix}Q&r\\ 0&1\end{bmatrix}:\ \forall i\in V,\ (Q_{ij})_{j\in V}\in\mathcal{U}_{i}^{\pm},\ \ r_{i}=1-\sum_{j\in V}Q_{ij}\right\}.$ $$

###### Remark 5.1 (Uniform transience under leak lower bounds)

Let $\underline{r}_{\min}:=\min_{i\in V}\underline{r}_{i}$.
For any $P\in\mathcal{U}^{\pm}$, $\|Q\|_{\infty}\leq 1-\underline{r}_{\min}<1$, hence $\rho(Q)<1$.
Therefore the fundamental matrix $N(P)$ exists for every admissible kernel, and

$$ $\|N(P)\|_{\infty}\leq\sum_{t\geq 0}\|Q\|_{\infty}^{t}\leq\frac{1}{\underline{r}_{\min}}.$ (19) $$

So the absorbing construction is uniformly well posed over the whole uncertainty set.

###### Remark 5.1 (Uniform transience under leak lower bounds)

### 5.2 AFC under a fixed perturbed kernel

Fix $P\in\mathcal{U}^{\pm}$ and write $Q=Q(P)$ for its transient block.
Let $\mathbb{P}_{s}^{P}$ and $\mathbb{E}_{s}^{P}$ denote probabilities and expectations for the AMC with kernel $P$ started from $X_{0}\sim s$. As before, $N(P):=(I-Q)^{-1},\mu(s;P):=sN(P),b(s;P):=\frac{\mu(s;P)}{\mu(s;P)\mathbf{1}}$.

For this fixed kernel, $\mathbb{P}_{s}^{P}(X_{t}=\cdot,\ T>t)=sQ^{t},\mathbb{P}_{s}^{P}(T>t)=sQ^{t}\mathbf{1}$. Hence the survival-conditional law at time $t$ is $\pi_{t}(\cdot;P):=\mathbb{P}_{s}^{P}(X_{t}=\cdot\mid T>t)=\frac{sQ^{t}}{sQ^{t}\mathbf{1}},\quad t\geq 0$, with arbitrary $\pi_{t}(\cdot;P)$ when $sQ^{t}\mathbf{1}=0$. Accordingly,

$$ $b(s;P)=\sum_{t\geq 0}w_{t}(P)\,\pi_{t}(\cdot;P),\qquad w_{t}(P):=\frac{sQ^{t}\mathbf{1}}{\sum_{k\geq 0}sQ^{k}\mathbf{1}}=\frac{\mathbb{P}_{s}^{P}(T>t)}{\mathbb{E}_{s}^{P}[T]}.$ (20) $$

Thus, even after perturbation, AFC remains the survival-weighted average of the conditional laws
$\pi_{t}(\cdot;P)$.

Why is it that $b(s;P)$ generally does not reduce to the earlier derived mixture formula? Theorem [4.3](#S4.Thmtheorem3) requires a single post-initial law $p$ such that $\pi_{t}=p,~\text{for all }t\geq 1$. For a fixed AMC kernel $P$, this becomes:

$$ $\frac{sQ^{t}}{sQ^{t}\mathbf{1}}=p,~\text{for all }t\geq 1.$ (21) $$

This is a strong structural constraint: every survival-conditional law after time 0 must coincide with the same distribution $p$.

Row-wise $\pm$ perturbations typically destroy that structure by separating the rows of $Q$.
Then the family $\{\pi_{t}(\cdot;P)\}_{t\geq 1}$ generally varies with $t$ and depends on the initial distribution $s$. In this regime AFC no longer collapses to the two-point mixture equation ([17](#S4.E17)); one must instead use the full survival-weighted representation equation ([20](#S5.E20)), or equivalently the fundamental-matrix formula.

###### Remark 5.2 (Quantifying deviation from post-initial stationarity)

Fix any reference distribution $\bar{p}\in\Delta^{|V|-1}$ and define the mixture proxy
$b_{\mathrm{mix}}(s;P,\bar{p}):=w_{0}(P)\,s+(1-w_{0}(P))\,\bar{p},\quad w_{0}(P)=\frac{1}{\mathbb{E}_{s}^{P}[T]}.$
Using equation ([20](#S5.E20)) and $\pi_{0}(\cdot;P)=s$, we obtain
$b(s;P)-b_{\mathrm{mix}}(s;P,\bar{p})=\sum_{t\geq 1}w_{t}(P)\bigl(\pi_{t}(\cdot;P)-\bar{p}\bigr).$
Therefore, for any norm $\|\cdot\|$ on $\mathbb{R}^{V}$,
$\|b(s;P)-b_{\mathrm{mix}}(s;P,\bar{p})\|\leq\sum_{t\geq 1}w_{t}(P)\,\|\pi_{t}(\cdot;P)-\bar{p}\|.$

More generally, let $D_{\mathcal{F}}(\nu,\eta):=\sup_{f\in\mathcal{F}}|\nu f-\eta f|$ be an integral probability metric (IPM) . Then $D_{\mathcal{F}}\!\bigl(b(s;P),b_{\mathrm{mix}}(s;P,\bar{p})\bigr)\leq\sum_{t\geq 1}w_{t}(P)\,D_{\mathcal{F}}\!\bigl(\pi_{t}(\cdot;P),\bar{p}\bigr)$.

In particular, if $V$ is endowed with a ground metric $d$, then the $1$-Wasserstein distance $W_{1}^{d}$ is an IPM with $\mathcal{F}$ the class of $1$-Lipschitz functions, so

$$ $W_{1}^{d}\!\bigl(b(s;P),b_{\mathrm{mix}}(s;P,\bar{p})\bigr)\leq\sum_{t\geq 1}w_{t}(P)\,W_{1}^{d}\!\bigl(\pi_{t}(\cdot;P),\bar{p}\bigr).$ $$

These bounds show clearly that the error from replacing the whole time-varying family
$\{\pi_{t}(\cdot;P)\}_{t\geq 1}$ by a single law $\bar{p}$ is controlled by the survival-weighted average discrepancy
between them. The same viewpoint applies to total variation or Wasserstein discrepancies
.
Information-theoretic quantities such as Kullback–Leibler divergence can also be useful as diagnostics
, but they are not IPMs and therefore do not enter the
preceding bound in the same way.

###### Remark 5.2 (Quantifying deviation from post-initial stationarity)

### 5.3 State-independent absorption

Let us know study a useful special case. Specifically, assume $P_{i\perp}\equiv\alpha\in(0,1]$ for all $i\in V$. Then $Q=(1-\alpha)\,M$, where $M$ is row-stochastic on $V$ . In this case,

$$ $b(s;P)=\frac{s\sum_{t\geq 0}Q^{t}}{s\sum_{t\geq 0}Q^{t}\mathbf{1}}=\alpha\sum_{t\geq 0}(1-\alpha)^{t}\,sM^{t},$ (22) $$

because $M^{t}\mathbf{1}=\mathbf{1}$ implies $\sum_{t\geq 0}sQ^{t}\mathbf{1}=\sum_{t\geq 0}(1-\alpha)^{t}=\frac{1}{\alpha}$. So in the constant-hazard case, AFC is a geometrically discounted average of the nonabsorbing evolution under $M$.

The mixture benchmark equation ([17](#S4.E17)) is recovered whenever the post-initial laws are constant, namely when $sM^{t}=p$ for all $t\geq 1$. Sufficient conditions include $M=\mathbf{1}p$, or more generally $sM=p$ and $pM=p$. Without such post-initial stationarity, equation ([22](#S5.E22)) remains the correct representation.

### 5.4 First-order sensitivity of AFC

We are also interested in quantifying AFC’s sensitivity to a small perturbation of $Q$. Let $Q=Q^{0}+E,\rho(Q)<1$, and let $N^{0},\mu^{0},b^{0}$ be the quantities induced by $Q^{0}$ through equation ([8](#S3.E8)), equation ([9](#S3.E9)), and equation ([11](#S3.E11)) .
The resolvent identity gives
$N-N^{0}=NEN^{0}=N^{0}EN.$
If $E$ is small, for example if $\|N^{0}E\|<1$ in a submultiplicative norm, then
$N=N^{0}+N^{0}EN^{0}+O(\|E\|^{2}).$
Hence
$\mu-\mu^{0}\approx sN^{0}EN^{0}.$
Since
$b=\frac{\mu}{\mu\mathbf{1}},$
the corresponding first-order change in AFC is

$$ $b-b^{0}\approx\frac{sN^{0}EN^{0}}{\mu^{0}\mathbf{1}}-b^{0}\,\frac{sN^{0}EN^{0}\mathbf{1}}{\mu^{0}\mathbf{1}}.$ $$

This gives a local sensitivity approximation for the effect of a small perturbation of the transient kernel.

###### Remark 5.3 (Robust optimization viewpoint)

Given $\mathcal{U}^{\pm}$, one may report envelopes $\inf_{P\in\mathcal{U}^{\pm}}b_{v}(s;P),\sup_{P\in\mathcal{U}^{\pm}}b_{v}(s;P)$,
or optimize a scalar objective over $\mathcal{U}^{\pm}$, obtain an adversarial kernel $P^{\star}$, and then report $b(s;P^{\star})$. The point is that departures from Theorem [4.3](#S4.Thmtheorem3) are driven by the generic failure of equation ([21](#S5.E21)) under row-wise perturbations.

###### Remark 5.3 (Robust optimization viewpoint)

### 5.5 Ranking reversals and Top- 𝒌 \boldsymbol{k} nstability

Under row-wise $\pm$ perturbations of $Q$ the AFC ordering may change, including Top-$k$ sets, and may produce ranking reversals .
For $u,v\in V$, define the visit-gap

$$ $G_{uv}(P):=\mu_{u}(s;P)-\mu_{v}(s;P)=\bigl(sN(P)\bigr)_{u}-\bigl(sN(P)\bigr)_{v},$ (23) $$

where $N(P)$ is the fundamental matrix associated with $Q(P)$.
Since $b(s;P)$ is a positive normalization of $\mu(s;P)$,
$\operatorname{sign}\bigl(b_{u}(s;P)-b_{v}(s;P)\bigr)=\operatorname{sign}\bigl(G_{uv}(P)\bigr).$

A convenient robust diagnostic is

$$ $\underline{G}_{uv}:=\inf_{P\in\mathcal{U}^{\pm}}G_{uv}(P),\qquad\overline{G}_{uv}:=\sup_{P\in\mathcal{U}^{\pm}}G_{uv}(P).$ $$

If $\underline{G}_{uv}>0$, then $u$ robustly outranks $v$ over all admissible perturbations. If $\overline{G}_{uv}<0$, then $v$ robustly outranks $u$. If $0\in[\underline{G}_{uv},\overline{G}_{uv}]$, then the interval diagnostic does not certify a strict order and indicates possible ranking instability. The same logic applies to Top-$k$ stability by checking boundary pairs between the nominal Top-$k$ set and its complement.

Now assume $P^{0}\in\mathcal{U}^{\pm}$ and set $\underline{r}_{\min}:=\min_{i}\underline{r}_{i}$, and
$\bar{\varepsilon}:=\max_{i\in V}\sum_{j\in V}\varepsilon_{ij}$. Then $\|Q(P)-Q^{0}\|_{\infty}\leq\bar{\varepsilon}$ for all $P\in\mathcal{U}^{\pm}$. Using $N(P)-N^{0}=N(P)\,(Q(P)-Q^{0})\,N^{0},$ together with equation ([19](#S5.E19)), we obtain $\|\mu(s;P)-\mu(s;P^{0})\|_{\infty}=\|s(N(P)-N^{0})\|_{\infty}\leq\frac{\bar{\varepsilon}}{\underline{r}_{\min}^{2}}.$
Therefore, for all $u,v\in V$,
$|G_{uv}(P)-G_{uv}(P^{0})|\leq 2\,\|\mu(s;P)-\mu(s;P^{0})\|_{\infty}\leq\frac{2\bar{\varepsilon}}{\underline{r}_{\min}^{2}}.$

Consequently, a sufficient certificate for preserving the nominal order $u\succ v$ under all admissible perturbations is $G_{uv}(P^{0})>\frac{2\bar{\varepsilon}}{\underline{r}_{\min}^{2}}$.

If this inequality fails, the certificate is inconclusive: the nominal order may still be robust, but one can no longer guarantee it from this bound alone, and a sharper interval computation or direct optimization over $\mathcal{U}^{\pm}$ is then needed.

## 6 Multi-reward absorbing–frequency centrality for valued local Top- 𝒌 \boldsymbol{k} etweenness candidates

The previous section studied how AFC changes when the AMC kernel is perturbed. Here we keep the same AMC and instead change what is rewarded. This distinction is important in applications: uncertainty may enter through the kernel, but the decision objective may also depend on what value is attached to each realized step of the process.

The basic AFC vector $b(s)$ in equation ([11](#S3.E11)) assigns unit reward to each pre-absorption visit of the reported node. That is appropriate when one wants to summarize where the node-valued AMC spends its time. In many applications, however, the object of interest is richer than the single reported node: what matters is the total value carried by the entire local Top-$k$ candidate set produced by the realized graph at each step. Examples include total capacity, cumulative risk, aggregate demand, or combined criticality of the current local betweenness candidates.

This section is needed precisely to capture such objectives without enlarging the AMC state space. The key idea is to attach rewards to the same one-step simulator used to construct the AMC in Subsection [3.2](#S3.SS2). This lets us score the whole realized local Top-$k$ set at each step while preserving both the fundamental-matrix formula and the length-biased uniform pre-absorption interpretation from Proposition [3.13](#S3.Thmtheorem13) and Corollary [3.15](#S3.Thmtheorem15).

### 6.1 Step rewards on the one-step simulator

For each transient state $i\in V$, let $\omega_{t}$ denote the simulator draw used at time $t$ when $X_{t}=i$. Thus, conditional on $X_{t}=i$, we have $\omega_{t}\sim\mathbb{P}_{i}$, and the next state is generated by the same one-step simulator as in Subsection [3.2](#S3.SS2). Define

$$ $\mathrm{Next}_{i}(\omega):=\begin{cases}c_{\mathrm{loc}}(i;\Pi_{i}(\omega)),&\omega\notin\mathcal{A}_{i},\\ \perp,&\omega\in\mathcal{A}_{i}.\end{cases}$ $$

So $\mathrm{Next}_{i}(\omega)$ is the next reported state generated from anchor $i$.

A step reward is a family $\ell=(\ell_{i})_{i\in V}$ of measurable maps $\ell_{i}:\Omega_{i}\to\mathbb{R}_{+},i\in V$, with finite expectations $\psi_{i}:=\mathbb{E}_{i}[\ell_{i}(\omega)]<\infty,i\in V$. Write $\psi=(\psi_{i})_{i\in V}\in\mathbb{R}_{+}^{V}$.
Along an absorbed trajectory, define the accumulated pre-absorption reward by
$R^{(\ell)}:=\sum_{t=0}^{T-1}\ell_{X_{t}}(\omega_{t}),$
where $T$ is the absorption time from equation ([7](#S3.E7)).

###### Proposition 6.1 (Reward-AFC on the same AMC)

For any nonnegative integrable step reward $\ell$,

$$ $b_{\ell}(s):=\frac{\mathbb{E}_{s}[R^{(\ell)}]}{\mathbb{E}_{s}[T]}=\frac{sN\psi}{sN\mathbf{1}}.$ (24) $$

Moreover, if $U$ is the length-biased uniform pre-absorption step from [3.12](#S3.Thmtheorem12), then

$$ $b_{\ell}(s)=\widehat{\mathbb{E}}_{s}\!\big[\ell_{X_{U}}(\omega_{U})\big].$ (25) $$

###### Proof 6.2

Proof.
By the tower property,

$$ $\mathbb{E}_{s}[R^{(\ell)}]=\mathbb{E}_{s}\!\Big[\sum_{t=0}^{T-1}\ell_{X_{t}}(\omega_{t})\Big]=\mathbb{E}_{s}\!\Big[\sum_{t=0}^{T-1}\mathbb{E}\!\big[\ell_{X_{t}}(\omega_{t})\mid X_{t}\big]\Big]=\mathbb{E}_{s}\!\Big[\sum_{t=0}^{T-1}\psi_{X_{t}}\Big].$ $$

The last expression is the accumulated node reward with reward vector $\psi$, so by
equation ([8](#S3.E8))– equation ([9](#S3.E9)),
$\mathbb{E}_{s}[R^{(\ell)}]=sN\psi.$
Dividing by $\mathbb{E}_{s}[T]=sN\mathbf{1}$ gives equation ([24](#S6.E24)).

For the uniform-step representation,
$\widehat{\mathbb{E}}_{s}\!\big[\ell_{X_{U}}(\omega_{U})\big]=\frac{1}{\mathbb{E}_{s}[T]}\,\mathbb{E}_{s}\!\Big[\sum_{t=0}^{T-1}\ell_{X_{t}}(\omega_{t})\Big]=b_{\ell}(s),$
which is equation ([25](#S6.E25)).

Proposition [6.1](#S6.Thmtheorem1) is the main reduction for the rest of this section: once a reward is
attached to the one-step simulator, the resulting reward-AFC is computed from the same fundamental matrix $N$. Applying
the construction to several reward systems in parallel yields a multi-reward profile on the same AMC.

###### Proposition 6.1 (Reward-AFC on the same AMC)

###### Proof 6.2

### 6.2 Valued local Top- 𝒌 \boldsymbol{k} ets on each realized graph

We now turn to the main case of interest: rewarding the entire local Top-$k$ set produced by the realized graph at each step, not just the single reported node.

For $i\in V$ and $\omega\in\Omega_{i}$, define the realized candidate set

$$ $\mathcal{C}_{k}(i,\omega):=\begin{cases}\mathrm{Top}^{\mathrm{loc}}_{k}(i;\Pi_{i}(\omega)),&\omega\notin\mathcal{A}_{i},\\ \emptyset,&\omega\in\mathcal{A}_{i}.\end{cases}$ (26) $$

Thus, invalid draws contribute no candidates, consistent with Definition [2.2](#S2.Thmtheorem2).

Let $\gamma\in\mathbb{R}_{+}^{V}$ be a vector of node values. The step reward associated with the whole local Top-$k$ set is
$\ell_{i}^{(k,\gamma)}(\omega):=\sum_{v\in\mathcal{C}_{k}(i,\omega)}\gamma_{v},i\in V.$
Its expected one-step reward is $\psi_{i}^{(k,\gamma)}:=\mathbb{E}_{i}\!\big[\ell_{i}^{(k,\gamma)}(\omega)\big]=\mathbb{E}_{i}\!\Big[\sum_{v\in\mathcal{C}_{k}(i,\omega)}\gamma_{v}\Big].$
The corresponding valued Top-$k$ reward-AFC is
$b_{k,\gamma}(s):=b_{\ell^{(k,\gamma)}}(s)=\frac{sN\psi^{(k,\gamma)}}{sN\mathbf{1}}.$
By equation ([25](#S6.E25)),
$b_{k,\gamma}(s)=\widehat{\mathbb{E}}_{s}\!\Big[\sum_{v\in\mathcal{C}_{k}(X_{U},\omega_{U})}\gamma_{v}\Big].$
So $b_{k,\gamma}(s)$ is the average total value of the entire local Top-$k$ candidate set seen at a length-biased
uniformly sampled pre-absorption step.

If $\gamma\equiv\mathbf{1}$, then $b_{k,\mathbf{1}}(s)=\widehat{\mathbb{E}}_{s}\!\big[\,|\mathcal{C}_{k}(X_{U},\omega_{U})|\,\big]$, the expected size of the current local Top-$k$ set on that random step.

This quantity is different from a node-reward summary based only on the visited state $X_{t}$. Here the reward is attached to the whole candidate set generated by the realized graph at step $t$. This is exactly the relevant object when the application cares about the current collection of betweenness candidates rather than only the single reported center.

### 6.3 Pool-restricted valued Top- 𝒌 \boldsymbol{k} ewards

Some applications may restrict our candidate pool to $W\subseteq V$, for example an empirical union of local Top-$k$ sets: $W:=\bigcup_{H\in\mathcal{H}_{0}}\ \bigcup_{i\in V}\ \mathrm{Top}^{\mathrm{loc}}_{k}(i;H)$, for some finite $\mathcal{H}_{0}\subset\mathcal{H}$.

Given such a candidate pool $W$ and node values $\gamma\in\mathbb{R}_{+}^{V}$, let $\ell_{i}^{(k,W,\gamma)}(\omega):=\sum_{v\in\mathcal{C}_{k}(i,\omega)\cap W}\gamma_{v},i\in V$. Let $\psi_{i}^{(k,W,\gamma)}:=\mathbb{E}_{i}\!\big[\ell_{i}^{(k,W,\gamma)}(\omega)\big]$. Then the pool-restricted reward-AFC is $b_{k,W,\gamma}(s):=b_{\ell^{(k,W,\gamma)}}(s)=\frac{sN\psi^{(k,W,\gamma)}}{sN\mathbf{1}}$. Equivalently:

$$ $b_{k,W,\gamma}(s)=\widehat{\mathbb{E}}_{s}\!\Big[\sum_{v\in\mathcal{C}_{k}(X_{U},\omega_{U})\cap W}\gamma_{v}\Big].$ (27) $$

If $\gamma\equiv\mathbf{1}$, this becomes $b_{k,W,\mathbf{1}}(s)=\widehat{\mathbb{E}}_{s}\!\big[\,|\mathcal{C}_{k}(X_{U},\omega_{U})\cap W|\,\big]$, the expected number of members of the current local Top-$k$ set that fall in the pool $W$.

### 6.4 Node-based and transition-based rewards as special cases

The simulator-level reward formulation is not a separate competing model. Rather, it is a strict extension of the reward summaries already used earlier in the paper. This subsection is included for two reasons. First, it shows that the valued local Top-$k$ construction is backward-compatible with node-based and transition-based AFC. Second, it makes clear that all these summaries are computed on the same AMC through the same expected one-step reward vector and the same fundamental matrix.

Let $f:V\to\mathbb{R}_{+}$ and define $\ell_{i}^{(f)}(\omega):=f(i),i\in V$. Then $\psi_{i}=f(i)$, so equation ([24](#S6.E24)) gives

$$ $b_{f}(s)=b_{\ell^{(f)}}(s)=\frac{sNf}{sN\mathbf{1}}=b(s)\,f=\sum_{v\in V}b_{v}(s)\,f_{v}.$ (28) $$

Thus the usual node-reward AFC is recovered by taking a reward that depends only on the currently visited node.

Let $\eta:S\times S\to\mathbb{R}_{+}$ with $\eta(\perp,\cdot)=0$, and define $\ell_{i}^{(\eta)}(\omega):=\eta\bigl(i,\mathrm{Next}_{i}(\omega)\bigr),i\in V$. Then $\sum_{t=0}^{T1}\eta(X_{t},X_{t+1})=\sum_{t=0}^{T-1}\ell_{X_{t}}^{(\eta)}(\omega_{t})$. Its expected one-step reward is $\psi_{i}^{(\eta)}=\mathbb{E}_{i}\!\big[\ell_{i}^{(\eta)}(\omega)\big]=\sum_{j\in V}Q_{ij}\,\eta(i,j)+P_{i\perp}\,\eta(i,\perp)$. Therefore
$b_{\eta}(s):=\frac{\mathbb{E}_{s}\!\big[\sum_{t=0}^{T-1}\eta(X_{t},X_{t+1})\big]}{\mathbb{E}_{s}[T]}=\frac{sN\psi^{(\eta)}}{sN\mathbf{1}}=b_{\psi^{(\eta)}}(s).$
So transition rewards again reduce to the same reward-AFC formula.

A canonical example is the switching count $\eta_{\mathrm{sw}}(i,j):=\mathbf{1}\{i\in V,\ j\in V,\ j\neq i\}$, and $\eta_{\mathrm{sw}}(i,\perp):=0$, which measures the average rate at which the reported center changes before absorption.

## 7 Set-constrained Top- 𝒌 \boldsymbol{k} election and node-value summaries on the AMC

The previous section kept the AMC fixed and changed what was rewarded. In particular, the target pool $W$ entered only through the reward definition, while the one-step transition rule itself was unchanged. Here we take the complementary step: the pool constraint is enforced directly in the one-step update. This changes the induced kernel on $S=V\cup\{\perp\}$, but it does not enlarge the AMC state space in equation ([6](#S3.E6)).

Thus the distinction is as follows. In Section [6](#S6), one scores the realized local Top-$k$ set produced at each step under a fixed kernel. In the present section, one changes the reported next state itself by requiring it to lie in a target pool whenever possible, and only then computes node-wise AFC and node-value summaries from the constrained kernel. This is useful when the structural constraint is part of the reporting rule rather than part of the reward.

We focus on three objects: the row-wise feasibility of the target constraint, the resulting node-wise occupancy profile, and the total occupancy mass on the target pool.

### 7.1 Stepwise Top- 𝒌 \boldsymbol{k} iltering and constrained selection

Recall the realized candidate set $\mathcal{C}_{k}(i,\omega)$ from equation ([26](#S6.E26)). Fix a deterministic
target pool $W\subseteq V$; in applications, $W$ is often a union of prescribed shapes such as $A\cup B$.

For each $i\in V$, define the constrained selector $\mathrm{Sel}_{W}(i,\omega)$ as follows:
if $\mathcal{C}_{k}(i,\omega)\cap W\neq\emptyset$, choose the highest-ranked element of
$\mathcal{C}_{k}(i,\omega)\cap W$ under the same deterministic ranking already used to form
$\mathrm{Top}^{\mathrm{loc}}_{k}$; otherwise set $\mathrm{Sel}_{W}(i,\omega)=\perp$.
Thus every nonabsorbing constrained update reports a node in $W$.

This induces a new AMC kernel
$P^{W}=\begin{bmatrix}Q^{W}&r^{W}\\
0&1\end{bmatrix}$
on the same state space $S$, with $P^{W}_{ij}:=\mathbb{P}_{i}\!\big(\mathrm{Sel}_{W}(i,\omega)=j\big),j\in V$,
and $P^{W}_{i\perp}:=1-\sum_{j\in V}P^{W}_{ij}$. Under this hard filtering rule, absorption occurs either because the original draw is invalid or because the realized local Top-$k$ set has empty intersection with $W$. As in the previous sections, we assume the resulting kernel is absorbing; this is automatic, for example, under the same row-wise leak lower bound used earlier.

Define the one-step feasibility probability by

$$ $\xi_{i}:=\mathbb{P}_{i}\!\big(\mathcal{C}_{k}(i,\omega)\cap W\neq\emptyset\big),\qquad i\in V.$ (29) $$

Equivalently, $\xi_{i}=\sum_{j\in W}P^{W}_{ij}=1-P^{W}_{i\perp}$. So $\xi_{i}$ measures how often the target constraint can be satisfied directly from anchor $i$.

### 7.2 Node-wise values and pool occupancy under the constrained kernel

Let $N^{W}:=(I-Q^{W})^{-1}$ be the fundamental matrix of the constrained kernel, and let $b^{W}(s):=\frac{sN^{W}}{sN^{W}\mathbf{1}}$ be the corresponding node-wise AFC.

For any node-value vector $f\in\mathbb{R}_{+}^{V}$, define the constrained node-value summary by
$b_{f}^{W}(s):=\frac{sN^{W}f}{sN^{W}\mathbf{1}}=b^{W}(s)\,f.$
Thus, once the kernel is replaced by $P^{W}$, node-wise value summaries follow from exactly the same linear-algebraic pipeline as in the unconstrained AMC. This differs from the pool-restricted reward $b_{k,W,\gamma}(s)$ in equation ([27](#S6.E27)), which leaves the kernel unchanged and modifies only the reward.

A basic global diagnostic is the AFC mass on the target pool:

$$ $m_{W}(s):=\sum_{v\in W}b_{v}^{W}(s)=b_{\mathbf{1}_{W}}^{W}(s).$ (30) $$

This is the pre-absorption fraction of time spent in the target pool under the constrained kernel.
Under hard filtering, every post-initial nonabsorbing update lies in $W$, so off-pool mass can come only from initial
states outside $W$. In particular, if the initial distribution $s$ is supported on $W$, then $m_{W}(s)=1$.

When $m_{W}(s)>0$, the within-pool profile is obtained by renormalizing $\bar{b}_{v}^{W}(s):=\frac{b_{v}^{W}(s)}{m_{W}(s)},v\in W$. This gives the occupancy distribution inside the target pool itself.

For “motif”-structured targets , we treat only the chosen primitives $A,B,\dots$ as admissible targets. Some of these “motifs” have been investigated in centrality studies, such as cliques and induced stars , among others . Mixed motifs formed by combining vertices across different primitives are not counted as separate targets unless they are already contained in at least one primitive. For example, if $A=\{1,2,3\}$ and $B=\{2,3,4\}$ are the designated triangles, then $\{1,3,4\}$ is not regarded as an additional target triangle unless it is itself one of the chosen primitives (i.e., a triangle).

### 7.3 Feasibility via a fixed fallback shape

On steps where $\mathcal{C}_{k}(i,\omega)\cap W=\emptyset$, searching for alternative target witnesses may be expensive. A simpler implementation is to use a fixed fallback set $V_{\mathrm{fb}}\subseteq V$, assumed disjoint from $W$, whose induced subgraph contains an admissible shape in every realization (for example, because its internal edges are
deterministic). Fix a representative node $v_{\mathrm{fb}}\in V_{\mathrm{fb}}$.

Define the fallback selector by

$$ $\mathrm{Sel}_{W,\mathrm{fb}}(i,\omega):=\begin{cases}\text{the highest-ranked element of }\mathcal{C}_{k}(i,\omega)\cap W,&\mathcal{C}_{k}(i,\omega)\cap W\neq\emptyset,\[5.69054pt] v_{\mathrm{fb}},&\omega\notin\mathcal{A}_{i},\ \mathcal{C}_{k}(i,\omega)\cap W=\emptyset,\[2.84526pt] \perp,&\omega\in\mathcal{A}_{i}.\end{cases}$ $$

This yields a second constrained kernel
$P^{W,\mathrm{fb}}=\begin{bmatrix}Q^{W,\mathrm{fb}}&r^{W,\mathrm{fb}}\\
0&1\end{bmatrix},$
again on the same state space $S$. The feasibility probability $\xi_{i}$ from equation ([29](#S7.E29)) is unchanged: it still records how often the target pool is reached directly. The difference is that filtered-empty but otherwise valid draws are now redirected to $v_{\mathrm{fb}}$ rather than absorbed.

Assuming this fallback kernel is absorbing, let $N^{W,\mathrm{fb}}:=(I-Q^{W,\mathrm{fb}})^{-1}$, and $b^{W,\mathrm{fb}}(s):=\frac{sN^{W,\mathrm{fb}}}{sN^{W,\mathrm{fb}}\mathbf{1}}$. For any node-value vector $f\in\mathbb{R}_{+}^{V}$, define $b_{f}^{W,\mathrm{fb}}(s):=\frac{sN^{W,\mathrm{fb}}f}{sN^{W,\mathrm{fb}}\mathbf{1}}$.

The corresponding pool mass is $m_{W}^{\mathrm{fb}}(s):=\sum_{v\in W}b_{v}^{W,\mathrm{fb}}(s)$, so that $1-m_{W}^{\mathrm{fb}}(s)$ is the pre-absorption fraction of time spent outside the target pool. This off-pool mass is driven mainly by fallback detours, together with any initial mass placed outside $W$.

One may keep fallback detours rare by connecting $V_{\mathrm{fb}}$ to $V\setminus V_{\mathrm{fb}}$ only through sparse or very low-probability links. The role of the fallback set is not to represent a target structure, but to maintain a well-defined continuation rule without enlarging the AMC state space.

### 7.4 Optional censoring of fallback visits

Sometimes the fallback states are used only as an implementation device, and one wishes to remove their contribution in the computations. At the path level this corresponds to deleting indices where $X_{t}\in V_{\mathrm{fb}}$ (e.g., a segment $3\!-\!v_{\mathrm{fb}}\!-\!4$ is read as $3\!-\!4$ after deletion). Note that this pathwise operation need not preserve the Markov property. At the level of AFC, however, it amounts to conditioning the uniform pre-absorption step on avoiding the fallback set.

Specifically, if $c_{\mathrm{fb}}(s):=\sum_{u\in V\setminus V_{\mathrm{fb}}}b_{u}^{W,\mathrm{fb}}(s)>0$, let $\widetilde{b}_{v}(s):=\frac{b_{v}^{W,\mathrm{fb}}(s)}{c_{\mathrm{fb}}(s)}$, and $v\in V\setminus V_{\mathrm{fb}}$. Then $\widetilde{b}_{v}(s)=\widehat{\mathbb{P}}_{s}^{W,\mathrm{fb}}\!\big(X_{U}=v\mid X_{U}\notin V_{\mathrm{fb}}\big),v\in V\setminus V_{\mathrm{fb}}$, where $U$ is the length-biased uniform pre-absorption step for the fallback kernel. Thus censoring removes the contribution of fallback detours at the occupancy level without changing the underlying AMC construction or the downstream linear-algebraic pipeline.

Figure: Figure 3: Illustrative fallback construction: primary nodes $1$–$4$ and a fallback triangle $V_{\mathrm{fb}}=\{5,6,7\}$. The figure uses full fallback connectivity only for visualization; in applications one may instead connect the fallback shape to the rest of the network through sparse or low-probability links so that fallback detours remain rare.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/30681769910217_.pic.jpg

Figure [3](#S7.F3) illustrates this idea. The primary network consists of nodes $\{1,2,3,4\}$, while $\{5,6,7\}$ forms the fallback triangle. Assume that, at some realized step, the target pool cannot be reached through the current local Top-$k$ set. This could happen because, for example, the realized graph effectively leaves only a small component such as $\{1,2\}$. Then, the procedure will output the fallback representative $v_{\mathrm{fb}}\in\{5,6,7\}$ for that particular step.

## 8 Constructing and estimating the absorbing Markov chain from the stochastic network

The previous sections treated the absorbing Markov chain through its kernel $P$. To apply the framework, one must
construct that kernel from the underlying stochastic network model. This section makes that step explicit. We first
recall how the one-step simulator from Subsection [3.2](#S3.SS2) induces each row of $P$, and then describe a
Monte Carlo estimator when the row probabilities are not available in closed form. The same construction also supports
the reward-aware and set-constrained variants developed earlier.

### 8.1 One-step simulation and the induced row law

Fix $i\in V$. As in Subsection [3.2](#S3.SS2), let $(\Omega_{i},\mathcal{F}_{i},\mathbb{P}_{i}),\Pi_{i}:\Omega_{i}\to\mathcal{H},\mathcal{A}_{i}$ denote the one-step simulator, realized-graph map, and effective absorption event, respectively. The next state is the tie-broken local center $c_{\mathrm{loc}}(i;\Pi_{i}(\omega))$ from equation ([3](#S2.E3)) on $\Omega_{i}\setminus\mathcal{A}_{i}$, and is $\perp$ on $\mathcal{A}_{i}$; equivalently, the row law is exactly the one defined in equation ([12](#S3.E12)).

Algorithm [1](#alg1) makes this construction operational. It is the basic primitive that turns one draw from the stochastic network model into one AMC step from anchor $i$. In particular, the exact row law of the AMC is the law of the output of Algorithm [1](#alg1), and the Monte Carlo estimator in Algorithm [2](#alg2) is obtained by repeating Algorithm [1](#alg1) independently row by row.

Figure: Algorithm 1 SampleNext$(i)$: primitive one-step simulator for the AMC row from anchor $i$

Algorithm [1](#alg1) is not merely a procedural description: it is the mechanism that defines the transition row from $i$. Therefore the probabilities below are simply the output law of Algorithm [1](#alg1). For $j\in V$,

$$ $P_{ij}:=\mathbb{P}_{i}\!\big(\textsc{SampleNext}(i)=j\big),$ (31) $$

and

$$ $P_{i\perp}:=\mathbb{P}_{i}\!\big(\textsc{SampleNext}(i)=\perp\big).$ (32) $$

These coincide with equation ([12](#S3.E12)), and row normalization follows from
Lemma [3.10](#S3.Thmtheorem10).

Let $\nu_{i}$ be the pushforward law on realized graphs from Definition [3.7](#S3.Thmtheorem7), and let $a_{i}(\cdot)$ be the conditional absorption function from Proposition [3.8](#S3.Thmtheorem8). Then the row probabilities are
$P_{ij}=\int_{\mathcal{H}}\mathbf{1}\{c_{\mathrm{loc}}(i;H)=j\}\,\bigl(1-a_{i}(H)\bigr)\,d\nu_{i}(H),j\in V,$
and
$P_{i\perp}=\int_{\mathcal{H}}a_{i}(H)\,d\nu_{i}(H).$
Hence any two generation procedures inducing the same $\nu_{i}$ and $a_{i}(\cdot)$ yield the same AMC row, by Proposition [3.8](#S3.Thmtheorem8).

### 8.2 Monte Carlo estimation of the transition matrix

When the row probabilities are not available in closed form, Algorithm [1](#alg1) can be used as a sampling oracle for each transient row. Repeating it independently $M$ times for every anchor $i$ yields the empirical row estimator below. Algorithm [2](#alg2) summarizes the resulting row-wise Monte Carlo construction of the entire transition matrix.

For each $i\in V$, draw i.i.d. outputs
$Z_{i}^{(1)},\dots,Z_{i}^{(M)}\stackrel{{\scriptstyle d}}{{=}}\textsc{SampleNext}(i),$
and define
$\widehat{P}_{ij}:=\frac{1}{M}\sum_{m=1}^{M}\mathbf{1}\{Z_{i}^{(m)}=j\},j\in V\cup\{\perp\}.$

Figure: Algorithm 2 Row-wise Monte Carlo construction of the AMC transition matrix

Algorithm [2](#alg2) is the basic estimation routine used: once $\widehat{P}$ is available, AFC, reward-aware summaries, robust post-processing, and set-constrained variants all follow from the same downstream linear-algebraic computations.

For each fixed pair $(i,j)$, the strong law of large numbers gives $\widehat{P}_{ij}\xrightarrow{\mathrm{a.s.}}P_{ij}$ as $M\to\infty$.

In finite samples, one may observe $\widehat{P}_{i\perp}=0$ for some $i$, which can make
$(I-\widehat{Q})$ ill-conditioned. Standard stabilizations include pseudocount (Laplace) smoothing or enforcing a small floor on $\widehat{P}_{i\perp}$ followed by row renormalization. After stabilization one forms $\widehat{Q},\widehat{N}:=(I-\widehat{Q})^{-1},\widehat{b}(s):=\frac{s\widehat{N}}{s\widehat{N}\mathbf{1}}$.

### 8.3 Post-processing extensions on the estimated AMC

Once the nominal kernel $\widehat{P}$ has been constructed by Algorithm [2](#alg2), the extensions developed earlier in the paper can be applied without changing the basic estimation pipeline.

Starting from the nominal estimate $\widehat{P}$ produced by Algorithm [2](#alg2), one may apply the row-wise uncertainty model of Subsection [5](#S5). For a continuation-value vector $v\in\mathbb{R}^{V\cup\{\perp\}}$, the row-wise robust inner problem is

$$ $\mathcal{T}_{i}(v):=\inf_{q_{i}\in\mathcal{U}_{i}^{\pm}}\left[\sum_{j\in V}q_{ij}v_{j}+\Bigl(1-\sum_{j\in V}q_{ij}\Bigr)v_{\perp}\right],\qquad i\in V.$ $$

A scenario kernel $P^{\star}\in\mathcal{U}^{\pm}(\widehat{P})$ is then selected by the specified robust rule (row-wise linear programs, a robust Bellman operator, or another criterion), and AFC is computed from $P^{\star}$ through the same formulas as before.

Once Algorithm [2](#alg2) has produced $\widehat{P}$ and hence $\widehat{N}$, any node-value summary follows immediately: for $f\in\mathbb{R}_{+}^{V}$, $\widehat{b}_{f}(s):=\frac{s\widehat{N}f}{s\widehat{N}\mathbf{1}}$, which is the empirical version of equation ([28](#S6.E28)). More generally, for a simulator-level reward family $\ell=(\ell_{i})_{i\in V}$ from Proposition [6.1](#S6.Thmtheorem1), estimate the expected one-step reward vector $\widehat{\psi}_{i}:=\frac{1}{M}\sum_{m=1}^{M}\ell_{i}(\omega_{i}^{(m)})$, using the same row samples, and then compute $\widehat{b}_{\ell}(s):=\frac{s\widehat{N}\widehat{\psi}}{s\widehat{N}\mathbf{1}}$. This covers, in particular, the valued local Top-$k$ rewards and the pool-restricted rewards in equation ([27](#S6.E27)).

For the constrained selectors from Section [7](#S7), Algorithm [2](#alg2) is applied unchanged to the modified one-step rule, thereby yielding $\widehat{P}^{W}$ or $\widehat{P}^{W,\mathrm{fb}}$. Their corresponding AFC vectors are then $\widehat{b}^{W}(s)=\frac{s\widehat{N}^{W}}{s\widehat{N}^{W}\mathbf{1}}$, and $\widehat{b}^{W,\mathrm{fb}}(s)=\frac{s\widehat{N}^{W,\mathrm{fb}}}{s\widehat{N}^{W,\mathrm{fb}}\mathbf{1}}$. Using the same samples, estimate the feasibility probabilities in equation ([29](#S7.E29)) by $\widehat{\xi}_{i}:=\frac{1}{M}\sum_{m=1}^{M}\mathbf{1}\{\mathcal{C}_{k}(i,\omega_{i}^{(m)})\cap W\neq\emptyset\}$, and record the fallback activation rate when relevant. The corresponding pool masses $\widehat{m}_{W}(s)$ and $\widehat{m}_{W}^{\mathrm{fb}}(s)$ are obtained by summing the relevant estimated AFC coordinates over $W$.

### 8.4 Finite-sample accuracy: how many realizations are needed?

We now record conservative sample-size guarantees for two settings:
(i) estimating the one-shot law in the connected/anchor-free special case, and
(ii) estimating the full transition matrix (hence $b(s)$) in the general component-wise regime.
Errors are measured in $\|\cdot\|_{\infty}$ and $\|\cdot\|_{1}$
(total variation up to a factor $1/2$) .

#### 8.4.1 Connected/anchor-free special case: estimating the one-shot law 𝒑 \boldsymbol{p}

Recall the one-shot center law $p$ from equation ([16](#S4.E16)). Given i.i.d. samples
$Y^{(1)},\dots,Y^{(M)}\sim\mathcal{L}$, define $\widehat{p}_{v}:=\frac{1}{M}\sum_{m=1}^{M}\mathbf{1}\{c(Y^{(m)})=v\},v\in V$. For each fixed $v\in V$, the indicators are i.i.d. Bernoulli with mean $p_{v}$, so Hoeffding’s inequality
gives $\mathbb{P}\big(|\widehat{p}_{v}-p_{v}|\geq\varepsilon\big)\leq 2\exp(-2M\varepsilon^{2}).$
Applying a union bound over $v\in V$ yields
$\mathbb{P}\!\left(\|\widehat{p}-p\|_{\infty}\geq\varepsilon\right)\leq 2n\,\exp(-2M\varepsilon^{2}).$
Equivalently, a sufficient condition for $\|\widehat{p}-p\|_{\infty}<\varepsilon$
with probability at leas $1-\delta$ is

$$ $M\geq\frac{1}{2\varepsilon^{2}}\log\!\Big(\frac{2n}{\delta}\Big).$ (33) $$

A conservative $\ell_{1}$ consequence is $\|\widehat{p}-p\|_{1}\leq n\,\|\widehat{p}-p\|_{\infty}$. Hence, if
$M\geq\frac{n^{2}}{2\varepsilon^{2}}\log\!\Big(\frac{2n}{\delta}\Big),$
then $\|\widehat{p}-p\|_{1}<\varepsilon$ with probability at least $1-\delta$.

In the connected/anchor-free regime, equation ([33](#S8.E33)) is a sufficient number of generated realizations to guarantee $\|p-\widehat{p}\|_{\infty}<\varepsilon$ with confidence $1-\delta$. Sharper multinomial concentration bounds are available
, but Hoeffding plus a union bound is often sufficient.

#### 8.4.2 General component-wise regime: estimating 𝑷 \boldsymbol{P} nd propagating error to 𝒃 ​ ( 𝒔 ) \boldsymbol{b(s)}

One call to Algorithm [1](#alg1) returns a state in $V\cup\{\perp\}$ with the row law
equation ([31](#S8.E31)) – equation ([32](#S8.E32)). With the empirical estimator from
Algorithm [2](#alg2), Hoeffding’s inequality gives, for any fixed $(i,j)$,
$\mathbb{P}\big(|\widehat{P}_{ij}-P_{ij}|\geq\varepsilon\big)\leq 2e^{-2M\varepsilon^{2}}.$
A union bound over all $n(n+1)$ pairs $(i,j)$ yields
$\mathbb{P}\!\left(\max_{i\in V}\max_{j\in V\cup\{\perp\}}|\widehat{P}_{ij}-P_{ij}|\geq\varepsilon\right)\leq 2n(n+1)\exp(-2M\varepsilon^{2}).$
Therefore, a sufficient condition for $\max_{i,j}|\widehat{P}_{ij}-P_{ij}|<\varepsilon$ with probability at least $1-\delta$ is
$M\geq\frac{1}{2\varepsilon^{2}}\log\!\Big(\frac{2n(n+1)}{\delta}\Big).$

Let $Q$ be the transient block of $P$ and $\widehat{Q}$ the transient block of $\widehat{P}$.
Assume a uniform hazard lower bound
$r_{i}=P_{i\perp}\geq\underline{r}>0,i\in V,$
so that $\|Q\|_{\infty}\leq 1-\underline{r}$ and $N=(I-Q)^{-1}$ exists. Write
$N:=(I-Q)^{-1},\widehat{N}:=(I-\widehat{Q})^{-1},b(s):=\frac{sN}{sN\mathbf{1}},\widehat{b}(s):=\frac{s\widehat{N}}{s\widehat{N}\mathbf{1}}.$
To control $\|\widehat{Q}-Q\|_{\infty}$, note that
$\|\widehat{Q}-Q\|_{\infty}\leq n\,\max_{i\in V}\max_{j\in V}|\widehat{P}_{ij}-P_{ij}|.$
Hence the entrywise guarantee
$\max_{i\in V}\max_{j\in V\cup\{\perp\}}|\widehat{P}_{ij}-P_{ij}|<\frac{\varepsilon_{Q}}{n}$
implies
$\|\widehat{Q}-Q\|_{\infty}\leq\varepsilon_{Q}.$

Assume first that $\|\widehat{Q}-Q\|_{\infty}\leq\frac{\underline{r}}{2}$. Then
$\|\widehat{Q}\|_{\infty}\leq 1-\frac{\underline{r}}{2},$
so the Neumann series implies $\|N\|_{\infty}\leq\frac{1}{\underline{r}}$, and
$\|\widehat{N}\|_{\infty}\leq\frac{2}{\underline{r}}$. Using the resolvent identity
$\widehat{N}-N=(I-\widehat{Q})^{-1}-(I-Q)^{-1}=\widehat{N}(\widehat{Q}-Q)N,$
we obtain

$$ $\|\widehat{N}-N\|_{\infty}\leq\frac{2}{\underline{r}^{2}}\,\|\widehat{Q}-Q\|_{\infty}.$ (34) $$

Now let $\mu:=sN$, and $\widehat{\mu}:=s\widehat{N}$. Then
$b(s)=\frac{\mu}{\mu\mathbf{1}}$,
and $\widehat{b}(s)=\frac{\widehat{\mu}}{\widehat{\mu}\mathbf{1}}$. By equation ([34](#S8.E34)),
$\|\widehat{\mu}-\mu\|_{\infty}\leq\|\widehat{N}-N\|_{\infty},$ and $\|\widehat{\mu}-\mu\|_{1}\leq n\|\widehat{\mu}-\mu\|_{\infty}\leq\frac{2n}{\underline{r}^{2}}\,\|\widehat{Q}-Q\|_{\infty}.$
Also,
$\mu\mathbf{1}=\mathbb{E}_{s}[T]\geq 1,$
since $T\geq 1$ almost surely when $X_{0}\in V$ almost surely.
If, in addition,
$\|\widehat{Q}-Q\|_{\infty}\leq\frac{\underline{r}^{2}}{8n},$
then $\|\widehat{\mu}-\mu\|_{1}\leq 1/4$, and therefore
$\widehat{\mu}\mathbf{1}\geq\mu\mathbf{1}-\|\widehat{\mu}-\mu\|_{1}\geq\frac{3}{4}.$

A direct normalization bound now gives

$$ $\|\widehat{b}(s)-b(s)\|_{1}\leq\frac{\|\widehat{\mu}-\mu\|_{1}}{\widehat{\mu}\mathbf{1}}+\frac{|\widehat{\mu}\mathbf{1}-\mu\mathbf{1}|}{\widehat{\mu}\mathbf{1}}\leq\frac{2\|\widehat{\mu}-\mu\|_{1}}{\widehat{\mu}\mathbf{1}}\leq\frac{8}{3}\|\widehat{\mu}-\mu\|_{1}.$ $$

Using the looser but cleaner constant $8$ produces

$$ $\|\widehat{b}(s)-b(s)\|_{1}\leq\frac{8n}{\underline{r}^{2}}\,\|\widehat{Q}-Q\|_{\infty}.$ (35) $$

Hence, to guarantee $\|\widehat{b}(s)-b(s)\|_{1}\leq\varepsilon_{b}$, it suffices to set
$\varepsilon_{Q}:=\min\Bigl\{\frac{\underline{r}}{2},\frac{\underline{r}^{2}}{8n},\frac{\varepsilon_{b}\,\underline{r}^{2}}{8n}\Bigr\},$ and require the per-row sample size to satisfy
$M\geq\frac{n^{2}}{2\varepsilon_{Q}^{2}}\log\!\Big(\frac{2n(n+1)}{\delta}\Big).$
Then, with probability at least $1-\delta$,
$\max_{i,j}|\widehat{P}_{ij}-P_{ij}|<\frac{\varepsilon_{Q}}{n},$
hence $\|\widehat{Q}-Q\|_{\infty}\leq\varepsilon_{Q}$, and equation ([35](#S8.E35)) implies
$\|\widehat{b}(s)-b(s)\|_{1}\leq\varepsilon_{b}$
for $s\in\Delta^{n-1}$.

The finite-sample bounds above are stated for the raw empirical estimator $\widehat{P}$. If one applies Laplace smoothing or a floor on $\widehat{P}_{i\perp}$ before forming $\widehat{N}$, the same perturbation argument still applies after accounting for the deterministic effect of the stabilization step.

## 9 Numerical Experiments

We are ready to evaluate our computational pipeline. Each realized working graph induces a betweenness-based local center and local Top-$k$ candidate set; the resulting one-step rule is compressed into an AMC on $S=V\cup\{\perp\}$ with transition matrix equation ([6](#S3.E6)). Given an estimated kernel, we compute AFC from the fundamental matrix via equation ([8](#S3.E8))– equation ([11](#S3.E11)). The same AMC estimates are subsequently reused for robust sensitivity analysis and reward/structure-aware
summaries (multi-reward and set-based Top-$k$ developments).

### 9.1 Numerical test in Erdős–Rényi and Watts–Strogatz

We draw a base topology $G_{0}=(V,E)$ from two standard random graph models with $n=|V|=100$. For Erdős–Rényi (ER) , we sample $G_{0}\sim G(n,p)$ with $p=0.08$. For Watts–Strogatz (WS) , we use ring degree $6$ and rewiring probability $0.10$.

At each step, we generate a realized working graph $H$ by retaining each base edge independently with probability $p_{\mathrm{on}}=0.85$. All computations use unweighted shortest paths, so betweenness is computed on the realized unweighted graph. One-step updates follow Algorithm [1](#alg1) with a deterministic tie-breaker, and terminate either by exogenous stopping (absorption coin $\alpha=0.15$ per step) or by invalid continuation when the anchor’s connected component has a smaller size than $k_{\min}=5$.

For the matrix-based AMC pipeline, we estimate $\widehat{P}$ row-wise using Algorithm [2](#alg2), with $M$ independent calls to the one-step routine per transient state (baseline $M=60$). The initial distribution is uniform, $s=\tfrac{1}{n}\mathbf{1}$. To stabilize inversion of $I-\widehat{Q}$ at finite $M$, we impose a small positive floor on $\widehat{P}_{i\perp}$ (followed by renormalization) whenever sampling yields $\widehat{P}_{i\perp}=0$.

#### 9.1.1 Baseline AFC under topology uncertainty

For each ER/WS base topology, we estimate the AMC kernel $\widehat{P}$ via Algorithm [2](#alg2) under the within-step sampling model and parameters $(p_{\mathrm{on}},\alpha,k_{\min})$, then compute $\widehat{b}(s)$ using equation ([8](#S3.E8))– equation ([11](#S3.E11)). We report the Top-$5$ nodes under $\widehat{b}(s)$ and plot
the Top-$10$ values $\widehat{b}_{v}(s)$.

Figure: Figure 4: Network plot on the base topology highlighting the Top-$5$ nodes under $\widehat{b}(s)$ in ER and WS, together with a bar chart of the Top-$10$ AFC values.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/ER_amc_topnodes_600dpi.png

Figure [4](#S9.F4) reports the Top-$10$ nodes under $\widehat{b}(s)$ for the ER and WS networks. In the ER case, the top four nodes exhibit markedly larger AFC scores than the remaining nodes, while in the WS case a similar separation is observed among the top three nodes. Overall, these results indicate that the multi-step center dynamics concentrates occupancy on a small subset of nodes, even as the realized working graph varies from step to step and termination occurs via absorption.

#### 9.1.2 Robust sensitivity under row-wise kernel perturbations

We also evaluate how sensitive the AFC profile $b(s)$ is to finite-sample uncertainty in the estimated AMC
kernel $\widehat{P}$. Starting from the nominal estimate $\widehat{P}^{0}$ from
Experiment [9.1.1](#S9.SS1.SSS1), we perturb the transient block $\widehat{Q}^{0}$ row-wise and
renormalize with $P_{i\perp}=1-\sum_{j\in V}Q_{ij}$, enforcing $P_{i\perp}\geq r_{\min}$. We use
multiplicative perturbations with relative radius $\delta_{\mathrm{rel}}=0.50$ (clipped to $[0,1]$)
and set $r_{\min}=0.05$. Discrepancies are measured by KL divergence and by $W_{1}$, where the ground
metric on $V$ is shortest-path distance on the base topology.

For KL we sample $N_{\mathrm{KL}}=100$ admissible kernels and select the maximizer; for $W_{1}$ we
sample $N_{\mathrm{W1}}=100$ and select the maximizer. The resulting AFC vectors are denoted
$b^{\mathrm{KL}}(s)$ and $b^{\mathrm{W1}}(s)$.

Figure: Figure 5: For each model, we mark the Top-(5) nodes under $b^{\mathrm{KL}}(s)$ and $b^{\mathrm{W1}}(s)$ on the network (with overlaps indicated) and plot a Top-(10) bar chart comparing $b^{\mathrm{KL}}_{v}(s)$ vs.$b^{\mathrm{W1}}_{v}(s)$ on the union of the two Top-(20) sets. This highlights that perturbations can change both magnitudes and rankings because AFC depends on $(I-Q)^{-1}$, not just one-step marginals.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/ER_robust_network_bars_arrows_600dpi.png

Figure [5](#S9.F5) summarizes the robust search. For both ER and WS, the KL- and $W_{1}$-based
maximizers yield the same Top-$5$ node set, though with different AFC values. Relative to the baseline
in Figure [4](#S9.F4), WS preserves membership among the top three nodes but reorders ranks $4$–$5$:
the Top-$5$ WS nodes change from $(83,6,54,48,41)$ (Baseline) to $(6,83,54,41,48)$, swapping $83$ with $6$ and
exchanging $48$ with $41$.

#### 9.1.3 Multi-reward evaluation

Next, we illustrate the reward-aware AFC from Section [6](#S6). Given
$\widehat{P}$, all reward quantities are computed by post-processing the same AMC (no state-space
change). Let $h_{1},h_{2},h_{3}$ be the top-$3$ degree nodes in $G_{0}$, with rewards
$(R_{1},R_{2},R_{3})=(10,10,10)$. Define reward function
$f(v)=\max_{\ell\in\{1,2,3\}}R_{\ell}\,\beta^{d_{G_{0}}(v,h_{\ell})}$ with $\beta=0.60,$ setting $f(v)=0$ if $v$ is unreachable from all hubs. On the same AMC we compute $b_{f}(s)=b(s)^{\top}f$ and two transition-based reward-AFC scalars: switching intensity (strict center changes) and improvement (nonnegative one-step increases in $f$) via the reduction in Section [6](#S6).

Figure: Figure 6: For each model, we report the Top-$5$ nodes under $\widehat{b}(s)$, mark the reward hubs, and report $b_{f}(s)$ plus the switching-intensity and improvement reward-AFC scalars.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/ER_multi_reward_network_bars_arrows_600dpi.png

Figure [6](#S9.F6) reports the results. Under our degree-based construction, the high-reward hub nodes are $\{8,70,6,23,27\}$. In the ER graph, the nodes ranked $8$th-$10$th are $\{50,96,27\}$ (vs. $\{45,96,50\}$ in the baseline network). In WS, the high-reward hub nodes are $\{8,27,29,83,6\}$. AFC mass concentrates on the top three nodes ($8,27,29$) with uniformly small values thereafter, contrasting with the baseline where nodes $83$ and $6$ remain comparatively prominent.

#### 9.1.4 Structure-constrained selection via 𝟑 \boldsymbol{3} -clique target pools

We impose a motif constraint by restricting the reported next center to a target pool $W\subseteq V$
when feasible, implementing the set-based Top-$k$ filtering of Section [7](#S7). This modifies the induced AMC kernel while preserving the state space in equation ([6](#S3.E6)). We enumerate all $3$-cliques in $G_{0}$, score each clique by the sum of its node degrees, and select the top $S=8$ cliques (ties lexicographically). Let $W=\bigcup_{\ell=1}^{S}K_{\ell}$. On each non-absorbing step, after computing the local Top-$k$ set, we output the highest-ranked node in $\mathrm{Top}\text{-}k\cap W$ under the same deterministic ordering; if $\mathrm{Top}\text{-}k\cap W=\emptyset$, we fall back to a fixed $v_{\mathrm{fb}}\in W$ (the smallest id in the first selected clique). Exogenous stopping and small-component invalidity checks remain unchanged.

Figure: Figure 7: Top-$5$ nodes under the structure-constrained AFC profile, visualize them on the base network with the relevant motif edges emphasized, and plot the Top-$10$ AFC values. This highlights how motif feasibility and fallback detours can concentrate occupancy on $W$ while preserving equation ([6](#S3.E6)).
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/ER_kclique_constraint_3_combined_600dpi.png

Figure [7](#S9.F7) shows that the induced rankings differ from the other tests, while the reported centers satisfy the imposed $3$-clique structure.

### 9.2 Les Misérables co-occurrence network

We apply the AMC–AFC pipeline to the Les Misérables co-occurrence benchmark , a well-known weighted network on $|V|=77$ vertices and $|E|=254$ edges. The integer weights satisfy $w_{e}^{0}\in[1,31]$, so we set $w_{\max}=31$. To introduce
stochastic heterogeneity without changing $E$, we resample edge weights at every call to $\texttt{SampleNext}(\cdot)$: conditional on $w_{e}^{0}$, we draw
$x_{e}\sim\mathcal{N}(\mu_{e},\sigma_{e}^{2})$ with
$\mu_{e}=w_{e}^{0}+\rho_{\mu}(w_{\max}-w_{e}^{0}),\sigma_{e}=\rho_{\sigma}(w_{\max}-w_{e}^{0}),$
and set $\tilde{w}_{e}=\Pi_{[w_{e}^{0},w_{\max}]}\!\big(\mathrm{round}(x_{e})\big)$, i.e., rounded and clipped to $[w_{e}^{0},w_{\max}]$.

Because the network is connected, we enforce anchor dependence by computing betweenness only on the $r$-hop neighborhood $S_{r}(i)$ of the current state $i$ and declaring absorption when $|S_{r}(i)|<k_{\min}$. At each non-absorbing step we resample $\{\tilde{w}_{e}\}$, compute weighted betweenness on the induced subgraph on $S_{r}(i)$ (using $\tilde{w}_{e}$ as edge lengths), and select the deterministic maximizer. We stop exogenously with probability $\alpha$ per step.

The resulting AMC kernel $P^{0}$ has no closed form and is estimated row-wise via Monte Carlo, yielding $\widehat{P}^{0}$ from $M$ independent calls to $\texttt{SampleNext}(i)$ per state. We then compute AFC $b(s)$ from the fundamental matrix under a uniform initial distribution $s$, using the same stabilization (absorption floor and renormalization) as in the random-graph tests.

Figure: Figure 8: The first panel reports the baseline AFC results. The second panel compares the robust variants under KL divergence and $W_{1}$ discrepancy.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/lesmis_afc_top10_highlight5_seed42_dpi600.png

Figure: Figure 9: The first panel presents the multi-reward extensions. The second panel shows the structure-constrained setting in which betweenness centrality selection is restricted to nodes contained in the chosen $3$-clique target pool.
Refer to caption: https://arxiv.org/html/2605.14743v1/2605.14743v1/LES_multi_reward_network_bars_arrows_600dpi.png

Figure [8](#S9.F8) - LABEL:fig:test:6 summarizes the Les Misérables experiments with per-step edge-weight resampling. The upper-left panel shows the baseline occupancy profile by highlighting the Top-$5$ nodes and plotting the Top-$10$ entries of $b(s)$. The upper-right panel studies kernel robustness: starting from the Monte Carlo estimate $\widehat{P}^{0}$, we apply row-wise feasible perturbations to the transient block $Q$ and search for admissible kernels that maximize either KL divergence or $W_{1}$ distance from the baseline AFC. Unlike ER/WS (where variability is driven by random edge presence), the topology here is fixed and randomness arises from weight resampling, so the robust results reflect both admissible deviations in $Q$ and finite-sample uncertainty in $\widehat{P}^{0}$. For $W_{1}$, the ground metric is shortest-path distance on the base graph.

The lower-left panel reports the multi-reward test, combining the Top-$10$ AFC bar chart with degree-based reward hubs and reward-aware summaries (e.g., $b_{f}(s)=b(s)^{\top}f$ and transition-reward post-processing). The lower-right panel reports the structure-constrained test, where a target pool $W$ is formed from selected $3$-cliques and the one-step output is restricted to $\mathrm{Top}\text{-}k\cap W$ when feasible, with a deterministic fallback otherwise. Together, the four panels separate baseline AMC dynamics, kernel-level sensitivity, reward-aware post-processing, and clique-based constraints under a fixed topology with stochastic edge weights.

## 10 Conclusion

Our work provides a unified and computationally tractable absorbing dynamics framework for stochastic network centrality. Its main limitations are that the node-valued AMC compression is not lossless, since unresolved candidate sets and full local Top-$k$ outputs are reduced by deterministic tie-breaking, and that estimation currently relies on row-wise Monte Carlo with conservative finite-sample guarantees. Additionally, practical stabilization may also introduce bias, and better temporal dependence would require state augmentation, which could hurt its tractability.

These limitations suggest several directions for future work, including sharper non-asymptotic bounds, bias-aware analysis of stabilized estimators, uncertainty models for robust AFC optimization, and set-valued or state-augmented formulations that retain more information from local Top-$k$ structure and temporal dependence. On the applied side, the framework could be extended to problems such as resilient routing, infrastructure monitoring, uncertainty-aware prioritization, and dynamic screening in weighted social or co-occurrence networks.
