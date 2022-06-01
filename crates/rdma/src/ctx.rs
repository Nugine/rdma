use crate::bindings as C;
use crate::device::Device;
use crate::error::create_resource;

use std::io;
use std::ptr::NonNull;
use std::sync::Arc;

#[derive(Clone)]
pub struct Context(Arc<Owner>);

impl Context {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_context {
        self.0.ffi_ptr()
    }

    #[inline]
    pub fn open(device: &Device) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let ctx = create_resource(
                || C::ibv_open_device(device.ffi_ptr()),
                || "failed to open device",
            )?;
            Arc::new(Owner { ctx })
        };
        Ok(Self(owner))
    }
}

struct Owner {
    ctx: NonNull<C::ibv_context>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn ffi_ptr(&self) -> *mut C::ibv_context {
        self.ctx.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let context = self.ffi_ptr();
            let ret = C::ibv_close_device(context);
            assert_eq!(ret, 0);
        }
    }
}
