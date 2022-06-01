use crate::bindings as C;
use crate::ctx::Context;
use crate::error::create_resource;

use std::ptr::NonNull;
use std::sync::Arc;
use std::{io, mem};

#[derive(Clone)]
pub struct DeviceMemory(Arc<Owner>);

impl DeviceMemory {
    #[inline]
    #[must_use]
    pub fn options() -> DeviceMemoryOptions {
        DeviceMemoryOptions::default()
    }

    #[inline]
    pub fn alloc(ctx: &Context, mut options: DeviceMemoryOptions) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let attr = &mut options.attr;
            let dm = create_resource(
                || C::ibv_alloc_dm(ctx.ffi_ptr(), attr),
                || "failed to allocate device memory",
            )?;
            Arc::new(Owner {
                dm,
                _ctx: ctx.clone(),
            })
        };
        Ok(Self(owner))
    }
}

struct Owner {
    dm: NonNull<C::ibv_dm>,
    _ctx: Context,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn ffi_ptr(&self) -> *mut C::ibv_dm {
        self.dm.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let dm = self.ffi_ptr();
            let ret = C::ibv_free_dm(dm);
            assert_eq!(ret, 0);
        }
    }
}

pub struct DeviceMemoryOptions {
    attr: C::ibv_alloc_dm_attr,
}

impl Default for DeviceMemoryOptions {
    #[inline]
    fn default() -> Self {
        Self {
            // SAFETY: POD ffi type
            attr: unsafe { mem::zeroed() },
        }
    }
}
