#[cfg(target_os = "macos")]
pub fn capture_angle() -> f64 {
    0.0
}

#[cfg(target_os = "ios")]
pub fn capture_angle() -> f64 {
    f64::from((450 - super::device_angle()) % 360)
}
