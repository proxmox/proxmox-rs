//! tar helper
use std::collections::HashMap;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::str;

use anyhow::Error;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use tar::{EntryType, Header};

/// An async Builder for tar archives based on [tar::Builder]
///
/// Wraps an inner [AsyncWrite] struct to write into.
/// Must call [finish()](Builder::finish) to write trailer + close
/// # Example
///
/// ```
/// use tar::{EntryType, Header};
/// use proxmox_compression::tar::Builder;
///
/// # async fn foo() {
/// let mut tar = Builder::new(Vec::new());
///
/// // Add file
/// let mut header = Header::new_gnu();
/// let mut data: &[u8] = &[1, 2, 3];
/// header.set_size(data.len() as u64);
/// tar.add_entry(&mut header, "foo", data).await.unwrap();
///
/// // Add symlink
/// let mut header = Header::new_gnu();
/// header.set_entry_type(EntryType::Symlink);
/// tar.add_link(&mut header, "bar", "foo").await.unwrap();
///
/// // must call finish at the end
/// let data = tar.finish().await.unwrap();
/// # }
/// ```
pub struct Builder<W: AsyncWrite + Unpin> {
    inner: W,
}

impl<W: AsyncWrite + Unpin> Builder<W> {
    /// Takes an AsyncWriter as target
    pub fn new(inner: W) -> Builder<W> {
        Builder { inner }
    }

    async fn add<R: AsyncRead + Unpin>(&mut self, header: &Header, mut data: R) -> io::Result<()> {
        append_data(&mut self.inner, header, &mut data).await
    }

    /// Adds a new entry to this archive with the specified path.
    pub async fn add_entry<P, R>(&mut self, header: &mut Header, path: P, data: R) -> io::Result<()>
    where
        P: AsRef<Path>,
        R: AsyncRead + Unpin,
    {
        append_path_header(&mut self.inner, header, path.as_ref()).await?;
        header.set_cksum();
        self.add(header, data).await
    }

    /// Adds a new link (symbolic or hard) entry to this archive with the specified path and target.
    pub async fn add_link<P: AsRef<Path>, T: AsRef<Path>>(
        &mut self,
        header: &mut Header,
        path: P,
        target: T,
    ) -> io::Result<()> {
        append_path_header(&mut self.inner, header, path.as_ref()).await?;

        // try to set the linkame, fallback to gnu extension header otherwise
        if let Err(err) = header.set_link_name(target.as_ref()) {
            let link_name = target.as_ref().as_os_str().as_bytes();
            if link_name.len() < header.as_old().linkname.len() {
                return Err(err);
            }
            // add trailing '\0'
            let mut ext_data = link_name.chain(tokio::io::repeat(0).take(1));
            let extension = get_gnu_header(link_name.len() as u64 + 1, EntryType::GNULongLink);
            append_data(&mut self.inner, &extension, &mut ext_data).await?;
        }
        header.set_cksum();
        self.add(header, tokio::io::empty()).await
    }

    /// Finish the archive and flush the underlying writer
    ///
    /// Consumes the Builder. This must be called when finishing the archive.
    /// Flushes the inner writer and returns it.
    pub async fn finish(mut self) -> io::Result<W> {
        self.inner.write_all(&[0; 1024]).await?;
        self.inner.flush().await?;
        Ok(self.inner)
    }
}

async fn append_data<W, R>(mut dst: &mut W, header: &Header, mut data: &mut R) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    dst.write_all(header.as_bytes()).await?;
    let len = tokio::io::copy(&mut data, &mut dst).await?;

    // Pad with zeros if necessary.
    let buf = [0; 512];
    let remaining = 512 - (len % 512);
    if remaining < 512 {
        dst.write_all(&buf[..remaining as usize]).await?;
    }

    Ok(())
}

fn get_gnu_header(size: u64, entry_type: EntryType) -> Header {
    let mut header = Header::new_gnu();
    let name = b"././@LongLink";
    header.as_gnu_mut().unwrap().name[..name.len()].copy_from_slice(&name[..]);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_size(size);
    header.set_entry_type(entry_type);
    header.set_cksum();
    header
}

