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
    clippy::default_numeric_fallback
)]
#![allow(
    clippy::missing_errors_doc, // TODO
    clippy::missing_docs_in_private_items, // TODO
    clippy::missing_panics_doc, // TODO
)]

mod error;

mod device;
pub use self::device::{Device, DeviceList, Guid};

mod context;
pub use self::context::Context;

mod pd;
pub use self::pd::ProtectionDomain;

mod utils;
