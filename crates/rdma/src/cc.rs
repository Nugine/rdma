use crate::ctx::{self, Context};
use crate::error::create_resource;
use crate::resource::Resource;

use rdma_sys::ibv_comp_channel;
use rdma_sys::{ibv_create_comp_channel, ibv_destroy_comp_channel};

use std::io;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::ptr::NonNull;
use std::sync::Arc;

pub struct CompChannel(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for CompChannel {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl CompChannel {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_comp_channel {
        self.0.ffi_ptr()
    }

    #[inline]
    pub fn create(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let cc = create_resource(
                || ibv_create_comp_channel(ctx.ffi_ptr()),
                || "failed to create completion channel",
            )?;

            Arc::new(Owner {
                cc,
                _ctx: ctx.strong_ref(),
            })
        };
        Ok(Self(owner))
    }
}

impl AsRawFd for CompChannel {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        let cc = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { (*cc).fd }
    }
}

pub(crate) struct Owner {
    cc: NonNull<ibv_comp_channel>,

    _ctx: Arc<ctx::Owner>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_comp_channel {
        self.cc.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let cc = self.ffi_ptr();
            let ret = ibv_destroy_comp_channel(cc);
            assert_eq!(ret, 0);
        }
    }
}
