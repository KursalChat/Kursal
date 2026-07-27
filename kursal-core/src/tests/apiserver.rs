use crate::apiserver::iprecord::{FAILURE_WINDOW, IpRecord, MAX_FAILURES};
use std::time::{Duration, Instant};

#[test]
fn ip_record_not_limited_when_fresh() {
    let mut rec = IpRecord::default();
    assert!(!rec.is_limited(Instant::now()));
}

#[test]
fn ip_record_limits_after_max_failures() {
    let now = Instant::now();
    let mut rec = IpRecord::default();
    for _ in 0..MAX_FAILURES {
        rec.record_failure(now);
    }
    assert!(rec.is_limited(now));
}

#[test]
fn ip_record_below_max_not_limited() {
    let now = Instant::now();
    let mut rec = IpRecord::default();
    for _ in 0..(MAX_FAILURES - 1) {
        rec.record_failure(now);
    }
    assert!(!rec.is_limited(now));
}

#[test]
fn ip_record_reset_clears_failures() {
    let now = Instant::now();
    let mut rec = IpRecord::default();
    for _ in 0..MAX_FAILURES {
        rec.record_failure(now);
    }
    assert!(rec.is_limited(now));

    rec.reset();
    assert!(!rec.is_limited(now));
}

#[test]
fn ip_record_window_expiry_clears_limit() {
    let start = Instant::now();
    let mut rec = IpRecord::default();
    for _ in 0..MAX_FAILURES {
        rec.record_failure(start);
    }
    assert!(rec.is_limited(start));

    let later = start + FAILURE_WINDOW + Duration::from_secs(1);
    assert!(!rec.is_limited(later));
}
