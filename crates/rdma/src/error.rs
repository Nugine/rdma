use std::io;
use std::ptr::NonNull;

pub fn last_error() -> io::Error {
    io::Error::last_os_error()
}

pub fn custom_error<E>(error: E) -> io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    io::Error::new(io::ErrorKind::Other, error)
}

pub fn from_errno(errno: i32) -> io::Error {
    io::Error::from_raw_os_error(errno)
}

pub fn create_resource<T, E>(
    f: impl FnOnce() -> *mut T,
    e: impl FnOnce() -> E,
) -> io::Result<NonNull<T>>
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    set_errno(0);
    let p = f();
    match NonNull::new(p) {
        Some(p) => Ok(p),
        None => {
            let errno = get_errno();
            Err(if errno == 0 {
                custom_error(e())
            } else {
                from_errno(errno)
            })
        }
    }
}

pub fn set_errno(errno: i32) {
    // SAFETY: write tls value
    unsafe { libc::__errno_location().write(errno) };
}

pub fn get_errno() -> i32 {
    // SAFETY: read tls value
    unsafe { libc::__errno_location().read() }
}
