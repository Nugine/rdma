use crate::{GatherList, ScatterList};

use rdma::qp::QueuePair;
use rdma::wc::{WorkCompletion, WorkCompletionError};
use rdma::wr::{self, Sge};
use scopeguard::ScopeGuard;

use std::future::Future;
use std::io;
use std::mem::{self, ManuallyDrop};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use anyhow::Result;
use numeric_cast::NumericCast;
use parking_lot::Mutex;

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
        let cq = qp.send_cq().expect("the qp can not post send");

        self.res = cq.req_notify_all();
        if self.res.is_err() {
            return false;
        }

        // TODO: small vector optimization
        let sg_list = unsafe {
            let len = self.slist.length();
            let mut v: Vec<Sge> = Vec::with_capacity(len);
            self.slist.fill(v.as_mut_ptr());
            v.set_len(len);
            v
        };

        let mut send_wr = wr::SendRequest::zeroed();
        send_wr
            .id(id)
            .sg_list(&sg_list)
            .opcode(wr::Opcode::Send)
            .send_flags(wr::SendFlags::SIGNALED);

        // SAFETY: managed state machine
        self.res = unsafe { qp.post_send(&send_wr) };
        self.res.is_ok()
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
        let cq = qp.recv_cq().expect("the qp can not post recv");

        self.res = cq.req_notify_all();
        if self.res.is_err() {
            return false;
        }

        // TODO: small vector optimization
        let sg_list = unsafe {
            let len = self.glist.length();
            let mut v: Vec<Sge> = Vec::with_capacity(len);
            self.glist.fill(v.as_mut_ptr());
            v.set_len(len);
            v
        };

        let mut recv_wr = wr::RecvRequest::zeroed();
        recv_wr.id(id).sg_list(&sg_list);

        // SAFETY: managed state machine
        self.res = unsafe { qp.post_recv(&recv_wr) };
        self.res.is_ok()
    }

    fn complete(&mut self, wc: &WorkCompletion) {
        self.status = wc.status();
        self.byte_len = wc.byte_len();
    }
}

pub async fn send<T>(qp: QueuePair, slist: T) -> (Result<()>, T)
where
    T: ScatterList + Send + Sync,
{
    let work = Work::new(
        qp,
        OpSend {
            slist,
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

pub async fn recv<T>(qp: QueuePair, glist: T) -> (Result<usize>, T)
where
    T: GatherList + Send + Sync,
{
    let work = Work::new(
        qp,
        OpRecv {
            glist,
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
