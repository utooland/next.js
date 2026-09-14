#[cfg(all(target_family = "wasm", target_os = "unknown"))]
use std::path::PathBuf;
use std::{
    io::{self, ErrorKind},
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use turbo_tasks::ResolvedVc;
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use url::Url;

use crate::{DiskFileSystem, FileSystemPath};

/// Converts a disk access `Result<T>` into a `Result<Some<T>>`, where a [`ErrorKind::NotFound`] (or
/// [`ErrorKind::InvalidFilename`]) error results in a [`None`] value. This is purely to reduce
/// boilerplate code comparing [`ErrorKind::NotFound`] errors against all other errors.
pub fn extract_disk_access<T>(value: io::Result<T>, path: &Path) -> Result<Option<T>> {
    match value {
        Ok(v) => Ok(Some(v)),
        Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::InvalidFilename) => Ok(None),
        // ast-grep-ignore: no-context-format
        Err(e) => Err(anyhow!(e).context(format!("reading file {}", path.display()))),
    }
}

pub async fn uri_from_file(root: FileSystemPath, path: Option<&str>) -> Result<String> {
    let root_fs = root.fs;
    let root_fs = &*ResolvedVc::try_downcast_type::<DiskFileSystem>(root_fs)
        .context("Expected root to have a DiskFileSystem")?
        .await?;

    let path = match path {
        Some(path) => root.join(path)?,
        None => root,
    };

    // `to_sys_path` returns a win32 path on Windows. `Url::from_file_path` can also handle
    // verbatim (`\\?\`-prefixed) disk and UNC paths, in case that conversion failed.
    let sys_path = root_fs.to_sys_path(&path);
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    return Ok(String::from(Url::from_file_path(&sys_path).map_err(
        |_| anyhow!("path {sys_path:?} cannot be converted to a file:// URI"),
    )?));

    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    Ok(uri_from_path_buf(sys_path))
}

/// Builds a file URI lexically for OPFS paths. `url::Url::from_file_path` is unavailable on
/// `wasm32-unknown-unknown`, and OPFS paths do not need native canonicalization.
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub fn uri_from_path_buf(sys_path: PathBuf) -> String {
    use turbo_unix_path::sys_to_unix;

    let path = sys_to_unix(&sys_path.to_string_lossy())
        .split('/')
        .map(|segment| urlencoding::encode(segment))
        .collect::<Vec<_>>()
        .join("/");
    format!("file://{path}")
}
