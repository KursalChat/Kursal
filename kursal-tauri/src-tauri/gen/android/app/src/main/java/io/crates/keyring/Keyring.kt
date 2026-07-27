package io.crates.keyring

import android.content.Context

// Bridges to the JNI symbol exported by android-native-keyring-store
// (Java_io_crates_keyring_Keyring_00024Companion_initializeNdkContext),
// which populates the ndk_context crate global. tao 0.35+ no longer does
// this, so keychain/cpal/bluetooth would otherwise panic with
// "android context was not initialized". Must run before any Rust code
// touches the keychain.
class Keyring {
    companion object {
        init {
            System.loadLibrary("kursal_tauri_lib")
        }

        external fun initializeNdkContext(context: Context)
    }
}
