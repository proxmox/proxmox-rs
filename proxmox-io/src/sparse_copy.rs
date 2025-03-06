use std::io::{self, BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write};

#[cfg(feature = "tokio")]
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt,
    BufReader as AsyncBufReader, BufWriter as AsyncBufWriter,
};

const CHUNK_SIZE: usize = 4096;
const BUF_SIZE: usize = 8192;

/// Efficiently check whether a byte slice contains only zeroes.
///
/// This is implemented in a way which allows the compiler to utilize SIMD instructions.
pub fn buffer_is_zero(buf: &[u8]) -> bool {
    !buf.chunks(128)
        .any(|aa| aa.iter().fold(0, |a, b| a | b) != 0)
}

/// Result of a sparse copy call.
///
/// Contains the amount of written/seeked bytes and if the last operation was a seek.
#[must_use = "if sparse_copy ended with a seek(), the output file's size may need to be adjusted"]
pub struct SparseCopyResult {
    pub written: u64,
    pub seeked_last: bool,
}

/// Copy similar to [`io::copy`], but seeks the target when encountering zero bytes instead of
/// writing them.
///
/// Example use:
/// ```
/// # use std::io;
/// # use proxmox_io::sparse_copy;
/// fn code<R, W>(mut reader: R, mut writer: W) -> io::Result<()>
/// where
///     R: io::Read,
///     W: io::Write + io::Seek,
/// {
///     let res = sparse_copy(&mut reader, &mut writer)?;
///
///     println!("last part was seeked: {}", res.seeked_last);
///     println!("written: {}", res.written);
///
///     Ok(())
/// }
/// ```
pub fn sparse_copy<R: Read + ?Sized, W: Write + Seek + ?Sized>(
    reader: &mut R,
    writer: &mut W,
) -> Result<SparseCopyResult, io::Error> {
    let mut reader = BufReader::with_capacity(BUF_SIZE, reader);
    let mut writer = BufWriter::with_capacity(BUF_SIZE, writer);

    let mut buf: Vec<u8> = crate::vec::undefined(CHUNK_SIZE);
    let mut written = 0;
    let mut seek_amount: i64 = 0;
    let mut seeked_last = false;

    loop {
        buf.clear();
        let len = match reader
            .by_ref()
            .take(CHUNK_SIZE as u64)
            .read_to_end(&mut buf)
        {
            Ok(len) => len,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };

        if len > 0 && buffer_is_zero(&buf[..len]) {
            seek_amount += len as i64;
            continue;
        }

        if seek_amount > 0 {
            writer.seek(SeekFrom::Current(seek_amount))?;
            written += seek_amount as u64;
            seek_amount = 0;
            seeked_last = true;
        }

        if len > 0 {
            writer.write_all(&buf[..len])?;
            written += len as u64;
            seeked_last = false;
        } else {
            return Ok(SparseCopyResult {
                written,
                seeked_last,
            });
        }
    }
}

