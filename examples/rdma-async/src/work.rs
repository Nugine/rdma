use crate::sg_list::SgList;
use crate::{GatherList, IntoGatherList, IntoScatterList, ScatterList};

use rdma::qp::QueuePair;
use rdma::wc::{WorkCompletion, WorkCompletionError};
use rdma::wr::{self, RecvRequest, SendRequest, Sge};

use std::future::Future;
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::{io, slice};

use anyhow::Result;
use numeric_cast::NumericCast;
use parking_lot::Mutex;
use scopeguard::ScopeGuard;

/// # Safety
/// TODO
unsafe trait Operation: Send + Sync + Unpin {
    fn submit(&mut self, qp: &QueuePair, id: u64) -> bool;
    fn complete(&mut self, wc: &WorkCompletion);
}

struct Work<T> {
    inner: Arc<WorkInner<T>>,
}

#[repr(C)]
struct WorkInner<T> {
    complete: unsafe fn(wc: *const WorkCompletion),
    state: Mutex<State<T>>,
}

struct State<T> {
    step: Step,
    waker: Option<Waker>,
    qp: QueuePair,
    op: ManuallyDrop<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Pending,
    Running,
    Completed,
    Invalid,
    Poisoned,
}

impl<T: Operation> Work<T> {
    fn new(qp: QueuePair, op: T) -> Self {
        Self {
            inner: Arc::new(WorkInner {
                complete: Self::complete,
                state: Mutex::new(State {
                    step: Step::Pending,
                    waker: None,
                    qp,
                    op: ManuallyDrop::new(op),
                }),
            }),
        }
    }

    unsafe fn complete(wc: *const WorkCompletion) {
        let wc = &*wc;
        let inner: Arc<WorkInner<T>> = Arc::from_raw(wc.wr_id() as usize as *mut _);
        {
            let mut guard: _ = inner.state.lock();
            let state = &mut *guard;
            assert_eq!(state.step, Step::Running);
            state.op.complete(wc);
            state.step = Step::Completed;
            if let Some(ref waker) = state.waker {
                waker.wake_by_ref();
            }
        }
    }
}

pub unsafe fn complete(wc: &WorkCompletion) {
    let inner: *mut WorkInner<()> = wc.wr_id() as usize as *mut _;
    ((*inner).complete)(wc)
}

impl<T: Operation> Future for Work<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut guard = self.inner.state.lock();
        let state = &mut *guard;
        match mem::replace(&mut state.step, Step::Poisoned) {
            Step::Pending => {
                let inner_ptr: *const WorkInner<T> = Arc::into_raw(Arc::clone(&self.inner));
                let arc_guard: _ =
                    scopeguard::guard((), |()| unsafe { Arc::decrement_strong_count(inner_ptr) });

                let id: u64 = (inner_ptr as usize).numeric_cast();

                if state.op.submit(&state.qp, id) {
                    // SAFETY: state refcount
                    ScopeGuard::into_inner(arc_guard);

                    state.step = Step::Running;
                    state.waker = Some(cx.waker().clone());
                    Poll::Pending
                } else {
                    // SAFETY: state refcount
                    drop(arc_guard);

                    state.step = Step::Invalid;
                    // SAFETY: managed state machine
                    let op = unsafe { ManuallyDrop::take(&mut state.op) };
                    Poll::Ready(op)
                }
            }
            Step::Running => {
                state.step = Step::Running;
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
            Step::Completed => {
                state.step = Step::Invalid;
                // SAFETY: managed state machine
                let op = unsafe { ManuallyDrop::take(&mut state.op) };
                Poll::Ready(op)
            }
            Step::Invalid => panic!("the future is completed or failed"),
            Step::Poisoned => panic!("the future is poisoned"),
        }
    }
}

impl<T> Drop for State<T> {
    fn drop(&mut self) {
        match self.step {
            Step::Pending | Step::Completed | Step::Poisoned => {
                // SAFETY: managed state machine
                unsafe { ManuallyDrop::drop(&mut self.op) };
            }
            Step::Running => panic!("state should not be dropped when step is running"),
            Step::Invalid => {}
        }
    }
}

