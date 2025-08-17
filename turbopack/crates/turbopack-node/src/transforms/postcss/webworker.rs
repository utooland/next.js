use std::{cell::RefCell, rc::Rc};

use anyhow::{Context, Result, bail};
use js_sys::Promise;
use send_wrapper::SendWrapper;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::{File, FileContent, FileSystemPath};
use turbopack_core::{
    asset::{Asset, AssetContent},
    context::AssetContext,
    ident::AssetIdent,
    issue::IssueDescriptionExt,
    resolve::{FindContextFileResult, find_context_file_or_package_key},
    source::Source,
    source_map::{GenerateSourceMap, OptionStringifiedSourceMap},
    source_transform::SourceTransform,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageEvent, Worker};

/// A wrapper to make non-Send futures appear Send for WASM environment
/// This is safe in WASM since it's single-threaded
struct WasmSafeFuture<F>(F);

impl<F> WasmSafeFuture<F> {
    fn new(future: F) -> Self {
        WasmSafeFuture(future)
    }
}

// SAFETY: In WASM, we're in a single-threaded environment, so Send is not meaningful
// and we can safely implement Send for any type
unsafe impl<F> Send for WasmSafeFuture<F> {}

impl<F: std::future::Future> std::future::Future for WasmSafeFuture<F> {
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // SAFETY: We're not moving the inner future, just projecting the Pin
        let inner = unsafe { self.map_unchecked_mut(|s| &mut s.0) };
        inner.poll(cx)
    }
}

use crate::{
    embed_js::embed_file_path,
    execution_context::ExecutionContext,
    transforms::{
        postcss::{
            PostCssConfigLocation, PostCssProcessingResult, ProcessPostCssResult, postcss_configs,
        },
        util::EmittedAsset,
    },
};

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
    async fn content(self: ResolvedVc<Self>) -> Result<Vc<AssetContent>> {
        let this = self.await?;
        Ok(*transform_webworker_process_operation(self)
            .issue_file_path(
                this.source.ident().path().owned().await?,
                "PostCSS WebWorker processing",
            )
            .await?
            .connect()
            .await?
            .content)
    }
}

#[turbo_tasks::function(operation)]
fn transform_webworker_process_operation(
    asset: ResolvedVc<PostCssTransformedAsset>,
) -> Vc<ProcessPostCssResult> {
    asset.process()
}

#[turbo_tasks::value_impl]
impl PostCssTransformedAsset {
    #[turbo_tasks::function]
    async fn process(&self) -> Result<Vc<ProcessPostCssResult>> {
        let source_content = self.source.content();
        let AssetContent::File(file) = *source_content.await? else {
            bail!("PostCSS Web Worker transform only supports transforming files");
        };
        let FileContent::Content(content) = &*file.await? else {
            return Ok(ProcessPostCssResult {
                content: AssetContent::File(FileContent::NotFound.resolved_cell()).resolved_cell(),
                assets: Vec::new(),
            }
            .cell());
        };

        let content_str = content.content().to_str()?;
        let source_map = self.source_map;

        // Get execution context for project path
        let ExecutionContext {
            project_path,
            chunking_context: _,
            env: _,
        } = &*self.execution_context.await?;

        // Find PostCSS configuration file
        let config_path =
            find_config_in_location(project_path.clone(), self.config_location, *self.source)
                .await?;

        // Get relative path for the current CSS file
        let css_fs_path = self.source.ident().path();
        let resource_path =
            if let Some(relative_path) = project_path.get_relative_path_to(&*css_fs_path.await?) {
                relative_path.to_string()
            } else {
                "input.css".to_string() // Fallback for virtual assets
            };

        // Execute PostCSS processing using WasmSafeFuture for Send compatibility
        let processing_result = {
            let content_str = content_str.clone();
            let resource_path = resource_path.clone();
            let config_path_opt = config_path.as_ref().map(|p| p.path.as_str().to_string());

            // Use WasmSafeFuture to make the non-Send WebWorker future appear Send
            WasmSafeFuture::new(async move {
                execute_postcss_with_sendwrapper(
                    &content_str,
                    &resource_path,
                    config_path_opt.as_deref(),
                    source_map,
                )
                .await
            })
            .await
            .context("Failed to process CSS with PostCSS Web Worker")?
        };

        let processed_css = processing_result.css;

        // TODO: Handle source map from processing_result.map if source_map is true
        let assets = if let Some(emitted_assets) = processing_result.assets {
            crate::transforms::util::emitted_assets_to_virtual_sources(Some(emitted_assets)).await?
        } else {
            Vec::new()
        };

        let file = File::from(processed_css);
        let content =
            AssetContent::File(FileContent::Content(file).resolved_cell()).resolved_cell();
        Ok(ProcessPostCssResult {
            content,
            assets: Vec::new(),
        }
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

async fn find_config_in_location(
    project_path: FileSystemPath,
    location: PostCssConfigLocation,
    source: Vc<Box<dyn Source>>,
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

/// Execute PostCSS processing via Web Worker with SendWrapper for WASM compatibility
async fn execute_postcss_with_sendwrapper(
    content: &str,
    resource_path: &str,
    config_path: Option<&str>,
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

    // Use SendWrapper to wrap all non-Send Web API types
    let blob_parts = SendWrapper::new(js_sys::Array::new());
    blob_parts.push(&JsValue::from_str(&worker_script));

    let blob_props = SendWrapper::new(web_sys::BlobPropertyBag::new());
    blob_props.set_type("application/javascript");

    let blob = web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &blob_props)
        .map_err(|e| anyhow::anyhow!("Failed to create worker script blob: {:?}", e))?;

    let blob_url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| anyhow::anyhow!("Failed to create blob URL for worker script: {:?}", e))?;

    // Create the Web Worker wrapped with SendWrapper
    let worker = SendWrapper::new(
        Worker::new(&blob_url)
            .map_err(|e| anyhow::anyhow!("Failed to create Web Worker: {:?}", e))?,
    );

    // Step 1: Initialize the worker with PostCSS configuration
    initialize_worker_sendwrapper(&worker, config_path)
        .await
        .context("Failed to initialize PostCSS worker")?;

    // Step 2: Send transform request to worker
    let transform_result = transform_css_sendwrapper(&worker, content, resource_path, source_map)
        .await
        .context("Failed to transform CSS with worker")?;

    // Clean up resources
    let _ = web_sys::Url::revoke_object_url(&blob_url);

    Ok(transform_result)
}

/// Initialize the PostCSS Web Worker with SendWrapper
async fn initialize_worker_sendwrapper(
    worker: &SendWrapper<Worker>,
    config_path: Option<&str>,
) -> Result<()> {
    let (resolve, reject) = create_promise_resolvers_sendwrapper();

    // Set up message handler for initialization
    let resolve_clone = resolve.clone();
    let reject_clone_msg = reject.clone();

    let onmessage = SendWrapper::new(Closure::wrap(Box::new(move |event: MessageEvent| {
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
    }) as Box<dyn FnMut(MessageEvent)>));

    // Set up error handler
    let reject_clone_err = reject.clone();
    let onerror = SendWrapper::new(Closure::wrap(Box::new(move |error: web_sys::ErrorEvent| {
        if let Some(reject_fn) = reject_clone_err.borrow_mut().take() {
            let error_msg = format!("Worker error during initialization: {:?}", error.message());
            let _ = reject_fn.call1(&JsValue::NULL, &JsValue::from_str(&error_msg));
        }
    }) as Box<dyn FnMut(web_sys::ErrorEvent)>));

    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));

