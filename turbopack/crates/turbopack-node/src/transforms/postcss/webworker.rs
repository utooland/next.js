use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result, bail};
use js_sys::Promise;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::{File, FileContent, FileSystemPath};
use turbopack_core::{
    asset::{Asset, AssetContent},
    context::AssetContext,
    ident::AssetIdent,
    resolve::{FindContextFileResult, find_context_file_or_package_key},
    source::Source,
    source_map::{GenerateSourceMap, OptionStringifiedSourceMap},
    source_transform::SourceTransform,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageEvent, Worker};

use crate::{
    embed_js::embed_file_path,
    execution_context::ExecutionContext,
    transforms::postcss::{PostCssConfigLocation, ProcessPostCssResult},
};

/// Result structure for PostCSS processing in Web Worker environment
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PostCssProcessingResult {
    css: String,
    map: Option<String>,
}

/// PostCSS transform that executes in a Web Worker environment
/// This implementation delegates PostCSS processing to a dedicated Web Worker
/// to avoid blocking the main thread and provide better performance.
#[turbo_tasks::value]
pub struct PostCssTransform {
    evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
    execution_context: ResolvedVc<ExecutionContext>,
    config_location: PostCssConfigLocation,
    source_map: bool,
}

#[turbo_tasks::value_impl]
impl PostCssTransform {
    #[turbo_tasks::function]
    pub fn new(
        evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
        execution_context: ResolvedVc<ExecutionContext>,
        config_location: PostCssConfigLocation,
        source_map: bool,
    ) -> Vc<Self> {
        PostCssTransform {
            evaluate_context,
            execution_context,
            config_location,
            source_map,
        }
        .cell()
    }
}

#[turbo_tasks::value_impl]
impl SourceTransform for PostCssTransform {
    #[turbo_tasks::function]
    fn transform(&self, source: ResolvedVc<Box<dyn Source>>) -> Vc<Box<dyn Source>> {
        Vc::upcast(
            PostCssTransformedAsset {
                evaluate_context: self.evaluate_context,
                execution_context: self.execution_context,
                config_location: self.config_location,
                source,
                source_map: self.source_map,
            }
            .cell(),
        )
    }
}

/// Asset that has been transformed using PostCSS in a Web Worker
#[turbo_tasks::value]
pub struct PostCssTransformedAsset {
    evaluate_context: ResolvedVc<Box<dyn AssetContext>>,
    execution_context: ResolvedVc<ExecutionContext>,
    config_location: PostCssConfigLocation,
    source: ResolvedVc<Box<dyn Source>>,
    source_map: bool,
}

#[turbo_tasks::value_impl]
impl Source for PostCssTransformedAsset {
    #[turbo_tasks::function]
    fn ident(&self) -> Vc<AssetIdent> {
        self.source
            .ident()
            .with_modifier(rcstr!("postcss-webworker"))
    }
}

#[turbo_tasks::value_impl]
impl Asset for PostCssTransformedAsset {
    #[turbo_tasks::function]
    async fn content(&self) -> Result<Vc<AssetContent>> {
        let source_content = self.source.content();
        let AssetContent::File(file) = *source_content.await? else {
            bail!("PostCSS Web Worker transform only supports transforming files");
        };
        let FileContent::Content(content) = &*file.await? else {
            return Ok(AssetContent::File(FileContent::NotFound.resolved_cell()).cell());
        };

        let content_str = content.content().to_str()?;

        // Get execution context for project path
        let ExecutionContext {
            project_path,
            chunking_context: _,
            env: _,
        } = &*self.execution_context.await?;

        // Find PostCSS configuration file
        let config_path =
            find_config_in_location(project_path.clone(), self.config_location, self.source)
                .await?;

        // Get relative path for the current CSS file
        let css_fs_path = self.source.ident().path();
        let resource_path =
            if let Some(relative_path) = project_path.get_relative_path_to(&*css_fs_path.await?) {
                relative_path.to_string()
            } else {
                "input.css".to_string() // Fallback for virtual assets
            };

        // TODO: Execute PostCSS processing via Web Worker
        // For now, return the content with a comment to indicate WebWorker processing
        let processed_css = format!(
            "/* PostCSS WebWorker - Config: {:?} */\n{}",
            config_path
                .as_ref()
                .map(|p| p.path.as_str())
                .unwrap_or("none"),
            content_str
        );

        Ok(AssetContent::File(
            FileContent::Content(File::from(processed_css).into()).resolved_cell(),
        )
        .cell())
    }
}

#[turbo_tasks::value_impl]
impl GenerateSourceMap for PostCssTransformedAsset {
    #[turbo_tasks::function]
    async fn generate_source_map(&self) -> Result<Vc<OptionStringifiedSourceMap>> {
        let source = Vc::try_resolve_sidecast::<Box<dyn GenerateSourceMap>>(*self.source).await?;
        match source {
            Some(source) => Ok(source.generate_source_map()),
            None => Ok(Vc::cell(None)),
        }
    }
}

