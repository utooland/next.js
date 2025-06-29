use anyhow::Result;
use serde::{Deserialize, Serialize};
use turbo_tasks::{debug::ValueDebugFormat, trace::TraceRawVcs, NonLocalValue, Value, Vc};
use turbopack_core::{
    chunk::ChunkingContext, compile_time_info::CompileTimeDefineValue, module_graph::ModuleGraph,
};

use super::AstPath;
use crate::{
    code_gen::{CodeGen, CodeGeneration},
    create_visitor,
};

#[derive(PartialEq, Eq, Serialize, Deserialize, TraceRawVcs, ValueDebugFormat, NonLocalValue)]
pub struct ConstantValueCodeGen {
    value: CompileTimeDefineValue,
    path: AstPath,
}

impl ConstantValueCodeGen {
    pub fn new(value: Value<CompileTimeDefineValue>, path: AstPath) -> Self {
        ConstantValueCodeGen {
            value: value.into_value(),
            path,
        }
    }
    pub async fn code_generation(
        &self,
        _module_graph: Vc<ModuleGraph>,
        _chunking_context: Vc<Box<dyn ChunkingContext>>,
    ) -> Result<CodeGeneration> {
        let value = self.value.clone();

        let visitor = create_visitor!(self.path, visit_mut_expr(expr: &mut Expr) {
            // TODO: avoid this clone
            *expr = value.clone().into();
        });

        Ok(CodeGeneration::visitors(vec![visitor]))
    }
}

impl From<ConstantValueCodeGen> for CodeGen {
    fn from(val: ConstantValueCodeGen) -> Self {
        CodeGen::ConstantValueCodeGen(val)
    }
}
