//! Runs the shared `SpillStore` contract suite (`ferrum-testkit`) against both implementations.

use ferrum_scrollback::{FileSpill, MemorySpill, SpillStore};
use ferrum_testkit::spill_contract;

#[test]
fn memory_spill_honors_the_contract() {
    spill_contract::run(&|capacity| Box::new(MemorySpill::new(capacity)));
}

#[test]
fn file_spill_honors_the_contract() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let make = |capacity: u64| -> Box<dyn SpillStore> {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("ferrum-contract-{}-{id}.spill", std::process::id()));
        Box::new(FileSpill::new(path, capacity))
    };
    spill_contract::run(&make);
}
