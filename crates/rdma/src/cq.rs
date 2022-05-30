use crate::bindings as C;
use crate::cc::{self, CompChannel};
use crate::ctx::{self, Context};
use crate::error::{create_resource, from_errno};
use crate::resource::Resource;
use crate::utils::{bool_to_c_int, ptr_as_mut};
use crate::wc::WorkCompletion;

use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::os::raw::{c_int, c_uint, c_void};
use std::ptr::NonNull;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Weak};
use std::{io, slice};

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
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_cq_ex {
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

            let mut cq_attr: C::ibv_cq_init_attr_ex = mem::zeroed();
            cq_attr.cqe = options.cqe.numeric_cast();

            if let Some(ref cc) = options.channel {
                cq_attr.channel = cc.ffi_ptr();
            }

            let cq = create_resource(
                || C::ibv_create_cq_ex(context, &mut cq_attr),
                || "failed to create completion queue",
            )?;

            Arc::new(Owner {
                cq,
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
            C::ibv_req_notify_cq(C::ibv_cq_ex_to_cq(cq), solicited_only)
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

    #[inline]
    pub fn poll<'wc>(
        &self,
        buf: &'wc mut [MaybeUninit<WorkCompletion>],
    ) -> io::Result<&'wc mut [WorkCompletion]> {
        // SAFETY: ffi
        unsafe {
            let num_entries: c_int = buf.len().numeric_cast();
            let wc = buf.as_mut_ptr().cast::<C::ibv_wc>();
            let cq = C::ibv_cq_ex_to_cq(self.ffi_ptr());
            let ret = C::ibv_poll_cq(cq, num_entries, wc);
            if ret < 0 {
                return Err(from_errno(ret.wrapping_neg()));
            }
            let len: usize = ret.numeric_cast();
            let data = wc.cast::<WorkCompletion>();
            Ok(slice::from_raw_parts_mut(data, len))
        }
    }
}

pub(crate) struct Owner {
    cq: NonNull<C::ibv_cq_ex>,
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
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_cq_ex {
        self.cq.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(ref cc) = self.cc {
            assert!(cc.del_cq_ref(self));
        }

        // SAFETY: ffi
        unsafe {
            let cq = C::ibv_cq_ex_to_cq(self.ffi_ptr());

            let comp_ack: c_uint = self.comp_events_completed.load(Relaxed).numeric_cast();
            // if the number overflows, the behavior is unspecified
            C::ibv_ack_cq_events(cq, comp_ack);

            let ret = C::ibv_destroy_cq(cq);
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
