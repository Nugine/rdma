use crate::bindings::{ibv_recv_wr, ibv_send_wr, ibv_sge};

use std::mem;
use std::os::raw::c_int;

use numeric_cast::NumericCast;

#[repr(transparent)]
pub struct SendRequest(ibv_send_wr);

/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`ibv_post_send`)
unsafe impl Send for SendRequest {}
/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`ibv_post_send`)
unsafe impl Sync for SendRequest {}

#[repr(transparent)]
pub struct RecvRequest(ibv_recv_wr);

/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`ibv_post_recv`)
unsafe impl Send for RecvRequest {}
/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`ibv_post_recv`)
unsafe impl Sync for RecvRequest {}

#[repr(C)]
pub struct Sge {
    pub addr: u64,
    pub length: u32,
    pub lkey: u32,
}

// layout test
const _: () = {
    assert!(mem::size_of::<Sge>() == mem::size_of::<ibv_sge>());
    assert!(mem::align_of::<Sge>() == mem::align_of::<ibv_sge>());
    let sge = Sge {
        addr: 0,
        length: 0,
        lkey: 0,
    };
    let _ = ibv_sge {
        addr: sge.addr,
        length: sge.length,
        lkey: sge.lkey,
    };
};

/// SAFETY: ffi pointer data
/// the actual usage is unsafe
unsafe impl Send for Sge {}
/// SAFETY: ffi pointer data
/// the actual usage is unsafe
unsafe impl Sync for Sge {}

impl SendRequest {
    #[inline]
    #[must_use]
    pub fn zeroed() -> Self {
        // SAFETY: POD ffi type
        unsafe { Self(mem::zeroed()) }
    }

    #[inline]
    pub fn id(&mut self, id: u64) -> &mut Self {
        self.0.wr_id = id;
        self
    }

    #[inline]
    pub fn next(&mut self, next: *mut Self) -> &mut Self {
        // SAFETY: repr(transparent)
        self.0.next = next.cast();
        self
    }

    #[inline]
    pub fn sg_list(&mut self, sg_list: &mut [Sge]) -> &mut Self {
        self.0.num_sge = sg_list.len().numeric_cast::<c_int>();
        self.0.sg_list = sg_list.as_mut_ptr().cast::<ibv_sge>();
        self
    }
}

impl RecvRequest {
    #[inline]
    #[must_use]
    pub fn zeroed() -> Self {
        // SAFETY: POD ffi type
        unsafe { Self(mem::zeroed()) }
    }

    #[inline]
    pub fn id(&mut self, id: u64) -> &mut Self {
        self.0.wr_id = id;
        self
    }

    #[inline]
    pub fn next(&mut self, next: *mut Self) -> &mut Self {
        // SAFETY: repr(transparent)
        self.0.next = next.cast();
        self
    }

    #[inline]
    pub fn sg_list(&mut self, sg_list: &mut [Sge]) -> &mut Self {
        self.0.num_sge = sg_list.len().numeric_cast::<c_int>();
        self.0.sg_list = sg_list.as_mut_ptr().cast::<ibv_sge>();
        self
    }
}
