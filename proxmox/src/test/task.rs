use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub fn poll_result_once<T, R>(mut fut: T) -> std::io::Result<R>
where
    T: Future<Output = std::io::Result<R>>,
{
    let waker = std::task::RawWaker::new(std::ptr::null(), &WAKER_VTABLE);
    let waker = unsafe { std::task::Waker::from_raw(waker) };
    let mut cx = Context::from_waker(&waker);
    unsafe {
        match Pin::new_unchecked(&mut fut).poll(&mut cx) {
            Poll::Pending => Err(crate::sys::error::io_err_other(
                "got Poll::Pending synchronous context",
            )),
            Poll::Ready(r) => r,
        }
    }
}

const WAKER_VTABLE: std::task::RawWakerVTable =
    std::task::RawWakerVTable::new(forbid_clone, forbid_wake, forbid_wake, ignore_drop);

unsafe fn forbid_clone(_: *const ()) -> std::task::RawWaker {
    panic!("tried to clone waker for synchronous task");
}

unsafe fn forbid_wake(_: *const ()) {
    panic!("tried to wake synchronous task");
}
unsafe fn ignore_drop(_: *const ()) {}
