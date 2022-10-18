use crate::bindings as C;
use crate::ctx::Context;
use crate::error::custom_error;
use crate::utils::c_uint_to_u32;

use std::mem::MaybeUninit;
use std::net::Ipv6Addr;
use std::os::raw::c_uint;
use std::{fmt, io, slice};

#[repr(transparent)]
pub struct GidEntry(C::ibv_gid_entry);

impl GidEntry {
    #[inline]
    pub fn query(ctx: &Context, port_num: u32, gid_index: u32) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let mut gid = MaybeUninit::<Self>::uninit();
            let context = ctx.ffi_ptr();
            let entry = gid.as_mut_ptr().cast::<C::ibv_gid_entry>();
            let flags = 0; // ASK: what is this?
            let ret = C::ibv_query_gid_ex(context, port_num, gid_index, entry, flags);
            if ret != 0 {
                return Err(custom_error("failed to query gid entry"));
            }
            Ok(gid.assume_init())
        }
    }

    #[inline]
    #[must_use]
    pub fn gid_type(&self) -> GidType {
        GidType::from_c_uint(self.0.gid_type)
    }

    #[inline]
    #[must_use]
    pub fn gid(&self) -> Gid {
        Gid(self.0.gid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GidType {
    IB = c_uint_to_u32(C::IBV_GID_TYPE_IB),
    RoceV1 = c_uint_to_u32(C::IBV_GID_TYPE_ROCE_V1),
    RoceV2 = c_uint_to_u32(C::IBV_GID_TYPE_ROCE_V2),
}

impl GidType {
    fn from_c_uint(val: c_uint) -> Self {
        match val {
            C::IBV_GID_TYPE_IB => GidType::IB,
            C::IBV_GID_TYPE_ROCE_V1 => GidType::RoceV1,
            C::IBV_GID_TYPE_ROCE_V2 => GidType::RoceV2,
            _ => panic!("unknown gid type"),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Gid(C::ibv_gid);

impl Gid {
    #[inline]
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(C::ibv_gid { raw: bytes })
    }

    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        // SAFETY: type raw bytes
        unsafe { &self.0.raw }
    }

    #[inline]
    #[must_use]
    pub fn to_ipv6_addr(&self) -> Ipv6Addr {
        Ipv6Addr::from(*self.as_bytes())
    }

    #[inline]
    #[must_use]
    pub const fn subnet_prefix(&self) -> u64 {
        // SAFETY: POD
        unsafe { self.0.global.subnet_prefix }
    }

    #[inline]
    #[must_use]
    pub const fn interface_id(&self) -> u64 {
        // SAFETY: POD
        unsafe { self.0.global.interface_id }
    }
}

impl PartialEq for Gid {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for Gid {}

impl fmt::Debug for Gid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gid({self:x})")
    }
}

impl fmt::LowerHex for Gid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        gid_to_hex(self, hex_simd::AsciiCase::Lower, |s| f.write_str(s))
    }
}

impl fmt::UpperHex for Gid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        gid_to_hex(self, hex_simd::AsciiCase::Upper, |s| f.write_str(s))
    }
}

fn gid_to_hex<R>(gid: &Gid, case: hex_simd::AsciiCase, f: impl FnOnce(&str) -> R) -> R {
    // SAFETY: same repr
    let src: &[u8; 16] = gid.as_bytes();
    let mut buf: MaybeUninit<[u8; 32]> = MaybeUninit::uninit();
    let ans = {
        // SAFETY: uninit project
        let bytes = unsafe { slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), 32) };
        let dst = hex_simd::OutBuf::uninit(bytes);
        let result = hex_simd::encode_as_str(src, dst, case);
        // SAFETY: the encoding never fails
        unsafe { result.unwrap_unchecked() }
    };
    f(ans)
}

#[cfg(feature = "serde")]
mod serde_impl {
    use super::Gid;

    use serde::{Deserialize, Serialize};

    impl Serialize for Gid {
        #[inline]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            // FIXME: bytes format or struct format?
            <[u8; 16] as Serialize>::serialize(self.as_bytes(), serializer)
        }
    }

    impl<'de> Deserialize<'de> for Gid {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            <[u8; 16] as Deserialize<'de>>::deserialize(deserializer).map(Self::from_bytes)
        }
    }
}

#[cfg(feature = "bytemuck")]
mod bytemuck_impl {
    use super::Gid;

    use bytemuck::{Pod, Zeroable};

    /// SAFETY: POD
    unsafe impl Zeroable for Gid {}

    /// SAFETY: POD
    unsafe impl Pod for Gid {}
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::require_send_sync;

    use const_str::hex;

    #[test]
    fn gid_fmt() {
        const GID_HEX: &str = "fe800000000000009acd3cec6916fc65";
        let gid = Gid::from_bytes(hex!(GID_HEX));

        let debug = format!("{gid:?}");
        let lower_hex = format!("{gid:x}");
        let upper_hex = format!("{gid:X}");

        assert_eq!(debug, format!("Gid({GID_HEX})"));
        assert_eq!(lower_hex, GID_HEX);
        assert_eq!(upper_hex, GID_HEX.to_ascii_uppercase());
    }

    #[test]
    fn marker() {
        require_send_sync::<GidEntry>();
        require_send_sync::<Gid>();
    }
}