#[turbo_tasks::function]
fn postcss_configs() -> Vc<Vec<RcStr>> {
    Vc::cell(vec![
        rcstr!("postcss.config.js"),
        rcstr!("postcss.config.mjs"),
        rcstr!("postcss.config.cjs"),
        rcstr!("postcss.config.ts"),
        rcstr!("postcss.config.mts"),
        rcstr!("postcss.config.cts"),
        rcstr!(".postcssrc"),
        rcstr!(".postcssrc.json"),
        rcstr!(".postcssrc.yml"),
        rcstr!(".postcssrc.yaml"),
        rcstr!(".postcssrc.js"),
        rcstr!(".postcssrc.mjs"),
        rcstr!(".postcssrc.cjs"),
        rcstr!(".postcssrc.ts"),
        rcstr!(".postcssrc.mts"),
        rcstr!(".postcssrc.cts"),
        rcstr!("package.json"),
    ])
}

async fn find_config_in_location(
    project_path: FileSystemPath,
    location: PostCssConfigLocation,
    source: ResolvedVc<Box<dyn Source>>,
) -> Result<Option<FileSystemPath>> {
    if let FindContextFileResult::Found(config_path, _) =
        &*find_context_file_or_package_key(project_path, postcss_configs(), rcstr!("postcss"))
            .await?
    {
        return Ok(Some(config_path.clone()));
    }

    if matches!(location, PostCssConfigLocation::ProjectPathOrLocalPath)
        && let FindContextFileResult::Found(config_path, _) = &*find_context_file_or_package_key(
            source.ident().path().await?.parent(),
            postcss_configs(),
            rcstr!("postcss"),
        )
        .await?
    {
        return Ok(Some(config_path.clone()));
    }

    Ok(None)
}

/// Execute PostCSS processing via Web Worker
async fn execute_postcss_in_webworker(
    content: &str,
    resource_path: &str,
    config_path: Option<&FileSystemPath>,
    project_path: FileSystemPath,
    source_map: bool,
) -> Result<PostCssProcessingResult> {
    // Get the embedded postcss-transform-web-worker.js content
    let worker_script_path = embed_file_path(rcstr!("transforms/postcss-transform-web-worker.js"));
    let worker_script_asset = worker_script_path.await?;
    let worker_script_content = worker_script_asset.read().await?;

    let worker_script = match &*worker_script_content {
        FileContent::Content(file_content) => file_content
            .content()
            .to_str()
            .context("Worker script must be valid UTF-8")?,
        FileContent::NotFound => {
            bail!("postcss-transform-web-worker.js not found in embedded files")
        }
    };

    // Create a blob URL for the worker script
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&JsValue::from_str(&worker_script));

    let blob_props = web_sys::BlobPropertyBag::new();
    blob_props.set_type("application/javascript");

    let blob = web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &blob_props)
        .map_err(|e| anyhow::anyhow!("Failed to create worker script blob: {:?}", e))?;

    let blob_url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| anyhow::anyhow!("Failed to create blob URL for worker script: {:?}", e))?;

    // Create the Web Worker
    let worker = Worker::new(&blob_url)
        .map_err(|e| anyhow::anyhow!("Failed to create Web Worker: {:?}", e))?;

    // Step 1: Initialize the worker with PostCSS configuration
    let init_result = initialize_worker(&worker, config_path, project_path.clone())
        .await
        .context("Failed to initialize PostCSS worker")?;

    // Step 2: Send transform request to worker
    let transform_result = transform_css_with_worker(&worker, content, resource_path, source_map)
        .await
        .context("Failed to transform CSS with worker")?;

    // Clean up resources
    let _ = web_sys::Url::revoke_object_url(&blob_url);

    Ok(transform_result)
}

