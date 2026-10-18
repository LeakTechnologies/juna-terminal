//! Same-machine cross-process intervals using the documented Windows performance counter.
// SAFETY: Kernel32's documented BOOL/LARGE_INTEGER ABI is i32 and a writable aligned i64 slot.
// These two infrastructure bindings avoid exposing any terminal semantics or process memory.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryPerformanceCounter(value: *mut i64) -> i32;
    fn QueryPerformanceFrequency(value: *mut i64) -> i32;
}
pub fn ticks() -> Result<i64, String> {
    let mut value = 0;
    // SAFETY: value is an aligned writable LARGE_INTEGER-sized slot for this synchronous API.
    if unsafe { QueryPerformanceCounter(&mut value) } == 0 {
        return Err("Performance counter unavailable".into());
    }
    Ok(value)
}
pub fn frequency() -> Result<i64, String> {
    let mut value = 0;
    // SAFETY: value is an aligned writable LARGE_INTEGER-sized slot for this synchronous API.
    if unsafe { QueryPerformanceFrequency(&mut value) } == 0 || value <= 0 {
        return Err("Performance counter frequency unavailable".into());
    }
    Ok(value)
}
pub fn milliseconds(start: i64, end: i64, hz: i64) -> Result<f64, String> {
    let delta = end
        .checked_sub(start)
        .ok_or("Performance counter overflow")?;
    // Microsoft documents one tick of cross-thread/process ordering uncertainty.
    if hz <= 0 || delta < -1 {
        return Err("Inconsistent performance counters".into());
    }
    Ok(delta.max(0) as f64 * 1000. / hz as f64)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counter_and_cross_process_uncertainty() {
        assert!(frequency().unwrap() > 0);
        let a = ticks().unwrap();
        assert!(ticks().unwrap() >= a);
        assert_eq!(milliseconds(10, 25, 1000).unwrap(), 15.);
        assert_eq!(milliseconds(10, 9, 1000).unwrap(), 0.);
        assert!(milliseconds(10, 8, 1000).is_err());
        assert!(milliseconds(1, 2, 0).is_err());
    }
}
