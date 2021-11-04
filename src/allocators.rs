use crate::{CoreAllocator, CoreGroup, CoreIndex};
use hwloc::{CpuSet, ObjectType};
use std::mem::replace;
use std::ops::Range;
use std::sync::Mutex;

pub struct NoAllocator;
impl CoreAllocator for NoAllocator {
    fn allocate_core(&self) -> Option<CoreGroup> {
        Some(CoreGroup::AnyCore)
    }
}
pub struct GroupedAllocator {
    groups: Vec<Vec<Mutex<CoreIndex>>>,
}
impl GroupedAllocator {
    pub fn new() -> Self {
        Self { groups: vec![] }
    }
    pub fn add_group(&mut self, group: Vec<CoreIndex>) {
        self.groups
            .push(group.into_iter().map(Mutex::new).collect());
    }
    pub fn filter_group(&mut self, filter: impl Fn(&CoreIndex) -> bool) {
        let groups = replace(&mut self.groups, vec![]);
        'outer: for group in groups {
            for core in &group {
                if !filter(&core.lock().unwrap()) {
                    continue 'outer;
                }
            }
            self.groups.push(group);
        }
    }
}
impl CoreAllocator for GroupedAllocator {
    fn allocate_core(&self) -> Option<CoreGroup> {
        for group in self.groups.iter() {
            let locked_all: Vec<_> = group.iter().filter_map(|x| x.try_lock().ok()).collect();
            if locked_all.len() == group.len() {
                return Some(CoreGroup::Cores(locked_all));
            }
        }
        None
    }
}
pub struct SequentialAllocator;

impl SequentialAllocator {
    pub fn new_range(range: Range<usize>, width: usize) -> GroupedAllocator {
        let mut groups = GroupedAllocator::new();
        let mut group = vec![];
        for i in range {
            group.push(CoreIndex::new(i));
            if group.len() == width {
                groups.add_group(replace(&mut group, vec![]));
            }
        }
        groups
    }
}

pub struct HierarchicalAllocator {
    depth: usize,
    on_cpus: Option<Vec<usize>>,
}
impl HierarchicalAllocator {
    pub fn new_at_depth(depth: usize) -> Self {
        Self {
            depth,
            on_cpus: None,
        }
    }
    pub fn on_cpu(mut self, on_cpus: Vec<usize>) -> Self {
        self.on_cpus = Some(on_cpus);
        self
    }
    pub fn finish(self) -> GroupedAllocator {
        let depth = self.depth;
        let topo = hwloc::Topology::new();
        let mut groups = GroupedAllocator::new();
        let mut allow = CpuSet::new();
        if let Some(allow_cpu) = self.on_cpus {
            for cpu in topo.objects_with_type(&ObjectType::Package).unwrap() {
                if allow_cpu
                    .iter()
                    .find(|x| **x == cpu.os_index() as _)
                    .is_some()
                {
                    for bit in cpu.allowed_cpuset().unwrap() {
                        allow.set(bit);
                    }
                }
            }
        } else {
            allow = CpuSet::full();
        }
        for object in topo.objects_at_depth(depth as u32).iter() {
            let cpu_set = object.allowed_cpuset();
            match cpu_set {
                Some(cpu_set) => groups.add_group(
                    cpu_set
                        .into_iter()
                        .filter(|x| allow.is_set(*x))
                        .map(|x| CoreIndex::new(x as _))
                        .collect(),
                ),
                None => {}
            }
        }
        groups
    }
}
