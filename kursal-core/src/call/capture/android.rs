use super::{CaptureConfig, EncodedFrame, FrameSink, VideoFormat, quality_dims};
use crate::Result;
use crate::dto::CameraInfo;
use crate::errors::KursalError;
use crate::sync::LockExt;
use jni::objects::{JClass, JObject, JValue};
use std::sync::{Mutex as StdMutex, OnceLock};

const CLASS: &str = "chat.kursal.CameraCapture";
const KEYFRAME_INTERVAL_SECS: i32 = 3;

fn sink_slot() -> &'static StdMutex<Option<FrameSink>> {
    static S: OnceLock<StdMutex<Option<FrameSink>>> = OnceLock::new();
    S.get_or_init(|| StdMutex::new(None))
}

fn jvm_err<E: std::fmt::Debug>(env: &jni::JNIEnv, prefix: &str, e: E) -> KursalError {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    KursalError::Misc(anyhow::anyhow!("{prefix}: {e:?}"))
}

fn java_vm() -> Result<jni::JavaVM> {
    let ctx = ndk_context::android_context();
    unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("JNI vm: {e:?}")))
}

fn app_context<'a>() -> JObject<'a> {
    (ndk_context::android_context().context() as jni::sys::jobject).into()
}

fn load_class<'a>(env: &jni::JNIEnv<'a>) -> Result<JClass<'a>> {
    let loader = env
        .call_method(
            app_context(),
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )
        .map_err(|e| jvm_err(env, "getClassLoader", e))?
        .l()
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("classloader.l: {e:?}")))?;

    let name = env
        .new_string(CLASS)
        .map_err(|e| jvm_err(env, "class name", e))?;

    let class = env
        .call_method(
            loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(name.into())],
        )
        .map_err(|e| jvm_err(env, "loadClass", e))?
        .l()
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("class.l: {e:?}")))?;

    Ok(JClass::from(class))
}

pub(super) fn list_cameras() -> Vec<CameraInfo> {
    let Ok(vm) = java_vm() else {
        return Vec::new();
    };
    let Ok(env) = vm.attach_current_thread() else {
        return Vec::new();
    };
    let Ok(class) = load_class(&env) else {
        return Vec::new();
    };

    let result = env.call_static_method(
        class,
        "listCameras",
        "(Landroid/content/Context;)[Ljava/lang/String;",
        &[JValue::Object(app_context())],
    );
    let Ok(Ok(array)) = result.map(|value| value.l()) else {
        return Vec::new();
    };
    let raw = array.into_inner();
    let Ok(len) = env.get_array_length(raw) else {
        return Vec::new();
    };

    let mut cameras = Vec::new();
    for index in 0..len {
        let Ok(item) = env.get_object_array_element(raw, index) else {
            continue;
        };
        let Ok(text) = env.get_string(item.into()) else {
            continue;
        };
        let text: String = text.into();
        let mut parts = text.split('\t');
        let (Some(id), Some(label)) = (parts.next(), parts.next()) else {
            continue;
        };
        let facing = parts.next().filter(|f| !f.is_empty()).map(str::to_string);
        cameras.push(CameraInfo {
            id: id.to_string(),
            label: label.to_string(),
            facing,
        });
    }
    cameras
}

pub(super) fn start(config: &CaptureConfig, sink: FrameSink) -> Result<VideoFormat> {
    let (budget_w, budget_h, bitrate) = quality_dims(config.quality);
    *sink_slot().lock_recover() = Some(sink);

    let vm = java_vm()?;
    let env = vm
        .attach_current_thread()
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("JNI attach: {e:?}")))?;
    let class = load_class(&env)?;

    let requested = match config.camera_id.as_deref() {
        Some(id) => JObject::from(
            env.new_string(id)
                .map_err(|e| jvm_err(&env, "camera id", e))?,
        ),
        None => JObject::null(),
    };

    let value = env
        .call_static_method(
            class,
            "start",
            "(Landroid/content/Context;Ljava/lang/String;IIII)Ljava/lang/String;",
            &[
                JValue::Object(app_context()),
                JValue::Object(requested),
                JValue::Int(i32::from(budget_w)),
                JValue::Int(i32::from(budget_h)),
                JValue::Int(i32::try_from(bitrate).unwrap_or(i32::MAX)),
                JValue::Int(KEYFRAME_INTERVAL_SECS),
            ],
        )
        .map_err(|e| jvm_err(&env, "CameraCapture.start", e))?;

    let handle = value
        .l()
        .map_err(|e| KursalError::Misc(anyhow::anyhow!("start ret: {e:?}")))?;
    if handle.is_null() {
        *sink_slot().lock_recover() = None;
        return Err(KursalError::Misc(anyhow::anyhow!("camera failed to start")));
    }

    let descriptor = env
        .get_string(handle.into())
        .map_err(|e| jvm_err(&env, "start ret string", e))?;
    let descriptor: String = descriptor.into();
    let mut parts = descriptor.split('|');
    let width = parts
        .next()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(budget_w);
    let height = parts
        .next()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(budget_h);
    let camera_id = parts.next().map(str::to_string);

    super::set_selected_camera(camera_id.clone());
    Ok(VideoFormat {
        codec: "avc1.42E01F".to_string(),
        width,
        height,
        camera_id,
    })
}

pub(super) fn stop() {
    if let Ok(vm) = java_vm()
        && let Ok(env) = vm.attach_current_thread()
        && let Ok(class) = load_class(&env)
    {
        let _ = env.call_static_method(
            class,
            "stop",
            "(Landroid/content/Context;)V",
            &[JValue::Object(app_context())],
        );
    }
    *sink_slot().lock_recover() = None;
}

pub(super) fn request_keyframe() {
    call_void("requestKeyframe", "()V", &[]);
}

pub(super) fn set_bitrate(bps: u32) {
    call_void(
        "setBitrate",
        "(I)V",
        &[JValue::Int(i32::try_from(bps).unwrap_or(i32::MAX))],
    );
}

fn call_void(name: &str, signature: &str, args: &[JValue]) {
    if let Ok(vm) = java_vm()
        && let Ok(env) = vm.attach_current_thread()
        && let Ok(class) = load_class(&env)
    {
        let _ = env.call_static_method(class, name, signature, args);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_chat_kursal_CameraCapture_nativeVideoFrame(
    env: jni::JNIEnv,
    _class: JClass,
    data: jni::sys::jbyteArray,
    keyframe: jni::sys::jboolean,
    timestamp_us: jni::sys::jlong,
) {
    let Ok(bytes) = env.convert_byte_array(data) else {
        return;
    };
    let sink = sink_slot().lock_recover().clone();
    if let Some(sink) = sink {
        sink(EncodedFrame {
            data: bytes,
            keyframe: keyframe != 0,
            timestamp_us: u64::try_from(timestamp_us).unwrap_or(0),
        });
    }
}

/// Called from CameraCapture.kt when the camera or encoder dies mid-call.
#[unsafe(no_mangle)]
pub extern "system" fn Java_chat_kursal_CameraCapture_nativeCaptureFailed(
    _env: jni::JNIEnv,
    _class: JClass,
) {
    super::report_failure();
}

pub(super) fn refresh_rotation() {}
