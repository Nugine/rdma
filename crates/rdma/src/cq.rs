use crate::error::{create_resource, from_errno};
use crate::resource::Resource;
use crate::utils::bool_to_c_int;
use crate::CompChannel;
use crate::Context;

use rdma_sys::{ibv_cq, ibv_cq_ex, ibv_cq_ex_to_cq, ibv_cq_init_attr_ex};
use rdma_sys::{ibv_create_cq_ex, ibv_destroy_cq, ibv_req_notify_cq};

use std::cell::UnsafeCell;
use std::io;
use std::mem::{self, ManuallyDrop};
use std::os::raw::c_void;
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::Arc;

use numeric_cast::NumericCast;

pub struct CompletionQueue(Pin<Arc<Inner>>);

/// SAFETY: shared resource type
unsafe impl Resource for CompletionQueue {
    type Ctype = ibv_cq_ex;

    fn ffi_ptr(&self) -> *mut Self::Ctype {
        self.0.cq.as_ptr().cast()
    }

    fn strong_ref(&self) -> Self {
        Self(Pin::clone(&self.0))
    }
}

impl CompletionQueue {
    #[inline]
    #[must_use]
    pub fn options() -> CompletionQueueOptions {
        CompletionQueueOptions::default()
    }

    #[inline]
    pub fn create(ctx: &Context, options: CompletionQueueOptions) -> io::Result<Self> {
        let inner = Arc::pin(Inner::create(ctx, options)?);
        // SAFETY: setup self-reference in cq_context
        unsafe {
            let cq: *mut ibv_cq_ex = inner.cq.as_ptr().cast();
            let inner_ptr: *const Inner = &*inner;
            (*cq).cq_context = mem::transmute(inner_ptr);
        };
        Ok(Self(inner))
    }

    #[inline]
    #[must_use]
    pub fn user_data(&self) -> usize {
        self.0.user_data
    }

    /// SAFETY:
    /// 1. `cq_context` must come from `CompletionQueue::ffi_ptr`
    /// 2. the completion queue must be alive when calling this method
    pub(crate) unsafe fn from_cq_context(cq_context: *mut c_void) -> Self {
        let inner_ptr: *const Inner = cq_context.cast();
        let inner = ManuallyDrop::new(Arc::from_raw(inner_ptr));
        Self(Pin::new(Arc::clone(&inner)))
    }

    fn req_notify(&self, solicited_only: bool) -> io::Result<()> {
        let cq = self.ffi_ptr();
        // SAFETY: ffi
        let ret = unsafe {
            let solicited_only = bool_to_c_int(solicited_only);
            ibv_req_notify_cq(ibv_cq_ex_to_cq(cq), solicited_only)
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

struct Inner {
    cq: NonNull<UnsafeCell<ibv_cq>>,
    user_data: usize,

    _ctx: Context,
    _cc: Option<CompChannel>,
}

/// SAFETY: owned type
unsafe impl Send for Inner {}
/// SAFETY: owned type
unsafe impl Sync for Inner {}

impl Inner {
    // TODO: comp vector
    fn create(ctx: &Context, options: CompletionQueueOptions) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let context = ctx.ffi_ptr();

            let mut cq_attr: ibv_cq_init_attr_ex = mem::zeroed();
            cq_attr.cqe = options.cqe.numeric_cast();
            cq_attr.cq_context = mem::transmute(options.user_data);

            if let Some(ref cc) = options.channel {
                cq_attr.channel = cc.ffi_ptr();
            }

            let cq = create_resource(
                || ibv_create_cq_ex(context, &mut cq_attr),
                || "failed to create completion queue",
            )?;

            Ok(Self {
                cq: cq.cast(),
                user_data: options.user_data,
                _ctx: ctx.strong_ref(),
                _cc: options.channel,
            })
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let cq: *mut ibv_cq_ex = self.cq.as_ptr().cast();
            let ret = ibv_destroy_cq(ibv_cq_ex_to_cq(cq));
            assert_eq!(ret, 0);
        };
    }
}

#[derive(Default)]
pub struct CompletionQueueOptions {
    cqe: usize,
    user_data: usize,
    channel: Option<CompChannel>,
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
        self.channel = Some(cc.strong_ref());
        self
    }
}
