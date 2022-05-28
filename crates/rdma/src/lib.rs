#![deny(clippy::all, clippy::pedantic, clippy::restriction)]
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
    clippy::unseparated_literal_suffix
)]
#![allow(
    clippy::missing_errors_doc, // TODO
    clippy::missing_docs_in_private_items, // TODO
)]

pub mod bindings {
    use libc::*;

    mod generated;
    pub use self::generated::*;

    mod ibverbs;
    pub use self::ibverbs::*;
}

mod error;
#[macro_use]
mod resource;
mod utils;
mod weakset;

pub mod device;

pub mod cc;
pub mod cq;
pub mod ctx;
pub mod dm;
pub mod mr;
pub mod mw;
pub mod pd;
pub mod qp;
pub mod wc;
pub mod wr;

pub mod query {
    mod device_attr;
    pub use self::device_attr::*;

    mod port_attr;
    pub use self::port_attr::*;

    mod gid;
    pub use self::gid::*;
}
