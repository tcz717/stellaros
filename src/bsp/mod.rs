#[cfg(target_arch = "aarch64")]
mod virt;

#[cfg(target_arch = "aarch64")]
pub use virt::*;
