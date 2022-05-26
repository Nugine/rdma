use crate::cc::{self, CompChannel};
use crate::ctx::{self, Context};
use crate::error::{create_resource, from_errno};
use crate::resource::Resource;
use crate::utils::{bool_to_c_int, usize_to_void_ptr};

use rdma_sys::{ibv_cq, ibv_cq_ex, ibv_cq_ex_to_cq, ibv_cq_init_attr_ex};
use rdma_sys::{ibv_create_cq_ex, ibv_destroy_cq, ibv_req_notify_cq};

use std::cell::UnsafeCell;
use std::io;
use std::mem;
use std::ptr::NonNull;
use std::sync::Arc;

use numeric_cast::NumericCast;

pub struct CompletionQueue(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for CompletionQueue {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl CompletionQueue {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_cq_ex {
        self.0.ffi_ptr()
    }

    #[inline]
    #[must_use]
    pub fn options() -> CompletionQueueOptions {
        CompletionQueueOptions::default()
    }

    #[inline]
    pub fn create(ctx: &Context, options: CompletionQueueOptions) -> io::Result<Self> {
        let owner = Arc::new(Owner::create(ctx, options)?);
        Ok(Self(owner))
    }

    #[inline]
    #[must_use]
    pub fn user_data(&self) -> usize {
        self.0.user_data
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

pub(crate) struct Owner {
    cq: NonNull<UnsafeCell<ibv_cq>>,
    user_data: usize,

    _ctx: Arc<ctx::Owner>,
    _cc: Option<Arc<cc::Owner>>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_cq_ex {
        self.cq.as_ptr().cast()
    }

    // TODO: comp vector
    fn create(ctx: &Context, options: CompletionQueueOptions) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let context = ctx.ffi_ptr();

            let mut cq_attr: ibv_cq_init_attr_ex = mem::zeroed();
            cq_attr.cqe = options.cqe.numeric_cast();
            cq_attr.cq_context = usize_to_void_ptr(options.user_data);

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

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let cq = self.ffi_ptr();
            let ret = ibv_destroy_cq(ibv_cq_ex_to_cq(cq));
            assert_eq!(ret, 0);
        };
    }
}

#[derive(Default)]
pub struct CompletionQueueOptions {
    cqe: usize,
    user_data: usize,
    channel: Option<Arc<cc::Owner>>,
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
