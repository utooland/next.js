#![feature(min_specialization)]
#![feature(arbitrary_self_types)]
#![feature(arbitrary_self_types_pointers)]

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub mod issue;
pub mod runtime_entry;
pub mod source_context;
