use core_allocator::*;
use lazy_static::lazy_static;
use log::*;

lazy_static! {
    pub static ref CORE_ALLOCATOR: Box<dyn CoreAllocator> = {
        let allocator = HierarchicalAllocator::new_at_depth(hwloc2::ObjectType::L2Cache).finish();
        Box::new(allocator)
    };
}

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();
    let first_group = CORE_ALLOCATOR.allocate_core().unwrap();
    let first = first_group.get_cores();
    let x = first_group.bind_nth(0).unwrap();
    let second_group = CORE_ALLOCATOR.allocate_core().unwrap();
    let second = second_group.get_cores();
    assert_ne!(first, second);
    drop(x);
    drop(first_group);
    let third_group = CORE_ALLOCATOR.allocate_core().unwrap();
    let third = third_group.get_cores();
    assert_eq!(first, third);
}
