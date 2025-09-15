use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use tokio::sync::oneshot;
use tokio_fs_ext::offload::{self, FsOffload};

static WASM_FS_OFFLOAD: LazyLock<(Mutex<offload::Server>, offload::Client)> = LazyLock::new(|| {
    let (server, client) = offload::split();
    (Mutex::new(server), client)
});

pub static CLIENT: LazyLock<offload::Client> = LazyLock::new(|| WASM_FS_OFFLOAD.1.clone());

pub async fn server(offload: impl FsOffload) {
    WASM_FS_OFFLOAD.0.lock().serve(offload).await
}
