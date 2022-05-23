use crate::{Error, Result};

use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::raw::c_int;
use std::ptr::NonNull;
use std::{fmt, mem, slice};

use rdma_sys::*;

use numeric_cast::NumericCast;
use scopeguard::guard_on_unwind;

pub struct DeviceList {
    arr: NonNull<NonNull<ibv_device>>,
    len: usize,
}

/// SAFETY: owned array
unsafe impl Send for DeviceList {}
/// SAFETY: owned array
unsafe impl Sync for DeviceList {}

#[repr(transparent)]
pub struct Device(NonNull<ibv_device>);

/// SAFETY: owned type
unsafe impl Send for Device {}
/// SAFETY: owned type
unsafe impl Sync for Device {}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Guid(__be64);

impl DeviceList {
    #[inline]
    pub fn available() -> Result<Self> {
        // SAFETY: ffi
        unsafe {
            let mut num_devices: c_int = 0;
            let arr = ibv_get_device_list(&mut num_devices);
            if arr.is_null() {
                return Err(Error::last());
            }
            let arr: NonNull<NonNull<ibv_device>> = NonNull::new_unchecked(arr.cast());

            let _guard = guard_on_unwind((), |()| ibv_free_device_list(arr.as_ptr().cast()));

            let len: usize = num_devices.numeric_cast();

            if mem::size_of::<c_int>() >= mem::size_of::<usize>() {
                let total_size = len.saturating_mul(mem::size_of::<*mut ibv_device>());
                assert!(total_size < usize::MAX.wrapping_div(2));
            }

            Ok(Self { arr, len })
        }
    }

    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[Device] {
        // SAFETY: guaranteed by `DeviceList::available`
        unsafe { slice::from_raw_parts(self.arr.as_ptr().cast(), self.len) }
    }
}

impl Drop for DeviceList {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe { ibv_free_device_list(self.arr.as_ptr().cast()) }
    }
}

impl Deref for DeviceList {
    type Target = [Device];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl Device {
    #[inline]
    #[must_use]
    pub fn c_name(&self) -> &CStr {
        // SAFETY: ffi
        unsafe { CStr::from_ptr(ibv_get_device_name(self.0.as_ptr())) }
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.c_name().to_str().expect("non-utf8 device name")
    }

    #[inline]
    #[must_use]
    pub fn guid(&self) -> Guid {
        // SAFETY: ffi
        unsafe { Guid(ibv_get_device_guid(self.0.as_ptr())) }
    }
}

impl Guid {
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 8] {
        // SAFETY: transparent be64
        unsafe { mem::transmute(self) }
    }
}

impl fmt::Debug for Guid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Guid({:x})", self)
    }
}

impl fmt::LowerHex for Guid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let src = self.as_bytes();
        let mut buf: MaybeUninit<[u8; 16]> = MaybeUninit::uninit();
        let ans = {
            // SAFETY: uninit project
            let bytes = unsafe { slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), 16) };
            let dst = hex_simd::OutBuf::from_uninit_mut(bytes);
            hex_simd::encode_as_str(src, dst, hex_simd::AsciiCase::Lower).unwrap()
        };
        f.write_str(ans)
    }
}

impl fmt::UpperHex for Guid {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let src = self.as_bytes();
        let mut buf: MaybeUninit<[u8; 16]> = MaybeUninit::uninit();
        let ans = {
            // SAFETY: uninit project
            let bytes = unsafe { slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), 16) };
            let dst = hex_simd::OutBuf::from_uninit_mut(bytes);
            hex_simd::encode_as_str(src, dst, hex_simd::AsciiCase::Upper).unwrap()
        };
        f.write_str(ans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_fmt() {
        let guid = Guid(u64::from_ne_bytes([
            0x26, 0x41, 0x8c, 0xff, 0xfe, 0x02, 0x1d, 0xf9,
        ]));
        let debug = format!("{:?}", guid);
        let lower_hex = format!("{:x}", guid);
        let upper_hex = format!("{:X}", guid);

        assert_eq!(debug, "Guid(26418cfffe021df9)");
        assert_eq!(lower_hex, "26418cfffe021df9");
        assert_eq!(upper_hex, "26418CFFFE021DF9");
    }
}
