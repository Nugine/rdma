#![deny(clippy::all, clippy::missing_inline_in_public_items)]
#![allow(non_camel_case_types, non_snake_case, clippy::missing_safety_doc)]

use libc::*;
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

mod ibverbs;
pub use self::ibverbs::*;
