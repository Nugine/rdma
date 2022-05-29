use crate::bindings as C;
use crate::utils::{c_uint_to_u32, u32_as_c_uint};

use std::mem;
use std::os::raw::{c_int, c_uint};

use numeric_cast::NumericCast;

#[repr(transparent)]
pub struct SendRequest(C::ibv_send_wr);

/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`C::ibv_post_send`)
unsafe impl Send for SendRequest {}
/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`C::ibv_post_send`)
unsafe impl Sync for SendRequest {}

#[repr(transparent)]
pub struct RecvRequest(C::ibv_recv_wr);

/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`C::ibv_post_recv`)
unsafe impl Send for RecvRequest {}
/// SAFETY: ffi pointer data
/// the actual usage is unsafe (`C::ibv_post_recv`)
unsafe impl Sync for RecvRequest {}

#[repr(C)]
pub struct Sge {
    pub addr: u64,
    pub length: u32,
    pub lkey: u32,
}

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
        self.0.sg_list = sg_list.as_mut_ptr().cast::<C::ibv_sge>();
        self
    }

    #[inline]
    pub fn opcode(&mut self, opcode: Opcode) -> &mut Self {
        self.0.opcode = opcode.to_c_uint();
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
        self.0.sg_list = sg_list.as_mut_ptr().cast::<C::ibv_sge>();
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Opcode {
    Send = c_uint_to_u32(C::IBV_WR_SEND),
    SendWithImm = c_uint_to_u32(C::IBV_WR_SEND_WITH_IMM),
    Write = c_uint_to_u32(C::IBV_WR_RDMA_WRITE),
    Read = c_uint_to_u32(C::IBV_WR_RDMA_READ),
    AtomicFetchAdd = c_uint_to_u32(C::IBV_WR_ATOMIC_FETCH_AND_ADD),
    AtomicCAS = c_uint_to_u32(C::IBV_WR_ATOMIC_CMP_AND_SWP),
}

impl Opcode {
    fn to_c_uint(self) -> c_uint {
        #[allow(clippy::as_conversions)]
        u32_as_c_uint(self as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sge_layout() {
        assert_eq!(mem::size_of::<Sge>(), mem::size_of::<C::ibv_sge>());
        assert_eq!(mem::align_of::<Sge>(), mem::align_of::<C::ibv_sge>());
        assert_eq!(offset_of!(Sge, addr), offset_of!(C::ibv_sge, addr));
        assert_eq!(offset_of!(Sge, length), offset_of!(C::ibv_sge, length));
        assert_eq!(offset_of!(Sge, lkey), offset_of!(C::ibv_sge, lkey));
    }
}
