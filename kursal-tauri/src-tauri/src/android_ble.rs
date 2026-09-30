#![cfg(target_os = "android")]

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::jbyteArray;
use jni::{EnvUnowned, errors::LogErrorAndDefault};
use kursal_core::first_contact::nearby::bluetooth as bt;

#[unsafe(no_mangle)]
pub extern "system" fn Java_chat_kursal_BleAdvertiser_nativeOnReadRequest(
    mut env: EnvUnowned,
    _class: JClass,
) -> jbyteArray {
    let bytes = bt::android_handle_read();
    env.with_env(|env| -> jni::errors::Result<jbyteArray> {
        env.byte_array_from_slice(&bytes)
            .map(|array| array.into_raw())
    })
    .resolve::<LogErrorAndDefault>()
}

#[unsafe(no_mangle)]
/// # Safety
///
/// The JVM invokes this callback with a valid byte-array reference for the current JNI frame.
pub unsafe extern "system" fn Java_chat_kursal_BleAdvertiser_nativeOnWriteRequest(
    mut env: EnvUnowned,
    _class: JClass,
    client: JString,
    data: jbyteArray,
) {
    env.with_env(|env| -> jni::errors::Result<()> {
        let client_str = client.try_to_string(env)?;
        let data = unsafe { JByteArray::from_raw(env, data) };
        let data_vec = env.convert_byte_array(&data)?;
        bt::android_handle_write(&client_str, &data_vec);
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
}
