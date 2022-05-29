#![allow(
    non_camel_case_types,
    non_snake_case,
    clippy::missing_safety_doc,
    clippy::unreadable_literal,
    clippy::decimal_literal_representation
)]

use super::*;

#[cfg(not(docsrs))]
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[cfg(all(
    docsrs,
    target_arch = "x86_64",
    target_os = "linux",
    target_env = "gnu"
))]
include!("./x86_64_unknown_linux_gnu.rs");
