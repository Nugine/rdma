#![deny(clippy::all)]
#![allow(non_camel_case_types, non_snake_case, clippy::missing_safety_doc)]

use libc::*;
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// -------- libibverbs 1.14.41 -----------------------
// static inline functions

use std::mem;
use std::ops::Not;
use std::ptr;

pub unsafe fn ibv_cq_ex_to_cq(cq: *mut ibv_cq_ex) -> *mut ibv_cq {
    cq.cast()
}

/// Calculates the offset of the specified field from the start of the named struct.
/// This macro is impossible to be const until feature(const_ptr_offset_from) is stable.
macro_rules! offset_of {
    ($ty: path, $field: tt) => {{
        // ensure the type is a named struct
        // ensure the field exists and is accessible
        let $ty { $field: _, .. };

        let uninit = <::core::mem::MaybeUninit<$ty>>::uninit(); // const since 1.36

        let base_ptr: *const $ty = uninit.as_ptr(); // const since 1.59

        #[allow(unused_unsafe)]
        let field_ptr = unsafe { ::core::ptr::addr_of!((*base_ptr).$field) }; // since 1.51

        // // the const version requires feature(const_ptr_offset_from)
        // // https://github.com/rust-lang/rust/issues/92980
        // #[allow(unused_unsafe)]
        // unsafe { (field_ptr as *const u8).offset_from(base_ptr as *const u8) as usize }

        (field_ptr as usize) - (base_ptr as usize)
    }};
}

macro_rules! container_of {
    ($ptr: expr, $ty: path, $field: tt) => {{
        let ptr = $ptr;
        let offset = offset_of!($ty, $field);
        ptr.cast::<u8>().sub(offset).cast::<$ty>()
    }};
}

unsafe fn set_errno(errno: i32) {
    __errno_location().write(errno)
}

unsafe fn verbs_get_ctx(ctx: *mut ibv_context) -> *mut verbs_context {
    if (*ctx).abi_compat != mem::transmute::<_, *mut c_void>(usize::MAX) {
        return ptr::null_mut();
    }
    container_of!(ctx, verbs_context, context)
}

macro_rules! verbs_get_ctx_op {
    ($ctx: expr, $op: tt) => {{
        let vctx: *mut verbs_context = verbs_get_ctx($ctx);
        if vctx.is_null()
            || (*vctx).sz < ::core::mem::size_of::<verbs_context>() - offset_of!(verbs_context, $op)
            || (*vctx).$op.is_none()
        {
            ptr::null_mut()
        } else {
            vctx
        }
    }};
}

pub unsafe fn ibv_create_cq_ex(
    context: *mut ibv_context,
    cq_attr: *mut ibv_cq_init_attr_ex,
) -> *mut ibv_cq_ex {
    let vctx = verbs_get_ctx_op!(context, create_cq_ex);
    if vctx.is_null() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }
    let op = (*vctx).create_cq_ex.unwrap_unchecked();
    (op)(context, cq_attr)
}

pub unsafe fn ibv_query_gid_ex(
    context: *mut ibv_context,
    port_num: u32,
    gid_index: u32,
    entry: *mut ibv_gid_entry,
    flags: u32,
) -> c_int {
    _ibv_query_gid_ex(
        context,
        port_num,
        gid_index,
        entry,
        flags,
        mem::size_of::<ibv_gid_entry>(),
    )
}

mod compat {
    use super::{_compat_ibv_port_attr, c_int, ibv_context};

    extern "C" {
        pub fn ibv_query_port(
            context: *mut ibv_context,
            port_num: u8,
            port_attr: *mut _compat_ibv_port_attr,
        ) -> c_int;
    }
}

pub unsafe fn ibv_query_port(
    context: *mut ibv_context,
    port_num: u8,
    port_attr: *mut ibv_port_attr,
) -> c_int {
    let vctx: *mut verbs_context = verbs_get_ctx_op!(context, query_port);
    if vctx.is_null() {
        ptr::write_bytes(port_attr, 0, 1);
        return compat::ibv_query_port(context, port_num, port_attr.cast());
    }
    let op = (*vctx).query_port.unwrap_unchecked();
    (op)(
        context,
        port_num,
        port_attr,
        mem::size_of::<ibv_port_attr>(),
    )
}

pub unsafe fn ibv_query_device_ex(
    context: *mut ibv_context,
    input: *const ibv_query_device_ex_input,
    attr: *mut ibv_device_attr_ex,
) -> c_int {
    if input.is_null().not() && (*input).comp_mask != 0 {
        return EINVAL;
    }

    let legacy = || {
        ptr::write_bytes(attr, 0, 1);
        ibv_query_device(context, ptr::addr_of_mut!((*attr).orig_attr))
    };

    let vctx: *mut verbs_context = verbs_get_ctx_op!(context, query_device_ex);
    if vctx.is_null() {
        return legacy();
    }

    let op = (*vctx).query_device_ex.unwrap_unchecked();
    let ret = (op)(context, input, attr, mem::size_of::<ibv_device_attr_ex>());
    if ret == EOPNOTSUPP || ret == ENOSYS {
        return legacy();
    }
    ret
}

