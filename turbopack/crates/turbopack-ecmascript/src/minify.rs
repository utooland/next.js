use std::sync::Arc;

use anyhow::{Context, Result, bail};
use bytes_str::BytesStr;
use swc_core::{
    atoms::atom,
    base::try_with_handler,
    common::{
        BytePos, FileName, FilePathMapping, GLOBALS, LineCol, Mark, SourceMap as SwcSourceMap,
        Span,
        comments::{Comment, CommentKind, Comments, SingleThreadedComments},
    },
    ecma::{
        self,
        ast::{EsVersion, Program},
        codegen::{
            Emitter,
            text_writer::{self, JsWriter, WriteJs},
        },
        minifier::option::{CompressOptions, ExtraOptions, MangleOptions, MinifyOptions},
        parser::{Parser, StringInput, Syntax, lexer::Lexer},
        transforms::base::{
            fixer::paren_remover,
            hygiene::{self, hygiene_with_config},
        },
        visit::{Visit, VisitWith},
    },
};
use tracing::instrument;
use turbopack_core::{
    chunk::{CompressType, MangleType},
    code_builder::{Code, CodeBuilder},
};

use crate::parse::{IdentCollector, generate_js_source_map};

fn default_compress_options(mangle: Option<MangleType>) -> CompressOptions {
    CompressOptions {
        // Only run 2 passes, this is a tradeoff between performance and
        // compression size. Default is 3 passes.
        passes: 2,
        keep_classnames: mangle.is_none(),
        keep_fnames: mangle.is_none(),
        ..Default::default()
    }
}

pub fn get_compress_options(
    compress: Option<CompressType>,
    mangle: Option<MangleType>,
) -> Option<CompressOptions> {
    compress.map(|compress| match compress {
        CompressType::Default => default_compress_options(mangle),
        CompressType::Options(custom) => {
            let mut options = default_compress_options(mangle);
            if let Some(passes) = custom.passes {
                options.passes = passes as usize;
            }
            if let Some(sequences) = custom.sequences {
                options.sequences = sequences;
            }
            if let Some(keep_classnames) = custom.keep_classnames {
                options.keep_classnames = keep_classnames;
            }
            if let Some(keep_fnames) = custom.keep_fnames {
                options.keep_fnames = keep_fnames;
            }
            options
        }
    })
}

pub fn get_compress_options_for_target(
    compress: Option<CompressType>,
    mangle: Option<MangleType>,
    supports_arrow_functions: bool,
) -> Option<CompressOptions> {
    let mut options = get_compress_options(compress, mangle);
    if let Some(options) = &mut options {
        // SWC's default compression can turn functions and object methods into arrow functions,
        // even when `ecma` is set to ES5.
        options.arrows = supports_arrow_functions;
    }
    options
}

#[instrument(level = "info", name = "minify ecmascript code", skip_all)]
pub fn minify(
    code: Code,
    source_maps: bool,
    mangle: Option<MangleType>,
    compress: Option<CompressOptions>,
) -> Result<Code> {
    Ok(minify_internal(code, source_maps, mangle, compress, false)?.0)
}

#[instrument(level = "info", name = "minify ecmascript code", skip_all)]
pub fn minify_with_legal_comments(
    code: Code,
    source_maps: bool,
    mangle: Option<MangleType>,
    compress: Option<CompressOptions>,
) -> Result<(Code, Vec<String>)> {
    minify_internal(code, source_maps, mangle, compress, true)
}

