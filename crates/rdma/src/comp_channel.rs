use crate::context::ContextRef;
use crate::error::custom_error;
use crate::Context;

use std::io;
use std::ptr::NonNull;

use rdma_sys::*;

use asc::Asc;

pub struct CompChannel {
    inner: Asc<Inner>,
    cc: NonNull<ibv_comp_channel>,
}

/// SAFETY: shared owned type
unsafe impl Send for CompChannel {}
/// SAFETY: shared owned type
unsafe impl Sync for CompChannel {}

pub struct CompChannelRef(Asc<Inner>);

impl CompChannel {
    pub fn create(ctx: &Context) -> io::Result<Self> {
        let inner = Asc::new(Inner::create(ctx)?);
        let cc = inner.cc;
        Ok(Self { inner, cc })
    }
}

struct Inner {
    _ctx_ref: ContextRef,
    cc: NonNull<ibv_comp_channel>,
}

/// SAFETY: owned type
unsafe impl Send for Inner {}
/// SAFETY: owned type
unsafe impl Sync for Inner {}

impl Inner {
    fn create(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let cc = ibv_create_comp_channel(ctx.ffi_ptr());
            if cc.is_null() {
                return Err(custom_error("failed to create completion channel"));
            }
            let cc = NonNull::new_unchecked(cc);
            Ok(Self {
                _ctx_ref: ctx.strong_ref(),
                cc,
            })
        }
    }
}

impl Drop for Inner {
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
        require_send_sync::<CompChannelRef>();
        require_send_sync::<Inner>();
    }
}
