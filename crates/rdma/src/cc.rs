use crate::ctx::Context;
use crate::error::create_resource;
use crate::resource::Resource;

use rdma_sys::ibv_comp_channel;
use rdma_sys::{ibv_create_comp_channel, ibv_destroy_comp_channel};

use std::io;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::ptr::NonNull;
use std::sync::Arc;

pub struct CompChannel(Arc<Owner>);

/// SAFETY: shared resource type
unsafe impl Resource for CompChannel {
    type Ctype = ibv_comp_channel;

    fn ffi_ptr(&self) -> *mut Self::Ctype {
        self.0.cc.as_ptr()
    }

    fn strong_ref(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl CompChannel {
    #[inline]
    pub fn create(ctx: &Context) -> io::Result<Self> {
        let owner = Owner::create(ctx)?;
        Ok(Self(Arc::new(owner)))
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

struct Owner {
    cc: NonNull<ibv_comp_channel>,

    _ctx: Context,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn create(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let cc = create_resource(
                || ibv_create_comp_channel(ctx.ffi_ptr()),
                || "failed to create completion channel",
            )?;

            Ok(Self {
                cc,
                _ctx: ctx.strong_ref(),
            })
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_destroy_comp_channel(self.cc.as_ptr()) };
        assert_eq!(ret, 0);
    }
}
