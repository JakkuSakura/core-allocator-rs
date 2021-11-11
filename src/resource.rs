use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, TryLockError, TryLockResult};

#[derive(Clone)]
pub struct Resource {
    taken: Arc<AtomicBool>,
}
impl Resource {
    pub fn new() -> Resource {
        Resource {
            taken: Default::default(),
        }
    }
    pub fn is_taken(&self) -> bool {
        self.taken.load(Ordering::Relaxed)
    }
    pub fn try_lock(&self) -> TryLockResult<ResourceHandle> {
        if self
            .taken
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            == Ok(false)
        {
            Ok(ResourceHandle(self.clone()))
        } else {
            Err(TryLockError::WouldBlock)
        }
    }
}
pub struct ResourceHandle(Resource);

impl Drop for ResourceHandle {
    fn drop(&mut self) {
        self.0.taken.store(false, Ordering::Relaxed)
    }
}
