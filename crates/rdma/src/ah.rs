use crate::bindings as C;
use crate::error::create_resource;
use crate::pd::{self, ProtectionDomain};
use crate::query::Gid;
use crate::resource::Resource;

use std::io;
use std::mem;
use std::ptr::NonNull;
use std::sync::Arc;

pub struct AddressHandle(Arc<Owner>);

impl AddressHandle {
    #[inline]
    pub fn create(pd: &ProtectionDomain, mut options: AddressHandleOptions) -> io::Result<Self> {
        // SAFETY: ffi
        let owner = unsafe {
            let attr = &mut options.attr;
            let ah = create_resource(
                || C::ibv_create_ah(pd.ffi_ptr(), attr),
                || "failed to create address handle",
            )?;
            Arc::new(Owner {
                ah,
                _pd: pd.strong_ref(),
            })
        };
        Ok(Self(owner))
    }
}

pub(crate) struct Owner {
    ah: NonNull<C::ibv_ah>,

    _pd: Arc<pd::Owner>,
}

// SAFETY: owned type
unsafe impl Send for Owner {}
// SAFETY: owned type
unsafe impl Sync for Owner {}

impl Owner {
    fn ffi_ptr(&self) -> *mut C::ibv_ah {
        self.ah.as_ptr()
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        // SAFETY: ffi
        unsafe {
            let ah = self.ffi_ptr();
            let ret = C::ibv_destroy_ah(ah);
            assert_eq!(ret, 0);
        }
    }
}

pub struct AddressHandleOptions {
    attr: C::ibv_ah_attr,
}

impl Default for AddressHandleOptions {
    #[inline]
    fn default() -> Self {
        Self {
            // SAFETY: POD ffi type
            attr: unsafe { mem::zeroed() },
        }
    }
}

impl AddressHandleOptions {
    pub(crate) fn into_ctype(self) -> C::ibv_ah_attr {
        self.attr
    }

    #[inline]
    pub fn dest_lid(&mut self, dest_lid: u16) -> &mut Self {
        self.attr.dlid = dest_lid;
        self
    }

    #[inline]
    pub fn service_level(&mut self, service_level: u8) -> &mut Self {
        self.attr.sl = service_level;
        self
    }

    #[inline]
    pub fn port_num(&mut self, port_num: u8) -> &mut Self {
        self.attr.port_num = port_num;
        self
    }
}

#[repr(C)]
pub struct GlobalRoute {
    pub dest_gid: Gid,
    pub flow_label: u32,
    pub sgid_index: u8,
    pub hop_limit: u8,
    pub traffic_class: u8,
}

// layout test
const _: () = {
    assert!(mem::size_of::<GlobalRoute>() == mem::size_of::<C::ibv_global_route>());
    assert!(mem::align_of::<GlobalRoute>() == mem::align_of::<C::ibv_global_route>());
    let gr = GlobalRoute {
        dest_gid: Gid::from_bytes([0; 16]),
        flow_label: 0,
        sgid_index: 0,
        hop_limit: 0,
        traffic_class: 0,
    };
    let _ = C::ibv_global_route {
        dgid: gr.dest_gid.into_ctype(),
        flow_label: gr.flow_label,
        sgid_index: gr.sgid_index,
        hop_limit: gr.hop_limit,
        traffic_class: gr.traffic_class,
    };
};
