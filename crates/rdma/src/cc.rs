use crate::ctx::ContextOwner;
use crate::error::create_resource;
use crate::resource::{Resource, ResourceOwner};
use crate::Context;

use rdma_sys::ibv_comp_channel;
use rdma_sys::{ibv_create_comp_channel, ibv_destroy_comp_channel};

use std::io;
use std::ptr::NonNull;

use asc::Asc;

#[derive(Clone)]
pub struct CompChannel(pub(crate) Resource<CompChannelOwner>);

impl CompChannel {
    #[inline]
    pub fn create(ctx: &Context) -> io::Result<Self> {
        let owner = CompChannelOwner::create(ctx)?;
        Ok(Self(Resource::new(owner)))
    }
}

pub(crate) struct CompChannelOwner {
    cc: NonNull<ibv_comp_channel>,

    _ctx: Asc<ContextOwner>,
}

/// SAFETY: owned type
unsafe impl Send for CompChannelOwner {}
/// SAFETY: owned type
unsafe impl Sync for CompChannelOwner {}

/// SAFETY: resource owner
unsafe impl ResourceOwner for CompChannelOwner {
    type Ctype = ibv_comp_channel;

    fn ctype(&self) -> *mut Self::Ctype {
        self.cc.as_ptr()
    }
}

impl CompChannelOwner {
    fn create(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let cc = create_resource(
                || ibv_create_comp_channel(ctx.0.ffi_ptr()),
                || "failed to create completion channel",
            )?;

            Ok(Self {
                cc,
                _ctx: ctx.0.strong_ref(),
            })
        }
    }
}

impl Drop for CompChannelOwner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_destroy_comp_channel(self.cc.as_ptr()) };
        assert_eq!(ret, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::require_send_sync;

    #[test]
    fn marker() {
        require_send_sync::<CompChannel>();
        require_send_sync::<CompChannelOwner>();
    }
}
