use crate::error::from_errno;
use crate::utils::{box_assume_init, box_new_uninit};
use crate::Context;

use std::io;

use rdma_sys::ibv_device_attr;
use rdma_sys::ibv_query_device;

pub struct DeviceAttr(Box<ibv_device_attr>);

// TODO: ibv_device_attr_ex

impl DeviceAttr {
    #[inline]
    pub fn query(ctx: &Context) -> io::Result<Self> {
        let mut device_attr = box_new_uninit::<ibv_device_attr>();
        // SAFETY: ffi
        unsafe {
            let ret = ibv_query_device(ctx.0.ffi_ptr(), device_attr.as_mut_ptr());
            if ret > 0 {
                return Err(from_errno(ret));
            }
            Ok(Self(box_assume_init(device_attr)))
        }
    }

    #[inline]
    #[must_use]
    pub fn physical_port_count(&self) -> u32 {
        self.0.phys_port_cnt.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::mem;

    use rdma_sys::ibv_device_attr_ex;

    #[test]
    fn track_type_size() {
        assert_eq!(mem::size_of::<ibv_device_attr>(), 232);
        assert_eq!(mem::size_of::<ibv_device_attr_ex>(), 400);
    }
}
