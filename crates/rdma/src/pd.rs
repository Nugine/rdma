use crate::ctx::Context;
use crate::error::create_resource;
use crate::resource::Resource;

use rdma_sys::ibv_pd;
use rdma_sys::{ibv_alloc_pd, ibv_dealloc_pd};

use std::io;
use std::ptr::NonNull;
use std::sync::Arc;

pub struct ProtectionDomain(Arc<Inner>);

/// SAFETY: shared resource type
unsafe impl Resource for ProtectionDomain {
    type Ctype = ibv_pd;

    fn ffi_ptr(&self) -> *mut Self::Ctype {
        self.0.pd.as_ptr()
    }

    fn strong_ref(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl ProtectionDomain {
    #[inline]
    pub fn alloc(ctx: &Context) -> io::Result<Self> {
        let inner = Inner::alloc(ctx)?;
        Ok(Self(Arc::new(inner)))
    }
}

struct Inner {
    pd: NonNull<ibv_pd>,

    _ctx: Context,
}

/// SAFETY: owned type
unsafe impl Send for Inner {}
/// SAFETY: owned type
unsafe impl Sync for Inner {}

impl Inner {
    fn alloc(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let pd = create_resource(
                || ibv_alloc_pd(ctx.ffi_ptr()),
                || "failed to allocate protection domain",
            )?;
            Ok(Self {
                pd,
                _ctx: ctx.strong_ref(),
            })
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
