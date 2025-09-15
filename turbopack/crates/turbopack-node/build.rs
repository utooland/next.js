fn main() {
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    if std::env::var("CARGO_FEATURE_WORKER_THREAD").is_ok() {
        napi_build::setup();
    }
}
