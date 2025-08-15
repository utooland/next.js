use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use either::Either;
use js_sys::{Function, Object, Promise, Reflect};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use serde_with::serde_as;
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{Completion, NonLocalValue, ResolvedVc, TaskInput, Vc, trace::TraceRawVcs};
use turbo_tasks_bytes::stream::SingleValue;
use turbo_tasks_fs::{
    File, FileContent, FileSystemPath, json::parse_json_with_source_context, rope::Rope,
};
use turbopack_core::{
    asset::{Asset, AssetContent},
    context::{AssetContext, ProcessResult},
    file_source::FileSource,
    ident::AssetIdent,
    reference_type::{InnerAssets, ReferenceType},
    source::Source,
    source_map::{
        GenerateSourceMap, OptionStringifiedSourceMap, utils::resolve_source_map_sources,
    },
    source_transform::SourceTransform,
    virtual_source::VirtualSource,
};
use turbopack_resolve::resolve_options_context::ResolveOptionsContext;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, Worker};

use crate::{
    embed_js::embed_file_path,
    execution_context::ExecutionContext,
    transforms::{
        util::{EmittedAsset, emitted_assets_to_virtual_sources},
        webpack::{ProcessWebpackLoadersResult, WebpackLoaderItem, WebpackLoaderItems},
    },
};

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct BytesBase64 {
    #[serde_as(as = "serde_with::base64::Base64")]
    binary: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[turbo_tasks::value(serialization = "custom")]
struct WebpackLoadersProcessingResult {
    #[serde(with = "either::serde_untagged")]
    #[turbo_tasks(debug_ignore, trace_ignore)]
    source: Either<RcStr, BytesBase64>,
    map: Option<RcStr>,
    #[turbo_tasks(trace_ignore)]
    assets: Option<Vec<EmittedAsset>>,
}

#[turbo_tasks::value]
pub struct WebpackLoaders {
    evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
    execution_context: ResolvedVc<ExecutionContext>,
    loaders: ResolvedVc<WebpackLoaderItems>,
    rename_as: Option<RcStr>,
    resolve_options_context: ResolvedVc<ResolveOptionsContext>,
    source_maps: bool,
}

#[turbo_tasks::value_impl]
impl WebpackLoaders {
    #[turbo_tasks::function]
    pub fn new(
        evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
        execution_context: ResolvedVc<ExecutionContext>,
        loaders: ResolvedVc<WebpackLoaderItems>,
        rename_as: Option<RcStr>,
        resolve_options_context: ResolvedVc<ResolveOptionsContext>,
        source_maps: bool,
    ) -> Vc<Self> {
        WebpackLoaders {
            evaluate_context,
            execution_context,
            loaders,
            rename_as,
            resolve_options_context,
            source_maps,
        }
        .cell()
    }
}

#[turbo_tasks::value_impl]
impl SourceTransform for WebpackLoaders {
    #[turbo_tasks::function]
    async fn transform(self: Vc<Self>, source: Vc<Box<dyn Source>>) -> Result<Vc<Box<dyn Source>>> {
        Ok(Vc::upcast(
            WebpackLoadersProcessedAsset {
                webpack_loaders: self.to_resolved().await?,
                source: source.to_resolved().await?,
            }
            .cell(),
        ))
    }
}

#[turbo_tasks::value]
pub struct WebpackLoadersProcessedAsset {
    webpack_loaders: ResolvedVc<WebpackLoaders>,
    source: ResolvedVc<Box<dyn Source>>,
}

#[turbo_tasks::value_impl]
impl Source for WebpackLoadersProcessedAsset {
    #[turbo_tasks::function]
    async fn ident(&self) -> Result<Vc<AssetIdent>> {
        let webpack_loaders = self.webpack_loaders.await?;
        if let Some(rename_as) = &webpack_loaders.rename_as {
            Ok(self.source.ident().rename_as(rename_as.clone()))
        } else {
            Ok(self
                .source
                .ident()
                .with_modifier(rcstr!("webpack-webworker")))
        }
    }
}

#[turbo_tasks::value_impl]
impl Asset for WebpackLoadersProcessedAsset {
    #[turbo_tasks::function]
    async fn content(&self) -> Result<Vc<AssetContent>> {
        // Ok(*self.process().await?.content)
        // TODO: use webpack-loader-webworker executor here.
        let source_content = self.source.content();
        let AssetContent::File(file) = *source_content.await? else {
            bail!("Webpack Loaders WebWorker transform only support transforming files");
        };
        let FileContent::Content(content) = &*file.await? else {
            return Ok(AssetContent::File(FileContent::NotFound.resolved_cell()).cell());
        };
        let content_str = content.content().to_str()?;
        let processed_content = format!("/* Processed by WebWorker */\n{}", content_str);

        Ok(AssetContent::File(
            FileContent::Content(File::from(processed_content).into()).resolved_cell(),
        )
        .cell())
    }
}

// #[turbo_tasks::value_impl]
// impl WebpackLoadersProcessedAsset {
//     #[turbo_tasks::function]
//     async fn process(self: Vc<Self>) -> Result<Vc<ProcessWebpackLoadersResult>> {
// let this = self.await?;
// let transform = this.transform.await?;

// let ExecutionContext {
//     project_path,
//     chunking_context,
//     env,
// } = &*transform.execution_context.await?;
// let source_content = this.source.content();
// let AssetContent::File(file) = *source_content.await? else {
//     bail!("Webpack Loaders transform only support transforming files");
// };
// let FileContent::Content(file_content) = &*file.await? else {
//     return Ok(ProcessWebpackLoadersResult {
//         content: AssetContent::File(FileContent::NotFound.resolved_cell()).resolved_cell(),
//         assets: Vec::new(),
//         source_map: ResolvedVc::cell(None),
//     }
//     .cell());
// };

// // If the content is not a valid string (e.g. binary file), handle the error and pass a
// // Buffer to Webpack instead of a Base64 string so the build process doesn't crash.
// let _content: JsonValue = match file_content.content().to_str() {
//     Ok(utf8_str) => utf8_str.to_string().into(),
//     Err(_) => JsonValue::Object(JsonMap::from_iter(std::iter::once((
//         "binary".to_string(),
//         JsonValue::from(
//             base64::engine::general_purpose::STANDARD
//                 .encode(file_content.content().to_bytes()),
//         ),
//     )))),
// };
// let _evaluate_context = transform.evaluate_context;

// Ok(ProcessWebpackLoadersResult {
//     content: AssetContent::File(FileContent::Content(file).resolved_cell()).resolved_cell(),
//     assets: Vec::new(),
//     source_map: ResolvedVc::cell(None),
// }
// .cell())
//         Ok(Vc::cell(None))
//     }
// }

#[turbo_tasks::value_impl]
impl GenerateSourceMap for WebpackLoadersProcessedAsset {
    #[turbo_tasks::function]
    async fn generate_source_map(&self) -> Result<Vc<OptionStringifiedSourceMap>> {
        // For now, return no source map
        // TODO: Implement actual source map generation from WebWorker
        Ok(Vc::cell(None))
    }
}

#[derive(Debug, Clone)]
#[turbo_tasks::value(shared)]
struct WebpackLoadersWebworkerParams {
    evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
    execution_context: ResolvedVc<ExecutionContext>,
    loaders: ResolvedVc<WebpackLoaderItems>,
    loader_runner_package: Option<ResolvedVc<turbopack_core::resolve::options::ImportMapping>>,
    source: ResolvedVc<Box<dyn Source>>,
    source_maps: bool,
}

async fn execute_webpack_loaders_webworker(params: WebpackLoadersWebworkerParams) -> Result<RcStr> {
    let source_content = params.source.content();
    let AssetContent::File(file) = *source_content.await? else {
        bail!("Webpack Loaders WebWorker transform only support transforming files");
    };
    let FileContent::Content(content) = &*file.await? else {
        return Ok(serde_json::to_string(&WebpackLoadersProcessingResult {
            source: Either::Left("".into()),
            map: None,
            assets: None,
        })?
        .into());
    };

    let ExecutionContext {
        project_path,
        chunking_context: _,
        env: _,
    } = &*params.execution_context.await?;

    let loaders = &*params.loaders.await?;
    let content_bytes = content.content().to_bytes();
    let source_path = params.source.ident().path().await?;

    let resource_path =
        if let Some(relative_path) = project_path.get_relative_path_to(&*source_path) {
            relative_path.to_string()
        } else {
            source_path.path.to_string()
        };

    // Execute webpack loaders directly in WebWorker environment
    let result = execute_in_webworker(
        &content_bytes,
        &resource_path,
        "",
        &*loaders,
        params.source_maps,
        project_path.clone(),
    )
    .await?;

    Ok(serde_json::to_string(&result)?.into())
}

// WebWorker execution function - delegates all loader processing to webpack-loaders-webworker.ts
async fn execute_in_webworker(
    content: &[u8],
    resource_path: &str,
    query: &str,
    loaders: &[WebpackLoaderItem],
    source_map: bool,
    cwd: FileSystemPath,
) -> Result<WebpackLoadersProcessingResult> {
    // Prepare the content for the WebWorker
    let content_value = if content.is_empty() {
        JsonValue::String("".to_string())
    } else {
        // Try to convert to string first, if it fails, use base64
        match std::str::from_utf8(content) {
            Ok(text) => JsonValue::String(text.to_string()),
            Err(_) => {
                // If it's binary content, wrap it in the binary format expected by
                // webpack-loaders-webworker.ts
                json!({
                    "binary": BASE64_STANDARD.encode(content)
                })
            }
        }
    };

    // Prepare WebWorker payload matching webpack-loaders-webworker.ts transform function signature
    let payload = json!({
        "content": content_value,
        "name": resource_path,
        "query": query,
        "loaders": loaders,
        "sourceMap": source_map,
        "cwd": cwd.path
    });

    execute_in_webworker_wasm(payload).await
}

async fn execute_in_webworker_wasm(payload: JsonValue) -> Result<WebpackLoadersProcessingResult> {
    // Get the embedded webpack-loaders-webworker.js content
    let worker_script_path = embed_file_path(rcstr!("transforms/webpack-loaders-webworker.js"))
        .owned()
        .await?;
    let worker_script_content = worker_script_path.read().await?;
    let worker_script = match &*worker_script_content {
        FileContent::Content(content) => content.content().to_str()?,
        _ => bail!("webpack-loaders-webworker.js not found"),
    };

    // Create a blob URL for the worker script
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&JsValue::from_str(&worker_script));
    let blob_props = web_sys::BlobPropertyBag::new();
    blob_props.set_type("application/javascript");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &blob_props)
        .map_err(|e| anyhow::anyhow!("Failed to create blob: {:?}", e))?;
    let blob_url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| anyhow::anyhow!("Failed to create blob URL: {:?}", e))?;

    // Create the worker
    let worker =
        Worker::new(&blob_url).map_err(|e| anyhow::anyhow!("Failed to create worker: {:?}", e))?;

    // Create a promise that resolves when the worker responds
    let promise = Promise::new(&mut |resolve, reject| {
        // Create message handler
        let resolve = Rc::new(RefCell::new(Some(resolve)));
        let reject = Rc::new(RefCell::new(Some(reject)));

        let resolve_clone = resolve.clone();
        let reject_clone = reject.clone();

        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            if let Some(resolve) = resolve_clone.borrow_mut().take() {
                let _ = resolve.call1(&JsValue::NULL, &event.data());
            }
        }) as Box<dyn FnMut(MessageEvent)>);

        let onerror = Closure::wrap(Box::new(move |error: web_sys::ErrorEvent| {
            if let Some(reject) = reject_clone.borrow_mut().take() {
                let _ = reject.call1(&JsValue::NULL, &error.into());
            }
        }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        // Send the payload to the worker
        let payload_js = JsValue::from_str(&payload.to_string());
        let _ = worker.post_message(&payload_js);

        // Keep closures alive
        onmessage.forget();
        onerror.forget();
    });

    // Wait for the worker to respond
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| anyhow::anyhow!("Worker execution failed: {:?}", e))?;

    // Clean up the blob URL
    let _ = web_sys::Url::revoke_object_url(&blob_url);

    // Parse the result
    let result_str = result
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("Worker returned non-string result"))?;

    let processed: WebpackLoadersProcessingResult =
        serde_json::from_str(&result_str).context("Failed to parse worker result")?;

    Ok(processed)
}
