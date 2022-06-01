use crate::bindings as C;
use crate::error::create_resource;
use crate::pd::ProtectionDomain;
use crate::utils::{c_uint_to_u32, u32_as_c_uint};

use std::io;
use std::os::raw::c_uint;
use std::ptr::NonNull;
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryWindow(Arc<Owner>);

impl MemoryWindow {
    #[inline]
    pub fn alloc(pd: &ProtectionDomain, mw_type: MemoryWindowType) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let mw_type = mw_type.to_c_uint();
            let mw = create_resource(
                || C::ibv_alloc_mw(pd.ffi_ptr(), mw_type),
                || "failed to allocate memory window",
            )?;
            Arc::new(Owner {
                mw,
                _pd: pd.clone(),
            })
        };
        Ok(Self(owner))
    }
}

struct Owner {
    mw: NonNull<C::ibv_mw>,
    _pd: ProtectionDomain,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn ffi_ptr(&self) -> *mut C::ibv_mw {
        self.mw.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let mw = self.ffi_ptr();
            let ret = C::ibv_dealloc_mw(mw);
            assert_eq!(ret, 0);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MemoryWindowType {
    Type1 = c_uint_to_u32(C::IBV_MW_TYPE_1),
    Type2 = c_uint_to_u32(C::IBV_MW_TYPE_2),
}

impl MemoryWindowType {
    fn to_c_uint(self) -> c_uint {
        #[allow(clippy::as_conversions)]
        u32_as_c_uint(self as u32)
    }
}
