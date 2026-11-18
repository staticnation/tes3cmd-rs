//! `cli::run_in` answers what the program answers started in that folder: the same exit code,
//! standard output, standard error and files, for reading commands and writing ones.

use std::path::Path;
use std::process::Command;

fn lay_out(root: &Path) -> std::path::PathBuf {
    let _ = std::fs::remove_dir_all(root);
    let data = root.join("Data Files");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(
        root.join("Morrowind.ini"),
        "[Game Files]\nGameFile0=all_records.esp\n",
    )
    .unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/all_records.esp"),
        data.join("all_records.esp"),
    )
    .unwrap();
    data
}

fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir).unwrap().to_string_lossy().into_owned(),
                    std::fs::read(&p).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn run_in_is_the_program_started_there() {
    let base = std::env::temp_dir().join(format!("t3_run_in_{}", std::process::id()));
    let cases: [&[&str]; 4] = [
        &["dump", "--type", "TES3", "all_records.esp"],
        &["header", "--author", "Someone", "all_records.esp"],
        &["clean", "all_records.esp"],
        &["delete", "--type", "GMST", "all_records.esp"],
    ];
    for (i, args) in cases.iter().enumerate() {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        // The program, started in the folder.
        let a = lay_out(&base.join(format!("{i}a")));
        let o = Command::new(env!("CARGO_BIN_EXE_tes3cmd"))
            .args(&args)
            .current_dir(&a)
            .env("PWD", &a)
            .output()
            .unwrap();
        // The library, run from elsewhere with the folder named.
        let b = lay_out(&base.join(format!("{i}b")));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = tes3cmd::cli::run_in(&b, &args, &mut out, &mut err);
        let show = |v: &[u8]| {
            String::from_utf8_lossy(v)
                .replace(&b.to_string_lossy().to_string(), &a.to_string_lossy())
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
        let (ta, tb) = (tree(a.parent().unwrap()), tree(b.parent().unwrap()));
        assert_eq!(
            ta.iter().map(|x| &x.0).collect::<Vec<_>>(),
            tb.iter().map(|x| &x.0).collect::<Vec<_>>(),
            "{args:?}: the files left"
        );
        for (x, y) in ta.iter().zip(&tb) {
            if !x.0.contains(".bak") && !x.0.contains("tes3cmd") {
                assert_eq!(x.1, y.1, "{args:?}: {}", x.0);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}
