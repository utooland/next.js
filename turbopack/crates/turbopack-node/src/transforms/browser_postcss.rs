use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{
    Completion, ResolvedVc, Vc, trace::TraceRawVcs, TaskInput, NonLocalValue, fxindexmap,
};
use turbo_tasks_bytes::stream::SingleValue;
use turbo_tasks_fs::{
    File, FileContent, FileSystemPath, json::parse_json_with_source_context,
};
use turbopack_core::{
    asset::{Asset, AssetContent},
    changed::any_content_changed_of_module,
    chunk::ChunkingContext,
    context::{AssetContext, ProcessResult},
    file_source::FileSource,
    ident::AssetIdent,
    issue::IssueDescriptionExt,
    module::Module,
    reference_type::{EntryReferenceSubType, InnerAssets, ReferenceType},
    resolve::{FindContextFileResult, find_context_file_or_package_key, options::ImportMapping},
    source::Source,
    source_map::{GenerateSourceMap, OptionStringifiedSourceMap},
    source_transform::SourceTransform,
    virtual_source::VirtualSource,
};

use super::{
    util::{EmittedAsset, emitted_assets_to_virtual_sources},
    postcss::JsonSource,
    webpack::WebpackLoaderContext,
};
use crate::{
    embed_js::embed_file_path, execution_context::ExecutionContext,
    evaluate::JavaScriptEvaluation,
    transforms::webpack::evaluate_webpack_loader,
};
use turbo_tasks_env::ProcessEnv;
use turbopack_resolve::resolve_options_context::ResolveOptionsContext;
use serde_json::Value as JsonValue;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[turbo_tasks::value(serialization = "custom")]
struct BrowserPostCssProcessingResult {
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
pub enum BrowserPostCssConfigLocation {
    #[default]
    ProjectPath,
    ProjectPathOrLocalPath,
}

#[turbo_tasks::value(shared)]
#[derive(Clone, Default)]
pub struct BrowserPostCssTransformOptions {
    pub postcss_package: Option<ResolvedVc<ImportMapping>>,
    pub config_location: BrowserPostCssConfigLocation,
    pub placeholder_for_future_extensions: u8,
}

#[turbo_tasks::function]
fn browser_postcss_configs() -> Vc<Vec<RcStr>> {
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

#[turbo_tasks::value]
pub struct BrowserPostCssTransform {
    evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
    execution_context: ResolvedVc<ExecutionContext>,
    config_location: BrowserPostCssConfigLocation,
    source_maps: bool,
}

#[turbo_tasks::value_impl]
impl BrowserPostCssTransform {
    #[turbo_tasks::function]
    pub fn new(
        evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
        execution_context: ResolvedVc<ExecutionContext>,
        config_location: BrowserPostCssConfigLocation,
        source_maps: bool,
    ) -> Vc<Self> {
        BrowserPostCssTransform {
            evaluate_context,
            execution_context,
            config_location,
            source_maps,
        }
        .cell()
    }
}

#[turbo_tasks::value_impl]
impl SourceTransform for BrowserPostCssTransform {
    #[turbo_tasks::function]
    fn transform(&self, source: ResolvedVc<Box<dyn Source>>) -> Vc<Box<dyn Source>> {
        Vc::upcast(
            BrowserPostCssTransformedAsset {
                evaluate_context: self.evaluate_context,
                execution_context: self.execution_context,
                config_location: self.config_location,
                source,
                source_map: self.source_maps,
            }
            .cell(),
        )
    }
}

#[turbo_tasks::value]
struct BrowserPostCssTransformedAsset {
    evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
    execution_context: ResolvedVc<ExecutionContext>,
    config_location: BrowserPostCssConfigLocation,
    source: ResolvedVc<Box<dyn Source>>,
    source_map: bool,
}

#[turbo_tasks::value_impl]
impl Source for BrowserPostCssTransformedAsset {
    #[turbo_tasks::function]
    fn ident(&self) -> Vc<AssetIdent> {
        self.source.ident()
    }
}

#[turbo_tasks::value_impl]
impl Asset for BrowserPostCssTransformedAsset {
    #[turbo_tasks::function]
    async fn content(self: ResolvedVc<Self>) -> Result<Vc<AssetContent>> {
        let this = self.await?;
        Ok(*transform_browser_postcss_process_operation(self)
            .issue_file_path(
                this.source.ident().path().owned().await?,
                "Browser PostCSS processing",
            )
            .await?
            .connect()
            .await?
            .content)
    }
}

#[turbo_tasks::function(operation)]
fn transform_browser_postcss_process_operation(
    asset: ResolvedVc<BrowserPostCssTransformedAsset>,
) -> Vc<ProcessBrowserPostCssResult> {
    asset.process()
}

#[turbo_tasks::value]
struct ProcessBrowserPostCssResult {
    content: ResolvedVc<AssetContent>,
    assets: Vec<ResolvedVc<VirtualSource>>,
}

#[turbo_tasks::function]
async fn browser_postcss_executor(
    asset_context: Vc<Box<dyn AssetContext>>,
    project_path: FileSystemPath,
    postcss_config_path: FileSystemPath,
) -> Result<Vc<ProcessResult>> {
    // 在浏览器环境中，我们需要使用不同的方式来处理 PostCSS 配置
    // 这里我们可以使用纯 JavaScript 的 PostCSS 实现
    
    let config_asset = asset_context
        .process(
            browser_config_loader_source(project_path, postcss_config_path),
            ReferenceType::Entry(EntryReferenceSubType::Undefined),
        )
        .module()
        .to_resolved()
        .await?;

    Ok(asset_context.process(
        Vc::upcast(FileSource::new(
            embed_file_path(rcstr!("transforms/browser-postcss.ts"))
                .owned()
                .await?,
        )),
        ReferenceType::Internal(ResolvedVc::cell(fxindexmap! {
            rcstr!("CONFIG") => config_asset
        })),
    ))
}

#[turbo_tasks::function]
pub(crate) async fn browser_config_loader_source(
    project_path: FileSystemPath,
    postcss_config_path: FileSystemPath,
) -> Result<Vc<Box<dyn Source>>> {
    // 在浏览器环境中，我们需要以不同的方式加载配置
    // 这里我们可以直接返回配置内容，而不是通过 Node.js 的 import
    
    let postcss_config_path_value = postcss_config_path.clone();
    let postcss_config_path_filename = postcss_config_path_value.file_name();

    if postcss_config_path_filename == "package.json" {
        return Ok(Vc::upcast(JsonSource::new(
            postcss_config_path,
            Vc::cell(Some(rcstr!("postcss"))),
            false,
        )));
    }

    if postcss_config_path_value.path.ends_with(".json")
        || postcss_config_path_filename == ".postcssrc"
    {
        return Ok(Vc::upcast(JsonSource::new(
            postcss_config_path,
            Vc::cell(None),
            true,
        )));
    }

    // 对于 JavaScript 配置文件，我们需要在浏览器环境中以不同的方式处理
    // 这里我们可以创建一个虚拟的配置加载器
    let Some(config_path) = project_path.get_relative_path_to(&postcss_config_path_value) else {
        bail!("Unable to get relative path to postcss config");
    };

    // 在浏览器环境中，我们直接读取配置文件内容
    let file_content = postcss_config_path.read().await?;
    let config_content = match &*file_content {
        FileContent::Content(file) => file.content().to_str()?,
        FileContent::NotFound => bail!("PostCSS config file not found"),
    };
    
    // 创建一个包含配置内容的虚拟源
    let code = format!(
        "export default {};",
        serde_json::to_string(&config_content)?
    );

    Ok(Vc::upcast(VirtualSource::new(
        postcss_config_path.append("_.browser-config.mjs")?,
        AssetContent::file(File::from(code).into()),
    )))
}

async fn find_browser_config_in_location(
    project_path: FileSystemPath,
    location: BrowserPostCssConfigLocation,
    source: Vc<Box<dyn Source>>,
) -> Result<Option<FileSystemPath>> {
    if let FindContextFileResult::Found(config_path, _) =
        &*find_context_file_or_package_key(project_path, browser_postcss_configs(), rcstr!("postcss"))
            .await?
    {
        return Ok(Some(config_path.clone()));
    }

    if matches!(location, BrowserPostCssConfigLocation::ProjectPathOrLocalPath)
        && let FindContextFileResult::Found(config_path, _) = &*find_context_file_or_package_key(
            source.ident().path().await?.parent(),
            browser_postcss_configs(),
            rcstr!("postcss"),
        )
        .await?
    {
        return Ok(Some(config_path.clone()));
    }

    Ok(None)
}

#[turbo_tasks::value_impl]
impl GenerateSourceMap for BrowserPostCssTransformedAsset {
    #[turbo_tasks::function]
    async fn generate_source_map(&self) -> Result<Vc<OptionStringifiedSourceMap>> {
        let source = Vc::try_resolve_sidecast::<Box<dyn GenerateSourceMap>>(*self.source).await?;
        match source {
            Some(source) => Ok(source.generate_source_map()),
            None => Ok(Vc::cell(None)),
        }
    }
}

#[turbo_tasks::value_impl]
impl BrowserPostCssTransformedAsset {
    #[turbo_tasks::function]
    async fn process(&self) -> Result<Vc<ProcessBrowserPostCssResult>> {
        let ExecutionContext {
            project_path,
            chunking_context,
            env,
            environment: _environment
        } = &*self.execution_context.await?;

        // 查找 PostCSS 配置文件
        let Some(config_path) =
            find_browser_config_in_location(project_path.clone(), self.config_location, *self.source)
                .await?
        else {
            return Ok(ProcessBrowserPostCssResult {
                content: self.source.content().to_resolved().await?,
                assets: Vec::new(),
            }
            .cell());
        };

        let source_content = self.source.content();
        let AssetContent::File(file) = *source_content.await? else {
            bail!("Browser PostCSS transform only support transforming files");
        };
        let FileContent::Content(content) = &*file.await? else {
            return Ok(ProcessBrowserPostCssResult {
                content: AssetContent::File(FileContent::NotFound.resolved_cell()).resolved_cell(),
                assets: Vec::new(),
            }
            .cell());
        };
        let content = content.content().to_str()?;
        let evaluate_context = self.evaluate_context;
        let source_map = self.source_map;

        // 在浏览器环境中，我们需要使用不同的方式来处理配置变更
        let config_changed = browser_config_changed(*evaluate_context, config_path.clone())
            .to_resolved()
            .await?;

        let browser_postcss_executor =
            browser_postcss_executor(*evaluate_context, project_path.clone(), config_path)
                .module()
                .to_resolved()
                .await?;
        let css_fs_path = self.source.ident().path();

        // 获取相对于项目的路径
        let css_path =
            if let Some(css_path) = project_path.get_relative_path_to(&*css_fs_path.await?) {
                css_path.into_owned()
            } else {
                "".into()
            };

        // 在浏览器环境中，我们需要使用 Web Worker 或直接调用 JavaScript
        // 这里我们假设有一个浏览器环境的 webpack loader 评估函数
        let config_value = evaluate_browser_webpack_loader(BrowserWebpackLoaderContext {
            module_asset: browser_postcss_executor,
            cwd: project_path.clone(),
            env: *env,
            context_source_for_issue: self.source,
            asset_context: evaluate_context,
            chunking_context: *chunking_context,
            resolve_options_context: None,
            args: vec![
                ResolvedVc::cell(content.into()),
                ResolvedVc::cell(css_path.into()),
                ResolvedVc::cell(source_map.into()),
            ],
            additional_invalidation: config_changed,
        })
        .await?;

        let SingleValue::Single(val) = config_value.try_into_single().await? else {
            return Ok(ProcessBrowserPostCssResult {
                content: AssetContent::File(FileContent::NotFound.resolved_cell()).resolved_cell(),
                assets: Vec::new(),
            }
            .cell());
        };
        let processed_css: BrowserPostCssProcessingResult = parse_json_with_source_context(val.to_str()?)
            .context("Unable to deserializate response from Browser PostCSS transform operation")?;

        let file = File::from(processed_css.css);
        let assets = emitted_assets_to_virtual_sources(processed_css.assets).await?;
        let content =
            AssetContent::File(FileContent::Content(file).resolved_cell()).resolved_cell();
        Ok(ProcessBrowserPostCssResult { content, assets }.cell())
    }
}

// 浏览器环境的 Webpack Loader 上下文
#[derive(Clone, Debug, TaskInput, PartialEq, Eq, Hash, Serialize, Deserialize, TraceRawVcs)]
pub struct BrowserWebpackLoaderContext {
    pub module_asset: ResolvedVc<Box<dyn Module>>,
    pub cwd: FileSystemPath,
    pub env: ResolvedVc<Box<dyn ProcessEnv>>,
    pub context_source_for_issue: ResolvedVc<Box<dyn Source>>,
    pub asset_context: ResolvedVc<Box<dyn AssetContext>>,
    pub chunking_context: ResolvedVc<Box<dyn ChunkingContext>>,
    pub resolve_options_context: Option<ResolvedVc<ResolveOptionsContext>>,
    pub args: Vec<ResolvedVc<JsonValue>>,
    pub additional_invalidation: ResolvedVc<Completion>,
}

#[turbo_tasks::function]
pub(crate) async fn evaluate_browser_webpack_loader(
    browser_webpack_loader_context: BrowserWebpackLoaderContext,
) -> Result<Vc<JavaScriptEvaluation>> {
    // 用 web-worker 去跑 webpack-loader
    todo!("Implement browser webpack loader evaluation")
}

#[turbo_tasks::function]
async fn browser_config_changed(
    asset_context: Vc<Box<dyn AssetContext>>,
    postcss_config_path: FileSystemPath,
) -> Result<Vc<Completion>> {
    // 在浏览器环境中，我们需要以不同的方式处理配置变更
    // 这里可以监听文件变化或使用其他机制
    todo!("Implement browser config change detection") 
}