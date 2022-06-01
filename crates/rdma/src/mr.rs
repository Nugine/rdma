use crate::bindings as C;
use crate::error::create_resource;
use crate::pd::ProtectionDomain;
use crate::utils::{c_uint_to_u32, ptr_to_addr, u32_as_c_uint};

use std::io;
use std::os::raw::{c_uint, c_void};
use std::ptr::NonNull;
use std::sync::Arc;

use bitflags::bitflags;
use numeric_cast::NumericCast;

#[derive(Clone)]
pub struct MemoryRegion<T = ()>(Arc<Owner<T>>);

impl<T> MemoryRegion<T> {
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
        metadata: T,
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
                metadata,
                _pd: pd.clone(),
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
    pub fn addr_ptr(&self) -> *mut u8 {
        let mr = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { (*mr).addr.cast() }
    }

    #[inline]
    #[must_use]
    pub fn addr_u64(&self) -> u64 {
        let mr = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { ptr_to_addr((*mr).addr) }.numeric_cast()
    }

    #[inline]
    #[must_use]
    pub fn length(&self) -> usize {
        let mr = self.ffi_ptr();
        // SAFETY: reading a immutable field of a concurrent ffi type
        unsafe { (*mr).length }
    }

    #[inline]
    #[must_use]
    pub fn metadata(&self) -> &T {
        self.0.metadata()
    }
}

struct Owner<T> {
    mr: NonNull<C::ibv_mr>,

    metadata: T,

    _pd: ProtectionDomain,
}

/// SAFETY: owned type
unsafe impl<T: Send> Send for Owner<T> {}
/// SAFETY: owned type
unsafe impl<T: Sync> Sync for Owner<T> {}

impl<T> Owner<T> {
    pub(crate) fn ffi_ptr(&self) -> *mut C::ibv_mr {
        self.mr.as_ptr()
    }

    fn metadata(&self) -> &T {
        &self.metadata
    }
}

impl<T> Drop for Owner<T> {
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
