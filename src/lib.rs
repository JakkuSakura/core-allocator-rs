mod allocators;
mod bind_core;

pub use allocators::*;

use crate::bind_core::{bind_to_cpu_set, to_cpu_set};
use anyhow::Context;
use anyhow::Result;
use log::*;
use nix::sched::CpuSet;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::MutexGuard;

pub trait CoreAllocator: Sync {
    fn allocate_core(&self) -> Option<CoreGroup>;
}
#[derive(Debug)]
pub enum CoreGroup<'a> {
    AnyCore,
    Cores(Vec<MutexGuard<'a, CoreIndex>>),
}
impl<'a> CoreGroup<'a> {
    pub fn bind_nth(&self, index: usize) -> Result<Cleanup<'a>> {
        match self {
            CoreGroup::AnyCore => Ok(Cleanup::new(None)),
            CoreGroup::Cores(cores) => {
                let core = cores.get(index).with_context(|| {
                    format!("Could not find {}th core in the group {:?}", index, cores)
                })?;
                core.bind()
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
