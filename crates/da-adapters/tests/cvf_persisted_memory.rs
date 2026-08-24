// witness-verify the persisted summarizer memory through the real adapter
#[test]
fn verify_persisted_summarizer_memory() {
    use da_adapters::agent_memory::CvfAgentMemory;
    use da_ports::agent_memory::AgentMemory as _;
    let m = CvfAgentMemory::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/agent_memory/summarizer").into()).unwrap();
    let s = m.stats();
    println!("epochs={} trajectories={} verify={}", s.epochs, s.trajectories, m.verify());
    assert_eq!(s.trajectories, 2);
    assert!(m.verify());
}
