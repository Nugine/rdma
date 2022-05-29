use crate::bindings as C;
use crate::ctx::Context;
use crate::error::from_errno;
use crate::utils::{box_assume_init, box_new_uninit, c_uint_to_u32};

use std::io;
use std::os::raw::c_uint;

use numeric_cast::NumericCast;

pub struct PortAttr(Box<C::ibv_port_attr>);

impl PortAttr {
    #[inline]
    pub fn query(ctx: &Context, port_num: u8) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let mut port_attr = box_new_uninit::<C::ibv_port_attr>();
            let context = ctx.ffi_ptr();
            let ret = C::ibv_query_port(context, port_num, port_attr.as_mut_ptr());
            if ret != 0 {
                return Err(from_errno(ret));
            }
            Ok(Self(box_assume_init(port_attr)))
        }
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> PortState {
        PortState::from_c_uint(self.0.state)
    }

    #[inline]
    #[must_use]
    pub fn gid_table_len(&self) -> u32 {
        self.0.gid_tbl_len.numeric_cast()
    }

    #[inline]
    #[must_use]
    pub fn link_layer(&self) -> LinkLayer {
        LinkLayer::from_c_uint(c_uint::from(self.0.link_layer))
    }

    #[inline]
    #[must_use]
    pub fn lid(&self) -> u16 {
        self.0.lid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PortState {
    Nop = c_uint_to_u32(C::IBV_PORT_NOP),
    Down = c_uint_to_u32(C::IBV_PORT_DOWN),
    Init = c_uint_to_u32(C::IBV_PORT_INIT),
    Armed = c_uint_to_u32(C::IBV_PORT_ARMED),
    Active = c_uint_to_u32(C::IBV_PORT_ACTIVE),
    ActiveDefer = c_uint_to_u32(C::IBV_PORT_ACTIVE_DEFER),
}

impl PortState {
    fn from_c_uint(val: c_uint) -> PortState {
        match val {
            C::IBV_PORT_NOP => PortState::Nop,
            C::IBV_PORT_DOWN => PortState::Down,
            C::IBV_PORT_INIT => PortState::Init,
            C::IBV_PORT_ARMED => PortState::Armed,
            C::IBV_PORT_ACTIVE => PortState::Active,
            C::IBV_PORT_ACTIVE_DEFER => PortState::ActiveDefer,
            _ => panic!("unknown state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LinkLayer {
    Ethernet = c_uint_to_u32(C::IBV_LINK_LAYER_ETHERNET),
    Infiniband = c_uint_to_u32(C::IBV_LINK_LAYER_INFINIBAND),
    Unspecified = c_uint_to_u32(C::IBV_LINK_LAYER_UNSPECIFIED),
}

impl LinkLayer {
    fn from_c_uint(val: c_uint) -> LinkLayer {
        match val {
            C::IBV_LINK_LAYER_ETHERNET => LinkLayer::Ethernet,
            C::IBV_LINK_LAYER_INFINIBAND => LinkLayer::Infiniband,
            C::IBV_LINK_LAYER_UNSPECIFIED => LinkLayer::Unspecified,
            _ => panic!("unknown link layer"),
        }
    }
}
