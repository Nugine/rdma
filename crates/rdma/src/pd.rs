use crate::bindings as C;
use crate::ctx::{self, Context};
use crate::error::create_resource;
use crate::qp::{QueuePair, QueuePairOptions};
use crate::resource::Resource;

use std::io;
use std::ptr::NonNull;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProtectionDomain(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for ProtectionDomain {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl ProtectionDomain {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_pd {
        self.0.ffi_ptr()
    }

    #[inline]
    pub fn alloc(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let pd = create_resource(
                || C::ibv_alloc_pd(ctx.ffi_ptr()),
                || "failed to allocate protection domain",
            )?;
            Arc::new(Owner {
                pd,
                _ctx: ctx.strong_ref(),
            })
        };
        Ok(Self(owner))
    }

    #[inline]
    pub fn create_qp(&self, options: QueuePairOptions) -> io::Result<QueuePair> {
        QueuePair::create(self, options)
    }
}

pub(crate) struct Owner {
    pd: NonNull<C::ibv_pd>,

    _ctx: Arc<ctx::Owner>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_pd {
        self.pd.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let pd = self.ffi_ptr();
            let ret = C::ibv_dealloc_pd(pd);
            assert_eq!(ret, 0);
        }
    }
}
