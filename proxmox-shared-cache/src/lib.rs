use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Error;
use serde::de::DeserializeOwned;
use serde::Serialize;

use proxmox_sys::fs::CreateOptions;

/// A simple cache that can be used from multiple processes concurrently.
///
/// The cache can be configured to keep a number of most recent values.
///
/// ## Concurrency
/// `set` and `delete` lock the cache via an exclusive lock on a separate lock file.
/// `get` and `get_last` do not need a lock, since `set` and `delete` atomically
/// replace the cache file.
pub struct SharedCache {
    path: PathBuf,
    create_options: CreateOptions,
    keep_old: u32,
}

impl SharedCache {
    /// Instantiate a new cache instance for a given `path`.
    ///
    /// The path containing the cache file must already exist.
    /// The cache file itself will be created when first calling `set`.
    /// The file permissions will be determined by `create_options`.
    pub fn new<P: Into<PathBuf>>(
        path: P,
        create_options: CreateOptions,
        keep_old: u32,
    ) -> Result<Self, Error> {
        Ok(SharedCache {
            path: path.into(),
            create_options,
            keep_old,
        })
    }

    /// Returns the cache value.
    ///
    /// If `keep_old` > 0, this will return the value stored last.
    ///
    /// If the cache file does not exist or if it is empty, Ok(None) is returned.
    /// If the file could not be read for some other reason, or if the
    /// entry could not be deserialized an error is returned.
    pub fn get<V: DeserializeOwned>(&self) -> Result<Option<V>, Error> {
        match File::open(&self.path) {
            Ok(f) => {
                let mut lines = BufReader::new(f).lines();
                match lines.next() {
                    Some(Ok(line)) => Ok(Some(serde_json::from_str(&line)?)),
                    Some(Err(err)) => Err(err.into()),
                    None => Ok(None),
                }
            }
            Err(err) => {
                if err.kind() != ErrorKind::NotFound {
                    Err(err.into())
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Returns any last stored items, including `old_entries` of old items.
    ///
    /// If the cache file does not exist or if it is empty, Ok(vec![]) is returned.
    /// If the file could not be read for some other reason, or if the
    /// entry could not be deserialized an error is returned.
    pub fn get_last<V: DeserializeOwned>(&self, old_entries: u32) -> Result<Vec<V>, Error> {
        let mut items = Vec::new();

        let f = match File::open(&self.path) {
            Ok(f) => f,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(items),
            Err(err) => return Err(err.into()),
        };

        let mut lines = BufReader::new(f).lines();

        for _ in 0..=old_entries {
            if let Some(Ok(line)) = lines.next() {
                let item = serde_json::from_str(&line)?;
                items.push(item);
            } else {
                break;
            }
        }

        Ok(items)
    }

    /// Stores a new value.
    ///
    /// If the number of stored items exceeds 1 + keep_old, the
    /// least recently stored item will be dropped.
    ///
    /// Returns an error if the cache file could not be read/written
    /// or if the new value could not be serialized.
    pub fn set<V: Serialize>(&self, value: &V, lock_timeout: Duration) -> Result<(), Error> {
        let _lock = self.lock(lock_timeout);

        let mut new_content = serde_json::to_string(value)?;
        new_content.push('\n');

        match File::open(&self.path) {
            Ok(f) => {
                let mut lines = BufReader::new(f).lines();

                for _ in 0..self.keep_old {
                    if let Some(Ok(line)) = lines.next() {
                        new_content.push_str(&line);
                        new_content.push('\n');
                    } else {
                        break;
                    }
                }
            }
            Err(err) => {
                if err.kind() != ErrorKind::NotFound {
                    return Err(err.into());
                }
            }
        };

        proxmox_sys::fs::replace_file(
            &self.path,
            new_content.as_bytes(),
            self.create_options.clone(),
            true,
        )?;

        Ok(())
    }

    /// Removes all items from the cache.
    pub fn delete(&self, lock_timeout: Duration) -> Result<(), Error> {
        let _lock = self.lock(lock_timeout)?;
        proxmox_sys::fs::replace_file(&self.path, &[], self.create_options.clone(), true)?;

        Ok(())
    }

    fn lock(&self, lock_timeout: Duration) -> Result<File, Error> {
        let mut lockfile_path = self.path.clone();
        lockfile_path.set_extension("lock");
        proxmox_sys::fs::open_file_locked(
            lockfile_path,
            lock_timeout,
            true,
            self.create_options.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    struct TestCache {
        inner: SharedCache,
        dir: PathBuf,
    }

    impl TestCache {
        fn new(keep_old: u32) -> Self {
            let path = proxmox_sys::fs::make_tmp_dir("/tmp/", None).unwrap();

            let options = CreateOptions::new()
                .owner(nix::unistd::Uid::effective())
                .group(nix::unistd::Gid::effective())
                .perm(nix::sys::stat::Mode::from_bits_truncate(0o600));

            let dir_options = CreateOptions::new()
                .owner(nix::unistd::Uid::effective())
                .group(nix::unistd::Gid::effective())
                .perm(nix::sys::stat::Mode::from_bits_truncate(0o700));

            proxmox_sys::fs::create_path(
                &path,
                Some(dir_options.clone()),
                Some(dir_options.clone()),
            )
            .unwrap();

            let cache = SharedCache::new(path.join("somekey"), options, keep_old).unwrap();
            Self {
                inner: cache,
                dir: path,
            }
        }
    }

    impl Drop for TestCache {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn num(n: u32) -> Value {
        Value::from(n)
    }

    fn timeout() -> Duration {
        Duration::from_secs(1)
    }

    #[test]
    fn test_get() -> Result<(), Error> {
        let wrapper = TestCache::new(2);
        let cache = &wrapper.inner;

        let result: Option<Value> = cache.get()?;
        assert_eq!(result, None);

        cache.set(&num(0), timeout())?;
        let result: Option<Value> = cache.get()?;
        assert_eq!(result, Some(num(0)));

        cache.set(&num(1), timeout())?;
        let result: Option<Value> = cache.get()?;
        assert_eq!(result, Some(num(1)));
        Ok(())
    }

    #[test]
    fn test_get_without_history() -> Result<(), Error> {
        let wrapper = TestCache::new(0);
        let cache = &wrapper.inner;

        let result: Option<Value> = cache.get()?;
        assert_eq!(result, None);

        cache.set(&num(0), timeout())?;
        let result: Option<Value> = cache.get()?;
        assert_eq!(result, Some(num(0)));

        cache.set(&num(1), timeout())?;
        let result: Option<Value> = cache.get()?;
        assert_eq!(result, Some(num(1)));
        Ok(())
    }

    #[test]
    fn test_get_last() -> Result<(), Error> {
        let wrapper = TestCache::new(2);
        let cache = &wrapper.inner;
        let mut result: Vec<Value>;

        // 1 element added
        cache.set(&num(0), timeout())?;
        result = cache.get_last(10)?;
        assert_eq!(result, vec![num(0)]);

        // 2 elements added (1 current, 1 old)
        cache.set(&num(1), timeout())?;
        result = cache.get_last(10)?;
        assert_eq!(result, vec![num(1), num(0)]);

        // 3 elements added (1 current, 2 old)
        cache.set(&num(2), timeout())?;
        result = cache.get_last(10)?;
        assert_eq!(result, vec![num(2), num(1), num(0)]);

        // 4 elements added (1 current, 2 old, oldest one is pushed out)
        cache.set(&num(3), timeout())?;
        result = cache.get_last(10)?;
        assert_eq!(result, vec![num(3), num(2), num(1)]);

        result = cache.get_last(0)?;
        assert_eq!(result, vec![num(3)]);
        result = cache.get_last(1)?;
        assert_eq!(result, vec![num(3), num(2)]);
        result = cache.get_last(2)?;
        assert_eq!(result, vec![num(3), num(2), num(1)]);

        Ok(())
    }

    #[test]
    fn test_get_last_without_history() -> Result<(), Error> {
        let wrapper = TestCache::new(0);
        let cache = &wrapper.inner;
        let mut result: Vec<Value>;

        cache.set(&num(0), timeout())?;
        result = cache.get_last(10)?;
        assert_eq!(result, vec![num(0)]);

        cache.set(&num(1), timeout())?;
        result = cache.get_last(10)?;
        assert_eq!(result, vec![num(1)]);

        cache.set(&num(2), timeout())?;
        result = cache.get_last(10)?;
        assert_eq!(result, vec![num(2)]);

        result = cache.get_last(0)?;
        assert_eq!(result, vec![num(2)]);

        Ok(())
    }

    #[test]
    fn test_deletion() -> Result<(), Error> {
        let wrapper = TestCache::new(2);
        let cache = &wrapper.inner;

        cache.set(&Value::String("bar".into()), timeout())?;
        cache.set(&Value::String("baz".into()), timeout())?;
        cache.delete(timeout())?;

        let result: Vec<Value> = cache.get_last(2)?;
        assert!(result.is_empty());

        Ok(())
    }
}
