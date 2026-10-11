//! T1 (Э2): embedded model registry — DATA describing every GGUF critic
//! model this tool may load. No model bytes are committed; the registry
//! binds stable ids to on-disk paths and the exact prompt scaffolding each
//! family needs (chat template tokens are NOT added automatically: the
//! legacy `CandleTokenizer::encode` runs with `add_special_tokens=false`,
//! and `decode` skips special tokens, so BOS/EOS control strings must be
//! part of the rendered prompt text itself).

use serde::Serialize;

/// One registered GGUF critic model.
#[derive(Debug, Clone, Serialize)]
pub struct ModelSpec {
    /// Stable registry id (`--model tinyllama-1.1b-chat`).
    pub id: &'static str,
    /// Directory holding `tokenizer.json` and the `.gguf` file.
    pub dir: &'static str,
    /// GGUF file name inside `dir`.
    pub gguf: &'static str,
    /// Model context length (tokens) — also caps the prompt budget.
    pub ctx: u32,
    /// Quantization label for reports.
    pub quant: &'static str,
    /// Architecture family; `load_gguf` accepts only llama/mistral layouts.
    pub arch: &'static str,
    /// Rendered chat scaffold: `{system}`/`{user}` placeholders, includes
    /// the family's BOS and turn-delimiter control tokens verbatim.
    pub chat_template: &'static str,
    /// EOS handling: literal stop strings that terminate generation
    /// (special-token stops never fire through the skipping decoder, so
    /// families that end turns with a non-EOS token need a text stop).
    pub stop_sequences: &'static [&'static str],
    /// What this model is expected to contribute (audit note, not code).
    pub expected: &'static str,
    /// Whether this model is expected to separate well-formed from
    /// degraded profiles (drives the discrimination test; a probe-grade
    /// model that answers canned verdicts is a documented finding, not a
    /// test failure).
    pub discriminates: bool,
}

/// Zephyr-style scaffold used by TinyLlama-1.1B-Chat-v1.0 (ChatML-v1 tags,
/// `</s>` turn ends; BOS `<s>` prepended because encode adds nothing).
const TINYLLAMA_TEMPLATE: &str =
    "<s><|system|>\n{system}</s>\n<|user|>\n{user}</s>\n<|assistant|>\n";

/// Llama-3 instruct scaffold (`<|eot_id|>` ends turns; BOS is
/// `<|begin_of_text|>`). Generation is capped by max_tokens: the skipping
/// decoder hides `<|eot_id|>` from the stop check, so we also stop on a
/// closing brace to cut CPU time once the JSON verdict is complete.
const LLAMA3_TEMPLATE: &str = "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{system}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{user}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n";

/// The registry. Paths live under /root/models (not committed, not repo DATA
/// in the da-* sense — this standalone tool owns its model catalog).
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "tinyllama-1.1b-chat",
        dir: "/root/models/tinyllama-1.1b-chat-v1.0",
        gguf: "tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf",
        ctx: 2048,
        quant: "Q4_K_M",
        arch: "llama",
        chat_template: TINYLLAMA_TEMPLATE,
        stop_sequences: &["</s>", "}"],
        expected: "pipeline probe only; measured NOT to discriminate structural profiles \
(canned 'good' verdicts for all-zero profiles too) - see the E2 report",
        discriminates: false,
    },
    ModelSpec {
        id: "llama-3.2-1b-instruct",
        dir: "/root/models/llama-3.2-1b-instruct",
        gguf: "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
        ctx: 8192,
        quant: "Q4_K_M",
        arch: "llama",
        chat_template: LLAMA3_TEMPLATE,
        stop_sequences: &["<|eot_id|>", "}"],
        expected: "primary 1B critic; separates well-formed from degraded profiles \
(0.8/good vs 0.0/bad on the built-in boundary fixtures)",
        discriminates: true,
    },
];

/// Look a model up by registry id, or accept a direct path.
pub fn resolve(model: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_unique_and_paths_absolute() {
        let mut seen = std::collections::HashSet::new();
        let mut discriminators = 0;
        for m in MODELS {
            assert!(seen.insert(m.id), "duplicate id {}", m.id);
            assert!(m.dir.starts_with('/'), "{} dir not absolute", m.id);
            assert!(m.chat_template.contains("{user}"));
            assert!(m.chat_template.contains("{system}"));
            if m.discriminates {
                discriminators += 1;
            }
        }
        // at least one registered model must carry the discrimination duty
        assert!(discriminators >= 1);
    }

    #[test]
    fn templates_carry_their_family_bos() {
        for m in MODELS {
            let has_bos = m.chat_template.starts_with("<s>")
                || m.chat_template.starts_with("<|begin_of_text|>");
            assert!(has_bos, "{} template lacks BOS control token", m.id);
        }
    }
}
