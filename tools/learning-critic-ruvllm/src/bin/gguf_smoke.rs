//! Live smoke test: load a GGUF model on CPU via ruvllm CandleBackend,
//! generate, report load time and generation throughput.
//! Usage: gguf-smoke --model <path.gguf> [--prompt "ping"] [--max-tokens 32]

use ruvllm::{CandleBackend, DeviceType, GenerateParams, LlmBackend, ModelConfig};
use std::path::Path;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let model = args
        .iter()
        .position(|a| a == "--model")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("--model <path> required"))?;
    let prompt = args
        .iter()
        .position(|a| a == "--prompt")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "Reply with exactly: SMOKE-OK".into());
    let max_tokens: usize = args
        .iter()
        .position(|a| a == "--max-tokens")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(32);

    let t0 = Instant::now();
    let mut backend = CandleBackend::with_device(DeviceType::Cpu)?;
    let t_dev = t0.elapsed();
    backend.load_gguf(Path::new(&model), &ModelConfig::default())?;
    let t_load = t0.elapsed();
    println!("device_init={t_dev:?}");
    println!("gguf_load_ms={}", t_load.as_millis());

    let t1 = Instant::now();
    let text = backend.generate(
        &prompt,
        GenerateParams {
            max_tokens,
            temperature: 0.2,
            top_p: 0.9,
            ..Default::default()
        },
    )?;
    let t_gen = t1.elapsed();
    println!(
        "generation: {t_gen:?} | text: {}",
        text.trim().chars().take(200).collect::<String>()
    );
    Ok(())
}