unsafe fn convert_sglist<R>(sg_list: SgList<'_>, f: impl FnOnce(&[Sge]) -> R) -> R {
    const N: usize = 4;
    let mut arr_sg_list;
    let mut vec_sg_list;
    let sg_list: &[Sge] = {
        let len = sg_list.length();
        if len <= N {
            arr_sg_list = MaybeUninit::<[Sge; N]>::uninit();
            let ptr = arr_sg_list.as_mut_ptr().cast();
            sg_list.fill(ptr);
            slice::from_raw_parts(ptr, len)
        } else {
            vec_sg_list = Vec::with_capacity(len);
            sg_list.fill(vec_sg_list.as_mut_ptr());
            vec_sg_list.set_len(len);
            vec_sg_list.as_slice()
        }
    };
    f(sg_list)
}

unsafe fn submit_single_send(
    qp: &QueuePair,
    id: u64,
    sg_list: SgList<'_>,
    res: &mut io::Result<()>,
    f: &mut dyn FnMut(&mut SendRequest),
) -> bool {
    let cq = qp.send_cq().expect("the qp can not post send");

    *res = cq.req_notify_all();
    if res.is_err() {
        return false;
    }

    convert_sglist(sg_list, |sg_list| {
        let mut send_wr = SendRequest::zeroed();
        send_wr
            .id(id)
            .sg_list(sg_list)
            .send_flags(wr::SendFlags::SIGNALED);
        f(&mut send_wr);

        *res = qp.post_send(&send_wr);
        res.is_ok()
    })
}

unsafe fn submit_single_recv(
    qp: &QueuePair,
    id: u64,
    sg_list: SgList<'_>,
    res: &mut io::Result<()>,
) -> bool {
    let cq = qp.recv_cq().expect("the qp can not post recv");

    *res = cq.req_notify_all();
    if res.is_err() {
        return false;
    }

    convert_sglist(sg_list, |sg_list| {
        let mut recv_wr = RecvRequest::zeroed();
        recv_wr.id(id).sg_list(sg_list);

        *res = qp.post_recv(&recv_wr);
        res.is_ok()
    })
}

struct OpSend<T> {
    slist: T,
    res: io::Result<()>,
    status: u32,
}

impl<T> Unpin for OpSend<T> {}

/// SAFETY: operation type
unsafe impl<T> Operation for OpSend<T>
where
    T: ScatterList + Send + Sync,
{
    fn submit(&mut self, qp: &QueuePair, id: u64) -> bool {
        unsafe {
            let sg_list = SgList::from_slist(&self.slist);
            let res: _ = &mut self.res;
            submit_single_send(qp, id, sg_list, res, &mut |send_wr| {
                send_wr.opcode(wr::Opcode::Send);
            })
        }
    }

    fn complete(&mut self, wc: &WorkCompletion) {
        self.status = wc.status();
    }
}

struct OpRecv<T> {
    glist: T,
    res: io::Result<()>,
    status: u32,
    byte_len: u32,
}

impl<T> Unpin for OpRecv<T> {}

/// SAFETY: operation type
unsafe impl<T> Operation for OpRecv<T>
where
    T: GatherList + Send + Sync,
{
    fn submit(&mut self, qp: &QueuePair, id: u64) -> bool {
        unsafe {
            let sg_list = SgList::from_glist(&self.glist);
            let res = &mut self.res;
            submit_single_recv(qp, id, sg_list, res)
        }
    }

    fn complete(&mut self, wc: &WorkCompletion) {
        self.status = wc.status();
        self.byte_len = wc.byte_len();
    }
}

pub async fn send<I>(qp: QueuePair, slist: I) -> (Result<()>, I::Output)
where
    I: IntoScatterList,
    I::Output: Send + Sync,
{
    let work = Work::new(
        qp,
        OpSend {
            slist: slist.into_scatter_list(),
            res: Ok(()),
            status: u32::MAX,
        },
    );

    let op = work.await;

    if let Err(err) = op.res {
        return (Err(err.into()), op.slist);
    }
    if let Err(err) = WorkCompletionError::result(op.status) {
        return (Err(err.into()), op.slist);
    }
    (Ok(()), op.slist)
}

pub async fn recv<I>(qp: QueuePair, glist: I) -> (Result<usize>, I::Output)
where
    I: IntoGatherList,
    I::Output: Send + Sync,
{
    let work = Work::new(
        qp,
        OpRecv {
            glist: glist.into_gather_list(),
            res: Ok(()),
            status: u32::MAX,
            byte_len: 0,
        },
    );

    let op = work.await;

    if let Err(err) = op.res {
        return (Err(err.into()), op.glist);
    }
    if let Err(err) = WorkCompletionError::result(op.status) {
        return (Err(err.into()), op.glist);
    }
    (Ok(op.byte_len.numeric_cast()), op.glist)
}
