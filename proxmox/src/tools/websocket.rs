//! Websocket helpers
//!
//! Provides methods to read and write from websockets
//! The reader and writer take a reader/writer with AsyncRead/AsyncWrite
//! respectively and provides the same

use std::pin::Pin;
use std::task::{Context, Poll};
use std::cmp::min;
use std::io::{self, Error, ErrorKind};
use std::future::Future;

use tokio::io::{AsyncWrite, AsyncRead, AsyncReadExt};

use futures::future::FutureExt;
use futures::ready;

use crate::tools::byte_buffer::ByteBuffer;

#[repr(u8)]
#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub enum OpCode {
    Continuation = 0,
    Text = 1,
    Binary = 2,
    Close = 8,
    Ping = 9,
    Pong = 10,
}

impl OpCode {
    pub fn is_control(self) -> bool {
        return self as u8 & 0b1000 > 0;
    }
}

fn mask_bytes(mask: Option<[u8; 4]>, data: &mut Vec<u8>) -> &mut Vec<u8> {
    let mask = match mask {
        Some([0,0,0,0]) | None => return data,
        Some(mask) => mask,
    };

    if data.len() < 32 {
        let mut_data = data.as_mut_slice();
        for i in 0..mut_data.len() {
            mut_data[i] ^= mask[i%4];
        }
        return data;
    }

    let mut newmask: u32 = u32::from_le_bytes(mask);

    let (prefix, middle, suffix) = unsafe { data.align_to_mut::<u32>() };

    for p in prefix {
        *p ^= newmask as u8;
        newmask = newmask.rotate_right(8);
    }

    for m in middle {
        *m ^= newmask;
    }

    for s in suffix {
        *s ^= newmask as u8;
        newmask = newmask.rotate_right(8);
    }

    data
}

pub fn create_frame(
    mask: Option<[u8; 4]>,
    mut data: Vec<u8>,
    frametype: OpCode,
) -> io::Result<Vec<u8>> {
    let first_byte = 0b10000000 | (frametype as u8);
    let len = data.len();
    if (frametype as u8) & 0b00001000 > 0 && len > 125 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "Control frames cannot have data longer than 125 bytes",
        ));
    }

    let mask_bit = if mask.is_some() { 0b10000000 } else { 0b00000000 };

    let mut buf = Vec::new();
    buf.push(first_byte);

    if len < 126 {
        buf.push(mask_bit | (len as u8));
    } else if len < std::u16::MAX as usize {
        buf.push(mask_bit | 126);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        buf.push(mask_bit | 127);
        buf.extend_from_slice(&(len as u64).to_be_bytes());
    }

    if let Some(mask) = mask {
        buf.extend_from_slice(&mask);
    }

    buf.append(&mut mask_bytes(mask, &mut data));
    Ok(buf)
}

pub struct WebSocketWriter<W: AsyncWrite> {
    writer: W,
    text: bool,
    mask: Option<[u8; 4]>,
    frame: Option<(Vec<u8>, usize, usize)>,
}

impl<W: AsyncWrite> WebSocketWriter<W> {
    pub fn new(mask: Option<[u8; 4]>, text: bool, writer: W) -> WebSocketWriter<W> {
        WebSocketWriter {
            writer: writer,
            text,
            mask,
            frame: None,
        }
    }
}

