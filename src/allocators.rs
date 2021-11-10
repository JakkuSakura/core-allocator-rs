use crate::{CoreAllocator, CoreGroup, CoreIndex};
use hwloc2::ObjectType;
use std::fmt::{Debug, Formatter};
use std::mem::replace;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
#[cfg(feature = "hwloc2")]
lazy_static::lazy_static! {
    static ref ALL_CORES: Arc<Vec<Arc<Mutex<CoreIndex>>>> = {
        let topo = hwloc2::Topology::new().unwrap();
        let cpuset = topo.object_at_root().cpuset().unwrap();
        let cores = cpuset.into_iter().map(|x| x as _).map(CoreIndex::new).map(Mutex::new).map(Arc::new).collect();
        Arc::new(cores)
    };
}
#[cfg(not(feature = "hwloc2"))]
lazy_static::lazy_static! {
    static ref ALL_CORES: Arc<Vec<Arc<Mutex<CoreIndex>>>> = {
        let cpuset = 0..256;
        let cores = cpuset.into_iter().map(|x| x as _).map(CoreIndex::new).map(Mutex::new).map(Arc::new).collect();
        Arc::new(cores)
    };
}
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
    pub fn add_group(&mut self, group: Vec<Arc<Mutex<CoreIndex>>>) {
        self.groups.push(ManagedGroup {
            allocated: AtomicBool::new(false),
            group,
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
            group.push(Arc::clone(&ALL_CORES.get(i).unwrap()));
            if group.len() == width {
                groups.add_group(replace(&mut group, vec![]));
            }
        }
        groups
    }
}

#[cfg(feature = "hwloc2")]
pub struct HierarchicalAllocator {
    object_type: hwloc2::ObjectType,
    on_cpus: Option<Vec<usize>>,
}
#[cfg(feature = "hwloc2")]
impl HierarchicalAllocator {
    pub fn new_at_depth(object_type: hwloc2::ObjectType) -> Self {
        Self {
            object_type,
            on_cpus: None,
        }
    }

    pub fn on_cpu(mut self, on_cpus: Vec<usize>) -> Self {
        self.on_cpus = Some(on_cpus);
        self
    }

    pub fn finish(self) -> GroupedAllocator {
        let obj_type = self.object_type;
        let topo = hwloc2::Topology::new().unwrap();
        let mut groups = GroupedAllocator::new();
        let mut allow = hwloc2::CpuSet::new();
        if let Some(allow_cpu) = self.on_cpus {
            for (i, cpu) in topo
                .objects_with_type(&hwloc2::ObjectType::Package)
                .unwrap()
                .iter()
                .enumerate()
            {
                if allow_cpu.iter().find(|x| **x == i).is_some() {
                    for bit in cpu.cpuset().unwrap() {
                        allow.set(bit);
                    }
                }
            }
        } else {
            allow = hwloc2::CpuSet::full();
        }
        if obj_type == ObjectType::L3Cache {
            for object in topo.objects_with_type(&obj_type).unwrap().iter() {
                let mut phys = hwloc2::CpuSet::new();
                let mut hypers = hwloc2::CpuSet::new();
                for l2 in object.children() {
                    let mut cpu = l2.cpuset().unwrap().into_iter();
                    phys.set(cpu.next().unwrap());
                    hypers.set(cpu.next().unwrap());
                    assert_eq!(cpu.next(), None);
                }
                for cpu_set in [phys, hypers] {
                    let group = cpu_set
                        .into_iter()
                        .filter(|x| allow.is_set(*x))
                        .flat_map(|x| ALL_CORES.get(x as usize))
                        .map(Arc::clone)
                        .collect::<Vec<_>>();
                    if group.len() > 0 {
                        groups.add_group(group)
                    }
                }
            }
        } else {
            for object in topo.objects_with_type(&obj_type).unwrap().iter() {
                let cpu_set = object.cpuset();
                match cpu_set {
                    Some(cpu_set) => {
                        let group = cpu_set
                            .into_iter()
                            .filter(|x| allow.is_set(*x))
                            .flat_map(|x| ALL_CORES.get(x as usize))
                            .map(Arc::clone)
                            .collect::<Vec<_>>();
                        if group.len() > 0 {
                            groups.add_group(group)
                        }
                    }
                    None => {}
                }
            }
        }
        groups
    }
}
