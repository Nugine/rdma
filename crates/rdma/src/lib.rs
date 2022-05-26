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
    clippy::exhaustive_enums
)]
#![allow(
    clippy::missing_errors_doc, // TODO
    clippy::missing_docs_in_private_items, // TODO
    clippy::missing_panics_doc, // TODO
)]

mod error;
mod resource;
mod utils;
mod weakset;

pub mod cc;
pub mod cq;
pub mod ctx;
pub mod device;
pub mod pd;
pub mod qp;

pub mod query {
    mod device_attr;
    pub use self::device_attr::*;

    mod port_attr;
    pub use self::port_attr::*;

    mod gid;
    pub use self::gid::*;
}
