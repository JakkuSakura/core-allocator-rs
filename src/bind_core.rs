use nix::sched::CpuSet;

pub fn to_io_error<T>(err: nix::Result<T>) -> std::io::Result<T> {
    match err {
        Ok(x) => Ok(x),
        Err(errno) => Err(errno.into()),
    }
}
/// Returns previous CpuSet
pub fn bind_to_cpu_set(cpuset: CpuSet) -> std::io::Result<CpuSet> {
    let pid = nix::unistd::gettid();

    // debug!("taskset -pc {} {}", set, pid);
    let previous = nix::sched::sched_getaffinity(pid)?;
    to_io_error(nix::sched::sched_setaffinity(pid, &cpuset))?;
    Ok(previous)
}

pub fn to_cpu_set(cores: impl IntoIterator<Item = usize>) -> CpuSet {
    let mut set = CpuSet::new();
    let mut is_set = false;
    for i in cores {
        set.set(i as _).unwrap();
        is_set = true;
    }
    if !is_set {
        for i in 0..CpuSet::count() {
            set.set(i as _).unwrap();
        }
    }
    set
}