impl<W: AsyncWrite> AsyncWrite for WebSocketWriter<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let this = unsafe { Pin::into_inner_unchecked(self) };

        let frametype = match this.text {
            true => OpCode::Text,
            false => OpCode::Binary,
        };

        if this.frame.is_none() {
            // create frame buf
            let frame = match create_frame(this.mask, buf.to_vec(), frametype) {
                Ok(f) => f,
                Err(e) => {
                    return Poll::Ready(Err(e));
                }
            };
            this.frame = Some((frame, 0, buf.len()));
        }

        // we have a frame in any case, so unwrap is ok
        let (buf, pos, origsize) = this.frame.as_mut().unwrap();
        loop {
            //let size = unsafe { Pin::new_unchecked(&mut this.writer) }.poll_write(cx, &buf[*pos..]))?
            match unsafe { Pin::new_unchecked(&mut this.writer) }.poll_write(cx, &buf[*pos..]) {
                Poll::Ready(Ok(size)) => {
                    *pos += size;
                    if *pos == buf.len() {
                        let size = *origsize;
                        this.frame = None;
                        return Poll::Ready(Ok(size));
                    }
                }
                other => return other,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        unsafe { self.map_unchecked_mut(|x| &mut x.writer).poll_flush(cx) }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Error>> {
        unsafe { self.map_unchecked_mut(|x| &mut x.writer).poll_shutdown(cx) }
    }
}

#[derive(Debug)]
pub struct FrameHeader {
    pub fin: bool,
    pub mask: Option<[u8; 4]>,
    pub frametype: OpCode,
    pub header_len: u8,
    pub payload_len: usize,
}

impl FrameHeader {
    pub fn is_control_frame(&self) -> bool {
        return self.frametype.is_control();
    }

    pub fn try_from_bytes(data: &[u8]) -> Result<Result<FrameHeader, usize>, Error> {
        let len = data.len();
        if len < 2 {
            return Ok(Err(2 - len));
        }

        let data = data;

        // we do not support extensions
        if data[0] & 0b01110000 > 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Extensions not supported",
            ));
        }

        let fin = data[0] & 0b10000000 != 0;
        let frametype = match data[0] & 0b1111 {
            0 => OpCode::Continuation,
            1 => OpCode::Text,
            2 => OpCode::Binary,
            8 => OpCode::Close,
            9 => OpCode::Ping,
            10 => OpCode::Pong,
            other => {
                return Err(Error::new(ErrorKind::InvalidData, format!("Unknown OpCode {}", other)));
            }
        };

        if !fin && frametype.is_control() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Control frames cannot be fragmented",
            ));
        }

        let mask_bit = data[1] & 0b10000000 != 0;
        let mut mask_offset = 2;
        let mut payload_offset = 2;
        if mask_bit {
            payload_offset += 4;
        }

        let mut payload_len: usize = (data[1] & 0b01111111).into();
        if payload_len == 126 {
            if len < 4 {
                return Ok(Err(4 - len));
            }
            payload_len = u16::from_be_bytes([data[2], data[3]]) as usize;
            mask_offset += 2;
            payload_offset += 2;
        } else if payload_len == 127 {
            if len < 10 {
                return Ok(Err(10 - len));
            }
            payload_len = u64::from_be_bytes([
                data[2], data[3], data[4], data[5], data[6], data[7], data[8], data[9],
            ]) as usize;
            mask_offset += 8;
            payload_offset += 8;
        }

        if payload_len > 125 && frametype.is_control() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Control frames cannot carry more than 125 bytes of data",
            ));
        }

        let mask = match mask_bit {
            true => {
                if len < mask_offset + 4 {
                    return Ok(Err(mask_offset + 4 - len));
                }
                let mut mask = [0u8; 4];
                mask.copy_from_slice(&data[mask_offset as usize..payload_offset as usize]);
                Some(mask)
            }
            false => None,
        };

        Ok(Ok(FrameHeader {
            fin,
            mask,
            frametype,
            payload_len,
            header_len: payload_offset,
        }))
    }
}

pub type CallBack = fn(frametype: OpCode, payload: &[u8]);

pub struct WebSocketReader<R: AsyncRead> {
    reader: Option<R>,
    callback: CallBack,
    read_buffer: Option<ByteBuffer>,
    header: Option<FrameHeader>,
    state: ReaderState<R>,
}

