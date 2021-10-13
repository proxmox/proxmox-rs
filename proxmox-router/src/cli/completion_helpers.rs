use std::path::PathBuf;
use std::collections::HashMap;

use nix::dir::Dir;
use nix::fcntl::{AtFlags, OFlag};
use nix::sys::stat::{fstatat, Mode};

pub fn complete_file_name(arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
    let mut result = vec![];

    let mut dirname = PathBuf::from(if arg.is_empty() { "./" } else { arg });

    let is_dir = match fstatat(libc::AT_FDCWD, &dirname, AtFlags::empty()) {
        Ok(stat) => (stat.st_mode & libc::S_IFMT) == libc::S_IFDIR,
        Err(_) => false,
    };

    if !is_dir {
        if let Some(parent) = dirname.parent() {
            dirname = parent.to_owned();
        }
    }

    let mut dir =
        match Dir::openat(libc::AT_FDCWD, &dirname, OFlag::O_DIRECTORY, Mode::empty()) {
            Ok(d) => d,
            Err(_) => return result,
        };

    for item in dir.iter() {
        if let Ok(entry) = item {
            if let Ok(name) = entry.file_name().to_str() {
                if name == "." || name == ".." {
                    continue;
                }
                let mut newpath = dirname.clone();
                newpath.push(name);

                if let Ok(stat) = fstatat(libc::AT_FDCWD, &newpath, AtFlags::empty())
                {
                    if (stat.st_mode & libc::S_IFMT) == libc::S_IFDIR {
                        newpath.push("");
                        if let Some(newpath) = newpath.to_str() {
                            result.push(newpath.to_owned());
                        }
                        continue;
                    }
                }
                if let Some(newpath) = newpath.to_str() {
                    result.push(newpath.to_owned());
                }
            }
        }
    }

    result
}