/// copy similar to tokio::io::copy, but seeks the target when encountering
/// zero bytes instead of writing them
///
/// Example:
/// ```no_run
/// # use std::io;
/// # use tokio::io::{AsyncRead, AsyncWrite, AsyncSeek};
/// # use proxmox_io::sparse_copy_async;
/// async fn code<R, W>(mut reader: R, mut writer: W) -> io::Result<()>
/// where
///     R: AsyncRead + Unpin,
///     W: AsyncWrite + AsyncSeek + Unpin,
/// {
///     let res = sparse_copy_async(&mut reader, &mut writer).await?;
///
///     println!("last part was seeked: {}", res.seeked_last);
///     println!("written: {}", res.written);
///
///     Ok(())
/// }
/// ```
#[cfg(feature = "tokio")]
pub async fn sparse_copy_async<R, W>(
    reader: &mut R,
    writer: &mut W,
) -> Result<SparseCopyResult, io::Error>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + AsyncSeek + Unpin,
{
    let mut reader = AsyncBufReader::with_capacity(BUF_SIZE, reader);
    let mut writer = AsyncBufWriter::with_capacity(BUF_SIZE, writer);

    let mut buf: Vec<u8> = crate::vec::undefined(CHUNK_SIZE);
    let mut written = 0;
    let mut seek_amount: i64 = 0;
    let mut seeked_last = false;
    loop {
        buf.clear();
        let len = match (&mut reader)
            .take(CHUNK_SIZE as u64)
            .read_to_end(&mut buf)
            .await
        {
            Ok(len) => len,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };

        if len > 0 && buffer_is_zero(&buf[..len]) {
            seek_amount += len as i64;
            continue;
        }

        if seek_amount > 0 {
            writer.seek(SeekFrom::Current(seek_amount)).await?;
            written += seek_amount as u64;
            seek_amount = 0;
            seeked_last = true;
        }

        if len > 0 {
            writer.write_all(&buf[..len]).await?;
            written += len as u64;
            seeked_last = false;
        } else {
            // buffer must be flushed before it goes out of scope
            writer.flush().await?;
            return Ok(SparseCopyResult {
                written,
                seeked_last,
            });
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::sparse_copy;

    const LEN: usize = 10000;

    #[test]
    fn test_sparse_copy() {
        // test sparse
        let mut test_data = Vec::new();
        test_data.resize(LEN / 2, 1u8);
        test_data.resize(LEN, 0u8);

        let mut test_data = Cursor::new(test_data);
        let mut result_data = Cursor::new(vec![0; LEN]);

        let result =
            sparse_copy(&mut test_data, &mut result_data).expect("error during sparse copy");
        assert_eq!(result.written, LEN as u64);
        assert!(result.seeked_last);
        for i in 0..LEN {
            if i < LEN / 2 {
                assert_eq!(result_data.get_ref()[i], 1);
            } else {
                assert_eq!(result_data.get_ref()[i], 0);
            }
        }

        // test non sparse
        let mut test_data = Cursor::new(vec![1; LEN]);
        let mut result_data = Cursor::new(vec![0; LEN]);

        let result =
            sparse_copy(&mut test_data, &mut result_data).expect("error during sparse copy");
        assert_eq!(result.written, LEN as u64);
        assert!(!result.seeked_last);
        for i in 0..LEN {
            assert_eq!(result_data.get_ref()[i], 1);
        }
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn test_sparse_copy_async() {
        use std::future::Future;
        use std::sync::Arc;
        use std::task::{Context, Poll};

        use super::sparse_copy_async;

        struct PanicWaker;
        impl std::task::Wake for PanicWaker {
            fn wake(self: Arc<Self>) {
                panic!("this test should not block");
            }
        }

        let mut fut = async {
            // test sparse
            let mut test_data = Vec::new();
            test_data.resize(LEN / 2, 1u8);
            test_data.resize(LEN, 0u8);
            let mut test_data = Cursor::new(test_data);
            let mut result_data = Cursor::new(vec![0; LEN]);

            let result = sparse_copy_async(&mut test_data, &mut result_data)
                .await
                .expect("error during sparse copy");

            assert_eq!(result.written, LEN as u64);
            assert!(result.seeked_last);
            for i in 0..LEN {
                if i < LEN / 2 {
                    assert_eq!(result_data.get_ref()[i], 1);
                } else {
                    assert_eq!(result_data.get_ref()[i], 0);
                }
            }

            // test non sparse
            let mut test_data = Cursor::new(vec![1; LEN]);
            let mut result_data = Cursor::new(vec![0; LEN]);

            let result = sparse_copy_async(&mut test_data, &mut result_data)
                .await
                .expect("error during sparse copy");

            assert_eq!(result.written, LEN as u64);
            assert!(!result.seeked_last);
            for i in 0..LEN {
                assert_eq!(result_data.get_ref()[i], 1);
            }
            Ok::<(), std::io::Error>(())
        };

        let fut = unsafe { std::pin::Pin::new_unchecked(&mut fut) };
        let waker = std::task::Waker::from(Arc::new(PanicWaker));
        let mut context = Context::from_waker(&waker);
        match fut.poll(&mut context) {
            Poll::Ready(res) => res.expect("expected ok"),
            Poll::Pending => panic!("yielded on blocking implementation"),
        }
    }
}
