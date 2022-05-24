use crate::comp_channel::CompChannelOwner;
use crate::context::ContextOwner;
use crate::error::custom_error;
use crate::resource::Resource;
use crate::resource::ResourceOwner;
use crate::CompChannel;
use crate::Context;

use std::io;
use std::mem;
use std::os::raw::c_int;
use std::os::raw::c_void;
use std::ptr;
use std::ptr::NonNull;

use rdma_sys::*;

use asc::Asc;
use numeric_cast::NumericCast;

pub struct CompletionQueue(pub(crate) Resource<CompletionQueueOwner>);

impl CompletionQueue {
    #[inline]
    pub fn create(ctx: &Context, cqe: usize, user_data: usize) -> io::Result<Self> {
        let owner = CompletionQueueOwner::create(ctx, cqe, user_data, None)?;
        Ok(Self(Resource::new(owner)))
    }

    #[inline]
    pub fn create_with_cc(
        ctx: &Context,
        cqe: usize,
        user_data: usize,
        cc: &CompChannel,
    ) -> io::Result<Self> {
        let owner = CompletionQueueOwner::create(ctx, cqe, user_data, Some(cc))?;
        Ok(Self(Resource::new(owner)))
    }
}

pub(crate) struct CompletionQueueOwner {
    _ctx: Asc<ContextOwner>,
    _cc: Option<Asc<CompChannelOwner>>,
    cq: NonNull<ibv_cq>,
}

/// SAFETY: owned type
unsafe impl Send for CompletionQueueOwner {}
/// SAFETY: owned type
unsafe impl Sync for CompletionQueueOwner {}

/// SAFETY: resource owner
unsafe impl ResourceOwner for CompletionQueueOwner {
    type Ctype = ibv_cq;

    fn ctype(&self) -> *mut Self::Ctype {
        self.cq.as_ptr()
    }
}

impl CompletionQueueOwner {
    // TODO: comp vector
    fn create(
        ctx: &Context,
        cqe: usize,
        user_data: usize,
        cc: Option<&CompChannel>,
    ) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let context = ctx.0.ffi_ptr();
            let cqe: c_int = cqe.numeric_cast();
            let user_data: *mut c_void = mem::transmute(user_data);
            let channel: *mut ibv_comp_channel = cc.map_or(ptr::null_mut(), |cc| cc.0.ffi_ptr());
            let comp_vector = 0;
            let cq = ibv_create_cq(context, cqe, user_data, channel, comp_vector);
            if cq.is_null() {
                return Err(custom_error("failed to create completion queue"));
            }
            let cq = NonNull::new_unchecked(cq);
            Ok(Self {
                _ctx: ctx.0.strong_ref(),
                _cc: None,
                cq,
            })
        }
    }
}

impl Drop for CompletionQueueOwner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_destroy_cq(self.cq.as_ptr()) };
        assert_eq!(ret, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::require_send_sync;

    #[test]
    fn marker() {
        require_send_sync::<CompletionQueue>();
        require_send_sync::<CompletionQueueOwner>();
    }
}
