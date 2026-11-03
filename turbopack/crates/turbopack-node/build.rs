fn main() {
    if std::env::var_os("CARGO_FEATURE_WORKER_POOL_NAPI").is_some() {
        println!("cargo:rerun-if-env-changed=TYPE_DEF_TMP_PATH");
        println!("cargo:rerun-if-env-changed=CARGO_CFG_NAPI_RS_CLI_VERSION");
    }
}
