## Abstract

Abstract Agents in social networks with threshold-based dynamics change opinions when influenced by sufficiently many peers. Existing literature typically assumes that the network structure and dynamics are fully known, which is often unrealistic. In this work, we ask how to learn a network structure from samples of the agents’ synchronous opinion updates. Firstly, if the opinion dynamics follow a threshold rule in which a fixed number of influencers prevent opinion change (e.g., unanimity and quasi-unanimity ), we provide an efficient PAC learning algorithm provided that the number of influencers per agent is bounded. Secondly, under standard computational complexity assumptions, we prove that if agents’ opinions follow the majority of their influencers, then there is no efficient PAC learning algorithm. We propose a polynomial-time heuristic that successfully learns consistent networks in over 98 % 98\% of our simulations on random graphs, with no failures for some specified conditions on the numbers of agents and opinion diffusion examples.

###### keywords:

## 1 Introduction

Opinions are ubiquitous, and learning how they emerge and evolve is crucial for understanding the spread of ideas, beliefs and behaviours within societies. From the rise of social movements to the diffusion of technological innovations, the mechanisms driving opinion change have profound implications in economics , sociology , and political science . Opinions are also contagious, making them practical tools for developing and analysing models in epidemiology and marketing . They are shaped and reshaped through interactions among agents in social networks, contributing to processes of cultural transmission and social learning .

From a computational perspective, opinion dynamics represent information propagating through a directed graph. This offers a framework for studying how social influence spreads in a network and how it can be exploited for campaigning purposes .

Nevertheless, opinion dynamics studies frequently assume a known network structure. This is often unrealistic , especially when the structure is hidden for data protection. When this is the case, connections among agents remain unidentifiable as multiple networks can have identical dynamics over certain opinion inputs. Some existing studies may not assume the structure to be fully available but rely on the network being derived from a Stochastic Block Model . Others use Bayesian optimisation techniques to predict the opinions’ transition matrix by empirically estimating the cross-correlation matrix . A polynomial-time algorithm was recently given for exact learning social networks endowed with majority dynamics, where opinions can be observed and manipulated . While this last framework is close in spirit to ours, it requires learning a social network with certainty and relies on direct intervention on the agents’ opinions.

### 1.1 Our Contribution.

