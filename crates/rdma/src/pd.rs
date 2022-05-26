use crate::ctx::{self, Context};
use crate::error::create_resource;
use crate::resource::Resource;

use rdma_sys::ibv_pd;
use rdma_sys::{ibv_alloc_pd, ibv_dealloc_pd};

use std::io;
use std::ptr::NonNull;
use std::sync::Arc;

pub struct ProtectionDomain(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for ProtectionDomain {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl ProtectionDomain {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_pd {
        self.0.ffi_ptr()
    }

    #[inline]
    pub fn alloc(ctx: &Context) -> io::Result<Self> {
        let owner = Owner::alloc(ctx)?;
        Ok(Self(Arc::new(owner)))
    }
}

pub(crate) struct Owner {
    pd: NonNull<ibv_pd>,

    _ctx: Arc<ctx::Owner>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_pd {
        self.pd.as_ptr()
    }

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

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let pd = self.ffi_ptr();
            let ret = ibv_dealloc_pd(pd);
            assert_eq!(ret, 0);
        }
    }
}
