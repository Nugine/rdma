use crate::error::{create_resource, from_errno};
use crate::resource::Resource;
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

pub struct CompletionQueue(Asc<Inner>);

/// SAFETY: shared resource type
unsafe impl Resource for CompletionQueue {
    type Ctype = ibv_cq_ex;

    fn ffi_ptr(&self) -> *mut Self::Ctype {
        self.0.cq.as_ptr().cast()
    }

    fn strong_ref(&self) -> Self {
        Self(Asc::clone(&self.0))
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
        let inner = Inner::create(ctx, options)?;
        Ok(Self(Asc::new(inner)))
    }

    // FIXME: clippy false positive: https://github.com/rust-lang/rust-clippy/issues/8622
    #[allow(clippy::transmutes_expressible_as_ptr_casts)]
    #[inline]
    #[must_use]
    pub fn user_data(&self) -> usize {
        let cq = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { mem::transmute((*cq).cq_context) }
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
