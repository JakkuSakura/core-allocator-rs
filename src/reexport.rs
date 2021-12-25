#[cfg(not(target_os = "macos"))]
pub use nix::sched::CpuSet;

#[cfg(target_os = "macos")]
#[derive(Copy, Clone)]
pub struct CpuSet;

#[cfg(target_os = "macos")]
impl CpuSet {
    pub fn new() -> Self {
        Self
    }
    pub fn count() -> usize {
        // we usually have 8 cores on a macbook?
        8
    }
    pub fn set(&mut self, _i: usize) -> Option<()> {
        Some(())
    }
}
