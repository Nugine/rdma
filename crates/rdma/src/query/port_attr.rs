use crate::error::from_errno;
use crate::utils::{box_assume_init, box_new_uninit, c_uint_to_u32};
use crate::Context;

use std::io;
use std::os::raw::c_uint;

use rdma_sys::{ibv_port_attr, ibv_query_port};
use rdma_sys::{IBV_LINK_LAYER_ETHERNET, IBV_LINK_LAYER_INFINIBAND, IBV_LINK_LAYER_UNSPECIFIED};
use rdma_sys::{
    IBV_PORT_ACTIVE,       //
    IBV_PORT_ACTIVE_DEFER, //
    IBV_PORT_ARMED,        //
    IBV_PORT_DOWN,         //
    IBV_PORT_INIT,         //
    IBV_PORT_NOP,          //
};

use numeric_cast::NumericCast;

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
            if ret != 0 {
                return Err(from_errno(ret));
            }
            Ok(Self(box_assume_init(port_attr)))
        }
    }

    pub(crate) fn as_inner(&self) -> &ibv_port_attr {
        self.0.as_ref()
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> PortState {
        PortState::from_c_uint(self.0.state)
    }

    #[inline]
    #[must_use]
    pub fn gid_table_len(&self) -> usize {
        self.0.gid_tbl_len.numeric_cast()
    }

    #[inline]
    #[must_use]
    pub fn link_layer(&self) -> LinkLayer {
        LinkLayer::from_c_uint(c_uint::from(self.0.link_layer))
    }
}

#[derive(Debug)]
#[repr(u32)]
pub enum PortState {
    Nop = c_uint_to_u32(IBV_PORT_NOP),
    Down = c_uint_to_u32(IBV_PORT_DOWN),
    Init = c_uint_to_u32(IBV_PORT_INIT),
    Armed = c_uint_to_u32(IBV_PORT_ARMED),
    Active = c_uint_to_u32(IBV_PORT_ACTIVE),
    ActiveDefer = c_uint_to_u32(IBV_PORT_ACTIVE_DEFER),
}

impl PortState {
    fn from_c_uint(val: c_uint) -> PortState {
        match val {
            IBV_PORT_NOP => PortState::Nop,
            IBV_PORT_DOWN => PortState::Down,
            IBV_PORT_INIT => PortState::Init,
            IBV_PORT_ARMED => PortState::Armed,
            IBV_PORT_ACTIVE => PortState::Active,
            IBV_PORT_ACTIVE_DEFER => PortState::ActiveDefer,
            _ => panic!("unknown state"),
        }
    }
}

#[derive(Debug)]
#[repr(u32)]
pub enum LinkLayer {
    Ethernet = c_uint_to_u32(IBV_LINK_LAYER_ETHERNET),
    Infiniband = c_uint_to_u32(IBV_LINK_LAYER_INFINIBAND),
    Unspecified = c_uint_to_u32(IBV_LINK_LAYER_UNSPECIFIED),
}

impl LinkLayer {
    fn from_c_uint(val: c_uint) -> LinkLayer {
        match val {
            IBV_LINK_LAYER_ETHERNET => LinkLayer::Ethernet,
            IBV_LINK_LAYER_INFINIBAND => LinkLayer::Infiniband,
            IBV_LINK_LAYER_UNSPECIFIED => LinkLayer::Unspecified,
            _ => panic!("unknown link layer"),
        }
    }
}
