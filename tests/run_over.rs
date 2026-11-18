//! `cli::run_over` answers what the program answers in a Morrowind folder holding every
//! plugin, with the masters read from elsewhere instead, and those masters never changed: the
//! same exit code, output and files in its own Data Files, for reading and writing commands,
//! the ones that write masters too (`fixit` synchronizes every active header).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const PLUGINS: [&str; 3] = ["mp_A.esp", "mp_B.esp", "mp_C.esp"];

fn src(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// A Morrowind folder: `Data Files` with the plugins, and the master too when `with_master`.
fn lay_out(root: &Path, with_master: bool) -> PathBuf {
    let _ = std::fs::remove_dir_all(root);
    let data = root.join("Data Files");
    std::fs::create_dir_all(&data).unwrap();
    let ini: String = ["Morrowind.esm", "mp_Master.esm"]
        .into_iter()
        .chain(PLUGINS)
        .enumerate()
        .map(|(i, p)| format!("GameFile{i}={p}\n"))
        .collect();
    std::fs::write(root.join("Morrowind.ini"), format!("[Game Files]\n{ini}")).unwrap();
    for p in PLUGINS {
        std::fs::copy(src(p), data.join(p)).unwrap();
    }
    if with_master {
        std::fs::copy(src("mp_Master.esm"), data.join("mp_Master.esm")).unwrap();
        std::fs::write(data.join("Morrowind.esm"), morrowind()).unwrap();
        set_date(&data.join("Morrowind.esm"));
    }
    data
}

/// A `Morrowind.esm` both `resetdates` (its date) and `fixit` (its record count, 0 here) write.
fn morrowind() -> Vec<u8> {
    let mut m = std::fs::read(src("mp_Master.esm")).unwrap();
    // TES3 header (16 bytes), HEDR (8): version f32, flags u32, author 32, description 256,
    // then the record count.
    let at = 16 + 8 + 4 + 4 + 32 + 256;
    m[at..at + 4].copy_from_slice(&0u32.to_le_bytes());
    m
}

fn set_date(p: &Path) {
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_500_000_000);
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn run_over_reads_masters_in_place_and_never_writes_them() {
    let base = std::env::temp_dir().join(format!("t3_run_over_{}", std::process::id()));
    let mods = base.join("mods");
    let cases: [&[&str]; 6] = [
        &["clean", "--replace", "mp_A.esp"],
        &["header", "--synchronize", "mp_B.esp"],
        &["multipatch", "--no-cache"],
        &["fixit"],
        &["resetdates"],
        &["delete", "--type", "GMST", "mp_C.esp"],
    ];
    for (i, args) in cases.iter().enumerate() {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let _ = std::fs::remove_dir_all(&mods);
        std::fs::create_dir_all(&mods).unwrap();
        let master = mods.join("mp_Master.esm");
        std::fs::copy(src("mp_Master.esm"), &master).unwrap();
        let mw = mods.join("Morrowind.esm");
        std::fs::write(&mw, morrowind()).unwrap();
        set_date(&mw);
        let state = |p: &Path| {
            (
                std::fs::read(p).unwrap(),
                std::fs::metadata(p).unwrap().modified().unwrap(),
            )
        };
        let before = [state(&master), state(&mw)];
        // The program, every plugin in its Data Files.
        let a = lay_out(&base.join(format!("{i}a")), true);
        let a_was = [
            state(&a.join("mp_Master.esm")),
            state(&a.join("Morrowind.esm")),
        ];
        let o = Command::new(env!("CARGO_BIN_EXE_tes3cmd"))
            .args(&args)
            .current_dir(&a)
            .env("PWD", &a)
            .output()
            .unwrap();
        // The library over an overlay, the master elsewhere.
        let b = lay_out(&base.join(format!("{i}b")), false);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let over = HashMap::from([
            ("mp_master.esm".to_string(), master.clone()),
            ("morrowind.esm".to_string(), mw.clone()),
        ]);
        let code = tes3cmd::cli::run_over(&b, over, &args, &mut out, &mut err);
        let (ra, rb) = (
            a.parent().unwrap().to_string_lossy().into_owned(),
            b.parent().unwrap().to_string_lossy().into_owned(),
        );
        // tes3cmd says a path with `/` on Windows too (`find_morrowind`): both spellings.
        let show = |v: &[u8]| {
            String::from_utf8_lossy(v)
                .replace(&rb, &ra)
                .replace(&rb.replace('\\', "/"), &ra.replace('\\', "/"))
        };
        assert_eq!(o.status.code().unwrap(), code, "{args:?}: exit code");
        assert_eq!(
            String::from_utf8_lossy(&o.stdout),
            show(&out),
            "{args:?}: standard output"
        );
        assert_eq!(
            String::from_utf8_lossy(&o.stderr),
            show(&err),
            "{args:?}: standard error"
        );
        let fa: Vec<_> = files(&a)
            .into_iter()
            .filter(|f| f.0 != "mp_Master.esm" && f.0 != "Morrowind.esm")
            .collect();
        let fb = files(&b);
        assert_eq!(
            fa.iter().map(|f| &f.0).collect::<Vec<_>>(),
            fb.iter().map(|f| &f.0).collect::<Vec<_>>(),
            "{args:?}: the files"
        );
        for (x, y) in fa.iter().zip(&fb) {
            assert_eq!(x.1, y.1, "{args:?}: {}", x.0);
        }
        // The masters where they are: their bytes and dates as they were, nothing new beside.
        assert_eq!(
            [state(&master), state(&mw)],
            before,
            "{args:?}: the masters as they were"
        );
        assert_eq!(
            std::fs::read_dir(&mods).unwrap().count(),
            2,
            "{args:?}: nothing new beside the masters"
        );
        // What the program did to a master in its Data Files, the overlay did to its copy.
        let copies: Vec<PathBuf> = std::fs::read_dir(b.parent().unwrap().join("tes3cmd/overlay"))
            .map(|r| {
                r.flatten()
                    .flat_map(|d| {
                        std::fs::read_dir(d.path())
                            .unwrap()
                            .flatten()
                            .map(|e| e.path())
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (k, name) in ["mp_Master.esm", "Morrowind.esm"].into_iter().enumerate() {
            let now = state(&a.join(name));
            if now != a_was[k] {
                let copy = copies
                    .iter()
                    .find(|c| c.file_name().unwrap() == name)
                    .unwrap_or_else(|| panic!("{args:?}: no copy of {name}"));
                let c = state(copy);
                assert_eq!(c.0, now.0, "{args:?}: the copy of {name}: its bytes");
                // A date set back (resetdates) is the same on the copy; one set to now cannot be.
                if now.1 < a_was[k].1 {
                    assert_eq!(c.1, now.1, "{args:?}: the copy of {name}: its date");
                }
                eprintln!("{args:?}: {name} written, on its copy");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}
