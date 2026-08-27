use super::{EncodedFrame, FrameSink};
use crate::Result;
use crate::errors::KursalError;
use objc2_core_foundation::{
    CFArray, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, kCFBooleanTrue,
};
use objc2_core_media::{
    CMBlockBuffer, CMSampleBuffer, CMTime, CMVideoCodecType,
    CMVideoFormatDescriptionGetH264ParameterSetAtIndex, kCMSampleAttachmentKey_NotSync,
    kCMTimeInvalid, kCMVideoCodecType_H264,
};
use objc2_core_video::CVImageBuffer;
use objc2_video_toolbox::{
    VTCompressionSession, VTEncodeInfoFlags, VTSessionSetProperty,
    kVTCompressionPropertyKey_AllowFrameReordering, kVTCompressionPropertyKey_AverageBitRate,
    kVTCompressionPropertyKey_ExpectedFrameRate, kVTCompressionPropertyKey_MaxKeyFrameInterval,
    kVTCompressionPropertyKey_ProfileLevel, kVTCompressionPropertyKey_RealTime,
    kVTEncodeFrameOptionKey_ForceKeyFrame, kVTProfileLevel_H264_ConstrainedBaseline_AutoLevel,
};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

const CODEC_H264: CMVideoCodecType = kCMVideoCodecType_H264;
const START_CODE: [u8; 4] = [0, 0, 0, 1];

pub struct Encoder {
    session: CFRetained<VTCompressionSession>,
    sink: *mut FrameSink,
    force_key: AtomicBool,
    dimensions: (u16, u16),
}

unsafe impl Send for Encoder {}

fn vt_err(status: i32, what: &str) -> KursalError {
    KursalError::Misc(anyhow::anyhow!("videotoolbox {what} failed: {status}"))
}

fn set_number(session: &VTCompressionSession, key: &CFString, value: i32) {
    let number = CFNumber::new_i32(value);
    unsafe {
        VTSessionSetProperty(session.as_ref(), key, Some(number.as_ref() as &CFType));
    }
}

fn set_bool(session: &VTCompressionSession, key: &CFString, value: bool) {
    let flag = unsafe {
        if value {
            kCFBooleanTrue
        } else {
            objc2_core_foundation::kCFBooleanFalse
        }
    };
    let Some(flag) = flag else { return };
    unsafe {
        VTSessionSetProperty(session.as_ref(), key, Some(flag as &CFBoolean as &CFType));
    }
}

impl Encoder {
    pub fn new(
        width: u16,
        height: u16,
        bitrate: u32,
        keyframe_interval: u32,
        sink: FrameSink,
    ) -> Result<Self> {
        let sink = Box::into_raw(Box::new(sink));
        let mut raw: *mut VTCompressionSession = std::ptr::null_mut();
        let status = unsafe {
            VTCompressionSession::create(
                None,
                i32::from(width),
                i32::from(height),
                CODEC_H264,
                None,
                None,
                None,
                Some(output_callback),
                sink as *mut c_void,
                NonNull::from(&mut raw),
            )
        };
        if status != 0 || raw.is_null() {
            drop(unsafe { Box::from_raw(sink) });
            return Err(vt_err(status, "session create"));
        }
        let session = unsafe { CFRetained::from_raw(NonNull::new(raw).unwrap()) };

        unsafe {
            set_bool(&session, kVTCompressionPropertyKey_RealTime, true);
            set_bool(
                &session,
                kVTCompressionPropertyKey_AllowFrameReordering,
                false,
            );
            set_number(
                &session,
                kVTCompressionPropertyKey_AverageBitRate,
                bitrate as i32,
            );
            set_number(
                &session,
                kVTCompressionPropertyKey_MaxKeyFrameInterval,
                keyframe_interval as i32,
            );
            set_number(&session, kVTCompressionPropertyKey_ExpectedFrameRate, 30);
            VTSessionSetProperty(
                session.as_ref(),
                kVTCompressionPropertyKey_ProfileLevel,
                Some(kVTProfileLevel_H264_ConstrainedBaseline_AutoLevel.as_ref() as &CFType),
            );
        }

        Ok(Self {
            session,
            sink,
            force_key: AtomicBool::new(false),
            dimensions: (width, height),
        })
    }

    pub fn dimensions(&self) -> (u16, u16) {
        self.dimensions
    }

    pub fn set_bitrate(&self, bps: u32) {
        unsafe {
            set_number(
                &self.session,
                kVTCompressionPropertyKey_AverageBitRate,
                bps as i32,
            );
        }
    }

    pub fn request_keyframe(&self) {
        self.force_key.store(true, Ordering::Relaxed);
    }

