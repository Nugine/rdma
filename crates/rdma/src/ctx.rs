use crate::cc::CompChannel;
use crate::cq::{CompletionQueue, CompletionQueueOptions};
use crate::device::Device;
use crate::error::create_resource;
use crate::pd::ProtectionDomain;
use crate::query::DeviceAttr;
use crate::query::GidEntry;
use crate::query::PortAttr;
use crate::resource::Resource;

use rdma_sys::ibv_context;
use rdma_sys::{ibv_close_device, ibv_open_device};

use std::io;
use std::ptr::NonNull;
use std::sync::Arc;

#[derive(Clone)]
pub struct Context(Arc<Owner>);

/// SAFETY: resource type
unsafe impl Resource for Context {
    type Owner = Owner;

    fn as_owner(&self) -> &Arc<Self::Owner> {
        &self.0
    }
}

impl Context {
    pub(crate) fn ffi_ptr(&self) -> *mut ibv_context {
        self.0.ffi_ptr()
    }

    #[inline]
    pub fn open(device: &Device) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let ctx = create_resource(
                || ibv_open_device(device.ffi_ptr()),
                || "failed to open device",
            )?;
            Arc::new(Owner { ctx })
        };
        Ok(Self(owner))
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
}

pub(crate) struct Owner {
    ctx: NonNull<ibv_context>,
}

/// SAFETY: owned type
unsafe impl Send for Owner {}
/// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn ffi_ptr(&self) -> *mut ibv_context {
        self.ctx.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let context = self.ffi_ptr();
            let ret = ibv_close_device(context);
            assert_eq!(ret, 0);
        }
    }
}