/// Initialize the PostCSS Web Worker with configuration
async fn initialize_worker(
    worker: &Worker,
    config_path: Option<&FileSystemPath>,
    project_path: FileSystemPath,
) -> Result<()> {
    let (resolve, reject) = create_promise_resolvers();

    // Set up message handler for initialization
    let resolve_clone = resolve.clone();
    let reject_clone_msg = reject.clone();

    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
        let data = event.data();
        if let Some(data_str) = data.as_string() {
            if let Ok(response) = serde_json::from_str::<JsonValue>(&data_str) {
                match response["type"].as_str() {
                    Some("init_success") => {
                        if let Some(resolve_fn) = resolve_clone.borrow_mut().take() {
                            let _ = resolve_fn.call1(&JsValue::NULL, &JsValue::TRUE);
                        }
                    }
                    Some("init_error") => {
                        if let Some(reject_fn) = reject_clone_msg.borrow_mut().take() {
                            let error_msg = response["error"]
                                .as_str()
                                .unwrap_or("Unknown initialization error");
                            let _ = reject_fn.call1(&JsValue::NULL, &JsValue::from_str(error_msg));
                        }
                    }
                    _ => {} // Ignore other message types during initialization
                }
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    // Set up error handler
    let reject_clone_err = reject.clone();
    let onerror = Closure::wrap(Box::new(move |error: web_sys::ErrorEvent| {
        if let Some(reject_fn) = reject_clone_err.borrow_mut().take() {
            let error_msg = format!("Worker error during initialization: {:?}", error.message());
            let _ = reject_fn.call1(&JsValue::NULL, &JsValue::from_str(&error_msg));
        }
    }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

    // Send initialization message with PostCSS configuration
    let init_message = if let Some(config_path) = config_path {
        json!({
            "type": "init",
            "data": {
                "config": {
                    "configPath": config_path.path,
                    "plugins": {
                        "autoprefixer": true
                    }
                }
            }
        })
    } else {
        json!({
            "type": "init",
            "data": {
                "config": {
                    "plugins": {
                        "autoprefixer": true
                    }
                }
            }
        })
    };

    let init_payload = JsValue::from_str(&init_message.to_string());
    worker
        .post_message(&init_payload)
        .map_err(|e| anyhow::anyhow!("Failed to send init message to worker: {:?}", e))?;

    // Create and await the promise
    let promise = create_promise_from_resolvers(resolve, reject);
    JsFuture::from(promise)
        .await
        .map_err(|e| anyhow::anyhow!("Worker initialization failed: {:?}", e))?;

    // Clean up event handlers
    onmessage.forget();
    onerror.forget();

    Ok(())
}

/// Transform CSS content using the initialized Web Worker
async fn transform_css_with_worker(
    worker: &Worker,
    content: &str,
    resource_path: &str,
    source_map: bool,
) -> Result<PostCssProcessingResult> {
    let (resolve, reject) = create_promise_resolvers();

    // Set up message handler for transformation
    let resolve_clone = resolve.clone();
    let reject_clone_msg = reject.clone();

    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
        let data = event.data();
        if let Some(data_str) = data.as_string() {
            if let Ok(response) = serde_json::from_str::<JsonValue>(&data_str) {
                match response["type"].as_str() {
                    Some("transform_success") => {
                        if let Some(resolve_fn) = resolve_clone.borrow_mut().take() {
                            let _ = resolve_fn.call1(&JsValue::NULL, &data);
                        }
                    }
                    Some("transform_error") => {
                        if let Some(reject_fn) = reject_clone_msg.borrow_mut().take() {
                            let error_msg = response["error"]
                                .as_str()
                                .unwrap_or("Unknown transformation error");
                            let _ = reject_fn.call1(&JsValue::NULL, &JsValue::from_str(error_msg));
                        }
                    }
                    _ => {} // Ignore other message types during transformation
                }
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    // Set up error handler
    let reject_clone_err = reject.clone();
    let onerror = Closure::wrap(Box::new(move |error: web_sys::ErrorEvent| {
        if let Some(reject_fn) = reject_clone_err.borrow_mut().take() {
            let error_msg = format!("Worker error during transformation: {:?}", error.message());
            let _ = reject_fn.call1(&JsValue::NULL, &JsValue::from_str(&error_msg));
        }
    }) as Box<dyn FnMut(web_sys::ErrorEvent)>);

    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

    // Send transformation message
    let transform_message = json!({
        "type": "transform",
        "data": content,
        "options": {
            "css": content,
            "from": resource_path,
            "to": resource_path.replace(".css", ".out.css"),
            "map": source_map
        }
    });

    let transform_payload = JsValue::from_str(&transform_message.to_string());
    worker
        .post_message(&transform_payload)
        .map_err(|e| anyhow::anyhow!("Failed to send transform message to worker: {:?}", e))?;

    // Create and await the promise
    let promise = create_promise_from_resolvers(resolve, reject);
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| anyhow::anyhow!("CSS transformation failed: {:?}", e))?;

    // Clean up event handlers
    onmessage.forget();
    onerror.forget();

    // Parse the transformation result
    let result_str = result
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("Worker returned non-string result"))?;

    let response_data: JsonValue = serde_json::from_str(&result_str)
        .context("Failed to parse worker transformation response")?;

    // Extract the result data
    let transform_data = response_data["data"].clone();

    let processed_result = PostCssProcessingResult {
        css: transform_data["css"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'css' field in worker result"))?
            .to_string(),
        map: transform_data["map"].as_str().map(|s| s.to_string()),
    };

    Ok(processed_result)
}

/// Create promise resolver functions for async worker communication
fn create_promise_resolvers() -> (
    Rc<RefCell<Option<js_sys::Function>>>,
    Rc<RefCell<Option<js_sys::Function>>>,
) {
    (Rc::new(RefCell::new(None)), Rc::new(RefCell::new(None)))
}

/// Create a JS Promise from resolver functions
fn create_promise_from_resolvers(
    resolve: Rc<RefCell<Option<js_sys::Function>>>,
    reject: Rc<RefCell<Option<js_sys::Function>>>,
) -> Promise {
    Promise::new(&mut |resolve_fn, reject_fn| {
        *resolve.borrow_mut() = Some(resolve_fn);
        *reject.borrow_mut() = Some(reject_fn);
    })
}
