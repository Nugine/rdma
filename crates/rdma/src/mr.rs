use crate::bindings as C;
use crate::error::create_resource;
use crate::pd::{self, ProtectionDomain};
use crate::resource::Resource;
use crate::utils::{c_uint_to_u32, u32_as_c_uint};

use std::io;
use std::os::raw::{c_uint, c_void};
use std::ptr::NonNull;
use std::sync::Arc;

use bitflags::bitflags;

pub struct MemoryRegion(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for MemoryRegion {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl MemoryRegion {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_mr {
        self.0.ffi_ptr()
    }

    /// Registers a memory region associated with the protection domain `pd`.
    /// The memory region's starting address is `addr` and its size is `length`.
    ///
    /// # Safety
    /// 1. the memory region must be valid until it is deregistered
    /// 2. the memory region must be initialized before it is read for the first time
    #[inline]
    pub unsafe fn register(
        pd: &ProtectionDomain,
        addr: *mut u8,
        length: usize,
        access_flags: AccessFlags,
    ) -> io::Result<Self> {
        let owner = {
            let addr: *mut c_void = addr.cast();
            let access_flags = access_flags.to_c_uint();
            let mr = create_resource(
                || C::ibv_reg_mr(pd.ffi_ptr(), addr, length, access_flags),
                || "failed to register memory region",
            )?;
            Arc::new(Owner {
                mr,
                _pd: pd.strong_ref(),
            })
        };
        Ok(Self(owner))
    }

    #[inline]
    #[must_use]
    pub fn lkey(&self) -> u32 {
        let mr = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { (*mr).lkey }
    }

    #[inline]
    #[must_use]
    pub fn rkey(&self) -> u32 {
        let mr = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { (*mr).rkey }
    }

    #[inline]
    #[must_use]
    pub fn addr(&self) -> *mut u8 {
        let mr = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { (*mr).addr.cast() }
    }

    #[inline]
    #[must_use]
    pub fn length(&self) -> usize {
        let mr = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { (*mr).length }
    }
}

pub(crate) struct Owner {
    mr: NonNull<C::ibv_mr>,

    _pd: Arc<pd::Owner>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_mr {
        self.mr.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let mr = self.ffi_ptr();
            let ret = C::ibv_dereg_mr(mr);
            assert_eq!(ret, 0);
        }
    }
}

bitflags! {
    pub struct AccessFlags: u32 {
        const LOCAL_WRITE       = c_uint_to_u32(C::IBV_ACCESS_LOCAL_WRITE);
        const REMOTE_WRITE      = c_uint_to_u32(C::IBV_ACCESS_REMOTE_WRITE);
        const REMOTE_READ       = c_uint_to_u32(C::IBV_ACCESS_REMOTE_READ);
        const REMOTE_ATOMIC     = c_uint_to_u32(C::IBV_ACCESS_REMOTE_ATOMIC);
        const MW_BIND           = c_uint_to_u32(C::IBV_ACCESS_MW_BIND);
        const ZERO_BASED        = c_uint_to_u32(C::IBV_ACCESS_ZERO_BASED);
        const ON_DEMAND         = c_uint_to_u32(C::IBV_ACCESS_ON_DEMAND);
        const HUGETLB           = c_uint_to_u32(C::IBV_ACCESS_HUGETLB);
        const RELAXED_ORDERING  = c_uint_to_u32(C::IBV_ACCESS_RELAXED_ORDERING);
    }
}

impl AccessFlags {
    pub(crate) fn to_c_uint(self) -> c_uint {
        u32_as_c_uint(self.bits())
    }
}
