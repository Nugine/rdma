use crate::error::create_resource;
use crate::query::DeviceAttr;
use crate::query::PortAttr;
use crate::resource::Resource;
use crate::CompChannel;
use crate::CompletionQueue;
use crate::CompletionQueueOptions;
use crate::Device;
use crate::GidEntry;
use crate::ProtectionDomain;
use crate::QueuePair;
use crate::QueuePairOptions;

use rdma_sys::ibv_context;
use rdma_sys::{ibv_close_device, ibv_open_device};

use std::cell::UnsafeCell;
use std::io;
use std::ptr::NonNull;

use asc::Asc;

#[derive(Clone)]
pub struct Context(Asc<Inner>);

/// SAFETY: shared resource type
unsafe impl Resource for Context {
    type Ctype = ibv_context;

    fn ffi_ptr(&self) -> *mut Self::Ctype {
        self.0.ctx.as_ptr().cast()
    }

    fn strong_ref(&self) -> Self {
        Self(Asc::clone(&self.0))
    }
}

impl Context {
    #[inline]
    pub fn open(device: &Device) -> io::Result<Self> {
        let inner = Inner::open(device)?;
        Ok(Self(Asc::new(inner)))
    }

    #[inline]
    pub fn alloc_pd(&self) -> io::Result<ProtectionDomain> {
        ProtectionDomain::alloc(self)
    }

    #[inline]
    pub fn create_cc(&self) -> io::Result<CompChannel> {
        CompChannel::create(self)
    }

    #[inline]
    pub fn create_cq(&self, options: CompletionQueueOptions) -> io::Result<CompletionQueue> {
        CompletionQueue::create(self, options)
    }

    #[inline]
    pub fn query_device(&self) -> io::Result<DeviceAttr> {
        DeviceAttr::query(self)
    }

    #[inline]
    pub fn query_port(&self, port_num: u32) -> io::Result<PortAttr> {
        PortAttr::query(self, port_num)
    }

    #[inline]
    pub fn query_gid_entry(&self, port_num: u32, gid_index: u32) -> io::Result<GidEntry> {
        GidEntry::query(self, port_num, gid_index)
    }

    #[inline]
    pub fn create_qp(&self, options: QueuePairOptions) -> io::Result<QueuePair> {
        QueuePair::create(self, options)
    }
}

pub(crate) struct Inner {
    ctx: NonNull<UnsafeCell<ibv_context>>,
}

/// SAFETY: owned type
unsafe impl Send for Inner {}
/// SAFETY: owned type
unsafe impl Sync for Inner {}

impl Inner {
    fn open(device: &Device) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let ctx = create_resource(
                || ibv_open_device(device.ffi_ptr()),
                || "failed to open device",
            )?;
            Ok(Self { ctx: ctx.cast() })
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let context: *mut ibv_context = self.ctx.as_ptr().cast();
            let ret = ibv_close_device(context);
            assert_eq!(ret, 0);
        }
    }
}
