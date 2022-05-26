use crate::cc::{self, CompChannel};
use crate::ctx::{self, Context};
use crate::error::{create_resource, from_errno};
use crate::resource::Resource;
use crate::utils::{bool_to_c_int, ptr_as_mut};

use rdma_sys::{ibv_ack_cq_events, ibv_cq, ibv_cq_ex, ibv_cq_ex_to_cq, ibv_cq_init_attr_ex};
use rdma_sys::{ibv_create_cq_ex, ibv_destroy_cq, ibv_req_notify_cq};

use std::cell::UnsafeCell;
use std::io;
use std::mem::{self, ManuallyDrop};
use std::os::raw::{c_uint, c_void};
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Weak};

use numeric_cast::NumericCast;

#[derive(Clone)]
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
        // SAFETY: ffi
        let owner = unsafe {
            let context = ctx.ffi_ptr();

            let mut cq_attr: ibv_cq_init_attr_ex = mem::zeroed();
            cq_attr.cqe = options.cqe.numeric_cast();

            if let Some(ref cc) = options.channel {
                cq_attr.channel = cc.ffi_ptr();
            }

            let cq = create_resource(
                || ibv_create_cq_ex(context, &mut cq_attr),
                || "failed to create completion queue",
            )?;

            Arc::new(Owner {
                cq: cq.cast(),
                user_data: options.user_data,
                comp_events_completed: AtomicU32::new(0),
                _ctx: ctx.strong_ref(),
                cc: options.channel,
            })
        };

        if let Some(ref cc) = owner.cc {
            cc.add_cq_ref(Arc::downgrade(&owner));
        }

        // SAFETY: setup self-reference in cq_context
        unsafe {
            let owner_ptr: *const Owner = &*owner;
            let cq = owner.ffi_ptr();
            (*cq).cq_context = ptr_as_mut(owner_ptr).cast();
        }

        Ok(Self(owner))
    }

    /// # Panics
    /// + if the completion queue has been destroyed
    ///
    /// # SAFETY
    /// 1. `cq_context` must come from the pointee of `CompletionQueue::ffi_ptr`
    /// 2. there must be at least one weak reference to the completion queue owner
    pub(crate) unsafe fn from_cq_context(cq_context: *mut c_void) -> Self {
        let owner_ptr: *const Owner = cq_context.cast();
        let weak = ManuallyDrop::new(Weak::from_raw(owner_ptr));
        let owner = Weak::upgrade(&weak).expect("the completion queue has been destroyed");
        Self(owner)
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

    #[inline]
    pub fn ack_cq_events(&self, cnt: u32) {
        self.0.comp_events_completed.fetch_add(cnt, Relaxed);
    }
}

pub(crate) struct Owner {
    cq: NonNull<UnsafeCell<ibv_cq>>,
    user_data: usize,
    comp_events_completed: AtomicU32,

    cc: Option<Arc<cc::Owner>>,
    _ctx: Arc<ctx::Owner>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_cq_ex {
        self.cq.as_ptr().cast()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(ref cc) = self.cc {
            cc.del_cq_ref(self);
        }

        // SAFETY: ffi
        unsafe {
            let cq = ibv_cq_ex_to_cq(self.ffi_ptr());

            let comp_ack: c_uint = self.comp_events_completed.load(Relaxed).numeric_cast();
            // if the number overflows, the behavior is unspecified
            ibv_ack_cq_events(cq, comp_ack);

            let ret = ibv_destroy_cq(cq);
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
