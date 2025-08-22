// affinity.rs
use core_affinity;

pub fn pin_to_core(core_id: usize) {
    if let Some(cores) = core_affinity::get_core_ids() {
        if let Some(core) = cores.get(core_id) {
            core_affinity::set_for_current(*core);
            println!("Pinned thread to core {}", core_id);
        } else {
            eprintln!("Core {} not available", core_id);
        }
    } else {
        eprintln!("Could not fetch core IDs");
    }
}