    // Send initialization message with PostCSS configuration
    let init_message = if let Some(config_path) = config_path {
        json!({
            "type": "init",
            "data": {
                "config": {
                    "configPath": config_path,
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
    let promise = create_promise_from_resolvers_sendwrapper(resolve, reject);
    JsFuture::from(promise)
        .await
        .map_err(|e| anyhow::anyhow!("Worker initialization failed: {:?}", e))?;

    // Clean up event handlers
    SendWrapper::take(onmessage).forget();
    SendWrapper::take(onerror).forget();

    Ok(())
}

/// Transform CSS content using the initialized Web Worker with SendWrapper
async fn transform_css_sendwrapper(
    worker: &SendWrapper<Worker>,
    content: &str,
    resource_path: &str,
    source_map: bool,
) -> Result<PostCssProcessingResult> {
    let (resolve, reject) = create_promise_resolvers_sendwrapper();

    // Set up message handler for transformation
    let resolve_clone = resolve.clone();
    let reject_clone_msg = reject.clone();

    let onmessage = SendWrapper::new(Closure::wrap(Box::new(move |event: MessageEvent| {
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
    }) as Box<dyn FnMut(MessageEvent)>));

    // Set up error handler
    let reject_clone_err = reject.clone();
    let onerror = SendWrapper::new(Closure::wrap(Box::new(move |error: web_sys::ErrorEvent| {
        if let Some(reject_fn) = reject_clone_err.borrow_mut().take() {
            let error_msg = format!("Worker error during transformation: {:?}", error.message());
            let _ = reject_fn.call1(&JsValue::NULL, &JsValue::from_str(&error_msg));
        }
    }) as Box<dyn FnMut(web_sys::ErrorEvent)>));

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
    let promise = create_promise_from_resolvers_sendwrapper(resolve, reject);
    let result = JsFuture::from(promise)
        .await
        .map_err(|e| anyhow::anyhow!("CSS transformation failed: {:?}", e))?;

    // Clean up event handlers
    SendWrapper::take(onmessage).forget();
    SendWrapper::take(onerror).forget();

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
        assets: transform_data["assets"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|asset| serde_json::from_value::<EmittedAsset>(asset.clone()).ok())
                .collect()
        }),
    };

    Ok(processed_result)
}

/// Create promise resolver functions for async worker communication with SendWrapper
fn create_promise_resolvers_sendwrapper() -> (
    Rc<RefCell<Option<js_sys::Function>>>,
    Rc<RefCell<Option<js_sys::Function>>>,
) {
    (Rc::new(RefCell::new(None)), Rc::new(RefCell::new(None)))
}

/// Create a JS Promise from resolver functions with SendWrapper
fn create_promise_from_resolvers_sendwrapper(
    resolve: Rc<RefCell<Option<js_sys::Function>>>,
    reject: Rc<RefCell<Option<js_sys::Function>>>,
) -> Promise {
    Promise::new(&mut |resolve_fn, reject_fn| {
        *resolve.borrow_mut() = Some(resolve_fn);
        *reject.borrow_mut() = Some(reject_fn);
    })
}
