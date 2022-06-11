use crate::driver::RdmaDriver;
use crate::{IntoLocalReadAccess, IntoLocalWriteAccess};
use crate::{LocalAccess, LocalReadAccess, LocalWriteAccess};

use rdma::mr::{AccessFlags, MemoryRegion};

use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use std::mem::ManuallyDrop;
use std::slice;
use std::sync::Arc;

use parking_lot::lock_api::{ArcRwLockReadGuard, ArcRwLockWriteGuard};
use parking_lot::{RawRwLock, RwLock};
use scopeguard::guard;

pub struct Buf {
    pub(crate) mr: ManuallyDrop<MemoryRegion<BufMetadata>>,
}

pub(crate) struct BufMetadata {
    align: usize,
}

impl Buf {
    pub fn new_zeroed(len: usize, align: usize) -> Self {
        assert!(len > 0 && len < usize::MAX.wrapping_div(2));
        let layout = Layout::from_size_align(len, align).expect("invalid layout");
        let driver = RdmaDriver::global();
        unsafe {
            let ptr = alloc_zeroed(layout);
            if ptr.is_null() {
                handle_alloc_error(layout)
            }
            let guard: _ = guard((), |()| dealloc(ptr, layout));

            let mr = {
                let addr = ptr;
                let length = len;
                let access_flags = AccessFlags::LOCAL_WRITE;
                let metadata = BufMetadata { align };
                MemoryRegion::register(&driver.pd, addr, length, access_flags, metadata)
                    .expect("failed to register memory region")
            };

            scopeguard::ScopeGuard::into_inner(guard);

            Self {
                mr: ManuallyDrop::new(mr),
            }
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        let base = self.mr.addr_ptr();
        let len = self.mr.length();
        unsafe { slice::from_raw_parts(base, len) }
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        let base = self.mr.addr_ptr();
        let len = self.mr.length();
        unsafe { slice::from_raw_parts_mut(base, len) }
    }

    pub fn head(self, len: usize) -> Head<Self> {
        Head::new(self, len)
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        let ptr = self.mr.addr_ptr();
        let len = self.mr.length() as usize;
        let align = self.mr.metadata().align;
        unsafe {
            ManuallyDrop::drop(&mut self.mr);
            let layout = Layout::from_size_align_unchecked(len, align);
            dealloc(ptr, layout);
        }
    }
}

unsafe impl LocalAccess for Buf {
    fn addr_u64(&self) -> u64 {
        self.mr.addr_u64()
    }

    fn length(&self) -> usize {
        self.mr.length()
    }

    fn lkey(&self) -> u32 {
        self.mr.lkey()
    }
}

unsafe impl LocalReadAccess for Buf {}
unsafe impl LocalWriteAccess for Buf {}

pub struct Head<T> {
    inner: T,
    len: usize,
}

impl<T: LocalAccess> Head<T> {
    pub fn new(inner: T, len: usize) -> Self {
        assert!(len <= inner.length());
        Self { inner, len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

unsafe impl<T: LocalAccess> LocalAccess for Head<T> {
    fn addr_u64(&self) -> u64 {
        self.inner.addr_u64()
    }

    fn length(&self) -> usize {
        self.len
    }

    fn lkey(&self) -> u32 {
        self.inner.lkey()
    }
}

unsafe impl<T: LocalReadAccess> LocalReadAccess for Head<T> {}
unsafe impl<T: LocalWriteAccess> LocalWriteAccess for Head<T> {}

#[derive(Clone)]
pub struct RwBuf(Arc<RwLock<Buf>>);

pub struct ReadRwBuf(Arc<RwLock<Buf>>, ArcRwLockReadGuard<RawRwLock, Buf>);
pub struct WriteRwBuf(Arc<RwLock<Buf>>, ArcRwLockWriteGuard<RawRwLock, Buf>);

impl RwBuf {
    pub fn new_zeroed(len: usize, align: usize) -> Self {
        let buf = Buf::new_zeroed(len, align);
        Self(Arc::new(RwLock::new(buf)))
    }

    pub fn read(&self) -> ReadRwBuf {
        ReadRwBuf(self.0.clone(), self.0.read_arc())
    }

    pub fn write(&self) -> WriteRwBuf {
        WriteRwBuf(self.0.clone(), self.0.write_arc())
    }
}

impl IntoLocalReadAccess for &RwBuf {
    type Output = ReadRwBuf;

    fn into_local_read_access(self) -> Self::Output {
        self.read()
    }
}

impl IntoLocalWriteAccess for &RwBuf {
    type Output = WriteRwBuf;

    fn into_local_write_access(self) -> Self::Output {
        self.write()
    }
}

unsafe impl LocalAccess for ReadRwBuf {
    fn addr_u64(&self) -> u64 {
        self.1.addr_u64()
    }

    fn length(&self) -> usize {
        self.1.length()
    }

    fn lkey(&self) -> u32 {
        self.1.lkey()
    }
}

unsafe impl LocalReadAccess for ReadRwBuf {}

unsafe impl LocalAccess for WriteRwBuf {
    fn addr_u64(&self) -> u64 {
        self.1.addr_u64()
    }

    fn length(&self) -> usize {
        self.1.length()
    }

    fn lkey(&self) -> u32 {
        self.1.lkey()
    }
}

unsafe impl LocalReadAccess for WriteRwBuf {}
unsafe impl LocalWriteAccess for WriteRwBuf {}
