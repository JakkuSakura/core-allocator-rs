use core_allocator::*;
use lazy_static::lazy_static;
use log::*;

lazy_static! {
    pub static ref CORE_ALLOCATOR: Box<dyn CoreAllocator> =
        Box::new(HierarchicalAllocator::new_at_depth(3));
        //Box::new(SequentialAllocator::new_range(0..8, 2));
}
fn use_core(condition: &str, core_id: usize) -> bool {
    let conditions = condition.split(",");
    for cond in conditions {
        if cond.find("-").is_some() {
            let mut dash = cond.split("-");
            let begin = dash.next().unwrap_or("0").parse::<usize>().unwrap();
            let end = dash.next().unwrap_or("9999").parse::<usize>().unwrap();
            if begin <= core_id && core_id <= end {
                return true;
            }
        } else {
            let core_id_cond = cond.parse::<usize>().unwrap();
            if core_id == core_id_cond {
                return true;
            }
        }
    }
    return false;
}

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .init();
    let mut allocated = vec![];
    while let Some(core_id) = CORE_ALLOCATOR.allocate_core() {
        allocated.push(core_id);
    }
    info!("{:?}", allocated);
}
