## Abstract

Abstract The rapid advancement of Large Vision-Language Models (LVLMs) is increasingly accompanied by unauthorized scraping and training on multimodal web data, posing severe copyright and privacy risks to data owners. Existing countermeasures, such as machine unlearning and watermarks, are inherent post-hoc approaches that act only after intellectual property infringement has already occurred. In this work, we propose MMGuard to empower data owners to proactively protect their multimodal data against unauthorized LVLM fine-tuning. MMGuard generates unlearnable examples by injecting human-imperceptible perturbations that actively exploit the learning dynamics of LVLMs. By minimizing the training loss, the perturbation creates an optimization shortcut, causing the model to overfit to the noise and thereby degrading downstream performance when the perturbation is absent during inference. To further strengthen this defense, MMGuard introduces a cross-modal binding disruption, strategically shifting LVLM attention to enforce a spurious correlation between the noise and the training target with theoretical guarantees. Enhanced by an ensemble learning strategy for cross-model transferability, MMGuard is evaluated against nine open-source LVLMs across six datasets. Our comprehensive results demonstrate effective, stealthy, and robust protection under white-box, gray-box, and black-box threat models, establishing a mechanistic advantage in proactively defending against aggressive fine-tuning exploitation. Our code is available at GitHub: https://github.com/ChengshuaiZhao0/MMGuard .

## 1 Introduction

Large Vision-Language Models (LVLMs)  have rapidly become a central component of modern artificial intelligence systems. By aligning visual content with natural-language instructions and responses, LVLMs support a wide range of multimodal tasks, including visual question answering , image captioning , document understanding , and multimodal instruction following . This success is largely fueled by an ever-growing appetite for image-text data: contemporary LVLMs are pretrained and fine-tuned on hundreds of millions of multimodal pairs scraped indiscriminately from the open web , which has created an urgent threat for the original owners of multimodal content. OpenAI was reported to have transcribed and ingested more than one million hours of YouTube videos without the consent of creators or the platform to obtain multimodal training material for GPT-4, with similar practices reported at Google and Meta and a class action subsequently filed by content creators . The resulting harms are concrete: copyright infringement, leakage of personal and commercial information embedded in images and captions, and the loss of control over how one’s creative work is repurposed by downstream models.

Figure: Figure 1: Protect multimodal data from unauthorized fine-tuning of LVLM.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x1.png

Existing protection mechanisms are insufficient for this setting. Legal takedowns, opt-out policies, watermarking , and model-output detection can help identify or respond to misuse, but they are inherently post-hoc: they act only after the data has already been scraped or after LVLMs have already been trained. Model-level remedies such as machine unlearning  require cooperation from the model provider and are difficult to supervise externally. Recent efforts have attempted to generate *unlearnable examples*  for unauthorized model training by injecting bounded, imperceptible perturbations. By minimizing the training loss, the perturbation creates an optimization shortcut, causing the model to overfit to the noise and thereby degrading downstream performance when the perturbation is absent during inference. Despite their promise, most methods are designed for single modality (e.g., image or audio) or specific tasks (e.g., classification, verification, or retrieval), while unauthorized LVLM fine-tuning presents a substantially different attack surface.

Protecting multimodal data against LVLM fine-tuning introduces several key challenges. First, image-text samples contain redundant sources of supervision. Protecting only one modality leaves enough residual information in other modalities for the model to learn generalizable knowledge . Second, LVLM fine-tuning optimizes an autoregressive generative objective, in which the model captures information from each part of the input. Moreover, LVLMs have prior knowledge of image-text-response semantics from pretraining. LVLMs can still rely on the same effective associations to exploit the protected data that undermines the protection. Third, the defender usually does not know the attacker’s exact LVLM architecture, data preprocessing pipeline, and fine-tuning recipe. The protection must consider practical black-box settings. Finally, multimodal data protection is a multi-objective constrained optimization problem; each perturbation has distinct constraints and optimization dynamics, and the attacker and defender have different objectives, with the overall objective a complex balance.

Our key insight is that effective multimodal data protection should not merely perturb all modalities. *It should also shape how LVLMs capture visual evidence, textual context, and generate target responses during fine-tuning.* If the protected samples encourage the LVLMs to rely on a non-semantic spurious association between the defense signal (i.e., perturbation) and the response, then standard fine-tuning can achieve low loss on the protected training data while learning behavior that does not transfer to clean evaluation data.

In this paper, we propose MMGuard, a proactive data-centeric framework that protects multimodal image-text data from unauthorized LVLM fine-tuning. Given a clean image-text-response sample, MMGuard releases a perturbed image together with a text input containing a short inserted trigger, while keeping the human-facing target response unchanged. The framework comprises five key designs: (*i*) It constructs an image-unlearnable perturbation via projected gradient descent (PGD)  over a differentiable approximation of the LVLM pipeline. (*ii*) It formulates text unlearnable protection through gradient-guided search under discrete readability constraints, guaranteed with a smoothness-based update optimality. (*iii*) It introduces *cross-modal binding disruption*, which steers the model away from genuine semantic bindings and toward protection-specific multimodal shortcuts with theoretical guarantees. (*iv*) It optimizes the perturbation over an ensemble of surrogate LVLMs to improve transferability across unknown LVLM models and processing pipelines. (*v*) It leverages a multi-objective alternative optimization strategy to balance the effectiveness, stealthiness, and robustness of the protection.

We conduct comprehensive experiments across six publicly available multimodal datasets and nine open-source LVLMs. Our evaluation covers white-box, gray-box, and black-box threat models. The results show that MMGuard consistently degrades downstream performance of LVLMs fine-tuned on protected data while preserving the perceptual quality of the released samples. We further evaluate robustness against aggressive LVLM attackers, including diverse data preprocessing, LVLM fine-tuning strategies, and data mixing with clean public samples. Moreover, analyses of each component, hyperparameters, and attention behavior confirm that MMGuard provides a mechanistic advantage in protecting against unauthorized fine-tuning of LVLM.

Our study makes the following contributions:

- •
We identify and formalize unauthorized LVLM fine-tuning on scraped multimodal data as a data protection problem, considering a practical threat model where defenders can only modify their own public image-text data before release.
- •
To the best of our knowledge, MMGuard is the *first data-centric protection framework* that proactively defends multimodal data against unauthorized LVLM fine-tuning. It perturbs both the image and text as a coupled multimodal protection tailored to the LVLM autoregressive objective.
- •
We introduce cross-modal binding disruption, a mechanism that shifts LVLM learning dynamics toward planted perturbations and enforces spurious associations between protection and target responses. We further provide a theoretical analysis explaining why this mechanism degrades generalization on clean downstream tasks.
- •
We design an ensemble-based perturbation optimization strategy to improve transferability across unknown LVLM attackers, enabling effective and robust protection under white-box, gray-box, and black-box scenarios.
- •
We evaluate MMGuard across six datasets and nine open-source LVLMs, demonstrating its effectiveness, transferability, and practicality. Further analysis confirms its robustness against adaptive attacks and provides mechanistic advantages for disrupting unauthorized LVLM fine-tuning.

## 2 Related Work

Defenses Against Generative Models. Existing defenses against unauthorized generative models fall into model-centric and data-centric approaches. Model-side methods include AI-generated content detection  and watermarking for images  and language outputs , machine unlearning , and membership inference for post-hoc auditing ; all are inherently *reactive*, acting only after data have been scraped or models trained, and typically require cooperation from the model provider. Data-side methods instead proactively modify data before release. They add perturbations to counteract a wide range of threats across modalities such as style mimicry , voice cloning , and personalized fine-tuning . While these methods raise the cost of misuse, they often degrade perceptual quality or remain fragile under purification and adversarial training .

Unlearnable Example Methods. Unlearnable examples (UEs) offer a proactive, data-centric alternative by injecting bounded perturbations that act as “shortcuts,” causing models trained on protected data to fail on clean test data. Pioneered for image classification via error-minimizing noise , UEs have been refined for robustness against adversarial training , transferability across architectures , and label-agnostic settings via CLIP surrogates , and broadened to text , graphs , audio/speech , image segmentation , and diffusion-based generation . For multimodal contrastive pretraining, MEM  extends error-minimizing noise to image-caption pairs to mislead CLIP-style models. Recent studies caution that UEs can be partially circumvented through relearning , pretrained backbones , or diffusion-based purification , motivating the need for stronger designs. Despite this progress, no prior work targets unauthorized fine-tuning of LVLMs, which differs fundamentally in objective, input space, and learning dynamics. MMGuard fills this gap with the first proactive, data-centric protection tailored to LVLM fine-tuning.

## 3 Background and Preliminaries

