use super::{CaptureConfig, EncodedFrame, FrameSink, VideoFormat, quality_dims};
use crate::Result;
use crate::dto::CameraInfo;
use crate::errors::KursalError;
use crate::sync::LockExt;
use jni::objects::{JByteArray, JClass, JObject, JObjectArray, JString, JValue};
use jni::{Env, EnvUnowned, jni_sig, jni_str};
use std::sync::{Mutex as StdMutex, OnceLock};

const CLASS: &str = "chat.kursal.CameraCapture";
const KEYFRAME_INTERVAL_SECS: i32 = 3;

fn sink_slot() -> &'static StdMutex<Option<FrameSink>> {
    static S: OnceLock<StdMutex<Option<FrameSink>>> = OnceLock::new();
    S.get_or_init(|| StdMutex::new(None))
}

fn jvm_err<E: std::fmt::Debug>(env: &Env, prefix: &str, e: E) -> anyhow::Error {
    if env.exception_check() {
        env.exception_describe();
        env.exception_clear();
    }
    anyhow::anyhow!("{prefix}: {e:?}")
}

fn java_vm() -> Result<jni::JavaVM> {
    let ctx = ndk_context::android_context();
    Ok(unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) })
}

fn with_env<T>(f: impl FnOnce(&mut Env) -> anyhow::Result<T>) -> Result<T> {
    java_vm()?
        .attach_current_thread(f)
        .map_err(KursalError::Misc)
}

fn app_context<'a>(env: &Env<'a>) -> JObject<'a> {
    unsafe {
        JObject::from_raw(
            env,
            ndk_context::android_context().context() as jni::sys::jobject,
        )
    }
}

fn load_class<'a>(env: &mut Env<'a>) -> anyhow::Result<JClass<'a>> {
    let context = app_context(env);
    let loader = env
        .call_method(
            &context,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )
        .map_err(|e| jvm_err(env, "getClassLoader", e))?
        .l()
        .map_err(|e| anyhow::anyhow!("classloader.l: {e:?}"))?;

    let name = env
        .new_string(CLASS)
        .map_err(|e| jvm_err(env, "class name", e))?;

    let class = env
        .call_method(
            loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&name)],
        )
        .map_err(|e| jvm_err(env, "loadClass", e))?
        .l()
        .map_err(|e| anyhow::anyhow!("class.l: {e:?}"))?;

    Ok(unsafe { JClass::from_raw(env, class.as_raw()) })
}

pub(super) fn list_cameras() -> Vec<CameraInfo> {
    with_env(|env| {
        let class = load_class(env)?;
        let context = app_context(env);
        let array = env
            .call_static_method(
                class,
                jni_str!("listCameras"),
                jni_sig!("(Landroid/content/Context;)[Ljava/lang/String;"),
                &[JValue::Object(&context)],
            )?
            .l()?;
        let array = unsafe { JObjectArray::<JObject>::from_raw(env, array.as_raw()) };
        let len = array.len(env)?;
        let mut cameras = Vec::new();
        for index in 0..len {
            let item = array.get_element(env, index)?;
            let item = unsafe { JString::from_raw(env, item.as_raw()) };
            let text = item.try_to_string(env)?;
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
        Ok(cameras)
    })
    .unwrap_or_default()
}

pub(super) fn start(config: &CaptureConfig, sink: FrameSink) -> Result<VideoFormat> {
    let (budget_w, budget_h, bitrate) = quality_dims(config.quality);
    *sink_slot().lock_recover() = Some(sink);

    let format = with_env(|env| {
        let class = load_class(env)?;
        let context = app_context(env);
        let requested = match config.camera_id.as_deref() {
            Some(id) => JObject::from(
                env.new_string(id)
                    .map_err(|e| jvm_err(env, "camera id", e))?,
            ),
            None => JObject::null(),
        };
        let value = env
            .call_static_method(
                class,
                jni_str!("start"),
                jni_sig!("(Landroid/content/Context;Ljava/lang/String;IIII)Ljava/lang/String;"),
                &[
                    JValue::Object(&context),
                    JValue::Object(&requested),
                    JValue::Int(i32::from(budget_w)),
                    JValue::Int(i32::from(budget_h)),
                    JValue::Int(i32::try_from(bitrate).unwrap_or(i32::MAX)),
                    JValue::Int(KEYFRAME_INTERVAL_SECS),
                ],
            )
            .map_err(|e| jvm_err(env, "CameraCapture.start", e))?;
        let handle = value.l().map_err(|e| anyhow::anyhow!("start ret: {e:?}"))?;
        if handle.is_null() {
            anyhow::bail!("camera failed to start");
        }
        let handle = unsafe { JString::from_raw(env, handle.as_raw()) };
        let descriptor = handle
            .try_to_string(env)
            .map_err(|e| jvm_err(env, "start ret string", e))?;
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
        Ok(VideoFormat {
            codec: "avc1.42E01F".to_string(),
            width,
            height,
            camera_id,
        })
    })?;
    super::set_selected_camera(format.camera_id.clone());
    Ok(format)
}

pub(super) fn stop() {
    let _ = with_env(|env| {
        let class = load_class(env)?;
        let context = app_context(env);
        env.call_static_method(
            class,
            jni_str!("stop"),
            jni_sig!("(Landroid/content/Context;)V"),
            &[JValue::Object(&context)],
        )?;
        Ok(())
    });
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
    let _ = with_env(|env| {
        let class = load_class(env)?;
        match (name, signature) {
            ("requestKeyframe", "()V") => {
                env.call_static_method(class, jni_str!("requestKeyframe"), jni_sig!("()V"), args)?;
            }
            ("setBitrate", "(I)V") => {
                env.call_static_method(class, jni_str!("setBitrate"), jni_sig!("(I)V"), args)?;
            }
            ("setDeviceAngle", "(I)V") => {
                env.call_static_method(class, jni_str!("setDeviceAngle"), jni_sig!("(I)V"), args)?;
            }
            _ => {}
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_chat_kursal_CameraCapture_nativeVideoFrame(
    mut env: EnvUnowned,
    _class: JClass,
    data: jni::sys::jbyteArray,
    keyframe: jni::sys::jboolean,
    timestamp_us: jni::sys::jlong,
    rotation: jni::sys::jint,
) {
    let _ = env
        .with_env(|env| -> anyhow::Result<()> {
            let data = unsafe { JByteArray::from_raw(env, data) };
            let Ok(bytes) = env.convert_byte_array(&data) else {
                return Ok(());
            };
            let sink = sink_slot().lock_recover().clone();
            if let Some(sink) = sink {
                sink(EncodedFrame {
                    data: bytes,
                    keyframe,
                    timestamp_us: u64::try_from(timestamp_us).unwrap_or(0),
                    rotation: u16::try_from(rotation.rem_euclid(360)).unwrap_or(0),
                });
            }
            Ok(())
        })
        .into_outcome();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_chat_kursal_CameraCapture_nativeCaptureFailed(
    _env: EnvUnowned,
    _class: JClass,
) {
    super::report_failure();
}

pub(super) fn refresh_rotation() {
    call_void(
        "setDeviceAngle",
        "(I)V",
        &[JValue::Int(i32::from(super::device_angle()))],
    );
}