We study the problem of probably approximately correct (PAC) learning the structure of a social network from its threshold-based opinion dynamics. We ask whether there are algorithms that can learn a network with error at most $\varepsilon$ and confidence at least $1-\delta$. When opinion dynamics follow the *all-but-$\kappa$* threshold dynamics, i.e., if agreement from a fixed number $\kappa$ of influencers inhibits opinion change, we give an efficient learning algorithm by constructing a Consistent Hypothesis Finder (Proposition [2](#Thmtheorem2) and Theorem [4](#Thmtheorem4)).
When the agents’ opinions follow the *majority* of their influencers, we prove under standard computational complexity assumptions ($\textbf{NP}\neq\textbf{RP}$)
that no polynomial-time algorithm can PAC learn the underlying network (Theorem [5](#Thmtheorem5)). To complement this hardness result, we design a heuristic (Algorithm [1](#alg1)) and evaluate its performance on synthetic random graphs.

### 1.2 Related Literature.

Our approach connects two prominent research lines: opinion dynamics and network inference . For a more comprehensive survey, see .

We narrow our scope to models with binary opinions governed by threshold-based diffusion rules, as these embody fundamental human decision-making factors such as homophily and biased assimilation . In these models, agents form their opinions after tallying those of others in a weighted sum. They may have a preferred opinion or attempt to conform with nearby agents, leading to Ising models inspired by the alignment of atom spins . Agents might adopt the most popular opinion from a random sample of neighbours, as in Voting Models , or external forces can be used to add a manipulation layer . A recent contribution explored the exact learning of social networks through direct intervention on opinions, but relied on having polynomially many interventions available. We remove these requirements by studying random samples of individual opinion diffusion steps and relaxing the exact learning condition to an approximately correct one.

Possibly the most prominent example of threshold-based dynamics is the Linear Threshold model with active or inactive agents . In it, agents need to attain a fixed number of active neighbours to activate, and cannot be deactivated. This model has been pivotal in the area of influence maximisation on networks and has inspired several efficient algorithms, such as DD , CELF , TIM, and TIM+ .

We are interested in the boundary between tractable and intractable problems for these models. This was previously explored in in the context of picking an optimal set of agents to spread an opinion. The authors showed that the problem’s NP-hardness depends on social network features, such as the presence of tree-like structures or cycles with sufficiently small periods. Similarly, showed that PAC learning influence functions can be achieved with polynomial sample complexity when the activation times of the nodes are known; however, without this knowledge, the problem becomes computationally hard. In contrast, showed, in an epidemiological context, that identifying the most likely infection structure is NP-hard when contagion times are known but the diffusion rule is not. Authors in extended this line of work by studying the simultaneous PAC learning of both network topology and interaction functions, showing that the problem is generally computationally intractable; yet, the special case of threshold dynamics and graphs containing a perfect matching is an efficiently learnable class. We aim to refine the boundary of PAC learnability for inferring connections between agents. We explore the conditions under which the problem is hard, trading off additional information from the interaction function against extending the class of learnable graphs.

### 1.3 Paper Structure

Section [2](#S2) introduces the notation and preliminary concepts used throughout the paper. It defines the *all-but-$\kappa$* and *$\tau$-margin* opinion diffusion protocols, from which unanimity and majority dynamics arise as special cases. The section reviews the PAC learning framework and formalises the Social Network inference problem as a PAC learning task. It also presents the *matching transformation*, our main tool for handling binary opinions in later proofs and algorithms. Section [3](#S3) presents our theoretical results for *all-but-$\kappa$* dynamics, while Section [4](#S4) establishes a hardness result for social networks equipped with majority dynamics. In Section [5](#S5), we propose a heuristic, the Waterfall algorithm, to tackle majority dynamics and analyse its theoretical guarantees and empirical performance. We discuss our results and future directions in Section [6](#S6).

For readability, full proofs are deferred to Appendix [A](#A1). An alternative characterisation of feasible solutions under majority dynamics is presented in Appendix [B](#A2), together with a tie-breaking subroutine for the Waterfall algorithm and illustrative examples. Technical specifications of the parameters used in our random graph generation models, as well as additional experiments across different sparsity regimes, are presented in Appendix [C](#A3).

## 2 Mathematical Preliminaries

### 2.1 Social Networks.

When agents update their opinions synchronously, the network can be decoupled. Hence, inferring a network can be parallelised into finding each agent’s influencers.
Throughout this paper, let $N=[n]$ be a finite set of agents and add agent $i$ as a distinguished target node. Let $G$ be a directed graph over $N\cup\{i\}$ whose edges represent influence relations.
An edge $j\to i$ in $G$ indicates that agent $j\in N$ *influences* agent $i$. We refer to a function $\ell:N\to L$ that assigns labels from a set $L$ to the agents in $N$ as a *network labelling* (or simply a *labelling*).
We consider the binary case with $L:=\{\phi,\neg\phi\}$ and, slightly abusing notation, write $\ell(j)=\phi$ when opinions of agent $j\in N$ and agent $i$ agree, and $\ell(j)=\neg\phi$ when they disagree.

In short, a *social network endowed with opinion dynamics* is a directed graph of agents and rules of how they update opinions. We denote them with the triple $(N\cup\{i\},G,f)$, where $f$ is a diffusion protocol with respect to agent $i$. The diffusion protocols assert whether agent $i$ changes opinion after interacting with a labelling $\ell\in L^{N}$. We define protocols with boolean sentences that return $\mathit{True}$ if agent $i$ changes opinion and $\mathit{False}$ if not.

We focus on two synchronous threshold-based protocols for binary opinions: all-but-$\kappa$, when an agent adopts an opinion unless $\kappa$ or more influencers support the current one; and $\tau$-margin, when an agent adopts an opinion if disagreeing influencers exceed those agreeing by more than $\tau$. These are defined below for an abstract set of agents $F\subseteq N$. Later on, we intend to retrieve the true *influencer set* $G_{i}$ for each agent $i$ from observing their threshold-based dynamics.

- 1.
All-but-$\kappa$: Let $\kappa\geq 0$. An agent changes opinion unless $\kappa$ or more of its influencers agree with it. For any network labelling $\ell\in L^{N}$ and $F\subseteq N$, the all-but-$\kappa$ protocol $f^{\leq\kappa}$ is given by the boolean sentence
$\displaystyle f^{\leq\kappa}(\ell,F):=\sum_{j\in F}\mathds{1}(\ell(j)=\phi)\leq\kappa.$
*Unanimity dynamics* arise for $\kappa=0$, where an agent changes opinion only if all its influencers disagree with it.
- 2.
$\tau$-margin: An agent changes opinion if the number of influencers who disagree with it exceeds those who agree by a margin greater than $\tau\geq 0$. For any network labelling $\ell\in L^{N}$ and $F\subseteq N$, the $\tau$-margin protocol $f^{+\tau}$ is given by the boolean sentence
$\displaystyle f^{+\tau}(\ell,F):=\sum_{j\in F}\mathds{1}(\ell(j)\neq\phi)-\mathds{1}(\ell(j)=\phi)>\tau.$
Choosing $\tau=0$ yields *majority dynamics*, where an agent changes opinion when a strict majority of its influencers disagree with it. We use $f^{+}$ to denote this.

Given a sample of network labellings, the main challenge is to deal with agents who disagreed (or agreed) with our target agent in some labellings where its opinion changed and in others where it did not. This begs the question of whether or not they caused the change in opinion. To address this, we divide the sample into two subsets, called the *always-changing* and the *never-changing* labellings. We aim to show how they can co-exist without contradicting each other.

###### Definition 1 .

Let $f$ be any diffusion protocol. We say a set of network labellings $\{\ell_{k}\}_{k=1}^{m}$ is *always-changing* for $F\subseteq N$ if the sentence $f(\ell_{k},F)$ is $\mathit{True}$ for $k=1,\dots,m$. It is *never-changing* if all $m$ sentences are $\mathit{False}$.

###### Definition 1 .

### 2.2 PAC Learning.

We formalise the *Probably Approximately Correct* (PAC) learning following . The goal is to design a learning model whose predictions are both highly probable and close to the true answer. Let $X$ be the *domain* of elements encoding the learner’s world. A *concept* $c$ over $X$ is a boolean mapping $c:X\to\{0,1\}$ indicating whether $x\in X$ has a desired property. A *concept class* $\mathcal{C}$ is a collection of concepts within a *hypothesis space* $\mathcal{H}$. A *learner* $\mathcal{L}$ is an algorithm whose objective is to distinguish between positive and negative examples of a target concept $c$ chosen arbitrarily from $\mathcal{C}$. Examples are given by an *oracle* $\text{Ex}(c,\mathcal{D})$, assumed to draw an example $x$ from $X$ with distribution $\mathcal{D}$ and return $c(x)$ in constant time. Note that $\mathcal{L}$ cannot query the oracle directly.

###### Definition 2 .

A concept class $\mathcal{C}$ over a domain $X$ is *$(\varepsilon,\delta)$-PAC learnable* using $\mathcal{H}\supseteq\mathcal{C}$ if a learner $\mathcal{L}$ exists such that, given any inputs $\varepsilon,\delta\in(0,\frac{1}{2})$ and access to $\emph{Ex}(c,\mathcal{D})$, $\mathcal{L}$ outputs a hypothesis concept $h\in\mathcal{H}$ satisfying with probability $1-\delta$ that $\mathbb{P}[h(x)\neq c(x)]\leq\varepsilon$, for every concept $c\in\mathcal{C}$ and $x\in X$ randomly sampled with distribution $\mathcal{D}$. The error probability $\mathbb{P}$ is taken over the random examples drawn from $\emph{Ex}(c,\mathcal{D})$ and any internal randomisation of $\mathcal{L}$. If these conditions are met, we also say $\mathcal{L}$ *$(\varepsilon,\delta)$-PAC learns* $\mathcal{C}$.

###### Definition 3 .

A *Consistent Hypothesis Finder* (CHF) for a concept class $\mathcal{C}$ over $X$ using $\mathcal{H}\supseteq\mathcal{C}$ is an algorithm $\mathcal{L}$ such that, for all $m>0$ and $c\in\mathcal{C}$, if $\mathcal{L}$ is given a sample $\{(x_{1},c(x_{1})),\dots,(x_{m},c(x_{m}))\}$,
then it outputs a hypothesis $h\in\mathcal{H}$ satisfying $h(x_{k})=c(x_{k})$ for $k=1,\dots,m$.

The learner gains more information as the oracle $\text{Ex}(c,\mathcal{D})$ draws more examples from $X$ since fewer hypotheses in $\mathcal{H}$ remain indistinguishable from the target concept $c\in\mathcal{C}$. Yet, the learner cannot refine its guess to the correct concept if multiple hypotheses are consistent with the training examples. This is resolved by the Fundamental Theorem of PAC Learning (see, e.g., ), which links the sample complexity of a concept class with its VC dimension.

###### Definition 4 .

For a concept class $\mathcal{C}$ over $X$ and a finite set of points $S=\{x_{1},\dots,x_{m}\}$, if $\{(c(x_{1}),\dots,c(x_{m})):c\in\mathcal{C}\}=\{0,1\}^{m}$, then we say that $S$ is *shattered* by $\mathcal{C}$. Furthermore, the cardinality of the largest set shattered by $\mathcal{C}$ is its *Vapnik-Chervonenkis* dimension $\text{VCD}(\mathcal{C})$.

###### Theorem 1 (Fundamental Theorem of PAC Learning) .

Let $\mathcal{C}$ be an arbitrary concept class in $X$ and $\mathcal{D}$ any distribution over $X$. Then, any algorithm that $(\varepsilon,\delta)$-PAC learns $\mathcal{C}$ requires at least

$$ $\displaystyle m=\Theta\left(\frac{1}{\varepsilon}\left(\text{VCD}(\mathcal{C})+\log\frac{1}{\delta}\right)\right)$ $$

many examples from $\text{Ex}(c,\mathcal{D})$, for any $c\in\mathcal{C}$.

Theorem [1](#Thmtheorem1) provides an upper bound on the number of examples sufficient to attain a desired PAC learning error $\varepsilon$ and confidence $\delta$. Moreover, showed this bound is tight, as it matches earlier lower bounds from and . Consequently, for a fixed sample size $m$, we can recover achievable values for $\varepsilon$ and $\delta$ up to multiplicative constants.

###### Definition 2 .

###### Definition 3 .

###### Definition 4 .

###### Theorem 1 (Fundamental Theorem of PAC Learning) .

### 2.3 The Social Network Inference Problem

We consider a learner who receives a sample of opinion updates for a target node. We assume it knows the diffusion protocol but not the underlying network. The learner’s guess for the network improves with more examples, but how many are sufficient for their guess to be good enough?

Let us formulate the social network inference problem in terms of PAC learning. Define the domain $X$ as the space of network labellings $L^{N}$ with concept classes induced by a diffusion protocol, $f^{\leq\kappa}$ or $f^{+\tau}$. For a given agent $i$, the concept to learn is the influencer set $G_{i}\subseteq N$ that drives its opinion updates. An oracle $\text{Ex}(G_{i},\mathcal{D})$ draws network labellings $\{\ell_{k}\}_{k=1}^{m}$ according to a distribution $\mathcal{D}$ over $L^{N}$ and returns each as a positive example if agent $i$ changes opinion (i.e., if $f(\ell_{k},G_{i})$ is $\mathit{True}$, for $f\in\{f^{\leq\kappa},\,f^{+\tau}\}$) or negative otherwise. Notice that both protocols are linear threshold functions over a domain with $n:=|N|$ dimensions. As such, they are well-studied concept classes with VC dimension $n+1$ (see, e.g., ). Thus, building a CHF in this context boils down to finding a subset $F\subseteq N$ whose oracle prediction matches $G_{i}$ for the labellings sampled so far. Altogether, we can $(\varepsilon,\delta)-$PAC learn an agent’s influencers if the subset $F\subseteq N$ matches the oracle prediction of $G_{i}$ for a labelling sample of size

$$ $m=\Theta\left(\frac{1}{\varepsilon}\left((n+1)+\log\frac{1}{\delta}\right)\right).$ (1) $$

### 2.4 Matching Transformations.

Working with opinions relative to a target agent allows us to efficiently format the inputs for our algorithms via what we call a *matching transformation*. Intuitively, this transformation recovers the agents whose opinions “match” the oracle’s prediction for agent $i$. If the oracle predicts agent $i$ changes opinion after a labelling $\ell\in L^{N}$, then the agents who disagreed with it (i.e., $j\in N$ s.t. $\ell(j)\neq\phi$) match this prediction. Conversely, if no change is predicted, then the agents who agreed with agent $i$ in $\ell$ (i.e., $j\in N$ s.t. $\ell(j)=\phi$) match the prediction.

###### Definition 5 .

Let $(N\cup\{i\},G,f)$ be a social network with opinion diffusion protocol $f$. For a labelling $\ell\in L^{N}$, $L=\{\phi,\neg\phi\}$, the *matching set* $M(\ell)\subseteq N$ is the subset of agents such that $j\in M(\ell)$ if $\ell(j)=\neg\phi$ when $f(\ell,G_{i})$ is $\mathit{True}$, or $\ell(j)=\phi$ when $f(\ell,G_{i})$ is $\mathit{False}$.

###### Definition 6 .

For $m,n>0$, let $N=[n]$, $(N\cup\{i\},G,f)$ be a social network, and $(\ell_{k})_{k=1}^{m}$ be a sequence of network labellings. The *matching transformation* for $(\ell_{k})_{k=1}^{m}$ is given by the $m\times n$ matrix $\mathbf{M}$ with entries

$$ $\displaystyle\mathbf{M}_{k,j}=\begin{cases}\hskip 7.11317pt1&\text{if}\quad j\in M(\ell_{k})\quad\text{and}\\ -1&\text{otherwise,}\end{cases}$ $$

where $M(\ell)\subseteq N$ is the matching set of $\ell\in(\ell_{k})_{k=1}^{m}$.

###### Example 1 .

Suppose there is a network under some opinion protocol $f$ with $N=[5]$ plus agent $i$. An oracle samples the following labellings $\ell_{1},\ell_{2}\in\{\phi,\neg\phi\}^{N}$ relative to agent $i$:

$$ $\displaystyle\begin{array}[]{c|rrrrr}N&1&2&3&4&5\\ \hline\cr\ell_{1}&\phi&\phi&\neg\phi&\neg\phi&\neg\phi\\ \ell_{2}&\neg\phi&\neg\phi&\phi&\phi&\neg\phi\\ \end{array}$ (5) $$

The agents who disagree with agent $i$ are $\{3,4,5\}$ in $\ell_{1}$ and $\{1,2,5\}$ in $\ell_{2}$. The oracle knows which agents are the influencers $G_{i}\subseteq N$ and predicts agent $i$ changes opinion after $\ell_{1}$ but not after $\ell_{2}$. So, $f(\ell_{1},G_{i})$ is a positive example while $f(\ell_{2},G_{i})$ is a negative one. This yields the matching sets

$$ $\displaystyle M(\ell_{1})=\{j\in N:\ell(j)=\neg\phi\}\quad\text{and }$ $\displaystyle M(\ell_{2})=\{j\in N:\ell(j)=\phi\}.$ $$

We store these sets in the matching transformation

$$ $\displaystyle\mathbf{M}=\begin{bmatrix}-1&-1&1&1&1\\ -1&-1&1&1&-1\end{bmatrix}.$ (6) $$

Graphically, we use when an agent belongs to a matching set and when it does not. $\mathbf{M}$ is shown in Figure [1](#S2.F1).

Figure: Figure 1: Matching transformation for $\mathbf{M}$ as in ([6](#S2.E6)).
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/x1.png

The dimensions $m\times n$ of $\mathbf{M}$ correspond to a labelling sample size $m$, and a social network with $n+1$ agents. Our learner’s inputs are $\mathbf{M}$ and the oracle’s predictions $f(\ell_{k},G_{i})$ for $k=1,\dots,m$. We aim to find a set of agents $F\subseteq N$ whose opinion dynamics mimic the true set of influencers $G_{i}$ for our labelling sample. We refer to the agents that achieve this as *feasible influencers*.

###### Remark 1 .

We write $\mathbf{M}_{F}$ for the columns in $\mathbf{M}$ restricted to agents in $F\subseteq N$. It is no coincidence that entries in $\mathbf{M}$ are $\pm 1$. This ensures that $F$ is a feasible influencer set under majority dynamics if and only if the row-sums in $\mathbf{M}_{F}$ are nonnegative, and zero only when there is no opinion change.

###### Definition 5 .

###### Definition 6 .

###### Example 1 .

###### Remark 1 .

## 3 Theoretical Results for All-But- κ \kappa Dynamics

Agents in networks ruled by an all-but-$\kappa$ protocol change their opinion unless more than $\kappa$ of their influencers already agree with them. Naturally, an agent changes opinion more often as $\kappa$ increases. We begin with the most restrictive case when $\kappa=0$. This yields *unanimity dynamics*, where agent $i$ changes opinion only when all its influencers disagree with it. In this case, the set of agents who matched the oracle’s prediction across all the labellings where agent $i$ changed opinion will form a feasible influencer set. Thus, returning this set is a CHF with the following runtime upper bound.

###### Proposition 2 .

Let $N=[n]$ and $(N\cup\{i\},G,f^{\leq 0})$ be a social network under an all-but-$\kappa$ protocol, with $\kappa=0$. There exists a CHF that finds a feasible influencer set $F\subseteq N$ in $\mathcal{O}(m\,n^{2})$ time for any labelling sample of size $m$.

Next, we consider the all-but-$\kappa$ model with $\kappa>0$, when all predictions state agent $i$ changes opinion. Note that selecting a subset with $\kappa+1$ or more agents in $M(\ell)^{c}$, $\ell\in L^{N}$, would be inconsistent for any labelling where the target agent changes opinion. So, if the learner checks all subsets in $N$ of size at most $\kappa+1$, it is bound to find at least one feasible influencer set, if such a set exists.

###### Proposition 3 .

Let $N=[n]$ and $(N\cup\{i\},G,f^{\leq\kappa})$ be a social network under an all-but-$\kappa$ protocol, $\kappa\geq 0$. There exists a CHF that finds a feasible influencer set $F\subseteq N$, if one exists, in $\mathcal{O}(m\,n^{\kappa+2})$ time for any always-changing labelling sample of size $m$.

The final step is to combine the always-changing and never-changing labellings. The learner knows that feasible sets must contain at least $\kappa$ influencers in the matching set of every never-changing labelling. So, if it checks all sets of $\kappa$ or more agents, progressively increasing in size, it will not need to check any set larger than the real influencer set $G_{i}$.

###### Theorem 4 .

Let $N=[n]$ and $(N\cup\{i\},G,f^{\leq\kappa})$ be a social network under an all-but-$\kappa$ protocol with $\kappa\geq 0$. If $G_{i}$ is the influencer set of agent $i$, then there exists a CHF that finds a feasible influencer set $F\subseteq N$ in $\mathcal{O}(m\,n^{|G_{i}|+1})$ time for any labelling sample of size $m$.

The complexity of the CHF depends on the size of the real influencer set $G_{i}$, not the $\kappa$ parameter in the all-but-$\kappa$ model. For networks where $|G_{i}|$ is bounded by a constant (e.g., sparse regular graphs), the complexity remains polynomial. However, the approach may become inefficient without this assumption. The upcoming section explores the boundary of efficient solutions for the majority dynamics case.(^1^11 Notice that majority dynamics is a special case of the all-but-$\kappa$ and $\tau$-margin protocols, with either $\kappa=\lceil|G_{i}|/2\rceil-1$ or $\tau=0$.)

###### Proposition 2 .

###### Proposition 3 .

###### Theorem 4 .

## 4 Hardness for Learning Feasible Influencer Sets Under Majority Dynamics

Our task for networks with majority dynamics is to find a set of agents $F\subseteq N$ such that at least half of them appear in every sampled labelling’s matching set; strictly more if the target agent changes opinion, and half or more if not. A naive approach would return the intersection of all matching sets. But what if this intersection is empty? We apply the known computational hardness of the Hitting Set problem (see, e.g., ) to prove that finding a feasible influencer set $F$ in these networks is NP-complete. To do so, we adapt the approach in , and construct a matching transformation $\mathbf{M}$ and an oracle prediction from a Hitting Set instance.

###### Definition 7 (Hitting Set problem) .

Given a family of sets $\{S_{1},S_{2},...,S_{m}\}$ and a budget $d>0$, we wish to find, if possible, a set $C$ of size $d$ that has a non-empty intersection with every set $S_{k}$, $k=1,\dots,m$.

###### Theorem 5 .

The Hitting Set problem for $\{S_{1},S_{2},...,S_{m}\}$ with budget $d>0$ reduces to finding a feasible set of influencers for a target agent in a network with $n+1$ agents, where $n=|\bigcup_{k=1}^{m}S_{k}|+d+1$, consistent with $m+d+2$ examples from a majority dynamics protocol.

###### Proof.

We encode any hitting set instance into a labelling sample for a network with majority dynamics. Let $\{S_{1},S_{2},...,S_{m}\}$ be a family of sets and $d>0$ a budget. Define $\hat{n}:=|\bigcup_{k=1}^{m}S_{k}|$, $\hat{m}:=m+d+2$, $n:=\hat{n}+d+1$ and $N:=[n]$.
We construct a labelling sample $(\ell_{k})_{k=1}^{\hat{m}}$ for a social network
$(N\cup\{i\},G,f^{+})$ following .

We split the agents in $N$ into two: $\{a_{1},\dots,a_{d+1}\}$ and $\{b_{1},\dots,b_{\hat{n}}\}$. Further, we assign to each agent $b_{j}\in\{b_{1},\dots,b_{\hat{n}}\}$ an element $\smash{s_{j}\in\bigcup_{k=1}^{m}S_{k}}$. The examples are built such that all auxiliary agents in $\{a_{1},\dots,a_{d+1}\}$ are in the matching set of $\ell_{\hat{m}}$. For the other labellings, $\ell_{1},\dots,\ell_{\hat{m}-1}$, agent $a_{j}$ belongs to $M(\ell_{k})$ if

$$ $\displaystyle\begin{cases}2\leq j\leq d+1&\text{for }k=1,\dots,m,\text{ or }\\ j=k-m&\text{for }k=m+1,\dots,\hat{m}-1.\end{cases}$ $$

For the agents in $\{b_{1},\dots,b_{\hat{n}}\}$, $b_{j}\in M(\ell_{k})$ when $s_{j}\in S_{k}$ for $k=1,\dots,m$, plus for $k=m+1,\dots,\hat{m}-1$. See Figure [2](#S4.F2) for a graphical example of a matching transform $\mathbf{M}$ constructed in this way.

We show that a hitting set $C\subseteq\bigcup_{k=1}^{m}S_{k}$, $|C|=d$, exists if and only if there is a feasible influencer set $F\subseteq N$ for which the target agent $i$ changes opinion on all these labellings. Without loss of generality, we suppose $C:=\{s_{1},\dots,s_{d}\}$ and show that $F=\{a_{1},\dots,a_{d+1}\}\cup\{b_{1},\dots,b_{d}\}$.

$(\Rightarrow)$ Assume $C$ is a hitting set for $\{S_{1},S_{2},...,S_{m}\}$. Since agent $i$ always changes opinion, $F:=\{a_{1},\dots,a_{d+1}\}\cup\{b_{1},\dots,b_{d}\}\subseteq N$ is a feasible influencer set if and only if

$$ $\displaystyle|M(\ell_{k})\cap F|>|M(\ell_{k})^{c}\cap F|,\text{ for }k=1,\dots,\hat{m}.$ $$

This holds for all our examples because $|M(\ell_{k})\cap F|\geq d+1$ while $|M(\ell_{k})^{c}\cap F|\leq d$.

More specifically, for $k=1,\dots,m$, there are $d$ agents in $\{a_{1},\dots,a_{d+1}\}$ in each matching set. Further, since agents in $b_{1},\dots,b_{d}$ are associated to the elements in the hitting set $C$, we have that

$$ $\displaystyle|M(\ell_{k})\cap\{b_{1},\dots,b_{d}\}|\geq 1\text{ and }$ $\displaystyle|M(\ell_{k})^{c}\cap\{b_{1},\dots,b_{d}\}|\leq d-1.$ $$

In contrast, all agents in $\{b_{1},\dots,b_{d}\}$ plus one agent from $\{a_{1},\dots,a_{d+1}\}$ will be in $M(\ell_{k})$ for $k=m+1,\dots,\hat{m}-1$. Therefore, $|M(\ell_{k})^{c}\cap F|\leq d$ since $|F|=2d+1$. Similarly, all (and only) the agents in $\{a_{1},\dots,a_{d+1}\}$ belong to the matching set when $k=\hat{m}$.

$(\Leftarrow)$ Assume $F\subseteq N$ is a feasible set of influencers. We need to show that

$$ $F=\{a_{1},\dots,a_{d+1}\}\cup\{b_{1},\dots,b_{d}\},$ $$

where the elements $\{s_{1},\dots,s_{d}\}\subseteq\bigcup_{k=1}^{m}S_{k}$ associated to agents $b_{1},\dots,b_{d}$ create a hitting set.

Let $F:=A\cup B$ for some $A\subseteq\{a_{1},\dots,a_{d+1}\}$ and $B\subseteq\{b_{1},\dots,b_{\hat{n}}\}$. However, since $F$ is a feasible influencer set, it must satisfy that

$$ $\displaystyle|M(\ell_{k})\cap F|>|M(\ell_{k})^{c}\cap F|,\text{ for }k=1,\dots,\hat{m}.$ (7) $$

For the $\hat{m}$-th example, we have that $|M(\ell_{\hat{m}})\cap A|=|A|$ and $|M^{c}(\ell_{\hat{m}})\cap B|=|B|$. So $|A|>|B|$ for ([7](#S4.E7)) to hold. This ensures that our hitting set remains within budget $d$.

Now, for $k=m+1,\dots,\hat{m}-1$, $|M(\ell_{k})\cap B|=|B|$ while $|M(\ell_{k})\cap A|=\mathds{1}(a_{k-m}\in A)$, where $\mathds{1}(a_{k-m}\in A)$ is $1$ if $a_{k-m}\in A$ and 0 otherwise. Therefore,

$$ $\displaystyle|M(\ell_{k})\cap F|-|M(\ell_{k})^{c}\cap F|$ $\displaystyle=\;$ $\displaystyle(\mathds{1}(a_{k-m}\in A)+|B|)-(|A|-\mathds{1}(a_{k-m}\in A))$ $\displaystyle=\;$ $\displaystyle 2\times\mathds{1}(a_{k-m}\in A)+|B|-|A|.$ (8) $$

Since ([8](#S4.E8)) has to be positive with $|A|>|B|$, it must be that $a_{k-m}\in A$ for $k=m+1,\dots,\hat{m}-1$.
Therefore, $A=\{a_{1},\dots,a_{d+1}\}$ and $|B|>d-1$. More so, $|B|=d$.

Finally, for $k=1,\dots,m$, we have $|M(\ell_{k})\cap A|=d$. Thus, to satisfy ([7](#S4.E7)) we need

$$ $|M(\ell_{k})\cap F|-|M(\ell_{k})^{c}\cap F|=2\,|M(\ell_{k})\cap B|-1>0.$ $$

This implies that $|M(\ell_{k})\cap B|>1$ for every $k=1,\dots,m$. However, recall that every agent $b_{j}\in\{b_{1},\dots,b_{\hat{n}}\}$ that belongs to $M(\ell_{k})$ has an element $s_{j}\in S_{k}$ associated with it, making $\{s_{1},\dots,s_{d}\}$ a hitting set for $\{S_{1},\dots,S_{m}\}$.
∎

Figure: Figure 2: Graphical representation of the matching transformation derived from a Hitting Set instance. Budget is $d=2$ and the input sets are $S_{1}=\{s_{2},s_{3},s_{4},s_{5}\}$, $S_{2}=\{s_{1},s_{4}\}$, $S_{3}=\{s_{1},s_{5}\}$ and $S_{4}=\{s_{2}\}$. One possible hitting set is $C=\{s_{1},s_{2}\}$. So, the set $F:=\{a_{1},a_{2},a_{3}\}\cup\{b_{1},b_{2}\}$, consisting of all the auxiliary agents together with agents $\{b_{1},b_{2}\}$ associated with $C$ is a feasible influencer set.
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/x2.png

###### Remark 2 .

Figure [2](#S4.F2) illustrates the matching transformation $\mathbf{M}$ built out of a Hitting Set instance following the steps in the proof of Theorem [5](#Thmtheorem5). We use the same notation as in Figure [1](#S2.F1), where represents the agents in the matching set. If there is a solution to the Hitting Set problem, we can trace $d+1$ paths from the top to the bottom of $\,\mathbf{M}$ flowing through the matching sets using just $2d+1$ agents. If we restrict $\mathbf{M}$ to only the columns of agents in $F$, denoted by $\mathbf{M}_{F}$, then more than half of the agents match the oracle in all the rows. Also, the far-right path is made of elements in the non-empty intersection of the hitting set with the sets in $\{S_{1},\dots,S_{m}\}$.

The Hitting Set problem is known to be NP-complete (see, e.g., ). Yet, we are also interested in the Randomised Polynomial-time (RP) complexity class. Algorithms in RP run in polynomial time in the input size. Also, they guarantee no false positives and a false-negative probability of less than $\nicefrac{{1}}{{2}}$.

In our context, false positives occur when an influencer set is retrieved but it is inconsistent with at least one of the examples in $(\ell_{k})_{k=1}^{m}$ (i.e., the agent changes opinion when predicted not to, or vice versa). A false negative occurs if the algorithm claims there is no feasible influencer set for the given sample when such a set actually exists. Therefore, the reduction from Theorem [5](#Thmtheorem5) yields that, unless $\mathbf{NP}=\mathbf{RP}$, no polynomial-time algorithm is capable of PAC learning feasible influencer sets in networks with majority dynamics.

###### Theorem 6 .

Let $f^{+}$ be the class of majority dynamics protocols in social networks formed by $n+1$ agents who can hold binary opinions in $L$. If there exists an algorithm $\mathcal{L}$ that is a CHF such that, for every $G_{i}\subseteq N$, distribution $\mathcal{D}$ over $L^{N}$ and error parameter $0<\varepsilon<1$, $\mathcal{L}$ runs in polynomial time for $n:=|N|$ and $\nicefrac{{1}}{{\varepsilon}}$ and, with probability of at least $\nicefrac{{1}}{{2}}$, outputs an influencer set $F\subseteq N$ satisfying $\mathbb{P}_{\ell\sim\mathcal{D}}(f^{+}(\ell,F)=f^{+}(\ell,G_{i}))\geq 1-\varepsilon$, then $\mathbf{NP}=\mathbf{RP}$.

###### Proof (Outline).

Since $\mathcal{L}$ can find a feasible influencer set in polynomial time for any distribution of network labellings, for any Hitting Set instance with $m$ sets and budget $d$, consider the uniform distribution over network labellings restricted to the examples built as in the proof of Theorem [5](#Thmtheorem5). Over this distribution, taking $\varepsilon\leq(m+d+3)^{-1}$ implies $\mathcal{L}$’s solution perfectly fits the oracle predictions, making it a CHF. Further, this solution can be translated back to a Hitting Set instance and into any NP-hard problem. As an RP-algorithm, $\mathcal{L}$ establishes the link between the classes.
∎

###### Corollary 1 .

Assuming $\mathbf{NP}\neq\mathbf{RP}$, there is no polynomial-time algorithm for learning the influencers of an agent in a social network with majority dynamics.

###### Definition 7 (Hitting Set problem) .

###### Theorem 5 .

###### Proof.

###### Remark 2 .

###### Theorem 6 .

###### Proof (Outline).

###### Corollary 1 .

## 5 A Heuristic for Majority Dynamics

Theorem [6](#Thmtheorem6) and Corollary [1](#Thmcor1) show that retrieving, even approximately, the structure of a social network just from observing its majority dynamics is computationally intractable. This, however, should not prevent us from designing practical heuristics. Recall that our first naive idea was to return the agents in the intersection of all matching sets as a feasible influencer set. While unanimity dynamics guarantee a non-empty intersection, that is not the case for majority dynamics. Thus, we relax our approach to consider agents outside some matching sets as feasible influencers, provided that these are never the strict majority.

Formally, given an $m\times n$ matching transformation $\mathbf{M}$, an oracle prediction $f^{+}(\ell_{k},G_{i})$ for $k=1,\dots,m$, and a subset $F\subseteq N$, the rows of the restricted matrix $\mathbf{M}_{F}$ fall into:

- 1.
Consistent (C): If $|M(\ell_{k})\cap F|>|M(\ell_{k})^{c}\cap F|$.
- 2.
Barely consistent (BC): If $|M(\ell_{k})\cap F|-|M(\ell_{k})^{c}\cap F|=1$ and $f^{+}(\ell_{k},G_{i})$ is $\mathit{True}$.
- 3.
Consistent tie (CT): If $|M(\ell_{k})\cap F|=|M(\ell_{k})^{c}\cap F|$ and $f^{+}(\ell_{k},G_{i})$ is $\mathit{False}$.
- 4.
Inconsistent tie (IT): If $|M(\ell_{k})\cap F|=|M(\ell_{k})^{c}\cap F|$ and $f^{+}(\ell_{k},G_{i})$ is $\mathit{True}$.
- 5.
Inconsistent (I): If $|M(\ell_{k})\cap F|<|M(\ell_{k})^{c}\cap F|$.

As we append an agent to $F$, each row becomes more consistent or inconsistent depending on whether the agent is in $M(\ell_{k})$ or $M(\ell_{k})^{c}$. Yet, a row in a consistent state cannot become inconsistent (or vice versa) without going through CT if $f^{+}(\ell_{k},G_{i})$ is $\mathit{False}$, or BC if it is $\mathit{True}$. Moreover, our learner $\mathcal{L}$ finds a feasible influencer set $F^{*}\subseteq N$ when no rows in $\mathbf{M}_{F^{*}}$ are in states I or IT.

All inconsistencies must be resolved simultaneously upon adding the final agent, so no labelling is prioritised over another. For a given $F\subseteq N$, we say a labelling $\ell\in(\ell_{k})_{k=1}^{m}$ *needs rescuing* if it does not admit adding any agent from $M(\ell)^{c}$ (i.e., its state is I, IT, CT or BC). Our heuristic employs a greedy strategy when adding agents to $F$. It picks the agent belonging to the most matching sets of labellings that need rescuing. We call this the Waterfall algorithm. Its pseudocode is shown in Algorithm [1](#alg1), and the implementation used in our experiments will be made publicly available upon publication at [https://github.com/…/Waterfall-algorithm.git](https://github.com/lf-estrada/Waterfall-algorithm.git)

Figure: Algorithm 1 Waterfall algorithm

###### Proposition 7 .

The Waterfall runs in $\mathcal{O}(m\,n^{3})$ time for any $m\times n$ matching matrix $\mathbf{M}$ and oracle prediction.

###### Remark 3 .

To adapt the Waterfall to any $\tau$-margin protocol, $\tau\geq 0$, we redefine the labellings that need rescuing as those where $|M(\ell_{k})\cap F|-|M(\ell_{k})^{c}\cap F|\leq\tau$. This preserves the intuition of requiring an additional agent from the matching set; otherwise, the labelling would be inconsistent. The margin is also incorporated into any feasibility check.

###### Proposition 7 .

###### Remark 3 .

### 5.1 Theoretical Guarantees

The Waterfall performs consistency checks in lines 1, 6 and 14, before returning a feasible influencer set. Still, it may incur false-negative errors, in the sense of incorrectly concluding there is no feasible influencer set for the given sample when, in fact, such a set exists. This is because, like any greedy approach, the Waterfall is prone to follow suboptimal routes. For instance, if multiple agents rescue the same number of labellings, the algorithm may select one outside the true influencer set $G_{i}$, though other feasible sets may exist. So, when can we guarantee the Waterfall will not add too many “incorrect” influencers?

###### Proposition 8 .

If a feasible influencer set $F\subseteq N$ exists and $|F|\leq 2$, then the Waterfall will find it.

###### Proposition 9 .

If a feasible influencer set $F\subseteq N$ exists and the Waterfall goes over $F\raisebox{0.51114pt}{\scriptsize$\hskip 0.56917pt\fgebackslash\hskip 0.56917pt$}\{j\}$, for any $j\in F$, then it will find a feasible influencer set of size $|F|$.

Propositions [8](#Thmtheorem8) and [9](#Thmtheorem9) ensure that the Waterfall finds a solution if it iterates over any subset of agents that is one agent away from being feasible. This is because any ties occur among agents who rescue all the remaining labellings. Therefore, the failure probability corresponds to the likelihood of omitting all subsets that are one agent short.

Our algorithm resembles the Greedy cover algorithm in , which was pivotal for proving that the ratio of optimal integral and fractional covers of a hypergraph $G$ is at most $1+\log\deg(G)$, where $\deg(G)$ is the hypergraph’s maximum degree. Greedy’s approximation is shown to be optimal in worst-case scenarios in . Further, characterised the integrality gap, up to multiplicative constants, for the average-case of randomised Hitting Set instances. These instances assume that each set includes the given element independently with probability $p$, and the characterisation relies on a combinatorial analysis in dense and sparse regimes.

Our Waterfall traverses the initialisation space of Greedy, mitigating the dependencies introduced by the first pick. Also, it operates on the problem of finding a feasible influencer set, which is a generalisation of the Hitting Set. For instance, there is no monotonicity to rely on, so adding more elements does not necessarily lead to a valid solution. Think of the case where taking all agents in $N$ as agent $i$’s influencers conflicts with the oracle’s prediction because the global majority differs from the majority in $G_{i}$. Still, the feasibility checks built into the Waterfall prevent it from returning such false positives.

###### Proposition 8 .

###### Proposition 9 .

### 5.2 Experiments

We run the Waterfall on four types of random networks: Erdős–Rényi (ER), Watts–Strogatz (WS), Regular Graph (RG) and Barabási–Albert (BA); generated using NetworkX. We vary the network size $n$ and number of examples $m$ between $10$ and $50$. For each $(n,m)$ pair, we generate $50$ labelling samples of size $m$ and $40$ random networks of size $n$ per sparsity regime $p\in\{0.1,0.25,0.5,0.75,0.9\}$. Networks are divided evenly among the graph types. We build the oracle prediction and matching transformation for each agent in the network and the labelling sample.

For fair comparison, the sparsity regime $p$ governs the structure across network types. It specifies the edge probability in ER and the rewiring probability in WS. We use $2+p\,(\frac{n}{2}-1)$, rounded to the nearest even integer, to control the number of neighbours in WS, the degree in RG, and how many edges each new node attaches to in BA. Exact generation functions are detailed in the Appendix.

Figure [3](#S5.F3) presents the False Negative Rate (FNR) of the Waterfall on inputs where a solution always exists. Our results show performance is agnostic to network topology. Breakdowns by network type and sparsity regime are available in the supplementary material. Instead, performance depends on the ratio of labelling samples to network size. Error rates are confined within a cone-shaped region and increase towards its interior. Intuitively, underdetermined cases fall below the cone, where extra agents enable incorrect yet feasible influencer sets. Conversely, the cone’s upper bound reflects how additional examples reduce the likelihood of ties. Overall, the Waterfall succeeded $98.13\%$ of the trials, with a mean FNR of $2.08\times 10^{-3}$ and variance $5.40\times 10^{-6}$.

Figure: Figure 3: False Negative Rate (FNR) for the Waterfall. Each grid point $(m,n)$ tested 40 networks with $n$ agents per density value $p\in\{0.1,0.25,0.5,0.75,0.9\}$ on 50 shared labelling samples of size $m$. The Waterfall runs in parallel across the agents in each network. For $n\leq 5$, we exhausted all possible networks and labelling samples without errors.
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/FNR_grid_all.png

## 6 Discussion

We studied the problem of learning the structure of a social network governed by threshold-based opinion dynamics. We presented Consistent Hypothesis Finders (CHFs) that run in polynomial time for the cases of unanimity and all-but-$\kappa$ update rules. For majority dynamics, we proved that finding feasible influencer sets is NP-complete. We designed a greedy polynomial-time heuristic to tackle this problem, which achieved over $98\%$ success rate on our tests in random networks, with false negatives contained within linear bounds of the parameters $n$ and $m$. These results align with the expected growth rate of the number of examples required for PAC learning shown in Equation ([1](#S2.E1)).

Future work includes deriving closed-form formulas for the probability of false negatives in the Waterfall, as well as testing its performance on real-world networks. Questions about how to incorporate noise into the model remain open. For example, consider probabilistic opinion changes or inaccurate opinion reports, which are common in clinical trials. Similarly, it is unclear how the heuristic performs when the diffusion protocol is also unknown, and even more so, when there is agent heterogeneity.

## Research Funding Statements

Dmitry Chistikov is supported in part by the Engineering and Physical Sciences Research Council [EP/X03027X/1]. Luisa Estrada acknowledges the support of the Engineering and Physical Sciences Research Council through the Mathematics of Systems II Centre for Doctoral Training at the University of Warwick [EP/S022244/1]. Paolo Turrini acknowledges the support of the Leverhulme Trust for the Research Grant RPG-2023-050 and the TAILOR Connectivity Fund (Agreement 29).

## Appendix A Full Proofs

###### Proof of Proposition 2 .

Let $L_{a}:=\{\ell_{a}\}_{a=1}^{m_{a}}$ and $L_{b}:=\{\ell_{b}\}_{b=1}^{m_{b}}$ be, respectively, the always-changing and never-changing subsets of the sample $\{\ell_{k}\}_{k=1}^{m}$, for some $m_{a}+m_{b}=m$. Since the network has unanimity dynamics, all influencers of agent $i$ must belong to the matching sets of the always-changing labellings. Therefore, $G_{i}\subseteq\bigcap_{\ell\in L_{a}}M(\ell)$, which also implies that $\bigcap_{\ell\in L_{a}}M(\ell)\neq\emptyset$ if a solution exists.

On the other hand, for never-changing labellings, there is no restriction on how many influencers may be in $M(\ell_{b})^{c}$, $\ell_{b}\in L_{b}$, as long as it is not all of them (i.e., $G_{i}\nsubseteq\bigcap_{\ell\in L_{a}}M^{c}(\ell)$). Moreover, for any superset $F\supseteq G_{i}$, if $G_{i}$ has at least one agent in the matching set of $\ell_{b}$, then so does $F$. Thus, any superset $F$ such that $G_{i}\subseteq F\subseteq\bigcap_{\ell\in L_{a}}M(\ell)$ will be a feasible influencer set. In particular, an algorithm that returns $\bigcap_{\ell\in L_{a}}M(\ell)$ qualifies as a CHF with time complexity $\mathcal{O}(m_{a}\,n^{2})$, further capped by $\mathcal{O}(m\,n^{2})$ for any oracle’s prediction.
∎

###### Proof of Proposition 3 .

We show that a feasible set of influencers of size at most $\kappa+1$ must exist. Consequently, an algorithm that generates all the subsets of agents of size $\kappa+1,\kappa,\kappa-1,\dots$, checks whether they are contained within each $M(\ell_{k})^{c}$, and returns the first one that is not contained in any of them, constitutes a CHF. Furthermore, since $\kappa$ is fixed, this CHF has a time complexity of $\mathcal{O}(m\,n^{\kappa+2})$

The first case is when the influencer set of agent $i$, $G_{i}\subseteq N$, is such that $|G_{i}|\leq\kappa+1$. But then, the exhaustive search over all subsets of size $\kappa+1$ or less will eventually reach $G_{i}$ and retrieve it as a feasible set.

On the other hand, when $|G_{i}|>\kappa+1$, it still cannot have more than $\kappa$ agents outside each matching set. In an always-changing sample, the agents in the matching set disagree with agent $i$ (i.e., $j\in M(\ell_{k})$ if and only if $\ell_{k}(j)=\neg\phi$). Therefore, for every $\ell\in(\ell_{k})_{k=1}^{m}$, any subset $F\subseteq G_{i}$ satisfies
κ≥—M(ℓ)^c ∩G_i—≥—M(ℓ)^c ∩F—, ensuring that $\sum_{j\in F}\mathds{1}(\ell(j)=\phi)\leq\kappa$. However, a feasible influencer set must have at least one agent who disagrees with agent $i$ for it to change opinion. This is guaranteed for any subset $F\subseteq G_{i}$, $|F|=\kappa+1$, as
—M(ℓ) ∩F—≥—F —- —M(ℓ)^c ∩F—≥1. Thus, our CHF returns the first subset of $G_{i}$ with $\kappa+1$ agents it finds during its search.
∎

###### Proof of Theorem 6 .

Suppose that there exists a polynomial-time algorithm $\mathcal{L}$ to learn the influencers of agents in social networks with majority dynamics. Given a budget $d>0$ and a set of sets $\{S_{1},\dots,S_{m}\}$, take the networks with $n+1$ agents, where $n=|\bigcup_{k=1}^{m}S_{k}|+d+1$. Pick a target agent $i$ and construct a set of $m+d+2$ example network labellings as in the proof of Theorem [5](#Thmtheorem5). Set the error parameter to

ε= 1m+d+3.

Run $\mathcal{L}$ to obtain with probability $\nicefrac{{1}}{{2}}$ a set of influencers $F\subseteq N$ for agent $i$, if a feasible set of influencers exists. If no such set exists, $\mathcal{L}$ halts without returning anything. The algorithm runs in polynomial time for $n$ and $\nicefrac{{1}}{{\varepsilon}}=m+d+3$, so it will be polynomial in the size of $\bigcup_{k=1}^{m}S_{k}$, the number of sets in $\{S_{1},\dots,S_{m}\}$ and the budget $d$.

Now, consider a uniform distribution $\mathcal{D}$ over the examples and recall that agent $i$ always changes opinion. Therefore, the error $\varepsilon$ of $F$ with respect to $\mathcal{D}$ is

$$ $\displaystyle\varepsilon$ $\displaystyle=\sum_{\ell\in\{1,0\}^{N}}\mathds{1}(f^{+}(\ell,F))\mathcal{D}(\ell)$ $\displaystyle=\frac{1}{m+d+2}\sum_{\ell\in(\ell_{k})_{k=1}^{m+d+2}}\mathds{1}(f^{+}(\ell,F)).$ $$

However, because $\varepsilon<(m+d+2)^{-1}$, we have that $f^{+}(\ell,F)$ is $\mathit{True}$ for all the labellings in our examples. Thus, $F$ is a feasible set of influencers, and from Theorem [5](#Thmtheorem5), we can reconstruct a hitting set for $\{S_{1},\dots,S_{m}\}$. Moreover, since the Hitting Set problem is NP-complete, $\mathcal{L}$ would be capable of solving any NP-hard problem while remaining RP itself.
∎

###### Proof of Proposition 2 .

###### Proof of Proposition 3 .

###### Proof of Theorem 6 .

## Appendix B In-Depth Analysis of the Waterfall Algorithm

When there are no agents that belong to the matching sets of all the labellings in our sample (i.e., $\cap_{k=1}^{m}M(\ell_{k})=\emptyset$), we need to strike a balance when picking agents from inside and outside the matching set of each labelling. There must never be strictly more agents that do not match the oracle prediction, and equality can only hold for non-changing labellings. We introduce the concepts of streams and waterfalls to keep track of this and provide a graphical intuition of the Waterfall.

### B.1 Streams and Waterfalls

###### Definition 8 .

Let $\mathbf{M}$ be the $m\times n$ matching transformation of a labelling sample $(\ell_{k})_{k=1}^{m}$. Then, the sequence $\mathbf{s}:=(s_{k})_{k=1}^{m}$ is a *stream* over $\mathbf{M}$ if $s_{k}\in M(\ell_{k})\subseteq N$ for $k=1,\dots,m$. Moreover, we say that a collection $W=\{\mathbf{s}^{1},\dots,\mathbf{s}^{w}\}$, $w\geq 1,$ of streams over $\mathbf{M}$ is a *waterfall* of size $w$ if $s^{u}_{k}\neq s^{v}_{k}$ for $k=1,\dots,m$ and any $u,v\in\{1,\dots,w\},\;u\neq v$.

###### Remark 4 .

We use $W_{k}:=\{s_{k}^{1},\dots,s_{k}^{w}\}$ as a shorthand for the agents that belong to a stream on the $k$-th row of $\mathbf{M}$.

###### Definition 9 .

The *ambit* of a waterfall $W$ of size $w\geq 1$ over an $m\times n$ matching transformation $\mathbf{M}$ is the minimal set of agents that can form all the streams in $W$. It is denoted by

$$ $\alpha(W):=\cup_{v=1}^{w}\cup_{k=1}^{m}s_{k}^{v}.$ $$

An easy way to avoid confusion between the size and ambit of a waterfall is to remember that size refers to the streams, while ambit refers to the agents.

We allude to streams and waterfalls as they can be visualised as paths flowing down the nodes of a matching transformation $\mathbf{M}$ without overlapping. We present how to use these devices to verify if a subset $F\subseteq N$ is a feasible influencer set.

###### Proposition 10 .

Let $\mathbf{M}$ be an $m\times n$ matching transformation. Then, a subset $F\subseteq N$ is a feasible set of influencers over $\mathbf{M}$ if and only if a waterfall $W$ of size $w\geq~\lceil|F|/2\rceil$ can be built over $\mathbf{M}_{F}$. Further, if $|F|$ is even and $w=|F|/2$, we allow $|M(\ell_{k})\cap F|=|F|/2$ only when $f^{+}(\ell_{k},F)$ is $\mathit{False}$.

###### Proof.

For $(\Leftarrow)$, we have that $|M(\ell_{k})\cap F|\geq\lceil|F|/2\rceil$ for $k=1,\dots,m$, since waterfall $W$ has at least that many streams flowing through $F$. Consequently, $\sum_{j\in F}\mathbf{M}_{kj}\geq 0$. If $\sum_{j\in F}\mathbf{M}_{kj}>0$, this means that there is a strict majority of agents in $F$ whose opinion in $\ell_{k}$ matches the output opinion of agent $i$ predicted by $f^{+}(\ell_{k},G_{i})$. When $\sum_{j\in F}\mathbf{M}_{kj}=0$, there is a tie, which can only occur on non-changing labellings. So, in either case, $F$ is a feasible set for $\ell_{k}$.

For $(\Rightarrow)$, we assume that $F$ is a feasible influencer set, so agent$~i$ agrees with at least half of the agents in $F$ matched the prediction after one opinion diffusion step of $f^{+}(\ell^{+}_{k},G_{i})$, for $k=1,\dots,m$. and Therefore, $|M(\ell_{k})\cap F|\geq|F|/2$. Ties occur if $|M(\ell_{k})\cap F|=|F|/2$, but since $F$ is a feasible influencer set, agent $i$ does not change opinion in this case, and $M(\ell_{k})$ are the agents who agree with agent $i$ in $\ell_{k}$. Thus, for each $\ell_{k}$, we can:

- 1.
Enumerate the agents in $M(\ell_{k})\cap F:=\{k_{1},\dots,k_{w_{k}}\}$, for some $w_{k}\geq|F|/2$.
- 2.
Pick $w:=\min_{k\in\{1,\dots,m\}}w_{k}\geq|F|/2$.
- 3.
Build a stream $\mathbf{s}^{v}=(k_{v})_{k=1}^{m}$ over $\mathbf{M}_{F}$ for every
$v=1,\dots,w$.

Since none of these streams overlap, together they form a waterfall $W=\{\mathbf{s}^{1},\dots,\mathbf{s}^{w}\}$ of size $w\geq|F|/2$.
∎

###### Proof of Proposition 7 .

Let $F:=\alpha(W)$ be the ambit of the waterfall and assume the Waterfall found a waterfall $W$ with the right ratio between $|\alpha(W)|$ and $w:=|W|$ according to Proposition [10](#Thmtheorem10). Given a source node $s\in N$, the algorithm has to update the state vector $\mathbf{r}$ at most $n$ times to build $W$. At each update, it computes the row sum of $\mathbf{M}_{\alpha(W)}$ and classifies it as C, CT, BC, IT or I. Next, it performs a column-sum over $\mathbf{M}_{\alpha(W)^{c}}$ to find how many labellings each agent rescues whilst keeping track of the set of top-rescuing agents. An agent from this set is selected in unit time.(^2^22 The selection can be refined by adding filters. This increases the complexity by at most a constant factor (number of filters).) These steps are sequential and take $\mathcal{O}(m\,n)$ time. Since up to $n$ agents can be added to $W$, building a waterfall from a source takes $\mathcal{O}(m\,n^{2})$ time. Iterating over all the possible source nodes $s\in N$ yields $\mathcal{O}(m\,n^{3})$ as claimed.
∎

###### Proof of Proposition 8 .

Let $\mathbf{M}$ be the matching transformation built from the labelling sample $(\ell_{k})_{k=1}^{m}$. When $|F|=0$, the Waterfall finds it because the first check it does is whether $\{\}$ is a feasible set of influencers. When $|F|=1$, we have that $\cap_{k=1}^{m}M(\ell_{k})\neq\emptyset$. This means that for any agent $j\in\cap_{k=1}^{m}M(\ell_{k})$, the algorithm will return $\alpha(W)=\{j\}$ as a feasible set of influencers once $j$ is the source node. Finally, let $|F|=\{j_{1},j_{2}\}$, for $j_{1},j_{2}\in N$, $j_{1}\neq j_{2}$. Without loss of generality, assume the Waterfall goes over agent $j_{1}$ as a source node and that the state vector of $\mathbf{M}_{\{j_{1}\}}$ has $m_{1}<m$ labellings that need rescuing. If $F=\{j_{1},j_{2}\}$ is a feasible set, then $j_{2}$ is a perfect match for $\{j_{1}\}$ and must rescue $m_{1}$ labellings. Any other agent tied with $j_{2}$ must also rescue $m_{1}$ labellings, so they will also be a perfect match for $\{j_{1}\}$. Regardless of which of these agents the Waterfall picks, the next validation point will find a feasible set.
∎

###### Proof of Proposition 9 .

Notice that one additional agent can only turn a labelling $\ell\in L^{N}$ from inconsistent to consistent if its margin is $M(\ell)\cap\alpha(W)|-|M(\ell)^{c}\cap\alpha(W)|\geq-1$. Additionally, we require equality if $f^{+}(\ell,G_{i})$ is $\mathit{False}$. Let $\bar{\mathbf{r}}$ be the state vector associated to $\mathbf{M}_{\alpha(W)}$, whose labellings that need rescuing are stored in
R:={k≤m : ¯r_k ∈{ I_1, IT, CT, BC}},   —R—= m_α¡m.
If $\alpha(W)$ has a perfect match, then all the agents in
argmax_j∈α(W)^c∑_r∈R 1(j∈M(ℓ_r)),
rescue $m_{\alpha}$ labellings. Regardless of which agent the Waterfall picks, the next validation point will find they form a feasible set as the new state vector will either have positive entries or maybe none where $f^{+}(\ell,G_{i})$ is $\mathit{False}$.
∎

###### Definition 8 .

###### Remark 4 .

###### Definition 9 .

###### Proposition 10 .

###### Proof.

###### Proof of Proposition 7 .

###### Proof of Proposition 8 .

###### Proof of Proposition 9 .

### B.2 Why Do We Call It the Waterfall ?

###### Example 2 .

Suppose the Waterfall receives as input the $m\times n$ matching transformation $\mathbf{M}$ and the oracle prediction for $(\ell_{k})_{k=1}^{m}$ shown in Figure [4(a)](#A2.F4.sf1). Thus, we have a social network with $n=6$ agents plus the target agent $i$ and a sample of $m=8$ examples. We initialise our waterfall on the source node $i_{1}\in N$. We bring an agent to the left of the state vector $\bar{\mathbf{r}}$ to show we added it to $\alpha(W)$. The state of each $\bar{\mathbf{r}}_{k}$ depends on the oracle prediction for $\ell_{k}$ and the sign of $|M(\ell_{k})\cap\alpha(W)|-|M(\ell_{k})^{c}\cap\alpha(W)|$.

For each row in the restricted matrix $\mathbf{M}_{\alpha(W)}$, we number the entries that belong to the majority set from left to right. Then, we let the streams in $\alpha(W)$ flow through the nodes with the same value. The size $w:=|W|$ of our waterfall is the minimax of these values.(^3^33 *Minimax:* Minimum of the maximums.)

In general, our assumption of $\cap_{k=1}^{m}M(\ell_{k})=\emptyset$ implies we cannot form a stream with a single node. This is seen in Figure [4(a)](#A2.F4.sf1) with $|\alpha(W)|=1$ and $w=0$. In it, $\alpha(W)$ is unfeasible for the labellings $\ell_{1},\ell_{2}$ and $\ell_{3}$, although only $\ell_{6}$ and $\ell_{7}$ do not need rescuing. Here, agent $i_{5}$ rescues the most labellings. We add it to $\alpha(W)$ to obtain Figure [4(b)](#A2.F4.sf2).

We have one stream over two agents if $\alpha(W)=\{i_{1},i_{5}\}$, so it has the potential to be a feasible set. Nevertheless, $\ell_{3}$ and $\ell_{5}$ have inconsistent ties. Thus, we need to build another stream. In this step, agent $i_{2}$ rescues the most labellings, so we add it to $\alpha(W)$, which yields Figure [5(a)](#A2.F5.sf1). Finally, because $\alpha(W)=\{i_{1},i_{5},i_{2}\}$ satisfies that $\lceil|\alpha(W)|/2\rceil=w=2$, Proposition [10](#Thmtheorem10) tells us that $\alpha(W)$ is a feasible set of influencers. This is further verified by the lack of inconsistent states in $\bar{\mathbf{r}}$. Consequently, the *else* condition in the Waterfall is met, and it returns $\alpha(W)$.

Figure: (a) Agent $i_{1}$ as source node in the Waterfall, $|\alpha(W)|=1,\;w=0.$
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/x3.png

Figure: (a) A feasible influencer set $\alpha(W)$ is found as per Proposition [10](#Thmtheorem10), $|\alpha(W)|=2,\;w=1.$
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/x5.png

###### Example 2 .

### B.3 Implementing the Tie-Breaking Subroutine

More robust versions of the Waterfall can be achieved by implementing tie-breaking subroutines. For example, Algorithm [2](#alg2) presents a subroutine that recalculates the labellings that need to be rescued by the tied agents, filtering out the most consistent labels from the last tie. That is, it first omits the BC states and then, if ties persist, it progressively ignores the labellings that satisfy $|M(\ell)\cap F|-|M(\ell)^{c}\cap F|\geq\tau$, raging $\tau$ from $\tau=0$ down to $\tau=-|F|$, or until there are no more ties. Notably, as seen in Figures [6](#A2.F6) and [7](#A2.F7), this subroutine ensures the feasible influencer set for $N=[4]$ is at most $|G_{i}|$, unlike the single-filter version. Yet, this comes with an increased complexity determined by the number of filters, which sets how many times the algorithm recalculates the labellings rescued by each agent in $\alpha(W)^{c}$. Since filters are capped by the sample size, the filtered version of the Waterfall terminates in $\mathcal{O}(m^{2}\,n^{3})$ time.

Figure: Algorithm 2 Filters subroutine

Figure: Figure 6: Number of agents in the feasible set $\alpha(W)$ compared to the true influencer set $G_{i}$ for networks with at most $5$ agents. The Waterfall in its single-filter version, with a uniformly at random tie-breaking criterion.
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/Gi_vs_ambit_single_filter.png

Figure: Figure 7: Number of agents in the feasible set $\alpha(W)$ compared to the true influencer set $G_{i}$ for networks with at most $5$ agents. The Waterfall is run using the tie-breaking subroutine from Algorithm [1](#alg1) in Section [B.3](#A2.SS3).
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/Gi_vs_ambit_multi_filter.png

## Appendix C Parameter Choice for Graph Generation Models

We test our algorithms on four random network structures: Erdős–Rényi (ER), Watts–Strogatz (WS), Regular Graph (RG), and Barabási–Albert (BA). All graphs are generated using the Python library NetworkX (version 3.5) with Python version 3.11. For fair comparison, given a sparsity regime $p\in\{0.1,0.25,0.5,0.75,0.9\}$, we define $\texttt{p${}_{1}$}=p$ and $\texttt{p${}_{2}$}=2+p\cdot(\frac{n}{2}-1)$, rounding p2 to the nearest even integer. All graphs are generated using the functions and parameters detailed below and then converted to their directed versions (i.e., undirected edges become bidirectional). Omitted parameters use default values.

- -
ER: gnp_random_graph(n, p=p1),
- -
WS: watts_strogatz_graph(n, k=p2, p=p1),
- -
RG: random_regular_graph(d=p2, n) and
- -
BA: barabasi_albert_graph(n, m=p2).

For a network of size $n\geq 1$, labelling samples were created using NumPy (version 1.24) via np.random.choice([-1, 1], size=n). Individual labellings were appended to a set to ensure no duplicates until the desired sample size $m$ was reached. Within the Waterfall algorithm, ties were resolved uniformly at random, again via np.random.choice. Parallel execution was handled using Python’s built-in concurrent.futures.

To ensure reproducibility, a global master_seed was set at the start of each experiment with parameters $(n,m)$. Each subprocess had its unique seed from offsetting the master seed with the process index (i.e., seed = master_seed + process_id), so that each parallel worker drew from an independent and deterministic stream. We applied this consistently to NetworkX’s graph generators, NumPy’s sampling, and the Waterfall’s tie-breaks. Experiments were run on Ubuntu 22.04 with 16GB RAM and a $12^{th}$ Gen Intel Core i7-1260P CPU (16 threads), using Visual Studio Code (version 1.102). No dedicated GPU was used.

Experimental results, differentiated by network type and sparsity regime, are shown in Figure [8](#A3.F8) and Figure [9](#A3.F9), respectively.

Figure: Figure 8: Waterfall FNR by network type. Each grid point corresponds to tests on 40 networks with $n$ agents per sparsity value, on 50 shared labelling samples of size $m$.
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/FNR_grid_net_type.png

Figure: Figure 9: Waterfall FNR by sparsity regime. Each grid point corresponds to tests on 10 networks with $n$ agents per graph type, on 50 shared labelling samples of size $m$.
Refer to caption: https://arxiv.org/html/2605.15033v1/2605.15033v1/FNR_grid_by_p.png
