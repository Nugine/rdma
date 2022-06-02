use numeric_cast::NumericCast;
use rdma::wr::Sge;

/// # Safety
/// TODO
pub unsafe trait LocalAccess {
    fn addr_u64(&self) -> u64;
    fn length(&self) -> usize;
    fn lkey(&self) -> u32;
}

/// # Safety
/// TODO
pub unsafe trait LocalReadAccess: LocalAccess {}

/// # Safety
/// TODO
pub unsafe trait LocalWriteAccess: LocalAccess {}

/// # Safety
/// TODO
pub unsafe trait RemoteAccess {
    fn addr_u64(&self) -> u64;
    fn length(&self) -> usize;
    fn rkey(&self) -> u32;
}

/// # Safety
/// TODO
pub unsafe trait RemoteReadAccess: RemoteAccess {}

/// # Safety
/// TODO
pub unsafe trait RemoteWriteAccess: RemoteAccess {}

/// # Safety
/// TODO
pub unsafe trait ScatterList {
    fn length(&self) -> usize;
    /// # Safety
    /// TODO
    unsafe fn fill(&self, ptr: *mut Sge);
}

/// # Safety
/// TODO
pub unsafe trait GatherList {
    fn length(&self) -> usize;
    /// # Safety
    /// TODO
    unsafe fn fill(&self, ptr: *mut Sge);
}

unsafe impl<T, const N: usize> ScatterList for [T; N]
where
    T: LocalReadAccess,
{
    fn length(&self) -> usize {
        N
    }

    unsafe fn fill(&self, mut ptr: *mut Sge) {
        for t in self {
            let sge = Sge {
                addr: t.addr_u64(),
                length: t.length().numeric_cast(),
                lkey: t.lkey(),
            };
            ptr.write(sge);
            ptr = ptr.add(1);
        }
    }
}

unsafe impl<T, const N: usize> GatherList for [T; N]
where
    T: LocalWriteAccess,
{
    fn length(&self) -> usize {
        N
    }

    unsafe fn fill(&self, mut ptr: *mut Sge) {
        for t in self {
            let sge = Sge {
                addr: t.addr_u64(),
                length: t.length().numeric_cast(),
                lkey: t.lkey(),
            };
            ptr.write(sge);
            ptr = ptr.add(1);
        }
    }
}

unsafe impl<T> ScatterList for T
where
    T: LocalReadAccess,
{
    fn length(&self) -> usize {
        1
    }

    unsafe fn fill(&self, ptr: *mut Sge) {
        let sge = Sge {
            addr: self.addr_u64(),
            length: self.length().numeric_cast(),
            lkey: self.lkey(),
        };
        ptr.write(sge);
    }
}

unsafe impl<T> GatherList for T
where
    T: LocalWriteAccess,
{
    fn length(&self) -> usize {
        1
    }

    unsafe fn fill(&self, ptr: *mut Sge) {
        let sge = Sge {
            addr: self.addr_u64(),
            length: self.length().numeric_cast(),
            lkey: self.lkey(),
        };
        ptr.write(sge);
    }
}
