## Abstract

Abstract Large language models (LLMs) achieve strong performance across a wide range of tasks, but remain frozen after pretraining until subsequent updates.
Many real-world applications require timely, domain-specific information, motivating the need for efficient mechanisms to incorporate new knowledge.
In this paper, we introduce MeMo ( Me mory as a Mo del), a modular framework that encodes new knowledge into a dedicated Memory model while keeping the LLM parameters
unchanged.
Compared to existing methods, MeMo offers several advantages: (a) it captures complex cross-document relationships, (b) it is robust to retrieval noise, (c) it avoids catastrophic forgetting in the LLM, (d) it does not require access to the LLM’s weights or output logits, enabling plug-and-play integration with both open and proprietary closed-source LLMs, and (e) its retrieval cost is independent of corpus size at inference time.
Our experimental results on three benchmarks, BrowseComp-Plus, NarrativeQA, and MuSiQue, show that MeMo achieves strong performance compared to existing methods across diverse settings.

## 1 Introduction

Large language models (LLMs) have demonstrated remarkable capabilities across diverse tasks .
Despite their successes, these models are effectively *frozen* for extended periods after pretraining  until subsequent updates,
causing their pretrained knowledge to become increasingly outdated as the world evolves.
For applications that require up to date  or domain-specific  knowledge, this dependence on static knowledge presents a fundamental architectural limitation .
Retraining is a natural solution but remains prohibitively expensive at modern scales , motivating the need for an efficient mechanism to integrate new external knowledge into LLMs without full retraining.

Figure: Figure 1: Overview of the training and inference pipeline of MeMo. During Memory model training (left), a frozen Generator model transforms a target corpus into a reflection QA dataset via fact extraction, consolidation, verification, entity surfacing, and cross-document synthesis, which is then used to train a dedicated Memory model. During inference (right), the frozen Executive model answers complex user queries by querying the Memory model through a structured multi-turn protocol: it decomposes the input into simpler, targeted sub-queries, retrieves intermediate responses from the Memory model, and reasons over them to produce a final answer to the user’s query.
Refer to caption: https://arxiv.org/html/2605.15156v1/2605.15156v1/x1.png

Existing methods for integrating new knowledge into LLMs fall into three categories.
\small1⃝ Non-parametric methods retrieve relevant information from an external store at inference time via lexical , dense , or graph-based retrievers , before incorporating it through in-context learning .
However, these methods are constrained by limited context windows and struggle to synthesize cross-document relationships when relevant information is distributed across multiple documents .
\small2⃝ Parametric methods internalize knowledge directly into model parameters via continual pretraining  or fine-tuning  on the target corpus directly. While effective, they are computationally expensive, prone to catastrophic forgetting , and tend to memorize training distributions rather than acquire transferable knowledge,
limiting generalization to unseen queries .
\small3⃝ Latent memory methods  compress knowledge into soft tokens or other model-specific representations, but suffer from representation coupling: the memory is tightly bound to the specific model used to produce these representations, limiting transferability across LLMs.

We introduce MeMo (Memory as a Model), a modular framework where a dedicated Memory model is trained on new knowledge, and an Executive model retrieves relevant information from the Memory model at inference time via targeted sub-queries and then reasons over the retrieved information to respond to user queries.
MeMo combines the complementary strengths of the three paradigms above while mitigating their individual limitations.
Like the non-parametric methods, it is able to leverage off-the-shelf frontier models unchanged by separating the memory from the reasoning model; it shares with the parametric methods the ability to internalize knowledge in model parameters, and it shares the benefits of a compact, queryable memory artifact with latent memory methods.
As a result, MeMo offers the following advantages: (a) it captures complex cross-document relationships,
(b) it is robust to retrieval noise,
(c) it avoids catastrophic forgetting by keeping the Executive model parameters unchanged,
(d) it does not require access to the Executive model’s weights or output logits, enabling plug-and-play integration with both open and proprietary LLMs, and
(e) its retrieval cost is independent of corpus size at inference time due to the fixed size of the Memory model.
However, designing MeMo to comprehensively capture cross-document relationships during training while accurately answering arbitrary queries at inference time introduces two key challenges, which we outline below and address them with novel methods.

