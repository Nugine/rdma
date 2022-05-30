use crate::bindings as C;
use crate::ctx;
use crate::ctx::Context;
use crate::error::create_resource;
use crate::pd;
use crate::pd::ProtectionDomain;
use crate::resource::Resource;
use crate::utils::usize_to_void_ptr;

use std::io;
use std::mem;
use std::ptr::NonNull;
use std::sync::Arc;

pub struct SharedReceiveQueue(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for SharedReceiveQueue {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl SharedReceiveQueue {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_srq {
        self.0.ffi_ptr()
    }

    #[inline]
    #[must_use]
    pub fn options() -> SharedReceiveQueueOptions {
        SharedReceiveQueueOptions::default()
    }

    /// # Panics
    /// + if `ctx` is not the same as the context of the specified protection domain in `options`.
    #[inline]
    pub fn create(ctx: &Context, mut options: SharedReceiveQueueOptions) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let context = ctx.ffi_ptr();
            let attr = &mut options.attr;

            if let Some(ref pd) = options.pd {
                let pd_context = (*pd.ffi_ptr()).context;
                assert_eq!(pd_context, context, "context mismatch");
            }

            let srq = create_resource(
                || C::ibv_create_srq_ex(context, attr),
                || "failed to create shared receive queue",
            )?;

            Arc::new(Owner {
                srq,
                _ctx: ctx.strong_ref(),
                _pd: options.pd,
            })
        };
        Ok(Self(owner))
    }
}

pub(crate) struct Owner {
    srq: NonNull<C::ibv_srq>,

    _ctx: Arc<ctx::Owner>,
    _pd: Option<Arc<pd::Owner>>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn ffi_ptr(&self) -> *mut C::ibv_srq {
        self.srq.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let srq = self.ffi_ptr();
            let ret = C::ibv_destroy_srq(srq);
            assert_eq!(ret, 0);
        }
    }
}

pub struct SharedReceiveQueueOptions {
    attr: C::ibv_srq_init_attr_ex,
    pd: Option<Arc<pd::Owner>>,
}

impl Default for SharedReceiveQueueOptions {
    #[inline]
    fn default() -> Self {
        Self {
            // SAFETY: POD ffi type
            attr: unsafe { mem::zeroed() },
            pd: None,
        }
    }
}

impl SharedReceiveQueueOptions {
    #[inline]
    pub fn protection_domain(&mut self, pd: &ProtectionDomain) -> &mut Self {
        self.attr.pd = pd.ffi_ptr();
        self.attr.comp_mask |= C::IBV_SRQ_INIT_ATTR_PD;
        self.pd = Some(pd.strong_ref());
        self
    }

    #[inline]
    pub fn user_data(&mut self, user_data: usize) -> &mut Self {
        self.attr.srq_context = usize_to_void_ptr(user_data);
        self
    }
}
