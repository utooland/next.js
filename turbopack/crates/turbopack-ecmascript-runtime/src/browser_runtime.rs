use std::io::Write;

use anyhow::Result;
use indoc::writedoc;
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{ResolvedVc, Vc};
use turbopack_core::{
    chunk::{AssetSuffix, ChunkLoadRetry, CrossOrigin, WorkerConfigurationOptions},
    code_builder::{Code, CodeBuilder},
    context::AssetContext,
    environment::ChunkLoading,
};
use turbopack_ecmascript::utils::StringifyJs;

use crate::{RuntimeType, embed_js::embed_static_code};

pub fn chunk_update_listeners_global_name(chunk_loading_global: &str) -> String {
    format!("{chunk_loading_global}_CHUNK_UPDATE_LISTENERS")
}

#[cfg(test)]
mod tests {
    use super::chunk_update_listeners_global_name;

    #[test]
    fn scopes_chunk_update_listeners_to_chunk_loading_global() {
        assert_eq!(
            chunk_update_listeners_global_name("TURBOPACK_APP"),
            "TURBOPACK_APP_CHUNK_UPDATE_LISTENERS"
        );
    }
}

#[turbo_tasks::value(cell = "new")]
pub struct BrowserRuntimeOptions {
    pub include_async_module_runtime: bool,
    pub has_external_modules: bool,
    pub entry_root_export: Option<RcStr>,
    pub support_component_chunks: bool,
}

#[turbo_tasks::function]
pub fn browser_runtime_options(
    include_async_module_runtime: bool,
    has_external_modules: bool,
    entry_root_export: Option<RcStr>,
    support_component_chunks: bool,
) -> Vc<BrowserRuntimeOptions> {
    BrowserRuntimeOptions {
        include_async_module_runtime,
        has_external_modules,
        entry_root_export,
        support_component_chunks,
    }
    .cell()
}

