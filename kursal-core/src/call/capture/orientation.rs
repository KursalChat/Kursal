#[cfg(target_os = "macos")]
pub fn capture_angle() -> f64 {
    0.0
}

#[cfg(target_os = "ios")]
mod ios_angle {
    use objc2::MainThreadMarker;
    use objc2_ui_kit::{UIDevice, UIDeviceOrientation};
    use std::sync::atomic::{AtomicU32, Ordering};

    static ANGLE: AtomicU32 = AtomicU32::new(90);

    fn angle_for(orientation: UIDeviceOrientation) -> u32 {
        match orientation {
            UIDeviceOrientation::LandscapeLeft => 0,
            UIDeviceOrientation::PortraitUpsideDown => 270,
            UIDeviceOrientation::LandscapeRight => 180,
            _ => 90,
        }
    }

    pub fn refresh() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let orientation = UIDevice::currentDevice(mtm).orientation();
        if orientation != UIDeviceOrientation::Unknown {
            ANGLE.store(angle_for(orientation), Ordering::Relaxed);
        }
    }

    pub fn current() -> f64 {
        refresh();
        f64::from(ANGLE.load(Ordering::Relaxed))
    }
}

#[cfg(target_os = "ios")]
pub fn capture_angle() -> f64 {
    ios_angle::current()
}
