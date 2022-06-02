use crate::driver::RdmaDriver;
use crate::{LocalAccess, LocalReadAccess, LocalWriteAccess};

use rdma::mr::{AccessFlags, MemoryRegion};

use std::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use std::mem::ManuallyDrop;
use std::slice;

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
