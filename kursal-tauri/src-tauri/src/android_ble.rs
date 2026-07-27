#![cfg(target_os = "android")]

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jbyteArray;
use kursal_core::first_contact::nearby::bluetooth as bt;

#[unsafe(no_mangle)]
pub extern "system" fn Java_chat_kursal_BleAdvertiser_nativeOnReadRequest(
    env: JNIEnv,
    _class: JClass,
) -> jbyteArray {
    let bytes = bt::android_handle_read();
    match env.byte_array_from_slice(&bytes) {
        Ok(arr) => arr,
        Err(err) => {
            log::warn!("[bt] nativeOnReadRequest byte_array: {err:?}");
            let _ = env.exception_clear();
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_chat_kursal_BleAdvertiser_nativeOnWriteRequest(
    env: JNIEnv,
    _class: JClass,
    client: JString,
    data: jbyteArray,
) {
    let client_str: String = match env.get_string(client) {
        Ok(s) => s.into(),
        Err(err) => {
            log::warn!("[bt] nativeOnWriteRequest get_string: {err:?}");
            let _ = env.exception_clear();
            return;
        }
    };
    let data_vec = match env.convert_byte_array(data) {
        Ok(v) => v,
        Err(err) => {
            log::warn!("[bt] nativeOnWriteRequest convert_byte_array: {err:?}");
            let _ = env.exception_clear();
            return;
        }
    };
    bt::android_handle_write(&client_str, &data_vec);
}