This section presents the notation, problem formulation, and technical background used throughout the paper. We first formulate multimodal data protection as an optimization problem over image-text datasets in Sec. [3.1](#S3.SS1). We then review the standard formulation of unlearnable examples in Sec. [3.2](#S3.SS2), which serves as the foundation for our defense. Finally, we introduce the common architecture and fine-tuning paradigm of Large Vision-Language Models in Sec. [3.3](#S3.SS3), highlighting the mechanism that motivates our design.

### 3.1 Problem Formulation

Let $\mathcal{X}$ denote the image space and $\mathcal{T}$ denote natural-language space. A multimodal dataset is given by

$$ $\mathcal{D}=\{(x_{i},t_{i},y_{i})\}_{i=1}^{n},$ (1) $$

where $x_{i}\in\mathcal{X}$ is an image, $t_{i}\in\mathcal{T}$ is a textual input (e.g., question or instruction), and $y_{i}\in\mathcal{Y}$ is a target output (e.g., answer or response).

A large vision language model is denoted by

$$ $f_{\theta}:\mathcal{X}\times\mathcal{T}\rightarrow\mathcal{Y},$ (2) $$

where $\theta=(\theta_{\mathrm{frz}},\theta_{\mathrm{tr}})$ denotes the model parameters, consisting of frozen pretrained parameters $\theta_{\mathrm{frz}}$ from the base LVLM checkpoint and trainable parameters $\theta_{\mathrm{tr}}$ that the attacker may select for fine-tuning. Given a multimodal training set $\mathcal{D}$, an unauthorized attacker seeks to obtain:

$$ $\theta^{\star}(\mathcal{D})\in\arg\min_{\theta\in\Theta}\mathcal{L}_{\mathrm{train}}(\theta;\mathcal{D}),$ (3) $$

where $\mathcal{L}_{\mathrm{train}}$ is the empirical training objective. For simplicity, we use ’fine-tune’ and ’train’ interchangeably in the paper.

The goal of multimodal data protection is to construct a protected dataset

$$ $\widetilde{\mathcal{D}}_{\phi}=\mathcal{G}_{\phi}(\mathcal{D})=\{(\widetilde{x}_{i},\widetilde{t}_{i},y_{i})\}_{i=1}^{n},$ (4) $$

where $\mathcal{G}_{\phi}$ is a protection map parameterized by $\phi$. The protection must remain close to each original sample under modality-specific perceptual and semantic budgets:

$$ $\mathcal{B}(\mathcal{D})=\Bigl\{\widetilde{\mathcal{D}}_{\phi}:\;d_{x}(\widetilde{x}_{i},x_{i})<\epsilon_{x},d_{t}(\widetilde{t}_{i},t_{i})<\epsilon_{t},\;\forall i\in[n]\Bigr\},$ (5) $$

where $d_{x}$ and $d_{t}$ measure visual and textual change, respectively, with budgets $\epsilon_{x}$ and $\epsilon_{t}$.

The defender aims to ensure that models trained on $\widetilde{\mathcal{D}}$ perform poorly on clean downstream data. Let $\mathcal{D}_{\mathrm{eval}}$ be a clean downstream evaluation dataset, and let $\mathcal{L}_{\mathrm{eval}}$ measure the corresponding evaluation loss. We formalize the defender’s objective as the following bilevel optimization problem:

$$ $\displaystyle\max_{\phi\in\Phi}$ $\displaystyle\mathcal{L}_{\mathrm{eval}}\bigl(\theta^{\star}(\widetilde{\mathcal{D}}_{\phi});\mathcal{D}_{\mathrm{eval}}\bigr)$ (6) $\displaystyle\mathrm{s.t.}$ $\displaystyle\widetilde{\mathcal{D}}_{\phi}=\mathcal{G}_{\phi}(\mathcal{D}),\quad\widetilde{\mathcal{D}}_{\phi}\in\mathcal{B}(\mathcal{D}),$ $\displaystyle\theta^{\star}(\widetilde{\mathcal{D}}_{\phi})\in\arg\min_{\theta\in\Theta}\mathcal{L}_{\mathrm{train}}(\theta;\widetilde{\mathcal{D}}_{\phi}).$ $$

Equivalently, the defender seeks a small, human-imperceptible transformation of the dataset that degrades the utility of models fine-tuned by an unauthorized trainer.

### 3.2 Unlearnable Examples

Unlearnable examples are training samples designed to remain semantically useful to human users while inhibiting unauthorized models from learning meaningful representations from them . In contrast to conventional adversarial examples, which perturb inputs to cause mistakes at inference time, unlearnable examples intervene prior to the training stage by modifying the data. Their core mechanism is to introduce a subtle, hard-to-detect perturbation that induces a spurious shortcut the model can exploit to reduce the training loss, thereby discouraging it from learning the genuine input-output relationship. Consequently, models trained on such data fit the protected samples but generalize poorly to clean inputs.

The standard unlearnable-example objective is often written as a bilevel *min-min* problem. Let $\delta=\{\delta_{i}\}_{i=1}^{n}$ denote bounded perturbations applied to the original data before release, and let $\mathcal{D}_{\delta}=\mathcal{G}_{\delta}(\mathcal{D})$ be the protected dataset. For instance, $\mathcal{D}_{\delta}=\{(x_{i}+\delta_{i},t_{i},y_{i})\}_{i=1}^{n}$ in the common image-only case. The defender constructs $\delta$ by solving:

$$ $\displaystyle\min_{\delta}$ $\displaystyle\mathcal{L}_{\mathrm{train}}\bigl(\theta^{\star}(\mathcal{D}_{\delta});\mathcal{D}_{\delta}\bigr)$ (7) $\displaystyle\mathrm{s.t.}$ $\displaystyle\mathcal{D}_{\delta}=\mathcal{G}_{\delta}(\mathcal{D}),\quad\mathcal{D}_{\delta}\in\mathcal{B}(\mathcal{D}),$ $\displaystyle\theta^{\star}(\mathcal{D}_{\delta})\in\arg\min_{\theta\in\Theta}\mathcal{L}_{\mathrm{train}}(\theta;\mathcal{D}_{\delta}).$ $$

The inner minimization describes the attacker’s training process: given the released protected data, the attacker optimizes the model parameters to fit that data. The outer minimization describes how the defender chooses the perturbation to ensure that the training process overfits the protected distribution.

### 3.3 Large Vision-Language Models

Modern LVLMs extend pretrained language models with a visual front end that converts images into tokens compatible with the language-model embedding space. Given an image-text input $(x_{i},t_{i})$, the image is first standardized (e.g., resizing and pixel quantization) by an image processor $g_{x}$, split into image patches by a patching function $\pi$, and encoded into visual features $H_{i}^{x}$ by a visual encoder $E_{x}$:

$$ $H_{i}^{x}=E_{x}(\pi(g_{x}(x_{i}))),$ (8) $$

The visual features are then aligned with the language-model hidden space through a modality projector $P_{x}$:

$$ $Z_{i}^{x}=P_{x}(H_{i}^{x}).$ (9) $$

In parallel, the textual input is tokenized by $\operatorname{Tok}$ and mapped to embeddings by $E_{t}$.

$$ $Z_{i}^{t}=E_{t}(\operatorname{Tok}(t_{i}))$ (10) $$

The visual and textual tokens are then interleaved into a joint multimodal sequence $S_{i}$ according to a predefined template:

$$ $S_{i}=\operatorname{Template}(Z_{i}^{x},Z_{i}^{t}).$ (11) $$

which will be processed by the downstream language model.

During supervised fine-tuning, LVLM is optimized by minimizing the autoregressive negative log-likelihood of the target response conditioned on the image and textual input:

$$ $\mathcal{L}_{\mathrm{train}}(\theta;\mathcal{D})=\frac{1}{n}\sum_{i=1}^{n}\sum_{j=1}^{|y_{i}|}-\log p_{\theta}\bigl(y_{i,j}\mid y_{i,<j},x_{i},t_{i}\bigr).$ (12) $$

## 4 Threat Model

We consider two parties: an *attacker*, who collects multimodal web data to fine-tune LVLMs, and a *defender*, who owns image-text data and wishes to publish it online while preventing its unauthorized use as effective fine-tuning data.

Attacker Objectives.
The attacker starts from an existing pretrained LVLM checkpoint and fine-tunes it on multimodal data scraped from the web. Their objective is to maximize the model utility of downstream task performance while disregarding potential copyright infringement and privacy risks.

Attacker Capabilities.
The attacker possesses the knowledge and expertise required to optimize LVLMs using a broad range of fine-tuning techniques and to evaluate their performance. In our study, we consider two types of attackers: (*i*) a naive attacker who follows standard fine-tuning pipelines, such as LoRA, and (*ii*) an adaptive attacker who is aware of MMGuard and seeks to circumvent its protection. The adaptive attacker may employ diverse strategies, including data transformations and data mixing. We discuss these strategies and our robustness evaluation in detail in Sec. [6.5](#S6.SS5).

Defense Objectives.
The defender may be an individual artist, photographer, news outlet, private dataset curator, or any data owner who seeks to prevent their content from being effectively exploited by LVLMs. The defender has two primary objectives. First, protected samples should disrupt unauthorized fine-tuning: when incorporated into the attacker’s training set, they should degrade the model’s downstream performance. Second, the protected content should preserve high perceptual and semantic fidelity, ensuring that it remains useful for legitimate online publication and human consumption.

Defense Assumptions.
The defender has full access to their own image-text pairs before publication and can modify them by adding imperceptible protective perturbations. However, the defender has no access to the attacker’s full training corpus, fine-tuning recipe, hyperparameters, or model weights, and cannot directly interfere with the attacker’s training process. We consider three levels of defender knowledge about the attacker’s target model. In the *white-box* setting, the defender knows the exact LVLM that the attacker will fine-tune and optimizes the protection directly against it. This setting provides an upper bound on defense effectiveness, although it is rarely realistic in practice. In the *gray-box* setting, the defender knows only partial information, such as the model family or architecture, but not the exact checkpoint. In the *black-box* setting, the defender has no concrete knowledge of the attacker’s model and must rely on architecture-agnostic protection and cross-family transferability. This setting is the most realistic and challenging scenario.

## 5 Our Approach MMGuard

This section presents MMGuard, our proposed framework for proactive multimodal data protection against unauthorized LVLM fine-tuning. We first construct a continuous image perturbation in Sec. [5.1](#S5.SS1) and a discrete textual trigger in Sec. [5.2](#S5.SS2) to form a coupled multimodal protection that eliminates residual learnable signal in either modality. We then introduce cross-modal binding disruption in Sec. [5.3](#S5.SS3) to redirect LVLM learning toward protection-specific shortcuts, preventing the autoregressive objective from bypassing the protection through pretrained knowledge. We further optimize the protection over an ensemble of surrogate LVLMs in Sec. [5.4](#S5.SS4) to improve the transferability of protection to black box scenarios. We finally cast the framework as a constrained multi-objective bilevel optimization in Sec. [5.6](#S5.SS6) to reconcile heterogeneous continuous and discrete variables under modality-specific budgets and the asymmetric objectives of defender and attacker.

Figure: Figure 2: Overview of MMGuard. The defender generates protected multimodal examples by coupling image-unlearnable perturbations with text-unlearnable triggers and using cross-modal binding disruption to steer LVLM attention toward protection-specific shortcuts that fail to transfer to the downstream task.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x2.png

### 5.1 Unlearnable Image Protection

We formulate the unlearnable image protection as two key components: a differentiable image processor that allows gradient flow from the surrogate LVLM back to pixel-space perturbations, and projected gradient descent optimization that updates perturbations to protect the image.

Differentiable Image Processor. As described in the Sec. [3.3](#S3.SS3), the image processor $g_{x}$ contains non-differentiable operations that create an obstacle for directly optimizing the image perturbation $\delta_{i}$ in the raw pixel space. This obstacle undermines the defender’s ability to create effective, stealthy, unlearnable examples. To address it, we construct a differentiable surrogate processor $\tilde{g}_{x}$ that approximates the practical processor $g_{x}$: $\tilde{g}_{x}\approx g_{x}$. Specifically, we handle the non-differentiable functions by three strategies. (*i*) For discretization steps (e.g., pixel quantization), we keep the forward value used by the practical processor but copy gradients through a continuous proxy. (*ii*) For geometric transformations (e.g., resizing), we use differentiable substitutes that approximate the same transformation during backpropagation. (*iii*) For layout decisions (e.g., resolution selection), we follow the same deterministic rules as $g_{x}$ and treat the resulting layout as fixed metadata for the current image. Operations that are already differentiable are kept unchanged. Together, these choices give the defender a differentiable path from the LVLM loss back to pixel-space perturbations while keeping the forward representation close to that in the surrogate model.

Projected Gradient Descent Optimization.
For each clean image $x_{i}$, we construct the protected image $\tilde{x}_{i}$ by adding a human-imperceptible perturbation $\delta_{i}$: $\tilde{x}_{i}=x_{i}+\delta_{i}$, where $\delta_{i}$ is constrained to lie in the image-side feasible set $\mathcal{C}^{\delta}_{i}$:

$$ $\mathcal{C}^{\delta}_{i}=\{\delta:\;x_{i}+\delta\in\mathcal{X},\ \|\delta\|_{\infty}\leq\epsilon_{x}\}.$ (13) $$

Formally, the unlearnable image protection solves the following bilevel optimization problem:

$$ $\displaystyle\min_{\{\delta_{i}\}}$ $\displaystyle\quad\frac{1}{n}\sum_{i=1}^{n}\ell_{\mathrm{prot}}\bigl(\tilde{f}_{\tilde{\theta}_{x}^{\star}};\tilde{x}_{i},t_{i},y_{i}\bigr)$ $\displaystyle\mathrm{s.t.}$ $\displaystyle\quad\tilde{x}_{i}=x_{i}+\delta_{i},\quad\delta_{i}\in\mathcal{C}^{\delta}_{i},$ $\displaystyle\quad\tilde{\theta}_{x}^{\star}\in\arg\min_{\tilde{\theta}}\sum_{i=1}^{n}\ell_{\mathrm{train}}\bigl(\tilde{f}_{\tilde{\theta}};\tilde{x}_{i},t_{i},y_{i}\bigr).$ (14) $$

where $\ell_{\mathrm{prot}}$ is the defender’s protection objective (e.g., the LVLM training loss), later defined in Eq. ([30](#S5.E30)), and $\ell_{\mathrm{train}}$ denotes the sample-wise LVLM training loss.

We optimize $\delta_{i}$ by projected gradient descent:

$$ $\delta_{i}\leftarrow\Pi_{\mathcal{C}^{\delta}_{i}}\!\left(\delta_{i}-\alpha_{x}\,\mathrm{sign}\bigl(\nabla_{\delta_{i}}\ell_{\mathrm{prot}}\bigr)\right).$ (15) $$

where $\Pi_{\mathcal{C}^{\delta}_{i}}$ denotes projection back onto the per-image feasible set, and $\alpha_{x}$ is the step size. The gradient is taken through the differentiable processor $\tilde{g}_{x}$, the visual encoder $E_{x}$, the projector $P_{x}$, and the language model.

### 5.2 Unlearnable Text Protection

Given a clean text input $t_{i}$, we construct the protected text $\tilde{t}_{i}$ by inserting a bounded-length trigger $\gamma_{i}=(\gamma_{i,1},\ldots,\gamma_{i,|\gamma_{i}|})$: $\tilde{t}_{i}=\operatorname{Insert}(t_{i},\gamma_{i})$, where insertion is restricted to textual input positions and does not replace tokens in $t_{i}$. Each trigger token is selected from an admissible vocabulary $\mathcal{V}_{\mathrm{adm}}\subseteq\mathcal{V}$ (e.g., excluding special tokens, control tokens, and non-linguistic tokens). The trigger length is bounded by the text budget length $\epsilon_{t}$, so the text-side feasible set for $\gamma_{i}$ is:

$$ $\mathcal{C}^{\gamma}_{i}=\{\gamma:\;|\gamma|\leq\epsilon_{t},\ \gamma_{j}\in\mathcal{V}_{\mathrm{adm}}\}.$ (16) $$

Analogous to image-side protection, the unlearnable text protection solves a discrete min-min problem:

$$ $\displaystyle\min_{\{\gamma_{i}\}}$ $\displaystyle\quad\frac{1}{n}\sum_{i=1}^{n}\ell_{\mathrm{prot}}\bigl(\tilde{f}_{\tilde{\theta}_{t}^{\star}};x_{i},\tilde{t}_{i},y_{i}\bigr)$ $\displaystyle\mathrm{s.t.}$ $\displaystyle\quad\tilde{t}_{i}=\operatorname{Insert}(t_{i},\gamma_{i}),\quad\gamma_{i}\in\mathcal{C}^{\gamma}_{i},$ $\displaystyle\quad\tilde{\theta}_{t}^{\star}\in\arg\min_{\tilde{\theta}}\sum_{i=1}^{n}\ell_{\mathrm{train}}\bigl(\tilde{f}_{\tilde{\theta}};x_{i},\tilde{t}_{i},y_{i}\bigr).$ (17) $$

Thus, insertion defines where the protected text differs from the clean input, while the discrete optimization below changes only the identities of tokens inside the inserted trigger.

Gradient-Based Candidate Insertion.
We approximate the outer update in Eq. ([17](#S5.E17)) with a HotFlip-style  first-order search over the inserted trigger tokens. Let $E_{t}$ map vocabulary tokens to embeddings and denote the embedding of token $v$ by $e_{v}=E_{t}(v)$. For a trigger position $j$, let $\gamma_{i}^{[j=v]}$ denote the trigger obtained by setting that inserted position to candidate token $v$. A first-order expansion of the protection loss gives

$$ $\ell_{\mathrm{prot}}\!\bigl(\gamma_{i}^{[j=v]}\bigr)\approx\ell_{\mathrm{prot}}(\gamma_{i})+\bigl(e_{v}-e_{\gamma_{i,j}}\bigr)^{\top}\nabla_{e_{\gamma_{i,j}}}\ell_{\mathrm{prot}}.$ (18) $$

The corresponding linear score is

$$ $s_{i,j}(v)=\bigl(e_{v}-e_{\gamma_{i,j}}\bigr)^{\top}\nabla_{e_{\gamma_{i,j}}}\ell_{\mathrm{prot}}.$ (19) $$

Candidate Verification.
The token with the best first-order score may still be suboptimal after deployment because the HotFlip score is computed in the token-embedding space, whereas the released trigger is ultimately a surface string processed by the LVLM tokenizer. Under byte-pair encoding (BPE) tokenization, decoding a candidate token and inserting it into the text can change neighboring token boundaries through merge or split operations, so the actual re-tokenized sequence may differ from the assumed single-position substitution. To account for this tokenization mismatch, we verify the shortlisted candidates and computing the exact protection loss:

$$ $v_{i,j}^{\star}=\arg\min_{v\in\mathcal{V}^{\mathrm{cand}}_{i,j}}\ell_{\mathrm{prot}}\!\bigl(\gamma_{i}^{[j=v]}\bigr).$ (20) $$

This screen-and-verify procedure is provably near-optimal: under standard smoothness, the selected token matches the best admissible substitution up to a small quadratic remainder, which is justified by Lemma [B.1](#A2.Thmtheorem1) in Appendix [B](#A2).

### 5.3 Cross-Modal Binding Disruption

Image and text unlearnable protections discourage the LVLM from learning robust features from either modality, but they do not guarantee a genuine unlearnable shortcut for fine-tuning. In LVLMs, answer tokens are generated by decoder attention over the full multimodal context. Even when both the image and the text are protected, this attention mechanism can still route generation through semantically meaningful evidence that remains available in either modality. Moreover, LVLMs have prior knowledge of image-text-response semantics from pretraining. LVLMs can rely on the effective associations to exploit the protected data that undermines the protection. Therefore, potent protection should not be limited to surface-level perturbation for both modalities; it also shapes how LVLMs capture visual evidence and textual context, and how they generate target responses during fine-tuning. To enable this, MMGuard introduces cross-modal binding disruption. By disrupting the normal semantic binding and steering optimization toward protection-specific spurious attention paths, the method encourages fine-tuning to rely on non-transferable perturbation shortcuts.

Attention Mass Distribution.
For a protected sample $(\tilde{x}_{i},\tilde{t}_{i},y_{i})$, the LVLM template assembles a joint token sequence in the language model backbone:

$$ $\Omega_{i}=\Omega_{x,i}\uplus\Omega_{t,i}\uplus\Omega_{\gamma,i}\uplus\Omega_{y,i},$ (21) $$

where $\Omega_{x,i}$, $\Omega_{t,i}$, $\Omega_{\gamma,i}$, and $\Omega_{y,i}$ denote the image tokens, original text tokens, inserted trigger tokens, and answer tokens, respectively. For simplicity, we omit template auxiliary tokens.

Among the image tokens, we further identify the subset most affected by the perturbation. Let $\mathcal{P}_{i,b}$ denote the preprocessed pixel region that corresponds to image token $b\in\Omega_{x,i}$, and define its average perturbation magnitude as

$$ $\rho_{i,b}=\frac{1}{|\mathcal{P}_{i,b}|}\sum_{u\in\mathcal{P}_{i,b}}|\delta_{i}(u)|.$ (22) $$

We select perturbation tokens $\Omega_{\delta,i}$ that are most affected in the image tokens based on a ratio $\tau_{\delta}\in(0,1]$:

$$ $\Omega_{\delta,i}=\left\{b\in\Omega_{x,i}:\;\operatorname{rank}_{\Omega_{x,i}}(\rho_{i,b})\leq\left\lceil\tau_{\delta}|\Omega_{x,i}|\right\rceil\right\},$ (23) $$

where $\operatorname{rank}_{\Omega_{x,i}}(\cdot)$ orders perturbation scores in descending order, with ties broken deterministically.

Let $A^{(k,h)}\!\in\![0,1]^{|\Omega|\times|\Omega|}$ denote the attention matrix of head $h$ in layer $k$, where $h\in\mathcal{H}$ and each row is a distribution over key tokens. For a source token set $\mathcal{S}\subseteq\Omega$, we define its head-averaged attention-mass distribution as

$$ $\bar{A}_{k}(\mathcal{S})=\frac{1}{|\mathcal{H}|\,|\mathcal{S}|}\sum_{h\in\mathcal{H}}\sum_{a\in\mathcal{S}}A^{(k,h)}_{a,:}\in\Delta^{|\Omega|-1}.$ (24) $$

This distribution summarizes how much attention mass the queries in $\mathcal{S}$ collectively assign to every key token. For a nonempty target token set $\mathcal{R}\subseteq\Omega$, define the uniform distribution on $\mathcal{R}$, embedded in the simplex over $\Omega$, as

$$ $U_{\mathcal{R}}=\frac{1}{|\mathcal{R}|}\mathbf{1}_{\mathcal{R}}\in\Delta^{|\Omega|-1},\qquad\mathbf{1}_{\mathcal{R}}(b)=\begin{cases}1,&b\in\mathcal{R},\\ 0,&b\notin\mathcal{R}.\end{cases}$ (25) $$

That is, $U_{\mathcal{R}}$ assigns equal probability to tokens in the target set and zero probability to all other tokens in the full sequence. We then measure how far the source attention mass is from this reference distribution, averaged over a chosen layer set $\mathcal{K}$:

$$ $\ell_{\mathrm{mass}}(\mathcal{S},\mathcal{R})=\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}\mathrm{D}_{\mathrm{KL}}\!\bigl(U_{\mathcal{R}}\,\|\,\bar{A}_{k}(\mathcal{S})\bigr).$ (26) $$

Equivalently, $\ell_{\mathrm{mass}}$ is a layer-averaged contrastive loss that raises the attention logits of tokens in $\mathcal{R}$ and suppresses those of other tokens, as justified by Proposition [C.1](#A3.Thmtheorem1) in Appendix [C](#A3).

Variant 1: Bridge Path Hijack (BPH).
Based on the attention-mass loss, we can design various cross-modal binding objectives that create different protection-specific attention shortcuts. Specifically, we consider three distinct paths:

- •
*Trigger-to-perturbation binding* $\ell_{\mathrm{mass}}(\Omega_{\gamma},\Omega_{\delta})$: it ties the text trigger to the perturbation, so that the image and text protections are coupled across modalities.
- •
*Answer-to-trigger shortcut* $\ell_{\mathrm{mass}}(\Omega_{y},\Omega_{\gamma})$: it creates a direct attention path from answer tokens to the inserted trigger, so that the answer loss can be reduced by attending to the trigger rather than to the original semantic evidence.
- •
*Answer-to-perturbation shortcut* $\ell_{\mathrm{mass}}(\Omega_{y},\Omega_{\delta})$: it preserves a spurious attention shortcut from answer tokens to perturbation that degrades clean downstream generalization.

We combine these three paths to form the BPH loss:

$$ $\displaystyle\ell_{\mathrm{bind}}^{\mathrm{BPH}}$ $\displaystyle=\beta_{1}\,\ell_{\mathrm{mass}}(\Omega_{\gamma},\Omega_{\delta})+\beta_{2}\,\ell_{\mathrm{mass}}(\Omega_{y},\Omega_{\gamma})$ $\displaystyle\quad+\beta_{3}\,\ell_{\mathrm{mass}}(\Omega_{y},\Omega_{\delta}),$ (27) $$

where $\beta_{1},\beta_{2},\beta_{3}\geq 0$ are hyperparameters that control the relative importance of the three paths.

###### Theorem 5.1 (Effectiveness of BPH) .

Let $\mathcal{R}_{i}\in\{\Omega_{\gamma,i},\,\Omega_{\delta,i}\}$ be a protection-induced target set that reduces to the empty set under the clean evaluation input $(x_{i},t_{i},y_{i})$. Suppose a model $\theta^{\star}$ achieves answer-token binding $\ell_{\mathrm{mass}}(\Omega_{y,i},\mathcal{R}_{i})\leq\eta$ on the protected sample $(\tilde{x}_{i},\tilde{t}_{i},y_{i})$. Denote by $\bar{A}^{\mathrm{p}}_{k,i}(\Omega_{y})$ and $\bar{A}^{\mathrm{c}}_{k,i}(\Omega_{y})$ the head-averaged answer-token attention-mass distributions of $\theta^{\star}$ on the protected and clean inputs at layer $k$, both extended to a common token universe by zero-padding the positions in $\mathcal{R}_{i}$ on the clean side. Then

$$ $\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}\mathrm{TV}\!\bigl(\bar{A}^{\mathrm{p}}_{k,i}(\Omega_{y}),\,\bar{A}^{\mathrm{c}}_{k,i}(\Omega_{y})\bigr)\;\geq\;1-\sqrt{\eta/2}.$ (28) $$

The proof is given in Appendix [D](#A4).

Theorem [5.1](#S5.Thmtheorem1) shows that every protection-induced binding term in Eq. ([27](#S5.E27)) that is driven to a small $\eta$ enforces an attention-pattern shift of at least $1-\sqrt{\eta/2}$ in mean total-variation distance between protected training and clean evaluation, providing the mechanistic effectiveness by which BPH’s protection signal fails to transfer to clean downstream data. The complementary trigger-to-perturbation term ensures that the trigger and perturbation pathways are coupled: severing either at evaluation collapses the joint route the surrogate adopted during fine-tuning, thus strengthening protection.

Variant 2: Contrastive Routing Shift (CRS).
A fixed bridge is effective when the surrogate and the attacker share a similar fusion mechanism, but its prescriptive form may over-constrain the path and limit transfer to unseen architectures. CRS relaxes the prescription: instead of dictating *where* the answer should attend, it only requires that the answer’s attention-mass pattern *differs* between the clean and protected forward passes. Let $\bar{A}_{k}^{\mathrm{c}}(\Omega_{y})$ and $\bar{A}_{k}^{\mathrm{p}}(\Omega_{y})$ denote the answer-token attention-mass distributions obtained under the clean and protected inputs, respectively. CRS minimizes

$$ $\ell_{\mathrm{bind}}^{\mathrm{CRS}}=-\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}\mathrm{D}_{\mathrm{KL}}\!\bigl(\mathrm{sg}(\bar{A}_{k}^{\mathrm{c}}(\Omega_{y}))\,\|\,\bar{A}_{k}^{\mathrm{p}}(\Omega_{y})\bigr),$ (29) $$

where the stop-gradient $\mathrm{sg}(\cdot)$ freezes the clean reference so that updates only reshape the protected-side route. CRS does not assume a specific target that the surrogate must attend to, thereby creating another effective protection variant.

Joint Protection Objective. For each cross-modal binding disruption loss, we combine it with the standard training loss to form a joint protection objective:

$$ $\ell_{\mathrm{joint}}=\lambda_{\mathrm{train}}\,\ell_{\mathrm{train}}+\lambda_{\mathrm{bind}}\,\ell_{\mathrm{bind}}.$ (30) $$

The first term retains the standard min-min unlearnable-example signal: the protected sample must remain easy to fit during unauthorized fine-tuning. The second term governs *how* that fitting is achieved, biasing the surrogate toward attention routes that exist only when the trigger is inserted and the image perturbation is present. The two terms are complementary, thus forming effective protection against unauthorized LVLMs.

###### Theorem 5.1 (Effectiveness of BPH) .

### 5.4 Ensemble Protection for Black-box Robustness

The joint objective in Eq. ([30](#S5.E30)) optimizes protection with respect to a single surrogate LVLM, which may provide limited robustness against unseen attackers with different model architectures. To improve black-box transferability, we extend the protection objective to an ensemble of surrogate models:

$$ $\ell_{\mathrm{prot}}=\sum_{m=1}^{M}\omega_{m}\ell_{\mathrm{joint}}^{(m)},\qquad\sum_{m=1}^{M}\omega_{m}=1,$ (31) $$

where $\ell_{\mathrm{train}}^{(m)}$, $\ell_{\mathrm{bind}}^{(m)}$ are the training loss and the binding disruption loss computed on surrogate $m$, and $\omega_{m}\geq 0$ is the weight for surrogate $m$. The ensemble objective encourages protected data to remain unlearnable across multiple surrogates, thereby providing more generalizable protection.

### 5.5 Adversarial Objective as an Unlearnable Variant

Optimizing the protection loss in Eq. ([31](#S5.E31)) encourages unlearnable protection through the standard min-min objective. The same framework also supports an adversarial protection objective $\ell_{\mathrm{prot}}^{\mathrm{AD}}$ by reversing the outer training-loss term while preserving the binding-disruption term:

$$ $\displaystyle\ell_{\mathrm{prot}}^{\mathrm{AD}}$ $\displaystyle=\sum_{m=1}^{M}\omega_{m}\,\ell_{\mathrm{joint}}^{\mathrm{AD}(m)},$ (32) $\displaystyle\ell_{\mathrm{joint}}^{\mathrm{AD}(m)}$ $\displaystyle=-\lambda_{\mathrm{train}}\,\ell_{\mathrm{train}}^{(m)}+\lambda_{\mathrm{bind}}\,\ell_{\mathrm{bind}}^{(m)}.$ $$

This adversarial variant targets training disruption by making protected samples hard to fit, thus directly degrading fine-tuning performance rather than relying on shortcut memorization. The binding term is preserved with the same sign because it shifts attention routing patterns that are shared across LVLMs to amplify the protection effect.

### 5.6 Constrained Multi-objective Optimization

Given a clean multimodal sample $(x_{i},t_{i},y_{i})$, the unlearnable image and text protections create two specialized protection signals, and the cross-modal binding disruption creates a complementary attention-level mechanism guidance. We combine them into a single constrained bilevel optimization over the released dataset. Formally, the protected input is

$$ $\tilde{x}_{i}=x_{i}+\delta_{i},\qquad\tilde{t}_{i}=\operatorname{Insert}(t_{i},\gamma_{i}),$ (33) $$

with $\delta_{i}\in\mathcal{C}^{\delta}_{i}$ and $\gamma_{i}\in\mathcal{C}^{\gamma}_{i}$. Given a surrogate set $\{\tilde{f}^{(m)}\}_{m=1}^{M}$, MMGuard solves

$$ $\displaystyle\min_{\{\delta_{i},\gamma_{i}\}}$ $\displaystyle\quad\frac{1}{n}\sum_{i=1}^{n}\sum_{m=1}^{M}\omega_{m}\ell_{\mathrm{joint}}^{(m)}\bigl(\tilde{f}_{\tilde{\theta}^{(m)\star}};\tilde{x}_{i},\tilde{t}_{i},y_{i}\bigr)$ $\displaystyle\mathrm{s.t.}$ $\displaystyle\quad\delta_{i}\in\mathcal{C}^{\delta}_{i},\quad\gamma_{i}\in\mathcal{C}^{\gamma}_{i},\quad\forall i,\quad\sum_{m=1}^{M}\omega_{m}=1,$ $\displaystyle\quad\tilde{\theta}^{(m)\star}\in\arg\min_{\tilde{\theta}^{(m)}}\sum_{i=1}^{n}\ell_{\mathrm{train}}\bigl(\tilde{f}_{\tilde{\theta}^{(m)}};\tilde{x}_{i},\tilde{t}_{i},y_{i}\bigr),\quad\forall m.$ (34) $$

Here $\ell_{\mathrm{joint}}^{(m)}$ is instantiated by Eq. ([30](#S5.E30)) for the min-min unlearnable objective, or by Eq. ([32](#S5.E32)) for the adversarial variant. This formulation is multi-objective in two senses. First, the defender optimizes both the task-level fitting signal and the attention-level binding signal through $\lambda_{\mathrm{train}}$ and $\lambda_{\mathrm{bind}}$. Second, the ensemble weights $\omega_{m}$ approximate transfer to possible attackers by optimizing the same released perturbations against multiple surrogates. The modality budgets remain hard constraints rather than soft penalties, ensuring that the optimized data stays within the protection set in Eq. ([13](#S5.E13)) and Eq. ([16](#S5.E16)).

Figure: Algorithm 1 MMGuard: Multimodal Data Protection via Constrained Multi-Objective Optimization.

Optimization Algorithm.
Directly solving Eq. ([34](#S5.E34)) is impractical because it would require repeatedly solving the attacker’s inner fine-tuning problem to convergence, while jointly optimizing continuous image perturbations and discrete text triggers under different feasibility constraints. We therefore use an alternating approximation summarized in Algorithm [1](#alg1). At each outer round, the current protected samples are used to approximate the inner solution by a small number of adaptation steps for all surrogate models. The outer loss is then evaluated on the adapted surrogate. Image perturbations are updated by projected gradient descent as in Eq. ([15](#S5.E15)), while trigger tokens are updated by the HotFlip screening and verification procedure in Eqs. ([19](#S5.E19))-([20](#S5.E20)). These two updates share the same joint loss, so the image perturbation and text trigger are optimized to support the same protection purpose. The algorithm preserves feasibility after every update: projection clips image perturbations to the $\ell_{\infty}$ budget and the valid pixel range, while text updates only select admissible tokens within the fixed trigger-length budget. This algorithm balances overall feasibility with the effectiveness, stealthiness, and robustness of the protection.

## 6 Experiments

We empirically validate MMGuard along six axes. We first detail the datasets, target LVLMs, attack scenarios, and evaluation metrics in Sec. [6.1](#S6.SS1). We then quantify protection effectiveness under white-box and gray-box attackers in Sec. [6.2](#S6.SS2), and assess transferability to black-box LVLMs and diverse fine-tuning techniques in Sec. [6.3](#S6.SS3). We further evaluate robustness against adaptive attackers that apply adaptive attacks by data transformations and data mixing in Sec. [6.5](#S6.SS5), and measure practicality in terms of stealthiness and computational cost in Sec. [6.4](#S6.SS4). We finally analyze the protection mechanism in Sec. [6.6](#S6.SS6) through ablations, parameter sensitivity, and attention-level visualization.

### 6.1 Experimental Setup

Datasets and Tasks.
We evaluate MMGuard across six public multimodal question-answering benchmarks spanning general visual reasoning, domain-specific reasoning, text-centric perception, and document understanding. RealWorldQA  tests understanding of everyday physical scenes, including spatial relations, object affordances, and commonsense visual cues. MMStar  evaluates fine-grained multimodal reasoning across six capability categories and eighteen task axes. ScienceQA  covers curriculum-grounded science reasoning that combines visual evidence with textual context. VQA-RAD  evaluates radiology-oriented medical VQA over clinical images. TextVQA  measures scene-text reading and reasoning in natural images, while DocVQA  targets document image understanding over structured and semi-structured text. Table [1](#S6.T1) summarizes the dataset statistics.

**Table 1: Statistics of the evaluation datasets.**
| Dataset | Domain | Task | Size |
| --- | --- | --- | --- |
| RealWorldQA | General | Visual Understanding | 765 |
| MMStar | General | Multimodal Reasoning | 1,500 |
| ScienceQA | Science | Science Reasoning | 21,208 |
| VQA-RAD | Medical | Medical VQA | 2,244 |
| TextVQA | Scene text | OCR Reasoning | 45,336 |
| DocVQA | Document | Document Understanding | 50,000 |

Large Vision-Language Models and Training Configuration.
We consider and evalute state-of-the-art open-source LVLMs with various architectures and parameter scales including Qwen3-VL-2B-Instruct, Qwen3-VL-4B-Instruct, and Qwen3-VL-8B-Instruct ; Qwen2.5-VL-7B-Instruct ; Llama-3.2-11B-Vision-Instruct ; LLaVA-v1.5-7B ; InternVL3.5-8B ; Gemma-3-4B-IT ; and GLM-4.1V-9B-Base . We consider three attack scenarios with different levels of surrogate knowledge: (*i*) white-box: we use Qwen3-VL-4B-Instruct as the surrogate model and evaluate it as the white-box attacker; (*ii*) gray-box: we use Qwen3-VL-4B-Instruct as the surrogate and evaluate the same Qwen-family variants with different scales; and (*iii*) black-box: we leverage Qwen3-VL-4B-Instruct  and MiniCPM-V-4  as surrogate models to generate the protected data and evaluate the other six LVLMs as black-box attackers.

Evaluation Metrics.
Our primary utility metric is task accuracy on the clean held-out test set after unauthorized fine-tuning. To quantify protection effectiveness, we further report the accuracy drop relative to clean fine-tuning:

$$ $\Delta_{\mathrm{acc}}=\mathrm{Acc}\bigl(f_{\theta_{\mathrm{clean}}};\mathcal{D}_{\mathrm{test}}\bigr)-\mathrm{Acc}\bigl(f_{\theta_{\mathrm{prot}}};\mathcal{D}_{\mathrm{test}}\bigr),$ (35) $$

Larger $\Delta_{\mathrm{acc}}$ indicates stronger protection, while clean fine-tuning accuracy serves as the task-specific upper reference. We provide complete dataset descriptions, fine-tuning and protection-optimization configurations, and the per-dataset evaluation protocol in Appendices [E](#A5) and [F](#A6).

### 6.2 Effectiveness Evaluation

We assess whether MMGuard produces a reliable protection signal when the attacker fine-tunes on the released data. We examine two aspects: (*i*) effectiveness under white-box and gray-box scenarios; and (*ii*) attacker training dynamics.

Figure: Figure 3: Protection effectiveness across MMGuard variants under white-box and gray-box scenarios. Lower bars indicate stronger protection.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x3.png

Protection Effectiveness Under White-Box and Gray-Box Scenarios.
Fig. [3](#S6.F3) shows that MMGuard consistently lowers post-fine-tuning accuracy compared with the Clean Fine-Tuning reference across the evaluated LVLMs and datasets, confirming that protected samples induce a reliable degradation signal during unauthorized training. The effect is most pronounced in the white-box setting on the surrogate Qwen3-VL-4B, where protected models often approach or fall below the Zero-Shot reference, largely eliminating the utility gain of clean fine-tuning. The degradation is especially clear on visually grounded benchmarks such as TextVQA, DocVQA, MMStar, and RealworldQA, where performance depends heavily on image-text alignment. The protection also transfers to gray-box target models. Although the reduction varies with model similarity and task type, both MMGuard-BPH and MMGuard-CRS remain below the Clean Fine-Tuning reference in nearly all settings, indicating that the shortcut does not overfit to a single checkpoint. The effect is milder on ScienceQA and VQA-RAD, where language priors and domain knowledge can partially compensate for disrupted multimodal learning. The Max variants further strengthen protection in several cases, suggesting improved cross-model transfer. Overall, MMGuard is effective under both white-box and more realistic gray-box scenarios.

Figure: Figure 4: Attacker training loss on protected data (log scale). Min-min variants converge to lower loss than Clean FT, indicating shortcut fitting, while Max variants plateau at higher loss, indicating training disruption.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x4.png

**Table 2: Protection transferability under the black-box scenario. Each cell reports clean-test accuracy with the drop in parentheses.**
|  | RealWorldQA | ScienceQA | TextVQA |  |  |  |  |  |  |  |  |  |  |  |  |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Method | Llama | LLaVA | InternVL | Gemma | GLM | Llama | LLaVA | InternVL | Gemma | GLM | Llama | LLaVA | InternVL | Gemma | GLM |
| Zero-Shot (Clean) | 49.7 (+6.9) | 53.6 (+3.9) | 54.9 (+5.9) | 36.6 (+19.6) | 27.5 (+39.8) | 84.5 (+11.0) | 63.0 (+16.5) | 97.5 (+1.5) | 76.5 (+8.0) | 88.5 (+9.5) | 14.5 (+67.5) | 51.5 (+4.0) | 75.0 (+1.5) | 54.5 (+28.5) | 0.0 (+85.0) |
| Fine-Tuning (Clean) | 56.6 (0.0) | 57.5 (0.0) | 60.8 (0.0) | 56.2 (0.0) | 67.3 (0.0) | 95.5 (0.0) | 79.5 (0.0) | 99.0 (0.0) | 84.5 (0.0) | 98.0 (0.0) | 82.0 (0.0) | 55.5 (0.0) | 76.5 (0.0) | 83.0 (0.0) | 85.0 (0.0) |
| Image | 61.4 (-4.8) | 55.6 (+1.9) | 60.8 (0.0) | 51.6 (+4.6) | 62.7 (+4.6) | 93.5 (+2.0) | 77.5 (+2.0) | 98.5 (+0.5) | 82.0 (+2.5) | 99.0 (-1.0) | 77.5 (+4.5) | 52.0 (+3.5) | 76.0 (+0.5) | 79.0 (+4.0) | 83.5 (+1.5) |
| Text | 57.5 (-0.9) | 55.6 (+1.9) | 60.1 (+0.7) | 52.9 (+3.3) | 60.1 (+7.2) | 95.0 (+0.5) | 76.5 (+3.0) | 99.0 (0.0) | 80.5 (+4.0) | 98.5 (-0.5) | 80.5 (+1.5) | 53.0 (+2.5) | 74.5 (+2.0) | 81.5 (+1.5) | 83.5 (+1.5) |
| Multimodal | 56.9 (-0.3) | 56.2 (+1.3) | 58.8 (+2.0) | 51.6 (+4.6) | 62.1 (+5.2) | 93.5 (+2.0) | 75.5 (+4.0) | 98.0 (+1.0) | 82.0 (+2.5) | 97.5 (+0.5) | 79.5 (+2.5) | 52.0 (+3.5) | 76.5 (0.0) | 80.0 (+3.0) | 82.5 (+2.5) |
| MMGuard-BPH | 52.9 (+3.7) | 55.6 (+1.9) | 54.9 (+5.9) | 50.3 (+5.9) | 60.8 (+6.5) | 90.0 (+5.5) | 73.5 (+6.0) | 97.5 (+1.5) | 78.5 (+6.0) | 95.0 (+3.0) | 76.0 (+6.0) | 51.0 (+4.5) | 74.0 (+2.5) | 75.5 (+7.5) | 82.0 (+3.0) |
| MMGuard-CRS | 51.7 (+4.9) | 54.4 (+3.1) | 56.2 (+4.6) | 51.0 (+5.2) | 61.4 (+5.9) | 92.0 (+3.5) | 74.5 (+5.0) | 97.5 (+1.5) | 79.5 (+5.0) | 93.5 (+4.5) | 78.0 (+4.0) | 50.5 (+5.0) | 74.5 (+2.0) | 77.0 (+6.0) | 82.0 (+3.0) |
| MMGuard-BPH-Max | 54.4 (+2.2) | 56.0 (+1.5) | 55.4 (+5.4) | 49.5 (+6.7) | 62.6 (+4.7) | 92.0 (+3.5) | 72.0 (+7.5) | 96.5 (+2.5) | 79.5 (+5.0) | 96.5 (+1.5) | 77.0 (+5.0) | 49.5 (+6.0) | 74.0 (+2.5) | 76.5 (+6.5) | 81.0 (+4.0) |
| MMGuard-CRS-Max | 53.6 (+3.0) | 53.4 (+4.1) | 57.3 (+3.5) | 51.4 (+4.8) | 63.2 (+4.1) | 91.5 (+4.0) | 76.0 (+3.5) | 96.5 (+2.5) | 78.5 (+6.0) | 95.0 (+3.0) | 78.5 (+3.5) | 50.0 (+5.5) | 73.0 (+3.5) | 78.5 (+4.5) | 80.5 (+4.5) |
|  | MMStar | VQA-RAD | DocVQA |  |  |  |  |  |  |  |  |  |  |  |  |
| Method | Llama | LLaVA | InternVL | Gemma | GLM | Llama | LLaVA | InternVL | Gemma | GLM | Llama | LLaVA | InternVL | Gemma | GLM |
| Zero-Shot (Clean) | 49.7 (+11.0) | 34.3 (+9.2) | 63.0 (+5.0) | 43.3 (+8.4) | 48.3 (+22.7) | 0.0 (+58.8) | 37.5 (+14.6) | 53.9 (+5.3) | 7.3 (+44.7) | 0.0 (+58.9) | 2.5 (+66.5) | 11.0 (+9.5) | 36.5 (+8.5) | 51.0 (+16.0) | 0.0 (+76.5) |
| Fine-Tuning (Clean) | 60.7 (0.0) | 43.5 (0.0) | 68.0 (0.0) | 51.7 (0.0) | 71.0 (0.0) | 58.8 (0.0) | 52.1 (0.0) | 59.2 (0.0) | 52.0 (0.0) | 58.9 (0.0) | 69.0 (0.0) | 20.5 (0.0) | 45.0 (0.0) | 67.0 (0.0) | 76.5 (0.0) |
| Image | 60.3 (+0.4) | 43.3 (+0.2) | 67.7 (+0.3) | 48.7 (+3.0) | 67.7 (+3.3) | 55.0 (+3.8) | 49.5 (+2.6) | 57.4 (+1.8) | 50.6 (+1.4) | 52.3 (+6.6) | 65.5 (+3.5) | 16.0 (+4.5) | 41.5 (+3.5) | 63.0 (+4.0) | 71.0 (+5.5) |
| Text | 59.7 (+1.0) | 41.7 (+1.8) | 66.3 (+1.7) | 51.3 (+0.4) | 68.7 (+2.3) | 53.9 (+4.9) | 49.7 (+2.4) | 56.8 (+2.4) | 49.2 (+2.8) | 53.7 (+5.2) | 66.0 (+3.0) | 16.5 (+4.0) | 41.5 (+3.5) | 60.5 (+6.5) | 71.0 (+5.5) |
| Multimodal | 58.0 (+2.7) | 41.0 (+2.5) | 67.7 (+0.3) | 50.3 (+1.4) | 67.7 (+3.3) | 55.4 (+3.4) | 48.8 (+3.3) | 55.9 (+3.3) | 50.1 (+1.9) | 52.3 (+6.6) | 66.0 (+3.0) | 16.0 (+4.5) | 40.5 (+4.5) | 62.0 (+5.0) | 73.5 (+3.0) |
| MMGuard-BPH | 56.3 (+4.4) | 39.0 (+4.5) | 65.0 (+3.0) | 45.7 (+6.0) | 65.7 (+5.3) | 52.1 (+6.7) | 46.2 (+5.9) | 55.4 (+3.8) | 47.5 (+4.5) | 52.1 (+6.8) | 63.5 (+5.5) | 13.5 (+7.0) | 41.0 (+4.0) | 60.5 (+6.5) | 68.5 (+8.0) |
| MMGuard-CRS | 54.0 (+6.7) | 38.7 (+4.8) | 65.3 (+2.7) | 45.3 (+6.4) | 66.3 (+4.7) | 51.2 (+7.6) | 45.8 (+6.3) | 56.1 (+3.1) | 47.0 (+5.0) | 52.1 (+6.8) | 63.0 (+6.0) | 14.5 (+6.0) | 40.5 (+4.5) | 61.0 (+6.0) | 68.0 (+8.5) |
| MMGuard-BPH-Max | 57.2 (+3.5) | 39.2 (+4.3) | 64.5 (+3.5) | 47.5 (+4.2) | 65.8 (+5.2) | 53.2 (+5.6) | 47.2 (+4.9) | 55.0 (+4.2) | 47.5 (+4.5) | 51.3 (+7.6) | 64.0 (+5.0) | 13.0 (+7.5) | 41.0 (+4.0) | 59.5 (+7.5) | 68.0 (+8.5) |
| MMGuard-CRS-Max | 56.2 (+4.5) | 39.2 (+4.3) | 65.2 (+2.8) | 48.8 (+2.9) | 65.5 (+5.5) | 52.3 (+6.5) | 46.7 (+5.4) | 54.6 (+4.6) | 49.9 (+2.1) | 50.8 (+8.1) | 64.5 (+4.5) | 14.0 (+6.5) | 40.0 (+5.0) | 59.5 (+7.5) | 70.5 (+6.0) |

Training Dynamics on Protected Data.
Fig. [4](#S6.F4) further explains the accuracy degradation by showing how protected data changes the attacker’s optimization trajectory. The min-min variants, MMGuard-BPH and MMGuard-CRS, consistently drive the training loss below Clean Fine-Tuning, often by one to two orders of magnitude. This indicates that the attacker can fit the protected training set more easily than the clean one by exploiting a perturbation-induced shortcut. As a result, the low training loss does not transfer to clean-test accuracy. In contrast, the adversarial Max variants follow the opposite pattern: their losses remain substantially above Clean Fine-Tuning and plateau during training. This suggests a direct training-disruption effect, where the protected samples prevent effective empirical risk minimization rather than merely offering an easier shortcut. The attacker, therefore, cannot recover by simply training longer. Together, these two regimes reveal complementary protection mechanisms: min-min variants induce shortcut overfitting, while Max variants obstruct optimization. These dynamics support the design intuition in Sec. [5.5](#S5.SS5) and explain the consistent effectiveness observed in Fig. [3](#S6.F3). Additional per-dataset, per-backbone, and per-variant results are deferred to Appendix [H](#A8).

### 6.3 Transferability Evaluation

In practice, a defender has no prior knowledge of the LVLM or fine-tuning recipe an attacker may employ. We evaluate transferability along two axes: (*i*) across LVLMs in the black-box setting, and (*ii*) across attacker fine-tuning strategies.

Transferability Across LVLMs.
We evaluate five black-box LVLMs with different architectures. Table [2](#S6.T2) shows a clear gap between binding-aware and binding-agnostic protections. Single-modality Image and Text baselines, as well as the naive Multimodal combination, provide limited and unstable protection: their drops are often small and occasionally negative, meaning that protected fine-tuning can even improve clean-test accuracy over Clean Fine-Tuning. This suggests that input-space perturbation alone is insufficient for black-box transfer. When one modality remains clean, the attacker can still learn from the unprotected channel, causing the perturbation to behave more like regularization than a reliable protection signal. In contrast, all MMGuard variants produce positive accuracy drops across the evaluations. The gains are modest on saturated tasks such as ScienceQA, where Clean Fine-Tuning is near the ceiling, but are more pronounced on visually grounded tasks such as TextVQA, DocVQA, MMStar, and VQA-RAD. These results identify cross-modal binding as the key transferable component: MMGuard disrupts the attention route linking visual evidence, textual triggers, and answers, a structure broadly shared by LVLMs. Overall, MMGuard transfers consistently to unseen architectures and provides more reliable black-box protection than binding-agnostic baselines.

Figure: Figure 5: Protection transferability of MMGuard-BPH and MMGuard-CRS across attacker fine-tuning strategies. Lower bars indicate stronger protection.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x5.png

Transferability Across Fine-Tuning Methods.
A defense-aware attacker may change the fine-tuning recipe to bypass the protection signal. We fix the protected data and let the attacker use eight representative strategies: LoRA with ranks $r{=}8/16/64$, QLoRA, DoRA, projector-only tuning, projector +  LLM LoRA, and full fine-tuning. Fig. [5](#S6.F5) shows that both MMGuard-BPH and MMGuard-CRS consistently reduce accuracy compared with the corresponding Clean Fine-Tuning reference across datasets and recipes, indicating that the protection is not tied to a specific tuning method. The degradation is most evident on visually grounded datasets such as TextVQA and DocVQA. Low-capacity recipes, such as projector-only tuning, are particularly affected because the limited trainable parameters encourage reliance on the planted shortcut. Importantly, the effect persists under full fine-tuning, in which all model parameters are updated, indicating that increasing adaptation capacity alone cannot remove the protection signal. Overall, MMGuard transfers across diverse fine-tuning strategies and limits recipe switching as an adaptive countermeasure. Detailed analyses are deferred to Appendix [I](#A9).

**Table 3: Multimodal stealthiness and semantic coherence of protected data. Arrows indicate whether higher or lower values are better. Human-study metrics are evaluated by three human experts and three SOTA LLM judges on a 1–3 scale.**
| Metric | Clean | Image | Text | Multi. | BPH | CRS |
| --- | --- | --- | --- | --- | --- | --- |
| Computational Metrics |  |  |  |  |  |  |
| PSNR $\uparrow$ | $+\infty$ | 34.69 | $+\infty$ | 34.63 | 34.48 | 34.68 |
| SSIM $\uparrow$ | 1.00 | 0.88 | 1.00 | 0.88 | 0.87 | 0.88 |
| LPIPS $\downarrow$ | 0.00 | 0.11 | 0.00 | 0.11 | 0.11 | 0.11 |
| PPL $\downarrow$ | 39.12 | 39.12 | 89.72 | 89.24 | 91.91 | 88.28 |
| Edit Distance $\downarrow$ | 0.00 | 0.00 | 5.57 | 29.48 | 36.29 | 29.70 |
| BLEU $\uparrow$ | 1.00 | 1.00 | 0.98 | 0.93 | 0.93 | 0.93 |
| Human Study / LLM-as-a-Judge |  |  |  |  |  |  |
| Image Naturalness $\uparrow$ | 3.00 | 2.52 | 3.00 | 2.52 | 2.52 | 2.52 |
| Text Naturalness $\uparrow$ | 2.86 | 2.86 | 1.33 | 1.36 | 1.64 | 1.62 |
| Image-Text Coherence $\uparrow$ | 2.92 | 2.92 | 2.00 | 2.14 | 2.14 | 2.14 |
| Human Answerability $\uparrow$ | 2.73 | 2.60 | 1.99 | 2.33 | 2.33 | 2.33 |

### 6.4 Practicality Evaluation

Beyond effectiveness, a deployable defense must remain unobtrusive to legitimate users and feasible to apply, so we evaluate MMGuard on two practicality axes: (*i*) multimodal stealthiness; and (*ii*) algorithmic efficiency.

Figure: Figure 6: Protection effectiveness under attacker-side data mixing. Lower curves indicate stronger protection.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x6.png

Multimodal Stealthiness and Semantic Coherence.
A practical data-side defense should preserve the utility of released data for human users. We evaluate image stealthiness using PSNR, SSIM, and LPIPS; text stealthiness using perplexity, edit distance, and BLEU; and semantic coherence using a 1–3 rubric covering image naturalness, text naturalness, image-text coherence, and human answerability. The rubric aggregates ratings from three human experts and three SOTA LLM judges. Table [3](#S6.T3) shows that the image-side distortion is minor: MMGuard-BPH and MMGuard-CRS achieve PSNR above $34$ dB, SSIM around $0.88$, and LPIPS $0.11$, matching the Image-only and naive Multimodal baselines. The main perceptual cost stems from text-trigger insertion, which increases perplexity and lowers text naturalness. Nevertheless, BLEU remains high at $0.93$, image-text coherence remains $2.14/3$, and human answerability remains $2.33/3$, indicating that the protected samples are still understandable and answerable to human users. Importantly, BPH and CRS introduce no additional observable stealthiness cost beyond the underlying image perturbation and text trigger, showing that the binding-disruption objective improves protection without further degrading multimodal usability.

Algorithmic Efficiency.
The cost of MMGuard is dominated by surrogate LVLM forward/backward passes. For $n$ samples, $M$ surrogate models, $R$ outer rounds, and $Q$ inner adaptation steps, the overall time complexity is approximately$O(RnM(Q+1)C_{\mathrm{LVLM}})$, where $C_{\mathrm{LVLM}}$ denotes one LVLM forward/backward update. The text-trigger search adds only a small overhead of $O(RnL_{\gamma}(|\mathcal{V}_{\mathrm{adm}}|d+cC_{\mathrm{fw}}))$, since the trigger length $L_{\gamma}$ and verified candidate size $c$ are small constants. The attention-based binding loss reuses attention maps already computed by the LVLM and adds $O(|\mathcal{K}||\mathcal{H}|L^{2})$ per pass, matching the standard attention order. The additional memory is mainly for image perturbations and trigger tokens, i.e., $O(nHWC+nL_{\gamma})$, plus negligible online attention aggregation. Since MMGuard is an offline defender-side preprocessing step, it introduces no inference-time overhead after the protected dataset is generated. More detailed analysis of stealthiness trade-offs and a complete complexity breakdown are deferred to Appendix [J](#A10).

Figure: Figure 7: Protection robustness of MMGuard-BPH and MMGuard-CRS under attacker-side data transformations. Lower bars indicate stronger protection.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x7.png

### 6.5 Adaptive Attack Evaluation

An attacker aware of MMGuard may seek to invalidate the protection. We discuss and evaluate two adaptive strategies through (*i*) data transformation and (*ii*) data mixing.
Robustness Against Data Transformations.
A defense-aware attacker may preprocess the scraped data to weaken the protection signal. We evaluate nine common transformations, including image-side operations (RCP, JPEG compression, and blurring), text-side normalizations (punctuation removal, case normalization, and whitespace normalization), and their compositions. Fig. [7](#S6.F7) summarizes the results. Across all four datasets and all transformations, MMGuard-BPH and MMGuard-CRS consistently remain below the clean fine-tuning reference under the same transformation, indicating that MMGuard preserves its robustness under adaptive preprocessing. Among the evaluated transformations, the composition RCP+Punct is the most effective because it simultaneously perturbs the visual input and normalizes the textual input. Nevertheless, the protection effect persists: MMGuard induces a strong shortcut through cross-modal attention binding, so surface-level changes to only part of the signal cannot fully neutralize it. More generally, image-side transformations are stronger than text-side normalizations, which is expected because these tasks rely heavily on visual evidence, and image transformations can partially distort the perturbation. Text-side normalizers are less effective because the trigger consists of admissible vocabulary tokens whose form and semantics are largely preserved. Overall, MMGuard remains robust against standard attacker-side data transformations.

Figure: Figure 8: Ablation study and parameter sensitivity of MMGuard-BPH. The leftmost panel reports nine ablation conditions on the full method, while the remaining five panels sweep the binding-loss weight $\lambda_{\mathrm{bind}}$, inner-loop steps $Q$, gradient layer depth $|\mathcal{K}|$, image perturbation budget $\epsilon_{x}$, and text trigger budget $\epsilon_{t}$. Lower values indicate stronger protection across all panels. The results are averages across six datasets.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x8.png

Robustness Against Data Mixing.
A defense-aware attacker may attempt to dilute the protection signal by mixing protected samples with clean data collected from external sources. We evaluate this setting by varying the protection ratio from $0\%$ to $100\%$ and reporting clean-test accuracy after fine-tuning. Fig. [6](#S6.F6) summarizes the results. Across all six datasets, both MMGuard-BPH and MMGuard-CRS consistently remain below the Clean Fine-Tuning reference, showing that data mixing cannot fully eliminate the protection effect. The degradation is particularly clear on TextVQA and DocVQA, where increasing the protected portion further reduces clean-test accuracy, with full protection yielding the largest gap relative to clean fine-tuning. On RealworldQA, MMStar, ScienceQA, and VQA-RAD, the curves are less strictly monotonic, but the protected models still stay below the clean reference across nearly all ratios. This indicates that even partial protected coverage can inject a usable pattern into the fine-tuning process. As the protection ratio increases, this shortcut receives stronger gradient support, but the exact accuracy trend depends on dataset difficulty and fine-tuning variance. Overall, MMGuard remains robust against attacker-side data mixing: clean data dilution may reduce the dosage of the protection signal, but it does not restore clean fine-tuning performance. Details analyses for robustness are deferred to Appendix [K](#A11).

### 6.6 Model Mechanism Analysis

We analyze *why* and *how* MMGuard and its components work from three perspectives: (*i*) an ablation study isolating the contribution of each component, (*ii*) a sensitivity analysis of the five core hyperparameters, and (*iii*) a visualization-based analysis of the cross-modal binding mechanism.

Ablation Study.
The leftmost panel of Fig. [8](#S6.F8) shows that the full MMGuard-BPH achieves the strongest protection among all ablation variants. Using only image perturbations, only text triggers, or a naive multimodal combination yields noticeably higher accuracy, indicating that neither modality alone nor a simple combination is sufficient. This isolates the binding-disruption objective as the key component: it does not merely add image and text perturbations, but actively couples them into a shortcut that interferes with clean multimodal learning. In the full design, removing the image perturbation results in greater degradation than removing the text trigger, suggesting that the perturbation provides the dominant optimization signal, while the trigger acts as a discrete activator. Among the binding paths in Eq. ([27](#S5.E27)), ablating the answer-anchored paths is more harmful than ablating the trigger-to-perturbation path, consistent with Theorem [5.1](#S5.Thmtheorem1): the answer-anchored routes are directly responsible for shifting the attention distribution of protected data away from that of a clean one.

Figure: Figure 9: Visualization of the cross-modal binding mechanism on a representative sample. Row 1: released images; Row 2: perturbation and perturbation tokens $\Omega_{\delta}$ (cyan boxes); Row 3: answer-token attention maps; Row 4: head-averaged attention mass distribution with corresponding layerwise curves.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x9.png

Parameter Analysis.
The remaining panels of Fig. [8](#S6.F8) examine the sensitivity of MMGuard to its main hyperparameters. Increasing the binding-loss weight $\lambda_{\mathrm{bind}}$ strengthens MMGuard-CRS, while MMGuard-BPH exhibits a non-monotonic trend, reflecting the difference between CRS’s route-agnostic attention shifting and BPH’s more prescriptive binding objective. The inner-loop step $Q$ is crucial: setting $Q{=}0$ removes the bilevel adaptation effect and substantially weakens protection, whereas a small positive value already captures most of the gain. For the gradient layer set $\mathcal{K}$, shallower layers provide stronger protection, suggesting that the relevant cross-modal binding behavior emerges early in multimodal fusion. Finally, both image and text budgets show clear strength–stealthiness trade-offs. Larger image perturbation budgets improve protection after a threshold, with $\epsilon_{x}{=}8/255$ serving as a practical default, while increasing the text trigger budget further strengthens protection but may reduce textual naturalness. Overall, the ablation and sensitivity results support the design of MMGuard: effective protection requires both modalities, explicit cross-modal binding disruption, and a moderate perturbation budget.

Cross-Modal Binding Mechanism Analysis.
Fig. [9](#S6.F9) visualizes the protection mechanism on a representative sample. The released image stays perceptually close to the clean input (Row 1), while the perturbation patches $\Omega_{\delta}$ (Row 2) form a structured area for the binding objective. Row 3 shows that attention is *redirected* from the genuine visual evidence (e.g., bicycle lane) onto $\Omega_{\delta}$ in off-content regions (e.g., sky), and under MMGuard, answer-to-trigger, answer-to-perturbation, and trigger-to-perturbation attention all increase from the clean baseline, while attention to non-perturbed image tokens is suppressed. These patterns support the mechanism predicted by Theorem [5.1](#S5.Thmtheorem1) because clean test inputs do not contain the learned trigger–perturbation binding, the model cannot reuse the shortcut acquired during protected fine-tuning. The layerwise curves also reveal different routing behaviors between the two variants. MMGuard-BPH concentrates answer-to-trigger attention in early-to-middle LM layers, consistent with the prescribed binding paths in Eq. ([27](#S5.E27)); MMGuard-CRS induces stronger answer-to-perturbation attention in later layers, matching its route-agnostic objective in Eq. ([29](#S5.E29)). Thus, both variants create a distributional shift between protected training and clean inference, but through different attention routes. This explains their similar overall effectiveness while allowing different trade-offs in transferability and robustness across architectures. Additional model design, parameter, and mechanism diagnostics are deferred to Appendix [L](#A12).

## 7 Limitations and Discussion.

Pre-Publication Scope.
MMGuard operates only on defender-controlled data before release. Content already scraped or published without protection is outside its scope; likewise, an adversary with access to an unprotected copy may bypass the protected signal. Thus, MMGuard should be viewed as one layer in a defense-in-depth strategy, complementing reactive mechanisms such as watermark-based provenance , machine unlearning , and legal recourse rather than replacing them. As a proactive defense, MMGuard blocks a specific abuse pathway and lets complementary mechanisms address the remaining attack surface.

Pretraining-from-Scratch Threats.
Our threat model addresses the dominant efficient setting in which commercial actors fine-tune publicly released LVLM checkpoints on scraped corpora. We do not evaluate MMGuard against adversaries who pre-train an LVLM from scratch on a corpus dominated by protected samples. Such a regime is economically unattractive for the long-tail data that we target; however, well-resourced adversaries (e.g., tech companies) with sufficiently large protected pools could, in principle, shift this trade-off. Quantifying the protection frontier against the LVLM pretraining settings is left for future work.

Computational Overhead.
Constructing unlearnable examples for LVLMs requires solving the multi-objective, constrained optimization problem in Eq. ([34](#S5.E34)), which jointly couples continuous image perturbations, discrete trigger search, and surrogate-side adaptation under modality-specific budgets. This optimization is inherent to data-centric protection: the defender must solve it per sample before release, so Algorithm [1](#alg1) incurs a one-time pre-release cost that scales linearly with the dataset size. We leave more advanced techniques, such as universal perturbation generators, shared surrogate caches across samples, and other acceleration techniques, to future work as a path toward web-scale deployment.

Stealthiness Cost on the Text Channel.
The residual stealthiness cost of MMGuard is concentrated on the text channel. The inserted trigger raises perplexity and reduces text naturalness under our human and LLM-judge rubric, although BLEU and human answerability remain largely preserved (Table [3](#S6.T3)). This effect arises because the trigger search in Sec. [5.2](#S5.SS2) optimizes protection strength under a discrete admissibility constraint without an explicit fluency prior. Augmenting the screen-and-verify procedure with a language-model fluency term, or filtering the admissible vocabulary by fluency, offers a principled way to tighten this trade-off.

Beyond LVLM.
Our evaluation spans a diverse range of multimodal tasks achieved by LVLM, including visual question answering, domain-specific reasoning, document-grounded reasoning, and understanding (Sec. [6.1](#S6.SS1)), while newly emerging topics, such as multiagent systems and agentic tool use, in-context learning introduce distinct problem and optimization challenges. Extending MMGuard to such regimes is a promising direction for future work.

## 8 Conclusion

We presented MMGuard, the first proactive, data-centric protection framework against unauthorized fine-tuning of large vision-language models. By coupling bounded image perturbations with discrete text triggers and steering them through a cross-modal binding-disruption objective, MMGuard reshapes how an attacker’s LVLM allocates attention among visual evidence, textual context, and target responses, so that fine-tuning minimizes loss along a protection-specific shortcut that does not transfer to clean evaluation. We formalized multimodal data protection under a practical threat model, derived a smoothness-based optimality bound for the discrete trigger search and a total-variation lower bound for the binding mechanism, and instantiated the framework as a constrained multi-objective optimization over an ensemble of surrogate LVLMs. Across six multimodal datasets and nine open-source LVLMs spanning white-box, gray-box, and black-box scenarios, MMGuard consistently degrades unauthorized fine-tuning performance while preserving perceptual fidelity, and remains effective under aggressive fine-tuning recipes, input transformations, and partial-coverage data mixing. We hope these results contribute a structural primitive for proactive multimodal data ownership and, in combination with reactive mechanisms such as watermarking and machine unlearning, help support a layered defense for public multimodal content.

## Ethics Considerations

We developed and evaluated MMGuard in accordance with the Menlo Report principles of respect for persons, beneficence, justice, and respect for law and public interest. MMGuard is a *defensive* primitive whose explicit purpose is to return agency to data owners whose public image-text content is otherwise absorbed into commercial LVLM fine-tuning without consent. The principal beneficiaries are individual creators, news outlets, medical-image curators, and small dataset providers, whose interests are routinely at risk in current scraping practice; the principal cost falls on parties seeking to monetize unauthorized adaptation. We argue that this trade-off is well-aligned with the public interest, and we frame the considerations below accordingly.

Datasets and Models.
All six evaluation datasets (RealWorldQA, MMStar, ScienceQA, VQA-RAD, TextVQA, DocVQA) are publicly released research benchmarks, used here strictly under their original licenses and intended scientific use. We used them only as carriers of a generic image-text-response structure and did not extract, redistribute, or attempt to re-identify any depicted individuals. The medical subset (VQA-RAD) is the publicly released, de-identified Hugging Face version; we performed no patient-level analysis. The nine LVLMs we evaluate are all open-weight checkpoints used under their respective licenses, and protection generation does not require any non-public access to the models.

Human Subjects and LLM-as-a-Judge Study.
Our stealthiness study (Sec. [6.4](#S6.SS4) and Appendix [G](#A7)) is a non-interventional rating task on protected versions of public benchmark images. We recruited three independent human raters and complemented them with three contemporary LLM judges (GPT, Gemini, and Claude Opus) per sample, applying them to ten samples per dataset for a total of 60 image-text pairs. The rubric (Table [6](#A7.T6)) elicits ordinal judgments about visual naturalness, textual fluency, cross-modal coherence, and answerability, and never asks raters about themselves or any third party. No personal data was collected from raters beyond what was needed to deliver the task; ratings were stored in de-identified form and used only in aggregate, and participation was voluntary with the right to withdraw at any time. The LLM judges were queried only with the protected sample under evaluation; no defender-side artifacts (surrogate gradients, optimization traces, or unreleased intermediate perturbations) were transmitted to third-party services.

Dual-Use Considerations.
Like other unlearnable-example research, MMGuard is dual-use in principle: the same optimization that produces protective shortcuts could in principle be retargeted to silently poison datasets that the perturber does not own. We took two structural steps to keep the contribution within its intended defensive envelope. First, the threat model in Sec. [4](#S4) restricts the defender’s reach to data they themselves control before publication, which is also the only setting in which our claims are validated; the framework does not propose, evaluate, or recommend application to third-party content. Second, the protection signal is bounded by a perceptual budget and leaves the human-facing target response unchanged, so its observable effect on a recipient is degraded LVLM training utility rather than altered semantic content visible to humans. We do not view MMGuard as enabling a novel attack capability beyond what is already available through prior unlearnable-example and adversarial-poisoning work , and our experiments do not target any specific deployed system.

Disclosure and Reproducibility Posture.
Because MMGuard is a data-side defense rather than a vulnerability in a specific product, no coordinated vulnerability disclosure was warranted. We notified no individual model provider, since the protection acts on the defender’s own data and does not exploit a flaw in any released checkpoint. To support replication and scrutiny by the security community, we plan to release the protection code, configuration files, and the ten-sample-per-dataset evaluation bundle used in the human/LLM-judge study, while withholding raw rater identifiers and any artifact that could re-identify participants.

Residual Risks.
Two residual concerns deserve naming. First, our defense raises the cost of unauthorized adaptation but does not eliminate it; data owners who rely solely on MMGuard could over-estimate their protection, and we therefore consistently frame it as one layer of a defense-in-depth strategy alongside watermarking, unlearning, and legal recourse (Sec. [2](#S2)). Second, widespread deployment of unlearnable examples could, in principle, interact with legitimate downstream uses such as accessibility tooling that fine-tunes on user-supplied imagery; the perceptual budget we adopt keeps human-facing utility intact for the tasks we evaluate, but operators of such tools should be aware of the protection signal when consuming third-party data.

## Appendix A Notation

Table [4](#A1.T4) summarizes the main notation used in the problem formulation and method.

**Table 4: Summary of main notation.**
| Symbol | Name | Meaning |
| --- | --- | --- |
| $\mathcal{D}=\{(x_{i},t_{i},y_{i})\}_{i=1}^{n}$ | Clean multimodal dataset | Collection of image, text, and target-response samples. |
| $\widetilde{\mathcal{D}}$ | Protected dataset | Dataset released by the defender after applying image and text protections. |
| $x_{i}$, $t_{i}$, $y_{i}$ | Clean sample components | Image, textual input, and target output for sample $i$. |
| $\tilde{x}_{i}$, $\tilde{t}_{i}$ | Protected inputs | Perturbed image and trigger-inserted text released for sample $i$. |
| $\mathcal{X}$, $\mathcal{T}$, $\mathcal{Y}$ | Data spaces | Image space, natural-language input space, and target-output space. |
| $f_{\theta}$ | LVLM | Model mapping an image-text input to an output response. |
| $\theta=(\theta_{\mathrm{frz}},\theta_{\mathrm{tr}})$ | Model parameters | Frozen pretrained parameters and trainable fine-tuning parameters. |
| $\Theta$ | Parameter space | Feasible model-parameter space used in the attacker’s training objective. |
| $\tilde{f}^{(m)}$ | Surrogate LVLM | The $m$-th surrogate model available to the defender. |
| $M$, $\omega_{m}$ | Ensemble size and weight | Number of surrogate LVLMs and the non-negative weight of surrogate $m$. |
| $\mathcal{G}_{\phi}$, $\phi$, $\Phi$ | Protection map and parameters | Defender-side transformation, its parameters, and feasible protection-parameter space. |
| $\mathcal{B}(\mathcal{D})$ | Protection budget set | Set of protected datasets satisfying modality-specific constraints. |
| $d_{x}$, $d_{t}$ | Distance measures | Visual and textual change measures. |
| $\epsilon_{x}$, $\epsilon_{t}$ | Budgets | Image perturbation budget and text trigger-length budget. |
| $\delta_{i}$, $\gamma_{i}$ | Protection variables | Image perturbation and inserted text trigger for sample $i$. |
| $\mathcal{C}^{\delta}_{i}$, $\mathcal{C}^{\gamma}_{i}$ | Feasible sets | Allowed image perturbations and text triggers for sample $i$. |
| $\mathcal{V}$, $\mathcal{V}_{\mathrm{adm}}$, $\mathcal{V}^{\mathrm{cand}}_{i,j}$ | Token sets | Tokenizer vocabulary, admissible trigger vocabulary, and screened candidate tokens. |
| $g_{x}$, $\tilde{g}_{x}$ | Image processors | Practical image processor and differentiable surrogate processor. |
| $E_{x}$, $P_{x}$, $E_{t}$ | Encoders/projector | Visual encoder, modality projector, and text embedding map. |
| $H_{i}^{x}$, $Z_{i}^{x}$, $Z_{i}^{t}$, $S_{i}$ | LVLM representations | Visual features, projected visual tokens, text embeddings, and joint multimodal sequence. |
| $\mathcal{L}_{\mathrm{train}}$, $\mathcal{L}_{\mathrm{eval}}$ | Dataset-level losses | Training and clean evaluation objectives. |
| $\ell_{\mathrm{train}}$, $\ell_{\mathrm{prot}}$ | Optimization losses | Sample-level training loss and defender protection loss. |
| $\ell_{\mathrm{bind}}$, $\ell_{\mathrm{joint}}$ | Binding and joint losses | Attention-binding disruption loss and weighted single-surrogate protection loss. |
| $\lambda_{\mathrm{train}}$, $\lambda_{\mathrm{bind}}$ | Loss weights | Weights for task-level fitting and binding-disruption terms. |
| $\Omega_{x}$, $\Omega_{t}$, $\Omega_{\gamma}$, $\Omega_{y}$ | Token subsets | Image, original text, inserted trigger, and answer tokens in the LVLM sequence. |
| $\Omega_{\delta}$, $\tau_{\delta}$, $\rho_{i,b}$ | Perturbation tokens | Top-$\tau_{\delta}$ fraction of image tokens by perturbation magnitude; $\rho_{i,b}$ is the average $\ell_{1}$ magnitude for token $b$. |
| $A^{(k,h)}$, $\bar{A}_{k}(\mathcal{S})$ | Attention distributions | Attention matrix for layer $k$ and head $h$, and head-averaged attention mass from source set $\mathcal{S}$. |
| $\mathcal{H}$, $\mathcal{K}$ | Head and layer sets | Attention heads and layers used for binding-disruption objectives. |
| $\mathcal{S}$, $\mathcal{R}$, $U_{\mathcal{R}}$ | Source and target sets | Source token set, target token set, and uniform target distribution over $\mathcal{R}$. |
| $\mathcal{B}_{k}$, $z_{k}$ | Attention support and logits | Support of nonzero attention mass at layer $k$ and corresponding effective logits. |
| $\beta_{1},\beta_{2},\beta_{3}$ | BPH weights | Weights for the three Bridge Path Hijack attention paths. |

## Appendix B Optimality of Trigger Updates

###### Lemma B.1 (Trigger Updates Optimality) .

Suppose $\ell_{\mathrm{prot}}$ is $L$-smooth as a function of the embedding at trigger position $j$, and let $R=\max_{v\in\mathcal{V}_{\mathrm{adm}}}\|e_{v}-e_{\gamma_{i,j}}\|$. Let $v^{\dagger}\in\arg\min_{v\in\mathcal{V}_{\mathrm{adm}}}\ell_{\mathrm{prot}}\!\bigl(\gamma_{i}^{[j=v]}\bigr)$ be the exact best admissible substitution, and let $v_{i,j}^{\star}$ be the candidate selected by Eq. ([20](#S5.E20)) from the top-$c$ screen $\mathcal{V}^{\mathrm{cand}}_{i,j}$ ranked by $s_{i,j}(\cdot)$. Then

$$ $\ell_{\mathrm{prot}}\!\bigl(\gamma_{i}^{[j=v_{i,j}^{\star}]}\bigr)-\ell_{\mathrm{prot}}\!\bigl(\gamma_{i}^{[j=v^{\dagger}]}\bigr)\;\leq\;LR^{2}.$ (36) $$

*Proof.*
For position $j$ of the trigger $\gamma_{i}$, let $e=e_{\gamma_{i,j}}$ denote its current embedding and write $\ell(e)=\ell_{\mathrm{prot}}(\gamma_{i})$ as a function of that embedding with all other variables fixed. Define the candidate-induced loss change

$$ $\Delta_{i,j}(v)\;=\;\ell_{\mathrm{prot}}\!\bigl(\gamma_{i}^{[j=v]}\bigr)-\ell_{\mathrm{prot}}(\gamma_{i}),$ (37) $$

and recall the linear score
$s_{i,j}(v)=(e_{v}-e)^{\top}\nabla_{e}\ell$ from Eq. ([19](#S5.E19)). By the standard descent-lemma form of $L$-smoothness applied to $\ell$ along the segment from $e$ to $e_{v}$,

$$ $\bigl|\Delta_{i,j}(v)-s_{i,j}(v)\bigr|\;\leq\;\tfrac{L}{2}\,\|e_{v}-e\|^{2}\;\leq\;\tfrac{LR^{2}}{2},$ (38) $$

for every admissible $v\in\mathcal{V}_{\mathrm{adm}}$.

Consider two cases for the exact admissible optimum $v^{\dagger}$.

*Case 1: $v^{\dagger}\in\mathcal{V}^{\mathrm{cand}}_{i,j}$.*
The verification step in Eq. ([20](#S5.E20)) performs an exact minimization of $\ell_{\mathrm{prot}}$ over $\mathcal{V}^{\mathrm{cand}}_{i,j}$, so $\ell_{\mathrm{prot}}(\gamma_{i}^{[j=v_{i,j}^{\star}]})\leq\ell_{\mathrm{prot}}(\gamma_{i}^{[j=v^{\dagger}]})$ and Eq. ([36](#A2.E36)) holds with gap zero.

*Case 2: $v^{\dagger}\notin\mathcal{V}^{\mathrm{cand}}_{i,j}$.*
By the top-$c$ screening rule, every $w\in\mathcal{V}^{\mathrm{cand}}_{i,j}$ has $s_{i,j}(w)\leq s_{i,j}(v^{\dagger})$. Pick any such $w$. Applying Eq. ([38](#A2.E38)) to both $w$ and $v^{\dagger}$,

$$ $\displaystyle\Delta_{i,j}(w)$ $\displaystyle\leq s_{i,j}(w)+\tfrac{LR^{2}}{2}$ $\displaystyle\leq s_{i,j}(v^{\dagger})+\tfrac{LR^{2}}{2}$ $\displaystyle\leq\Delta_{i,j}(v^{\dagger})+LR^{2}.$ (39) $$

The verification step then selects $v_{i,j}^{\star}\in\mathcal{V}^{\mathrm{cand}}_{i,j}$ as the exact minimizer of $\ell_{\mathrm{prot}}$ on the shortlist, so $\Delta_{i,j}(v_{i,j}^{\star})\leq\Delta_{i,j}(w)\leq\Delta_{i,j}(v^{\dagger})+LR^{2}$. Subtracting the common offset $\ell_{\mathrm{prot}}(\gamma_{i})$ from both sides of the last inequality recovers Eq. ([36](#A2.E36)). $\square$

###### Lemma B.1 (Trigger Updates Optimality) .

## Appendix C Contrastive Form of the Attention-Mass Loss

###### Proposition C.1 (Contrastive Learning for Attention Mass Alignment) .

For each $k\in\mathcal{K}$, let $\mathcal{B}_{k}=\{b\in\Omega:\bar{A}_{k,b}(\mathcal{S})>0\}$. If $\mathcal{R}\subseteq\mathcal{B}_{k}$ for all $k\in\mathcal{K}$, write each $\bar{A}_{k}(\mathcal{S})$ on its support as a softmax over effective logits $z_{k}$. Then the attention-mass loss in Eq. ([26](#S5.E26)) satisfies

$$ $\displaystyle\ell_{\mathrm{mass}}(\mathcal{S},\mathcal{R})$ $\displaystyle=\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}\ell_{\mathrm{NCE}}^{(k)}(\mathcal{S},\mathcal{R})-\log|\mathcal{R}|,$ (40) $\displaystyle\ell_{\mathrm{NCE}}^{(k)}(\mathcal{S},\mathcal{R})$ $\displaystyle=\frac{1}{|\mathcal{R}|}\sum_{r\in\mathcal{R}}-\log\frac{\exp(z_{k,r})}{\sum_{b\in\mathcal{B}_{k}}\exp(z_{k,b})}.$ (41) $$

*Proof.*
Fix any $k\in\mathcal{K}$. If any $r\in\mathcal{R}$ has $\bar{A}_{k,r}(\mathcal{S})=0$, then $\mathrm{D}_{\mathrm{KL}}(U_{\mathcal{R}}\|\bar{A}_{k}(\mathcal{S}))=+\infty$, so the finite case requires $\mathcal{R}\subseteq\mathcal{B}_{k}$ for every averaged layer. In this case, since $U_{\mathcal{R}}$ is uniform on $\mathcal{R}$ and zero elsewhere,

$$ $\displaystyle\mathrm{D}_{\mathrm{KL}}\!\bigl(U_{\mathcal{R}}\|\bar{A}_{k}(\mathcal{S})\bigr)$ $\displaystyle=\sum_{b\in\Omega}U_{\mathcal{R}}(b)\log\frac{U_{\mathcal{R}}(b)}{\bar{A}_{k,b}(\mathcal{S})}$ $\displaystyle=\frac{1}{|\mathcal{R}|}\sum_{r\in\mathcal{R}}\log\frac{1/|\mathcal{R}|}{\bar{A}_{k,r}(\mathcal{S})}$ $\displaystyle=\frac{1}{|\mathcal{R}|}\sum_{r\in\mathcal{R}}-\log\bar{A}_{k,r}(\mathcal{S})-\log|\mathcal{R}|.$ (42) $$

Substituting the softmax form
$\bar{A}_{k,r}(\mathcal{S})=\exp(z_{k,r})/\sum_{b\in\mathcal{B}_{k}}\exp(z_{k,b})$
into the last line gives Eq. ([41](#A3.E41)) for layer $k$. Averaging the resulting identity over $k\in\mathcal{K}$ gives Eq. ([40](#A3.E40)). The term $\ell_{\mathrm{NCE}}^{(k)}$ is the average softmax loss obtained by selecting each $r\in\mathcal{R}$ as a positive token and normalizing against the attention support. Therefore, minimizing the finite attention-mass loss is equivalent, up to the constant $-\log|\mathcal{R}|$, to minimizing an averaged multi-positive contrastive objective whose negatives are the non-target tokens in each supported LVLM context. $\square$

###### Proposition C.1 (Contrastive Learning for Attention Mass Alignment) .

## Appendix D Proof of Bridge Path Hijack Effectiveness

*Proof.*
Fix sample $i$, a protection-induced target set $\mathcal{R}_{i}\in\{\Omega_{\gamma,i},\,\Omega_{\delta,i}\}$, and any layer $k\in\mathcal{K}$. Write $P_{k}=\bar{A}^{\mathrm{p}}_{k,i}(\Omega_{y})$ and $Q_{k}=\bar{A}^{\mathrm{c}}_{k,i}(\Omega_{y})$, both viewed as probability distributions on the common, zero-padded token universe in which the positions in $\mathcal{R}_{i}$ receive mass 0 on the clean side. The latter convention follows from the input-dependent definition of $\mathcal{R}_{i}$: trigger tokens are physically absent at clean evaluation, and the perturbation-magnitude ranking that selects $\Omega_{\delta,i}$ in Eq. ([23](#S5.E23)) is degenerate when $\delta_{i}=0$.

*Step 1: Pinsker on the protected attention.*
By definition of $\ell_{\mathrm{mass}}$ in Eq. ([26](#S5.E26)),

$$ $\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}\mathrm{D}_{\mathrm{KL}}\!\bigl(U_{\mathcal{R}_{i}}\,\|\,P_{k}\bigr)\;=\;\ell_{\mathrm{mass}}(\Omega_{y,i},\mathcal{R}_{i})\;\leq\;\eta.$ (43) $$

Pinsker’s inequality applied per layer gives
$\mathrm{TV}(U_{\mathcal{R}_{i}},P_{k})\leq\sqrt{\mathrm{D}_{\mathrm{KL}}(U_{\mathcal{R}_{i}}\|P_{k})/2}$, and Jensen’s inequality applied to the concave map $u\mapsto\sqrt{u}$ yields

$$ $\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}\mathrm{TV}\!\bigl(U_{\mathcal{R}_{i}},P_{k}\bigr)\;\leq\;\sqrt{\eta/2}.$ (44) $$

*Step 2: Target-mass concentration.*
For any probability distributions $P,U$ on a finite set, $\mathrm{TV}(P,U)\geq|P(B)-U(B)|$ for every event $B$. Choosing $B=\mathcal{R}_{i}$ and noting $U_{\mathcal{R}_{i}}(\mathcal{R}_{i})=1$,

$$ $\mathrm{TV}\!\bigl(U_{\mathcal{R}_{i}},P_{k}\bigr)\;\geq\;1-P_{k}(\mathcal{R}_{i}),$ (45) $$

so combining with Eq. ([44](#A4.E44)) yields

$$ $\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}P_{k}(\mathcal{R}_{i})\;\geq\;1-\sqrt{\eta/2}.$ (46) $$

That is, on average across layers, at least a $1-\sqrt{\eta/2}$ fraction of the answer-token attention mass on protected inputs is placed on positions in the target set $\mathcal{R}_{i}$.

*Step 3: Forced redistribution at clean evaluation.*
Because $\mathcal{R}_{i}$ is empty under the clean evaluation input, the model assigns no attention mass to the positions in $\mathcal{R}_{i}$ on the zero-padded clean side: $Q_{k}(\mathcal{R}_{i})=0$ for every $k$. Applying the same event-based bound to the pair $(P_{k},Q_{k})$ with $B=\mathcal{R}_{i}$,

$$ $\mathrm{TV}(P_{k},Q_{k})\;\geq\;\bigl|P_{k}(\mathcal{R}_{i})-Q_{k}(\mathcal{R}_{i})\bigr|\;=\;P_{k}(\mathcal{R}_{i}).$ (47) $$

Averaging over $k\in\mathcal{K}$ and using Eq. ([46](#A4.E46)) gives

$$ $\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}\mathrm{TV}(P_{k},Q_{k})\;\geq\;\frac{1}{|\mathcal{K}|}\sum_{k\in\mathcal{K}}P_{k}(\mathcal{R}_{i})\;\geq\;1-\sqrt{\eta/2},$ (48) $$

which is Eq. ([28](#S5.E28)). The argument is uniform in the choice of $\mathcal{R}_{i}\in\{\Omega_{\gamma,i},\,\Omega_{\delta,i}\}$, so the bound holds for both the $\beta_{2}$ and $\beta_{3}$ binding terms of Eq. ([27](#S5.E27)). $\square$

## Appendix E Dataset Details

This section expands the compact dataset summary in Sec. [6.1](#S6.SS1) by providing the source-reported objectives, formats, and statistics for each of the six benchmark datasets used in our evaluation, followed by representative protected and clean image-text examples.

### E.1 Benchmark Descriptions

RealWorldQA  is an image-text benchmark released on Hugging Face with 765 examples in a single test split. Each example contains an image, a natural-language question, and a free-form answer. The dataset card shows questions involving real-world visual attributes such as object counts, relative position, traffic lights, road layout, and spatial relations.

MMStar  is a vision-indispensable multimodal benchmark. The released dataset contains 1,500 multiple-choice offline evaluation samples selected from 22,401 initial samples through filtering and manual review. The benchmark is organized into six core capabilities and 18 detailed axes, with 250 samples per core capability.

ScienceQA  is collected from elementary and high-school science curricula and contains 21,208 multimodal multiple-choice science questions. Among these questions, 10,332 have image context, 10,220 have text context, and 6,532 have both. The dataset spans natural, language, and social sciences and is organized into 26 topics, 127 categories, and 379 skills.

VQA-RAD  is a radiology visual question answering dataset. The cleaned Hugging Face version contains 2,244 image-question-answer triplets over 314 referenced images, after removing duplicate or overlapping triplets. The question set includes both open-ended free-form questions and close-ended yes/no questions; the supported evaluation distinguishes close-ended yes/no accuracy, open-ended accuracy, and overall accuracy.

TextVQA  is a visual question answering dataset for natural images containing readable scene text. It contains 45,336 questions on 28,408 images, with free-form answers collected from annotators. The benchmark is designed for questions whose answers require reading the text in the image and reasoning about it in the context of the image and the question.

DocVQA  is a visual question answering dataset for document images. It contains 50,000 free-form questions over more than 12,000 document images. The dataset focuses on document-image understanding and includes questions for which document structure and layout can be important for answering.

## Appendix F Training Details

This section details the configurations used by both the defender’s protection-generation pipeline and the attacker’s downstream supervised fine-tuning (SFT). The complete hyperparameter tuning grid spanning the surrogate inner loop, the protection outer loop, and the attacker SFT is summarized in Table [5](#A6.T5). We organize the description into the defender-side protection generation and the attacker-side fine-tuning and evaluation.

### F.1 Defender-Side Protection Generation

Surrogate Fine-Tuning (Inner Loop).
Within each outer round of Algorithm [1](#alg1), the surrogate is adapted to the current protected samples by $Q$ supervised gradient steps. We use LoRA with rank $8$ for Qwen3-VL-4B-Instruct and rank $16$ for MiniCPM-V-4, with dropout $0.1$ and target modules all (attention and MLP projections). The inner learning rate is swept over $\{1{\times}10^{-5},1{\times}10^{-4}\}$ with per-device batch size $2$, and the inner-step count $Q$ is swept over $\{0,1,2,3,4,5\}$. Surrogate adapter weights are discarded after each round and never released.

Protection Optimization (Outer Loop).
MMGuard runs for $R$ outer rounds, with $R$ swept over $\{1,3,5,7\}$. The image perturbation $\delta_{i}$ is updated by PGD under an $\ell_{\infty}$ budget swept over $\epsilon_{x}\in\{1,2,4,8,16\}/255$, with step size $\alpha{=}1/255$ and the number of PGD iterations per round swept over $\{0,1,2,5,8\}$, followed by projection onto the feasible set $\mathcal{C}^{\delta}_{i}$. The text trigger $\gamma_{i}$ is updated by gradient-guided screening followed by exact verification: at each round, we rank candidate tokens by the HotFlip score in Eq. ([19](#S5.E19)), retain the top-$K$ candidates per slot with $K$ swept over $\{8,16,64,128,512\}$, and select the substitution that minimizes the joint loss in Eq. ([20](#S5.E20)). The admissible vocabulary $\mathcal{V}_{\mathrm{adm}}$ is restricted to ASCII alphabetic words with numeric and punctuation tokens forbidden and a leading whitespace prepended; the trigger length $\epsilon_{t}$ is swept over $\{1,3,5,7,9\}$ tokens. The joint loss in Eq. ([34](#S5.E34)) combines a task-side term ($\lambda_{\mathrm{train}}{=}1.0$) and a binding-disruption term with weight $\lambda_{\mathrm{bind}}$ swept over $\{0.1,0.2,0.5,1.0,2.0\}$. For MMGuard-BPH, the answer-to-trigger, trigger-to-image, and answer-to-image paths each receive unit weight; for MMGuard-CRS, the routing-shift loss replaces the three structured paths. The binding gradient is restricted to the first $|\mathcal{K}|$ attention layers, with $|\mathcal{K}|$ swept over $\{1,2,3,4,5,6,7\}$. We instantiate the outer objective as either the adversarial variant in Eq. ([32](#S5.E32)) or the min-min variant in Eq. ([30](#S5.E30)). For the black-box scenario, the ensemble surrogates are Qwen3-VL-4B-Instruct and MiniCPM-V-4 with uniform weights $\omega_{m}{=}1/M$.

### F.2 Attacker-Side Fine-Tuning and Evaluation

Attacker Fine-Tuning (Downstream SFT).
After receiving the released dataset, the attacker fine-tunes each target LVLM in a supervised manner. We evaluate six fine-tuning recipes that span the threat-model spectrum: LoRA, QLoRA, DoRA, projector-only, projector + LLM LoRA, and full fine-tuning. For LoRA-style adapters, we further sweep the rank over $\{8,16,64\}$. We additionally sweep the learning rate over $\{1{\times}10^{-5},4{\times}10^{-5},1{\times}10^{-4},3{\times}10^{-4}\}$, the number of training epochs over $\{1,2,4,8,10\}$, and the per-device batch size over $\{1,2,4\}$, with cosine schedule and warmup ratio $0.1$ held fixed. The training set is the protected version of each dataset’s standard split. Additional aggressive-attacker variants—input-modality sanitization (RCP, JPEG, Blur, Punct/Case/WS) and clean-data mixing at ratios $\{0,5,10,20,40,60,80,100\}\%$—are described in Sec. [6.4](#S6.SS4) and Appendix [I](#A9).

Evaluation Protocol.
Predictions are scored under each benchmark’s official answer convention. For multiple-choice datasets (MMStar, ScienceQA), a prediction is correct if the generated answer matches the ground-truth option letter after answer normalization (case folding, whitespace stripping, and prefix removal). For short-answer and free-form datasets (RealWorldQA, VQA-RAD, TextVQA, DocVQA), we use normalized exact match; when a dataset provides answer aliases, any alias is accepted as correct. The per-dataset answer formats are summarized in Appendix [E](#A5).

**Table 5: Hyperparameter tuning grid of MMGuard and the attacker SFT.**
| Group / Parameter | Values |
| --- | --- |
| *Surrogate fine-tuning (inner loop)* |  |
| LoRA rank | $8$ / $16$ |
| LoRA dropout / target modules | $0.1$ / all |
| Learning rate / per-device batch | $\{1{\times}10^{-5},1{\times}10^{-4}\}$ / $2$ |
| Inner steps $Q$ | $\{0,1,2,3,4,5\}$ |
| *Protection optimization (outer loop)* |  |
| Outer rounds $R$ | $\{1,3,5,7\}$ |
| Image budget $\epsilon_{x}$ ($\times 1/255$) | $\{1,2,4,8,16\}$ |
| PGD step $\alpha$ / iterations | $1/255$ / $\{0,1,2,5,8\}$ |
| Trigger length $\epsilon_{t}$ (tokens) | $\{1,3,5,7,9\}$ |
| HotFlip top-$K$ candidates | $\{8,16,64,128,512\}$ |
| $\lambda_{\mathrm{train}}$ | $1.0$ |
| $\lambda_{\mathrm{bind}}$ | $\{0.1,0.2,0.5,1.0,2.0\}$ |
| Binding-layer count $|\mathcal{K}|$ | $\{1,2,3,4,5,6,7\}$ |
| Black-box ensemble surrogates | Qwen3-VL-4B-Instruct, MiniCPM-V-4 |
| *Attacker SFT* |  |
| Fine-tuning recipe | {LoRA, QLoRA, DoRA, Projector, Projector+LLM LoRA, Full FT} |
| LoRA rank | $\{8,16,64\}$ |
| Learning rate | $\{1{\times}10^{-5},4{\times}10^{-5},1{\times}10^{-4},3{\times}10^{-4}\}$ |
| Scheduler / warmup ratio | cosine / $0.1$ |
| Epochs | $\{1,2,4,8,10\}$ |
| Batch size (per device) | $\{1,2,4\}$ |

## Appendix G Human Evaluation Rubric

To assess whether protected multimodal examples remain suitable for human interpretation and downstream use, we evaluate each image-text pair using a shared rubric across four dimensions: image naturalness, text naturalness, image-text coherence, and human answerability. Each dimension is rated on a three-point ordinal scale, where 1 denotes severe degradation, 2 denotes mild but tolerable degradation, and 3 denotes no noticeable degradation. Together, these dimensions capture perceptual image quality, linguistic fluency, cross-modal semantic alignment, and task-level usability.

Study Setup.
We conduct the study on $60$ protected image-text pairs in total, comprising $10$ randomly selected samples per dataset across the six benchmarks. Each pair is independently rated by three human experts and three commercial LLM judges: GPT-5.4, Gemini-3.1-Pro-Preview, and Claude-Sonnet-4-6 under the same four-dimensional rubric described below. The values reported in the *Human Study / LLM-as-a-Judge* block of Table [3](#S6.T3) are the mean per-dimension score across these six raters per sample. The rater-facing rubric and the LLM-judge prompt are described in the two subsubsections below.

### G.1 Rubric for Human Raters

Table [6](#A7.T6) formalizes the four dimensions for human raters, listing the precise definition and the three-point criteria for each.

**Table 6: Human evaluation rubric for protected multimodal examples.**
| Dimension | Definition | Criteria |
| --- | --- | --- |
| Image Naturalness | The degree to which the protected image preserves perceptual quality and visual plausibility, without noticeable perturbation-induced noise, distortion, color artifacts, texture irregularities, or other abnormal patterns that would make the image appear manipulated or low-quality to a human observer. | 1: The image contains clearly visible artifacts, corruption, or unnatural patterns that interfere with normal perception.<br>2: The image is generally recognizable and usable, but contains mild visual artifacts, slight distortion, or subtle abnormal patterns.<br>3: The image appears visually natural, clean, and indistinguishable from a normal, unmodified image. |
| Text Naturalness | The degree to which the protected text remains fluent, grammatical, readable, and contextually appropriate. Inserted or substituted tokens are penalized according to how much they disrupt readability and plausibility: clearly random, code-like, or semantically incoherent strings indicate low naturalness, while isolated awkward words or recognizable real-word/proper-noun fragments may still be understandable but stylistically unusual. | 1: The text is grammatically flawed, difficult to read, or contains clearly random, code-like, gibberish, or strongly suspicious tokens that noticeably disrupt natural reading.<br>2: The text remains understandable but includes minor awkwardness, unusual wording, single-word substitutions, or inserted recognizable words/proper nouns/phrase-like fragments that are stylistically abnormal but do not prevent comprehension.<br>3: The text is fluent, coherent, and appears naturally written without noticeable suspicious, irrelevant, or stylistically abnormal content. |
| Image-Text Coherence | The degree to which the protected image and protected text remain semantically aligned as a multimodal pair, such that the text refers to visual content present in the image and the image provides sufficient evidence for the textual input or question. | 1: The text is largely inconsistent with the image, refers to absent or contradictory visual content, or forms an incoherent multimodal pair.<br>2: The text is partially related to the image, but some visual references are vague, incomplete, or only weakly supported by the image.<br>3: The text is clearly and semantically consistent with the image, and the image provides appropriate visual evidence for the text or question. |
| Human Answerability | The degree to which a human observer can understand and respond to the intended task from the protected image-text pair, such as visual question answering, document understanding, or multimodal reasoning, without being hindered by perturbation-induced ambiguity or degradation. | 1: The question or task cannot be answered reliably from the protected pair due to visual degradation, textual ambiguity, or image-text mismatch.<br>2: The question or task is answerable, but with noticeable uncertainty caused by mild ambiguity, reduced clarity, or incomplete visual/textual evidence.<br>3: The question or task is clearly answerable from the protected pair, with sufficient visual and textual information for a confident human response. |

### G.2 LLM-as-a-Judge Prompt

For the automated evaluation, we instantiate the following LLM-as-a-judge prompt for each protected image-text pair. The model is instructed to rate each dimension independently according to the same ordinal rubric and to return only a JSON object for consistent parsing and aggregation. The prompt includes criterion-specific placeholders for the *Definition* and *Criteria* fields in Table [6](#A7.T6), ensuring that every judge receives the same rubric content.

## Appendix H Detailed Effectiveness Analysis

This section extends Sec. [6.2](#S6.SS2) with per-dataset, per-backbone, and per-variant observations. We organize the analysis into two parts: the post-fine-tuning accuracy panel in Fig. [3](#S6.F3), examined along the dataset, target, and variant axes; and the training-loss dynamics in Fig. [4](#S6.F4), examined along the optimization axis.

### H.1 Accuracy Analysis on Protected Data

Dataset Sensitivity.
The magnitude of the protection drop varies systematically with the role of the visual modality across datasets. Vision-heavy and OCR-driven benchmarks are most strongly protected on the white-box surrogate Qwen3-VL-4B: TextVQA falls by up to $\sim$$30$ points under the -Max variants (Clean FT $\sim$$87\%\!\to\!\sim$$58\%$), DocVQA by up to $\sim$$15$ points (Clean FT $\sim$$67\%\!\to\!\sim$$52\%$), and MMStar by up to $\sim$$17$ points (Clean FT $\sim$$67\%\!\to\!\sim$$50\%$). These tasks rely on dense visual evidence—scene text, document layout, vision-indispensable reasoning—so disrupting the cross-modal binding directly removes the dominant supervision signal. RealWorldQA exhibits a moderate drop ($9$–$18$ points white-box) on the same surrogate. In contrast, ScienceQA and VQA-RAD, where the language prior alone explains a substantial fraction of the answers, show the smallest absolute drops ($3$–$5$ points), although the protection still lowers accuracy below Clean FT in every cell. This pattern is consistent with the cross-modal binding hypothesis in Sec. [5.3](#S5.SS3): the protection’s leverage scales with how much the model must route generation through the perturbed image and inserted trigger.

White-Box vs. Gray-Box Targets.
Across all six datasets, the surrogate Qwen3-VL-4B receives the largest accuracy reduction, while the gray-box targets retain a smaller but consistent gap to Clean FT. Two effects are noticeable. First, the gap shrinks as the gray-box target diverges from the surrogate. Qwen3-VL-2B and Qwen3-VL-8B (same generation, different scale) preserve $4$–$10$ point drops on the vision-heavy MMStar, TextVQA, and DocVQA, while Qwen2.5-VL-7B (different generation, with a distinct visual encoder, projector, and chat template) drops by only 0–$5$ points, reflecting a transfer cost when the upstream stack differs from the surrogate. Second, on cells where Clean FT itself approaches the benchmark ceiling (e.g., $\sim$$88\%$ on TextVQA Q3-8B/Q2.5-7B and $\sim$$96\%$ on ScienceQA Q3-8B), the residual headroom is small, and the protection drop saturates near zero; this is a property of the upper reference rather than a transfer failure of the protection. Even on the most distant gray-box target, the four MMGuard variants still remain at or below Clean FT across datasets, indicating that the protection survives moderate architecture mismatch within the same model family without invoking the ensemble strategy of Sec. [5.4](#S5.SS4).

Variant-Level Comparison.
Three patterns hold consistently across datasets. (*i*) Under the white-box setting, the adversarial -Max variants produce the deepest drops, particularly on OCR-heavy tasks where direct training disruption is hardest for the attacker to absorb (TextVQA white-box: BPH-Max and CRS-Max reach $\sim$$58$ and $\sim$$62$ versus min-min BPH and CRS at $\sim$$78$ and $\sim$$73$). (*ii*) Under the gray-box setting, the min-min variants are marginally more stable, especially MMGuard-CRS, whose route-agnostic objective only requires the protected attention pattern to differ from the clean one rather than committing to a fixed bridge, and is therefore less sensitive to architectural differences. (*iii*) BPH and CRS produce comparable protection on average, but BPH benefits from the structural guarantee in Theorem [5.1](#S5.Thmtheorem1) when the surrogate matches the attacker’s fusion behavior, while CRS is the safer default when fusion behavior is unknown.

### H.2 Training Dynamics on Protected Data

Optimization Dynamics.
The training-loss curves in Fig. [4](#S6.F4) provide a direct mechanistic view of the two regimes. On every dataset, the min-min variants reach a final loss $1$–$2$ orders of magnitude below Clean (e.g., on MMStar, BPH and CRS converge below $10^{-3}$ while Clean settles around $10^{-2}$), confirming that the planted shortcut is not only learnable but *easier* to fit than the genuine image-text-answer association. The adversarial-Max variants instead form a high-loss plateau that never approaches the Clean curve (e.g., on DocVQA, BPH-Max and CRS-Max plateau near $10^{0}$–$10^{1}$, while Clean falls to $\sim$$0.14$). The plateaus are remarkably stable across training steps, which means the attacker cannot escape the disruption by extending the fine-tuning budget; this is a desirable property for a data-side defense, since it removes “train longer” as a trivial counter-strategy. The two failure modes, therefore, offer the defender a principled choice: when the attacker is expected to monitor training loss and discard high-loss samples, the min-min variant is preferable because the protected data appears trainable; when the attacker performs unmonitored fine-tuning at scale, the -Max variant is preferable because it directly inflates training cost without relying on shortcut adoption.

## Appendix I Detailed Transferability Analysis

This section expands Sec. [6.3](#S6.SS3) along two axes of the aggressive-attacker model: cross-model transferability against unseen target LVLMs (Table [2](#S6.T2)) and cross-recipe transferability against varying attacker fine-tuning strategies (Fig. [5](#S6.F5)).

### I.1 Cross-Model Transferability

Why Single-Modality Baselines Fail at Black-Box Transfer.
The Image-only and Text-only UE rows of Table [2](#S6.T2) contain several non-positive drops: e.g., Image protection *improves* attacker accuracy on RealWorldQA/Llama by $4.8$ points, and Text protection improves it by $0.9$ points; on ScienceQA/GLM, Image and Text similarly produce $-1.0$ and $-0.5$ point “drops.” These cases are not statistical noise but the predicted consequence of perturbing a single channel: when the attacker’s model relies on the unperturbed modality for that dataset, the perturbed modality contributes additional input regularization rather than protection. The naive Multimodal baseline, which simultaneously perturbs image and inserts a text trigger but does *not* disrupt cross-modal binding, mostly closes these negative drops but still produces only $1$–$5$ point reductions, well within the noise floor of pretraining variability. This empirically supports the challenge in the autoregressive LVLM setting, perturbing both modalities is necessary but not sufficient—attention can still route generation through whichever evidence remains semantically informative, so the defender must additionally constrain *how* the model binds the two modalities.

Per-Dataset Transfer Behavior.
The protection magnitude tracks how strongly each dataset depends on visual evidence. DocVQA produces the largest cross-model drop, reaching $-8.5$ points on GLM and LLaVA, because its document-image inputs carry dense layout and OCR signals that the binding disruption directly removes. MMStar (vision-indispensable reasoning) and VQA-RAD (medical imaging) follow with $4$–$8$ point drops across most targets. ScienceQA exhibits the smallest transfer drops ($1.5$–$7.5$ points), consistent with its strong language prior; the InternVL/ScienceQA cell, where Clean FT already reaches $99.0\%$, leaves little headroom for protection-induced degradation. RealWorldQA and TextVQA fall in between, with TextVQA particularly sensitive when the target has a different vision-encoder family (e.g., $-7.5$ points on Gemma).

Per-Target Transfer Behavior.
The five targets span maximally different LVLM stacks, yet all four MMGuard variants transfer with positive drops in every cell. Two patterns emerge. (*i*) The protection is largest on Gemma (mean drop $\sim$$5.7$ points), GLM ($\sim$$5.5$), Llama ($\sim$$5.1$), and LLaVA ($\sim$$5.0$), which are architecturally distant from both surrogates and offer moderate Clean FT performance and therefore sufficient headroom for attention-route disruption. (*ii*) Drops on InternVL are smaller in absolute terms ($\sim$$3.5$ points on average) because its Clean FT accuracy is already saturated on multiple cells (e.g., $99.0\%$ on ScienceQA, $76.5\%$ on TextVQA). When normalized by the clean-versus-zero-shot gain (i.e., the maximum protection budget), InternVL drop-offs are comparable to those of the other targets, indicating that protection consumes a similar fraction of the attacker’s fine-tuning gain across the panel.

Variant Selection at Black-Box.
No single MMGuard variant dominates across the table, but two trends inform variant choice. MMGuard-CRS is the most uniformly positive: it produces non-trivial drops in every cell, including the saturated InternVL columns. MMGuard-BPH-Max achieves the deepest individual drops on OCR-heavy tasks ($-7.5$ on LLaVA/DocVQA, $-7.6$ on GLM/VQA-RAD), consistent with the training-disruption mechanism being most damaging when the attacker relies most on visual fitting. The two min-min variants (BPH and CRS) are safer when the target architecture is unknown, since they do not rely on the surrogate-attacker fusion match required by the structural BPH bound (Theorem [5.1](#S5.Thmtheorem1)); the two adversarial variants are preferable when the attacker is expected to use full fine-tuning, where shortcut absorption is harder to enforce but training-loss inflation still applies.

### I.2 Cross-Recipe Transferability

Transferability Across Fine-Tuning Recipes.
Fig. [5](#S6.F5) stresses the recipe axis of the aggressive-attacker model. Across LoRA-8 to full fine-tuning, both MMGuard-BPH and MMGuard-CRS maintain lower accuracy than Clean FT across all datasets. Three observations are worth noting. First, the protection gap is largest under low-capacity adapters (projector and projector + LLM LoRA), where the attacker has limited parameters to absorb the planted shortcut and therefore commits to it during fitting; on TextVQA, projector-only fine-tuning drops from $\sim$$74\%$ (Clean FT) to $\sim$$65\%$ for BPH and $\sim$$69\%$ for CRS, the largest within-dataset gap in the panel. Second, full fine-tuning—the most resource-intensive recipe—narrows but does not close the gap: TextVQA and DocVQA still incur $5$–$15$ point drops, indicating that even with all parameters trainable, the attacker cannot fully separate the planted shortcut from the genuine image-text-answer association in the protected data. Third, QLoRA and DoRA behave similarly to LoRA-8/16/64, suggesting that the protection is robust to the choice of low-rank adapter family rather than tuned to a specific PEFT method. Together with the cross-model results, these observations show that MMGuard survives joint variation in the attacker model and attacker recipe, which captures the realistic black-box threat surface.

## Appendix J Detailed Practicality Analysis

This section expands Sec. [6.4](#S6.SS4) with a detailed analysis of the deployment costs incurred by the defender, covering perceptual stealthiness (Table [3](#S6.T3)) and computational overhead.

### J.1 Stealthiness and Computational Cost

Stealthiness Trade-offs.
Table [3](#S6.T3) reports per-modality stealthiness. On the image side, all variants that include image perturbation produce PSNR ${\approx}34.5$ dB, SSIM ${\approx}0.88$, LPIPS ${\approx}0.11$, and image-naturalness $2.52/3$, with no measurable cost for binding-disruption (BPH and CRS match Image-only UE). On the text side, the inserted trigger raises perplexity ($89$–$92$ vs. clean $39$) and lowers text-naturalness ($1.62$–$1.64$ vs. clean $2.86$), reflecting the visible token insertion; however, image-text coherence stays at $2.14/3$ and human answerability at $2.33/3$, both well above the failure threshold of the rubric. The dominant stealthiness cost therefore comes from text insertion rather than from image perturbation or binding-disruption, suggesting that future work on lower-perplexity trigger forms (e.g., paraphrastic insertion) is the most promising direction for tightening the stealthiness-protection trade-off.

Computational Cost Breakdown.
We analyze the computational and memory overhead of MMGuard in detail. Let $n$ be the number of protected samples, $M$ the number of surrogate LVLMs, $R$ the number of outer protection rounds, and $Q$ the number of inner adaptation steps used to approximate the attacker-side fine-tuning process. Let $L_{x}$, $L_{t}$, $L_{\gamma}$, and $L_{y}$ denote the numbers of image tokens, original text tokens, inserted trigger tokens, and response tokens, respectively, and define the total multimodal sequence length as $L=L_{x}+L_{t}+L_{\gamma}+L_{y}$. For surrogate model $m$, we denote the cost of one forward pass and one backward pass by $C_{\mathrm{fw}}^{(m)}$ and $C_{\mathrm{bw}}^{(m)}$, respectively.

In each outer round, MMGuard first performs $Q$ inner gradient steps on every surrogate model to approximate the model state obtained after unauthorized fine-tuning. This stage costs $O\!\left(nQ\sum_{m=1}^{M}(C_{\mathrm{fw}}^{(m)}+C_{\mathrm{bw}}^{(m)})\right)$. The subsequent outer update differentiates the protection objective with respect to the image perturbations and text triggers, incurring one additional forward/backward pass per surrogate, with cost $O\!\left(n\sum_{m=1}^{M}(C_{\mathrm{fw}}^{(m)}+C_{\mathrm{bw}}^{(m)})\right)$. Projection of the image perturbation onto the $\ell_{\infty}$ budget is linear in the number of pixels and is negligible compared with LVLM backpropagation.

The discrete trigger optimization adds a smaller candidate-search overhead. For each trigger position, the HotFlip-style screening step computes first-order scores over the admissible vocabulary $\mathcal{V}_{\mathrm{adm}}$. With embedding dimension $d$, this step costs $O(L_{\gamma}|\mathcal{V}_{\mathrm{adm}}|d)$ per sample. MMGuard then verifies only the top-$c$ candidates by exact loss evaluation, which costs $O\!\left(L_{\gamma}c\sum_{m=1}^{M}C_{\mathrm{fw}}^{(m)}\right)$ per sample per outer round. Since both $L_{\gamma}$ and $c$ are small fixed hyperparameters, this overhead is typically dominated by surrogate LVLM backpropagation.

The cross-modal binding loss reuses attention matrices already produced by the LVLM forward pass. For selected layers $\mathcal{K}$ and heads $\mathcal{H}$, aggregating the attention mass over a multimodal sequence of length $L$ costs $O(|\mathcal{K}||\mathcal{H}|L^{2})$ per forward pass. This does not change the asymptotic order of transformer attention, which is already quadratic in $L$. Combining these terms, the overall time complexity is

$$ $O\Bigg(Rn\sum_{m=1}^{M}\Big[(Q+1)\big(C_{\mathrm{fw}}^{(m)}+C_{\mathrm{bw}}^{(m)}\big)+L_{\gamma}cC_{\mathrm{fw}}^{(m)}+|\mathcal{K}||\mathcal{H}|L^{2}\Big]+RnL_{\gamma}|\mathcal{V}_{\mathrm{adm}}|d\Bigg).$ (49) $$

When surrogate models have comparable computational cost, i.e., $C_{\mathrm{fw}}^{(m)}+C_{\mathrm{bw}}^{(m)}\approx C_{\mathrm{LVLM}}$, and $L_{\gamma}$, $c$, $|\mathcal{K}|$, and $|\mathcal{H}|$ are treated as small constants, the dominant term simplifies to

$$ $O\!\left(RnM(Q+1)C_{\mathrm{LVLM}}\right).$ (50) $$

Therefore, the runtime scales linearly with the dataset size, the number of surrogate models, the number of outer rounds, and the number of inner adaptation steps.

We next analyze memory usage. The dominant memory cost comes from standard surrogate LVLM training, including model parameters, gradients, optimizer states, and backpropagation activations. Beyond this standard cost, MMGuard stores image perturbations, text triggers, and lightweight attention statistics. For images of resolution $H\times W$ with $C$ channels, perturbation storage costs $O(nHWC)$, while trigger storage costs $O(nL_{\gamma})$. The HotFlip update requires temporary token-gradient and candidate-score buffers, at most $O(L_{\gamma}d+L_{\gamma}|\mathcal{V}_{\mathrm{adm}}|)$ per processed sample. If the binding loss aggregates attention online, it only stores layer-wise attention-mass distributions, requiring $O(|\mathcal{K}|L)$ additional memory per sample rather than retaining all attention tensors. Thus, excluding standard LVLM training memory, the additional protection-specific storage is

$$ $O\!\left(nHWC+nL_{\gamma}+|\mathcal{K}|L\right),$ (51) $$

up to small temporary buffers for token screening and verification. This overhead is modest compared with the memory required for surrogate LVLM optimization. Finally, MMGuard is an offline defender-side preprocessing procedure: once the protected dataset is generated, it requires no auxiliary model, detector, or runtime optimization, and therefore introduces no inference-time overhead for legitimate users or downstream consumers.

## Appendix K Detailed Adaptive Attack Evaluation

This section expands Sec. [6.5](#S6.SS5) with a detailed analysis of MMGuard’s robustness against adaptive attackers along two axes: input-modality sanitization (Fig. [7](#S6.F7)) and partial-coverage data mixing (Fig. [6](#S6.F6)).

### K.1 Robustness Against Aggressive Attackers

Per-Operator Robustness.
Fig. [7](#S6.F7) sweeps ten attacker-side data-preparation choices: no defense (None), three image-side operators (RCP, JPEG, Blur), three text-side operators (punctuation removal, case normalization, whitespace normalization, denoted Punct/Case/WS), and three image-text combinations (RCP+Punct, JPEG+Case, Blur+WS). Three patterns are visible. (*i*) Image-side operators applied to the Clean baseline already cause moderate accuracy loss (e.g., on TextVQA, Clean drops from $\sim$$87$ with no defense to $\sim$$77$ under RCP/JPEG), confirming that these operators are not benign even for clean training data; the relevant comparison is therefore against *Clean under the same operator*, not against unprotected Clean. (*ii*) Under this fair comparison, BPH and CRS still produce $4$–$10$ point drops on TextVQA and DocVQA across every image-side operator, indicating that the protection survives bounded image purification. (*iii*) Text-side operators (Punct, Case, WS) are largely no-ops because the inserted trigger consists of admissible vocabulary tokens whose surface form is preserved by these normalizers; combos are correspondingly close to their image-side parent. The empirical message is that a defense-aware attacker cannot wash out MMGuard by stacking standard sanitizers, because the protection lives in the attention route between perturbation and trigger, not in pixel-space or surface-string features that purification removes.

Dosage Curves.
Fig. [6](#S6.F6) sweeps the protection ratio over $\{0,5,10,20,40,60,80,100\}\%$. Three observations are worth noting. (*i*) Vision-heavy datasets (TextVQA, DocVQA) show the steepest dosage response: TextVQA accuracy under BPH falls from $\sim$$75\%$ at the lowest ratio to $\sim$$71\%$ at full coverage, and DocVQA from $\sim$$61\%$ to $\sim$$56\%$, while the Clean Fine-Tuning reference remains flat by construction. (*ii*) Language-prior-heavy datasets (ScienceQA, MMStar, RealWorldQA) show a smaller but consistent decline ($2$–$4$ point gap to Clean Fine-Tuning), with most of the protection effect delivered already at moderate dosages ($20$–$40\%$). (*iii*) The curves are smooth rather than thresholded, indicating that the planted shortcut acts as a graded contamination signal rather than an all-or-nothing backdoor. In practice, a defender who controls only a portion of the attacker’s training corpus still obtains useful protection, and a defender who saturates the corpus achieves the largest gap to clean fine-tuning.

## Appendix L Detailed Mechanism Analysis

This section expands Sec. [6.6](#S6.SS6) with two complementary mechanism analyses: a quantitative study of component ablations and parameter sensitivity (Fig. [8](#S6.F8)), followed by a qualitative visualization of the cross-modal binding routes induced on a representative sample (Fig. [9](#S6.F9)).

### L.1 Ablation and Parameter Sensitivity

Component Decomposition (Ablation).
The leftmost panel of Fig. [8](#S6.F8) reports clean-test accuracy after attacker fine-tuning under nine conditions; lower numbers indicate stronger protection. The full MMGuard-BPH ($\sim$$66$) is the strongest, and removing components produces the following pattern: Image-only UE ($\sim$$71$), Text-only UE ($\sim$$73$), and Multimodal without binding ($\sim$$70$) confirm that the binding-disruption objective is responsible for the $4$–$7$ point gap to BPH that input-space perturbation alone cannot deliver. Within BPH, ablating the image perturbation (*w/o Pert.*, $\sim$$72$) is more harmful than ablating the trigger (*w/o Trig.*, $\sim$$69$), reflecting that perturbation-carrying tokens are the main attention sink that anchors the planted shortcut, whereas the trigger is the discrete switch that activates it. Among the three binding-path ablations, removing the answer-to-trigger and answer-to-perturbation paths costs the most ($\sim$$+2$ points each), while removing the trigger-to-perturbation coupling costs the least ($\sim$$+1$). This ranking matches the structural argument in Theorem [5.1](#S5.Thmtheorem1): the answer-anchored paths are the ones whose target sets are empty on clean inputs and therefore force the protection-time-versus-clean-time TV shift, while the trigger-to-perturbation coupling stitches the two modalities together but does not directly produce that shift.

Parameter Sensitivity.
We discuss each of the five parameters in turn.
*(i) Binding-loss weight $\lambda_{\mathrm{bind}}$.* BPH peaks at $\lambda_{\mathrm{bind}}{=}2.0$ ($\sim$$63$) but is non-monotonic earlier (approximately $\{65,65,66,68,63\}$ over $\lambda_{\mathrm{bind}}{\in}\{0.1,0.2,0.5,1.0,2.0\}$), reflecting interaction with the training-loss term: at moderate weights, the bridge competes with $\ell_{\mathrm{train}}$, and only at $\lambda_{\mathrm{bind}}{\geq}2.0$ does the bridge dominate. CRS, in contrast, is monotonically improving and reaches $\sim$$54$ at $\lambda_{\mathrm{bind}}{=}2.0$, because its route-agnostic objective never over-prescribes a target and therefore tolerates a heavier weight without sacrificing fitting capacity.
*(ii) Inner-loop steps $Q$.* $Q{=}0$ removes the inner adaptation entirely and gives the weakest protection ($\sim$$73$ for BPH and $\sim$$72$ for CRS), confirming the necessity of the bilevel structure in Eq. ([34](#S5.E34)). $Q{=}1$ is the elbow ($\sim$$65$ and $\sim$$64$); larger $Q$ slightly degrades protection because the surrogate over-fits the protected sample during inner adaptation, leaving less signal for the outer perturbation/trigger update.
*(iii) Gradient layer depth $|\mathcal{K}|$.* The protection is strongest at depth $1$ ($\sim$$64$ for BPH and $\sim$$63$ for CRS) and weakest at depth $7$ ($\sim$$68$ and $\sim$$67$). This is consistent with the observation that cross-modal binding occurs early in the language stack: deeper layers remix tokens, and constraining attention there does not effectively redirect the answer route. In practice, restricting $\mathcal{K}$ to the first one or two attention layers is strictly preferable along both effectiveness and efficiency axes.
*(iv) Image budget $\epsilon_{x}$.* Accuracy curves are roughly piecewise: $\epsilon_{x}{\in}\{1,2\}/255$ delivers little protection ($\sim$$74$–$78$), $\epsilon_{x}{\in}\{4,8\}/255$ delivers strong protection ($\sim$$64$–$66$), and $\epsilon_{x}{=}16/255$ delivers the deepest drop but at substantial perceptual cost ($\sim$$46$–$48$ accuracy with visible artifacts). The setting $\epsilon_{x}{=}8/255$ lies at the elbow of the effectiveness-versus-PSNR/LPIPS curve.
*(v) Text budget $\epsilon_{t}$.* A single inserted token gives only weak protection ($\sim$$65$ for BPH and $\sim$$61$ for CRS); three to five tokens form a stable plateau ($\sim$$64$–$65$); and seven tokens produce a sharp gain ($\sim$$56$ and $\sim$$46$). The plateau corresponds to the trigger reaching enough capacity to encode the discrete switch reliably, while the seven-token jump corresponds to the trigger acquiring enough redundancy to survive different surrogate tokenizations. The setting $\epsilon_{t}{=}5$ keeps text-naturalness and image-text-coherence ratings competitive (Table [3](#S6.T3)); a defender willing to accept higher perplexity can push $\epsilon_{t}$ to $7$ for an additional $\sim$$10$ point drop.

### L.2 Cross-Modal Binding Visualization

Row-by-Row Mechanism Diagnostics.
Fig. [9](#S6.F9) dissects the protection on the representative sample (*“Is there a dedicated bicycle lane?”*, answer *Yes*) along four rows.

*Row 1 (released image).* The released images for MMGuard-BPH and MMGuard-CRS are visually indistinguishable from the clean reference and differ only by a short inserted trigger (*‘Describe’* for BPH, *‘Pent’* for CRS). This is consistent with the perceptual budget $\epsilon_{x}{=}8/255$ and trigger-length budget $\epsilon_{t}{=}5$ used by default, and supports the input-side stealthiness numbers in Table [3](#S6.T3) on a per-sample basis. Different surrogates also pick different trigger tokens, reflecting that the HotFlip-style search in Eq. ([19](#S5.E19))–([20](#S5.E20)) optimizes against the local protection objective rather than relying on a fixed trigger string.

*Row 2 (image perturbation $\Omega_{\delta}$).* The cyan boxes mark the perturbation-heavy patches selected by Eq. ([23](#S5.E23)) at perturbation-token ratio $\tau_{\delta}$. The protection signal is concentrated on a structured subset of patches rather than spread uniformly across the image, which is what makes $\Omega_{\delta}$ informative as a binding target: a uniform perturbation would not induce a localized attention sink, whereas the structured pattern provides discrete keys that the binding objective can route attention onto. The two variants produce different $\Omega_{\delta}$ layouts under the same budget, indicating that the location of the perturbation is selected by the joint optimization rather than fixed in advance.

*Row 3 (answer attention map).* On the clean input, answer-token attention concentrates on the lower road and lane region, the genuine visual evidence for the bicycle-lane question, which corresponds to $\Omega_{x}\!\setminus\!\Omega_{\delta}$ in the notation of Eq. ([21](#S5.E21)). Under both MMGuard-BPH and MMGuard-CRS, the same attention is pulled away from this region and onto off-content positions in the upper sky/streetlight area that coincide exactly with $\Omega_{\delta}$. This is a per-sample visualization of the attention reallocation that the binding objective in Sec. [5.3](#S5.SS3) is designed to induce, and it confirms that the protection not only suppresses generalization in aggregate metrics but also rewires the route taken by individual answer tokens.

*Row 4 (quantitative routing).* The leftmost bar chart compares head-averaged attention mass on five token-pair categories: answer-to-trigger, answer-to-perturbation, trigger-to-perturbation, answer-to-non-perturbation-image, and answer-to-text. On clean inputs, $\Omega_{\gamma}$ and $\Omega_{\delta}$ are empty by construction (no insertion, no perturbation-magnitude ranking), so the first three categories are identically zero. Under protection, all three rise to nonzero values, while attention to non-perturbation image tokens is suppressed, and attention to the original text is largely preserved. This decomposition is exactly the reallocation Theorem [5.1](#S5.Thmtheorem1) requires for the clean-time TV shift: protection mass is gained on $\Omega_{\gamma}$ and $\Omega_{\delta}$, while the clean-side semantic route through $\Omega_{x}\!\setminus\!\Omega_{\delta}$ loses mass, leaving the model with an attention pattern that cannot be reused on clean inputs, where $\Omega_{\gamma}$ and $\Omega_{\delta}$ are absent.

The two-layerwise curves further reveal a complementary specialization between the variants. MMGuard-BPH concentrates its answer-to-trigger mass in early-to-mid language-model layers (peaking near layer $15$), which is consistent with the prescribed answer-trigger bridge in Eq. ([27](#S5.E27)): the $\beta_{2}$ term explicitly anchors the answer onto the trigger, and the trigger onto the perturbation, and trigger anchoring is most natural in early layers where text tokens are syntactically grounded. MMGuard-CRS, in contrast, produces a substantially larger answer-to-perturbation mass in late vision-fusion layers (peaking near layer $30$), which is consistent with its route-agnostic objective in Eq. ([29](#S5.E29)): rather than prescribing a specific path, CRS only requires the protected attention pattern to differ from the clean one, and the model finds the path of least resistance, which in modern LVLMs is the late vision-fusion route through $\Omega_{\delta}$. The two designs therefore induce a comparable distribution-level KL/TV shift through structurally different routes, which is why their effectiveness is similar on average yet they trade off differently against architecturally distant attackers (Sec. [6.3](#S6.SS3)).

### L.3 Protection Examples

Figures [10](#A12.F10)–[15](#A12.F15) provide per-sample qualitative views of MMGuard protection across the six benchmark datasets used in our evaluation, complementing the aggregate stealthiness metrics in Table [3](#S6.T3) with direct visual evidence at the released-image level.

Figure: Figure 10: Per-sample protection examples on RealWorldQA. Rows (top to bottom): Clean, MMGuard-BPH, MMGuard-CRS, MMGuard-BPH-Max, and MMGuard-CRS-Max. Columns: samples selected randomly to span the dataset’s aspect-ratio range. Within each row, cells share a fixed pixel height and are concatenated edge-to-edge at native aspect; within each column, the same source image appears under all five protection variants, so perturbation-induced differences can be directly compared with the clean reference. All protected images are generated at the default perceptual budget $\epsilon_{x}{=}8/255$. Driving and street-scene cues (e.g., lane geometry, vehicles, traffic lights, and signage) are preserved under all four protected variants.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x10.png

Figure: Figure 11: Per-sample protection examples on MMStar. Layout follows Fig. [10](#A12.F10). The discriminative content required by vision-indispensable reasoning, including chart axes, color coding, and fine geometric structure, remains legible across all four protected variants.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x11.png

Figure: Figure 12: Per-sample protection examples on ScienceQA. Layout follows Fig. [10](#A12.F10). Diagrammatic primitives that carry the question-relevant signal in scientific figures, such as lines, arrows, axes, and regional shading, are preserved across all four protected variants.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x12.png

Figure: Figure 13: Per-sample protection examples on VQA-RAD. Layout follows Fig. [10](#A12.F10). On grayscale clinical radiographs, where diagnostic information is concentrated in low-contrast tissue boundaries, the protected images retain the global anatomical layout and the salient contrast structure of the originals.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x13.png

Figure: Figure 14: Per-sample protection examples on TextVQA. Layout follows Fig. [10](#A12.F10). Embedded scene text and signage—the question-relevant signal for OCR reasoning—remain legible across all four protected variants.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x14.png

Figure: Figure 15: Per-sample protection examples on DocVQA. Layout follows Fig. [10](#A12.F10). Document layout, including paragraph structure, tables, and form fields, is preserved across all four protected variants.
Refer to caption: https://arxiv.org/html/2605.14291v1/2605.14291v1/x15.png

Visual Faithfulness at the Default Budget.
Across all six datasets, the four protected rows are difficult to distinguish from the clean reference at the default budget $\epsilon_{x}{=}8/255$. Residual differences manifest as low-amplitude high-frequency texture rather than as content loss, and the dominant semantic content of each cell is preserved row by row. This per-sample observation is consistent with the aggregate input-side stealthiness numbers reported in Table [3](#S6.T3)—PSNR ${\approx}34.5$ dB, SSIM ${\approx}0.88$, LPIPS ${\approx}0.11$, and image-naturalness $2.52/3$—and supports the interpretation in Sec. [5.3](#S5.SS3) that the protection signal is encoded as a structured pixel-space residual that is read out only at the attention-binding level of the LVLM, rather than as a visible artifact in the released image.

Domain-Specific Observations.
The six datasets stress different visual statistics, yet the same $\epsilon_{x}{=}8/255$ budget remains visually acceptable across all of them. RealWorldQA preserves the driving and street-scene cues that the dataset’s spatial-relation questions depend on, including lane geometry, vehicles, traffic lights, and signage. MMStar retains the chart axes, color coding, and fine geometric structure required by its vision-indispensable reasoning items. ScienceQA preserves the diagrammatic primitives such as lines, arrows, axes, and regional shading that carry the question-relevant signal in scientific figures. VQA-RAD is the most demanding domain because the diagnostic content of grayscale radiographs is concentrated in low-contrast tissue boundaries; nevertheless, the protected images retain the global anatomical layout and the salient contrast structure of the originals. TextVQA keeps embedded scene text and signage legible, which matters because the dataset requires reading text in the image. DocVQA preserves document structure, including paragraph layout, tables, and form fields, which carry the question-relevant signal for document understanding. Together, these per-dataset views indicate that a single domain-agnostic perceptual budget suffices for both consumer-grade web imagery and specialized modalities such as clinical radiology and high-resolution document scans, removing the need for dataset-specific tuning of $\epsilon_{x}$ at deployment.

Variant Comparison.
Within each dataset, the four protected rows (MMGuard-BPH, MMGuard-CRS, MMGuard-BPH-Max, and MMGuard-CRS-Max) are visually comparable to one another. This is expected: all four variants share the same $\ell_{\infty}$ image budget $\epsilon_{x}{=}8/255$ and the same admissible trigger vocabulary, so the only difference between rows is the surrogate-side objective that drove the optimization: route-prescribed binding (BPH; Sec. [5.3](#S5.SS3)), route-agnostic divergence (CRS; Sec. [5.1](#S5.Thmtheorem1)), or their adversarial -Max counterparts (Sec. [5.5](#S5.SS5)). The choice between min-min and -Max regimes, therefore, reflects an operational decision about the assumed attacker behavior rather than a perceptual trade-off, and the choice between BPH and CRS reflects a structural design decision about whether to commit to a specific cross-modal route. The fact that all four variants produce comparably faithful images under a uniform budget supports the use of MMGuard as a single-knob defense at deployment, where the defender selects a variant based on the threat model without retuning the perceptual budget.

## Appendix M Use of Generative AI

We employed the GPT-5.5 and Opus-4.7 models solely as language-polishing tools to improve clarity and readability. Their role was limited to proofreading, grammatical correction, and stylistic refinement—functions analogous to those provided by traditional grammar checkers and reference dictionaries. These tools did not generate new scientific content or ideas, and their use is consistent with standard practices for manuscript preparation.
