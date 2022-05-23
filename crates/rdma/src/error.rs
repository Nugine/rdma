use std::fmt;

pub struct Error(i32);

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! declare_messages {
    {$($no:ident => $msg: literal,)+} => {
        const MESSAGES: &[(i32, &str)] = &[
            $(
                (
                    libc::$no,
                    concat!($msg, " (", stringify!($no), ")")
                ),
            )+
        ];
    }
}

declare_messages! {
    EPERM   => "Permission denied",
    ENOMEM  => "Insufficient memory to complete the operation",
    ENOSYS  => "No kernel support for RDMA",
}

const _: () = {
    let mut i = 1;
    while i < MESSAGES.len() {
        let lhs = MESSAGES[i - 1];
        let rhs = MESSAGES[i];
        assert!(lhs.0 < rhs.0);
        i += 1;
    }
};

fn lookup_message(errno: i32) -> &'static str {
    static MESSAGE_TABLE: &[(i32, &str)] = MESSAGES;
    let table = MESSAGE_TABLE;
    match table.binary_search_by(|probe| probe.0.cmp(&errno)) {
        // SAFETY: binary search return value
        Ok(idx) => unsafe { table.get_unchecked(idx).1 },
        Err(_) => "Unknown error",
    }
}

impl Error {
    pub(crate) fn new(errno: i32) -> Self {
        Self(errno)
    }

    pub(crate) fn last() -> Self {
        // SAFETY: ffi
        let errno = unsafe { libc::__errno_location().read() };
        Self(errno)
    }

    #[inline]
    #[must_use]
    pub fn errno(&self) -> i32 {
        self.0
    }
}

impl fmt::Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", lookup_message(self.0))
    }
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}

impl std::error::Error for Error {}

impl From<Error> for std::io::Error {
    #[inline]
    fn from(err: Error) -> Self {
        std::io::Error::from_raw_os_error(err.errno())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message() {
        let err = Error::new(libc::EPERM);
        let msg = err.to_string();
        assert_eq!(msg, "Permission denied (EPERM)");
    }
}
