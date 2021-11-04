use crate::{CoreAllocator, CoreGroup, CoreIndex};
use std::mem::replace;
use std::ops::Range;
use std::sync::Mutex;

pub struct NoAllocator;
impl CoreAllocator for NoAllocator {
    fn allocate_core(&self) -> Option<CoreGroup> {
        Some(CoreGroup::AnyCore)
    }
}
pub struct SequentialAllocator {
    groups: Vec<Vec<Mutex<CoreIndex>>>,
}
impl SequentialAllocator {
    pub fn new_range(range: Range<usize>, width: usize) -> Self {
        let mut groups = vec![];
        let mut group = vec![];
        for i in range {
            group.push(Mutex::new(CoreIndex::new(i)));
            if group.len() == width {
                groups.push(replace(&mut group, vec![]));
            }
        }
        Self { groups }
    }
}
impl CoreAllocator for SequentialAllocator {
    fn allocate_core(&self) -> Option<CoreGroup> {
        for group in self.groups.iter() {
            let locked_all: Vec<_> = group.iter().filter_map(|x| x.try_lock().ok()).collect();
            if locked_all.len() == group.len() {
                return Some(CoreGroup::LockedCores(locked_all));
            }
        }
        None
    }
}
pub struct HierarchicalAllocator;
impl HierarchicalAllocator {
    pub fn new() -> Self {
        let topo = hwloc::Topology::new();

        for i in 0..topo.depth() {
            println!("*** Objects at level {}", i);

            for (idx, object) in topo.objects_at_depth(i).iter().enumerate() {
                println!("{}: {} {:?}", idx, object, object.allowed_cpuset());
            }
        }
        panic!()
    }
}

impl CoreAllocator for HierarchicalAllocator {
    fn allocate_core(&self) -> Option<CoreGroup> {
        None
    }
}
