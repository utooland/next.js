use std::sync::LazyLock;

use parking_lot::Mutex;
use tokio_fs_ext::offload::{self, FsOffload};

static WASM_FS_OFFLOAD: LazyLock<(Mutex<Option<offload::Server>>, offload::Client)> =
    LazyLock::new(|| {
        let (server, client) = offload::split();
        (Mutex::new(Some(server)), client)
    });

pub static CLIENT: LazyLock<offload::Client> = LazyLock::new(|| WASM_FS_OFFLOAD.1.clone());

pub async fn server(offload: impl FsOffload) {
    let mut server = WASM_FS_OFFLOAD
        .0
        .lock()
        .take()
        .expect("WASM filesystem offload server can only be started once");
    server.serve(offload).await
}
