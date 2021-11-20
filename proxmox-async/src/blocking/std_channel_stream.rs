use std::pin::Pin;
use std::task::{Context, Poll};
use std::sync::mpsc::Receiver;

use futures::stream::Stream;

use crate::runtime::block_in_place;

/// Wrapper struct to convert a sync channel [Receiver] into a [Stream]
pub struct StdChannelStream<T>(pub Receiver<T>);

impl<T> Stream for StdChannelStream<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        match block_in_place(|| self.0.recv()) {
            Ok(data) => Poll::Ready(Some(data)),
            Err(_) => Poll::Ready(None),// channel closed
        }
    }
}
