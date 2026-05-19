## Abstract

Abstract The ability of a small set of coordinated actors to manipulate opinions in online social networks poses a serious challenge to the fairness and integrity of public debate.
We investigate this problem by studying how targeted stubborn agents can shift the average opinion of a network governed by the Hegselmann–Krause bounded-confidence dynamics.
Experiments are conducted on weighted LFR benchmark networks with community structure, using multiple node-selection strategies based on degree, strength, PageRank, betweenness, k-coreness, s-coreness, and salience.
We compare static interventions, in which stubborn agents keep a fixed extreme opinion, with dynamic interventions, in which their opinion gradually evolves from moderate to extreme values.
Results show that dynamic strategies are substantially more effective than static ones, as they exploit bounded-confidence dynamics to progressively recruit intermediate agents and extend influence across the network.
In contrast, static strategies tend to create early opinion separation and therefore have a more limited reach.
We also find that while some centrality measures offer advantages in static settings, dynamic interventions can achieve strong performance even with simple or random node selection.
Overall, the study clarifies how intervention design and target selection interact in shaping collective opinions, with implications for understanding and countering manipulation in social networks.

###### Index Terms:

## 1 Introduction

Public opinion has long been shaped by communication systems that amplify selected messages, narratives, and viewpoints.
In contemporary digital environments, this process has become faster, more targeted, and more scalable because online social platforms enable the rapid dissemination of information across large and heterogeneous audiences .
As a result, the same mechanisms that support information access and civic participation can also be exploited to steer collective attention, reinforce biases, and condition opinion formation .
This dual role has made the study of influence and manipulation in online networks increasingly important, especially in contexts where the fairness and impartiality of public debate are at stake .

The effects of coordinated influence campaigns have been documented in several socially relevant domains, including public health communication during the COVID-19 pandemic , electoral and referendum campaigns such as the 2016 and 2020 U.S. elections and Brexit , and online discourse surrounding international conflicts .
More broadly, growing evidence points to a significant interplay between online influence dynamics and political polarization .

To mitigate these forms of opinion manipulation, it is first necessary to understand the mechanisms through which they operate.
This makes it possible to characterize potential threats, identify the conditions under which they are most effective, and pinpoint vulnerable regions of the underlying social network.
A large body of research has proposed agent-based models of opinion dynamics .
More recently, the growing interest in propaganda optimization in political, advertising, and commercial settings has motivated the design of targeted influence strategies.
Many of these approaches seek network configurations that maximize diffusion or rely on optimization-based metrics that may become computationally demanding on large networks .
In such cases, simpler and more scalable heuristics based on standard centrality measures may be preferable, as they require only limited structural information.
However, most existing strategies assume fixed interventions and do not explicitly consider influence policies that evolve over the course of the opinion-dynamics process.

In this work, we compare several centrality-based targeting strategies for influencing collective opinion in a social network.
To this end, we study the Hegselmann–Krause opinion-dynamics model and examine how the introduction of a fraction of stubborn agents affects the network’s average opinion under different initial conditions.
We consider both a static intervention setting, in which stubborn agents keep a fixed opinion, and a dynamic one, in which their opinion changes over time.
Our goal is to assess which centrality measures provide the most effective and robust criteria for selecting target nodes, and how their performance varies across intervention strategies and simulation settings.

