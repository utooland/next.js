use serde::{Deserialize, Serialize};
use turbo_rcstr::RcStr;
use turbo_tasks::{NonLocalValue, ResolvedVc, TaskInput, Vc, trace::TraceRawVcs};
use turbopack_core::{
    asset::AssetContent, resolve::options::ImportMapping, virtual_source::VirtualSource,
};

use super::util::EmittedAsset;

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub mod nodejs;
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub use nodejs::{PostCssTransform, PostCssTransformedAsset};

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub mod webworker;
#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use webworker::{PostCssTransform, PostCssTransformedAsset};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[turbo_tasks::value(serialization = "custom")]
struct PostCssProcessingResult {
    css: String,
    map: Option<String>,
    assets: Option<Vec<EmittedAsset>>,
}

#[derive(
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Debug,
    TraceRawVcs,
    Serialize,
    Deserialize,
    TaskInput,
    NonLocalValue,
)]
pub enum PostCssConfigLocation {
    #[default]
    ProjectPath,
    ProjectPathOrLocalPath,
}

#[turbo_tasks::value(shared)]
#[derive(Clone, Default)]
pub struct PostCssTransformOptions {
    pub postcss_package: Option<ResolvedVc<ImportMapping>>,
    pub config_location: PostCssConfigLocation,
    pub placeholder_for_future_extensions: u8,
}

#[turbo_tasks::value]
struct ProcessPostCssResult {
    content: ResolvedVc<AssetContent>,
    assets: Vec<ResolvedVc<VirtualSource>>,
}

#[turbo_tasks::function]
fn postcss_configs() -> Vc<Vec<RcStr>> {
    Vc::cell(
        [
            ".postcssrc",
            ".postcssrc.json",
            ".postcssrc.yaml",
            ".postcssrc.yml",
            ".postcssrc.js",
            ".postcssrc.mjs",
            ".postcssrc.cjs",
            ".config/postcssrc",
            ".config/postcssrc.json",
            ".config/postcssrc.yaml",
            ".config/postcssrc.yml",
            ".config/postcssrc.js",
            ".config/postcssrc.mjs",
            ".config/postcssrc.cjs",
            "postcss.config.js",
            "postcss.config.mjs",
            "postcss.config.cjs",
            "postcss.config.json",
        ]
        .into_iter()
        .map(RcStr::from)
        .collect(),
    )
}
