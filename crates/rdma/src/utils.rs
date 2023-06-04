#![allow(clippy::as_conversions)]

use std::mem;
use std::os::raw::{c_int, c_uint, c_void};

#[cfg(test)]
pub fn require_send_sync<T: Send + Sync>() {}

#[allow(clippy::unnecessary_cast)]
pub const fn c_uint_to_u32(x: c_uint) -> u32 {
    assert!(!(mem::size_of::<c_uint>() > mem::size_of::<u32>() && x > u32::MAX as c_uint));
    x as u32
}

pub const fn bool_to_c_int(b: bool) -> c_int {
    b as c_int
}

pub fn ptr_to_addr<T>(p: *const T) -> usize {
    p as usize
}

pub fn ptr_from_addr<T>(val: usize) -> *const T {
    val as *const T
}

pub fn ptr_as_mut<T>(p: *const T) -> *mut T {
    p.cast_mut()
}

pub fn usize_to_void_ptr(val: usize) -> *mut c_void {
    val as *mut c_void
}

pub fn void_ptr_to_usize(p: *mut c_void) -> usize {
    p as usize
}

pub fn u32_as_c_uint(val: u32) -> c_uint {
    val as c_uint
}