\small1⃝Training Memory model.
A core challenge in the Memory model is ensuring it can accurately answer diverse, unseen queries at inference time, including those requiring cross-document reasoning and long-context understanding.
A natural approach is to train directly on the raw corpus using standard data augmentation techniques such as paraphrasing , additional sampling of generated QA pairs , or targeted gap-filling, where the model identifies and completes missing knowledge from the corpus . However, these approaches fail to consolidate related facts into compositional representations necessary for robust generalization to unseen queries .
With this challenge in mind, we design a novel *five-step data synthesis pipeline* guided by a Generator model ([Section˜4.1](#S4.SS1)) that distills the corpus into a question–answer (QA) dataset of *reflections*: compositional representations that expose underlying corpus knowledge under diverse query variations (illustrated in [Fig.˜1](#S1.F1) (left) and details in [Section˜4.1](#S4.SS1)).
We train Memory model on the synthesized reflection QA dataset via supervised fine-tuning (see [Section˜4.2](#S4.SS2)), enabling Memory model to capture more complex, cross-document relationships and compositional structure than retrieval-based methods.

\small2⃝Querying Memory model.
At inference time, complex or compositional queries often require multi-step reasoning and aggregation of information across multiple documents. Naively querying Memory model via single-turn or unstructured multi-turn interactions fails to reliably retrieve the knowledge required to answer such queries.
To address this, we design a *three-stage inference pipeline* in which Executive model queries and retrieves information from Memory model via a structured multi-turn protocol, decomposing complex user queries into targeted sub-queries that align with the shared reflection interface (illustrated in [Fig.˜1](#S1.F1) (right) and more details are in [Section˜4.4](#S4.SS4)).
Unlike retrieval-based methods, this approach incurs retrieval cost independent of corpus size and is robust to retrieval noise (see [Section˜5.2](#S5.SS2))
Crucially, because MeMo treats Executive model as a black box and does not access its weights, gradients, or output logits, it supports *plug-and-play* integration with *any LLM*, including both both open and proprietary closed-source models.

Our method is guided by a single design principle: *reflections*, corpus-derived structures that require no knowledge of future queries, yet naturally serve as the precise interface through which any query can access the underlying corpus without ever observing it directly. During training, the Memory model internalizes these reflections; Executive model retrieves relevant knowledge through targeted sub-queries at inference time.
Building on the challenges outlined above and the methods proposed to address them, we summarize the key contributions of this paper as follows:

- •
Novel data synthesis pipeline.
We propose a five-step data synthesis pipeline that uses a Generator model, an LLM that may be the same as or smaller than Executive model, to distill a target corpus into reflections, enabling a dedicated Memory model to internalize knowledge in compositional forms that capture more complex cross-document relationships and generalize robustly to diverse, unseen query variations at inference time (see [Sections˜4.1](#S4.SS1) and [4.2](#S4.SS2)).
- •
Structured multi-turn protocol.
We introduce a *structured* multi-turn protocol that systematically decomposes complex queries into targeted sub-queries aligned with the shared reflection interface. The protocol supports plug-and-play integration with any arbitrary LLM, including proprietary closed-source LLMs, and has retrieval cost independent of corpus size (see [Section˜4.4](#S4.SS4)).
- •
Empirical validation.
We evaluate MeMo on BrowseComp-Plus, NarrativeQA, and MuSiQue, demonstrating strong performance against both parametric and non-parametric baselines. We further empirically validate MeMo’s robustness to retrieval noise (see [Section˜5](#S5)).

## 2 Related Work

Non-parametric methods.
Non-parametric alternatives  avoid parameter updates entirely, instead, supplying new knowledge at inference time.
In particular, in-context learning (ICL)  inserts relevant knowledge directly into the prompt, avoiding catastrophic forgetting.
However, ICL scales poorly with increasing context length: the computational cost of autoregressive generation  leads to substantial token overhead and inference latency as the knowledge base grows , and even explicitly long-context models exhibit significant performance degradation as context length increases .
Retrieval-augmented generation (RAG)  addresses this scalability bottleneck by selectively retrieving relevant chunks of knowledge rather at inference time.
However, RAG systems are highly sensitive to retrieval noise , where irrelevant or misleading passages substantially degrade generation quality .
In addition, RAG systems often struggle to reason over complex cross-document dependencies , as they lack robust mechanisms for synthesizing information that is distributed across multiple chunks or a large corpus .

Parametric methods.
Existing post-training approaches, such as continual pretraining on new corpora  or supervised fine-tuning (SFT) on curated instruction data , attempt to address this limitation by incorporating new knowledge into LLMs during post-training.
While conceptually straightforward, these parametric methods often suffer from catastrophic forgetting, whereby adaptation to newly observed knowledge degrades previously acquired knowledge, learned capabilities , and erodes safety alignment learned during LLM post-training .
In addition, the scale of modern LLMs makes frequent fine-tuning computationally expensive , and fine-tuning is often infeasible for proprietary, closed-source models , substantially limiting the practicality of parametric methods in real-world, large-scale applications.

Latent memory methods.
Another approach to storing knowledge is via *compressed latent representations*, which lie between non-parametric retrieval and fully parametric methods. Context compression techniques such as AutoCompressor , Gist tokens , and ICAE  encode knowledge into compact soft tokens prepended at inference, reducing ICL token overhead without discarding information.
However, these representations are tightly coupled to the encoder and cannot be consumed by other model families, limiting compatibility with black-box LLMs.
Similarly, recurrent-state models  and nearest-neighbor memory methods such as Memorizing Transformers  and $k$NN-LM  rely on model-specific representations or architectures, preventing post hoc use with pretrained LLMs.
Although Memory Decoder  is a plug-and-play pretrained memory module that integrates without modifying model parameters, it is limited to architectures sharing a common tokenizer, enabling reuse only within this subset.
The core limitation of these methods is *representation coupling*: latent memory is inseparable from the model that produces it. In contrast, MeMo allows a plug-and-play integration with any LLM, including closed-source models.

**Table 1: A comparison of desirable properties across different memory paradigms, showing that MeMo satisfies them through its modular memory construction and memory-augmented reasoning.**
| Methods | Frozen<br>base LLM | No<br>retrieval index | Black-box<br>compatible | No catastrophic<br>forgetting | Constant-size<br>memory | Cross-LLM<br>transferable |
| --- | --- | --- | --- | --- | --- | --- |
| Non-parametric (RAG, ICL) | ✓ | $\times$ | ✓ | ✓ | $\times$ | ✓ |
| Parametric (CPT, SFT) | $\times$ | ✓ | $\times$ | $\times$ | $\times$ | $\times$ |
| Latent memory (AutoCompressor, Gist, ICAE) | ✓ | ✓ | $\times$ | ✓ | ✓ | $\times$ |
| MeMo (Ours) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## 3 Preliminaries

Problem setting.
Let $\mathcal{M}_{\theta}$ denote a large language model with frozen parameters $\theta\in\mathbb{R}^{p}$, pretrained on a corpus $\mathcal{D}_{\text{pre}}$. We treat $\mathcal{M}_{\theta}$ as a conditional distribution that maps a prompt $x$ to a response $\mathcal{M}_{\theta}(x)$, and assume only black-box access; in particular, $\mathcal{M}_{\theta}$ may be either a white-box model or a closed-source model accessed via API. Let $\mathcal{D}=\{d_{1},\ldots,d_{N}\}$ denote a target corpus of $N$ documents containing knowledge that $\mathcal{M}_{\theta}$ cannot reliably recall(^1^11We do not assume $\mathcal{D}$ is disjoint from $\mathcal{D}_{\text{pre}}$, as training data is rarely disclosed by model providers. A document is considered effectively absent from $\mathcal{M}_{\theta}$’s knowledge if the model fails to answer questions grounded in it, either because it never appeared in $\mathcal{D}_{\text{pre}}$ or because the training process was insufficient to retain it. For more information, refer to [Appendix I](#A9).).
Let $\mathcal{Q}$ be a set of queries, each $q\in\mathcal{Q}$ associated with a ground-truth answer $a^{\star}(q)$ and a set of supporting documents $\mathcal{S}(q)\subseteq\mathcal{D}$. Note that $\mathcal{S}(q)$ is a theoretical construct used to characterize query complexity.

Knowledge integration mechanism.
A *knowledge integration mechanism* is a pair $(\Phi,f)$, where $\Phi$ maps the corpus to a representation $\mathcal{K}\doteq\Phi(\mathcal{D})$ and $f$ combines $\mathcal{K}$ with $\mathcal{M}_{\theta}$ at inference to produce responses $f(\mathcal{M}_{\theta},\mathcal{K},q)$. We formalize the goal as follows.

###### Definition 1 (Knowledge Integration Problem) .

Given a frozen model $\mathcal{M}_{\theta}$ and target corpus $\mathcal{D}$, find a mechanism $(\Phi,f)$ such that, without modifying $\theta$, for all $q\in\mathcal{Q}$,
$\mathbb{P}\left\{f(\mathcal{M}_{\theta},\Phi(\mathcal{D}),q)=a^{\star}(q)\right\}=1.$

Existing approaches.
Existing methods differ in their choice of $(\Phi,f)$. *ICL* sets $\mathcal{K}=\mathcal{D}$ and $f(\mathcal{M}_{\theta},\mathcal{K},q)=\mathcal{M}_{\theta}([\mathcal{D};q])$, i.e., appending the corpus directly to the prompt. *RAG* constructs $\mathcal{K}$ as a retrieval index and defines $f$ to retrieve a subset $\hat{\mathcal{S}}\subseteq\mathcal{D}$ before passing $[\hat{\mathcal{S}};q]$ to $\mathcal{M}_{\theta}$. *Fine-tuning* sets $\mathcal{K}=\emptyset$ and $f=\mathcal{M}_{\theta^{\prime}}$, where $\theta^{\prime}$ is obtained by updating $\theta$ on $\mathcal{D}$. In contrast, MeMo defines $\mathcal{K}$ as the parameters of a small, dedicated Memory model $\mathcal{M}_{\varphi}$ with $\varphi\ll\theta$, trained on reflection QA dataset derived from $\mathcal{D}$, and queried by a frozen Executive model $\mathcal{M}_{\theta}$ at inference time. [Table˜1](#S2.T1) summarizes how these paradigms compare across desirable properties.

###### Definition 1 (Knowledge Integration Problem) .

## 4 MeMo: Memory as a Model

MeMo addresses the knowledge integration problem (Def. [1](#Thmdefi1)) through two components: a frozen *model* $\mathcal{M}_{\theta}$ (Executive model), which handles reasoning and responds to user queries, and a *Memory model* $\mathcal{M}_{\varphi}$, which is trained to encode knowledge in its parameters from a target corpus $\mathcal{D}$.
Our pipeline operates in two phases: (i) a *training phase* that constructs Memory model from $\mathcal{D}$, and (ii) an *inference phase* in which Executive model queries and retrieves information from Memory model to answer knowledge-intensive questions (see [Sections˜4.1](#S4.SS1), [4.2](#S4.SS2) and [4.4](#S4.SS4)).

### 4.1 Data Synthesis Pipeline

Given a corpus of documents $\mathcal{D}$, our objective in the data generation process is to construct a reflection QA dataset $\mathcal{Q}_{\text{final}}$ that captures both single-document facts and cross-document relationships. This process is driven by a Generator model $\mathcal{M}_{\text{gen}}$ and proceeds through five steps, as summarized in [Algorithm˜1](#alg1) and illustrated in [Fig.˜1](#S1.F1):
(1) fact extraction from raw documents,
(2) consolidation of redundant or overlapping information,
(3) verification and rewriting to ensure correctness and clarity,
(4) entity surfacing to explicitly represent key entities, and
(5) cross-document synthesis to integrate evidence across the corpus. Importantly, no document identifiers or watermarks are embedded in the generated QA pairs at any step, preventing Memory model from exploiting shortcut signals during evaluation.

Figure: Algorithm 1 Reflection QA Dataset Generation Pipeline from Target Corpus

Step 1: Fact extraction.
Each document $d\in\mathcal{D}$ is segmented into chunks $C$, where each chunk corresponds either to an entire document or to a contiguous segment of a longer document.
For each chunk, $\mathcal{M}_{\text{gen}}$ performs two parallel extraction processes: *direct extraction*, which captures explicitly stated facts (producing $\mathcal{Q}_{\text{dir}}$), and *indirect extraction*, which targets inferred or synthesized information beyond the surface text (producing $\mathcal{Q}_{\text{indir}}$).
This dual extraction process ensures that both factual recall and inferential reasoning are represented in the training signal for Memory model.

Step 2: Consolidation.
The Generator model $\mathcal{M}_{\text{gen}}$ consolidates $\mathcal{Q}_{\text{dir}}\cup\mathcal{Q}_{\text{indir}}$ by identifying QA pairs that share a common underlying context (such as entity, time period, or relationship type) and combining them into QA pairs that encompass multiple facts, denoted $\mathcal{Q}_{\text{mrg}}$. This merging process produces training instances that require integrating multiple facts within the same contextual chunk, going beyond single-fact question answering pairs. The synthesized QA pairs are subsequently unified with the original sets to form the consolidated dataset $\mathcal{Q}_{\text{con}}=\mathcal{Q}_{\text{dir}}\cup\mathcal{Q}_{\text{indir}}\cup\mathcal{Q}_{\text{mrg}}$.

Step 3: Verification and rewriting.
Each QA pair in $\mathcal{Q}_{\text{con}}$ is evaluated for *self-containment* by $\mathcal{M}_{\text{gen}}$, i.e., whether it can be
fully understood and correctly answered in isolation, without access to the source chunk.
Common failure modes include unresolved pronouns (e.g., “What did *they* propose?”) and implicit references (e.g., “As noted in the above table…”).
Non-self-contained QA pairs are rewritten by $\mathcal{M}_{\text{gen}}$ using the source chunk $C$ as context; QA pairs that remain ambiguous after rewriting are discarded. This check-and-rewrite procedure yields the verified set $\mathcal{Q}_{\text{ver}}$, a set of QA pairs that can be used as training examples without access to the source chunk.

Step 4: Entity surfacing.
For each named entity in $\mathcal{Q}_{\text{ver}}$, $\mathcal{M}_{\text{gen}}$ generates a set of entity-surfacing QA pairs in which the question encodes the entity’s attributes and relationships (including connections to other named entities) and the answer reveals its identity. Facts about each entity are aggregated across all QA pairs within the chunk prior to generation, enabling the integration and composition of information from multiple source pairs. Questions are generated at varying levels of complexity, ranging from single-fact to multi-fact queries. These pairs, denoted $\mathcal{Q}_{\text{ent}}$, aim to mitigate the reversal curse  by training Memory model to infer entities from indirect or partially specified descriptions. This capability supports the *entity identification turn* at inference time ([Section˜4.4](#S4.SS4)).

Step 5: Cross-document synthesis.
The final step operates over pre-defined document groups $\mathcal{G}=\{G_{1},\ldots,G_{k}\}$, where chunks within each group $G_{i}$ are topically related.
Such groups arise naturally, for example, when a large document is segmented into chunks (forming a single group) or from human-provided labels.
For each group $G_{i}$, $\mathcal{M}_{\text{gen}}$ is provided with the entity-surfacing pairs ${\mathcal{Q}_{\text{ent}}^{d}:d\in G_{i}}$ from all member documents and identifies two types of cross-document connections:

- •
*Converging clues*: multiple documents provide complementary facts about the same entity, which together enable its identification.
- •
*Parallel properties*: different entities across documents share a common attribute or role, enabling comparative and analogical reasoning.

Both types yield QA pairs with support size $s(q)>1$ ([Section˜3](#S3)), directly targeting the cross-document synthesis objective. The final dataset is $\mathcal{Q}_{\text{final}}=\mathcal{Q}_{\text{ver}}\cup\mathcal{Q}_{\text{ent}}\cup\mathcal{Q}_{\text{cross}}$, which collectively captures self-contained, entity-centric, and cross-document reflections for training Memory model.

Ablations of the pipeline design are presented in [Appendix˜E](#A5).

### 4.2 Training the Memory model

Given $\mathcal{Q}_{\text{final}}$, Memory model is trained via supervised fine-tuning to map questions directly to answers *without* access to source documents at inference time.
Memory model is initialized from a small pretrained language model, substantially smaller than Executive model (e.g., 1.5B vs. 32B parameters), and optimized by minimizing the next-token prediction loss over answer tokens only.

$$ $\mathcal{L}(\varphi)\;=\;-\!\!\!\sum_{(q_{i},\,a_{i})\,\in\,\mathcal{Q}_{\text{final}}}\;\;\sum_{t=1}^{|a_{i}|}\log\mathcal{M}_{\varphi}\!\left(a_{i}^{(t)}\;\middle|\;q_{i},\,a_{i}^{(1:t-1)}\right).$ $$

Conditioning only on the question and preceding answer tokens, and never on source documents, forces Memory model to internalize knowledge *parametrically* rather than rely on copying from retrieved context.
This constitutes a key distinction from RAG-based readers: at inference time, Memory model generates answers solely from its internalized parametric knowledge, without access to any external corpus.
Further details on hyperparameter choices and training paradigms (full SFT vs. LoRA) are provided in [Appendix˜F](#A6) and [Appendix˜O](#A15), respectively.

### 4.3 Continual Knowledge Integration via Model Merging

A practical desideratum of any knowledge integration system is the ability to incorporate new corpora incrementally without retraining on or rebuilding from all previously ingested sources. For parametric models, integrating new knowledge typically requires retraining on the union of all observed corpora, a cost that grows prohibitively with the number of sources. In contrast, non-parametric systems such as knowledge graphs and vector databases support efficient incremental updates. We explore *model merging*  as an approach to close this gap for parametric models. Model merging aims to preserve knowledge from multiple sources without requiring joint training on their union, by combining $K$ Memory model models, each trained independently on a distinct corpus, into a single model.

Continual knowledge integration.
Let $\{\mathcal{D}_{1},\dots,\mathcal{D}_{K}\}$ be a collection of pairwise disjoint target corpora. For each corpus $\mathcal{D}_{i}$, we generate a reflection QA dataset $\mathcal{Q}^{(i)}_{\text{final}}$ ([Section˜4.1](#S4.SS1)) and train a corresponding Memory model $\mathcal{M}_{\varphi_{i}}$ via SFT ([Section˜4.2](#S4.SS2)), initializing all $K$ models from the same pretrained base $\mathcal{M}_{\varphi_{0}}$. We define the *task vector* for $\mathcal{D}_{i}$ as $\tau_{i}\;{=}\;\varphi_{i}-\varphi_{0}$, capturing the parametric shift induced by training on $\mathcal{D}_{i}$ alone. The merged Memory model is then obtained as

$$ $\varphi_{\text{merged}}\;{=}\;\mathrm{Merge}(\varphi_{0},\,\{\tau_{i}\}_{i=1}^{K};\,\Theta),$ $$

where $\Theta$ denotes method-specific hyperparameters (e.g., merging coefficients, sparsification densities). We discuss alternative merging methods and their respective limitations in [Appendix˜H](#A8).

### 4.4 Inference-Time Integration

At inference time, Executive model queries and retrieves information from Memory model through a structured multi-turn protocol, with Executive model treating Memory model as an external knowledge oracle.
The pipeline has three sequential stages, each designed to progressively improve the likelihood of producing a correct final answer, as illustrated in [Fig.˜1](#S1.F1) (right).
Each stage utilizes distinct prompts, sampling temperatures and independent budgets to control the number of interactions between Executive model and Memory model.

Stage 1: Grounding.
Given a query $q$, Executive model decomposes it into a set of atomic, clue-probing sub-questions $\{q_{1}^{\prime},\ldots,q_{K}^{\prime}\}$, where each sub-question targets a single identifying constraint in $q$, and $K$ is adaptively determined by Executive model.
The Memory model answers each sub-question independently, without shared context, producing grounding responses $\{m_{1},\ldots,m_{K}\}$.
These responses draw on Memory model’s parametric knowledge to provide additional contextual grounding for subsequent interactions in the later stages.

Stage 2: Entity identification.
Using the grounding responses as context, Executive model iteratively narrows a set of candidate entities by issuing targeted follow-up sub-queries to Memory model across multiple interactions.
This process continues until Executive model converges on a single entity $e^{\star}$ or the stage budget is exhausted. If no candidates are identified, Stage 3 is skipped and Executive model synthesizes a final answer from the grounding responses alone.
This stage leverages Memory model’s training on the entity-surfacing QA pairs $\mathcal{Q}_{\text{ent}}$ ([Section˜4.1](#S4.SS1)).

Stage 3: Answer seeking and synthesis.
Conditioned on the identified entity $e^{\star}$, Executive model queries Memory model for additional supporting facts through targeted follow-up questions.
Once sufficient evidence is gathered, or the stage budget is exhausted, Executive model synthesizes the accumulated responses into a final answer:

$$ $\hat{a}\;=\;\mathcal{M}_{\theta}\!\bigl(q,\;\{m_{k}\}_{k=1}^{K},\;e^{\star},\;m_{\text{seek}}\bigr).$ $$

Notably, the Memory model responses $m_{k}$ and $m_{\text{seek}}$ are compact natural-language snippets whose lengths are independent of the corpus size, ensuring constant-time inference.
As all interactions with $\mathcal{M}_{\theta}$ occur through its input–output interface, MeMo remains fully compatible with black-box Executive models, including proprietary APIs, without requiring access to internal parameters. For full implementation details, refer to [Appendix˜J](#A10) and the supplementary materials.

## 5 Experiments

Datasets.
We evaluate MeMo on three knowledge-intensive benchmarks. BrowseComp-Plus  is a deep-research benchmark requiring multi-hop, multi-document retrieval and reasoning; we filter non-English instances with LangDetect , sample 300 questions, and pair each question’s evidence documents with an equal number of negative documents,(^2^22BrowseComp-Plus and MuSiQue provide annotations of gold (correct), evidence (supporting), and negative (distractor) documents. Gold documents are a subset of the evidence documents.) yielding 3,541 documents in total.
NarrativeQA  tests discourse understanding over long documents such as books and movie scripts; we use 293 questions across 10(^3^33We follow HippoRAG2 and evaluate on 10 such documents from the NarrativeQA validation split (294 questions); one duplicate is removed for consistency.) documents.
MuSiQue  requires composing 2–4 reasoning steps across multiple Wikipedia paragraphs; we use 1,000 questions and construct the target corpus following the same procedure as for BrowseComp-Plus, yielding 5,296 documents. Further details are in [Appendix˜D](#A4); datasets and code are in the supplementary materials.

Baselines.
We compare MeMo against four baselines: BM25  (lexical retrieval), NV-Embed-V2  (dense retrieval), HippoRAG2  (graph-based RAG, state-of-the-art), and Cartridges  (a trained KV-cache loaded onto Executive model at inference; the closest existing parametric baseline to MeMo). Newer methods exist  but typically require white-box access to Executive model and are therefore not directly comparable. We additionally include Perfect Retrieval as an empirical upper bound, where Executive model receives exclusively the evidence documents in context . Retrieval baselines use top-$k{=}9$ with adaptive backoff: reducing $k$ progressively until the retrieved context fits Executive model’s context window.

Implementation and evaluation.
*(a) Data generation.* We use Qwen2.5-32B-Instruct  as the Generator model, served via vLLM  with YaRN RoPE scaling  to support a 131K-token context window.
*(b) Training.* We train Memory model initialized from Qwen2.5-14B-Instruct for 3 epochs with fused AdamW  and DeepSpeed 2  at learning rate $2{\times}10^{-5}$; full hyperparameters in [Appendix˜F](#A6).
*(c) Evaluation.* We instantiate Executive model with initialized Qwen2.5-32B-Instruct or Gemini-3.0-Flash  to evaluate the same trained Memory model across models of varying reasoning capability; both models have minimal prior knowledge of the evaluation datasets ([Appendix˜I](#A9)). Executive model queries Memory model through the multi-turn protocol described in [Section˜4.4](#S4.SS4). We report binary accuracy judged by Gemini-2.5-Flash-Lite  via DeepEval , as mean $\pm$ standard deviation over three runs for Qwen2.5-32B-Instruct and a single run for Gemini-3.0-Flash.
*(d) Continual integration.* For the model-merging experiment ([Section˜5.5](#S5.SS5)), we partition NarrativeQA into two pairwise-disjoint subsets ($K{=}2$, $\sim$640k QA pairs each), SFT a separate Qwen2.5-14B-Instruct Memory model on each, and sweep six merging methods at three densities (14 configurations total).

### 5.1 Experimental results

MeMo achieves strong performance across benchmarks.
As shown in Table [2](#S5.T2), MeMo consistently outperforms all baselines on NarrativeQA and MuSiQue across both Executive models. On NarrativeQA, the most challenging benchmark ([Appendix˜I](#A9)), MeMo achieves $26.85\%$ with Qwen2.5-32B-Instruct and $53.58\%$ with Gemini-3-Flash, substantially surpassing all baselines. This is notable: NarrativeQA requires reasoning over long passages with complex connections, where retrieval-based methods are constrained by context windows and struggle to synthesize information across long documents; MeMo instead captures these connections via reflections during training and retrieves them through its multi-turn protocol at inference. The same trend holds on MuSiQue, where MeMo achieves $48.30\%$ and $58.70\%$ respectively, outperforming baselines that struggle with multi-hop reasoning across independently retrieved passages. On BrowseComp-Plus, MeMo leads with Gemini-3-Flash ($66.67\%$) and remains competitive with Qwen2.5-32B-Instruct ($54.22\%$, narrowly trailing HippoRAG2’s $56.11\%$). This gap reflects BrowseComp-Plus’s nature: its answers are absent from Executive model’s parametric knowledge ([Appendix˜I](#A9)), making direct access to evidence documents especially valuable and favoring retrieval methods that pass raw documents to Executive model.

**Table 2: Accuracy (%) on BrowseComp-Plus, NarrativeQA, and MuSiQue under two Executive models: Qwen2.5-32B-Instruct (Qwen2.5-32B-I) and Gemini-3-Flash (Gemini-3-F). Bold values indicate the best result in each column, excluding Perfect Retrieval. MeMo uses Qwen2.5-14B-Instruct as Memory model, and results are reported at the best training epoch. ^⋆Perfect Retrieval represents an empirical upper bound.**
|  | BrowseComp-Plus | NarrativeQA | MuSiQue |  |  |  |
| --- | --- | --- | --- | --- | --- | --- |
| Method | Qwen2.5-32B-I | Gemini-3-F | Qwen2.5-32B-I | Gemini-3-F | Qwen2.5-32B-I | Gemini-3-F |
| Perfect Retrieval^⋆ | $79.67\pm 1.45$ | $88.33$ | $51.42\pm 0.52$ | $60.41$ | $62.83\pm 0.90$ | $73.00$ |
| BM25 | $1.11\pm 0.69$ | $27.00$ | $10.24\pm 0.34$ | $14.33$ | $20.00\pm 0.30$ | $23.20$ |
| NV-Embed-V2 | $50.67\pm 0.33$ | $57.00$ | $20.59\pm 0.86$ | $26.62$ | $37.47\pm 0.15$ | $46.60$ |
| HippoRAG2(^4^44These results differ from the original paper , which uses Llama3.3-70B-Instruct instead of Qwen2.5-32B-Instruct.) | $\mathbf{56.11\pm 0.51}$ | $66.33$ | $21.39\pm 0.20$ | $23.21$ | $42.17\pm 0.12$ | $57.00$ |
| Cartridges(^5^55Cartridges requires white-box access to Executive model as well; its results for Gemini-3-Flash are therefore omitted.) | $0.00\pm 0.00$ | - | $3.75\pm 0.11$ | - | $8.57\pm 0.40$ | - |
| MeMo | $54.22\pm 0.84$ | $\mathbf{66.67}$ | $\mathbf{26.85\pm 0.39}$ | $\mathbf{53.58}$ | $\mathbf{48.30\pm 1.25}$ | $\mathbf{60.20}$ |

MeMo supports plug-and-play integration.
Across the three benchmarks, MeMo consistently achieves higher performance when paired with a more capable Executive model (Gemini-3-Flash): switching from Qwen2.5-32B-Instruct to Gemini-3-Flash yield gains of 12.45, 26.73, 11.90 pp on BrowseComp-Plus, NarrativeQA and MuSiQue respectively. This demonstrates that MeMo can be trained once with a weaker Generator model, and seamlessly paired with *any* LLM at inference — including proprietary models such as Gemini-3-Flash. This *plug-and-play* capability allows MeMo to directly leverage state-of-the-art models without any additional training or overhead.

### 5.2 Ablation on the amount of noise for the dataset

**Table 3: Accuracy (%) on BrowseComp-Plus and MuSiQue with Qwen2.5-32B-Instruct as Executive model. MeMo results are based on Qwen2.5-14B-Instruct and reported at the best training epoch. $N=N_{\text{evidence}}^{\text{dataset}}$ denotes the number of ground-truth evidence documents in the corpus; column headers indicate the number of additional negative (distractor) documents added, as a multiple of $N$. $\Delta$ denotes accuracy difference (pp) compared to $0N$.**
| Method | Dataset | $0\times N$ | $1\times N$ |  |
| --- | --- | --- | --- | --- |
|  |  | Acc. (%) | Acc. (%) | $\Delta$ |
| NV-Embed-V2 | BrowseComp-Plus | $56.89\pm 0.51$ | $50.67\pm 0.33$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 6.22}$ |
| MuSiQue | $42.30\pm 0.53$ | $37.47\pm 0.15$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 4.83}$ |  |
| HippoRAG2 | BrowseComp-Plus | $62.33\pm 1.15$ | $56.11\pm 0.51$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 6.22}$ |
| MuSiQue | $47.33\pm 0.74$ | $42.17\pm 0.12$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 5.16}$ |  |
| MeMo | BrowseComp-Plus | $53.67\pm 1.15$ | $54.22\pm 0.84$ | ${\color[rgb]{0,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{0,0,0}\uparrow 0.55}$ |
| MuSiQue | $50.07\pm 0.81$ | $48.30\pm 1.25$ | ${\color[rgb]{1,.5,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,.5,0}\downarrow 1.77}$ |  |

We investigate the robustness of MeMo against two strong retrieval-based baselines, NV-Embed-V2 and HippoRAG2, under increasing levels of retrieval noise, controlled by varying the number of negative (distractor) documents added to the target corpus as a multiple of the total number of ground-truth evidence documents in each dataset ($N_{\text{evidence}}^{\text{dataset}}=1{,}775$ for BrowseComp-Plus and $N_{\text{evidence}}^{\text{dataset}}=2{,}648$ for MuSiQue). The datasets used throughout this paper (detailed in [Appendix˜D](#A4)) correspond to a ratio of 1$\times N_{\text{evidence}}^{\text{dataset}}$; we additionally evaluate at ratio 0$\times N_{\text{evidence}}^{\text{dataset}}$ (no distractors) as an idealized noise-free reference to isolate the effect of distractors.

Results in [Table˜3](#S5.T3) demonstrate that retrieval-based methods exhibit pronounced sensitivity to noise. Both NV-Embed-V2 and HippoRAG2 suffer drops of up to 6.22 pp on BrowseComp-Plus and up to 5.16 pp on MuSiQue when scaling from $0\times N$ to $1\times N$, confirming that these systems struggle to filter irrelevant documents under realistic corpus conditions. In contrast, MeMo maintains stable performance across both benchmarks, with a marginal improvement of 0.55 pp on BrowseComp-Plus and a decline of only 1.77 pp on MuSiQue, both within one standard deviation, demonstrating that MeMo is robust to increasing retrieval noise. We attribute this robustness to MeMo’s design: despite being trained on a corpus containing negative documents, Memory model provides more precise information to Executive model’s sub-queries than direct document retrieval. Additional analysis of performance degradation in retrieval-based methods is provided in [Appendix˜L](#A12).

### 5.3 Ablation on Memory model size

We investigate how the size of Memory model affects downstream task performance by comparing models of 1.5B and 14B parameters in the Qwen2.5 family. Implementation details are provided in [Appendix˜M](#A13).
Results in [Table˜4](#S5.T4) show a consistent positive scaling trend: larger Memory models yield improved performance across all benchmarks and Executive models. However, the results also show that a stronger Executive model reasoning capability modulates this gap non-uniformly across tasks: the performance difference between Memory model sizes widens for NarrativeQA but shrinks for BrowseComp-Plus and MuSiQue. This suggests that the interaction between Executive model reasoning capability and Memory model size is task-dependent.

**Table 4: Ablation on Memory model size within the Qwen2.5 family. Bold results indicate best performing results in the column.**
|  | BrowseComp-Plus | NarrativeQA | MuSiQue |  |  |  |
| --- | --- | --- | --- | --- | --- | --- |
| Memory Model | Qwen2.5-32B | Gemini-3-Flash | Qwen2.5-32B | Gemini-3-Flash | Qwen2.5-32B | Gemini-3-Flash |
| Qwen2.5-1.5B-Instruct | $44.11\pm 2.22$ | $61.00$ | $24.00\pm 0.20$ | $47.44$ | $42.90\pm 1.39$ | $59.70$ |
| Qwen2.5-14B-Instruct | $\mathbf{54.22\pm 0.84}$ | $\mathbf{66.67}$ | $\mathbf{26.85\pm.39}$ | $\mathbf{53.58}$ | $\mathbf{50.07\pm 0.81}$ | $\mathbf{60.20}$ |

### 5.4 Ablation on Memory model family

We investigate whether the choice of Memory model family affects performance by comparing three models of similar parameter scale ($\sim$1–2B) but distinct architectures and pretraining lineages: Qwen2.5-1.5B-Instruct , Gemma3-1B-IT , and LFM2.5-1.2B-Instruct . Implementation details are provided in [Appendix˜N](#A14).
Results in [Table˜5](#S5.T5) show that MeMo performance is largely robust to the choice of Memory model architecture, demonstrating that the framework is not sensitive to the specific pretraining lineage of Memory model at similar parameter scale,
and that the parametric knowledge compression induced by our training procedure generalizes across diverse model families.

**Table 5: Ablation across Memory models at similar parameter scales ($\sim$1–2B). Bold results indicate best performing results in the column.**
|  | BrowseComp-Plus | NarrativeQA | MuSiQue |  |  |  |
| --- | --- | --- | --- | --- | --- | --- |
| Memory Model | Qwen2.5-32B-I | Gemini-3-F | Qwen2.5-32B-I | Gemini-3-F | Qwen2.5-32B-I | Gemini-3-F |
| Qwen2.5-1.5B-Instruct | $\mathbf{44.11\pm 2.22}$ | 61.00 | $\mathbf{24.00\pm 0.20}$ | $47.44$ | $42.90\pm 1.39$ | $\mathbf{59.70}$ |
| Gemma3-1B-IT | $41.67\pm 2.03$ | $59.00$ | $22.30\pm 2.47$ | $\mathbf{48.81}$ | $41.17\pm 1.20$ | $56.20$ |
| LFM2.5-1.2B-Instruct | $37.33\pm 1.86$ | $59.67$ | $21.96\pm 1.97$ | $46.42$ | $\mathbf{45.23\pm 2.49}$ | $58.30$ |

### 5.5 Continual integration via model merging

We test the streaming-update scenario described in [Section˜4.2](#S4.SS2) on NarrativeQA, comparing model merging against full retraining of Memory model on the union of both subsets when the second arrives. Of the 14 sweep configurations (see [Table˜12](#A8.T12), [Appendix˜H](#A8)), we report TIES  at $\rho{=}0.3$ in the main paper, the top-performing one. Letting $X$ and $Y$ denote the SFT cost on each subset alone (cost scales approximately linearly with the number of QA pairs, so the union costs $X{+}Y$), cumulative compute across the two arrivals is $X{+}Y$ for merging versus $X{+}(X{+}Y)$ for full retraining.

**Table 6: Model merging vs. full retraining on NarrativeQA. Memory model = Qwen2.5-14B-Instruct. Merge-TIES ($\rho{=}0.3$) is the best of 14 configurations swept ([Table˜12](#A8.T12), [Appendix˜H](#A8)). Cumulative compute is reported in 8$\times$H100 GPU-hours for $K{=}2$ subsets of $\sim$640k reflection QA pairs each. $\Delta$ denotes accuracy difference (pp) relative to full retraining.**
| Method | Cumulative compute | Qwen2.5-32B-I | Gemini-3-F |  |  |
| --- | --- | --- | --- | --- | --- |
|  | (8$\times$H100 GPU-h) | Acc. (%) | $\Delta$ | Acc. (%) | $\Delta$ |
| Full retrain ($X{+}(X{+}Y)$) | $\approx 72$h | $\mathbf{26.85\pm 0.39}$ | — | $\mathbf{53.58}$ | — |
| Merge-TIES ($\rho{=}0.3$, $X{+}Y$) | $\approx 48$h | $15.81\pm 0.39$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 11.04}$ | $34.47$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 19.11}$ |

Merging cuts compute by $\mathbf{33\%}$ at $K{=}2$, with widening returns at scale.
As reported in [Table˜6](#S5.T6), the full-retrain baseline incurs $X{+}(X{+}Y)\approx 72$ GPU-hours of cumulative compute, while merging accumulates only $X{+}Y\approx 48$ GPU-hours — a $33\%$ reduction ([Fig.˜2](#A8.F2)). The gap widens with $K$: under the same per-corpus cost, merging scales as $\Theta(K)$ while full retraining scales as $\Theta(K^{2})$, yielding a $5.5{\times}$ saving at $K{=}10$ ($240$ vs. $1{,}320$ GPU-hours).

Merging trades a measurable accuracy gap for the compute saving, but still beats retrieval.
Merge-TIES ($\rho{=}0.3$) trails the full-retrain Memory model by $11.0$ pp under Qwen2.5-32B-Instruct and $19.1$ pp under Gemini-3-Flash ([Table˜6](#S5.T6)); across the full 14-configuration sweep, accuracy ranges from $7.85\%$ (SLERP, worst) to $15.81\%$ (TIES, best), shown in [Fig.˜2](#A8.F2). Despite this gap, the merged Memory model still outperforms every retrieval baseline (BM25, NV-Embed-V2, HippoRAG2, Cartridges; see [Table˜2](#S5.T2)) on NarrativeQA, indicating that even an aggressively-cheaper merging procedure preserves most of MeMo’s qualitative advantage over retrieval-based approaches. TIES and DARE-Linear at $\rho{=}0.3$ dominate the sweep, suggesting that aggressive sparsification combined with sign-conflict resolution is the most reliable merging recipe in this regime.

## 6 Conclusion

We introduced MeMo, a modular framework for integrating updated or domain-specific knowledge into LLMs via a Memory model trained on a synthesized reflection QA dataset. MeMo addresses key limitations of existing methods: it bypasses context constraints and weak cross-document reasoning in retrieval-based approaches, avoids costly and brittle parametric updates (including catastrophic forgetting), and removes representation coupling in latent memory methods. Its core components are a data synthesis pipeline capturing explicit facts and implicit relationships, and a multi-turn inference protocol that decomposes complex queries into targeted sub-queries for desired information retrieval from the memory model.
While MeMo demonstrates strong performance, it has limitations regarding training cost, evaluation scope, and the capacity of Memory model to scale with corpus size (see [Appendix˜B](#A2)).
Empirically, MeMo outperforms strong baselines across diverse benchmarks. It also provides a scalable pathway for knowledge integration, supporting efficient updates and plug-and-play deployment with both open-source and proprietary LLMs.
Future work includes more efficient memory construction, extensions to dynamic corpora, and tighter coordination between the Executive model and Memory model. We view MeMo (Memory as a Model) as a promising foundation for more flexible, updatable, and knowledge-aware AI systems.

## Appendix A Impact statement

MeMo advances the ability of LLMs to internalize knowledge over large, domain-specific corpora without requiring access to model weights, lowering the barrier for deploying capable AI systems in knowledge-intensive domains such as law, medicine, and scientific research. By enabling plug-and-play integration with any LLM, including proprietary models, MeMo democratizes access to powerful knowledge integration capabilities that would otherwise require significant computational resources or white-box model access. At the same time, this accessibility introduces dual-use concerns, as the same capability that enables beneficial applications could be used to internalize misinformation, proprietary data without authorization, or harmful content at scale. Additionally, as MeMo reduces reliance on explicit retrieval, it may obscure the provenance of retrieved information, making it harder to attribute the sources underlying a model’s responses. We encourage future work to investigate attribution mechanisms and access controls for memory-based systems, and urge practitioners to carefully consider the nature of the documents used to train Memory model.

## Appendix B Limitations

MeMo incurs an upfront training cost for each new corpus, and performance may vary across domains, document types, or LLM families beyond those covered in our experiments. Furthermore, the performance of MeMo is inherently bounded by the representational capacity of Memory model to internalize the target corpus. Although our experiments do not reveal clear signs that Memory model has reached its capacity limit, we hypothesize that sufficiently large or information-dense corpora will exceed what a fixed-size Memory model can correctly compress and represent.

## Appendix C Future work

We outline several directions for future work. The data generation pipeline is computationally expensive, with Step 5 in [Algorithm˜1](#alg1) scaling quadratically at $O(k\cdot C^{2}\cdot Q^{2})$, and reducing this cost remains an open problem. A systematic evaluation of chunking strategies and their associated tradeoffs ([Appendix˜D](#A4)) is likewise an open direction. On the training side, scaling Memory model with corpus size and developing more effective model merging strategies for reducing per-corpus training costs ([Section˜5.5](#S5.SS5)) are promising directions. Other post-training methods such as Reinforcement Learning  have also shown to be effective in improving model task performance , and applying such methods to Memory model training warrants future investigation.

LoRA configurations better suited to specific architectures, including per-architecture tuning of rank and learning rate, also warrant further investigation ([Appendix˜O](#A15)). Finally, a more systematic study of the interaction between Executive model reasoning capability and Memory model model size ([Section˜5.3](#S5.SS3)), as well as the optimal interaction budget at each stage and Executive model selection ([Section˜J.2](#A10.SS2)), are other promising future directions.

## Appendix D Preparation of datasets

Corpus construction.
Extending from our description in [Section˜5](#S5), we distinguish between two types of documents(^6^66Note that for BrowseComp-Plus, the gold documents are a subset of the evidence documents.): evidence documents, which contain information relevant to answering a given question, and negative documents, which are irrelevant and serve as noise. For BrowseComp-Plus, we used $1{,}775$ unique evidence documents and $1{,}766$ unique negative documents (after removal of non-English documents), yielding $3{,}541$ documents in total. For MuSiQue, we used $2{,}648$ documents for each of the evidence and negative documents, yielding $5{,}296$ documents in total. NarrativeQA does not have negative documents.

Chunking strategy.
As shown in [Table˜7](#A4.T7), NarrativeQA full documents span the $32{,}769$–$131{,}072$ token range with a median length of $65{,}925$ tokens, reflecting the long-form nature of the source novels. Processing such documents without chunking risks reduced coverage of extractable QA pairs in Step 1 of [Algorithm˜1](#alg1), as attention quality is known to deteriorate over longer contexts . We therefore chunk NarrativeQA documents using a fixed sliding window of $6{,}400$ words with a $640$-word overlap ($10\%$ overlap ratio), yielding $75$ chunks concentrated in the $4{,}097$–$16{,}384$ token range and accounting for $96\%$ of all chunks, with a median group size of $7$ per document as shown in [Table˜8](#A4.T8). Unlike NarrativeQA, MuSiQue documents are compact with $99.70\%$ falling below $512$ tokens, and each MuSiQue document is treated as a single chunk.

**Table 7: Token length distribution across corpora at the chunk level, where $n$ represents the total number of individual chunks processed by [Algorithm˜1](#alg1). Each entry reflects the token count of a single text chunk. Statistics for NarrativeQA are reported before and after chunking.**
| Token Range | BrowseComp-Plus<br>($n=3{,}541$) | NarrativeQA<br>Full Docs<br>($n=10$) | NarrativeQA<br>Chunks<br>($n=75$) | MuSiQue<br>($n=5{,}296$) |
| --- | --- | --- | --- | --- |
| 0–$512$ | $606$ ($17.11\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) | $5{,}280$ ($99.70\%$) |
| $513$–$1{,}024$ | $591$ ($16.69\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) | $16$ ($0.30\%$) |
| $1{,}025$–$2{,}048$ | $746$ ($21.07\%$) | 0 ($0.00\%$) | $1$ ($1.33\%$) | 0 ($0.00\%$) |
| $2{,}049$–$4{,}096$ | $598$ ($16.89\%$) | 0 ($0.00\%$) | $2$ ($2.67\%$) | 0 ($0.00\%$) |
| $4{,}097$–$8{,}192$ | $428$ ($12.09\%$) | 0 ($0.00\%$) | $36$ ($48.00\%$) | 0 ($0.00\%$) |
| $8{,}193$–$16{,}384$ | $323$ ($9.12\%$) | 0 ($0.00\%$) | $36$ ($48.00\%$) | 0 ($0.00\%$) |
| $16{,}385$–$32{,}768$ | $145$ ($4.09\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) |
| $32{,}769$–$65{,}536$ | $56$ ($1.58\%$) | $5$ ($50.00\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) |
| $65{,}537$–$131{,}072$ | $20$ ($0.56\%$) | $5$ ($50.00\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) |
| $>131{,}072$ | $28$ ($0.79\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) |
| Min tokens | $14$ | $32{,}804$ | $1{,}943$ | $23$ |
| Median tokens | $1{,}756$ | $65{,}925$ | $8{,}158$ | $105$ |
| Mean tokens | $7{,}192$ | $66{,}324$ | $8{,}713$ | $123$ |
| $p_{95}$ tokens | $20{,}330$ | $119{,}267$ | $11{,}266$ | $270$ |
| Max tokens | $1{,}235{,}897$ | $119{,}267$ | $12{,}104$ | $828$ |

**Table 8: Distribution of document group sizes across datasets, where group size denotes the number of chunks associated with a single question or document. For BrowseComp-Plus and MuSiQue, each question is associated with a subset of chunks drawn from the corpus, and group size represents the number of chunks per *question*. For NarrativeQA, each subset of chunks is derived from the original document used for multiple questions, and group size represents the number of chunks per *document*.**
| Document Group<br>Size Range | BrowseComp-Plus<br>($n_{\text{group}}=300$) | NarrativeQA Chunks<br>($n_{\text{group}}=10$) | MuSiQue<br>($n_{\text{group}}=1{,}000$) |
| --- | --- | --- | --- |
| 0–2 | $2$ ($0.67\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) |
| 3–4 | $14$ ($4.67\%$) | $3$ ($30.00\%$) | $518$ ($51.80\%$) |
| 5–8 | $78$ ($26.00\%$) | $4$ ($40.00\%$) | $482$ ($48.20\%$) |
| 9–16 | $159$ ($53.00\%$) | $3$ ($30.00\%$) | 0 ($0.00\%$) |
| $>$16 | $47$ ($15.67\%$) | 0 ($0.00\%$) | 0 ($0.00\%$) |
| Min group size | $2$ | $3$ | $4$ |
| Median group size | $12$ | $7$ | $4$ |
| Mean group size | $11.8$ | $7.5$ | $5.3$ |
| $p_{95}$ group size | $20$ | $16$ | $8$ |
| Max group size | $23$ | $16$ | $8$ |

BrowseComp-Plus documents are also treated as a single chunk. The time complexity of Step 5 in [Algorithm˜1](#alg1) is $O(k\cdot C^{2}\cdot Q^{2})$, where $k=n_{\text{group}}$ is the number of groups, $C=|G_{i}|$ is the number of participating chunks per group, and $Q=\bar{Q}_{i}$ is the average number of QA pairs extracted per chunk. Since chunking increases $C$, pipeline costs at Step 5 scale quadratically as the number of chunks per group increases. Given that only $2.93\%$ of BrowseComp-Plus documents exceed $32{,}768$ tokens, the majority of documents fit within a single chunk, making the cost of chunking difficult to justify. We therefore opted against chunking in favor of lower pipeline cost, and leave a systematic evaluation of chunking strategies and related tradeoffs to future work.

Subset selection of negative documents.
We include only a subset of negative documents for BrowseComp-Plus and MuSiQue due to computational constraints arising from the quadratic scaling of Step 5. As reported in [Table˜8](#A4.T8), BrowseComp-Plus currently has a mean group size of $11.8$ and a maximum of $23$, while MuSiQue has a mean group size of $5.3$ and a maximum of $8$. Incorporating all available negative documents, which average $78$ per question (up to $197$) for BrowseComp-Plus and $17$ per question (up to $18$) for MuSiQue, would cause the group size to increase substantially. Given the quadratic dependence on $C$ in Step 5, this would result in a prohibitive increase in pipeline cost for BrowseComp-Plus ($k=300$) and MuSiQue ($k=1{,}000$). Hence, we opted to only include up to $N_{\text{evidence}}^{\text{dataset}}$ number of negative documents for each question in the corpus.

## Appendix E Discussion on steps in data generation pipeline

### E.1 Ablation of data synthesis steps

We experiment with the data generation pipeline to show the importance of each step. We perform a leave-one-out (LOO) ablation for each step of data synthesis and train the model on the synthesized QA pairs generated. Results are reported in [Table˜9](#A5.T9) on the NarrativeQA and MuSiQue datasets using Qwen2.5-32B-Instruct as the Executive model and Qwen2.5-1.5B-Instruct as the Memory model.

**Table 9: LOO ablation accuracy at best performing Qwen2.5-1.5B-Instruct epoch across datasets. Data ratio indicates the number of QA pairs retained relative to the baseline. For each step removed, the Qwen2.5-1.5B-Instruct was retrained, and we report the mean $\pm$ std. dev. over 3 runs at the same training epoch as the baseline.**
|  | NarrativeQA | MuSiQue |  |  |
| --- | --- | --- | --- | --- |
| Ablation | Data Ratio | Accuracy (%) | Data Ratio | Accuracy (%) |
| Baseline (all steps) | $1.000\times$ | $24.00\pm 0.20$ | $1.000\times$ | $42.90\pm 1.25$ |
| Step 1a removed | $0.434\times$ | $20.48\pm 0.90$ | $0.381\times$ | $30.00\pm 0.17$ |
| Step 1b removed | $0.598\times$ | $22.98\pm 1.04$ | $0.651\times$ | $37.33\pm 0.25$ |
| Step 2 removed | $0.739\times$ | $24.69\pm 1.10$ | $0.621\times$ | $37.10\pm 1.76$ |
| Step 3 removed | $2.078\times$ | $28.90\pm 0.86$ | $1.128\times$ | $41.70\pm 0.78$ |
| Step 4 removed | $0.378\times$ | $23.21\pm 1.56$ | $0.501\times$ | $39.10\pm 0.02$ |
| Step 5 removed | $0.002\times$ | $6.37\pm 0.39$ | $0.195\times$ | $24.17\pm 0.25$ |

Step 5 (Cross-document synthesis) is the most critical component of the pipeline. Its removal causes accuracy to collapse to $6.37\%$ and $24.17\%$ on NarrativeQA and MuSiQue respectively, against baseline scores of $24.00\%$ and $42.90\%$, accompanied by a near-total loss of training data ($0.002\times$ and $0.195\times$ retention). As described in [Section˜4.1](#S4.SS1), Step 5 enables cross-document synthesis where $\mathcal{M}_{\text{gen}}$ constructs $\mathcal{Q}_{\text{cross}}$ pairs spanning inter-document connections and cross-chunk connections within a single long document, making it the dominant source of training pairs in $\mathcal{Q}_{\text{final}}$ and directly targeting the multi-source synthesis objective central to both benchmarks.

An interesting anomaly arises with Step 2 and Step 3, where their removal does not consistently hurt performance and improves accuracy on NarrativeQA. Step 2 merges related QA pairs from a single document chunk into multi-fact questions by identifying commonalities such as shared entities, overlapping time periods, and sequential events. For MuSiQue, these commonalities reflect genuine knowledge relationships that directly resemble the multi-hop factual reasoning the benchmark evaluates, such that removing Step 2 eliminates a large fraction of useful training pairs, leading to a drop in accuracy from $42.90\%$ to $37.10\%$. For NarrativeQA, however, the same consolidation patterns operate on superficial narrative co-occurrences rather than meaningful knowledge relationships. The predominant commonality categories are event or scene groupings that NarrativeQA does not evaluate, and entity co-occurrence patterns that are trivially satisfied given the pervasive presence of central characters across scenes. Removing Step 2 eliminates these low-quality pairs, leading to the marginal accuracy improvement from $24.00\%$ to $24.69\%$.

Removing Step 3 retains more data than the baseline ($2.078\times$ and $1.100\times$ for NarrativeQA and MuSiQue respectively), yet the effect on performance diverges. For MuSiQue, performance drops from $42.90\%$ to $41.78\%$, whereas for NarrativeQA, performance improves from $24.00\%$ to $28.90\%$. Step 3 applies a self-containment filter that rewrites or discards pairs whose questions cannot be understood without access to the source chunk.
For MuSiQue, violations are predominantly localized and shallow, making them amenable to filtering; the proposed filter effectively identifies and removes defective pairs.
For NarrativeQA, long-form narrative text frequently contains pronouns and temporal references that span many paragraphs, which are structural features of the domain rather than fixable defects. This causes the rewriting loop to introduce substitute unrelated content and corrupt the pairs produced by earlier steps. Removing Step 3 for NarrativeQA therefore avoids this domain-induced corruption and retains the original pairs intact, explaining both the data retention ratio increase and the accuracy improvement. This suggests that Step 3 is most beneficial when applied to domains where self-containment violations are well-defined and resolvable.

The remaining steps follow a consistent trend: removing Step 1a, Step 1b, or Step 4 reduces both data volume and accuracy across both datasets, confirming that each step contributes a distinct and meaningful role to the final training corpus quality.

### E.2 Additional steps considered but excluded

Three additional steps were considered but ultimately excluded from the pipeline. These include paraphrasing , increasing the number of sampling trials at Step 1 of [Algorithm˜1](#alg1), and a targeted fill whereby $\mathcal{M}_{\text{gen}}$ reviews the generated QA pairs and rewrites them to incorporate additional missed information. Paraphrasing was excluded as the scale of generated pairs already provides sufficient coverage ($\approx$600k–1.6M across the three datasets, see Table [11](#A6.T11)), and the potential gains were outweighed by the additional computational overhead. Increasing sampling trials proved unreliable, as additional trials did not consistently extract facts that the initial pass had failed to extract. The targeted fill similarly offered limited gains, where appending the existing QA pairs as context to prompt a revision only lengthens the context when the model had already failed to extract a fact from the original chunk, likely exacerbating attention further degradation over long inputs  and making retrieval of relevant information less reliable at inference time.

## Appendix F Memory model hyperparameter settings

Training was conducted on H100 and H200 GPUs using the hyperparameter settings reported in [Table˜10](#A6.T10). The effective batch size for each dataset is summarized in [Table˜11](#A6.T11).

**Table 10: Memory model SFT Training Configuration**
| Parameter | Value |
| --- | --- |
| Optimizer | Fused AdamW |
| Gradient checkpointing | True |
| Learning rate (LR) | $2\times 10^{-5}$ |
| Num of Training epochs | 3 |
| LR scheduler type | Constant with warmup |
| Warmup ratio | 0.05 |
| Weight decay | 0.01 |
| Max gradient norm | 1.0 |
| Max sequence length | 8096 |
| Precision | BF16 |
| Attention implementation | Flash Attention 2 |

**Table 11: Effective batch sizes and number of QA pairs used. NarrativeQA.1 and NarrativeQA.2 are independent subsets partitioned from the original that were used for model merging.**
| Dataset | Target Num of Questions | Num of QA Pairs | Effective Batch Size |
| --- | --- | --- | --- |
| BrowseComp-Plus | $300$ | $1{,}639{,}995$ | $512$ |
| NarrativeQA | $293$ | $1{,}276{,}676$ | $512$ |
| NarrativeQA.1 | $146$ | $635{,}009$ | $256$ |
| NarrativeQA.2 | $147$ | $641{,}667$ | $256$ |
| MuSiQue | $1{,}000$ | $664{,}762$ | $256$ |

## Appendix G Compute resources

All experiments were conducted on using NVIDIA H200 GPUs. We report computational cost in GPU-hours.

#### Data generation.

Generating the full reflection dataset for BrowseComp-Plus, NarrativeQA, and MuSiQue took approximately 240, 200, and 150 GPU-hours respectively.

#### Training.

Memory model (Qwen2.5-14B-Instruct) training for a single run BrowseComp-Plus, NarrativeQA, and MuSiQue took approximately 180, 150, 90 GPU-hours.

## Appendix H Model training discussion

We considered three training paradigms: CPT, SFT, and LoRA-based SFT. CPT was excluded as it risks degrading instruction-following capability , which is critical for downstream QA evaluation. Full SFT was selected as it directly optimizes for the target task while preserving alignment . LoRA-based SFT serves as a parameter-efficient alternative and we include a comparison to these training methods in [Appendix˜O](#A15).

Model merging targets the practical streaming setting in which new corpora arrive over time and Memory model must continually integrate them. Retraining Memory model from scratch on the union of all observed corpora is the natural baseline but quickly becomes prohibitive at scale, since its cost grows with the cumulative corpus size. Model merging instead trains a separate Memory model on each new corpus and combines it with the existing model in parameter space, so the cost of each update scales only with the size of the new corpus rather than the entire history. This decoupling comes at a measurable accuracy cost relative to full retraining, which we quantify in [Fig.˜2](#A8.F2). We assume the corpora to be merged are pairwise disjoint.

### H.1 Model merging

Merging methods.
We consider the following methods, all of which produce $\varphi_{\text{merged}}$ without ever training on $\mathcal{D}_{1}\cup\dots\cup\mathcal{D}_{K}$:

- •
Linear merging  computes a weighted sum of task vectors: $\varphi_{\text{merged}}=\varphi_{0}+\sum_{i=1}^{K}\lambda_{i}\tau_{i}$, where $\lambda_{i}>0$ are merging coefficients.
- •
SLERP  interpolates between two task vectors along the unit sphere, preserving their magnitudes: $\varphi_{\text{merged}}=\varphi_{0}+\mathrm{SLERP}(\tau_{1},\tau_{2};\,t)$, with $t\in[0,1]$ controlling the interpolation factor.
- •
Task arithmetic  adds task vectors directly without further processing, recovering linear merging as a special case with uniform $\lambda_{i}$.
- •
TIES  resolves interference among task vectors before summation by (i) *trimming* each $\tau_{i}$ to its top-$\rho$ fraction of largest-magnitude entries, (ii) *electing* a sign at each coordinate by magnitude-weighted majority vote, and (iii) *disjoint-merging* only the entries that agree with the elected sign.
- •
DARE  sparsifies each task vector by randomly dropping a fraction $1-\rho$ of its entries and rescaling the survivors by $1/\rho$ to preserve expected magnitude, before linear merging.
- •
DARE-TIES  combines DARE-style stochastic sparsification with TIES sign-conflict resolution, retaining the diversity of random dropout while filtering out conflicting updates.

Avoiding catastrophic forgetting.
Because no individual Memory model $\mathcal{M}_{\varphi_{i}}$ is ever fine-tuned on another corpus’ data, model merging cannot induce the kind of distributional interference that drives catastrophic forgetting in sequential fine-tuning . Knowledge from each corpus is preserved within its own task vector $\tau_{i}$, and conflicts between task vectors are addressed at merge time via the methods above rather than during gradient updates.

Scalability.
When a new corpus $\mathcal{D}_{K+1}$ arrives, we train auxiliary model $\mathcal{M}_{\varphi_{K+1}}$ on its reflection QA dataset, derive $\tau_{K+1}$, and re-merge in $\mathcal{O}(1)$ additional cost relative to the full collection. This enables modular, plug-and-play integration over a continuous stream of disjoint knowledge sources, unlike retraining from scratch on $\bigcup_{i}\mathcal{D}_{i}$, which scales linearly with the cumulative corpus size.

Inference.
The merged Memory model is queried identically to a single-corpus Memory model via the structured multi-turn protocol described in [Section˜4.4](#S4.SS4). Because merging operates entirely in parameter space and produces a model with the same architecture and interface as $\mathcal{M}_{\varphi_{0}}$, it inherits the plug-and-play property of MeMo without requiring changes to the Executive model or the inference protocol. Importantly, the Executive model queries a *single* merged Memory model at inference rather than dispatching across $K$ separate per-corpus Memory models, keeping the multi-turn retrieval pipeline unchanged regardless of how many corpora have been integrated.

Procedure.
For our experiments we partition NarrativeQA into two pairwise-disjoint subsets, NarrativeQA.1 and NarrativeQA.2, of $\sim$640k reflection QA pairs each. Each subset is used to fine-tune an independent Memory model from the same Qwen2.5-14B-Instruct base via SFT for $3$ epochs, producing $\mathcal{M}_{\varphi_{1}}$ and $\mathcal{M}_{\varphi_{2}}$ at SFT costs of $X$ and $Y$ GPU-hours, respectively (each is $\approx 24$ GPU-hours on 8$\times$H100; full-retrain on the union NarrativeQA.1 $\cup$ NarrativeQA.2 costs $X{+}Y\approx 48$ GPU-hours by linear scaling). We evaluate every saved checkpoint of each run on the held-out NarrativeQA evaluation set and select the best-performing checkpoint per subset; the corresponding task vectors $\tau_{1}$ and $\tau_{2}$ are the inputs to the merging step. We then sweep all six merging methods listed above (Linear, Task arithmetic, SLERP, TIES, DARE, DARE-TIES) at three sparsification densities $\rho\in\{0.3,0.5,0.7\}$ (or three interpolation factors $t\in\{0.3,0.5,0.7\}$ for SLERP), giving $14$ merged-Memory model configurations in total. Each configuration is evaluated on NarrativeQA with Qwen2.5-32B-Instruct as Executive model (mean $\pm$ std over 3 runs). The configuration that we report in the [Section˜5](#S5) as *Merge-TIES* is the best of the sweep (TIES with $\rho{=}0.3$).

Figure: Figure 2: Cost–accuracy trade-off on NarrativeQA when a second corpus arrives ($K{=}2$, Memory model = Qwen2.5-14B-Instruct, 8$\times$H100). Cumulative training cost is shown on the $x$-axis (one Qwen-14B SFT run takes $\approx 24$ GPU-hours on a 640k-QA-pair corpus). Merging trains Memory model only on the new corpus, costing $X{+}Y\approx 48$ GPU-hours, while full retraining re-runs on the union, costing $X{+}(X{+}Y)\approx 72$ GPU-hours — a 33% saving. Merge-TIES ($\rho{=}0.3$) trails full retraining by $11.0$ pp with Qwen2.5-32B-Instruct and $19.1$ pp with Gemini-3-Flash as Executive model, but still outperforms all retrieval baselines (BM25, NV-Embed-V2, HippoRAG2, Cartridges). The vertical $\updownarrow$ at the merge cost shows the worst-to-best range across the 14 merge configurations swept ([Table˜12](#A8.T12)). Perfect Retrieval is shown as the upper bound.
Refer to caption: https://arxiv.org/html/2605.15156v1/2605.15156v1/x2.png

Results.
A single SFT run consumes $\approx 24$ GPU-hours on 8$\times$H100; after two arrivals, full retraining incurs $X{+}(X{+}Y){=}72$ GPU-hours of cumulative compute, whereas merging accumulates only $X{+}Y{=}48$ GPU-hours — a 33% reduction ([Fig.˜2](#A8.F2)). The asymptotic gap widens with $K$: under the same per-corpus cost, merging scales as $\Theta(K)$ while full retraining scales as $\Theta(K^{2})$, yielding a $5.5{\times}$ saving at $K{=}10$ ($240$ vs. $1{,}320$ GPU-hours). On accuracy, Merge-TIES ($\rho{=}0.3$) trails full retraining by $11.0$ pp with Qwen2.5-32B-Instruct as Executive model ($15.81\%$ vs. $26.85\%$) and by $19.1$ pp with Gemini-3-Flash ($34.47\%$ vs. $53.58\%$), placing the merged Memory model below the union-retrained Memory model but above every retrieval baseline. The full per-method sweep is reported in [Table˜12](#A8.T12): TIES ($\rho{=}0.3$) and DARE-Linear ($\rho{=}0.3$) lead at $15.81\%$ and $15.47\%$ respectively, while SLERP ($t{=}0.5$) is the worst configuration at $7.85\%$. The pattern across families suggests that aggressive sparsification at low $\rho$ paired with sign-conflict resolution (TIES, DARE-Linear) is the most reliable merging recipe in this regime. These results confirm the predicted compute–accuracy trade-off: merging recovers most of Memory model’s headroom over retrieval methods at substantially lower cumulative cost.

**Table 12: Sweep of all 14 merge configurations on NarrativeQA. Two Memory models (Qwen2.5-14B-Instruct) are independently SFT-trained on the disjoint NarrativeQA.1 and NarrativeQA.2 subsets; each subset’s best-performing checkpoint provides the task vector entering the merge. Executive model = Qwen2.5-32B-Instruct; results are mean $\pm$ std. dev. over 3 runs. Best merge in bold; full-retrain accuracy ($26.85\pm 0.39$) is shown for reference. Hyperparameter conventions: $t\in[0,1]$ is the SLERP interpolation factor along the unit sphere connecting the two task vectors ($t{=}0$ recovers Memory model on NarrativeQA.1, $t{=}1$ recovers Memory model on NarrativeQA.2, $t{=}0.5$ is the geodesic midpoint); $\rho\in(0,1]$ is the sparsification density — the fraction of largest-magnitude task-vector entries kept (TIES) or the keep probability for random-drop sparsification (DARE, DARE-TIES). Linear and Task arithmetic merge with uniform weights ($\lambda_{i}=1$) and have no hyperparameter.**
| Method family | Hyperparameter | Accuracy (%) |
| --- | --- | --- |
| Linear | — | $11.60\pm 1.02$ |
| Task arithmetic | — | $12.74\pm 1.75$ |
| SLERP | $t=0.3$ | $11.60\pm 2.24$ |
| $t=0.5$ | $\phantom{0}7.85\pm 1.71$ |  |
| $t=0.7$ | $11.60\pm 2.13$ |  |
| TIES | $\rho=0.3$ | $\mathbf{15.81\pm 0.39}$ |
| $\rho=0.5$ | $12.17\pm 1.94$ |  |
| $\rho=0.7$ | $12.06\pm 2.58$ |  |
| DARE-Linear | $\rho=0.3$ | $15.47\pm 0.79$ |
| $\rho=0.5$ | $\phantom{0}9.78\pm 1.20$ |  |
| $\rho=0.7$ | $13.65\pm 2.08$ |  |
| DARE-TIES | $\rho=0.3$ | $11.72\pm 0.52$ |
| $\rho=0.5$ | $12.97\pm 1.23$ |  |
| $\rho=0.7$ | $11.04\pm 1.20$ |  |

## Appendix I Validating evaluation dataset suitability

**Table 13: Performance gap between no context and perfect retrieval across datasets and Executive models.**
|  | Qwen2.5-32B-Instruct | Gemini-3-Flash |  |  |  |  |
| --- | --- | --- | --- | --- | --- | --- |
|  | BrowseComp-Plus | NarrativeQA | MuSiQue | BrowseComp-Plus | NarrativeQA | MuSiQue |
| No Context | $0.00\pm 0.00$ | $5.35\pm 0.20$ | $17.03\pm 0.40$ | $1.33$ | $26.62$ | $41.80$ |
| Perfect Retrieval | $79.67\pm 1.45$ | $51.42\pm 0.52$ | $62.83\pm 0.90$ | $88.33$ | $60.41$ | $73.00$ |

To assess the suitability of the evaluation datasets for Executive model and whether the Executive model has memorized answers from training data, we evaluate performance both without any context (No Context) and with evidence documents provided (Perfect Retrieval), the latter serving as an empirical upper-bound that assumes perfect retrieval of relevant documents.

As shown in [Table˜13](#A9.T13), the large disparity in performance between No Context and Perfect Retrieval confirms that these datasets require access to evidence documents to achieve correct answers, validating their suitability for evaluating MeMo.

Unsurprisingly, MuSiQue yields the highest No Context scores, as its Wikipedia-grounded questions fall within models’ parametric knowledge. NarrativeQA proves most challenging as it achieves the lowest Perfect Retrieval scores across both Executive models, reflecting the demand for careful reasoning over full-length books and movie scripts. BrowseComp-Plus yields the largest disparity between No Context and Perfect Retrieval, with near-zero No Context performance but strong recovery when evidence documents are provided.

These findings confirm that Executive model heavily relies on evidence documents across all three datasets to perform well. MuSiQue tests multi-hop factual reasoning where parametric knowledge provides partial signals, NarrativeQA tests narrative comprehension that remains challenging even with perfect context, and BrowseComp-Plus tests the ability to exploit retrieved documents for facts otherwise entirely inaccessible to the model.

## Appendix J Evaluation details

### J.1 Implementation details

The current temperature settings are described in [Table˜14](#A10.T14). Stage 1 only has a budget of $1$ interaction, Stage 2 has a budget of $7$ interactions, Stage 3 has a budget $8$ interactions.

**Table 14: Temperature Configuration of each Stage from [Section˜4.4](#S4.SS4)**
| Stage | Model | Temperature Value | Intent |
| --- | --- | --- | --- |
| Evaluation Stage 1 – Grounding | Executive model | 0.4 | Moderate exploration to generate diverse but focused sub-questions |
| Evaluation Stage 1 – Grounding | Memory model | 0.1 | Near-deterministic to ensure stable, consistent grounding answers |
| Evaluation Stage 2 – Entity identification | Executive model | 0.4 | Moderate exploration to identify varied candidate entities without excess noise |
| Evaluation Stage 2 – Entity identification | Memory model | 0.1 | Near-deterministic to produce reliable entity-targeted answers |
| Evaluation Stage 3 – Answer Seeking | Executive model | 1.0 | High exploration to maximally diversify sub-questions once the entity is confirmed |
| Evaluation Stage 3 – Answer Seeking | Memory model | 0.3 | Slightly relaxed determinism to allow nuanced answers while remaining consistent |
| Final Synthesis | Executive model | 0.3 | Low temperature to produce a consistent final answer |

**Table 15: Helper Functions for Stage 2 and 3 of the Evaluation Pipeline**
| Function | Stage | Intent |
| --- | --- | --- |
| Track uncertain answer streaks | Stage 2 | Maintains a running tally of how many unanswerable questions each candidate entity has accumulated across Stage 2, allowing the Executive model to progressively prioritize candidates that the Memory model consistently cannot corroborate |
| Select the best candidate | Stage 2 | Fallback bridge from Stage 2 to Stage 3 when entity pinning ends without a confirmed entity. Selects the highest Executive model-ranked candidate, with ties broken by the order in which the Memory model produced the candidates |
| Entity pivot correction | Stage 3 | Allows the pipeline to self-correct mid Stage 3 if the Stage 2 entity proves incorrect. When the Executive model nominates a different entity, the confirmed entity is overwritten and marked as unconfirmed so subsequent turns are aware it was not pinned through the full Stage 2 process |

Beyond what is described in [Section˜4.4](#S4.SS4), there are additional helper functions that help manage failure modes across Stage 2 and Stage 3. Within Stage 2, the uncertain answer streak tracker is called at the start of every entity-pinning interaction and its output is passed directly into the entity-pinning prompt, giving the Executive model a live view of which candidates the Memory model has repeatedly failed to corroborate. This allows the Executive model to continuously re-rank and prune the candidate pool as evidence accumulates. When Stage 2 concludes without a confirmed entity, either because the Executive model explicitly exhausts its options or the interaction budget is reached, the best candidate selector acts as the bridge into Stage 3 by returning the top-ranked candidate. In cases where multiple candidates share the highest rank, the first candidate in the order produced by Executive model is selected. In both cases, the downstream Stage 3 prompt is informed of whether the entity was formally confirmed or merely a best guess. Finally, if Stage 3 reveals that the Stage 2 entity was incorrect due to persistent Memory model failures, the entity pivot mechanism allows the Executive model to nominate a replacement entity mid-stage. The confirmed entity is then overwritten and marked as unconfirmed, ensuring subsequent stages treat it with appropriate uncertainty rather than the confidence of a fully pinned entity.

### J.2 Ablations on evaluation setup

To justify our structured multi-turn evaluation design, we compare against two baselines: a single-turn setup and an *unstructured* multi-turn setup; in both cases, the same trained Memory model is used and Executive model is held fixed. Results are reported in [Table˜16](#A10.T16).

**Table 16: MeMo accuracy results with Qwen2.5-32B-Instruct as Executive model and Qwen2.5-14B-Instruct as Memory model across evaluation setups. The best performing epoch was used in comparison across all 3 setups, with mean $\pm$ std. dev. reported across 3 runs. Bold results indicate best performing results in the column.**
| Evaluation Setup | BrowseComp-Plus Accuracy | NarrativeQA Accuracy | MuSiQue Accuracy |
| --- | --- | --- | --- |
| Single turn evaluation | $32.56\pm 1.58$ | $24.80\pm 0.20$ | $37.57\pm 1.15$ |
| Unstructured multi-turn evaluation<br>(15 turns) | $47.33\pm 0.88$ | $26.73\pm 2.17$ | $40.13\pm 1.12$ |
| Unstructured multi-turn evaluation<br>(50 turns) | $48.67\pm 1.00$ | $27.19\pm 0.71$ | $40.57\pm 0.31$ |
| Structured multi-turn evaluation<br>(7 Entity Identification turns +<br>8 Answer seeking turns) | $\mathbf{54.22\pm 0.84}$ | $26.39\pm 1.75$ | $\mathbf{48.30\pm 1.25}$ |
| Structured multi-turn evaluation<br>(7 Entity Identification turns +<br>15 Answer seeking turns) | $51.44\pm 2.41$ | $\mathbf{27.76\pm 0.20}$ | $47.57\pm 0.95$ |

In a single-turn interaction, Executive model first determines whether the question requires external memory retrieval, and if so, decomposes it into a set of sub-questions (Stage 1, [Section˜4.4](#S4.SS4)) and poses them all simultaneously to Memory model. Memory model responds to each sub-question independently, and responses indicating uncertainty are discarded before the remaining answers are passed to Executive model for final synthesis. This design requires Executive model to commit to its full sub-question set before observing any responses, preventing it from reformulating uninformative queries, following up on answers that introduce new candidate entities, or correcting retrievals that are incomplete, contradictory, or anchored to the wrong entity. This is a fundamental limitation that is reflected in its consistently lowest performance across all three datasets ([Table˜16](#A10.T16)).

A natural extension of the single-turn setting is an unstructured multi-turn interaction, where Executive model examines the responses from Memory model and decides whether sufficient information has been gathered, or whether additional retrieval rounds are needed (Stage 3, [Section˜4.4](#S4.SS4). In this setting, Executive model is presented with the full history of question-answer pairs and prompted to either synthesize a final answer or generate a new batch of sub-questions targeting remaining gaps, repeating for up to $T$ interactions. While iterative retrieval yields clear improvements over the single-turn baseline, performance plateaus quickly when increasing from 15 to 50 interactions ($47.33\pm 0.88$ to $48.67\pm 1.00$ on BrowseComp-Plus, $26.73\pm 2.17$ to $27.19\pm 0.71$ on NarrativeQA, and $40.13\pm 1.12$ to $40.57\pm 0.31$ on MuSiQue), suggesting that iterative retrieval alone is insufficient.

The structured multi-turn setup (see [Sections˜4.4](#S4.SS4) and [J.1](#A10.SS1)) outperforms the unstructured multi-turn baseline, with 8 answer-seeking interactions achieving the strongest overall performance for BrowseComp-Plus and MuSiQue. This is consistent with the expectation that explicit entity identification is well-suited to the multi-hop reasoning demands of these datasets. NarrativeQA, which tests discourse understanding over long documents, has the unstructured 15- and 50-interaction baselines ($26.73\pm 2.17$ and $27.19\pm 0.71$) initially outperform the structured setup with 8 answer-seeking interactions ($26.39\pm 1.75$). Inference logs indicate that Executive model rarely utilizes the entity identification stage on NarrativeQA, likely because its questions are less reliant on resolving specific entities.
Consequently, the fixed-entity identification budget effectively reduces the number of available answer-seeking interactions compared to unstructured baselines. Increasing the answer-seeking budget to 15 interactions recovers this gap, with NarrativeQA reaching $27.76\pm 0.20$, surpassing both unstructured baselines. We hypothesize that this could be due to the additional answer-seeking interactions continue to surface useful signal without the risk of entity drift or state corruption that compounds in open-domain multi-hop settings.

Unlike NarrativeQA, structured entity identification and state tracking in BrowseComp-Plus and MuSiQue introduce sensitivity to error accumulation as the number of answer-seeking interactions increases beyond the optimal budget. Additional interactions increase the risk of erroneous Memory model responses corrupting the known facts state, and provide more opportunities for Executive model to commit to an incorrect intermediate entity via the entity pivot correction helper ([Table˜15](#A10.T15)). Furthermore, more interactions dilute the correct signal with potentially incorrect answers at the final synthesis stage. These failure modes are partly a function of the reasoning capability of Executive model, as structured state maintenance demands strong in-context reasoning to accurately track entities and avoid premature entity commitment. Corroborating this, we observe in [Table˜2](#S5.T2) that a stronger reasoning model as Executive model yields improved performance when paired with the same Memory model, suggesting that these failure modes can be mitigated by scaling the reasoning capability of Executive model.

The stage budget used in our experiments was selected without systematic tuning, and alternative settings may yield similar performance with greater token efficiency. We therefore leave a systematic study of the optimal interaction budget and Executive model selection as future work.

## Appendix K Discussion on number of training epochs

From [Figs.˜3](#A11.F3), [4](#A11.F4) and [5](#A11.F5), we observe that additional training epochs do not consistently improve accuracy, as peak performance for most Memory model occurs at epoch 2 with marginal gains or mild regression thereafter.
We attribute the early saturation and subsequent regression to overfitting on the SFT corpus, which exhibits substantial lexical overlap across steps by design, as later steps are derived from earlier ones [Algorithm˜1](#alg1). To quantify this lexical overlap, we compute the lossless compression ratio of the combined QA text across all steps for each dataset by extracting all question and answer strings, concatenating them into a single text corpus, and applying gzip compression at maximum level (compression level 9), where the compression ratio is defined as the ratio of the original text size to the compressed size.

Figure: Figure 3: BrowseComp-Plus accuracy (%) vs. training epoch (Full SFT) for each MeMo model size and model family. Lines show the mean over 3 runs, and the shaded band shows $\pm$ std. dev. for Qwen2.5-32B-Instruct Runs.
Refer to caption: https://arxiv.org/html/2605.15156v1/2605.15156v1/figures/bcp_epoch_vs_acc.png

Figure: Figure 4: NarrativeQA accuracy (%) vs. training epoch (Full SFT) for each MeMo model size and model family. Lines show the mean over 3 runs, and the shaded band shows $\pm$ std. dev. for Qwen2.5-32B-Instruct Runs.
Refer to caption: https://arxiv.org/html/2605.15156v1/2605.15156v1/figures/nqa_epoch_vs_acc.png

Figure: Figure 5: MuSiQue accuracy (%) vs. training epoch (Full SFT) for each MeMo model size and model family. Lines show the mean over 3 runs, and the shaded band shows $\pm$ std. dev. for Qwen2.5-32B-Instruct Runs.
Refer to caption: https://arxiv.org/html/2605.15156v1/2605.15156v1/figures/msq_epoch_vs_acc.png

BrowseComp-Plus ($1{,}639{,}995$ pairs) achieves a ratio of $5.80\times$ ($82.8\%$ savings), MuSiQue ($664{,}762$ pairs) achieves $7.03\times$ ($85.8\%$ savings), and NarrativeQA ($1{,}276{,}676$ pairs) achieves $5.45\times$ ($81.7\%$ savings), indicating substantial lexical overlap within each dataset. We note that compression ratio captures lexical overlap only, and semantic diversity across QA pairs may remain higher, as each step targets distinct reasoning operations ranging from direct fact extraction to cross-document synthesis [Algorithm˜1](#alg1), which is consistent with the impact of removing Step 5 (see [Appendix˜E](#A5)).

## Appendix L Performance degradation of retrieval-based methods with increasing noise

**Table 17: Accuracy (%) on BrowseComp-Plus and MuSiQue with Qwen2.5-32B-Instruct as Executive model. MeMo results are based on Qwen2.5-14B-Instruct and reported at the best training epoch. $N$ is the number of evidence documents present in the target corpus. $\Delta$ denotes accuracy difference (pp) compared to $0N$.**
| Method | Dataset | $0N$ | $1N$ | $2N$ |  |  |
| --- | --- | --- | --- | --- | --- | --- |
|  |  | Acc. (%) | Acc. (%) | $\Delta$ | Acc. (%) | $\Delta$ |
| NV-Embed-V2 | BrowseComp-Plus | $56.89\pm 0.51$ | $50.67\pm 0.33$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 6.22}$ | $49.44\pm 0.19$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 7.45}$ |
| MuSiQue | $42.30\pm 0.53$ | $37.47\pm 0.15$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 4.83}$ | $33.03\pm 1.10$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 9.27}$ |  |
| HippoRAG2 | BrowseComp-Plus | $62.33\pm 1.15$ | $56.11\pm 0.51$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 6.22}$ | $50.78\pm 1.35$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 11.55}$ |
| MuSiQue | $47.33\pm 0.74$ | $42.17\pm 0.12$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 5.16}$ | $41.70\pm 0.69$ | ${\color[rgb]{1,0,0}\definecolor[named]{pgfstrokecolor}{rgb}{1,0,0}\downarrow 5.63}$ |  |

[Table˜17](#A12.T17) reports the performance of two retrieval-based baselines (NV-Embed-V2 and HippoRAG2) under increasing retrieval noise. Both methods degrade monotonically as noise increases, confirming their susceptibility to irrelevant documents. The degradation is most severe for HippoRAG2 on BrowseComp-Plus, which drops 11.55 pp from 0N to 2N, and for NV-Embed-V2 on MuSiQue, which drops 9.27 pp over the same range. Notably, even a single negative document per evidence document (1N) causes substantial drops of up to 6.22 pp for both methods on BrowseComp-Plus, suggesting that retrieval-based methods are extremely sensitive to noisy retrieval settings.

## Appendix M Ablation on Memory model size

Both Qwen2.5-1.5B-Instruct and Qwen2.5-14B-Instruct Memorys are trained on the same QA dataset generated by the Generator model (Qwen2.5-32B-Instruct) under training settings described in [Appendix˜F](#A6).
Each Memory model is evaluated using Qwen2.5-32B-Instruct and Gemini-3-Flash as the Executive model.

## Appendix N Ablation on Memory model family

Each Memory model is trained on the same QA dataset generated by Generator model (Qwen2.5-32B-Instruct) and evaluated using Qwen2.5-32B-Instruct and Gemini-3-Flash as Executive model.
Notably, while Qwen2.5-1.5B-Instruct and Gemma3-1B-IT are based on standard transformer architectures, LFM2.5-1.2B-Instruct adopts a hybrid architecture combining state-space convolution with transformer attention blocks, thereby providing a broader test of Memory model across diverse model designs. These models are trained on the same training settings in [Appendix˜F](#A6), with Gemma3-1B-IT using eager attention during training instead of Flash Attention 2.

## Appendix O Comparison between full SFT and LoRA

We train all models using LoRA  applied to the attention and feed-forward projection layers: q_proj, k_proj, v_proj, o_proj, gate_proj, up_proj, and down_proj. The general LoRA configuration is summarised in Table [18](#A15.T18), with model-specific rank and scaling settings reported in Table [19](#A15.T19). All remaining training hyperparameters follow Table [10](#A6.T10), and per-dataset batch sizes are given in Table [11](#A6.T11).

**Table 18: LoRA Specific Training Configuration. All other parameters are the same as those in [Table˜10](#A6.T10).**
| Parameter | Value |
| --- | --- |
| Target modules | q_proj, k_proj, v_proj, o_proj, |
|  | gate_proj, up_proj, down_proj |
| LoRA dropout | 0.05 |
| Bias | None |
| Learning rate | $2\times 10^{-4}$ |

**Table 19: Model-Specific LoRA Configuration.**
| Model | Size | LoRA rank | LoRA alpha | Trainable params |
| --- | --- | --- | --- | --- |
| LFM2.5-1.2B-Instruct | 1.2B | 8 | 16 | 6.1M (0.41%) |
| Gemma3-1B-IT | 1B | 8 | 16 | 6.6M (0.65%) |
| Qwen2.5-1.5B-Instruct | 1.5B | 8 | 16 | 9.2M (0.60%) |
| Qwen2.5-14B-Instruct | 14B | 16 | 32 | 68.8M (0.47%) |

**Table 20: Ablation on LoRA vs Full SFT training across all Memory models, evaluated with Qwen2.5-32B-Instruct as Executive model. All results are mean $\pm$ std. dev. over 3 runs. Bold results indicate best performing results in the column.**
|  | BrowseComp-Plus | NarrativeQA | MuSiQue |  |  |  |
| --- | --- | --- | --- | --- | --- | --- |
| Memory model | LoRA | Full SFT | LoRA | Full SFT | LoRA | Full SFT |
| Gemma3-1B-IT | $25.22\pm 1.39$ | $41.67\pm 2.03$ | $21.62\pm 0.86$ | $22.30\pm 2.47$ | $26.17\pm 1.10$ | $41.17\pm 1.20$ |
| LFM2.5-1.2B-Instruct | $0.78\pm 0.19$ | $37.33\pm 1.86$ | $5.69\pm 0.71$ | $21.96\pm 1.97$ | $7.50\pm 0.26$ | $45.23\pm 2.49$ |
| Qwen2.5-1.5B-Instruct | $29.78\pm 0.51$ | $44.11\pm 2.22$ | $21.84\pm 0.34$ | $24.00\pm 0.20$ | $31.53\pm 0.55$ | $42.90\pm 1.39$ |
| Qwen2.5-14B-Instruct | $\mathbf{48.78\pm 1.02}$ | $\mathbf{54.22\pm 0.84}$ | $\mathbf{23.78\pm 0.52}$ | $\mathbf{26.85\pm 0.39}$ | $\mathbf{43.94\pm 0.97}$ | $\mathbf{50.07\pm 0.81}$ |

The notably poor LoRA performance of LFM2.5-1.2B-Instruct can be attributed to its hybrid convolution–attention architecture, which differs from the standard transformer models in our evaluation. Following the LFM2 architecture , LFM2.5-1.2B-Instruct consists of $16$ layers — $6$ grouped-query attention (GQA) blocks (at indices $\{2,5,8,10,12,14\}$) interleaved with $10$ short-range LIV convolution (ShortConv) blocks . Crucially, the LFM2 attention output projection is named out_proj (rather than o_proj) and its SwiGLU MLP uses w1/w3/w2 (rather than gate_proj/up_proj/down_proj), while the ShortConv blocks expose their own in_proj and out_proj layers. A LoRA configuration targeting the standard Llama-family module names therefore adapts only a strict subset of the projections that exist in LFM2.5, leaving the remainder frozen. The result is $6.1$M trainable parameters ($0.41\%$ of total), disproportionately low given the model size and below our target of ${\sim}0.5\%$. The rank $r=8$ was kept fixed across all sub-2B models for a controlled comparison; in retrospect, this penalises LFM2.5-1.2B-Instruct due to its architectural mismatch with the standard Llama-style target set.

Furthermore, the $10$ ShortConv blocks which handle the bulk of the model’s local feature extraction and the SwiGLU MLPs attached to every block remain entirely unadapted under standard LoRA targeting, severely limiting the adapter’s ability to shift the model’s behaviour. As shown in [Table˜20](#A15.T20), the large performance gap between LoRA and Full SFT confirms that the model is capable of learning the task when all parameters are updated.

Future work could explore LoRA configurations better suited to this architecture: targeting the LFM2-specific module names (out_proj, w1, w3, w2) alongside the ShortConv projections (in_proj, out_proj), as well as tuning the rank and learning rate per architecture rather than holding them fixed across families for the controlled comparison reported here.
