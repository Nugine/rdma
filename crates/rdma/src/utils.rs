#![allow(clippy::as_conversions)]

use std::alloc::{alloc, handle_alloc_error, Layout};
use std::mem::{self, MaybeUninit};
use std::os::raw::{c_int, c_uint, c_void};

#[cfg(test)]
pub fn require_send_sync<T: Send + Sync>() {}

/// See <https://github.com/rust-lang/rust/issues/63291>
pub fn box_new_uninit<T>() -> Box<MaybeUninit<T>> {
    let layout = Layout::new::<T>();
    // SAFETY: alloc
    unsafe {
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout)
        }
        Box::from_raw(ptr.cast())
    }
}

/// See <https://github.com/rust-lang/rust/issues/63291>
pub unsafe fn box_assume_init<T>(b: Box<MaybeUninit<T>>) -> Box<T> {
    let ptr = Box::into_raw(b).cast::<T>();
    Box::from_raw(ptr)
}

pub const fn c_uint_to_u32(x: c_uint) -> u32 {
    assert!(!(mem::size_of::<c_uint>() > mem::size_of::<u32>() && x > u32::MAX as c_uint));
    x as u32
}

pub const fn bool_to_c_int(b: bool) -> c_int {
    if b {
        1
    } else {
        0
    }
}

pub fn ptr_to_addr<T>(p: *const T) -> usize {
    p as usize
}

pub fn ptr_from_addr<T>(val: usize) -> *const T {
    val as *const T
}

pub fn ptr_as_mut<T>(p: *const T) -> *mut T {
    p as *mut T
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

/// Calculates the offset of the specified field from the start of the named struct.
/// This macro is impossible to be const until `feature(const_ptr_offset_from)` is stable.
macro_rules! offset_of {
    ($ty: path, $field: tt) => {{
        // ensure the type is a named struct
        // ensure the field exists and is accessible
        #[allow(clippy::unneeded_field_pattern)]
        let $ty { $field: _, .. };

        let uninit = <::core::mem::MaybeUninit<$ty>>::uninit(); // const since 1.36

        let base_ptr: *const $ty = uninit.as_ptr(); // const since 1.59

        #[allow(unused_unsafe)]
        // SAFETY: get raw ptr
        let field_ptr = unsafe { ::core::ptr::addr_of!((*base_ptr).$field) }; // since 1.51

        // // the const version requires feature(const_ptr_offset_from)
        // // https://github.com/rust-lang/rust/issues/92980
        // #[allow(unused_unsafe)]
        // unsafe { (field_ptr as *const u8).offset_from(base_ptr as *const u8) as usize }

        #[allow(clippy::integer_arithmetic, clippy::as_conversions)]
        {
            (field_ptr as usize) - (base_ptr as usize)
        }
    }};
}
