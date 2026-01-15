use anyhow::{Result, bail};
use turbo_rcstr::{RcStr, rcstr};
use turbo_tasks::{ReadRef, ResolvedVc, TryJoinIterExt, ValueDefault, ValueToString, Vc};
use turbopack_core::chunk::{
    AsyncModuleInfo, Chunk, ChunkItem, ChunkItemBatchGroup, ChunkItemOrBatchWithAsyncModuleInfo,
    ChunkType, ChunkingContext, round_chunk_item_size,
};

use super::{EcmascriptChunk, EcmascriptChunkContent, EcmascriptChunkItem};
use crate::chunk::batch::{EcmascriptChunkItemBatchGroup, EcmascriptChunkItemOrBatchWithAsyncInfo};

#[turbo_tasks::value]
#[derive(Default)]
pub struct EcmascriptChunkType {}

#[turbo_tasks::value_impl]
impl ValueToString for EcmascriptChunkType {
    #[turbo_tasks::function]
    fn to_string(&self) -> Vc<RcStr> {
        Vc::cell(rcstr!("ecmascript"))
    }
}

#[turbo_tasks::value_impl]
impl ChunkType for EcmascriptChunkType {
    #[turbo_tasks::function]
    fn is_style(self: Vc<Self>) -> Vc<bool> {
        Vc::cell(false)
    }

    #[turbo_tasks::function]
    async fn chunk(
        &self,
        chunking_context: Vc<Box<dyn ChunkingContext>>,
        chunk_items: Vec<ChunkItemOrBatchWithAsyncModuleInfo>,
        batch_groups: Vec<ResolvedVc<ChunkItemBatchGroup>>,
    ) -> Result<Vc<Box<dyn Chunk>>> {
        // Convert and sort chunk items by their content identifier for deterministic output
        let mut chunk_items_with_ident: Vec<_> = chunk_items
            .iter()
            .map(|item| async move {
                let ecma_item =
                    EcmascriptChunkItemOrBatchWithAsyncInfo::from_chunk_item_or_batch(item).await?;
                let ident_str = match &ecma_item {
                    EcmascriptChunkItemOrBatchWithAsyncInfo::ChunkItem(item) => {
                        item.chunk_item.content_ident().to_string().await?
                    }
                    EcmascriptChunkItemOrBatchWithAsyncInfo::Batch(batch) => {
                        let batch_ref = batch.await?;
                        if let Some(first_item) = batch_ref.chunk_items.first() {
                            first_item.chunk_item.content_ident().to_string().await?
                        } else {
                            ReadRef::new_owned(RcStr::default())
                        }
                    }
                };
                Ok((ident_str, ecma_item))
            })
            .try_join()
            .await?;

        // Sort by identifier string for deterministic ordering
        chunk_items_with_ident.sort_by(|a, b| a.0.cmp(&b.0));

        let content = EcmascriptChunkContent {
            chunk_items: chunk_items_with_ident
                .into_iter()
                .map(|(_, item)| item)
                .collect(),
            batch_groups: batch_groups
                .into_iter()
                .map(|batch_group| {
                    EcmascriptChunkItemBatchGroup::from_chunk_item_batch_group(*batch_group)
                        .to_resolved()
                })
                .try_join()
                .await?,
        }
        .cell();
        Ok(Vc::upcast(EcmascriptChunk::new(chunking_context, content)))
    }

    #[turbo_tasks::function]
    async fn chunk_item_size(
        &self,
        _chunking_context: Vc<Box<dyn ChunkingContext>>,
        chunk_item: ResolvedVc<Box<dyn ChunkItem>>,
        async_module_info: Option<Vc<AsyncModuleInfo>>,
    ) -> Result<Vc<usize>> {
        let Some(chunk_item) = ResolvedVc::try_downcast::<Box<dyn EcmascriptChunkItem>>(chunk_item)
        else {
            bail!("Chunk item is not an ecmascript chunk item but reporting chunk type ecmascript");
        };
        Ok(Vc::cell(
            chunk_item
                .content_with_async_module_info(async_module_info, true)
                .await
                .map_or(0, |content| round_chunk_item_size(content.inner_code.len())),
        ))
    }
}

#[turbo_tasks::value_impl]
impl ValueDefault for EcmascriptChunkType {
    #[turbo_tasks::function]
    fn value_default() -> Vc<Self> {
        Self::default().cell()
    }
}
