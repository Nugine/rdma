use crate::error::custom_error;
use crate::Device;

use std::io;
use std::ptr::NonNull;

use rdma_sys::*;

pub struct Context {
    ctx: NonNull<ibv_context>,
}

/// SAFETY: owned type
unsafe impl Send for Context {}
/// SAFETY: owned type
unsafe impl Sync for Context {}

impl Context {
    #[inline]
    pub fn open(device: &Device) -> io::Result<Self> {
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

impl Drop for Context {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: ffi
        let ret = unsafe { ibv_close_device(self.ctx.as_ptr()) };
        assert_eq!(ret, 0);
    }
}
