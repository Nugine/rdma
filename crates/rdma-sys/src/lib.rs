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

        pub fn rs_ibv_query_port(
            context: *mut ibv_context,
            port_num: u8,
            port_attr: *mut ibv_port_attr,
        ) -> c_int;

        pub fn rs_ibv_query_gid_ex(
            context: *mut ibv_context,
            port_num: u32,
            gid_index: u32,
            entry: *mut ibv_gid_entry,
            flags: u32,
        ) -> c_int;

        pub fn rs_ibv_create_cq_ex(
            context: *mut ibv_context,
            cq_attr: *mut ibv_cq_init_attr_ex,
        ) -> *mut ibv_cq;
    }
}

pub use self::rsrdma::rs_ibv_create_cq_ex as ibv_create_cq_ex;
pub use self::rsrdma::rs_ibv_query_device_ex as ibv_query_device_ex;
pub use self::rsrdma::rs_ibv_query_gid_ex as ibv_query_gid_ex;
pub use self::rsrdma::rs_ibv_query_port as ibv_query_port;
