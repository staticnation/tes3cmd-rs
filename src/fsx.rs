//! Where a relative path is: against the folder [`crate::cli::run_in`] was handed, for the
//! run on this thread, else against the process's own folder, as the Perl's is.
//!
//! The command line runs in a folder of its own and never sets one, so everything reads as
//! it always did. A program that runs the port inside itself (Wraithguard's Tools page) has
//! many threads and one process folder: it names the folder per run instead, and plugin
//! names relative to it work as they do for `tes3cmd` run from that folder - the messages
//! still say them as given.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

thread_local! {
    static BASE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// The folder of this thread's run, when one was named.
pub fn base() -> Option<PathBuf> {
    BASE.with(|b| b.borrow().clone())
}

/// `p` resolved against this thread's run folder (unchanged when absolute, or none named).
pub fn at(p: impl AsRef<Path>) -> PathBuf {
    let p = p.as_ref();
    if p.is_absolute() {
        return p.to_path_buf();
    }
    match base() {
        Some(b) => b.join(p),
        None => p.to_path_buf(),
    }
}

/// Names `dir` as this thread's run folder until the guard is dropped (the previous one, if
/// any, comes back then - also when the run panics).
pub fn enter(dir: &Path) -> Guard {
    let was = BASE.with(|b| b.replace(Some(dir.to_path_buf())));
    Guard(was)
}

pub struct Guard(Option<PathBuf>);

impl Drop for Guard {
    fn drop(&mut self) {
        let was = self.0.take();
        BASE.with(|b| *b.borrow_mut() = was);
    }
}

// `std::fs`'s calls, each path resolved by `at`.
pub fn read(p: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    std::fs::read(at(p))
}
pub fn write(p: impl AsRef<Path>, data: impl AsRef<[u8]>) -> std::io::Result<()> {
    std::fs::write(at(p), data)
}
pub fn metadata(p: impl AsRef<Path>) -> std::io::Result<std::fs::Metadata> {
    std::fs::metadata(at(p))
}
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<u64> {
    std::fs::copy(at(from), at(to))
}
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::rename(at(from), at(to))
}
pub fn remove_file(p: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::remove_file(at(p))
}
pub fn create_dir_all(p: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(at(p))
}
pub fn read_dir(p: impl AsRef<Path>) -> std::io::Result<std::fs::ReadDir> {
    std::fs::read_dir(at(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_folder_for_this_thread_only() {
        assert_eq!(at("a.esp"), PathBuf::from("a.esp"));
        {
            let _g = enter(Path::new("/x/Data Files"));
            assert_eq!(at("a.esp"), PathBuf::from("/x/Data Files/a.esp"));
            assert_eq!(at("/abs/b.esp"), PathBuf::from("/abs/b.esp"));
            // Another thread has none.
            assert_eq!(
                std::thread::spawn(|| at("a.esp")).join().unwrap(),
                PathBuf::from("a.esp")
            );
        }
        assert_eq!(at("a.esp"), PathBuf::from("a.esp"));
    }
}
