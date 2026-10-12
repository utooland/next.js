use std::sync::Arc;

use anyhow::Result;
use swc_core::common::{BytePos, FileName, LineCol, SourceMap};
use tokio::io::AsyncReadExt;
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{ResolvedVc, ValueToStringRef, Vc};
use turbo_tasks_fs::{
    File, FileContent, FileSystemPath,
    rope::{Rope, RopeBuilder},
};
use turbo_tasks_hash::HashAlgorithm;
use turbopack_core::{
    asset::{Asset, AssetContent},
    chunk::ChunkingContext,
    output::{OutputAsset, OutputAssets, OutputAssetsReference, OutputAssetsWithReferenced},
    source::Source,
    source_map::{GenerateSourceMap, SourceMapAsset},
};

use crate::parse::generate_js_source_map;

/// An EcmaScript OutputAsset composed of one file, no parsing and no references. Includes a source
/// map to the original file.
#[turbo_tasks::value]
pub struct SingleFileEcmascriptOutput {
    chunking_context: ResolvedVc<Box<dyn ChunkingContext>>,
    source: ResolvedVc<Box<dyn Source>>,
}

#[turbo_tasks::value_impl]
impl SingleFileEcmascriptOutput {
    #[turbo_tasks::function]
    fn source_map(self: Vc<Self>) -> Vc<SourceMapAsset> {
        SourceMapAsset::new(Vc::upcast(self), Vc::upcast(self))
    }
}

#[turbo_tasks::value_impl]
impl OutputAsset for SingleFileEcmascriptOutput {
    #[turbo_tasks::function]
    async fn path(self: Vc<Self>) -> Result<Vc<FileSystemPath>> {
        let this = self.await?;
        Ok(this.chunking_context.chunk_path(
            Some(Vc::upcast(self)),
            this.source.ident(),
            None,
            rcstr!(".js"),
        ))
    }
}

#[turbo_tasks::value_impl]
impl Asset for SingleFileEcmascriptOutput {
    #[turbo_tasks::function]
    async fn content_hash(
        self: Vc<Self>,
        salt: Vc<RcStr>,
        algorithm: HashAlgorithm,
    ) -> Result<Vc<Option<RcStr>>> {
        let this = self.await?;
        let include_source_map = *this
            .chunking_context
            .reference_chunk_source_maps(Vc::upcast(self))
            .await?;
        if !include_source_map {
            return Ok(this.source.content_hash(salt, algorithm));
        }

        let source_content = this.source.content().file_content().await?;
        let FileContent::Content(file) = &*source_content else {
            return Ok(this.source.content_hash(salt, algorithm));
        };

        // The map contains the original source path, which can change without changing the JS.
        // Hash its content directly so resolving the output filename never reads the map path.
        let mut content = RopeBuilder::default();
        content.concat(file.content());
        content.push_static_bytes(b"\0");
        if let FileContent::Content(map) = &*self.generate_source_map().await? {
            content.concat(map.content());
        }
        Ok(FileContent::Content(File::from(content.build()))
            .cell()
            .content_hash(salt, algorithm))
    }

    #[turbo_tasks::function]
    fn content(&self) -> Vc<AssetContent> {
        self.source.content()
    }
}

#[turbo_tasks::value_impl]
impl SingleFileEcmascriptOutput {
    #[turbo_tasks::function]
    pub fn new(
        chunking_context: ResolvedVc<Box<dyn ChunkingContext>>,
        source: ResolvedVc<Box<dyn Source>>,
    ) -> Vc<SingleFileEcmascriptOutput> {
        SingleFileEcmascriptOutput {
            source,
            chunking_context,
        }
        .cell()
    }
}

#[turbo_tasks::value_impl]
impl GenerateSourceMap for SingleFileEcmascriptOutput {
    #[turbo_tasks::function]
    pub async fn generate_source_map(&self) -> Result<Vc<FileContent>> {
        let FileContent::Content(file) = &*self.source.content().file_content().await? else {
            return Ok(FileContent::NotFound.cell());
        };

        let file_source = {
            let mut s = String::new();
            file.read().read_to_string(&mut s).await?;
            s
        };

        let mut mappings = vec![];
        // Start from 1 because 0 is reserved for dummy spans in SWC.
        let mut pos: u32 = 1;
        for (index, line) in file_source.split_inclusive('\n').enumerate() {
            mappings.push((
                BytePos(pos),
                LineCol {
                    line: index as u32,
                    col: 0,
                },
            ));
            pos += line.len() as u32;
        }

        let source_path = self
            .source
            .ident()
            .await?
            .path
            .to_string_ref()
            .await?
            .to_string();

        let sm: Arc<SourceMap> = Default::default();
        sm.new_source_file(FileName::Custom(source_path).into(), file_source);

        let map = generate_js_source_map(
            &*sm,
            mappings,
            None::<&Rope>,
            true,
            true,
            Default::default(),
        )?;
        Ok(FileContent::Content(File::from(map.to_rope())).cell())
    }
}

#[turbo_tasks::value_impl]
impl OutputAssetsReference for SingleFileEcmascriptOutput {
    #[turbo_tasks::function]
    async fn references(self: Vc<Self>) -> Result<Vc<OutputAssetsWithReferenced>> {
        let this = self.await?;

        let include_source_map = *this
            .chunking_context
            .reference_chunk_source_maps(Vc::upcast(self))
            .await?;

        let references = if include_source_map {
            Vc::cell(vec![ResolvedVc::upcast(
                self.source_map().to_resolved().await?,
            )])
        } else {
            OutputAssets::empty()
        };

        Ok(OutputAssetsWithReferenced::from_assets(references))
    }
}
