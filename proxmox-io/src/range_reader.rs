use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;

/// A reader that only exposes a sub-range of an underlying `Read + Seek`.
///
/// # Examples
///
/// ```
/// # use proxmox_io::RangeReader;
/// # use std::io::{Cursor, Read, Seek, SeekFrom};
/// # fn func() -> Result<(), std::io::Error> {
/// let reader = Cursor::new("Lorem ipsum dolor sit amet");
///
/// let mut range_reader = RangeReader::new(reader, 6..17);
///
/// // Read all bytes in the range
/// let mut buf = Vec::new();
/// range_reader.read_to_end(&mut buf)?;
/// assert_eq!(buf, "ipsum dolor".as_bytes());
///
/// // Seek back to start of the range and read one byte
/// range_reader.seek(SeekFrom::Start(0))?;
/// let mut b = [0u8; 1];
/// range_reader.read_exact(&mut b)?;
/// assert_eq!(b, "i".as_bytes());
///
/// # Ok(())
/// # }
/// # func().unwrap();
/// ```
pub struct RangeReader<R: Read + Seek> {
    /// Underlying reader
    reader: R,

    /// Range inside `R`
    range: Range<u64>,

    /// Relative position inside `range`
    position: u64,

    /// True once the initial seek has been performed
    has_seeked: bool,
}

impl<R: Read + Seek> RangeReader<R> {
    pub fn new(reader: R, range: Range<u64>) -> Self {
        Self {
            reader,
            range,
            position: 0,
            has_seeked: false,
        }
    }

    pub fn into_inner(self) -> R {
        self.reader
    }

    pub fn size(&self) -> usize {
        (self.range.end - self.range.start) as usize
    }

    pub fn remaining(&self) -> usize {
        self.size() - self.position as usize
    }
}

impl<R: Read + Seek> Read for RangeReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let max_read = buf.len().min(self.remaining());
        let limited_buf = &mut buf[..max_read];

        if !self.has_seeked {
            self.reader
                .seek(SeekFrom::Start(self.range.start + self.position))?;
            self.has_seeked = true;
        }

        let bytes_read = self.reader.read(limited_buf)?;
        self.position += bytes_read.min(max_read) as u64;

        Ok(bytes_read)
    }
}

impl<R: Read + Seek> Seek for RangeReader<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.position = match pos {
            SeekFrom::Start(position) => position.min(self.size() as u64),
            SeekFrom::End(offset) => {
                if offset > self.size() as i64 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Tried to seek before the beginning of the file",
                    ));
                }

                (if offset <= 0 {
                    self.size()
                } else {
                    self.size() - offset as usize
                }) as u64
            }
            SeekFrom::Current(offset) => {
                if let Some(position) = self.position.checked_add_signed(offset) {
                    position.min(self.size() as u64)
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Tried to seek before the beginning of the file",
                    ));
                }
            }
        };

        self.reader
            .seek(SeekFrom::Start(self.range.start + self.position))?;
        self.has_seeked = true;

        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::RangeReader;
    use std::io::{Cursor, Read, Seek, SeekFrom};

    #[test]
    fn test_read_full_range() {
        let reader = Cursor::new("Hello world!");
        let mut range_reader = RangeReader::new(reader, 6..11);

        let mut buf = Vec::new();
        let len = range_reader.read_to_end(&mut buf).unwrap();

        assert_eq!(len, 5);
        assert_eq!(buf, "world".as_bytes());
    }

    #[test]
    fn test_read_partial() {
        let reader = Cursor::new("Hello world!");
        let mut range_reader = RangeReader::new(reader, 0..5);

        let mut buf = [0u8; 4];
        range_reader.read_exact(&mut buf).unwrap();

        assert_eq!(buf, "Hell".as_bytes());
    }

    #[test]
    fn test_seek_and_read() {
        let reader = Cursor::new("Lorem ipsum dolor sit amet");
        let mut range_reader = RangeReader::new(reader, 6..21);

        assert_eq!(range_reader.seek(SeekFrom::Start(6)).unwrap(), 6);
        let mut buf = [0u8; 5];
        range_reader.read_exact(&mut buf).unwrap();

        assert_eq!(buf, "dolor".as_bytes());
    }

    #[test]
    fn test_seek_out_of_range() {
        let reader = Cursor::new("Lorem ipsum dolor sit amet");
        let mut range_reader = RangeReader::new(reader, 6..21);

        let err = range_reader.seek(SeekFrom::Current(-3)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

        let err = range_reader.seek(SeekFrom::End(20)).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }
}