fn minify_internal(
    code: Code,
    source_maps: bool,
    mangle: Option<MangleType>,
    compress: Option<CompressOptions>,
    extract_legal_comments: bool,
) -> Result<(Code, Vec<String>)> {
    // Pass None for the debug ID so we don't needlessly compute it for the pre-minified content, it
    // will be added by the Code object returned from this function
    let source_maps = source_maps.then(|| code.generate_source_map_ref(None));

    let generate_debug_id = code.should_generate_debug_id();
    let source_code = BytesStr::from_utf8(code.into_source_code().into_bytes())?;

    let cm = Arc::new(SwcSourceMap::new(FilePathMapping::empty()));
    let (src, mut src_map_buf, source_map_names, legal_comments) = {
        let fm = cm.new_source_file(FileName::Anon.into(), source_code);

        // Collect all comments and pass to the minifier so that `PURE` comments are respected.
        let comments = SingleThreadedComments::default();

        let lexer = Lexer::new(
            Syntax::default(),
            EsVersion::latest(),
            StringInput::from(&*fm),
            Some(&comments),
        );
        let mut parser = Parser::new_from(lexer);

        let (program, source_map_names, legal_comments) =
            try_with_handler(cm.clone(), Default::default(), |handler| {
                GLOBALS.set(&Default::default(), || {
                    let program = match parser.parse_program() {
                        Ok(program) => program,
                        Err(err) => {
                            err.into_diagnostic(handler).emit();
                            bail!("failed to parse source code\n{}", fm.src)
                        }
                    };

                    // Collect identifier names for source maps before minification
                    let source_map_names = if source_maps.is_some() {
                        let mut collector = IdentCollector::default();
                        program.visit_with(&mut collector);
                        collector.into_map()
                    } else {
                        Default::default()
                    };

                    let unresolved_mark = Mark::new();
                    let top_level_mark = Mark::new();

                    let program = program.apply(paren_remover(Some(&comments)));

                    let program = program.apply(swc_core::ecma::transforms::base::resolver(
                        unresolved_mark,
                        top_level_mark,
                        false,
                    ));

                    let mut program = swc_core::ecma::minifier::optimize(
                        program,
                        cm.clone(),
                        Some(&comments),
                        None,
                        &MinifyOptions {
                            compress,
                            mangle: mangle.map(|mangle| {
                                let reserved = vec![atom!("AbortSignal")];
                                match mangle {
                                    MangleType::OptimalSize => MangleOptions {
                                        reserved,
                                        ..Default::default()
                                    },
                                    MangleType::Deterministic => MangleOptions {
                                        reserved,
                                        disable_char_freq: true,
                                        ..Default::default()
                                    },
                                }
                            }),
                            ..Default::default()
                        },
                        &ExtraOptions {
                            top_level_mark,
                            unresolved_mark,
                            mangle_name_cache: None,
                        },
                    );

                    if mangle.is_none() {
                        program.mutate(hygiene_with_config(hygiene::Config {
                            top_level_mark,
                            ..Default::default()
                        }));
                    }

                    let legal_comments = if extract_legal_comments {
                        collect_legal_comments(&comments, &program)
                    } else {
                        Vec::new()
                    };

                    let program = program.apply(ecma::transforms::base::fixer::fixer(Some(
                        &comments as &dyn Comments,
                    )));

                    Ok((program, source_map_names, legal_comments))
                })
            })
            .map_err(|e| e.to_pretty_error())?;

        let (src, src_map_buf) = print_program(cm.clone(), program, source_maps.is_some())?;
        (src, src_map_buf, source_map_names, legal_comments)
    };

    let mut builder = CodeBuilder::new(source_maps.is_some(), generate_debug_id);
    if let Some(original_map) = source_maps.as_ref() {
        src_map_buf.shrink_to_fit();
        builder.push_source(
            &src.into(),
            Some(generate_js_source_map(
                &*cm,
                src_map_buf,
                Some(original_map),
                true,
                // We do not inline source contents.
                // We provide a synthesized value to `cm.new_source_file` above, so it cannot be
                // the value user expect anyway.
                false,
                source_map_names,
            )?),
        );
    } else {
        builder.push_source(&src.into(), None::<turbo_tasks_fs::rope::Rope>);
    }
    Ok((builder.build(), legal_comments))
}

struct LegalCommentsCollector<'a> {
    comments: &'a SingleThreadedComments,
    legal_comments: Vec<(BytePos, String)>,
}

impl<'a> LegalCommentsCollector<'a> {
    fn new(comments: &'a SingleThreadedComments) -> Self {
        Self {
            comments,
            legal_comments: Vec::new(),
        }
    }

    fn collect(&mut self, comments: Option<Vec<Comment>>) -> Vec<Comment> {
        let Some(comments) = comments else {
            return Vec::new();
        };
        let mut remaining = Vec::new();
        for comment in comments {
            if is_legal_comment(comment.text.as_ref()) {
                let text = match comment.kind {
                    CommentKind::Line => format!("//{}", comment.text),
                    CommentKind::Block => format!("/*{}*/", comment.text),
                };
                self.legal_comments.push((comment.span.lo, text));
            } else {
                remaining.push(comment);
            }
        }
        remaining
    }

    fn collect_leading(&mut self, pos: BytePos) {
        let remaining = self.collect(self.comments.take_leading(pos));
        if !remaining.is_empty() {
            self.comments.add_leading_comments(pos, remaining);
        }
    }

    fn collect_trailing(&mut self, pos: BytePos) {
        let remaining = self.collect(self.comments.take_trailing(pos));
        if !remaining.is_empty() {
            self.comments.add_trailing_comments(pos, remaining);
        }
    }

