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
macro_rules! offset_of {
    ($ty: path, $field: tt) => {
        // // feature(inline_const)
        // const
        {
            #[allow(
                unused_unsafe,
                clippy::as_conversions,
                clippy::unneeded_field_pattern,
                clippy::undocumented_unsafe_blocks,
                clippy::integer_arithmetic,
                clippy::arithmetic
            )]
            unsafe {
                use ::core::mem::MaybeUninit;
                use ::core::primitive::usize;
                use ::core::ptr;

                // ensure the type is a named struct
                // ensure the field exists and is accessible
                let $ty { $field: _, .. };

                // const since 1.36
                let uninit: MaybeUninit<$ty> = MaybeUninit::uninit();

                // const since 1.59
                let base_ptr: *const $ty = uninit.as_ptr();

                // stable since 1.51
                let field_ptr: *const _ = ptr::addr_of!((*base_ptr).$field);

                // // feature(const_ptr_offset_from)
                // let base_addr = base_ptr.cast::<u8>();
                // let field_addr = field_ptr.cast::<u8>();
                // field_addr.offset_from(base_addr) as usize

                (field_ptr as usize) - (base_ptr as usize)
            }
        }
    };
}
