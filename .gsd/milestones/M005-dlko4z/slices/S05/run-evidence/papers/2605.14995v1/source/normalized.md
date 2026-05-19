## Abstract

Abstract Every day, users generate digital traces (e.g., social media posts, chats, and online interactions) that are inherently timestamped and may reflect aspects of their mental state. These traces can be organized into temporal trajectories that capture how a user’s mental health signals evolve, including phases of improvement, deterioration, or stability.
In this work, we propose an explainable framework for detecting and analyzing depression-related status shifts in user digital traces. The approach combines multiple BERT-based models to extract complementary signals across different dimensions (e.g., sentiment, emotion, and depression severity). Such signals are then aggregated over time to construct user-level trajectories that are analyzed to identify meaningful change points.
To enhance interpretability, the framework integrates a large language model to generate concise and human-readable reports that describe the evolution of mental-health signals and highlight key transitions.
We evaluate the framework on two social media datasets. Results show that the approach produces more coherent and informative summaries than direct LLM-based reporting, achieving higher coverage of user history, stronger temporal coherence, and improved sensitivity to change points. An ablation study confirms the contribution of each component, particularly temporal modeling and segmentation. Overall, the method provides an interpretable view of mental health signals over time, supporting research and decision making without aiming at clinical diagnosis.

## 1 Introduction

In recent years, everyday digital traces, including private communications, web searches, mobility traces, and social media interactions, have increasingly become a medium through which individuals express emotions, opinions, behaviors, and personal experiences . When analyzed longitudinally, these traces may implicitly reflect aspects of a person’s psychological condition and reveal how mental well-being evolves over time. Rather than providing a static representation, user-generated content often forms a temporal trajectory in which phases of deterioration, recovery, or relative stability can emerge through linguistic and behavioral patterns.

Several studies have investigated the use of social media data to automatically detect indicators of mental distress, including depression, major depressive disorder, and suicidal ideation, through natural language processing, machine learning, and deep learning techniques . Early work showed that linguistic and behavioral signals extracted from platforms such as Twitter and Reddit can support the recognition of depressive symptoms, while more recent approaches have exploited transformer-based models and richer textual representations to improve classification performance . However, much of this literature still relies on post-level or user-level classification settings, whereas comparatively less attention has been devoted to modeling how mental-health signals evolve over time. Recent work on temporal boundaries in depression classification suggests that longitudinal segmentation can provide useful information for distinguishing depressive and control users .

Transformer-based architectures such as BERT and related language models have significantly advanced textual analysis by capturing contextual and semantic relationships within language. These models have achieved state-of-the-art performance in tasks including sentiment analysis, emotion recognition, and mental health classification . However, they are still commonly applied to classify posts independently, without explicitly modeling how mental health signals evolve across sequences of interactions over time. As a consequence, current approaches often provide limited insight into the temporal dynamics underlying changes in a user’s psychological condition and may fail to identify when meaningful shifts in mental state occur.

To address this limitation, this work introduces an explainable framework for tracing mental-health trajectories in individual digital traces. Our approach analyzes not only single posts but the entire sequence of posts associated with a user, modeling how linguistic, emotional, and psychological patterns evolve over time. The framework is based on a multi-classification strategy that combines several fine-tuned BERT-based models, each specialized in a specific dimension (e.g., sentiment, emotion, mental-health category) . By integrating these complementary signals, the system builds a multidimensional representation of each post. A temporal reasoning module then processes the sequence of annotated posts to infer the user’s trajectory, explicitly identifying change points where the trajectory significantly changes direction (toward improvement or deterioration), as well as intervals of stability.

Explainability is a central part of the proposed framework, as interpretability is widely recognized as essential for the adoption of AI-based mental-health detection systems in clinical and high-stakes settings . Each BERT-based model produces signals that capture linguistic, emotional, and semantic patterns associated with the user’s mental state. These signals are aggregated over time to provide a global view of how such patterns evolve across the user’s timeline. To make this information accessible to clinicians and researchers, the framework employs a large language model (LLM) to transform the enriched signals into concise and human-readable summaries. These summaries describe the overall trend and highlight key transitions, linking them to recurring expressions, themes, and behavioral patterns observed in the data. More broadly, this type of system could be integrated into tools that support users and professionals (e.g., clinicians, psychologists, researchers) by highlighting meaningful changes in mental-health–related signals and facilitating reflection and early awareness.
Unlike traditional explainable AI approaches, which primarily focus on feature attribution at the token level, our framework provides *trajectory-level explainability*, where explanations are expressed as structured summaries of temporal patterns, phases, and transitions that directly reflect the evolution of mental-health signals over time.

