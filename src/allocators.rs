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
    pub fn new_at_depth(depth: usize) -> SequentialAllocator {
        let topo = hwloc::Topology::new();
        let mut groups = vec![];

        for object in topo.objects_at_depth(depth as u32).iter() {
            let cpu_set = object.allowed_cpuset();
            match cpu_set {
                None => {}
                Some(cpu_set) => groups.push(
                    cpu_set
                        .into_iter()
                        .map(|x| Mutex::new(CoreIndex::new(x as _)))
                        .collect(),
                ),
            }
        }
        SequentialAllocator { groups }
    }
}