The remainder of the paper is organized as follows. Section [2](#S2) reviews the main agent-based models of opinion dynamics, with particular emphasis on the Hegselmann–Krause model. Section [3](#S3) surveys influence strategies proposed in the literature and introduces a classification of intervention types. Section [4](#S4) describes the network model adopted in our experiments, presents the centrality measures considered in the analysis, and details the simulation protocol and evaluation metrics. Section [5](#S5) reports and discusses the results. Finally, Section [6](#S6) concludes the paper and outlines possible directions for future research.

## 2 Models of Opinion Dynamics

Individual opinions evolve as a result of personal predispositions and social influence.
A population can be modeled as a network of agents whose opinions change over time due to two concurrent forces: (i) an intrinsic tendency or prejudice, reflecting each agent’s private information, background, and media consumption, and (ii) social influence arising from interactions via standard communication channels, such as mass media and face-to-face conversations, as well as online social platforms. The influence intensity of these platforms can be adjusted through their algorithms.

Here, we assume that interactions occur through a simple, undirected graph $G=(V,E)$, in which multi-edges and self-loops are not permitted.
In this model, $V$ is a set of nodes representing agents and $E$ is a set of undirected edges representing the social contacts between agents. The literature discusses two broad modeling paradigms:

$\bullet$ Deterministic (differential/difference) models:
Each agent $i$ has a real-valued opinion $x_{i}(t)\in\mathbb{R}$.
In classical “averaging” models, agents update their opinions at discrete time steps, as in the French–DeGroot model and the Krause model , or continuously, as in the Abelson model .
They do so by averaging their own opinion and those of their neighbors:

$$ $x_{i}(t+1)=\sum_{j}w_{ij}x_{j}(t),\quad\dot{x}_{i}(t)=-\sum_{j}l_{ij}x_{j}(t),$ (1) $$

where $W=[w_{ij}]$ is a row-stochastic “trust” matrix and $L=[l_{ij}]=I-W$ is the corresponding Laplacian.
Under mild connectivity conditions, these models converge to a consensus state. Therefore, they cannot explain persistent disagreement among agents , which is instead the typical outcome of nonlinear couplings. For example, the Hegselmann-Krause model yields clustering of the agents rather than full consensus (see below).

$\bullet$ Stochastic (microscopic) models:
These models are often Markov chains with a finite set of opinions.
Examples include the voter model and its modifications , Galam’s majority dynamics , and the continuous-time “opinionation” model , in which each agent’s jump rates depend on their “prejudice” and the current mix of neighbors’ opinions.
These stochastic models capture spontaneous (i.e., endogenous) changes and the probabilistic nature of interpersonal influence. They also allow for the calculation of long-term expected vote shares and notions of individual “social power” .
Stochastic models naturally handle random neighbor sampling, asynchronous events, individual-level volatility, and bounded, discrete opinion sets. While they can predict fluctuations around consensus and the impact of stubborn agents (“zealots”), they typically require more sophisticated probabilistic tools, such as martingales and mean-field approximations, or large-scale simulations.

### 2.1 Hegselmann–Krause Model

This study adopts the heterogeneous Hegselmann–Krause (HK) model , which describes opinion dynamics under bounded confidence.
Each agent $i$ of the population is equipped with a continuous dynamical variable $x_{i}(t)\in[0,1]$, representing their opinion, and a fixed quantity $\epsilon_{i}$, which models their confidence.
Nodes only interact with neighbors whose opinions differ by less than their confidence range, i.e., $x_{j}\in[x_{i}-\epsilon_{i},x_{i}+\epsilon_{i}]$.
Thus, considering both the network constraint and the confidence range, the set of interactions for agent $i$ is given by:

$$ $I(i,\vec{x})=\left\{1\leq j\leq N\ |\ (i,j)\in E\ \land|x_{i}-x_{j}|\leq\epsilon_{i}\right\}.$ (2) $$

Opinions are updated synchronously at discrete time steps.
Agent $i$ adopts the average opinion of set $I(i,\vec{x})$:

$$ $x_{i}(t+1)=\frac{1}{|I(i,\vec{x}(t))|}\sum_{j\in I(i,\vec{x}(t))}x_{j}(t).$ (3) $$

We adapted the update equation ([3](#S2.E3)) to weighted networks.
More precisely, the opinions of neighbors contained in $I(i,\vec{x})$ are weighted by $w_{ji}$, i.e., the weight of the edge connecting them to node $i$.
This gives more importance to neighbors with a strong connection to the updating agent:

$$ $x_{i}(t+1)=\frac{1}{\sigma(i,\vec{x}(t))}\sum_{j\in I(i,\vec{x}(t))}w_{ji}x_{j}(t).$ (4) $$

Here, $\sigma(i,\vec{x}(t))=\sum_{j\in I(i,\vec{x}(t))}w_{ji}$ is the strength of node $i$ restricted to its neighbors contained in $I$ at time $t$.
Including this feature is essential for accurately capturing the strength of influence that characterizes real-world networks , and it is particularly important for modeling social media dynamics .

By restricting each agent’s update to the subset of neighbors whose opinions lie within their individual confidence threshold $\epsilon_{i}$, the HK model naturally generates stable opinion clusters that closely resemble real‐world community polarization .
Despite its nonlinear confidence bounds, the model is amenable to rigorous convergence analysis via fixed‐point arguments and spectral methods .
The model is known to converge in finite time to a stable configuration of opinion clusters.
It has been proven that every trajectory reaches a steady state within a polynomial number of synchronous update steps .
Subsequent analyses have characterized the cluster‐formation process itself.
In , it was shown that once opinion distances exceed the confidence bound, agents remain in distinct, non-interacting clusters.
In , the results were extended to heterogeneous confidence levels. This extension predicted both the number and composition of the final clusters based on the distribution of the individual $\epsilon_{i}$ values.

## 3 Strategies for Influencing Opinions

Since the early 2010s, we have witnessed the rapid development of influence strategies within social networks due to the exponential growth of social media and the advent of targeted online marketing.
These strategies have been used not only for marketing purposes, as in the case of the so-called “Internet Water Army” , but also in many other sectors, including politics .
Indeed, it has been shown that both the 2016 and 2020 US elections underwent significant attempts at manipulation and distortion of the results caused by fake social accounts , which inserted themselves into online debates with the aim of spreading propaganda and false news .
One recurring strategy is to transform nodes in the network into “stubborn agents” with a strong bias toward a certain opinion.
Examples of this strategy in opinion‐dynamics models can be seen in , where agents are inserted into a generalized voter model and influence their neighbors through unidirectional influence links; in , where the highest-degree nodes are targeted; and in , where two external controllers clash to sway the average opinion toward two opposite extremes.
An application of this strategy in a Hegselmann-Krause model is presented in .
Other research uses the concept of social power as defined in , as seen in and .
The latter analyzes the effects of attacks on different network communities. This aspect of opinion manipulation has been studied in , which have produced diverse results depending on the properties of the networks and the diffusion models used.
In , small perturbations are made to the edge weights within a voter model to alter the voting dynamics.
Further strategies for majority-based models can be found in , and a summary of manipulation strategies applied to deterministic and stochastic models can be found in .
Finally, some research has focused on maximizing the effects of these attacks, as seen in .

In opinion‐dynamics models, influence strategies can be classified as either soft or hard attacks, depending on whether they subtly bias interaction rules or directly impose fixed opinions through dedicated agents .

### 3.1 Soft Attacks

Soft attacks indirectly modify the dynamics to tilt the “playing field” in favor of a desired outcome.
The objective is to shift the average standalone opinion of certain agents .
Common approaches include the following:

- •
Tuning interaction parameters, such as the confidence bound $\epsilon$ in bounded‐confidence models, can be achieved through manipulative advertising or monetary incentives .
- •
Adjusting the weights $w_{ij}$ in DeGroot‐style averaging increases or decreases receptivity to particular viewpoints .
- •
Broadcasting external fields similar to mass‐media opinion sources can gradually shift all agents’ private biases over time. This can be done by adjusting the probability of interacting with a “media” node .
- •
Altering the network topology by adjusting edge weights or rewiring can amplify connectivity around pro‐target clusters or isolate dissenting groups, indirectly steering the consensus .

These interventions are typically smooth and widespread.
Although agents have complete freedom to update, the landscape of influences favors the attacker’s goals.
It is challenging to detect these subtle biases in real networks, and effectively implementing them often requires extensive global knowledge or coordination.

### 3.2 Hard Attacks

In contrast, hard attacks introduce one or more stubborn (or “zealot”) agents into the social network.
These agents have fixed opinions that do not change over time or under the influence of their neighbors’ opinions .

- •
In stochastic models, stubborn agents influence the random‐walk dynamics of opinions, causing them to converge toward the agent’s own value .
- •
In deterministic averaging frameworks, such as the DeGroot model , stubborn agents can be included in every neighbor‐average update as immovable boundary conditions .
In contrast, the Hegselmann–Krause model states that each non-stubborn agent includes a stubborn node in their update only if their confidence is sufficiently large. Thus, the influence of a stubborn agent on the population is filtered by the confidence threshold; only agents “close enough” in opinion will be pulled toward the zealot value.

## 4 Materials and Methods

### 4.1 Network Model

In this work, we implement the HK process on graphs defined by the Lancichinetti–Fortunato–Radicchi (LFR) benchmark model , which is an algorithm that generates networks with power-law distributions for both node degree and edge weight, as well as a predefined community structure. These features allow the network to closely resemble real-world networks, making them essential for modeling phenomena such as intra-community consensus, inter-community polarization, and “echo-chamber” effects in opinion dynamics .

We used the implementation provided by the authors with the following parameters to construct the desired networks: $N=1000$ or $2000$ nodes, average degree $\langle k\rangle=20$, maximum degree $k_{\max}=200$ , mixing parameter for strength $\mu_{w}=0.1$, minimum and maximum community size $c_{\min}=20$ and $c_{\max}=50$.
To build a weighted network, the algorithm first creates an unweighted network and then assigns a positive real weight to each link.
This assignment is based on two parameters, $\beta$ and $\mu_{w}$. The first parameter is set to $\beta=1.5$ by default and assigns a strength $\sigma_{i}$ to each node according to $\sigma_{i}=k_{i}^{\beta}$, which mimics the strength-degree power-law relation frequently observed in real-world weighted networks.
The second parameter, $\mu_{w}$, is then used to assign the internal strength $\sigma_{i}^{(in)}$, which is the strength directed inside the community of node $i$, as follows $\sigma_{i}^{(in)}=(1-\mu_{w})\sigma_{i}$. Using this procedure, we generated 20 network instances for each network size $N=1000$ and $2000$.

One of the network instances is shown in Fig. [1](#S4.F1) (top panel), together with the corresponding High-Salience Skeleton (bottom panel), which is a backbone containing almost all of the nodes (94%) but only less than 9% of the edges (see Sec. [4.2](#S4.SS2) for definition and details).

Figure: Figure 1: Top panel: An LFR network generated as described in Sec. [4.1](#S4.SS1) ($N=1000$): colors correspond to communities. Bottom panel: the High-Salience Skeleton (see Sec. [4.2](#S4.SS2)) of the network in the top panel.
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/networks/LFRnet_N_1000_01_draw.png

### 4.2 Network Centralities

To characterize the structural roles of the nodes in our network and guide the selection of stubborn agents, we compute several centrality measures . Each metric captures a different aspect of “importance” within the graph. By comparing these different centralities, we can evaluate the impact of connectivity-based (e.g., degree or coreness), path‐based (e.g., betweenness, salience), and recursive (e.g., PageRank) notions of importance on the ability of a small set of agents to influence overall opinion formation.

$\bullet$ Degree Centrality: It corresponds to the number of neighbors of node $i$:

$$ $d_{i}=\sum_{j=1}^{N}a_{ij},$ (5) $$

where $A=[a_{ij}]$ is the adjacency matrix, which has $a_{ij}=a_{ji}=1$ if an edge connects nodes $(i,j)$, $a_{ij}=a_{ji}=0$ otherwise ($a_{ii}=0$ $\forall i$ because self-loops are not allowed).
Nodes with a high degree have many direct connections, enabling them to communicate quickly with a large number of neighboring agents.

$\bullet$ Betweenness Centrality:
It measures the fraction of the shortest paths that include node $i$:

$$ $b_{i}=\sum^{N}_{\begin{subarray}{c}j,k=1\\ j\neq i\neq k\end{subarray}}\frac{\eta_{jk}(i)}{\eta_{jk}}.$ (6) $$

Here, $\eta_{jk}$ is the total number of shortest paths from $j$ to $k$ and $\eta_{jk}(i)$ is the number of those paths that pass through $i$.
Nodes with high betweenness act like bridges and can control the flow of information across different regions of the network.

$\bullet$ Strength: In a weighted network with weight matrix $W=[w_{ij}]$, the strength of node $i$ is defined as follows:

$$ $\sigma_{i}=\sum_{j=1}^{N}w_{ij},$ (7) $$

where $w_{ij}$ is the weight of the edge $(i,j)$.
This measures the overall intensity of node $i$’s connections.

$\bullet$ PageRank: It was originally developed for web page ranking and assigns each node $i$ the following score:

$$ $\pi_{i}=\gamma\sum_{j=1}^{N}\pi_{j}\frac{w_{ji}}{\sigma_{j}}+(1-\gamma)\frac{1}{N},$ (8) $$

where $N$ is the number of nodes in the graph, $\sigma_{j}$ is the strength of node $j$, and $\gamma\in[0,1]$ is a damping factor.
Nodes linked to other well-connected nodes receive a higher PageRank, reflecting both local and global influence.
In our experiments, we set the damping factor to the standard value $\gamma=0.85$.

$\bullet$ k-coreness: The $k$-core of a graph is the largest subgraph in which every node has an internal degree of at least $k$.
The coreness of a node $i$ is the largest $k$ such that $i$ belongs to the $k$-core.
Nodes with high coreness are deeply embedded in the core of the network.

$\bullet$ s-coreness: It is analogous to $k$-coreness, but it is based on node strength rather than degree.
The $s$-core of a weighted graph is the largest subgraph in which every node has an internal strength of at least $s$.
The $s$-coreness of a node $i$ is the largest value $s$ such that $i$ belongs to the $s$-core.
This measure captures resilience under weighted node removal: nodes with high $s$-coreness are deeply embedded in the weighted core of the network.

$\bullet$ Salience: Edge salience categorizes edges based on their intrinsic properties within the network. It can be seen as the shared consensus among nodes about the importance of an edge . It revolves around the notion of the average Shortest Path Tree (SPT):

$$ $S=\langle T\rangle=\frac{1}{N}\sum_{r=1}^{N}T(r),$ (9) $$

where, given a reference node $r$, $T(r)$ is the symmetric $N\times N$ matrix summarizing the shortest paths from $r$ to all other nodes ($t_{ij}=1$ if the edge $(i,j)$ is part of at least one shortest path, $t_{ij}=0$ otherwise). $S$ is the superimposition of all SPTs, so that $0\leq s_{ij}\leq 1$ is a consensus variable defined by the ensemble of nodes that quantifies the fraction of SPTs in which the edge $(i,j)$ participates. If $s_{ij}=1$, then the edge $(i,j)$ is essential for all nodes. If $s_{ij}=0$, then the edge has no role. If, for example, $s_{ij}=0.5$, then the edge is important for only half of the nodes. Real-world networks are often scale-free , with a few hubs and many weakly connected nodes.
If they are weighted, they usually exhibit power-law distributions for both node degrees and edge weights.
In such networks, it is possible to obtain a robust classification of edges based on edge salience because the latter typically has a bimodal distribution on the unit interval, accumulating at the extremes .
Taking only the edges with $s_{ij}\approx 1$, we define the High-Salience Skeleton (HSS), a robust, disassortative backbone with a scale-free topology that is often divided into multiple components.
The concept of salience can then easily be extended from edges to nodes.
We define the salience of node $i$ as the sum of the salience values of all its incident edges:

$$ $s_{i}=\sum_{j=1}^{N}s_{ij}.$ (10) $$

Note that, in the limit case where $s_{ij}=1$ for each edge $(i,j)$ in the HSS, and 0 otherwise, $s_{i}$ reduces exactly to the HSS degree of $i$, i.e., the number of edges incident on $i$ in the HSS.

### 4.3 Driving Opinions: Static and Dynamic Strategies

In our experiments, we select a small fraction of nodes and transform them into stubborn agents, which are biased toward an extreme opinion and can shift the average opinion toward their own extreme value.
We set the target at the upper extreme of the possible opinion range, which is 1.
The stubborn agents’ opinions are fixed by assumption, so they are not influenced by their neighbors.
Thus, their opinion value is not updated according to the HK dynamic model.
Stubborn nodes can be chosen randomly or according to a centrality-based logic that selects the top-ranking fraction of agents based on a specific centrality metric.
All stubborn agents have the same opinion value $x_{S}$, which is set using two strategies: static and dynamic (Fig. [2](#S4.F2)).

$\bullet$ Static strategy: The value of all stubborn agents’ opinions is set to $x_{S}=1$ for the entire simulation horizon (see Fig. [2](#S4.F2), blue curve).
We also experimented with $x_{S}=0$ and obtained similar results because the setting is symmetrical. We omit these results for brevity’s sake.

$\bullet$ Dynamic strategy:
The stubborn opinion $x_{S}$ increases over time within the range $x_{S}\in[0.5,1]$.
More precisely, the simulation is divided into six equal periods, and $x_{S}$ increases by $0.1$ after each period.
This process begins with $x_{S}=0.5$ and ends with $x_{S}=1$ (see Fig. [2](#S4.F2), red curve).
This approach addresses the issue of rapid conditioning in the HK process, which excludes nodes with distant opinions or small confidence values.

Figure: Figure 2: The temporal patterns of the opinion of stubborn agents under static and dynamic strategies with final $x_{S}=1$.
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/strategies.png

### 4.4 Initialization of the HK Model

For each simulation, the initial opinions and confidence values are drawn from uniform distributions. That is, $x_{i}(0)\in\mathcal{U}[0,1]$ and $\epsilon_{i}\in\mathcal{U}[\epsilon_{l},\epsilon_{u}]$, $\forall i=1,...,N$, with $\epsilon_{l},\epsilon_{u}\in[0,1]$ and $\epsilon_{l}\leq\epsilon_{u}$.

In order to accurately represent the interactions of a real-world network, the range of possible confidence values must be limited.
The lower threshold has been experimentally set to $\epsilon_{l}=0.05$: nodes close to this lower bound are unlikely to change their opinion yet they still interact with neighbors whose opinions are nearly identical to their own.
When the upper threshold $\epsilon_{u}$ is sufficiently large, the system tends to converge toward global consensus. However, when it is low, the system fragments into multiple opinion groups, leading to polarization.
In a real-world context, it is unrealistic for a node to immediately switch from one extreme opinion to another due to the strong influence of a node with an almost opposite view.
If such a transition occurs, it is gradual and takes time.
Furthermore, global consensus is rarely observed. Typically, one finds a small number – greater than one – of large clusters with a broad distribution of opinions.
For these reasons, the upper threshold has been experimentally set to $\epsilon_{u}=0.25$, which usually results in two or three main clusters of opinions in an uncontrolled process.

### 4.5 Simulation Protocol

We carried out two sets of simulations, one for each strategy (static and dynamic).
In both cases, we performed 50 simulations with random initialization (see Sec. [4.4](#S4.SS4)) on each of the 20 network instances, for all centrality metrics and with a proportion of stubborn agents $f_{S}$ taking the values 0.1%, 0.2%, 0.5%, 1%, and 2%. Note that 0.1% in a network with 1000 nodes corresponds to only one stubborn agent.

Each simulation runs for $10000$ time steps or stops earlier if the opinions converge and reach a steady state.
The convergence criterion is as follows:

$$ $\sum_{i=1}^{N}|x_{i}(t-1)-x_{i}(t)|<10^{-4}.$ (11) $$

For the dynamic strategy, this criterion is only checked when the stubborn agents reach the final opinion value, $x_{S}=1$.

## 5 Results

In this section, we present and discuss the results for a network size of $N=1000$. The results for $N=2000$ are qualitatively similar and available in the Supplementary Material.
Unless otherwise specified, all results for a given parameter setting are averaged over 50 simulations and 20 network instances.
Confidence bounds are specified whenever relevant.

### 5.1 Static Strategy

Using the Static Strategy, we examine the final average opinion of the population, which is expected to be around $0.5$ in the Uncontrolled case.
Figure [3](#S5.F3) shows that the increase yielded across various centralities is similar, with the exception of k-coreness and s-coreness as well as the Random baseline.
Other centralities, such as Salience, Betweenness, and PageRank, outperform the others in most cases.
This advantage is more evident at small fractions of stubborn agents, where the increase in the final average opinion is about 10 percentage points (roughly from 0.55 to 0.65), whereas it becomes less pronounced at larger fractions, dropping to about 5 percentage points (roughly from 0.65 to 0.7).
Furthermore, the performance of all centralities increases monotonically with the fraction of stubborn agents, although this effect tends to saturate around $f_{S}=1\%$.

Figure: Figure 3: Final average opinion as a function of the fraction $f_{S}$ of stubborn agents, with fixed opinion $x_{S}=1$, for the different centrality measures used to select them in the Static Strategy setting ($N=1000$).
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/static/N1000/avgopi_byfr_scat_net_LFRnet1000_opi_1.0_ci95_noti.png

Examining the distribution of opinions at the final time step (Fig. [4](#S5.F4)), we observe that opinions split into two groups: one centered around $x_{S}=1$ and the other close to zero.
Comparing these results with the Uncontrolled case shows that stubborn agents attract only the portion of the population already close to $1$, rather than the entire network.
Indeed, the part of the distribution far from $x_{S}=1$ remains nearly unchanged across all centralities and in the Uncontrolled case.
A small but noticeable fraction of agents remains at an intermediate opinion around $0.5$; however, this residual group progressively vanishes as $f_{S}$ increases (see $f_{S}=2.0\%$).

Figure: Figure 4: Final opinion distribution for two values of the fraction $f_{S}$ of stubborn agents (top: 0.1%, bottom: 2.0%), with fixed opinion $x_{S}=1$, obtained for all the centrality measures used to select them in the Static Strategy ($N=1000$).
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/static/N1000/opi_ridge_net_LFRnet1000_fr_0.001_opi_1.0.png

The above observation suggests measuring the number of agents “captured” by stubborn agents—i.e., those whose final opinions lie close to 1.
Figure [5](#S5.F5) shows the fraction of the nodes $i$ satisfying $|x_{i}-x_{S}|<0.05$ at the final time.
Among the centrality measures, those based on shortest paths—namely Salience and Betweenness—together with PageRank, yield the best overall performance.
Strength and Degree perform comparably well, while k-coreness and s-coreness generally perform worse.
We observe again a clear saturation limit in the fraction of the population that can be driven to $x_{S}=1$.
Increasing the fraction of stubborn agents from $f_{S}=1\%$ to $f_{S}=2\%$ does not significantly improve the performance of the best-performing centrality measures (e.g., Betweenness and Salience).

Figure: Figure 5: Fraction of the population with final opinion at distance less than $0.05$ from $x_{S}=1$, as a function of the fraction $f_{S}$ of stubborn agents and of the centrality used to select them in the Static strategy ($N=1000$).
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/static/N1000/lines_frac_near_LFRnet1000_opi_1.0_ci95.png

To complete the above analysis, Fig. [6](#S5.F6) shows the fraction of the population close to the stubborn agents over time, for the two extreme values of the fraction of stubborn agents.
In both cases, a steep initial increase is observed. For the smallest fraction of stubborn agents ($f_{S}=0.1\%$, top panel), the behavior is sensitive to the chosen centrality measure from the very beginning.
In contrast, for the largest fraction ($f_{S}=2\%$, bottom panel), the initial steep increase is consistent across all centrality measures, after which an additional phase of increase dependent on the chosen measure follows.
Overall, within the initial time steps, the stubborn nodes attract approximately about 30% of the entire population, demonstrating that the static strategy quickly creates a significant opinion divide in the network.

Figure: Figure 6: Time evolution of the fraction of the population with opinion at distance less than $0.05$ from $x_{S}=1$, for two values of the fraction $f_{S}$ of stubborn agents (top: 0.1%, bottom: 2.0%), with fixed opinion $x_{S}=1$, obtained for all the centrality measures used to select them in the Static Strategy ($N=1000$).
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/static/N1000/frac_near_hist_net_LFRnet1000_fr_0.001_opi_1.0_mean_ci95.png

### 5.2 Dynamic Strategy

We next present the results of applying the Dynamic Strategy, in which the initial opinion of the stubborn agents is set to $x_{S}=0.5$ and then gradually increases to $x_{S}=1$ (see Fig. [2](#S4.F2)).
This procedure attracts a substantially larger fraction of the population, as evidenced by the high final average opinion values in Fig. [7](#S5.F7) and the opinion distributions in Fig. [8](#S5.F8), which demonstrate the superior effectiveness of this strategy in guiding agents toward the target (compare these distributions with those in Fig. [4](#S5.F4)).

In fact, even a very small fraction of stubborn agents, $f_{S}=0.1\%$ (i.e., one agent for $N=1000$), can drive at least $85\%$ of the population toward the final opinion $x_{S}=1$, as shown in Fig. [9](#S5.F9).
However, as the fraction of stubborn agents increases, k-coreness and s-coreness, together with the Random baseline, attract larger fractions of the population, whereas the performance of other centralities decreases.

This counterintuitive result stems from the underlying opinion dynamics.
When many nodes are selected using highly effective centralities such as Betweenness, PageRank, or Salience, a significant portion of the population quickly converges to the initial stubborn opinion $x_{S}=0.5$ (see the top panels of Fig. [10](#S5.F10)), thereby isolating itself from opinions farther away due to the bounded confidence mechanism of the Hegselmann–Krause (HK) model.
In contrast, when nodes are chosen randomly or via k-coreness/s-coreness, the initial convergence to $x_{S}=0.5$ is slower or less cohesive, preventing premature lock-in.
This allows a “herd effect” to take place: non-stubborn nodes moving toward the gradually increasing $x_{S}$ attract additional non-stubborn nodes in a cascading manner.
Consequently, more agents are drawn to the final opinion $x_{S}=1$.
The herd effect also explains why the dynamic strategy performs remarkably well even with a single well-chosen stubborn node ($f_{S}=0.1\%$): a single influential node can trigger a cascade without causing early mass convergence to $x_{S}=0.5$.

Figure: Figure 7: Final average opinion as a function of the fraction $f_{S}$ of stubborn agents (with final opinion $x_{S}=1$) and of the centrality used to select them in the Dynamic Strategy ($N=1000$).
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/dynamic/N1000/avgopi_byfr_scat_net_LFRnet1000_opi_1.0_ci95_noti.png

Figure: Figure 8: Final opinion distribution for $f_{S}=0.1\%$ of stubborn agents, with final opinion $x_{S}=1$, obtained for all the centrality measures used to select them in the Dynamic Strategy ($N=1000$).
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/dynamic/N1000/opi_ridge_net_LFRnet1000_fr_0.001_opi_1.0_not.png

Figure: Figure 9: Fraction of the population with final opinion at distance less than $0.05$ from $x_{S}=1$, as a function of the fraction $f_{S}$ of stubborn agents and of the centrality used to select them in the Dynamic Strategy ($N=1000$). The bottom panel shows a magnified view of the saturation region from the top panel.
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/dynamic/N1000/lines_frac_near_LFRnet1000_opi_1.0_ci95.png

Figure: Figure 10: Time evolution of the density of opinions, averaged over 50 simulations and 20 network instances, comparing the dynamics obtained with Salience (top panels) and Random (bottom panels) for a large fraction of stubborn agents ($f_{S}=1\%$). The left panels show the entire simulation. The right panels show only the initial stages up to $t=1000$. The red dashed lines indicate the opinion of the stubborn agents (Dynamic Strategy, $N=1000$).
Refer to caption: https://arxiv.org/html/2605.14918v1/2605.14918v1/images/dynamic/N1000/hist_opi_N_1000_centr_salience_opi_1.0_fr_0.01_united.png

### 5.3 Discussion

It is useful to split the analysis of the results into two parts: the first covers cases with $0.1\%$ to $1\%$ stubborn agents under both static and dynamic strategies; the second examines the $2\%$ case, in light of the peculiar results obtained for that fraction.

Starting with the 0.1% to 1% cases, the dynamic strategies consistently outperform their static counterparts.
Static strategies create a sharp, rapid separation from the rest of the network: due to the bounded confidence mechanism, links to nodes with more distant opinions are completely severed, leading to several well‑defined, isolated clusters.
In contrast, dynamic strategies gradually attract the network’s nodes over a larger number of time steps.
This allows nodes with intermediate opinions to interact with the stubborn agents during the initial phase (when the stubborn agents’ opinion is $0.5$), and enables nodes with opinions close to zero to follow the trajectories of those in the intermediate zones.
The result is a global attraction process in which each agent’s opinion $x_{i}$ is slowly shifted toward $1$ by neighbors whose values are only slightly higher.

This outcome aligns with real‑world social network dynamics.
When attempting to shift the average opinion of a group via bots, one rarely starts by posting overtly extremist content.
Instead, one enters more moderate discussion contexts to attract agents with higher confidence, hoping they will gradually influence users with lower confidence or those at the opposite extreme of the opinion spectrum .
The theory of mass ideology is strongly supported by the opinion‑dynamics model we adopt , since an agent’s ability to change opinion is proportional to the number of neighbors influencing it, provided the confidence bounds are sufficiently large.
This explains why a dynamic, gradual conditioning is more effective in most simulations .

However, these advantages come at a cost.
Programming, deploying, managing, and controlling stubborn agents whose opinions adapt dynamically is far more expensive than using static bots that produce the same type of content without communicating with nodes holding very different opinions.
Moreover, the dynamic process requires significantly more time.
A slower convergence is more effective than a rapid one if sufficient time is available.
Conversely, a static strategy typically converges faster, making it preferable when time is limited.
As expected, a larger fraction of stubborn agents generally yields better results.
However, introducing a large number of stubborn agents into real‑world networks is not always feasible.
Therefore, a balance must be struck between the achieved results and the cost of the attack.

For $f_{S}=2\%$, the two strategies that performed worst in the previous scenarios now achieve the highest average opinions.
More effective centralities—such as Salience, Betweenness, Degree, Strength, and PageRank—produce an immediate impact, swaying the opinions of many agents.
However, they also create a sharp divide with the rest of the network, which becomes unable to interact with that cluster.
Consequently, the global attraction process discussed earlier becomes less effective.
Random, s-coreness, and k-coreness do not exhibit this behavior because their lower inherent influence is balanced by the sheer volume of stubborn agents in the network.
Among the centrality measures that require network knowledge, Degree and Strength offer a favorable trade-off between performance and computational cost, as they are static and easily obtainable.
Random selection instead, especially in the dynamic setting, delivers comparable results while incurring the lowest implementation cost, as it does not require full network knowledge.

## 6 Conclusions

In this work, we analyzed the effectiveness of attacks that influence the average opinion of a social network.
We used various centrality measures to select the positions of stubborn agents—i.e., nodes with fixed opinions that attempt to influence their neighbors.
The Lancichinetti–Fortunato–Radicchi (LFR) network model was chosen for its ability to capture degree heterogeneity and community structure, which are essential characteristics of real social networks.
For opinion propagation, we adopted the deterministic Hegselmann–Krause continuous model, which allows us to evaluate the impact of each attack and the effectiveness of each centrality measure without the unpredictability of stochastic effects.

Our analysis shows that, under a Static Strategy (stubborn agents holding an extreme opinion fixed over time), certain centralities outperform others in identifying optimal target nodes.
However, due to the bounded confidence mechanism, agents cannot be influenced by neighbors whose opinions differ too greatly.
Consequently, the fraction of the network that can be “pulled” toward the extreme opinion remains limited and tends to saturate as the fraction of stubborn agents increases.

A Dynamic Strategy proves far more effective.
Here, the opinion of stubborn nodes is modulated over time, gradually moving from a moderate value to an extreme one.
In this scenario, even a very small number of stubborn agents can pull nearly the entire population toward the extreme opinion.
Moreover, this occurs using virtually any centrality metric—even random placement yields remarkably good performance.
This is an important observation, given that network structure is often largely unknown.

Of course, this work has limitations, including the use of a single network model and a single opinion dynamics mechanism, although both are widely recognized as representative of realistic situations.
Moreover, our study exclusively considered stubborn agents aiming to push public opinion toward an extreme value.
In real networks, external agents may have diverse objectives—some may seek moderate shifts, others may attempt to stifle debate or amplify certain interactions.
Our findings therefore do not directly generalize to all types of opinion conditioning.
Nevertheless, the study lends itself to several future developments.
For example, the dynamic scenario could be extended to propose alternative strategies and evaluate the optimal trade‑off between effectiveness, speed, and implementation simplicity.
One could also consider making the conditioning effort dependent on the state of the social network.
Additionally, the inherent community structure of LFR networks makes them suitable for studying targeted attacks on individual communities, i.e., groups of individuals who presumably share the same opinion.

In conclusion, this work contributes to the research line aimed at understanding opinion propagation in social networks and the mechanisms that can condition it.
The ultimate goal is to develop effective countermeasures that ensure fairness and impartiality in public debate.
