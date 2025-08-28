use goldylox::cache::tier::warm::atomic_float::*;
use std::sync::atomic::Ordering;

#[test]
fn test_basic_operations() {
    let atomic = AtomicF64::new(5.0);

    assert_eq!(atomic.load(Ordering::Relaxed), 5.0);

    atomic.store(10.0, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 10.0);

    let old = atomic.swap(15.0, Ordering::Relaxed);
    assert_eq!(old, 10.0);
    assert_eq!(atomic.load(Ordering::Relaxed), 15.0);
}

#[test]
fn test_arithmetic_operations() {
    let atomic = AtomicF64::new(10.0);

    let old = atomic.fetch_add(5.0, Ordering::Relaxed);
    assert_eq!(old, 10.0);
    assert_eq!(atomic.load(Ordering::Relaxed), 15.0);

    atomic.fetch_sub(3.0, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 12.0);

    atomic.fetch_mul(2.0, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 24.0);

    atomic.fetch_div(4.0, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 6.0);
}

#[test]
fn test_compare_exchange() {
    let atomic = AtomicF64::new(5.0);

    let result = atomic.compare_exchange(5.0, 10.0, Ordering::Relaxed, Ordering::Relaxed);
    assert_eq!(result, Ok(5.0));
    assert_eq!(atomic.load(Ordering::Relaxed), 10.0);

    let result = atomic.compare_exchange(5.0, 15.0, Ordering::Relaxed, Ordering::Relaxed);
    assert_eq!(result, Err(10.0));
    assert_eq!(atomic.load(Ordering::Relaxed), 10.0);
}

#[test]
fn test_ema_update() {
    let atomic = AtomicF64::new(0.0);

    atomic.update_ema(10.0, 0.1, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 1.0);

    atomic.update_ema(10.0, 0.1, Ordering::Relaxed);
    assert_eq!(atomic.load(Ordering::Relaxed), 1.9);
}