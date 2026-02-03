use anyhow::{Result, bail};
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, TryJoinIterExt, ValueDefault, ValueToString, Vc};
use turbopack_core::chunk::{
    AsyncModuleInfo, Chunk, ChunkItem, ChunkItemBatchGroup, ChunkItemExt,
    ChunkItemOrBatchWithAsyncModuleInfo, ChunkType, ChunkingContext, ModuleId,
    round_chunk_item_size,
};

use super::{EcmascriptChunk, EcmascriptChunkContent, EcmascriptChunkItem};
use crate::chunk::batch::{EcmascriptChunkItemBatchGroup, EcmascriptChunkItemOrBatchWithAsyncInfo};

#[turbo_tasks::value]
#[derive(Default, ValueToString)]
#[value_to_string("ecmascript")]
pub struct EcmascriptChunkType {}

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
        // Convert chunk items first
        let converted_chunk_items: Vec<_> = chunk_items
            .iter()
            .map(EcmascriptChunkItemOrBatchWithAsyncInfo::from_chunk_item_or_batch)
            .try_join()
            .await?;

        // Sort chunk items by their module ID for deterministic content ordering
        // This ensures chunks with the same modules produce identical content
        let mut items_with_id: Vec<_> = converted_chunk_items
            .into_iter()
            .map(|item| async move {
                let id: ModuleId = match &item {
                    EcmascriptChunkItemOrBatchWithAsyncInfo::ChunkItem(item) => {
                        item.chunk_item.id().await?
                    }
                    EcmascriptChunkItemOrBatchWithAsyncInfo::Batch(batch) => {
                        let batch_ref = batch.await?;
                        if let Some(first_item) = batch_ref.chunk_items.first() {
                            first_item.chunk_item.id().await?
                        } else {
                            ModuleId::String(RcStr::default())
                        }
                    }
                };
                Ok((id, item))
            })
            .try_join()
            .await?;

        items_with_id.sort_by(|a, b| a.0.cmp(&b.0));

        let sorted_items: Vec<_> = items_with_id.into_iter().map(|(_, item)| item).collect();

        let content = EcmascriptChunkContent {
            chunk_items: sorted_items,
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
        let chunk_item = chunk_item.into_trait_ref().await?;
        let size = match chunk_item
            .content_with_async_module_info(async_module_info, true)
            .await
        {
            Ok(content) => {
                let content = content.await?;
                round_chunk_item_size(content.inner_code.len())
            }
            Err(_) => 0,
        };
        Ok(Vc::cell(size))
    }
}

#[turbo_tasks::value_impl]
impl ValueDefault for EcmascriptChunkType {
    #[turbo_tasks::function]
    fn value_default() -> Vc<Self> {
        Self::default().cell()
    }
}
