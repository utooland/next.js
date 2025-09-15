use serde::{Deserialize, Serialize};
use turbo_rcstr::RcStr;
use turbo_tasks::{NonLocalValue, OperationValue, ResolvedVc, trace::TraceRawVcs};

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub mod nodejs;

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub use nodejs::{WebpackLoaders, WebpackLoadersProcessedAsset};

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub mod webworker;

use turbopack_core::{
    asset::AssetContent, source_map::OptionStringifiedSourceMap, virtual_source::VirtualSource,
};
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use webworker::{WebpackLoaders, WebpackLoadersProcessedAsset};

#[derive(
    Clone, PartialEq, Eq, Debug, TraceRawVcs, Serialize, Deserialize, NonLocalValue, OperationValue,
)]
pub struct WebpackLoaderItem {
    pub loader: RcStr,
    pub options: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
#[turbo_tasks::value(shared, transparent)]
pub struct WebpackLoaderItems(pub Vec<WebpackLoaderItem>);

#[turbo_tasks::value(shared)]
#[derive(Clone, Default)]
pub struct WebpackLoadersTransformOptions {
    pub source_maps: bool,
    pub placeholder_for_future_extensions: u8,
}

#[turbo_tasks::value]
pub struct ProcessWebpackLoadersResult {
    content: ResolvedVc<AssetContent>,
    source_map: ResolvedVc<OptionStringifiedSourceMap>,
    assets: Vec<ResolvedVc<VirtualSource>>,
}