pub unsafe fn ibv_create_qp_ex(
    context: *mut ibv_context,
    qp_attr: *mut ibv_qp_init_attr_ex,
) -> *mut ibv_qp {
    let mask = (*qp_attr).comp_mask;
    if mask == IBV_QP_INIT_ATTR_PD {
        let pd = (*qp_attr).pd;
        return ibv_create_qp(pd, qp_attr.cast());
    }
    let vctx = verbs_get_ctx_op!(context, create_qp_ex);
    if vctx.is_null() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }
    let op = (*vctx).create_qp_ex.unwrap_unchecked();
    (op)(context, qp_attr)
}

pub unsafe fn ibv_req_notify_cq(cq: *mut ibv_cq, solicited_only: c_int) -> c_int {
    let ctx: *mut ibv_context = (*cq).context;
    let op: _ = (*ctx).ops.req_notify_cq.unwrap_unchecked();
    (op)(cq, solicited_only)
}

pub unsafe fn ibv_poll_cq(cq: *mut ibv_cq, num_entries: c_int, wc: *mut ibv_wc) -> c_int {
    let ctx: *mut ibv_context = (*cq).context;
    let op: _ = (*ctx).ops.poll_cq.unwrap_unchecked();
    (op)(cq, num_entries, wc)
}

pub unsafe fn ibv_alloc_mw(pd: *mut ibv_pd, mw_type: ibv_mw_type) -> *mut ibv_mw {
    let ctx: *mut ibv_context = (*pd).context;
    let op: _ = (*ctx).ops.alloc_mw;
    if op.is_none() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }
    let op: _ = op.unwrap_unchecked();
    (op)(pd, mw_type)
}

pub unsafe fn ibv_dealloc_mw(mw: *mut ibv_mw) -> c_int {
    let ctx: *mut ibv_context = (*mw).context;
    let op: _ = (*ctx).ops.dealloc_mw.unwrap_unchecked();
    (op)(mw)
}

pub unsafe fn ibv_bind_mw(qp: *mut ibv_qp, mw: *mut ibv_mw, mw_bind: *mut ibv_mw_bind) -> c_int {
    {
        let mw = &*mw;
        if (*mw).type_ != IBV_MW_TYPE_1 {
            return EINVAL;
        }

        let bind_info = &((*mw_bind).bind_info);
        if bind_info.mr.is_null() && (bind_info.addr != 0 || bind_info.length != 0) {
            return EINVAL;
        }
        if bind_info.mr.is_null().not() && (mw.pd != (*bind_info.mr).pd) {
            return EPERM;
        }
    }

    {
        let ctx: *mut ibv_context = (*mw).context;
        let op: _ = (*ctx).ops.bind_mw.unwrap_unchecked();
        (op)(qp, mw, mw_bind)
    }
}

pub unsafe fn ibv_alloc_dm(context: *mut ibv_context, attr: *mut ibv_alloc_dm_attr) -> *mut ibv_dm {
    let vctx: *mut verbs_context = verbs_get_ctx_op!(context, alloc_dm);

    if vctx.is_null() {
        set_errno(EOPNOTSUPP);
        return ptr::null_mut();
    }

    let op: _ = (*vctx).alloc_dm.unwrap_unchecked();
    (op)(context, attr)
}

pub unsafe fn ibv_free_dm(dm: *mut ibv_dm) -> c_int {
    let vctx: *mut verbs_context = verbs_get_ctx_op!((*dm).context, free_dm);

    if vctx.is_null() {
        return EOPNOTSUPP;
    }

    let op: _ = (*vctx).free_dm.unwrap_unchecked();
    (op)(dm)
}

pub unsafe fn ibv_post_send(
    qp: *mut ibv_qp,
    wr: *mut ibv_send_wr,
    bad_wr: *mut *mut ibv_send_wr,
) -> c_int {
    let ctx: *mut ibv_context = (*qp).context;
    let op: _ = (*ctx).ops.post_send.unwrap_unchecked();
    (op)(qp, wr, bad_wr)
}

pub unsafe fn ibv_post_recv(
    qp: *mut ibv_qp,
    wr: *mut ibv_recv_wr,
    bad_wr: *mut *mut ibv_recv_wr,
) -> c_int {
    let ctx: *mut ibv_context = (*qp).context;
    let op: _ = (*ctx).ops.post_recv.unwrap_unchecked();
    (op)(qp, wr, bad_wr)
}
