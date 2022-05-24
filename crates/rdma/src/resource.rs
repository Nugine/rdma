use std::ops::Deref;
use std::ptr::NonNull;

use asc::Asc;

/// # Safety
/// TODO
pub unsafe trait ResourceOwner: Send + Sync + Sized {
    type Ctype;
    fn ctype(&self) -> *mut Self::Ctype;
}

pub struct Resource<R: ResourceOwner> {
    owner: Asc<R>,
    ctype: NonNull<R::Ctype>,
}

/// SAFETY: guaranteed by `ResourceOwner`
unsafe impl<R: ResourceOwner> Send for Resource<R> {}
/// SAFETY: guaranteed by `ResourceOwner`
unsafe impl<R: ResourceOwner> Sync for Resource<R> {}

impl<R: ResourceOwner> Resource<R> {
    pub fn new(owner: R) -> Self {
        let owner = Asc::new(owner);
        // SAFETY: guaranteed by `ResourceOwner`
        let ctype = unsafe { NonNull::new_unchecked(owner.ctype()) };
        Self { owner, ctype }
    }

    pub fn ffi_ptr(&self) -> *mut R::Ctype {
        self.ctype.as_ptr()
    }

    pub fn strong_ref(&self) -> Asc<R> {
        Asc::clone(&self.owner)
    }
}

impl<R: ResourceOwner> Deref for Resource<R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &*self.owner
    }
}