    pub fn encode(
        &mut self,
        image: &CVImageBuffer,
        timestamp: CMTime,
        duration: CMTime,
    ) -> Result<()> {
        let properties = if self.force_key.swap(false, Ordering::Relaxed) {
            force_key_properties()
        } else {
            None
        };
        let status = unsafe {
            self.session.encode_frame(
                image,
                timestamp,
                duration,
                properties.as_deref(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err(vt_err(status, "encode frame"));
        }
        Ok(())
    }
}

fn force_key_properties() -> Option<CFRetained<CFDictionary>> {
    let key = unsafe { kVTEncodeFrameOptionKey_ForceKeyFrame };
    let value = unsafe { kCFBooleanTrue }?;
    let dict = CFDictionary::<CFString, CFBoolean>::from_slices(&[key], &[value]);
    Some(unsafe { CFRetained::cast_unchecked::<CFDictionary>(dict) })
}

unsafe extern "C-unwind" fn output_callback(
    refcon: *mut c_void,
    _source_frame_refcon: *mut c_void,
    status: i32,
    _flags: VTEncodeInfoFlags,
    sample: *mut CMSampleBuffer,
) {
    if status != 0 || sample.is_null() || refcon.is_null() {
        return;
    }
    let sink = unsafe { &*(refcon as *const FrameSink) };
    let sample = unsafe { &*sample };
    if let Some(frame) = annexb_frame(sample) {
        sink(frame);
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            self.session.complete_frames(kCMTimeInvalid);
            self.session.invalidate();
            drop(Box::from_raw(self.sink));
        }
    }
}

const NAL_TYPE_IDR: u8 = 5;

fn contains_idr(annexb: &[u8]) -> bool {
    let mut offset = 0usize;
    while let Some(start) = find_start_code(annexb, offset) {
        let header = start + START_CODE.len();
        if header >= annexb.len() {
            return false;
        }
        if annexb[header] & 0x1F == NAL_TYPE_IDR {
            return true;
        }
        offset = header;
    }
    false
}

fn find_start_code(bytes: &[u8], from: usize) -> Option<usize> {
    bytes
        .get(from..)?
        .windows(START_CODE.len())
        .position(|w| w == START_CODE)
        .map(|p| from + p)
}

fn is_keyframe(sample: &CMSampleBuffer) -> bool {
    let Some(attachments) = (unsafe { sample.sample_attachments_array(false) }) else {
        return false;
    };
    let attachments = unsafe { CFRetained::cast_unchecked::<CFArray<CFDictionary>>(attachments) };
    let Some(first) = attachments.get(0) else {
        return false;
    };
    let first = unsafe { CFRetained::cast_unchecked::<CFDictionary<CFString, CFBoolean>>(first) };
    let not_sync_key = unsafe { kCMSampleAttachmentKey_NotSync };
    match first.get(not_sync_key) {
        Some(flag) => !flag.value(),
        None => true,
    }
}

fn annexb_frame(sample: &CMSampleBuffer) -> Option<EncodedFrame> {
    let mut nals = Vec::new();
    append_nal_units(sample, &mut nals)?;

    let keyframe = is_keyframe(sample) || contains_idr(&nals);

    let mut data = Vec::new();
    if keyframe {
        append_parameter_sets(sample, &mut data);
    }
    data.extend_from_slice(&nals);

    let timestamp = unsafe { sample.presentation_time_stamp() };
    let timestamp_us = if timestamp.timescale > 0 {
        u64::try_from(
            (i128::from(timestamp.value) * 1_000_000 / i128::from(timestamp.timescale)).max(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    Some(EncodedFrame {
        data,
        keyframe,
        timestamp_us,
        rotation: 0,
    })
}

fn append_parameter_sets(sample: &CMSampleBuffer, out: &mut Vec<u8>) {
    unsafe {
        let Some(desc) = sample.format_description() else {
            return;
        };
        let mut count: usize = 0;
        if CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
            &desc,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut count,
            std::ptr::null_mut(),
        ) != 0
        {
            return;
        }
        for index in 0..count {
            let mut ptr: *const u8 = std::ptr::null();
            let mut size: usize = 0;
            if CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                &desc,
                index,
                &mut ptr,
                &mut size,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
                && !ptr.is_null()
            {
                out.extend_from_slice(&START_CODE);
                out.extend_from_slice(std::slice::from_raw_parts(ptr, size));
            }
        }
    }
}

fn block_bytes(block: &CMBlockBuffer) -> Option<Vec<u8>> {
    unsafe {
        let total = block.data_length();
        if total == 0 {
            return None;
        }
        let mut contiguous: usize = 0;
        let mut raw: *mut std::ffi::c_char = std::ptr::null_mut();
        if block.data_pointer(0, &mut contiguous, std::ptr::null_mut(), &mut raw) != 0
            || raw.is_null()
        {
            return None;
        }
        if contiguous >= total {
            return Some(std::slice::from_raw_parts(raw as *const u8, total).to_vec());
        }
        let mut out = vec![0u8; total];
        let dst = NonNull::new(out.as_mut_ptr().cast::<c_void>())?;
        if block.copy_data_bytes(0, total, dst) != 0 {
            return None;
        }
        Some(out)
    }
}

fn append_nal_units(sample: &CMSampleBuffer, out: &mut Vec<u8>) -> Option<()> {
    let block = unsafe { sample.data_buffer() }?;
    let bytes = block_bytes(&block)?;
    let mut offset = 0usize;
    while offset + 4 <= bytes.len() {
        let len = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;
        if len > bytes.len() - offset {
            break;
        }
        out.extend_from_slice(&START_CODE);
        out.extend_from_slice(&bytes[offset..offset + len]);
        offset += len;
    }
    Some(())
}
