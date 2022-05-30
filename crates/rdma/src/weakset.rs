use crate::utils::{ptr_from_addr, ptr_to_addr};

use std::marker::PhantomData;
use std::sync::Weak;

use fnv::FnvHashSet;

pub struct WeakSet<T> {
    set: FnvHashSet<usize>,
    _marker: PhantomData<Weak<T>>,
}

impl<T> WeakSet<T> {
    pub fn new() -> Self {
        Self {
            set: FnvHashSet::default(),
            _marker: PhantomData,
        }
    }

    pub fn insert(&mut self, weak: Weak<T>) {
        self.set.insert(ptr_to_addr(Weak::into_raw(weak)));
    }

    pub fn remove(&mut self, p: *const T) -> bool {
        let is_removed = self.set.remove(&ptr_to_addr(p));
        if is_removed {
            // SAFETY: guaranteed by `WeakSet::insert`
            unsafe { drop(Weak::from_raw(p)) }
        }
        is_removed
    }
}

impl<T> Drop for WeakSet<T> {
    fn drop(&mut self) {
        for addr in self.set.drain() {
            // SAFETY: guaranteed by `WeakSet::insert`
            unsafe {
                let p = ptr_from_addr::<T>(addr);
                drop(Weak::from_raw(p));
            }
        }
    }
}
