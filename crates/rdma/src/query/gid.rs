use crate::error::custom_error;
use crate::utils::c_uint_to_u32;
use crate::Context;

use rdma_sys::{ibv_gid, ibv_gid_entry, ibv_query_gid_ex};
use rdma_sys::{IBV_GID_TYPE_IB, IBV_GID_TYPE_ROCE_V1, IBV_GID_TYPE_ROCE_V2};

use std::mem::MaybeUninit;
use std::{fmt, io, slice};

#[repr(transparent)]
pub struct GidEntry(ibv_gid_entry);

impl GidEntry {
    #[inline]
    pub fn query(ctx: &Context, port_num: u32, gid_index: u32) -> io::Result<Self> {
        // SAFETY: ffi
        unsafe {
            let mut gid = MaybeUninit::<Self>::uninit();
            let context = ctx.0.ffi_ptr();
            let entry = gid.as_mut_ptr().cast::<ibv_gid_entry>();
            let flags = 0; // ASK: what is this?
            let ret = ibv_query_gid_ex(context, port_num, gid_index, entry, flags);
            if ret != 0 {
                return Err(custom_error("failed to query gid entry"));
            }
            Ok(gid.assume_init())
        }
    }

    #[inline]
    #[must_use]
    pub fn gid_type(&self) -> GidType {
        match self.0.gid_type {
            IBV_GID_TYPE_IB => GidType::IB,
            IBV_GID_TYPE_ROCE_V1 => GidType::RoceV1,
            IBV_GID_TYPE_ROCE_V2 => GidType::RoceV2,
            _ => panic!("unknown gid type"),
        }
    }

    #[inline]
    #[must_use]
    pub fn gid(&self) -> Gid {
        Gid(self.0.gid)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum GidType {
    IB = c_uint_to_u32(IBV_GID_TYPE_IB),
    RoceV1 = c_uint_to_u32(IBV_GID_TYPE_ROCE_V1),
    RoceV2 = c_uint_to_u32(IBV_GID_TYPE_ROCE_V2),
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Gid(ibv_gid);

impl Gid {
    #[inline]
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(ibv_gid { raw: bytes })
    }

    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        // SAFETY: type raw bytes
        unsafe { &self.0.raw }
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
        write!(f, "Gid({:x})", self)
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
        let dst = hex_simd::OutBuf::from_uninit_mut(bytes);
        let result = hex_simd::encode_as_str(src, dst, case);
        // SAFETY: the encoding never fails
        unsafe { result.unwrap_unchecked() }
    };
    f(ans)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::require_send_sync;

    use const_str::hex_bytes as hex;

    #[test]
    fn gid_fmt() {
        const GID_HEX: &str = "fe800000000000009acd3cec6916fc65";
        let gid = Gid::from_bytes(hex!(GID_HEX));

        let debug = format!("{:?}", gid);
        let lower_hex = format!("{:x}", gid);
        let upper_hex = format!("{:X}", gid);

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
