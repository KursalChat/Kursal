# Vendored droidplug sources

`com/nonpolynomial/**` and `io/github/gedgygedgy/**` are **not** ours. They are
copied verbatim from the btleplug crate:

    ~/.cargo/registry/src/*/btleplug-<version>/src/droidplug/java/src/main/java/

btleplug's Android backend is split across Rust and Java: `platform::init()`
registers native methods against `com.nonpolynomial.btleplug.android.impl.Adapter`
by name, and the crate ships the Java half as source rather than publishing an
artifact, so it has to be compiled into the app. Without these classes every BLE
call fails class lookup and takes the process down with it.

Currently vendored from **btleplug 0.12.0**. Re-copy the tree whenever the
btleplug dependency in `Cargo.toml` changes. They need no dependencies beyond the
Android framework, and `proguard-rules.pro` keeps them from being stripped.
