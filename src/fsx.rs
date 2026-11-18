//! Where a relative path is: against the folder [`crate::cli::run_in`] was handed, for the
//! run on this thread, else against the process's own folder, as the Perl's is.
//!
//! The command line runs in a folder of its own and never sets one, so everything reads as
//! it always did. A program that runs the port inside itself (Wraithguard's Tools page) has
//! many threads and one process folder: it names the folder per run instead, and plugin
//! names relative to it work as they do for `tes3cmd` run from that folder - the messages
//! still say them as given.

//!
//! An overlay ([`overlay`], [`crate::cli::run_over`]) goes one step further: plugins outside
//! the run's Morrowind folder are found by name in a list the caller gives (its own data
//! folders, OpenMW's VFS), read where they are, and never changed there. A write, a rename,
//! a removal or a new file time on one lands on a private copy inside the run's folder, which
//! the run then reads in its place.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

thread_local! {
    static BASE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

struct Overlay {
    /// The run's Morrowind folder: inside it, files are what they are.
    root: PathBuf,
    /// Plugins outside it by lower-case name (`datapath`'s second place to look).
    files: HashMap<String, PathBuf>,
    /// A file outside the root as this run has it: its private copy, or a path that does not
    /// exist for one the run removed or moved away.
    shadows: HashMap<PathBuf, PathBuf>,
}

/// A plain path: `.` parts dropped, `..` taken back (without touching the disk).
fn plain(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// Whether this run has an overlay over files outside `root`, and whether `p` is outside.
fn outside(p: &Path) -> bool {
    OVERLAY.with(|o| {
        o.borrow()
            .as_ref()
            .is_some_and(|o| !plain(p).starts_with(&o.root))
    })
}

/// The overlay's plugin of that name, when the caller named one (`Install::datapath`).
pub fn overlay_file(name: &str) -> Option<PathBuf> {
    OVERLAY.with(|o| {
        o.borrow()
            .as_ref()
            .and_then(|o| o.files.get(&name.to_lowercase()).cloned())
    })
}

/// Names `root` (the run's Morrowind folder) and the plugins outside it, by name, until the
/// guard is dropped; the private copies made meanwhile are under `root/tes3cmd/overlay`.
pub fn overlay(root: &Path, files: HashMap<String, PathBuf>) -> OverlayGuard {
    let files = files
        .into_iter()
        .map(|(k, v)| (k.to_lowercase(), plain(&v)))
        .collect();
    let was = OVERLAY.with(|o| {
        o.replace(Some(Overlay {
            root: plain(root),
            files,
            shadows: HashMap::new(),
        }))
    });
    OverlayGuard(was)
}

pub struct OverlayGuard(Option<Overlay>);

impl Drop for OverlayGuard {
    fn drop(&mut self) {
        let was = self.0.take();
        OVERLAY.with(|o| *o.borrow_mut() = was);
    }
}

/// The private copy of `real` (outside the root), made the first time: the file's bytes and
/// times when `keep`, else none yet. Answers where to write.
fn shadow(real: &Path, keep: bool) -> std::io::Result<PathBuf> {
    let real = plain(real);
    let (root, known) = OVERLAY.with(|o| {
        let o = o.borrow();
        let o = o.as_ref().expect("an overlay");
        (o.root.clone(), o.shadows.get(&real).cloned())
    });
    let dir = root.join("tes3cmd").join("overlay");
    let copy = match &known {
        Some(c) => c.clone(),
        None => {
            let n = OVERLAY.with(|o| o.borrow().as_ref().map_or(0, |o| o.shadows.len()));
            let name = real
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_default();
            dir.join(n.to_string()).join(name)
        }
    };
    if known.is_none() {
        std::fs::create_dir_all(copy.parent().unwrap_or(&dir))?;
        if keep && real.is_file() {
            std::fs::copy(&real, &copy)?;
            if let Ok(m) = std::fs::metadata(&real)
                && let Ok(f) = std::fs::File::options().write(true).open(&copy)
            {
                let mut t = std::fs::FileTimes::new();
                if let Ok(a) = m.accessed() {
                    t = t.set_accessed(a);
                }
                if let Ok(w) = m.modified() {
                    t = t.set_modified(w);
                }
                let _ = f.set_times(t);
            }
        }
        OVERLAY.with(|o| {
            if let Some(o) = o.borrow_mut().as_mut() {
                o.shadows.insert(real, copy.clone());
            }
        });
    }
    Ok(copy)
}

/// `p` as a file this run is about to change (open for writing, new times): its own path,
/// or under an overlay its private copy, made with the file's bytes when `keep`.
pub fn at_write(p: impl AsRef<Path>, keep: bool) -> std::io::Result<PathBuf> {
    let p = resolve(p.as_ref());
    if outside(&p) { shadow(&p, keep) } else { Ok(p) }
}

/// A file outside the root removed or moved away for the rest of this run.
fn gone(real: &Path) -> std::io::Result<()> {
    let copy = shadow(real, false)?;
    let _ = std::fs::remove_file(&copy);
    Ok(())
}

/// The folder of this thread's run, when one was named.
pub fn base() -> Option<PathBuf> {
    BASE.with(|b| b.borrow().clone())
}

fn resolve(p: &Path) -> PathBuf {
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let joined = match base() {
        Some(b) => b.join(p),
        None => p.to_path_buf(),
    };
    // A bare plugin name the run's folder does not have (fixit's clean and header over every
    // active plugin, the masters among them): the overlay's plugin of that name.
    if p.components().count() == 1
        && !joined.exists()
        && let Some(f) = p.to_str().and_then(overlay_file)
    {
        return f;
    }
    joined
}

/// `p` resolved against this thread's run folder (unchanged when absolute, or none named);
/// under an overlay, a file outside the root this run changed is its private copy.
pub fn at(p: impl AsRef<Path>) -> PathBuf {
    let p = resolve(p.as_ref());
    if outside(&p)
        && let Some(c) = OVERLAY.with(|o| {
            o.borrow()
                .as_ref()
                .and_then(|o| o.shadows.get(&plain(&p)).cloned())
        })
    {
        return c;
    }
    p
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

// `std::fs`'s calls, each path resolved by `at`; under an overlay, nothing outside the root
// is changed (`at_write`).
pub fn read(p: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    std::fs::read(at(p))
}
pub fn write(p: impl AsRef<Path>, data: impl AsRef<[u8]>) -> std::io::Result<()> {
    std::fs::write(at_write(p, false)?, data)
}
pub fn metadata(p: impl AsRef<Path>) -> std::io::Result<std::fs::Metadata> {
    std::fs::metadata(at(p))
}
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<u64> {
    std::fs::copy(at(from), at_write(to, false)?)
}
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
    let real = resolve(from.as_ref());
    if outside(&real) {
        // Moved off a file that is not the run's to move: copied, and gone for this run.
        let src = at(&real);
        if !src.exists() {
            return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        }
        std::fs::copy(&src, at_write(to, false)?)?;
        return gone(&real);
    }
    std::fs::rename(at(from), at_write(to, false)?)
}
pub fn remove_file(p: impl AsRef<Path>) -> std::io::Result<()> {
    let real = resolve(p.as_ref());
    if outside(&real) {
        if !at(&real).exists() {
            return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        }
        return gone(&real);
    }
    std::fs::remove_file(at(p))
}
pub fn create_dir_all(p: impl AsRef<Path>) -> std::io::Result<()> {
    let p = resolve(p.as_ref());
    // Outside an overlay's root no folder is made: what is written there goes to a copy.
    if outside(&p) {
        return Ok(());
    }
    std::fs::create_dir_all(p)
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

    #[test]
    fn an_overlay_never_changes_a_file_outside_the_run() {
        let dir = std::env::temp_dir().join(format!("t3_fsx_overlay_{}", std::process::id()));
        let (root, mods) = (dir.join("run"), dir.join("mods"));
        std::fs::create_dir_all(root.join("Data Files")).unwrap();
        std::fs::create_dir_all(&mods).unwrap();
        let m = mods.join("M.esm");
        std::fs::write(&m, b"master").unwrap();
        let files = HashMap::from([("m.esm".to_string(), m.clone())]);
        {
            let _b = enter(&root.join("Data Files"));
            let _o = overlay(&root, files);
            assert_eq!(overlay_file("M.ESM"), Some(m.clone()));
            assert_eq!(read(&m).unwrap(), b"master");
            write(&m, b"written").unwrap();
            assert_eq!(read(&m).unwrap(), b"written", "the run reads its copy");
            rename(&m, "moved.esm").unwrap();
            assert!(!at(&m).exists(), "moved away for the run");
            assert_eq!(read("moved.esm").unwrap(), b"written");
            write("inside.esp", b"x").unwrap();
            assert!(root.join("Data Files/inside.esp").is_file());
            create_dir_all(mods.join("new")).unwrap();
        }
        assert_eq!(
            std::fs::read(&m).unwrap(),
            b"master",
            "the real file is untouched"
        );
        assert!(!mods.join("new").exists());
        assert_eq!(at(&m), m, "no overlay after the guard");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
