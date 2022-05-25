use crate::ctx::ContextOwner;
use crate::error::create_resource;
use crate::resource::Resource;
use crate::resource::ResourceOwner;
use crate::Context;

use rdma_sys::ibv_pd;
use rdma_sys::{ibv_alloc_pd, ibv_dealloc_pd};

use std::io;
use std::ptr::NonNull;

use asc::Asc;

#[derive(Clone)]
pub struct ProtectionDomain(pub(crate) Resource<ProtectionDomainOwner>);

impl ProtectionDomain {
    #[inline]
    pub fn alloc(ctx: &Context) -> io::Result<Self> {
        let owner = ProtectionDomainOwner::alloc(ctx)?;
        Ok(Self(Resource::new(owner)))
    }
}

pub(crate) struct ProtectionDomainOwner {
    pd: NonNull<ibv_pd>,

    _ctx: Asc<ContextOwner>,
}

/// SAFETY: owned type
unsafe impl Send for ProtectionDomainOwner {}
/// SAFETY: owned type
unsafe impl Sync for ProtectionDomainOwner {}

/// SAFETY: resource owner
unsafe impl ResourceOwner for ProtectionDomainOwner {
    type Ctype = ibv_pd;

    fn ctype(&self) -> *mut Self::Ctype {
        self.pd.as_ptr()
    }
}

impl ProtectionDomainOwner {
    fn alloc(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let pd = create_resource(
                || ibv_alloc_pd(ctx.0.ffi_ptr()),
                || "failed to allocate protection domain",
            )?;
            Ok(Self {
                pd,
                _ctx: ctx.0.strong_ref(),
            })
        }
    }
}

impl Drop for ProtectionDomainOwner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_dealloc_pd(self.pd.as_ptr()) };
        assert_eq!(ret, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::require_send_sync;

    #[test]
    fn marker() {
        require_send_sync::<ProtectionDomain>();
        require_send_sync::<ProtectionDomainOwner>();
    }
}
