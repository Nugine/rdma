#![deny(clippy::all)]
#![allow(non_camel_case_types, non_snake_case, clippy::missing_safety_doc)]

use libc::*;
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

mod rsrdma {
    use super::*;
    extern "C" {
        pub fn rs_ibv_query_device_ex(
            context: *mut ibv_context,
            input: *const ibv_query_device_ex_input,
            attr: *mut ibv_device_attr_ex,
        ) -> c_int;
    }
}

pub use self::rsrdma::rs_ibv_query_device_ex as ibv_query_device_ex;
