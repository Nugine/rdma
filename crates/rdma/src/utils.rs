use std::alloc::{alloc, handle_alloc_error, Layout};
use std::mem::{self, MaybeUninit};
use std::os::raw::{c_int, c_uint};

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

#[allow(clippy::as_conversions)]
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
