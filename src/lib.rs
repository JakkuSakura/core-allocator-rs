mod allocators;
mod bind_core;
mod reexport;
mod resource;

pub(crate) use resource::*;

pub use allocators::*;
pub use reexport::*;

use std::fmt::{Debug, Formatter};

use crate::bind_core::{bind_to_cpu_set, to_cpu_set};
use anyhow::Result;
use anyhow::{anyhow, Context};
use log::*;

use std::marker::PhantomData;
use std::mem::forget;
use std::rc::Rc;
use std::sync::{Arc, TryLockError};

pub trait CoreAllocator: Sync {
    fn allocate_core(&self) -> Option<CoreGroup>;
}

pub struct CoreGroup {
    _lock: Option<ResourceHandle>,
    inner: CoreGroupInner,
}

impl CoreGroup {
    pub fn any_core() -> Self {
        Self {
            _lock: None,
            inner: CoreGroupInner::AnyCore,
        }
    }
    pub fn cores(lock: ResourceHandle, cores: Vec<Arc<ManagedCore>>) -> Self {
        Self {
            _lock: Some(lock),
            inner: CoreGroupInner::Cores(cores),
        }
    }
    pub fn reserve(&self) -> Vec<ManagedCore> {
        match &self.inner {
            CoreGroupInner::AnyCore => {
                vec![] // no dedicated cores
            }
            CoreGroupInner::Cores(cores) => cores.iter().map(|x| x.reserve().unwrap()).collect(),
        }
    }
    pub fn bind_nth(&self, index: usize) -> Result<Cleanup> {
        match &self.inner {
            CoreGroupInner::AnyCore => Ok(Cleanup::new(None, None)),
            CoreGroupInner::Cores(cores) => {
                let core = cores.get(index).with_context(|| {
                    format!("Could not find {}th core in the group {:?}", index, cores)
                })?;
                core.bind()
            }
        }
    }

    /// This may block due to Mutex
    pub fn get_cores(&self) -> Option<Vec<usize>> {
        match &self.inner {
            CoreGroupInner::AnyCore => None,
            CoreGroupInner::Cores(cores) => Some(cores.iter().map(|x| x.index).collect()),
        }
    }
}

enum CoreGroupInner {
    AnyCore,
    Cores(Vec<Arc<ManagedCore>>),
}
impl Debug for CoreGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            CoreGroupInner::AnyCore => f.write_str("AnyCore"),
            CoreGroupInner::Cores(cores) => {
                let mut numbers = vec![];
                for c in cores {
                    numbers.push(c.get_raw());
                }
                f.write_fmt(format_args!("Cores({:?})", numbers))
            }
        }
    }
}
pub struct ManagedCore {
    index: usize,
    taken: Resource,
}
impl Debug for ManagedCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.index, f)
    }
}
impl PartialEq for ManagedCore {
    fn eq(&self, other: &Self) -> bool {
        self.index.eq(&other.index)
    }
}
impl ManagedCore {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            taken: Resource::new(),
        }
    }
    pub fn get_raw(&self) -> usize {
        self.index
    }
    fn bind(&self) -> anyhow::Result<Cleanup> {
        let guard = self.taken.try_lock().map_err(|x| match x {
            TryLockError::Poisoned(_) => {
                anyhow!("Poisoned")
            }
            TryLockError::WouldBlock => {
                anyhow!("Core already bound: {}", self.index)
            }
        })?;
        let prior = bind_to_cpu_set(to_cpu_set(Some(self.get_raw())))?;
        Ok(Cleanup::new(Some(prior), Some(guard)))
    }
    pub fn reserve(&self) -> anyhow::Result<ManagedCore> {
        let guard = self.taken.try_lock().map_err(|x| match x {
            TryLockError::Poisoned(_) => {
                anyhow!("Poisoned")
            }
            TryLockError::WouldBlock => {
                anyhow!("Core already bound: {}", self.index)
            }
        })?;
        forget(guard);
        Ok(ManagedCore::new(self.index))
    }
}

pub struct Cleanup<'a> {
    prior_state: Option<CpuSet>,
    alive: Option<ResourceHandle>,
    _phantom: PhantomData<(Rc<()>, &'a ())>,
}
impl<'a> Cleanup<'a> {
    pub fn new(prior_state: Option<CpuSet>, alive: Option<ResourceHandle>) -> Self {
        Self {
            prior_state,
            alive,
            _phantom: Default::default(),
        }
    }
    pub fn detach(mut self) {
        self.prior_state.take();
        forget(self.alive.take());
    }
}

impl<'a> Drop for Cleanup<'a> {
    fn drop(&mut self) {
        if let Some(prior) = self.prior_state.take() {
            if let Err(err) = bind_to_cpu_set(prior) {
                error!("Error restoring state: {:?}", err);
            }
        }
    }
}
