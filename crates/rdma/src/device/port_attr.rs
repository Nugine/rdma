use crate::bindings as C;
use crate::ctx::Context;
use crate::error::from_errno;
use crate::utils::{c_uint_to_u32, u32_as_c_uint};

use std::os::raw::c_uint;
use std::{io, mem};

use numeric_cast::NumericCast;
use rust_utils::{box_assume_init, box_new_zeroed};

pub struct PortAttr(Box<C::ibv_port_attr>);

impl PortAttr {
    #[inline]
    pub fn query(ctx: &Context, port_num: u8) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let mut port_attr = box_new_zeroed::<C::ibv_port_attr>();

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

    #[inline]
    #[must_use]
    pub fn active_mtu(&self) -> Mtu {
        Mtu::from_c_uint(self.0.active_mtu)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Mtu {
    Mtu256 = c_uint_to_u32(C::IBV_MTU_256),
    Mtu512 = c_uint_to_u32(C::IBV_MTU_512),
    Mtu1024 = c_uint_to_u32(C::IBV_MTU_1024),
    Mtu2048 = c_uint_to_u32(C::IBV_MTU_2048),
    Mtu4096 = c_uint_to_u32(C::IBV_MTU_4096),
}

impl Mtu {
    #[allow(clippy::as_conversions, clippy::unnecessary_cast)]
    fn from_c_uint(val: c_uint) -> Self {
        assert!((1..6).contains(&val), "unexpected MTU value");
        // SAFETY: continuous integer enum
        unsafe { mem::transmute(val as u32) }
    }

    #[allow(clippy::as_conversions)]
    fn to_u32(self) -> u32 {
        self as u32
    }

    pub(crate) fn to_c_uint(self) -> c_uint {
        u32_as_c_uint(self.to_u32())
    }

    #[inline]
    #[must_use]
    pub fn size(self) -> usize {
        let level = self.to_u32();
        1usize.wrapping_shl(level.wrapping_add(7))
    }
}
