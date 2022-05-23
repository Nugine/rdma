use crate::error::custom_error;
use crate::Device;
use crate::ProtectionDomain;

use std::io;
use std::ptr::NonNull;

use rdma_sys::*;

use asc::Asc;

pub struct Context {
    inner: Asc<Inner>,
    ctx: NonNull<ibv_context>,
}

/// SAFETY: shared owned type
unsafe impl Send for Context {}
/// SAFETY: shared owned type
unsafe impl Sync for Context {}

pub(crate) struct ContextRef(Asc<Inner>);

impl Context {
    #[inline]
    pub fn open(device: &Device) -> io::Result<Self> {
        let inner = Asc::new(Inner::open(device)?);
        let ctx = inner.ctx;
        Ok(Self { inner, ctx })
    }

    pub(crate) fn ffi_ptr(&self) -> *mut ibv_context {
        self.ctx.as_ptr()
    }

    pub(crate) fn strong_ref(&self) -> ContextRef {
        let inner = Asc::clone(&self.inner);
        ContextRef(inner)
    }

    #[inline]
    pub fn alloc_pd(&self) -> io::Result<ProtectionDomain> {
        ProtectionDomain::alloc(self)
    }
}

struct Inner {
    ctx: NonNull<ibv_context>,
}

/// SAFETY: owned type
unsafe impl Send for Inner {}
/// SAFETY: owned type
unsafe impl Sync for Inner {}

impl Inner {
    fn open(device: &Device) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let ctx = ibv_open_device(device.ffi_ptr());
            if ctx.is_null() {
                return Err(custom_error("failed to open device"));
            }
            let ctx = NonNull::new_unchecked(ctx);
            Ok(Self { ctx })
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_close_device(self.ctx.as_ptr()) };
        assert_eq!(ret, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::require_send_sync;

    #[test]
    fn marker() {
        require_send_sync::<Context>();
        require_send_sync::<ContextRef>();
        require_send_sync::<Inner>();
    }
}
