use crate::context::ContextRef;
use crate::error::custom_error;
use crate::Context;

use std::io;
use std::ptr::NonNull;

use rdma_sys::*;

use asc::Asc;

pub struct ProtectionDomain {
    inner: Asc<Inner>,
    pd: NonNull<ibv_pd>,
}

/// SAFETY: shared owned type
unsafe impl Send for ProtectionDomain {}
/// SAFETY: shared owned type
unsafe impl Sync for ProtectionDomain {}

pub(crate) struct ProtectionDomainRef(Asc<Inner>);

impl ProtectionDomain {
    #[inline]
    pub fn alloc(ctx: &Context) -> io::Result<Self> {
        let inner = Asc::new(Inner::alloc(ctx)?);
        let pd = inner.pd;
        Ok(Self { inner, pd })
    }

    pub(crate) fn ffi_ptr(&self) -> *mut ibv_pd {
        self.pd.as_ptr()
    }

    pub(crate) fn strong_ref(&self) -> ProtectionDomainRef {
        let inner = Asc::clone(&self.inner);
        ProtectionDomainRef(inner)
    }
}

struct Inner {
    ctx_ref: ContextRef,
    pd: NonNull<ibv_pd>,
}

/// SAFETY: owned type
unsafe impl Send for Inner {}
/// SAFETY: owned type
unsafe impl Sync for Inner {}

impl Inner {
    fn alloc(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let pd = ibv_alloc_pd(ctx.ffi_ptr());
            if pd.is_null() {
                return Err(custom_error("failed to allocate protection domain"));
            }
            let ctx_ref = ctx.strong_ref();
            let pd = NonNull::new_unchecked(pd);
            Ok(Self { ctx_ref, pd })
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_dealloc_pd(self.pd.as_ptr()) };
        assert_eq!(ret, 0);
    }
}
