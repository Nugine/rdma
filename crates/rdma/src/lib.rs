#![deny(clippy::all, clippy::pedantic, clippy::restriction, clippy::cargo)]
#![allow(
    clippy::module_name_repetitions,
    clippy::blanket_clippy_restriction_lints,
    clippy::pub_use,
    clippy::implicit_return,
    clippy::panic_in_result_fn,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::wildcard_imports,
    clippy::unwrap_in_result,
    clippy::transmute_ptr_to_ptr,
    clippy::shadow_reuse,
    clippy::default_numeric_fallback,
    clippy::shadow_unrelated,
    clippy::panic,
    clippy::enum_glob_use,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::unseparated_literal_suffix,
    clippy::mod_module_files,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::missing_trait_methods
)]
#![allow(
    clippy::missing_errors_doc, // TODO
    clippy::missing_docs_in_private_items, // TODO
)]

#[macro_use]
mod utils;

pub mod bindings {
    use libc::*;

    mod generated;
    pub use self::generated::*;

    mod ibverbs;
    pub use self::ibverbs::*;
}

mod error;
mod weakset;

pub mod device;

pub mod ah;
pub mod cc;
pub mod cq;
pub mod ctx;
pub mod dm;
pub mod mr;
pub mod mw;
pub mod pd;
pub mod qp;
pub mod srq;
pub mod wc;
pub mod wr;
