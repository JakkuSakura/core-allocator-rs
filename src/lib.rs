mod allocators;
mod bind_core;

pub use allocators::*;
use std::fmt::{Debug, Formatter};

use crate::bind_core::{bind_to_cpu_set, to_cpu_set};
use anyhow::Context;
use anyhow::Result;
use log::*;
use nix::sched::CpuSet;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub trait CoreAllocator: Sync {
    fn allocate_core(&self) -> Option<CoreGroup>;
}

pub struct CoreGroup {
    inner: CoreGroupInner,
}

impl CoreGroup {
    pub fn any_core() -> Self {
        Self {
            inner: CoreGroupInner::AnyCore,
        }
    }
    pub fn cores(cores: Vec<Arc<Mutex<CoreIndex>>>) -> Self {
        Self {
            inner: CoreGroupInner::Cores(cores),
        }
    }
}
enum CoreGroupInner {
    AnyCore,
    Cores(Vec<Arc<Mutex<CoreIndex>>>),
}
impl Debug for CoreGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            CoreGroupInner::AnyCore => f.write_str("AnyCore"),
            CoreGroupInner::Cores(cores) => {
                let mut numbers = vec![];
                for c in cores {
                    numbers.push(c.lock().unwrap().get_raw());
                }
                f.write_fmt(format_args!("Cores({:?})", numbers))
            }
        }
    }
}

impl CoreGroup {
    pub fn bind_nth(&self, index: usize) -> Result<Cleanup> {
        match &self.inner {
            CoreGroupInner::AnyCore => Ok(Cleanup::new(None)),
            CoreGroupInner::Cores(cores) => {
                let core = cores.get(index).with_context(|| {
                    format!("Could not find {}th core in the group {:?}", index, cores)
                })?;
                core.lock().unwrap().bind()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoreIndex(usize);

impl CoreIndex {
    pub fn new(index: usize) -> Self {
        Self(index)
    }
    pub fn get_raw(self) -> usize {
        self.0
    }
    fn bind(self) -> anyhow::Result<Cleanup<'static>> {
        Ok(Cleanup::new(Some(bind_to_cpu_set(to_cpu_set(Some(
            self.get_raw(),
        )))?)))
    }
}

pub struct Cleanup<'a> {
    prior_state: Option<CpuSet>,
    // !Send
    _phantom: PhantomData<(Rc<()>, &'a ())>,
}
impl<'a> Cleanup<'a> {
    pub fn new(prior_state: Option<CpuSet>) -> Self {
        Self {
            prior_state,
            _phantom: Default::default(),
        }
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
