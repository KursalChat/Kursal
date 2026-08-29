use crate::dto::CameraInfo;
use objc2::rc::Retained;
use objc2_av_foundation::{
    AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDevicePosition, AVCaptureDeviceType,
    AVMediaTypeVideo,
};
use objc2_foundation::NSArray;

fn position_str(position: AVCaptureDevicePosition) -> Option<String> {
    match position {
        AVCaptureDevicePosition::Front => Some("user".to_string()),
        AVCaptureDevicePosition::Back => Some("environment".to_string()),
        _ => None,
    }
}

fn device_types() -> Retained<NSArray<AVCaptureDeviceType>> {
    unsafe {
        let mut types: Vec<&AVCaptureDeviceType> =
            vec![objc2_av_foundation::AVCaptureDeviceTypeBuiltInWideAngleCamera];
        #[cfg(target_os = "macos")]
        types.push(objc2_av_foundation::AVCaptureDeviceTypeExternal);
        #[cfg(target_os = "ios")]
        types.push(objc2_av_foundation::AVCaptureDeviceTypeBuiltInUltraWideCamera);
        NSArray::from_slice(&types)
    }
}

fn discovery() -> Option<Retained<AVCaptureDeviceDiscoverySession>> {
    let media_type = unsafe { AVMediaTypeVideo }?;
    unsafe {
        Some(
            AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                &device_types(),
                Some(media_type),
                AVCaptureDevicePosition::Unspecified,
            ),
        )
    }
}

pub fn list() -> Vec<CameraInfo> {
    let Some(session) = discovery() else {
        return Vec::new();
    };
    unsafe {
        session
            .devices()
            .iter()
            .map(|device| CameraInfo {
                id: device.uniqueID().to_string(),
                label: device.localizedName().to_string(),
                facing: position_str(device.position()),
            })
            .collect()
    }
}

pub fn find(unique_id: Option<&str>) -> Option<Retained<AVCaptureDevice>> {
    let media_type = unsafe { AVMediaTypeVideo }?;
    unsafe {
        match unique_id {
            Some(id) => discovery()?
                .devices()
                .iter()
                .find(|device| device.uniqueID().to_string() == id),
            None => AVCaptureDevice::defaultDeviceWithMediaType(media_type),
        }
    }
}