impl<R: AsyncReadExt + Unpin + Send + 'static> WebSocketReader<R> {
    pub fn new(reader: R, callback: CallBack) -> WebSocketReader<R> {
        Self::with_capacity(reader, callback, 4096)
    }

    pub fn with_capacity(reader: R, callback: CallBack, capacity: usize) -> WebSocketReader<R> {
        WebSocketReader {
            reader: Some(reader),
            callback,
            read_buffer: Some(ByteBuffer::with_capacity(capacity)),
            header: None,
            state: ReaderState::NoData,
        }
    }
}


enum ReaderState<R> {
    NoData,
    WaitingForData(Pin<Box<dyn Future<Output = Result<(R, ByteBuffer), Error>> + Send + 'static>>),
    HaveData,
}

impl<R: AsyncReadExt + Unpin + Send + 'static> AsyncRead for WebSocketReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        let this = Pin::get_mut(self);
        let mut offset = 0;

        loop {
            match &mut this.state {
                ReaderState::NoData => {
                    let mut reader = match this.reader.take() {
                        Some(reader) => reader,
                        None => return Poll::Ready(Err(Error::new(ErrorKind::Other, "no reader"))),
                    };

                    let mut buffer = match this.read_buffer.take() {
                        Some(buffer) => buffer,
                        None => return Poll::Ready(Err(Error::new(ErrorKind::Other, "no buffer"))),
                    };

                    let future = async move {
                        buffer.read_from_async(&mut reader)
                            .await
                            .map(move |_| (reader, buffer))
                    };

                    this.state = ReaderState::WaitingForData(future.boxed());
                },
                ReaderState::WaitingForData(ref mut future) => {
                    match ready!(future.as_mut().poll(cx)) {
                        Ok((reader, buffer)) => {
                            this.reader = Some(reader);
                            this.read_buffer = Some(buffer);
                            this.state = ReaderState::HaveData;

                        },
                        Err(err) => return Poll::Ready(Err(Error::new(ErrorKind::Other, err))),
                    }
                },
                ReaderState::HaveData => {
                    let mut read_buffer = match this.read_buffer.take() {
                        Some(read_buffer) => read_buffer,
                        None => return Poll::Ready(Err(Error::new(ErrorKind::Other, "no buffer"))),
                    };

                    let mut header = match this.header.take() {
                        Some(header) => header,
                        None => {
                            let header = match FrameHeader::try_from_bytes(read_buffer.get_data_slice())? {
                                Ok(header) => header,
                                Err(_) => {
                                    this.state = ReaderState::NoData;
                                    this.read_buffer = Some(read_buffer);
                                    continue;
                                }
                            };

                            read_buffer.consume(header.header_len as usize);
                            header
                        },
                    };

                    if header.is_control_frame() {
                        if read_buffer.data_size() >= header.payload_len {
                            (this.callback)(
                                header.frametype,
                                mask_bytes(
                                    header.mask,
                                    &mut read_buffer.consume(header.payload_len).into_vec(),
                                ),
                            );
                            this.state =  if read_buffer.is_empty() {
                                ReaderState::NoData
                            } else {
                                ReaderState::HaveData
                            };
                            this.read_buffer = Some(read_buffer);
                            continue;
                        } else {
                            this.header = Some(header);
                            this.read_buffer = Some(read_buffer);
                            this.state = ReaderState::NoData;
                            continue;
                        }
                    }

                    let len = min(buf.len() - offset, min(header.payload_len, read_buffer.data_size()));
                    let mut data = read_buffer.consume(len).into_vec();
                    buf[offset..offset+len].copy_from_slice(mask_bytes(header.mask, &mut data));
                    offset += len;

                    header.payload_len -= len;

                    if header.payload_len > 0 {
                        this.header = Some(header);
                    }

                    this.state = if read_buffer.is_empty() {
                        ReaderState::NoData
                    } else {
                        ReaderState::HaveData
                    };
                    this.read_buffer = Some(read_buffer);

                    return Poll::Ready(Ok(offset));
                },
            }
        }
    }
}