// tries to set the path in header, or add a gnu header with 'LongName'
async fn append_path_header<W>(dst: &mut W, header: &mut Header, path: &Path) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let mut relpath = PathBuf::new();
    let components = path.components();
    for comp in components {
        if Component::RootDir == comp {
            continue;
        }
        relpath.push(comp);
    }
    // try to set the path directly, fallback to gnu extension header otherwise
    if let Err(err) = header.set_path(&relpath) {
        let data = relpath.as_os_str().as_bytes();
        let max = header.as_old().name.len();
        if data.len() < max {
            return Err(err);
        }
        // add trailing '\0'
        let mut ext_data = data.chain(tokio::io::repeat(0).take(1));
        let extension = get_gnu_header(data.len() as u64 + 1, EntryType::GNULongName);
        append_data(dst, &extension, &mut ext_data).await?;

        // add the path as far as we can
        let truncated = match str::from_utf8(&data[..max]) {
            Ok(truncated) => truncated,
            Err(err) => str::from_utf8(&data[..err.valid_up_to()]).unwrap(),
        };
        header.set_path(truncated)?;
    }
    Ok(())
}

pub async fn tar_directory<W>(target: W, source: &Path) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + Send,
{
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    use walkdir::WalkDir;

    let base_path = source.parent().unwrap_or_else(|| Path::new("/"));
    let mut encoder = Builder::new(target);
    let mut hardlinks: HashMap<u64, HashMap<u64, PathBuf>> = HashMap::new(); // dev -> inode -> first path

    for entry in WalkDir::new(source).into_iter() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("zip: error reading directory entry: {err}");
                continue;
            }
        };

        let entry_path = entry.path().to_owned();
        let encoder = &mut encoder;
        let hardlinks = &mut hardlinks;

        if let Err(err) = async move {
            let entry_path_no_base = entry.path().strip_prefix(base_path)?;
            let metadata = entry.metadata()?;
            let mut header = Header::new_gnu();
            header.set_mode(metadata.mode());
            header.set_mtime(metadata.mtime() as u64);
            header.set_uid(metadata.uid() as u64);
            header.set_gid(metadata.gid() as u64);
            header.set_size(0);
            let dev = metadata.dev();

            let file_type = entry.file_type();

            if file_type.is_file() {
                if metadata.nlink() > 1 {
                    let ino = metadata.ino();
                    if let Some(map) = hardlinks.get_mut(&dev) {
                        if let Some(target) = map.get(&ino) {
                            header.set_entry_type(tar::EntryType::Link);
                            encoder
                                .add_link(&mut header, entry_path_no_base, target)
                                .await?;
                            return Ok(());
                        } else {
                            map.insert(ino, entry_path_no_base.to_path_buf());
                        }
                    } else {
                        let mut map = HashMap::new();
                        map.insert(ino, entry_path_no_base.to_path_buf());
                        hardlinks.insert(dev, map);
                    }
                }
                let file = tokio::fs::File::open(entry.path()).await?;
                header.set_size(metadata.size());
                header.set_cksum();
                encoder
                    .add_entry(&mut header, entry_path_no_base, file)
                    .await?;
            } else if file_type.is_dir() {
                header.set_entry_type(EntryType::Directory);
                header.set_cksum();
                encoder
                    .add_entry(&mut header, entry_path_no_base, tokio::io::empty())
                    .await?;
            } else if file_type.is_symlink() {
                let target = std::fs::read_link(entry.path())?;
                header.set_entry_type(EntryType::Symlink);
                encoder
                    .add_link(&mut header, entry_path_no_base, target)
                    .await?;
            } else if file_type.is_block_device() {
                header.set_entry_type(EntryType::Block);
                header.set_device_major(unsafe { libc::major(dev) })?;
                header.set_device_minor(unsafe { libc::minor(dev) })?;
                header.set_cksum();
                encoder
                    .add_entry(&mut header, entry_path_no_base, tokio::io::empty())
                    .await?;
            } else if file_type.is_char_device() {
                header.set_entry_type(EntryType::Char);
                header.set_device_major(unsafe { libc::major(dev) })?;
                header.set_device_minor(unsafe { libc::minor(dev) })?;
                header.set_cksum();
                encoder
                    .add_entry(&mut header, entry_path_no_base, tokio::io::empty())
                    .await?;
            } else if file_type.is_fifo() {
                header.set_entry_type(EntryType::Fifo);
                header.set_device_major(0)?;
                header.set_device_minor(0)?;
                header.set_cksum();
                encoder
                    .add_entry(&mut header, entry_path_no_base, tokio::io::empty())
                    .await?;
            }
            // ignore other file_types
            Ok::<_, Error>(())
        }
        .await
        {
            eprintln!(
                "zip: error encoding file or directory '{}': {}",
                entry_path.display(),
                err
            );
        }
    }
    Ok(())
}
