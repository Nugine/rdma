use std::mem::MaybeUninit;
use std::{fmt, slice};

/// A RDMA device guid
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct Guid([u8; 8]);

impl Guid {
    /// Constructs a Guid from network bytes.
    #[inline]
    #[must_use]
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Returns the bytes of GUID in network byte order.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl fmt::Debug for Guid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({self:x})")
    }
}

fn guid_to_hex<R>(guid: Guid, case: hex_simd::AsciiCase, f: impl FnOnce(&str) -> R) -> R {
    let src: &[u8; 8] = guid.as_bytes();
    let mut buf: MaybeUninit<[u8; 16]> = MaybeUninit::uninit();
    let ans = {
        // SAFETY: uninit project
        let bytes = unsafe { slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), 16) };
        let dst = hex_simd::Out::from_uninit_slice(bytes);
        hex_simd::encode_as_str(src, dst, case)
    };
    f(ans)
}

impl fmt::LowerHex for Guid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        guid_to_hex(*self, hex_simd::AsciiCase::Lower, |s| {
            <str as fmt::Display>::fmt(s, f)
        })
    }
}

impl fmt::UpperHex for Guid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        guid_to_hex(*self, hex_simd::AsciiCase::Upper, |s| {
            <str as fmt::Display>::fmt(s, f)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::require_send_sync;

    use const_str::hex;

    #[test]
    fn guid_fmt() {
        const GUID_HEX: &str = "26418cfffe021df9";
        let guid = Guid::from_bytes(hex!(GUID_HEX));

        let debug = format!("{guid:?}");
        let lower_hex = format!("{guid:x}");
        let upper_hex = format!("{guid:X}");

        assert_eq!(debug, format!("Guid({GUID_HEX})"));
        assert_eq!(lower_hex, GUID_HEX);
        assert_eq!(upper_hex, GUID_HEX.to_ascii_uppercase());
    }

    #[test]
    fn marker() {
        require_send_sync::<Guid>();
    }
}
