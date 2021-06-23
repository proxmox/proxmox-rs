use std::path::PathBuf;

use anyhow::{bail, format_err, Error};

use proxmox_apt::repositories::APTRepositoryFile;

#[test]
fn test_parse_write() -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let read_dir = test_dir.join("sources.list.d");
    let write_dir = test_dir.join("sources.list.d.actual");
    let expected_dir = test_dir.join("sources.list.d.expected");

    if write_dir.is_dir() {
        std::fs::remove_dir_all(&write_dir)
            .map_err(|err| format_err!("unable to remove dir {:?} - {}", write_dir, err))?;
    }

    std::fs::create_dir_all(&write_dir)
        .map_err(|err| format_err!("unable to create dir {:?} - {}", write_dir, err))?;

    let mut files = vec![];
    let mut errors = vec![];

    for entry in std::fs::read_dir(read_dir)? {
        let path = entry?.path();

        match APTRepositoryFile::new(&path)? {
            Some(mut file) => match file.parse() {
                Ok(()) => files.push(file),
                Err(err) => errors.push(err),
            },
            None => bail!("unexpected None for '{:?}'", path),
        }
    }

    assert!(errors.is_empty());

    for file in files.iter_mut() {
        let path = PathBuf::from(&file.path);
        let new_path = write_dir.join(path.file_name().unwrap());
        file.path = new_path.into_os_string().into_string().unwrap();
        file.digest = None;
        file.write()?;
    }

    let mut expected_count = 0;

    for entry in std::fs::read_dir(expected_dir)? {
        expected_count += 1;

        let expected_path = entry?.path();
        let actual_path = write_dir.join(expected_path.file_name().unwrap());

        let expected_contents = std::fs::read(&expected_path)
            .map_err(|err| format_err!("unable to read {:?} - {}", expected_path, err))?;

        let actual_contents = std::fs::read(&actual_path)
            .map_err(|err| format_err!("unable to read {:?} - {}", actual_path, err))?;

        assert_eq!(
            expected_contents, actual_contents,
            "Use\n\ndiff {:?} {:?}\n\nif you're not fluent in byte decimals",
            expected_path, actual_path
        );
    }

    let actual_count = std::fs::read_dir(write_dir)?.count();

    assert_eq!(expected_count, actual_count);

    Ok(())
}

#[test]
fn test_digest() -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let read_dir = test_dir.join("sources.list.d");
    let write_dir = test_dir.join("sources.list.d.digest");

    if write_dir.is_dir() {
        std::fs::remove_dir_all(&write_dir)
            .map_err(|err| format_err!("unable to remove dir {:?} - {}", write_dir, err))?;
    }

    std::fs::create_dir_all(&write_dir)
        .map_err(|err| format_err!("unable to create dir {:?} - {}", write_dir, err))?;

    let path = read_dir.join("standard.list");

    let mut file = APTRepositoryFile::new(&path)?.unwrap();
    file.parse()?;

    let new_path = write_dir.join(path.file_name().unwrap());
    file.path = new_path.clone().into_os_string().into_string().unwrap();

    let old_digest = file.digest.unwrap();

    // file does not exist yet...
    assert!(file.read_with_digest().is_err());
    assert!(file.write().is_err());

    // ...but it should work if there's no digest
    file.digest = None;
    file.write()?;

    // overwrite with old contents...
    std::fs::copy(path, new_path)?;

    // modify the repo
    let mut repo = file.repositories.first_mut().unwrap();
    repo.enabled = !repo.enabled;

    // ...then it should work
    file.digest = Some(old_digest);
    file.write()?;

    // expect a different digest, because the repo was modified
    let (_, new_digest) = file.read_with_digest()?;
    assert_ne!(old_digest, new_digest);

    assert!(file.write().is_err());

    Ok(())
}

#[test]
fn test_empty_write() -> Result<(), Error> {
    let test_dir = std::env::current_dir()?.join("tests");
    let read_dir = test_dir.join("sources.list.d");
    let write_dir = test_dir.join("sources.list.d.remove");

    if write_dir.is_dir() {
        std::fs::remove_dir_all(&write_dir)
            .map_err(|err| format_err!("unable to remove dir {:?} - {}", write_dir, err))?;
    }

    std::fs::create_dir_all(&write_dir)
        .map_err(|err| format_err!("unable to create dir {:?} - {}", write_dir, err))?;

    let path = read_dir.join("standard.list");

    let mut file = APTRepositoryFile::new(&path)?.unwrap();
    file.parse()?;

    let new_path = write_dir.join(path.file_name().unwrap());
    file.path = new_path.clone().into_os_string().into_string().unwrap();

    file.digest = None;

    file.write()?;

    assert!(file.exists());

    file.repositories.clear();

    file.write()?;

    assert!(!file.exists());

    Ok(())
}
