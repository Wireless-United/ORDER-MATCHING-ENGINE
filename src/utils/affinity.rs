// Placeholder for CPU affinity utilities

#[allow(dead_code)]
pub fn set_thread_affinity(_core_id: usize) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[allow(dead_code)]
pub fn get_cpu_count() -> usize {
    // Use available_parallelism from std::thread instead of num_cpus
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}
