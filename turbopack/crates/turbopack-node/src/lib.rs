#![feature(min_specialization)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]

use std::iter::once;

use anyhow::Result;
use rustc_hash::FxHashMap;
use turbo_tasks::{
    FxIndexSet, ResolvedVc, TryJoinIterExt, Vc,
    graph::{AdjacencyMap, GraphTraversal},
};
use turbo_tasks_fs::{File, FileSystemPath};
use turbopack_core::{
    asset::{Asset, AssetContent},
    output::{OutputAsset, OutputAssetsSet},
    source_map::GenerateSourceMap,
    virtual_output::VirtualOutputAsset,
};

pub mod debug;
pub mod embed_js;
pub mod evaluate;
pub mod execution_context;
mod heap_queue;
mod pool;
pub mod source_map;
pub mod transforms;

#[turbo_tasks::function]
async fn emit(
    intermediate_asset: Vc<Box<dyn OutputAsset>>,
    intermediate_output_path: FileSystemPath,
) -> Result<()> {
    for asset in internal_assets(intermediate_asset, intermediate_output_path).await? {
        let _ = asset
            .content()
            .write(asset.path().owned().await?)
            .resolve()
            .await?;
    }
    Ok(())
}

/// List of the all assets of the "internal" subgraph and a list of boundary
/// assets that are not considered "internal" ("external")
#[derive(Debug)]
#[turbo_tasks::value]
struct SeparatedAssets {
    internal_assets: ResolvedVc<OutputAssetsSet>,
    external_asset_entrypoints: ResolvedVc<OutputAssetsSet>,
}

/// Extracts the subgraph of "internal" assets (assets within the passes
/// directory). Also lists all boundary assets that are not part of the
/// "internal" subgraph.
#[turbo_tasks::function]
async fn internal_assets(
    intermediate_asset: ResolvedVc<Box<dyn OutputAsset>>,
    intermediate_output_path: FileSystemPath,
) -> Result<Vc<OutputAssetsSet>> {
    Ok(
        *separate_assets_operation(intermediate_asset, intermediate_output_path)
            .read_strongly_consistent()
            .await?
            .internal_assets,
    )
}

#[turbo_tasks::value(transparent)]
pub struct AssetsForSourceMapping(FxHashMap<String, ResolvedVc<Box<dyn GenerateSourceMap>>>);

/// Extracts a map of "internal" assets ([`internal_assets`]) which implement
/// the [GenerateSourceMap] trait.
#[turbo_tasks::function]
async fn internal_assets_for_source_mapping(
    intermediate_asset: Vc<Box<dyn OutputAsset>>,
    intermediate_output_path: FileSystemPath,
) -> Result<Vc<AssetsForSourceMapping>> {
    let internal_assets =
        internal_assets(intermediate_asset, intermediate_output_path.clone()).await?;
    let intermediate_output_path = intermediate_output_path.clone();
    let mut internal_assets_for_source_mapping = FxHashMap::default();
    for asset in internal_assets.iter() {
        if let Some(generate_source_map) =
            ResolvedVc::try_sidecast::<Box<dyn GenerateSourceMap>>(*asset)
            && let Some(path) = intermediate_output_path.get_path_to(&*asset.path().await?)
        {
            internal_assets_for_source_mapping.insert(path.to_string(), generate_source_map);
        }
    }
    Ok(Vc::cell(internal_assets_for_source_mapping))
}

/// Splits the asset graph into "internal" assets and boundaries to "external"
/// assets.
#[turbo_tasks::function(operation)]
async fn separate_assets_operation(
    intermediate_asset: ResolvedVc<Box<dyn OutputAsset>>,
    intermediate_output_path: FileSystemPath,
) -> Result<Vc<SeparatedAssets>> {
    let intermediate_output_path = intermediate_output_path.clone();
    #[derive(PartialEq, Eq, Hash, Clone, Copy)]
    enum Type {
        Internal(ResolvedVc<Box<dyn OutputAsset>>),
        External(ResolvedVc<Box<dyn OutputAsset>>),
    }
    let get_asset_children = |asset| {
        let intermediate_output_path = intermediate_output_path.clone();
        async move {
            let Type::Internal(asset) = asset else {
                return Ok(Vec::new());
            };
            asset
                .references()
                .await?
                .iter()
                .map(|asset| async {
                    // Assets within the output directory are considered as "internal" and all
                    // others as "external". We follow references on "internal" assets, but do not
                    // look into references of "external" assets, since there are no "internal"
                    // assets behind "externals"
                    if asset.path().await?.is_inside_ref(&intermediate_output_path) {
                        Ok(Type::Internal(*asset))
                    } else {
                        Ok(Type::External(*asset))
                    }
                })
                .try_join()
                .await
        }
    };

    let graph = AdjacencyMap::new()
        .skip_duplicates()
        .visit(once(Type::Internal(intermediate_asset)), get_asset_children)
        .await
        .completed()?
        .into_inner();

    let mut internal_assets = FxIndexSet::default();
    let mut external_asset_entrypoints = FxIndexSet::default();

    for item in graph.into_postorder_topological() {
        match item {
            Type::Internal(asset) => {
                internal_assets.insert(asset);
            }
            Type::External(asset) => {
                external_asset_entrypoints.insert(asset);
            }
        }
    }

    Ok(SeparatedAssets {
        internal_assets: ResolvedVc::cell(internal_assets),
        external_asset_entrypoints: ResolvedVc::cell(external_asset_entrypoints),
    }
    .cell())
}

/// Emit a basic package.json that sets the type of the package to commonjs.
/// Currently code generated for Node is CommonJS, while authored code may be
/// ESM, for example.
fn emit_package_json(dir: FileSystemPath) -> Result<Vc<()>> {
    Ok(emit(
        Vc::upcast(VirtualOutputAsset::new(
            dir.join("package.json")?,
            AssetContent::file(File::from("{\"type\": \"commonjs\"}").into()),
        )),
        dir,
    ))
}