In this work, we apply and evaluate our framework on two social media datasets from Reddit and Twitter, focusing on users who utilize online platforms as a personal space to share experiences. This enables the construction of detailed timelines that capture the evolution of their behavior and emotions over time. Experimental results show that our approach, which explicitly models temporal dynamics, is more effective at detecting meaningful shifts in mental-health signals compared to methods that ignore time or use a single model. In addition, the framework preserves interpretability by producing structured and human-readable summaries.
The system is not intended for clinical diagnosis; rather, it is designed as a decision-support and research tool that provides transparent signals and interpretable summaries. These outputs must be carefully interpreted and validated by domain experts, such as clinicians or researchers, within an appropriate clinical and ethical context.

The remainder of this paper is organized as follows. Section [2](#S2) reviews existing literature on mental health detection from social data. Section [3](#S3) presents the proposed framework. Section [4](#S4) reports the experimental evaluation and comparative analysis. Section [5](#S5) discusses limitations and future research directions. Finally, Section [6](#S6) concludes the paper.

## 2 Related work

Digital traces generated through online interactions have increasingly been used to investigate indicators of mental health conditions, including depression, anxiety, suicidal ideation, and eating disorders. Several studies have shown that linguistic and behavioral patterns extracted from social media platforms can provide meaningful signals associated with psychological distress . With the increasing availability of user-generated data from platforms such as Twitter, Reddit, and Facebook, machine learning and NLP techniques have been widely adopted to automatically identify signals associated with mental distress .

Early approaches mainly relied on traditional machine learning algorithms, including Naive Bayes, Random Forests, support vector machines, and handcrafted linguistic or affective features . Several studies demonstrated that lexical, emotional, and behavioral signals extracted from social media activity can provide meaningful indicators of depressive symptoms and psychological vulnerability .

The introduction of transformer architectures, particularly BERT and its variants, significantly advanced the field by enabling richer contextual and semantic representations of user-generated text . Recent studies have shown that transformer-based models can effectively capture subtle linguistic patterns associated with depression and mental distress . More recent research has also explored the integration of large language models (LLMs) and explainable artificial intelligence (XAI) techniques to improve the interpretability of mental health prediction systems . In particular, approaches based on SHAP, LIME, and natural-language explanations have been proposed to bridge the gap between model predictions and human-understandable psychological interpretations .

Beyond post-level classification, several recent studies have emphasized the importance of modeling temporal dynamics and longitudinal behavioral patterns. Villa-Pérez et al.  demonstrated that temporal segmentation of user timelines can improve depression classification performance by distinguishing behavioral patterns before and after self-reported diagnoses. Other works have investigated sequential and trajectory-aware approaches to reconstruct the evolution of depressive symptoms over time using recurrent neural networks, hybrid architectures, and transformer-based representations . In parallel, topic modeling and LLM-driven semantic analysis have increasingly been used to identify thematic transitions and evolving psychological trends within social media discussions .

Despite these advances, most existing approaches still focus on isolated tasks such as classification, topic extraction, or explanation independently. Comparatively less attention has been devoted to unified frameworks capable of integrating heterogeneous digital traces, temporal trajectory modeling, explainable classification, and interactive narrative interpretation within a single end-to-end pipeline. Moreover, many existing systems provide static predictions without explicitly modeling how psychological states evolve over time or how users and experts can interactively explore such trajectories.

To address these limitations, the framework proposed in this work combines: $(i)$ heterogeneous digital trace acquisition and preprocessing, $(ii)$ BERT-based multi-dimensional classification for extracting mental-health–related signals, $(iii)$ temporal trajectory construction with change-point detection to model behavioral evolution over time, and $(iv)$ an LLM-driven reporting layer for generating human-interpretable outputs. Unlike prior work focused on isolated prediction tasks, the proposed approach integrates semantic enrichment, temporal analysis, explainability, and interactive exploration within a unified framework. The framework can be extended with a Retrieval-Augmented Generation (RAG) mechanism, allowing users and domain experts to query trajectories, inspect contextual information associated with specific periods, and obtain narrative-level explanations of behavioral changes over time.

## 3 Proposed methodology

The proposed framework represents mental-health–related trajectories through a structured pipeline that integrates heterogeneous digital traces, semantic enrichment via BERT-based models, and temporal reasoning, as illustrated in Figure [1](#S3.F1). The workflow is organized into five main stages: $(i)$ digital trace acquisition, $(ii)$ data preparation, $(iii)$ multidimensional data enrichment using BERT, $(iv)$ trajectory construction and change-point detection, and $(v)$ report generation with user interaction.

Figure: Figure 1: Execution flow of the proposed framework.
Refer to caption: https://arxiv.org/html/2605.14995v1/2605.14995v1/x1.png

### 3.1 Digital trace acquisition

The framework is designed to ingest heterogeneous textual data generated by users across multiple digital environments, such as social media posts, chats, web searches, transcripts, clinical records, and data obtained through APIs or digital services. In this study, we evaluate the framework on social media traces, while other sources represent possible extensions of the same event-based representation. Each piece of information is treated as a *digital trace*, potentially encoding implicit signals about the mental state of the user.
To keep the framework general, all traces are mapped into a unified event representation. Each *event* is defined as a tuple that contains a user identifier, a timestamp, a source label, and the text content. This representation lets the framework work with data from different sources while preserving the time order. For each user $u$, all events are merged and sorted in chronological order to form a single timeline $\{e_{u,1},\dots,e_{u,n}\}$, where each event can be written as $e_{u,i}=(u,d_{u,i},\rho_{u,i},t_{u,i})$, with timestamp $d_{u,i}$, source label $\rho_{u,i}$, and text content $t_{u,i}$.

### 3.2 Data preparation

The data preparation stage ensures that heterogeneous and potentially noisy traces are transformed into a consistent and reliable format suitable for downstream analysis. This process includes filtering, missing value handling, and normalization. Filtering removes duplicated, corrupted, or non-informative events, as well as source-specific irrelevant content. Missing values are handled by either inferring attributes (e.g., estimating timestamps from neighboring events) or discarding incomplete records when reconstruction is not reliable. Finally, normalization standardizes textual content (e.g., lowercasing and cleaning) and harmonizes timestamp formats across sources, thereby enabling robust inputs for subsequent BERT-based analysis.

### 3.3 Multidimensional data classification using BERT

To extract depression-related signals from each event, the framework employs BERT-based models trained to classify data across several dimensions, such as sentiment (e.g., positive, neutral, negative), emotion (e.g., sadness, anger, fear, joy), and discrete levels of depression severity (no, moderate, severe). In addition to the predicted label, the BERT-based classifier outputs a probability distribution over the set of classes:

$$ $\boldsymbol{\pi}_{u,i}=\big(\pi_{u,i}^{(1)},\pi_{u,i}^{(2)},\dots,\pi_{u,i}^{(C)}\big),\quad\sum_{c=1}^{C}\pi_{u,i}^{(c)}=1,$ $$

where $\pi_{u,i}^{(c)}$ represents the probability that event $i$ of user $u$ belongs to class $c$ (e.g., positive, negative, or neutral for the sentiment classifier).

In addition, BERTopic is employed to identify the main topics discussed in the dataset, providing a high-level understanding of thematic trends and emerging issues. This combined classification and topic modeling process enables a more comprehensive characterization of the data, supporting the identification of recurring themes, behavioral patterns, and depression-related signals by grouping semantically similar events. Although the current implementation is limited to English-language data, the proposed methodology can be extended to other languages by leveraging multilingual models such as mBERT, which is trained on several languages and provides strong cross-lingual capabilities.

### 3.4 Trajectory construction and change-point detection

Once each event has been annotated, the outputs of the depression classifier are aggregated at the daily level to compute an overall depression severity score for each day in the user’s timeline. Let $\{d_{u,1},\dots,d_{u,n}\}$ denote the ordered set of days on which user $u$ is active, and $\mathcal{C}=\{\text{no},\,\text{moderate},\,\text{severe}\}$ the set of depression severity classes. For each event $i$, the classifier produces a probability distribution over $\mathcal{C}$, which is mapped to a scalar score as:

$$ $s_{u,i}=\sum_{c\in\mathcal{C}}w_{c}\,\pi_{u,i}^{(c)},$ $$

where $\pi_{u,i}^{(c)}$ denotes the probability assigned to class $c$, and $w_{c}$ are predefined weights reflecting increasing severity levels (e.g., $w_{\text{no}}=0$, $w_{\text{moderate}}=1$, and $w_{\text{severe}}=2$). A higher score value indicates a higher level of depression. For each day $d_{u,j}$, we compute a daily score by aggregating all events occurring on that day:

$$ $r_{u,j}=\frac{1}{|\mathcal{I}_{u,j}|}\sum_{i\in\mathcal{I}_{u,j}}s_{u,i},$ $$

where $\mathcal{I}_{u,j}$ is the set of events associated with user $u$ on day $d_{u,j}$. The resulting sequence $\mathbf{r}_{u}=\{r_{u,1},\dots,r_{u,n}\}$ defines a univariate time series representing the user’s estimated depressive state over time. To reduce noise and highlight long-term trends, the trajectory is smoothed using a moving average filter, yielding the sequence $\tilde{\mathbf{r}}_{u}=\{\tilde{r}_{u,1},\dots,\tilde{r}_{u,n}\}$, which represents the user’s depression status trajectory over time.

Figure: Algorithm 1 Top-down piecewise linear segmentation

On the smoothed trajectory, we apply a top-down piecewise linear segmentation procedure to identify a small number of change points, as detailed in Algorithm [1](#alg1). This procedure is inspired by the Ramer–Douglas–Peucker algorithm for polygonal curve approximation , but it is adapted here to time-series segmentation by selecting a fixed maximum number of segments $K$ rather than using a distance-tolerance stopping criterion. Let $\{(d_{1},r_{1}),\dots,(d_{n},r_{n})\}$ denote the daily scores sorted by date, and let $\tilde{r}_{1},\dots,\tilde{r}_{n}$ be the corresponding smoothed values obtained with a centered moving average. For each day $d_{i}$, we define $x_{i}$ as the number of days elapsed from the first observation $d_{1}$, and we set $y_{i}=\tilde{r}_{i}$. A segment is then defined by an interval of indices $[a,b]$, with $1\leq a<b\leq n$, and is approximated by the straight line $\ell_{a,b}$ joining the two endpoints $(x_{a},y_{a})$ and $(x_{b},y_{b})$.

For each segment $[a,b]$, we measure its approximation error as the maximum perpendicular distance between the line $\ell_{a,b}$ and the intermediate points:

$$ $E(a,b)=\max_{a<j<b}\mathrm{dist}\big((x_{j},y_{j}),\ell_{a,b}\big).$ $$

This quantity captures how well a single straight line approximates the trajectory within the interval. A large value of $E(a,b)$ indicates that the segment contains an internal change in trend that is not well represented by a linear approximation.

The procedure starts from a single segment covering the entire trajectory, i.e., $\mathcal{S}=\{[1,n]\}$. At each iteration, the approximation error is evaluated for all current segments. Segments with fewer than three points, i.e., such that $b-a\leq 1$, are not further split, since they contain no internal point on which the deviation can be evaluated. Among all current segments, the one with the largest approximation error is selected. The split position $j^{\star}$ is then identified as the internal point with maximum distance from the line joining the segment endpoints, and the selected segment is replaced by the two subsegments $[a,j^{\star}]$ and $[j^{\star},b]$.

This process is repeated until the desired number of segments $K$ is reached or no segment can be further split with positive approximation error. After termination, the segments are sorted by their starting index, yielding an ordered segmentation

$$ $[a_{u,1},b_{u,1}],\dots,[a_{u,K_{u}},b_{u,K_{u}}],$ $$

where $K_{u}\leq K$. The internal boundaries of consecutive segments can be interpreted as change points of the trajectory. Each resulting segment therefore represents a coherent temporal interval that is subsequently analyzed and summarized by the LLM-based reporting component. In the experiments, we set $K=10$ to obtain a compact yet sufficiently detailed phase representation; sensitivity to this choice is indirectly assessed through the fixed-window and no-segmentation ablation variants.

### 3.5 Report generation and user interaction

The final step of the proposed framework transforms the segmented trajectory into a concise and human-readable report. After segmentation, each segment $[a_{u,k},b_{u,k}]$ is provided to a generative AI model together with the events belonging to that time interval and a small set of trajectory descriptors, such as the average severity level, the local trend, and the segment position in the overall timeline. To improve report quality, this information can be enriched with additional contextual evidence, such as operator annotations, relevant life events, or background notes that help interpret the trajectory. The framework can also incorporate a RAG component when the LLM needs access to evidence that should not be compressed into the trajectory alone. For example, the retriever may supply original data explaining a worsening phase, metadata about a specific interval, previously identified key events, clinician notes stored in the system, or domain-specific reference material.
The report generation process is explicitly grounded in the structured signals produced by the previous stages of the pipeline. In particular, each segment is associated with quantitative descriptors (e.g., average severity, trend direction, and temporal position) and a well-defined subset of events. The LLM is guided to generate explanations conditioned on this structured representation, through prompts that explicitly encode segment-level descriptors and associated events. This design encourages the model to produce summaries that reflect the underlying temporal dynamics rather than arbitrary narrative generation.

Starting from these inputs, the LLM generates a short natural-language report describing the inferred phase. These reports are designed to support clinicians and researchers by summarizing how depression-related signals evolve over time and by highlighting phases of worsening or improvement. In this way, the report does not replace the underlying trajectory, but presents it in a form that is easier to read, inspect, discuss, and validate.

Once the report has been generated, the operator can interact with the LLM to better understand specific parts of the trajectory. For example, the operator may ask what changed around a certain date, request the evidence behind a worsening phase, or ask for more details about a specific segment. In these cases, the RAG component can support the response by retrieving the most relevant information, such as original posts, segment summaries, trajectory statistics, clinician notes, or other useful external material. This allows the LLM to provide answers that are more accurate and better grounded in the available evidence.

## 4 Experimental Results

This section evaluates the proposed framework from both a qualitative and a quantitative perspective. We first describe the datasets used in the paper and motivate their suitability for longitudinal trajectory analysis. We then illustrate, through a small set of representative users, how raw writings are transformed into daily depression scores, segmented trajectories, and final narrative reports. Finally, we compare direct LLM-based summarization against the proposed trajectory-aware strategy and analyze the contribution of the main pipeline components through an ablation study.

To assess the effectiveness of our framework, we used two datasets: $(i)$ eRisk 2018 , a widely adopted benchmark for the early detection of depression from Reddit posts, and $(ii)$ the *Mental Health Social Media* dataset available on Kaggle(^0^00[https://www.kaggle.com/datasets/infamouscoder/mental-health-social-media](https://www.kaggle.com/datasets/infamouscoder/mental-health-social-media)). The eRisk collection is organized at the *user level*, where each subject is associated with a chronologically ordered history of writings, including both posts and comments. The official release contains 1,707 users and 1,076,582 posts. The *Mental Health Social Media* dataset contains 20,000 Twitter posts annotated with mental-health-related labels and spans a broader range of mental-health conditions, including depression. For the Mental Health Social Media dataset, trajectory-based analysis is applied only to records for which temporal information is available.
To support reproducibility, the implementation of the proposed framework is made publicly available(^1^11[https://github.com/SCAlabUnical/X-MiND](https://github.com/SCAlabUnical/X-MiND)). Due to the sensitivity of mental-health-related user data, we provide instructions for obtaining the original datasets from their official sources rather than redistributing raw posts.

### 4.1 Qualitative analysis of representative users

To clarify how the proposed framework operates, we analyze four representative users from the eRisk 2018 dataset, namely IDs 1257, 2714, 3307, and 9280, selected because they exhibit different temporal profiles. As an example, Table [1](#S4.T1) reports selected posts from User 2714, together with their dates, the depression label predicted by the classifier (*no*, *moderate*, *severe*), and the corresponding aggregated daily score. Figure [2](#S4.F2) shows the smoothed trajectories and the corresponding piecewise linear segmentations for the four users. User 2714 presents a long, non-monotonic trajectory with several relapses, whereas User 9280 shows a more localized episode of deterioration surrounded by relatively stable intervals. User 1257 follows a smoother and more regular trajectory, with fewer abrupt transitions, while User 3307 is characterized by a pronounced initial phase followed by a long, low-severity plateau. Overall, the framework does not impose a fixed temporal model on all users; instead, it produces individualized, phase-based representations that capture sustained deterioration, gradual improvement, temporary relapses, and relatively stable conditions.

**Table 1: Representative posts of user ID 2714 (eRisk 2018) with depression level and daily score.**
| Date | Post excerpt | Depression<br>Level | Class<br>Probability | Daily<br>Score |
| --- | --- | --- | --- | --- |
| 2013-10-04 | Well done, that’s a great achievement!<br>Better than mine: 0.<br>B…or stopping! If you don’t mind<br>me asking, how did you stop? | No | 0.983 | 0.057 |
| 2014-08-11 | Hi, I’ve been diagnosed with PTSD,<br>Major Depressive Disorder and I hear voices.<br>I’ve been hospitalized once for 7 months. | Severe | 0.914 | 0.585 |
| 2015-10-19 | My death is likely to send my mum back<br>to hospital.<br>The person …me and he has said<br>once I kill myself,<br>he won’t be far behind. | Moderate | 0.553 | 0.261 |

Figure: (a) User 2714.
Refer to caption: https://arxiv.org/html/2605.14995v1/2605.14995v1/x2.png

### 4.2 Report generation with generative models

Our framework aims at transforming the inferred trajectory into a concise and human-readable report. These reports are intended as decision-support outputs for clinicians and researchers: they should summarize the temporal evolution of depression-related signals, highlight phases of worsening or improvement, and provide a narrative that remains grounded in the user’s own language without making diagnostic claims. To this end, we compare two configurations. The first, denoted as GPT-base, is a direct summarization baseline. In this setting, all events of a user are concatenated in chronological order and passed to the LLM with a generic prompt to summarize the user’s mental-health history. The model is provided with only the raw texts and dates, and does not have access to the outputs of the proposed trajectory analysis.

The second configuration, denoted as GPT-traj, exploits the full pipeline described in Section [3](#S3). Posts are first enriched through the multidimensional classification, then aggregated into daily severity scores, smoothed, and segmented into phases by identifying the change points. Each phase is summarized separately by conditioning the LLM on the posts belonging to that interval together with basic segment-level descriptors such as average severity and trend direction. Then, a second LLM call composes these phase-level descriptions into a single global report, explicitly structured around the sequence of phases and the main change points.

To illustrate the qualitative difference between the two configurations, we consider User 2714, which is particularly suitable as an example because the timeline spans several years and contains both acute crisis periods and more stable intervals.
Below we report a shortened example of the type of summary produced using the GPT-base approach.

This baseline summary captures the general tone of chronic depression, but it tends to merge the entire history into a single narrative arc. As a consequence, changes over time are described only loosely, and the distinction between temporally separate phases remains weak.

In the following, we present the corresponding report produced by GPT-traj, in which the LLM leverages the segmented trajectory and phase-level summaries.

Compared with GPT-base, the trajectory-aware report is explicitly organized into temporally ordered phases and makes the evolution of the user’s condition much more visible. Instead of collapsing the entire history into a generic summary of chronic distress, it connects specific themes and emotional patterns to well-defined intervals of time. This makes the resulting narrative more aligned with the underlying trajectory and more useful for understanding how depression-related signals develop across the user’s history.

### 4.3 Aggregate evaluation

The qualitative examples discussed above suggest that trajectory-aware report generation yields richer and more temporally grounded summaries than direct LLM summarization. We now move to an aggregate evaluation in order to quantify this effect across a broader set of users.

Our aggregate analysis focuses on two complementary questions. First, does the trajectory-aware strategy cover the user’s history more completely and coherently than the baseline? Second, does the explicit use of segmentation and change points lead to better narrative reports from the point of view of independent judge models? To answer these questions, we evaluate report quality through two perspectives: topic coverage and *LLM-as-a-judge* comparison.

#### 4.3.1 Topic Coverage

We consider the ability of the generated reports to capture the semantic content of the user’s original timeline. In the baseline condition, the LLM processes the user’s entire timeline as a single input, which tends to favor a high-level synthesis around dominant and recurring topics. In contrast, the trajectory-based approach requires a step-by-step analysis of the user’s timeline, increasing the likelihood that temporally localized topics will be retained in the final report.

To quantitatively evaluate this aspect, for each user in the dataset, we extracted the top 15 topics from their posts using BERTopic. We then generated two reports for each user: one using the baseline approach (GPT-base) and one using the trajectory-based approach (GPT-traj). Finally, we used an external LLM (GPT 5.2) to evaluate how many of the extracted topics are actually covered in each report.
The results show a clear advantage for the trajectory-based approach. On average, GPT-base covers 40.62% of the extracted topics, whereas GPT-traj covers 84.08%. This substantial improvement indicates that the trajectory-based method provides significantly broader and more balanced topic coverage. Specifically, it is more effective at preserving not only the dominant depressive topics, but also secondary and stage-specific themes, which are often overlooked in the baseline setting. This aspect is particularly relevant in clinical and research contexts, where a more comprehensive representation of the user’s experiences can support domain experts in interpreting longitudinal patterns without replacing professional assessment.

Figure [3](#S4.F3) illustrates this behavior for two representative users, namely User 2714 and User 9280. For User 2714, GPT-base covers 7 out of 15 topics, whereas GPT-traj covers 12 out of 15 topics. Similarly, for User 9280, GPT-base covers 4 out of 15 topics, while GPT-traj covers 11 out of 15 topics. These examples visually confirm that the trajectory-based method captures a wider portion of the user’s topical space.

Figure: (a) User 2714.
Refer to caption: https://arxiv.org/html/2605.14995v1/2605.14995v1/x6.png

#### 4.3.2 LLM-as-a-judge

For a more systematic evaluation, we adopt an *LLM-as-a-judge* approach, where independent models compare, for each user, the reports produced by GPT-base and GPT-traj. For each user $u$ in the dataset, we generate two reports: $(i)$ a baseline report obtained by directly summarizing the full chronological post history, and $(ii)$ a trajectory-aware report derived from the multidimensional enrichment phase followed by segmented trajectory analysis. The two reports are presented in randomized order to four evaluator models: *GPT 5.2*, *Gemini 3.1 pro*, *DeepSeek 3.2*, and *Claude Opus 4.6*. Each model rates the reports on a five-point Likert scale (1 = lowest, 5 = highest) according to the following criteria:

- •
Trajectory Coverage: how well the report captures the main phases of the user’s history, rather than focusing on a limited portion of the timeline.
- •
Temporal Coherence: how clearly the report describes changes over time and preserves a consistent chronology.
- •
Sensitivity to Change Points: how effectively the report identifies and explains key transitions.
- •
Segment-Level Specificity: the extent to which the report includes concrete, phase-specific details rather than generic statements.
- •
Overall Preference: the overall usefulness and coherence of the report.

**Table 2: Aggregated LLM-as-a-judge scores (1–5) comparing baseline (Base) and trajectory-based (Traj) report generation across different evaluator models and datasets. Higher values indicate better report quality.**
| Criterion | eRisk 2018 (Reddit) | Mental Health Social Media (Twitter) |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GPT | Gemini | DeepSeek | Claude | GPT | Gemini | DeepSeek | Claude |  |  |  |  |  |  |  |  |  |
|  | Base | Traj | Base | Traj | Base | Traj | Base | Traj | Base | Traj | Base | Traj | Base | Traj | Base | Traj |
| Trajectory<br>coverage | 1.5 | 4.9 | 2.6 | 4.9 | 2.6 | 4.3 | 2.1 | 4.8 | 2.5 | 3.9 | 2.7 | 3.8 | 1.7 | 3.3 | 2.4 | 3.9 |
| Temporal<br>coherence | 2.3 | 4.6 | 3.0 | 4.5 | 3.0 | 4.3 | 2.3 | 4.8 | 2.8 | 3.4 | 3.0 | 3.5 | 2.0 | 3.6 | 2.7 | 3.6 |
| Sensitivity to<br>change points | 1.3 | 4.2 | 1.9 | 4.2 | 2.4 | 4.0 | 1.7 | 4.2 | 1.9 | 3.2 | 2.2 | 3.3 | 1.3 | 2.9 | 2.1 | 3.4 |
| Segment-level<br>specificity | 1.5 | 4.0 | 2.8 | 3.9 | 2.3 | 4.3 | 1.4 | 4.8 | 2.6 | 3.6 | 2.9 | 3.5 | 1.3 | 3.7 | 2.4 | 3.8 |
| Overall<br>preference | 1.5 | 4.1 | 2.5 | 4.0 | 2.5 | 3.9 | 1.9 | 4.8 | 2.6 | 3.5 | 2.6 | 3.4 | 1.8 | 3.6 | 2.4 | 3.6 |

Table [2](#S4.T2) shows and compares the average scores obtained with the four evaluation models. Overall, the GPT-traj approach is systematically preferred over GPT-base, producing reports with more coherent, time-contextualized, and informative narratives.
Trajectory-based reports achieve substantially greater coverage (e.g., 4.9 vs. 1.5 for GPT), capturing the entire depression-related signal evolution; in contrast, base reports often focus on limited portions of the timeline, omitting important phases. Regarding temporal coherence, trajectory-based outputs preserve the order of events and explicitly link observations over time (e.g., 4.6 vs. 2.3 for GPT), whereas base reports often distort the chronology or obscure significant changes.
The greatest improvement concerns sensitivity to change points (e.g., 4.2 vs. 1.3 for GPT): trajectory-based reports explicitly identify change points (e.g., emotional breakdowns, therapeutic changes, triggering events) and provide structured explanations of transitions; in contrast, base reports tend to rely on static and vague descriptions that overlook such discontinuities.
Improvements in segment-level specificity are also observed (e.g., 4.8 vs. 1.4 for Claude), with trajectory-based analyses incorporating more specific and detailed descriptions for each stage.

Similar results are observed on the Mental Health Social Media dataset, where trajectory-based reports consistently outperform the baseline across all evaluation criteria, although with slightly lower absolute scores due to the shorter and less structured nature of the posts.

### 4.4 Ablation study

To assess the contribution of each component of the proposed framework, we evaluate several simplified variants of the full pipeline.
The first variant, NoSeg, does not apply the dynamic segmentation defined in Algorithm [1](#alg1); instead, the entire trajectory is computed and passed to the LLM as a whole, without any explicit phase decomposition. The second variant, FixedWin, replaces adaptive segmentation with a fixed partitioning into equal-length temporal windows (in our case, $K=10$). The third variant, NoSmooth, applies segmentation directly on raw daily scores without smoothing. The fourth variant, NoStats, keeps the segmented structure but removes numerical descriptors such as average severity and trend direction. We also include Base as the direct summarization baseline and Full as the complete framework.
All variants are evaluated using the same *LLM-as-a-judge* protocol described in Section [4.3.2](#S4.SS3.SSS2). The results are reported in Table [3](#S4.T3).

**Table 3: Ablation study on report generation quality, conducted using the eRisk 2018 dataset. Average LLM-as-a-judge scores (1-5). Higher values indicate better performance.**
| Variant | Trajectory<br>Coverage | Temporal<br>Coherence | Sensitivity to<br>Change Points | Segment-Level<br>Specificity | Overall<br>Preference |
| --- | --- | --- | --- | --- | --- |
| Traj (Full) | 4.9 | 4.6 | 4.2 | 4.0 | 4.1 |
| NoSeg | 3.7 | 3.8 | 3.5 | 3.4 | 3.7 |
| FixedWin | 3.8 | 3.6 | 3.8 | 3.5 | 3.7 |
| NoSmooth | 3.9 | 3.7 | 3.7 | 3.7 | 3.6 |
| NoStats | 3.9 | 3.9 | 3.8 | 3.6 | 3.8 |
| Base | 1.5 | 2.3 | 1.3 | 1.5 | 1.5 |

The results show a consistent pattern. First, all trajectory-aware variants outperform the Base configuration, confirming that modeling user history as a temporal signal significantly improves report quality. Removing segmentation (NoSeg) leads to a clear drop in temporal coherence and sensitivity to change points, indicating that explicitly modeling phases is crucial for capturing how mental-health signals evolve over time.
Replacing adaptive segmentation with fixed windows (FixedWin) slightly improves structure but does not align well with real change points, resulting in lower coherence. Similarly, removing smoothing (NoSmooth) introduces more noise in the trajectory, reducing the stability of the identified phases.
Finally, removing segment-level statistics (NoStats) has a smaller but noticeable impact, suggesting that simple numerical descriptors help guide the LLM in producing more precise and structured summaries.
Overall, the best performance is achieved when all components are combined, showing that trajectory construction, smoothing, adaptive segmentation, and structured prompting contribute jointly to generating coherent and interpretable reports.

## 5 Limitations and Future Work

This study demonstrates that modeling mental-health signals as temporal trajectories, enriched with multidimensional classification and topic analysis, provides significant benefits for generating structured and interpretable reports. However, some limitations should be acknowledged, which also point to directions for future work.

First, the current approach relies primarily on English data and models. Although multilingual extensions are feasible (e.g., using mBERT), performance may degrade for underrepresented languages or dialects due to limited training data and cultural differences in the expression of mental states. A second limitation concerns variability in data availability across users. Sparse or irregularly distributed digital traces may lead to incomplete or noisy trajectories, thereby reducing the reliability of temporal modeling and change-point detection. Another limitation involves the use of large language models for report generation. While these models enhance interpretability, they may introduce hallucinations or overgeneralizations, particularly when contextual evidence is limited or ambiguous. Although a RAG component could help mitigate this issue, ensuring factual consistency and traceability remains a critical challenge.
From an ethical perspective, the system is intended as a decision-support tool rather than a diagnostic instrument. Nevertheless, risks related to privacy, data misuse, and misinterpretation of results must be carefully managed, especially when dealing with sensitive personal data.

Future work will address these limitations and explore several directions, including the integration of multimodal data (e.g., images, audio, and wearable signals), the evaluation on clinical datasets, and the incorporation of external contextual information, such as clinical annotations, to improve robustness and interpretability.

## 6 Conclusions

In this paper, we introduced an explainable framework for detecting and analyzing depression-related status shifts in user digital traces. Unlike traditional approaches that focus on isolated posts, the proposed method models mental-health signals as temporal trajectories, capturing how depression-related signals evolve over time. The framework integrates multidimensional BERT-based classification, daily aggregation and smoothing, change-point detection through piecewise linear segmentation, and LLM-based report generation to produce interpretable, phase-oriented summaries.

The experimental evaluation, conducted on Reddit and Twitter datasets, shows that incorporating temporal structure significantly improves the quality of the generated reports. In particular, trajectory-aware summaries provide better coverage of the user’s history, stronger temporal coherence, and higher sensitivity to meaningful change points compared to direct LLM-based summarization. The ablation study further confirms that each component of the pipeline (especially segmentation and temporal modeling) contributes to the overall effectiveness of the system.

Overall, the proposed framework offers a step toward more transparent and longitudinal analysis of mental-health–related signals in digital environments. While not intended for diagnostic purposes, it provides a structured and interpretable representation of behavioral evolution that can support research and assist domain experts in understanding complex temporal patterns.
