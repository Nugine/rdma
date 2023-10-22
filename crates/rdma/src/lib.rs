#![deny(clippy::all, clippy::pedantic, clippy::cargo)]
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
    clippy::missing_trait_methods,
    clippy::semicolon_outside_block,
    clippy::semicolon_inside_block,
    clippy::multiple_unsafe_ops_per_block,
    clippy::question_mark_used,
    clippy::impl_trait_in_params,
    clippy::ref_patterns,
    clippy::struct_field_names
)]
#![allow(
    clippy::missing_errors_doc, // TODO
    clippy::missing_docs_in_private_items, // TODO
    clippy::missing_assert_message,
    clippy::multiple_crate_versions, // needs upstream fix
)]

#[macro_use]
mod utils;

pub mod bindings {
    use libc::*;

    use rust_utils::offset_of;

    mod generated;
    pub use self::generated::*;

    mod ibverbs;
    pub use self::ibverbs::*;
}

mod error;
mod weakset;

pub mod device {
    mod device_list;
    pub use self::device_list::*;

    mod device_attr;
    pub use self::device_attr::*;

    mod gid;
    pub use self::gid::*;

    mod port_attr;
    pub use self::port_attr::*;

    mod guid;
    pub use self::guid::*;
}

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
