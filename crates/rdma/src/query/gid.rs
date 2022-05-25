use crate::error::custom_error;
use crate::Context;

use rdma_sys::{ibv_gid, ibv_query_gid};

use std::io;
use std::mem::MaybeUninit;
use std::os::raw::c_int;

use numeric_cast::NumericCast;

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Gid(ibv_gid);
// TODO: ibv_query_gid_ex

impl Gid {
    #[inline]
    pub fn query(ctx: &Context, port_num: u32, index: usize) -> io::Result<Self> {
        let port_num: u8 = port_num.numeric_cast();
        let index: c_int = index.numeric_cast();

        // SAFETY: ffi
        unsafe {
            let mut gid = MaybeUninit::<Self>::uninit();
            let ret = ibv_query_gid(ctx.0.ffi_ptr(), port_num, index, gid.as_mut_ptr().cast());
            if ret != 0 {
                return Err(custom_error("failed to query gid"));
            }
            Ok(gid.assume_init())
        }
    }

    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        // SAFETY: type raw bytes
        unsafe { &self.0.raw }
    }
}

impl PartialEq for Gid {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for Gid {}
