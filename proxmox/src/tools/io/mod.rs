//! Module providing I/O helpers (sync and async).
//!
//! The [`ReadExt`] trait provides additional operations for handling byte buffers for types
//! implementing [`Read`](std::io::Read).

use std::io::{self, ErrorKind, Read, Seek, SeekFrom, Write};

mod read;
pub use read::*;

mod write;
pub use write::*;

fn buffer_is_zero(buf: &[u8]) -> bool {
    !buf.chunks(128)
        .map(|aa| aa.iter().fold(0, |a, b| a | b) != 0)
        .any(|a| a)
}

/// Result of a sparse copy call
/// contains the amount of written/seeked bytes
/// and if the last operation was a seek
pub struct SparseCopyResult {
    pub written: u64,
    pub seeked_last: bool,
}

/// copy similar to io::copy, but seeks the target when encountering
/// zero bytes instead of writing them
///
/// Example use:
/// ```
/// # use std::io;
/// # use proxmox::tools::io::sparse_copy;
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
    let mut buf = crate::tools::byte_buffer::ByteBuffer::with_capacity(4096);
    let mut written = 0;
    let mut seek_amount: i64 = 0;
    let mut seeked_last = false;
    loop {
        buf.clear();
        let len = match buf.read_from(reader) {
            Ok(len) => len,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };

        if len > 0 && buffer_is_zero(&buf[..]) {
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
            writer.write_all(&buf[..])?;
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

#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

#[cfg(feature = "tokio")]
/// copy similar to tokio::io::copy, but seeks the target when encountering
/// zero bytes instead of writing them
///
/// Example:
/// ```no_run
/// # use std::io;
/// # use tokio::io::{AsyncRead, AsyncWrite, AsyncSeek};
/// # use proxmox::tools::io::sparse_copy_async;
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
pub async fn sparse_copy_async<R, W>(
    reader: &mut R,
    writer: &mut W,
) -> Result<SparseCopyResult, io::Error>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + AsyncSeek + Unpin,
{
    let mut buf = crate::tools::byte_buffer::ByteBuffer::with_capacity(4096);
    let mut written = 0;
    let mut seek_amount: i64 = 0;
    let mut seeked_last = false;
    loop {
        buf.clear();
        let len = match buf.read_from_async(reader).await {
            Ok(len) => len,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };

        if len > 0 && buffer_is_zero(&buf[..]) {
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
            writer.write_all(&buf[..]).await?;
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

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use crate::test::io::{AsyncBlockingReader, AsyncBlockingWriter};
    use crate::tools::io::{sparse_copy, sparse_copy_async};

    const LEN: usize = 10000;

    #[test]
    fn test_sparse_copy() {
        // test sparse
        let mut test_data = Vec::new();
        for _ in 0..LEN / 2 {
            test_data.push(1u8);
        }
        for _ in 0..LEN / 2 {
            test_data.push(0u8);
        }
        let mut test_data = Cursor::new(test_data);
        let mut result_data = Cursor::new(vec![0; LEN]);

        let result =
            sparse_copy(&mut test_data, &mut result_data).expect("error during sparse copy");
        assert_eq!(result.written, LEN as u64);
        assert_eq!(result.seeked_last, true);
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
        assert_eq!(result.seeked_last, false);
        for i in 0..LEN {
            assert_eq!(result_data.get_ref()[i], 1);
        }
    }

    #[test]
    fn test_sparse_copy_async() {
        let fut = async {
            // test sparse
            let mut test_data = Vec::new();
            for _ in 0..LEN / 2 {
                test_data.push(1u8);
            }
            for _ in 0..LEN / 2 {
                test_data.push(0u8);
            }
            let mut test_data = AsyncBlockingReader::new(Cursor::new(test_data));
            let mut result_data = AsyncBlockingWriter::new(Cursor::new(vec![0; LEN]));

            let result = sparse_copy_async(&mut test_data, &mut result_data)
                .await
                .expect("error during sparse copy");

            assert_eq!(result.written, LEN as u64);
            assert_eq!(result.seeked_last, true);
            for i in 0..LEN {
                if i < LEN / 2 {
                    assert_eq!(result_data.inner().get_ref()[i], 1);
                } else {
                    assert_eq!(result_data.inner().get_ref()[i], 0);
                }
            }

            // test non sparse
            let mut test_data = AsyncBlockingReader::new(Cursor::new(vec![1; LEN]));
            let mut result_data = AsyncBlockingWriter::new(Cursor::new(vec![0; LEN]));

            let result = sparse_copy_async(&mut test_data, &mut result_data)
                .await
                .expect("error during sparse copy");

            assert_eq!(result.written, LEN as u64);
            assert_eq!(result.seeked_last, false);
            for i in 0..LEN {
                assert_eq!(result_data.inner().get_ref()[i], 1);
            }
            Ok(())
        };

        crate::test::task::poll_result_once(fut).expect("ok")
    }
}
