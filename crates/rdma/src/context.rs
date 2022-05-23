use crate::error::custom_error;
use crate::Device;

use std::io;
use std::ptr::NonNull;

use rdma_sys::*;

use asc::Asc;

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

pub struct Context(Asc<Inner>);

impl Context {
    #[inline]
    pub fn open(device: &Device) -> io::Result<Self> {
        let inner = Inner::open(device)?;
        Ok(Self(Asc::new(inner)))
    }
}
