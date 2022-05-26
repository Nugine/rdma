/// # Safety
/// TODO
pub unsafe trait Resource: Send + Sync + Sized {
    type Ctype;
    fn ffi_ptr(&self) -> *mut Self::Ctype;
    fn strong_ref(&self) -> Self;
}
