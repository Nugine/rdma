use crate::context::ContextOwner;
use crate::error::custom_error;
use crate::resource::Resource;
use crate::resource::ResourceOwner;
use crate::Context;

use std::io;
use std::ptr::NonNull;

use rdma_sys::*;

use asc::Asc;

pub struct CompChannel(pub(crate) Resource<CompChannelOwner>);

impl CompChannel {
    #[inline]
    pub fn create(ctx: &Context) -> io::Result<Self> {
        let owner = CompChannelOwner::create(ctx)?;
        Ok(Self(Resource::new(owner)))
    }
}

pub(crate) struct CompChannelOwner {
    _ctx: Asc<ContextOwner>,
    cc: NonNull<ibv_comp_channel>,
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
            let cc = ibv_create_comp_channel(ctx.0.ffi_ptr());
            if cc.is_null() {
                return Err(custom_error("failed to create completion channel"));
            }
            let cc = NonNull::new_unchecked(cc);
            Ok(Self {
                _ctx: ctx.0.strong_ref(),
                cc,
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
