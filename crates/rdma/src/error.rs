use std::io;

pub fn last_errno() -> io::Error {
    io::Error::last_os_error()
}

pub fn custom_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

pub fn from_errno(errno: i32) -> io::Error {
    io::Error::from_raw_os_error(errno)
}
