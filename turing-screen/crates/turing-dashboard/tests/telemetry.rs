use std::time::Duration;

use turing_dashboard::telemetry::{
    fmt_gb, fmt_pct, fmt_rate, fmt_temp, Ring, Sampler, SysInfoSampler,
};

#[test]
fn ring_keeps_last_60_oldest_first() {
    let mut r = Ring::new();
    assert!(r.is_empty());
    for i in 0..100 {
        r.push(i as f32);
    }
    assert_eq!(r.len(), Ring::LEN);
    let v: Vec<f32> = r.iter().collect();
    assert_eq!(v.len(), 60);
    assert_eq!(v[0], 40.0);
    assert_eq!(v[59], 99.0);
    assert_eq!(r.max(), 99.0);
}

#[test]
fn ring_partial_fill_iterates_in_order() {
    let mut r = Ring::new();
    r.push(3.0);
    r.push(1.0);
    r.push(2.0);
    assert_eq!(r.iter().collect::<Vec<_>>(), vec![3.0, 1.0, 2.0]);
    assert_eq!(r.len(), 3);
    assert_eq!(r.max(), 3.0);
}

#[test]
fn ring_max_of_empty_is_zero() {
    assert_eq!(Ring::new().max(), 0.0);
}

#[test]
fn rate_formatting() {
    assert_eq!(fmt_rate(0.0), "0 B/s");
    assert_eq!(fmt_rate(512.0), "512 B/s");
    assert_eq!(fmt_rate(212.0 * 1024.0), "212 KB/s");
    assert_eq!(fmt_rate(1.8 * 1024.0 * 1024.0), "1.8 MB/s");
    assert_eq!(fmt_rate(1.2 * 1024.0 * 1024.0 * 1024.0), "1.2 GB/s");
}

#[test]
fn gb_formatting() {
    assert_eq!(fmt_gb(31 * 1024 * 1024 * 1024), "31.0");
    assert_eq!(fmt_gb(0), "0.0");
}

#[test]
fn pct_formatting() {
    assert_eq!(fmt_pct(36.6), "37%");
    assert_eq!(fmt_pct(250.0), "100%");
    assert_eq!(fmt_pct(-5.0), "0%");
    assert_eq!(fmt_pct(0.5), "1%");
}

#[test]
fn temp_formatting() {
    assert_eq!(fmt_temp(None), "--");
    assert_eq!(fmt_temp(Some(61.4)), "61\u{b0}C");
}

#[test]
fn sampler_smoke() {
    let mut s = SysInfoSampler::new();
    let a = s.sample();
    assert!(a.mem_total > 0);
    assert!(!a.cpu_cores.is_empty());
    check_ranges(&a);
    std::thread::sleep(Duration::from_millis(300));
    let b = s.sample();
    assert!(b.mem_total > 0);
    check_ranges(&b);
    assert!(b.net_down_bps >= 0.0 && b.net_up_bps >= 0.0);
}

fn check_ranges(s: &turing_dashboard::telemetry::Snapshot) {
    assert!((0.0..=100.0).contains(&s.cpu_load));
    for c in &s.cpu_cores {
        assert!((0.0..=100.0).contains(c));
    }
    assert!(s.mem_used <= s.mem_total);
    if let Some(g) = &s.gpu {
        assert!((0.0..=100.0).contains(&g.load));
    }
}
