use crate::{GatherList, ScatterList};

use rdma::wr::Sge;

use std::marker::PhantomData;

pub struct SgList<'a> {
    this: *const (),
    length: usize,
    fill: unsafe fn(*const (), *mut Sge),
    _marker: PhantomData<&'a ()>,
}

impl<'a> SgList<'a> {
    pub fn from_slist<T: ScatterList>(slist: &'a T) -> Self {
        unsafe fn fill<T: ScatterList>(this: *const (), ptr: *mut Sge) {
            <T as ScatterList>::fill(&*this.cast::<T>(), ptr)
        }
        Self {
            this: <*const T>::cast::<()>(slist),
            length: slist.length(),
            fill: fill::<T>,
            _marker: PhantomData,
        }
    }

    pub fn from_glist<T: GatherList>(glist: &'a T) -> Self {
        unsafe fn fill<T: GatherList>(this: *const (), ptr: *mut Sge) {
            <T as GatherList>::fill(&*this.cast::<T>(), ptr)
        }
        Self {
            this: <*const T>::cast::<()>(glist),
            length: glist.length(),
            fill: fill::<T>,
            _marker: PhantomData,
        }
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub unsafe fn fill(&self, ptr: *mut Sge) {
        (self.fill)(self.this, ptr)
    }
}
