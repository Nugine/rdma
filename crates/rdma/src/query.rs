use crate::error::from_errno;
use crate::utils::{box_assume_init, box_new_uninit};
use crate::Context;

use rdma_sys::{ibv_device_attr_ex, ibv_port_state, ibv_query_device_ex};
use rdma_sys::{ibv_port_attr, ibv_query_port};
use rdma_sys::{
    IBV_PORT_ACTIVE,       //
    IBV_PORT_ACTIVE_DEFER, //
    IBV_PORT_ARMED,        //
    IBV_PORT_DOWN,         //
    IBV_PORT_INIT,         //
    IBV_PORT_NOP,          //
};

use std::{io, mem, ptr};

use numeric_cast::NumericCast;

pub struct DeviceAttr(Box<ibv_device_attr_ex>);

impl DeviceAttr {
    #[inline]
    pub fn query(ctx: &Context) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let mut device_attr = box_new_uninit::<ibv_device_attr_ex>();
            let context = ctx.0.ffi_ptr();
            let input = ptr::null();
            let ret = ibv_query_device_ex(context, input, device_attr.as_mut_ptr());
            if ret > 0 {
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

    pub(crate) fn as_inner(&self) -> &ibv_device_attr_ex {
        self.0.as_ref()
    }
}

pub struct PortAttr(Box<ibv_port_attr>);

impl PortAttr {
    #[inline]
    pub fn query(ctx: &Context, port_num: u32) -> io::Result<Self> {
        let port_num: u8 = port_num.numeric_cast();

        // SAFETY: ffi
        unsafe {
            let mut port_attr = box_new_uninit::<ibv_port_attr>();
            let context = ctx.0.ffi_ptr();
            let ret = ibv_query_port(context, port_num, port_attr.as_mut_ptr());
            if ret > 0 {
                return Err(from_errno(ret));
            }
            Ok(Self(box_assume_init(port_attr)))
        }
    }

    pub(crate) fn as_inner(&self) -> &ibv_port_attr {
        self.0.as_ref()
    }

    pub fn state(&self) -> PortState {
        use self::PortState::*;
        match self.0.state {
            IBV_PORT_NOP => Nop,
            IBV_PORT_DOWN => Down,
            IBV_PORT_INIT => Init,
            IBV_PORT_ARMED => Armed,
            IBV_PORT_ACTIVE => Active,
            IBV_PORT_ACTIVE_DEFER => ActiveDefer,
            _ => panic!("unknown state"),
        }
    }
}

#[allow(clippy::as_conversions)]
#[derive(Debug)]
#[repr(u32)]
pub enum PortState {
    Nop = IBV_PORT_NOP as u32,
    Down = IBV_PORT_DOWN as u32,
    Init = IBV_PORT_INIT as u32,
    Armed = IBV_PORT_ARMED as u32,
    Active = IBV_PORT_ACTIVE as u32,
    ActiveDefer = IBV_PORT_ACTIVE_DEFER as u32,
}

const _: () = {
    assert!(mem::size_of::<ibv_port_state>() <= mem::size_of::<u32>());
};

#[cfg(test)]
mod tests {
    use super::*;

    use std::mem;

    #[test]
    fn track_type_size() {
        assert_eq!(mem::size_of::<ibv_device_attr_ex>(), 400);
        assert_eq!(mem::size_of::<ibv_port_attr>(), 52);
    }
}