    fn into_comments(self) -> Vec<String> {
        let mut comments = self.legal_comments;
        comments.sort_by(|(a_position, a_comment), (b_position, b_comment)| {
            a_comment
                .cmp(b_comment)
                .then_with(|| a_position.cmp(b_position))
        });
        comments.dedup_by(|a, b| a.1 == b.1);
        comments.sort_by_key(|(position, _)| *position);
        comments.into_iter().map(|(_, comment)| comment).collect()
    }
}

impl Visit for LegalCommentsCollector<'_> {
    fn visit_span(&mut self, span: &Span) {
        if span.is_dummy() {
            return;
        }

        self.collect_leading(span.lo);
        self.collect_trailing(span.hi);

        if span.hi > span.lo {
            self.collect_leading(span.hi - BytePos(1));
            self.collect_trailing(span.lo + BytePos(1));
        }
    }
}

fn collect_legal_comments(comments: &SingleThreadedComments, program: &Program) -> Vec<String> {
    let mut collector = LegalCommentsCollector::new(comments);
    program.visit_children_with(&mut collector);
    collector.into_comments()
}

fn is_legal_comment(comment: &str) -> bool {
    let comment = comment.trim_start();
    if comment.trim_start_matches('*').starts_with('!') {
        return true;
    }
    if !comment.contains('@') {
        return false;
    }
    let lowercase = comment.to_ascii_lowercase();
    lowercase.contains("@preserve") || lowercase.contains("@lic") || lowercase.contains("@cc_on")
}

// From https://github.com/swc-project/swc/blob/11efd4e7c5e8081f8af141099d3459c3534c1e1d/crates/swc/src/lib.rs#L523-L560
fn print_program(
    cm: Arc<SwcSourceMap>,
    program: Program,
    source_maps: bool,
) -> Result<(String, Vec<(BytePos, LineCol)>)> {
    let mut src_map_buf = vec![];

    let src = {
        let mut buf = vec![];
        {
            let wr = Box::new(text_writer::omit_trailing_semi(Box::new(JsWriter::new(
                cm.clone(),
                "\n",
                &mut buf,
                source_maps.then_some(&mut src_map_buf),
            )))) as Box<dyn WriteJs>;

            let mut emitter = Emitter {
                cfg: swc_core::ecma::codegen::Config::default().with_minify(true),
                comments: None,
                cm: cm.clone(),
                wr,
            };

            emitter
                .emit_program(&program)
                .context("failed to emit module")?;
        }
        // Invalid utf8 is valid in javascript world.
        // SAFETY: SWC generates valid utf8.
        unsafe { String::from_utf8_unchecked(buf) }
    };

    Ok((src, src_map_buf))
}

#[cfg(test)]
mod tests {
    use turbopack_core::{chunk::CompressType, code_builder::CodeBuilder};

    use super::{
        default_compress_options, get_compress_options_for_target, minify,
        minify_with_legal_comments,
    };

    fn minified_source(supports_arrow_functions: bool) -> Vec<u8> {
        let mut code = CodeBuilder::default();
        code += "const queue = { delete(value) { return value; } }; globalThis.queue = queue;";

        minify(
            code.build(),
            false,
            None,
            get_compress_options_for_target(
                Some(CompressType::Default),
                None,
                supports_arrow_functions,
            ),
        )
        .unwrap()
        .into_source_code()
        .into_bytes()
        .to_vec()
    }

    #[test]
    fn respects_arrow_function_support() {
        assert!(
            minified_source(true)
                .windows(2)
                .any(|window| window == b"=>")
        );
        assert!(
            !minified_source(false)
                .windows(2)
                .any(|window| window == b"=>")
        );
    }

    #[test]
    fn extracts_and_deduplicates_legal_comments() {
        let mut builder = CodeBuilder::default();
        builder += r#"
            /*! package license */
            /**! starred bang license */
            /** @license another package */
            // @preserve line license
            /* ordinary comment */
            const value = 1;
            /*! package license */
            console.log(value);
        "#;

        let (_, comments) = minify_with_legal_comments(builder.build(), false, None, None).unwrap();

        assert_eq!(
            comments,
            vec![
                "/*! package license */",
                "/**! starred bang license */",
                "/** @license another package */",
                "// @preserve line license",
            ]
        );
    }

    #[test]
    fn excludes_legal_comments_from_discarded_code() {
        let mut builder = CodeBuilder::default();
        builder += r#"
            if (false) {
                /*! discarded package license */
                console.log("unused");
            }
            /*! retained package license */
            console.log("used");
        "#;

        let (_, comments) = minify_with_legal_comments(
            builder.build(),
            false,
            None,
            Some(default_compress_options(None)),
        )
        .unwrap();

        assert_eq!(comments, vec!["/*! retained package license */"]);
    }
}
