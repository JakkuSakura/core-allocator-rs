use crate::{CoreAllocator, CoreGroup, CoreIndex};
use hwloc::CpuSet;
use std::fmt::{Debug, Formatter};
use std::mem::replace;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct NoAllocator;
impl CoreAllocator for NoAllocator {
    fn allocate_core(&self) -> Option<CoreGroup> {
        Some(CoreGroup::any_core())
    }
}
struct ManagedGroup {
    allocated: AtomicBool,
    group: Vec<Arc<Mutex<CoreIndex>>>,
}

pub struct GroupedAllocator {
    groups: Vec<ManagedGroup>,
}
impl GroupedAllocator {
    pub fn new() -> Self {
        Self { groups: vec![] }
    }
    pub fn add_group(&mut self, group: Vec<CoreIndex>) {
        self.groups.push(ManagedGroup {
            allocated: AtomicBool::new(false),
            group: group.into_iter().map(Mutex::new).map(Arc::new).collect(),
        });
    }
    pub fn filter_group(&mut self, filter: impl Fn(&CoreIndex) -> bool) {
        let groups = replace(&mut self.groups, vec![]);
        'outer: for group in groups {
            for core in &group.group {
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
            if group.allocated.load(Ordering::Relaxed) == true {
                let mut only = true;
                for c in &group.group {
                    if Arc::strong_count(c) > 1 {
                        only = false;
                        break;
                    }
                }
                if only {
                    group.allocated.store(false, Ordering::Relaxed);
                }
            }
            if group
                .allocated
                .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                == Ok(false)
            {
                return Some(CoreGroup::cores(group.group.clone()));
            }
        }

        None
    }
}
impl Debug for GroupedAllocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let groups = self
            .groups
            .iter()
            .map(|x| CoreGroup::cores(x.group.clone()))
            .collect::<Vec<_>>();
        groups.fmt(f)
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
    // only for references: see also hwloc-ls
    pub const PHYSICAL_CPU: usize = 1;
    pub const L3_CACHE: usize = 2;
    pub const L2_CACHE: usize = 3;
    pub const LOGICAL_CORE: usize = 4;

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
            for (i, cpu) in topo
                .objects_at_depth(HierarchicalAllocator::PHYSICAL_CPU as u32)
                .iter()
                .enumerate()
            {
                if allow_cpu.iter().find(|x| **x == i).is_some() {
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
                Some(cpu_set) => {
                    let group = cpu_set
                        .into_iter()
                        .filter(|x| allow.is_set(*x))
                        .map(|x| CoreIndex::new(x as _))
                        .collect::<Vec<_>>();
                    if group.len() > 0 {
                        groups.add_group(group)
                    }
                }
                None => {}
            }
        }
        groups
    }
}
