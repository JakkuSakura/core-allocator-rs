use core_allocator::*;
use lazy_static::lazy_static;
use log::*;

lazy_static! {
    pub static ref CORE_ALLOCATOR: Box<dyn CoreAllocator> = {
        // Box::new(HierarchicalAllocator::new_at_depth(3))
        //Box::new(SequentialAllocator::new_range(0..8, 2))
        let allocator = HierarchicalAllocator::new_at_depth(hwloc2::ObjectType::L3Cache).on_cpu(vec![0]).finish();
        Box::new(allocator)
    };
}

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();
    let mut allocated = vec![];
    while let Some(core_id) = CORE_ALLOCATOR.allocate_core() {
        info!("{:?}", core_id);
        allocated.push(core_id);
    }
}
