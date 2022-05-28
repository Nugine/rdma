use crate::bindings as C;
use crate::ctx::Context;
use crate::error::from_errno;
use crate::utils::{box_assume_init, box_new_uninit};

use std::io;
use std::ptr;

pub struct DeviceAttr(Box<C::ibv_device_attr_ex>);

impl DeviceAttr {
    #[inline]
    pub fn query(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let mut device_attr = box_new_uninit::<C::ibv_device_attr_ex>();
            let context = ctx.ffi_ptr();
            let input = ptr::null();
            let ret = C::ibv_query_device_ex(context, input, device_attr.as_mut_ptr());
            if ret != 0 {
                return Err(from_errno(ret));
            }
            Ok(Self(box_assume_init(device_attr)))
        }
    }

    #[inline]
    #[must_use]
    pub fn physical_port_count(&self) -> u32 {
        self.0.orig_attr.phys_port_cnt.into()
    }
}