/// Returns the code for the ECMAScript runtime.
#[turbo_tasks::function]
pub async fn get_browser_runtime_code(
    asset_context: ResolvedVc<Box<dyn AssetContext>>,
    chunk_base_path: Vc<Option<RcStr>>,
    worker_configuration_options: Vc<WorkerConfigurationOptions>,
    asset_suffix: Vc<AssetSuffix>,
    runtime_type: RuntimeType,
    output_root_to_root_path: RcStr,
    generate_source_map: bool,
    chunk_loading_global: Vc<RcStr>,
    cross_origin: Vc<CrossOrigin>,
    chunk_load_retry: Vc<ChunkLoadRetry>,
    chunk_loading: Vc<ChunkLoading>,
    options: Vc<BrowserRuntimeOptions>,
) -> Result<Vc<Code>> {
    let asset_context = *asset_context;
    let environment = asset_context.compile_time_info().environment();

    let shared_runtime_utils_code = embed_static_code(
        asset_context,
        rcstr!("shared/runtime/runtime-utils.ts"),
        generate_source_map,
    );

    let mut runtime_base_code = vec!["browser/runtime/base/runtime-base.ts"];
    match runtime_type {
        RuntimeType::Production => runtime_base_code.push("browser/runtime/base/build-base.ts"),
        RuntimeType::Development => {
            runtime_base_code.push("shared/runtime/hmr-runtime.ts");
            runtime_base_code.push("browser/runtime/base/dev-base.ts");
        }
        #[cfg(feature = "test")]
        RuntimeType::Dummy => {
            panic!("This configuration is not supported in the browser runtime")
        }
    }

    let chunk_loading = &*chunk_loading.await?;

    let mut runtime_backend_code = vec![];
    match (chunk_loading, runtime_type) {
        // The self-contained backend performs no runtime chunk loading and registers chunks only
        // via `globalThis`/`self` (no DOM).
        (ChunkLoading::Edge | ChunkLoading::SingleChunk, RuntimeType::Development) => {
            runtime_backend_code
                .push("browser/runtime/self-contained/runtime-backend-self-contained.ts");
            runtime_backend_code
                .push("browser/runtime/self-contained/dev-backend-self-contained.ts");
        }
        (ChunkLoading::Edge | ChunkLoading::SingleChunk, RuntimeType::Production) => {
            runtime_backend_code
                .push("browser/runtime/self-contained/runtime-backend-self-contained.ts");
        }
        // This case should never be hit.
        (ChunkLoading::NodeJs, _) => {
            panic!("Node.js runtime is not supported in the browser runtime!")
        }
        (ChunkLoading::Dom, RuntimeType::Development) => {
            runtime_backend_code.push("browser/runtime/dom/runtime-backend-dom.ts");
            runtime_backend_code.push("browser/runtime/dom/dev-backend-dom.ts");
        }
        (ChunkLoading::Dom, RuntimeType::Production) => {
            runtime_backend_code.push("browser/runtime/dom/runtime-backend-dom.ts");
        }

        #[cfg(feature = "test")]
        (_, RuntimeType::Dummy) => {
            panic!("This configuration is not supported in the browser runtime")
        }
    };

    let mut code: CodeBuilder = CodeBuilder::default();
    let relative_root_path = output_root_to_root_path;
    let chunk_base_path = chunk_base_path.await?;
    let chunk_base_path = chunk_base_path.as_ref().map_or_else(|| "", |f| f.as_str());
    let worker_configuration_options = worker_configuration_options.await?;
    // `null` (no override) and `Some("")` (empty-string prefix) are distinct
    // states, so inject as a JS literal instead of collapsing both to "".
    let worker_asset_prefix_js: String = worker_configuration_options
        .asset_prefix
        .as_ref()
        .map_or_else(
            || "null".to_string(),
            |f| format!("{}", StringifyJs(f.as_str())),
        );
    let asset_suffix = asset_suffix.await?;
    let chunk_loading_global = chunk_loading_global.await?;
    let cross_origin = *cross_origin.await?;
    let chunk_lists_global = format!("{}_CHUNK_LISTS", chunk_loading_global);
    let chunk_update_listeners_global =
        chunk_update_listeners_global_name(chunk_loading_global.as_str());
    let options = options.await?;
    let include_async_module_runtime = options.include_async_module_runtime;
    let entry_root_export = &options.entry_root_export;
    let support_component_chunks = options.support_component_chunks;

    let runtime_versions = environment.runtime_versions();
    if *runtime_versions.supports_arrow_functions().await? {
        code += "(() => {\n";
    } else {
        code += "(function(){\n";
    }
    if !*runtime_versions.supports_global_this().await? {
        // Browser chunks use `self` until this runtime executes. Defining `globalThis` here keeps
        // the embedded runtime and module factories compatible without changing application code.
        code += "if (typeof globalThis === \"undefined\") self.globalThis = self;\n";
    }
    // A shared runtime can execute before any async chunk has initialized the chunk queue. Both
    // wrapper forms below treat a missing queue as empty, while still returning when another
    // runtime has already installed its registry object.
    // Start the IIFE
    if let Some(ref export_name) = *entry_root_export {
        writedoc!(
            code,
            r#"
                (function(root, factory) {{
                    if (typeof exports === 'object' && typeof module === 'object')
                        module.exports = factory();
                    else if (typeof exports === 'object')
                        exports[{}] = factory();
                    else
                        root[{}] = factory();
                }}(typeof self !== 'undefined' ? self : this, function() {{

                const __chunk__ = (() => {{
                var chunksToRegister = globalThis["{chunk_loading_global}"];
                if (chunksToRegister === undefined) {{
                    chunksToRegister = [];
                }} else if (!Array.isArray(chunksToRegister)) {{
                    return;
                }}

                let __entryExports__ = undefined;

                var CHUNK_BASE_PATH = {};
                var WORKER_BASE_PATH = {};
                var RELATIVE_ROOT_PATH = {};
                var RUNTIME_PUBLIC_PATH = {};
                const SUPPORT_COMPONENT_CHUNKS = {};
            "#,
            StringifyJs(export_name.as_str()),
            StringifyJs(export_name.as_str()),
            StringifyJs(chunk_base_path),
            worker_asset_prefix_js,
            StringifyJs(relative_root_path.as_str()),
            StringifyJs(chunk_base_path),
            support_component_chunks,
        )?;
    } else {
        writedoc!(
            code,
            r#"
                var chunksToRegister = globalThis[{}];
                if (chunksToRegister === undefined) {{
                    chunksToRegister = [];
                }} else if (!Array.isArray(chunksToRegister)) {{
                    return;
                }}

                var CHUNK_BASE_PATH = {};
                var WORKER_BASE_PATH = {};
                var RELATIVE_ROOT_PATH = {};
                var RUNTIME_PUBLIC_PATH = {};
                const SUPPORT_COMPONENT_CHUNKS = {};
            "#,
            StringifyJs(&chunk_loading_global),
            StringifyJs(chunk_base_path),
            worker_asset_prefix_js,
            StringifyJs(relative_root_path.as_str()),
            StringifyJs(chunk_base_path),
            support_component_chunks,
        )?;
    }

    if matches!(runtime_type, RuntimeType::Development) {
        writedoc!(
            code,
            r#"
                globalThis[{chunk_update_listeners_global}] ||= [];
                var CHUNK_UPDATE_LISTENERS = {{
                    push: (registration) => globalThis[{chunk_update_listeners_global}].push(registration),
                }};
            "#,
            chunk_update_listeners_global = StringifyJs(&chunk_update_listeners_global),
        )?;
    }

    match &*asset_suffix {
        AssetSuffix::None => {
            writedoc!(
                code,
                r#"
                    var ASSET_SUFFIX = "";
                "#
            )?;
        }
        AssetSuffix::Constant(suffix) => {
            writedoc!(
                code,
                r#"
                    var ASSET_SUFFIX = {};
                "#,
                StringifyJs(suffix.as_str())
            )?;
        }
        AssetSuffix::Inferred => {
            if matches!(
                chunk_loading,
                ChunkLoading::Edge | ChunkLoading::SingleChunk
            ) {
                panic!("AssetSuffix::Inferred is not supported in Edge or single-chunk runtimes");
            }
            writedoc!(
                code,
                r#"
                    var ASSET_SUFFIX = getAssetSuffixFromScriptSrc();
                "#
            )?;
        }
        AssetSuffix::FromGlobal(global_name) => {
            writedoc!(
                code,
                r#"
                    var ASSET_SUFFIX = globalThis[{}] || "";
                "#,
                StringifyJs(global_name)
            )?;
        }
    }

    let cross_origin = cross_origin.as_str();
    writedoc!(
        code,
        r#"
            var CROSS_ORIGIN = {};
        "#,
        StringifyJs(&cross_origin)
    )?;

    // The chunk-load retry policy is owned by the framework (e.g. Next.js) and
    // passed in via the chunking context, so the runtime never hard-codes it.
    let chunk_load_retry = *chunk_load_retry.await?;
    writedoc!(
        code,
        r#"
            var CHUNK_LOAD_RETRY_MAX_ATTEMPTS = {};
            var CHUNK_LOAD_RETRY_BASE_DELAY_MS = {};
            var CHUNK_LOAD_RETRY_MAX_JITTER_MS = {};
        "#,
        chunk_load_retry.max_retry_attempts,
        chunk_load_retry.base_delay_ms,
        chunk_load_retry.max_jitter_ms,
    )?;

    writedoc!(
        code,
        r#"
            var WORKER_FORWARDED_GLOBALS = {};
        "#,
        StringifyJs(&worker_configuration_options.forwarded_globals)
    )?;

    code.push_code(&*shared_runtime_utils_code.await?);
    if include_async_module_runtime {
        code.push_code(
            &*embed_static_code(
                asset_context,
                rcstr!("shared/runtime/async-module.ts"),
                generate_source_map,
            )
            .await?,
        );
    }
    for runtime_code in runtime_base_code {
        code.push_code(
            &*embed_static_code(asset_context, runtime_code.into(), generate_source_map).await?,
        );
    }

    if options.has_external_modules || *environment.supports_commonjs_externals().await? {
        code.push_code(
            &*embed_static_code(
                asset_context,
                rcstr!("shared-node/base-externals-utils.ts"),
                generate_source_map,
            )
            .await?,
        );
    }
    if *environment.node_externals().await? {
        code.push_code(
            &*embed_static_code(
                asset_context,
                rcstr!("shared-node/node-externals-utils.ts"),
                generate_source_map,
            )
            .await?,
        );
    }
    for backend_code in runtime_backend_code {
        code.push_code(
            &*embed_static_code(asset_context, backend_code.into(), generate_source_map).await?,
        );
    }

    // Registering chunks/chunk lists depends on the BACKEND variable set by the specific
    // runtime code, so it must be appended after it. `registerChunk` handles both queued forms:
    // chunk-registration arrays and inlined entry-only params objects.
    writedoc!(
        code,
        r#"
            globalThis[{chunk_loading_global}] = {{ push: registerChunk }};
            chunksToRegister.forEach(registerChunk);
        "#,
        chunk_loading_global = StringifyJs(&chunk_loading_global),
    )?;
    if matches!(runtime_type, RuntimeType::Development) {
        writedoc!(
            code,
            r#"
            var chunkListsToRegister = globalThis[{chunk_lists_global}] || [];
            globalThis[{chunk_lists_global}] = {{ push: registerChunkList }};
            chunkListsToRegister.forEach(registerChunkList);
        "#,
            chunk_lists_global = StringifyJs(&chunk_lists_global),
        )?;
    }

    // Add expose entry exports code if enabled
    if entry_root_export.is_some() {
        writedoc!(
            code,
            r#"

                try {{
                for (const registration of chunksToRegister) {{
                    const runtimeParams = registration.length === 2 ? registration[1] : null;
                    if (runtimeParams && runtimeParams.runtimeModuleIds && runtimeParams.runtimeModuleIds.length > 0) {{
                        const entryModuleId = runtimeParams.runtimeModuleIds[runtimeParams.runtimeModuleIds.length - 1];
                        const chunkPath = getPathFromScript(registration[0]);

                        const entryModule = getOrInstantiateRuntimeModule(chunkPath, entryModuleId);

                        if (entryModule && entryModule.exports) {{
                            const moduleExports = entryModule.namespaceObject || entryModule.exports;

                            // Save for return value (will be handled by UMD wrapper)
                            __entryExports__ = moduleExports;
                        }}
                        break;
                    }}
                }}
                }} catch (e) {{
                    console.error('Failed to expose entry module exports:', e);
                }}
            "#
        )?;
    }

    // Close the IIFE and return exports if enabled
    if entry_root_export.is_some() {
        writedoc!(
            code,
            r#"
                return __entryExports__;
            }})();

            // Return the exports from the factory function
            return __chunk__;
            }}));
            "#
        )?;
    } else {
        writedoc!(
            code,
            r#"
            }})();
            "#
        )?;
    }

    Ok(Code::cell(code.build()))
}

/// Returns the code for the ECMAScript worker entrypoint bootstrap.
pub fn get_worker_runtime_code(
    asset_context: Vc<Box<dyn AssetContext>>,
    generate_source_map: bool,
) -> Result<Vc<Code>> {
    Ok(embed_static_code(
        asset_context,
        rcstr!("browser/runtime/base/worker-entrypoint.ts"),
        generate_source_map,
    ))
}
