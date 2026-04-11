use swc_core::{
    common::util::take::Take,
    ecma::{
        ast::{self, AwaitExpr, Expr, YieldExpr},
        visit::{VisitMut, VisitMutWith},
    },
};

/// AST visitor that converts all `AwaitExpr` nodes into `YieldExpr` nodes.
///
/// Used for environments that don't support native async/await. The containing
/// module wrapper is changed from `async function` to `function*` (generator),
/// so `await` must become `yield`. Operating at the AST level avoids false
/// positives from string replacement (e.g. `"await "` inside string literals).
pub(crate) struct AwaitToYield;

impl VisitMut for AwaitToYield {
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        // Recurse first so nested expressions are handled
        expr.visit_mut_children_with(self);

        if let Expr::Await(AwaitExpr { span, arg }) = expr {
            *expr = Expr::Yield(YieldExpr {
                span: *span,
                delegate: false,
                arg: Some(arg.take()),
            });
        }
    }

    // Defense-in-depth: don't descend into nested async functions.
    // At this pipeline stage SWC has already converted their `await` to
    // `yield`, but guard against edge cases where that doesn't hold.
    fn visit_mut_function(&mut self, f: &mut ast::Function) {
        if !f.is_async {
            f.visit_mut_children_with(self);
        }
    }

    fn visit_mut_arrow_expr(&mut self, f: &mut ast::ArrowExpr) {
        if !f.is_async {
            f.visit_mut_children_with(self);
        }
    }
}
