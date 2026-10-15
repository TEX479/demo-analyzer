#![warn(clippy::todo)]
#![deny(clippy::panic, clippy::unwrap_used)]

use std::sync::atomic::AtomicU32;
pub mod base {
    pub mod cheat_analyser_base;
    pub mod demo_handler_base;
}

pub mod chat_view;
pub mod header_view;
pub mod playerlist_view;
#[cfg(feature = "rcon")]
pub mod rcon;

#[cfg(feature = "static_algorithms_module")]
pub mod algorithms;
#[cfg(not(feature = "static_algorithms_module"))]
pub mod algorithms {
    // Stable and experimental modules are auto-discovered by build.rs at compile time.
    // To add, remove, or move an algorithm between folders, simply place its .rs file
    // in src/algorithms/, src/algorithms/fidoo/, src/algorithms/nocrex/,
    // src/algorithms/experimental/, or src/algorithms/fidoo_experimental/
    // — no manual module declarations or code changes needed here.
    include!(concat!(env!("OUT_DIR"), "/stable_mods.rs"));
    include!(concat!(env!("OUT_DIR"), "/experimental_mods.rs"));
}

pub mod util {
    // bsp module is not being used, thus it can be deactivated
    // pub mod bsp;
    pub mod datatable_validator;
    pub mod helpers;
    pub mod schema_equip_regions;
    pub mod nocrex {
        pub mod jankguard;
    }
}

pub mod lib {
    pub mod algorithm;
    #[cfg(feature = "parallel")]
    pub mod algorithm_costs;
    #[cfg(has_experimental_exclusions)]
    pub mod excluded_algorithms_experimental;
    pub mod excluded_algorithms_stable;
    pub mod general_settings;
    pub mod parameters;
}

#[cfg(feature = "live_analyzer")]
pub mod live_analyzer;

pub static SILENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub const MAX_WORKERS: usize = 32;
pub static WORKER_TICKS: [AtomicU32; MAX_WORKERS] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
pub static WORKER_COUNT: AtomicU32 = AtomicU32::new(1);
pub static PROGRESS_CURRENT: AtomicU32 = AtomicU32::new(0);
pub static PROGRESS_TOTAL: AtomicU32 = AtomicU32::new(0);

pub fn reset_progress(num_workers: usize) {
    let workers = num_workers.clamp(1, MAX_WORKERS) as u32;
    WORKER_COUNT.store(workers, std::sync::atomic::Ordering::Relaxed);
    PROGRESS_CURRENT.store(0, std::sync::atomic::Ordering::Relaxed);
    PROGRESS_TOTAL.store(0, std::sync::atomic::Ordering::Relaxed);
    WORKER_TICKS.iter().for_each(|t| t.store(0, std::sync::atomic::Ordering::Relaxed));
}

pub fn get_average_progress() -> (u32, u32) {
    let count = WORKER_COUNT.load(std::sync::atomic::Ordering::Relaxed).max(1) as usize;
    let total = PROGRESS_TOTAL.load(std::sync::atomic::Ordering::Relaxed);
    let sum: u64 = WORKER_TICKS
        .iter()
        .take(count.min(MAX_WORKERS))
        .map(|w| w.load(std::sync::atomic::Ordering::Relaxed) as u64)
        .sum();
    let avg = (sum / count as u64) as u32;
    (avg, total)
}

#[macro_export]
macro_rules! dev_print {
    ($($arg:tt)*) => {
        if !$crate::SILENT.load(std::sync::atomic::Ordering::Relaxed) {
            println!($($arg)*);
        }
    }
}
