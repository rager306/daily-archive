//! Live smoke test: load a GGUF model on CPU via ruvllm CandleBackend,
//! generate, report load time and generation throughput.
//!
//! `--model` takes either a directory holding `tokenizer.json` + a `.gguf`
//! file, or a bare `.gguf` path (tokenizer then expected next to it).
//! Usage: gguf-smoke --model <dir|path.gguf> [--prompt "..."] [--max-tokens 32]

use ruvllm::{CandleBackend, DeviceType, GenerateParams, LlmBackend, ModelConfig};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let model = flag("--model").ok_or_else(|| anyhow::anyhow!("--model <path> required"))?;
    let prompt = flag("--prompt").unwrap_or_else(|| "Reply with exactly: SMOKE-OK".into());
    let max_tokens: usize = flag("--max-tokens")
        .and_then(|v| v.parse().ok())
        .unwrap_or(32);

    let t0 = Instant::now();
    let mut backend = CandleBackend::with_device(DeviceType::Cpu)?;
    let t_dev = t0.elapsed();
    backend.load_model(&model, ModelConfig::default())?;
    let t_load = t0.elapsed();
    println!("device_init_ms={}", t_dev.as_millis());
    println!("load_ms={}", t_load.as_millis());

    let t1 = Instant::now();
    let text = backend.generate(
        &prompt,
        GenerateParams {
            max_tokens,
            temperature: 0.0, // greedy: deterministic ArgMax sampling
            top_p: 1.0,
            top_k: 0,
            repetition_penalty: 1.0,
            stop_sequences: Vec::new(),
            seed: Some(42),
            ..Default::default()
        },
    )?;
    let t_gen = t1.elapsed();
    let tps = text.chars().filter(|c| !c.is_whitespace()).count() as f64
        / 4.0
        / t_gen.as_secs_f64().max(1e-9);
    println!("gen_ms={}", t_gen.as_millis());
    println!("approx_tok_s={tps:.1}");
    println!(
        "text: {}",
        text.trim().chars().take(400).collect::<String>()
    );
    Ok(())
}
