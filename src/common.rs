
pub use slog::{info, warn, error, debug, trace, o};
pub use anyhow::{Result, anyhow, bail};
pub use confomat::*;

pub fn sleep(s: u64) {
    std::thread::sleep(std::time::Duration::from_secs(s));
}
