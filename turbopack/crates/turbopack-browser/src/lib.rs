#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]

pub mod chunking_context;
pub mod ecmascript;
pub mod react_refresh;

pub use chunking_context::{
    BrowserChunkingContext, BrowserChunkingContextBuilder, CurrentChunkMethod,
};
