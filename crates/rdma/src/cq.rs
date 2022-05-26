use crate::cc::CompChannelOwner;
use crate::ctx::ContextOwner;
use crate::error::{create_resource, from_errno};
use crate::resource::{Resource, ResourceOwner};
use crate::utils::bool_to_c_int;
use crate::CompChannel;
use crate::Context;

use rdma_sys::{ibv_cq, ibv_cq_ex, ibv_cq_ex_to_cq, ibv_cq_init_attr_ex};
use rdma_sys::{ibv_create_cq_ex, ibv_destroy_cq, ibv_req_notify_cq};

use std::cell::UnsafeCell;
use std::io;
use std::mem;
use std::ptr::NonNull;

use asc::Asc;
use numeric_cast::NumericCast;

#[derive(Clone)]
pub struct CompletionQueue(pub(crate) Resource<CompletionQueueOwner>);

impl CompletionQueue {
    #[inline]
    #[must_use]
    pub fn options() -> CompletionQueueOptions {
        CompletionQueueOptions::default()
    }

    #[inline]
    pub fn create(ctx: &Context, options: CompletionQueueOptions) -> io::Result<Self> {
        let owner = CompletionQueueOwner::create(ctx, options)?;
        Ok(Self(Resource::new(owner)))
    }

    // FIXME: clippy false positive: https://github.com/rust-lang/rust-clippy/issues/8622
    #[allow(clippy::transmutes_expressible_as_ptr_casts)]
    #[inline]
    #[must_use]
    pub fn user_data(&self) -> usize {
        let cq = self.0.ctype();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { mem::transmute((*cq).cq_context) }
    }

    fn req_notify(&self, solicited_only: bool) -> io::Result<()> {
        // SAFETY: ffi
        let ret = unsafe {
            let cq = ibv_cq_ex_to_cq(self.0.ffi_ptr());
            let solicited_only = bool_to_c_int(solicited_only);
            ibv_req_notify_cq(cq, solicited_only)
        };
        if ret != 0 {
            return Err(from_errno(ret));
        }
        Ok(())
    }

    #[inline]
    pub fn req_notify_all(&self) -> io::Result<()> {
        self.req_notify(false)
    }

    #[inline]
    pub fn req_notify_solicited(&self) -> io::Result<()> {
        self.req_notify(true)
    }
}

#[derive(Default)]
pub struct CompletionQueueOptions {
    cqe: usize,
    user_data: usize,
    channel: Option<Asc<CompChannelOwner>>,
}

impl CompletionQueueOptions {
    #[inline]
    pub fn cqe(&mut self, cqe: usize) -> &mut Self {
        self.cqe = cqe;
        self
    }
    #[inline]
    pub fn user_data(&mut self, user_data: usize) -> &mut Self {
        self.user_data = user_data;
        self
    }
    #[inline]
    pub fn channel(&mut self, cc: &CompChannel) -> &mut Self {
        self.channel = Some(cc.0.strong_ref());
        self
    }
}

pub(crate) struct CompletionQueueOwner {
    cq: NonNull<UnsafeCell<ibv_cq>>,

    _ctx: Asc<ContextOwner>,
    _cc: Option<Asc<CompChannelOwner>>,
}

/// SAFETY: owned type
unsafe impl Send for CompletionQueueOwner {}
/// SAFETY: owned type
unsafe impl Sync for CompletionQueueOwner {}

/// SAFETY: resource owner
unsafe impl ResourceOwner for CompletionQueueOwner {
    type Ctype = ibv_cq_ex;

    fn ctype(&self) -> *mut Self::Ctype {
        self.cq.as_ptr().cast()
    }
}

impl CompletionQueueOwner {
    // TODO: comp vector
    fn create(ctx: &Context, options: CompletionQueueOptions) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let context = ctx.0.ffi_ptr();

            let mut cq_attr: ibv_cq_init_attr_ex = mem::zeroed();
            cq_attr.cqe = options.cqe.numeric_cast();
            cq_attr.cq_context = mem::transmute(options.user_data);

            if let Some(ref cc) = options.channel {
                cq_attr.channel = cc.ctype();
            }

            let cq = create_resource(
                || ibv_create_cq_ex(context, &mut cq_attr),
                || "failed to create completion queue",
            )?;

            Ok(Self {
                cq: cq.cast(),
                _ctx: ctx.0.strong_ref(),
                _cc: options.channel,
            })
        }
    }
}

impl Drop for CompletionQueueOwner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_destroy_cq(ibv_cq_ex_to_cq(self.ctype())) };
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
