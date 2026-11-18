//! tes3cmd's Perl and this port on the same plugins: the same standard output, standard error and
//! files left behind, byte for byte. Each case lays out a Morrowind folder (Morrowind.ini, "Data
//! Files" with the plugins), runs the Perl in one copy and the port in another, and compares.
//!
//! Needs Perl and tes3cmd's script (`TES3CMD_PL=path/to/tes3cmd`), or tes3cmd's own program
//! (`TES3CMD_EXE=path/to/tes3cmd.exe`): `cargo test --test vs_perl`. Without either the cases are
//! skipped (said on standard error).
//!
//! Real plugins: `T3_REAL_MW` the Morrowind folder (its Morrowind.ini and Data Files) and
//! `T3_REAL_PLUGINS` the plugins to clean, comma-separated: they and the masters they name are
//! copied into a folder of their own and cleaned by both (`real_plugins`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---- building plugins ----

fn sub(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut v = tag.to_vec();
    v.extend_from_slice(&(data.len() as u32).to_le_bytes());
    v.extend_from_slice(data);
    v
}
fn rec(tag: &[u8; 4], flags: u32, subs: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = subs.concat();
    let mut v = tag.to_vec();
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&flags.to_le_bytes());
    v.extend(body);
    v
}
fn z(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    v
}
fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn l32(v: &[i32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}
/// A plugin: its header (with masters and their sizes) and records.
fn plugin(masters: &[(&str, u64)], n_records: Option<u32>, records: &[Vec<u8>]) -> Vec<u8> {
    let mut hedr = f32s(&[1.3]);
    hedr.extend_from_slice(&0u32.to_le_bytes());
    let mut a = b"test author".to_vec();
    a.resize(32, 0);
    hedr.extend(a);
    let mut d = b"test description".to_vec();
    d.resize(256, 0);
    hedr.extend(d);
    hedr.extend_from_slice(&n_records.unwrap_or(records.len() as u32).to_le_bytes());
    let mut subs = vec![sub(b"HEDR", &hedr)];
    for (m, size) in masters {
        subs.push(sub(b"MAST", &z(m)));
        subs.push(sub(b"DATA", &size.to_le_bytes()));
    }
    let mut out = rec(b"TES3", 0, &subs);
    for r in records {
        out.extend_from_slice(r);
    }
    out
}
fn stat(id: &str, model: &str, flags: u32) -> Vec<u8> {
    rec(
        b"STAT",
        flags,
        &[sub(b"NAME", &z(id)), sub(b"MODL", &z(model))],
    )
}
fn gmst(id: &str, value: (&[u8; 4], Vec<u8>)) -> Vec<u8> {
    rec(
        b"GMST",
        0,
        &[sub(b"NAME", id.as_bytes()), sub(value.0, &value.1)],
    )
}
fn frmr(idx: u32, name: &str, pos: [f32; 6], extra: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut v = vec![sub(b"FRMR", &idx.to_le_bytes()), sub(b"NAME", &z(name))];
    v.extend(extra.iter().cloned());
    v.push(sub(b"DATA", &f32s(&pos)));
    v
}
fn interior(name: &str, extra: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut v = vec![
        sub(b"NAME", &z(name)),
        sub(b"DATA", &[l32(&[1, 0]), f32s(&[0.5])].concat()),
    ];
    v.extend(extra.iter().cloned());
    v
}
fn exterior(name: &str, x: i32, y: i32, region: Option<&str>) -> Vec<Vec<u8>> {
    let mut v = vec![sub(b"NAME", &z(name)), sub(b"DATA", &l32(&[2, x, y]))];
    if let Some(r) = region {
        v.push(sub(b"RGNN", &z(r)));
    }
    v
}
fn pgrd(name: &str, x: i32, y: i32) -> Vec<u8> {
    rec(
        b"PGRD",
        0,
        &[
            sub(b"DATA", &[l32(&[x, y]), vec![0, 0, 1, 0]].concat()),
            sub(b"NAME", &z(name)),
            sub(b"PGRP", &[0u8; 16]),
        ],
    )
}

// ---- running both ----

struct Case {
    name: &'static str,
    files: Vec<(&'static str, Vec<u8>)>,
    args: Vec<&'static str>,
}

/// How tes3cmd itself is run: its program, or Perl with its script.
fn tes3cmd() -> Option<Command> {
    if let Some(exe) = std::env::var_os("TES3CMD_EXE")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(Command::new(exe));
    }
    let p = PathBuf::from(std::env::var("TES3CMD_PL").ok()?);
    let perl_ok = Command::new("perl")
        .arg("-v")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    (p.is_file() && perl_ok).then(|| {
        let mut c = Command::new("perl");
        c.arg(p);
        c
    })
}

/// The folder laid out for a case; answers its Data Files.
fn lay_out(root: &Path, case: &Case) -> PathBuf {
    let _ = std::fs::remove_dir_all(root);
    let data = root.join("Data Files");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(
        root.join("Morrowind.ini"),
        "[Game Files]\r\nGameFile0=Morrowind.esm\r\n",
    )
    .unwrap();
    // A case's own Morrowind.ini goes beside Data Files; each plugin is dated a day after the
    // one before it, so the load order (by date) is the same in both copies.
    let day = std::time::Duration::from_secs(86400);
    let mut t = std::time::UNIX_EPOCH + day * 18262;
    for (n, b) in &case.files {
        let path = if *n == "Morrowind.ini" {
            root.join(n)
        } else {
            data.join(n)
        };
        std::fs::write(&path, b).unwrap();
        if n != &"Morrowind.ini" {
            let f = std::fs::File::options().write(true).open(&path).unwrap();
            f.set_modified(t).unwrap();
            t += day;
        }
    }
    data
}

/// Every file under a folder, by its path from it.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let p = e.path();
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if p.is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(base, &p, out);
            } else if !rel.ends_with(".cache") {
                // (tes3cmd's master cache is Perl's Storable format; the port keeps none yet.)
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    walk(root, root, &mut out);
    out
}

/// Perl's `croak`/`die` place (" at FILE line N.") taken off, so the messages compare.
fn unplace(s: &str) -> String {
    let mut out = String::new();
    for line in s.split_inclusive('\n') {
        let t = match line.rfind(" at ") {
            Some(k) if line[k..].contains(" line ") && line.trim_end().ends_with('.') => {
                let nl = if line.ends_with('\n') { "\n" } else { "" };
                format!("{}{nl}", &line[..k])
            }
            _ => line.to_string(),
        };
        out.push_str(&t);
    }
    out
}

/// `sort_err`: standard error compared as a set of lines, for a command that walks Perl hashes
/// (whose order changes from run to run) while it warns.
fn run_both_with(case: &Case, sort_err: bool) -> Option<(String, String)> {
    let mut theirs = tes3cmd()?;
    let base = std::env::temp_dir().join(format!("t3rs-{}-{}", case.name, std::process::id()));
    let (pd, rd) = (base.join("perl"), base.join("rust"));
    let pdata = lay_out(&pd, case);
    let rdata = lay_out(&rd, case);
    let perl = theirs
        .args(&case.args)
        .current_dir(&pdata)
        .env("PWD", &pdata)
        .output()
        .unwrap();
    let rust = Command::new(env!("CARGO_BIN_EXE_tes3cmd"))
        .args(&case.args)
        .current_dir(&rdata)
        .env("PWD", &rdata)
        .output()
        .unwrap();
    // Each run's own folder, named the same.
    let root = |s: &[u8], dir: &Path| {
        String::from_utf8_lossy(s).replace(&dir.to_string_lossy().into_owned(), "<ROOT>")
    };
    let (po, ro) = (root(&perl.stdout, &pd), root(&rust.stdout, &rd));
    let (pe, re) = (
        unplace(&root(&perl.stderr, &pd)),
        unplace(&root(&rust.stderr, &rd)),
    );
    let (pe, re) = if sort_err {
        let sorted = |s: &str| {
            let mut l: Vec<&str> = s.lines().collect();
            l.sort();
            l.join("\n")
        };
        (sorted(&pe), sorted(&re))
    } else {
        (pe, re)
    };
    let mut report = String::new();
    if po != ro {
        report += &format!("standard output differs:\n--- perl\n{po}\n--- port\n{ro}\n");
    }
    if pe != re {
        report += &format!("standard error differs:\n--- perl\n{pe}\n--- port\n{re}\n");
    }
    if perl.status.code() != rust.status.code() {
        report += &format!(
            "exit codes differ: perl {:?}, port {:?}\n",
            perl.status.code(),
            rust.status.code()
        );
    }
    let (pt, rt) = (tree(&pd), tree(&rd));
    for k in pt
        .keys()
        .chain(rt.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (pt.get(k), rt.get(k)) {
            (Some(a), Some(b)) if a == b => {}
            (Some(a), Some(b)) => {
                let at = a
                    .iter()
                    .zip(b)
                    .position(|(x, y)| x != y)
                    .unwrap_or(a.len().min(b.len()));
                report += &format!(
                    "{k}: differs ({} bytes against {}, first at {at})\n",
                    a.len(),
                    b.len()
                );
            }
            (a, b) => {
                report += &format!(
                    "{k}: perl {}, port {}\n",
                    if a.is_some() { "has it" } else { "has not" },
                    if b.is_some() { "has it" } else { "has not" }
                )
            }
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    Some((report, po))
}

fn check(case: Case) {
    check_with(case, false)
}

fn check_with(case: Case, sort_err: bool) {
    match run_both_with(&case, sort_err) {
        None => eprintln!(
            "skipped {}: set TES3CMD_PL to tes3cmd's Perl script (and have perl), or TES3CMD_EXE to tes3cmd's program",
            case.name
        ),
        Some((report, out)) => {
            assert!(report.is_empty(), "{}: {report}", case.name);
            eprintln!(
                "{}: the same ({} lines of output)",
                case.name,
                out.lines().count()
            );
            if std::env::var("T3_SHOW").is_ok() {
                eprintln!("{out}");
            }
        }
    }
}

// ---- the cases ----

fn master_esm() -> Vec<u8> {
    plugin(
        &[],
        None,
        &[
            stat("rock_01", "r\\rock.nif", 0),
            stat("rock_02", "r\\rock2.nif", 0),
            stat("Tree_A", "t\\tree.nif", 0),
            gmst("fWerewolfSpeed", (b"FLTV", f32s(&[150.0]))),
            rec(
                b"CELL",
                0,
                &[
                    interior(
                        "Test Room",
                        &[
                            sub(b"WHGT", &f32s(&[-128.0])),
                            sub(b"AMBI", &l32(&[0x101010, 0x202020, 0x303030, 0])),
                        ],
                    ),
                    frmr(1, "rock_01", [1.0, 2.0, 3.0, 0.0, 0.0, 0.0], &[]),
                    frmr(2, "rock_02", [4.0, 5.0, 6.0, 0.0, 0.0, 0.0], &[]),
                    frmr(
                        3,
                        "Tree_A",
                        [7.0, 8.0, 9.0, 0.0, 0.0, 0.0],
                        &[sub(b"XSCL", &f32s(&[1.5]))],
                    ),
                ]
                .concat(),
            ),
            rec(
                b"CELL",
                0,
                &interior(
                    "Old Room",
                    &[
                        sub(b"INTV", &l32(&[-50])),
                        sub(b"AMBI", &l32(&[1, 2, 3, 0])),
                    ],
                ),
            ),
            rec(
                b"CELL",
                0,
                &exterior("", 2, 3, Some("Ascadian Isles Region")),
            ),
            rec(
                b"CELL",
                0,
                &exterior("Balmora", 4, 5, Some("West Gash Region")),
            ),
            pgrd("Test Room", 0, 0),
            pgrd("wilds", 2, 3),
        ],
    )
}

fn with_master(name: &'static str, records: Vec<Vec<u8>>, args: Vec<&'static str>) -> Case {
    let m = master_esm();
    let size = m.len() as u64;
    Case {
        name,
        files: vec![
            ("Test.esm", m),
            ("Plug.esp", plugin(&[("Test.esm", size)], None, &records)),
        ],
        args,
    }
}

#[test]
fn evil_gmsts() {
    check(with_master(
        "gmsts",
        vec![
            gmst("fWerewolfSpeed", (b"FLTV", f32s(&[150.0]))),
            gmst("fWerewolfRunMult", (b"FLTV", f32s(&[1.5]))),
            gmst("sMaxSale", (b"STRV", b"Max Sale".to_vec())),
            gmst("iWerewolfBounty", (b"INTV", l32(&[999]))),
            gmst("sProfitValue", (b"STRV", b"Not the evil one".to_vec())),
        ],
        vec!["clean", "Plug.esp"],
    ));
}

#[test]
fn duplicate_records() {
    check(with_master(
        "dups",
        vec![
            stat("rock_01", "r\\rock.nif", 0),
            stat("rock_02", "r\\other.nif", 0),
            stat("tree_a", "t\\tree.nif", 0),
            stat("Tree_A", "t\\tree.nif", 0x400),
            stat("new_one", "n.nif", 0),
        ],
        vec!["clean", "Plug.esp"],
    ));
}

#[test]
fn cells_instances_and_params() {
    check(with_master(
        "cells",
        vec![rec(
            b"CELL",
            0,
            &[
                interior(
                    "Test Room",
                    &[
                        sub(b"WHGT", &f32s(&[-128.0])),
                        sub(b"AMBI", &l32(&[0x101010, 0x202020, 0x303030, 0])),
                    ],
                ),
                vec![
                    sub(b"FRMR", &(1u32 | (1 << 24)).to_le_bytes()),
                    sub(b"NAME", &z("rock_01")),
                    sub(b"DATA", &f32s(&[1.0, 2.0, 3.0, 0.0, 0.0, 0.0])),
                    sub(b"FRMR", &(2u32 | (1 << 24)).to_le_bytes()),
                    sub(b"NAME", &z("rock_02")),
                    sub(b"DATA", &f32s(&[40.0, 5.0, 6.0, 0.0, 0.0, 0.0])),
                    sub(b"NAM0", &1u32.to_le_bytes()),
                    sub(b"FRMR", &3u32.to_le_bytes()),
                    sub(b"NAME", &z("Tree_A")),
                    sub(b"XSCL", &f32s(&[1.5])),
                    sub(b"DATA", &f32s(&[7.0, 8.0, 9.0, 0.0, 0.0, 0.0])),
                    sub(b"FRMR", &4u32.to_le_bytes()),
                    sub(b"NAME", &z("new_thing")),
                    sub(b"CNAM", &z("Hlaalu")),
                    sub(b"INTV", &l32(&[100])),
                    sub(b"DATA", &f32s(&[1.0, 1.0, 1.0, 0.0, 0.0, 0.0])),
                ],
            ]
            .concat(),
        )],
        vec!["clean", "Plug.esp"],
    ));
}

#[test]
fn junk_cells_and_water() {
    check(with_master(
        "junk",
        vec![
            rec(
                b"CELL",
                0,
                &exterior("", 2, 3, Some("Ascadian Isles Region")),
            ),
            rec(
                b"CELL",
                0,
                &exterior("Balmora", 4, 5, Some("Bitter Coast Region")),
            ),
            rec(
                b"CELL",
                0,
                &interior(
                    "Old Room",
                    &[
                        sub(b"WHGT", &f32s(&[-50.0])),
                        sub(b"AMBI", &l32(&[1, 2, 3, 0])),
                    ],
                ),
            ),
            rec(
                b"CELL",
                0x400,
                &interior("Old Room", &[sub(b"AMBI", &l32(&[9, 9, 9, 0]))]),
            ),
        ],
        vec!["clean", "Plug.esp"],
    ));
}

#[test]
fn path_grids_by_letter_case() {
    check(with_master(
        "pgrd",
        vec![
            pgrd("Test Room", 0, 0),
            pgrd("wilds", 2, 3),
            pgrd("test room", 0, 0),
        ],
        vec!["clean", "Plug.esp"],
    ));
}

#[test]
fn a_gmst_whose_id_dies() {
    check(with_master(
        "gmst_dies",
        vec![
            gmst("xFoo", (b"FLTV", f32s(&[1.0]))),
            stat("new_one", "n.nif", 0),
        ],
        vec!["clean", "Plug.esp"],
    ));
}

#[test]
fn nothing_to_clean_and_the_skips() {
    check(with_master(
        "nothing",
        vec![stat("new_one", "n.nif", 0)],
        vec![
            "clean",
            "Plug.esp",
            "Morrowind.esm",
            "Plug~2.esp",
            "save.ess",
        ],
    ));
}

#[test]
fn overwrite_with_hidden_backups() {
    let mut c = with_master(
        "overwrite",
        vec![
            stat("rock_01", "r\\rock.nif", 0),
            stat("new_one", "n.nif", 0),
        ],
        vec!["clean", "--overwrite", "--hide-backups", "Plug.esp"],
    );
    c.files.push(("Plug~1.esp", b"an old backup".to_vec()));
    check(c);
}

#[test]
fn options_chosen_and_abbreviated() {
    check(with_master(
        "options",
        vec![
            stat("rock_01", "r\\rock.nif", 0),
            gmst("fWerewolfSpeed", (b"FLTV", f32s(&[150.0]))),
        ],
        vec!["clean", "--gm", "-dups", "Plug.esp"],
    ));
}

#[test]
fn several_plugins_ignored_and_globbed() {
    let m = master_esm();
    let size = m.len() as u64;
    let p = |r: Vec<Vec<u8>>| plugin(&[("Test.esm", size)], None, &r);
    check(Case {
        name: "several",
        files: vec![
            ("Test.esm", m),
            ("A.esp", p(vec![stat("rock_01", "r\\rock.nif", 0)])),
            (
                "B.esp",
                p(vec![gmst("sMaxSale", (b"STRV", b"Max Sale".to_vec()))]),
            ),
            ("C Two.esp", p(vec![stat("rock_02", "r\\rock2.nif", 0)])),
        ],
        args: vec![
            "clean",
            "--ignore-plugin",
            "b.esp",
            "*.esp",
            "--ignore-plugin=A.ESP",
        ],
    });
}

#[test]
fn persistent_objects_moved_refs_and_odd_subrecords() {
    let m = master_esm();
    let size = m.len() as u64;
    let cell = [
        interior(
            "Test Room",
            &[
                sub(b"CNAM", &z("odd")),
                sub(b"AMBI", &l32(&[0x101010, 0x202020, 0x303030, 0])),
            ],
        ),
        vec![
            sub(b"MVRF", &2u32.to_le_bytes()),
            sub(b"CNDT", &l32(&[1, 1])),
            sub(b"FRMR", &(1u32 | (1 << 24)).to_le_bytes()),
            sub(b"NAME", &z("rock_01")),
            sub(b"DATA", &f32s(&[1.0, 2.0, 3.0, 0.0, 0.0, 0.0])),
            sub(b"FRMR", &(3u32 | (1 << 24)).to_le_bytes()),
            sub(b"NAME", &z("Tree_A")),
            sub(b"XSCL", &f32s(&[1.5])),
            sub(b"DATA", &f32s(&[7.0, 8.0, 9.0, 0.0, 0.0, 0.0])),
            sub(b"FRMR", &9u32.to_le_bytes()),
            sub(b"NAME", &z("light_x")),
            sub(b"INTV", &f32s(&[123.5])),
            sub(b"QQQQ", b"unknown"),
            sub(b"DATA", &l32(&[1, 2])),
        ],
    ]
    .concat();
    check(Case {
        name: "persistent",
        files: vec![
            ("Test.esm", m),
            (
                "Plug.esp",
                plugin(
                    &[("Test.esm", size)],
                    None,
                    &[
                        stat("rock_01", "r\\rock.nif", 0x400),
                        stat("tree_a", "t\\x.nif", 0x400),
                        rec(b"CELL", 0x2000, &cell),
                    ],
                ),
            ),
        ],
        args: vec!["clean", "Plug.esp"],
    });
}

#[test]
fn broken_and_unknown_records_and_a_missing_master() {
    let m = master_esm();
    let size = m.len() as u64;
    let mut cut = plugin(
        &[("Test.esm", size)],
        None,
        &[
            stat("rock_01", "r\\rock.nif", 0),
            rec(b"ZZZZ", 0, &[sub(b"NAME", &z("what"))]),
        ],
    );
    cut.extend_from_slice(&rec(b"STAT", 0, &[sub(b"NAME", &z("cut"))])[..10]);
    check(Case {
        name: "broken",
        files: vec![
            ("Test.esm", m),
            ("Cut.esp", cut),
            (
                "Lost.esp",
                plugin(
                    &[("Nowhere.esm", 10), ("Test.esm", size)],
                    None,
                    &[stat("rock_01", "r\\rock.nif", 0)],
                ),
            ),
            (
                "Short.esp",
                plugin(
                    &[("Test.esm", size)],
                    None,
                    &[rec(
                        b"STAT",
                        0,
                        &[sub(b"NAME", &z("rock_01")), b"MO".to_vec()],
                    )],
                ),
            ),
        ],
        args: vec!["clean", "Cut.esp", "Lost.esp", "Short.esp"],
    });
}

#[test]
fn output_dir_verbose_and_a_subfolder() {
    let m = master_esm();
    let size = m.len() as u64;
    check(Case {
        name: "outdir",
        files: vec![
            ("Test.esm", m),
            (
                "Plug.esp",
                plugin(
                    &[("Test.esm", size)],
                    Some(99),
                    &[stat("rock_01", "r\\rock.nif", 0), stat("keep", "k.nif", 0)],
                ),
            ),
        ],
        args: vec!["clean", "--output-dir", "-v", "Plug.esp"],
    });
}

/// Plugins of a real install, cleaned by both (see the top of the file).
#[test]
fn real_plugins() {
    let (Some(mw), Some(list)) = (
        std::env::var_os("T3_REAL_MW").map(PathBuf::from),
        std::env::var("T3_REAL_PLUGINS").ok(),
    ) else {
        eprintln!("skipped real_plugins: set T3_REAL_MW and T3_REAL_PLUGINS");
        return;
    };
    let data = std::fs::read_dir(&mw)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("data files"))
        })
        .expect("Data Files");
    let find = |name: &str| {
        std::fs::read_dir(&data)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
            })
    };
    let plugins: Vec<String> = list
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let mut files: Vec<(&'static str, Vec<u8>)> = Vec::new();
    let mut want: Vec<String> = plugins.clone();
    let mut i = 0;
    while i < want.len() {
        let name = want[i].clone();
        i += 1;
        let Some(path) = find(&name) else {
            panic!("{name} is not in {}", data.display())
        };
        let bytes = std::fs::read(&path).unwrap();
        // The masters it names (MAST in its header).
        let hlen = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let mut p = 16;
        while p + 8 <= 16 + hlen {
            let tag = &bytes[p..p + 4];
            let len = u32::from_le_bytes(bytes[p + 4..p + 8].try_into().unwrap()) as usize;
            if tag == b"MAST" {
                let m = String::from_utf8_lossy(&bytes[p + 8..p + 8 + len])
                    .trim_end_matches('\0')
                    .to_string();
                if !want.iter().any(|w| w.eq_ignore_ascii_case(&m)) {
                    want.push(m);
                }
            }
            p += 8 + len;
        }
        let file_name: &'static str = Box::leak(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
                .into_boxed_str(),
        );
        files.push((file_name, bytes));
    }
    let args: Vec<&'static str> = std::iter::once("clean")
        .chain(
            plugins
                .iter()
                .map(|p| &*Box::leak(p.clone().into_boxed_str())),
        )
        .collect();
    check(Case {
        name: "real",
        files,
        args,
    });
}

// ---- the other commands ----

fn dial(id: &str, ty: u8) -> Vec<u8> {
    rec(b"DIAL", 0, &[sub(b"NAME", &z(id)), sub(b"DATA", &[ty])])
}

#[test]
fn header_shown_and_changed() {
    for (name, args) in [
        ("header-plain", vec!["header", "Plug.esp", "Test.esm"]),
        (
            "header-multiline",
            vec!["header", "--multiline", "Plug.esp"],
        ),
        ("header-verbose", vec!["header", "-v", "Plug.esp"]),
        (
            "header-author",
            vec!["header", "--author", "Somebody", "Plug.esp"],
        ),
        (
            "header-author-plus",
            vec![
                "header",
                "--author",
                "+ and friends",
                "--description",
                "Line one\\nLine two",
                "Plug.esp",
            ],
        ),
        (
            "header-count",
            vec!["header", "--update-record-count", "Plug.esp"],
        ),
        (
            "header-sync",
            vec!["header", "--synchronize", "--hide-backups", "Plug.esp"],
        ),
        (
            "header-backup-skipped",
            vec!["header", "--update-masters", "Plug~1.esp"],
        ),
    ] {
        let m = master_esm();
        // A wrong record count and a wrong master size, for the updates to find.
        let p = plugin(
            &[("Test.esm", 1234)],
            Some(9),
            &[stat("rock_09", "r\\rock9.nif", 0)],
        );
        check(Case {
            name,
            files: vec![("Test.esm", m), ("Plug.esp", p.clone()), ("Plug~1.esp", p)],
            args,
        });
    }
}

#[test]
fn esp_and_esm() {
    for (name, args) in [
        ("esm-from-esp", vec!["esm", "Plug.esp"]),
        ("esp-from-esm", vec!["esp", "Test.esm"]),
        ("esm-exists", vec!["esm", "Other.esp"]),
        (
            "esm-exists-overwrite",
            vec!["esm", "--overwrite", "Other.esp"],
        ),
        ("esm-wrong-input", vec!["esm", "Test.esm"]),
    ] {
        let mut c = with_master(name, vec![stat("rock_09", "r\\rock9.nif", 0)], args);
        c.files.push(("Other.esp", plugin(&[], None, &[])));
        c.files.push(("Other.esm", plugin(&[], None, &[])));
        check(c);
    }
}

#[test]
fn common_records() {
    check(with_master(
        "common",
        vec![
            stat("rock_01", "r\\rock.nif", 0),
            stat("New_Thing", "n.nif", 0),
            gmst("fWerewolfSpeed", (b"FLTV", f32s(&[1.0]))),
        ],
        vec!["common", "Plug.esp", "Test.esm"],
    ));
}

#[test]
fn active_list() {
    let mut c = with_master(
        "active-list",
        vec![stat("rock_09", "r\\rock9.nif", 0)],
        vec!["active"],
    );
    c.files.push(("Morrowind.esm", plugin(&[], None, &[])));
    check(c);
}

/// `active --on` / `--off`: tes3cmd 0.40-PRE-RELEASE-2 dies in `write_gamefiles` ("Undefined
/// subroutine &TES3::Util::move"), leaving `Morrowind.ini.tmp` and the ini as it was. The port
/// does what the command says (README, "Where it differs"), so it is checked on its own.
#[test]
fn active_on_and_off() {
    let root = std::env::temp_dir().join(format!("t3rs-active-{}", std::process::id()));
    let mut c = with_master("active-onoff", vec![], vec![]);
    c.files.push(("Morrowind.esm", plugin(&[], None, &[])));
    let data = lay_out(&root, &c);
    std::fs::write(root.join("Morrowind.ini"), "[General]\r\nX=1\r\n[Game Files]\r\nGameFile0=Morrowind.esm\r\n[Archives]\r\nArchive 0=a.bsa\r\n").unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_tes3cmd"))
            .args(args)
            .current_dir(&data)
            .env("PWD", &data)
            .output()
            .unwrap();
        (
            String::from_utf8_lossy(&o.stdout).into_owned(),
            o.status.code(),
        )
    };
    let ini = || std::fs::read_to_string(root.join("Morrowind.ini")).unwrap();
    assert_eq!(
        run(&["active", "--on", "Plug.esp", "Test.esm"]),
        ("ACTIVATED: Plug.esp\nACTIVATED: Test.esm\n".into(), Some(0))
    );
    assert_eq!(
        ini(),
        "[General]\r\nX=1\r\n[Game Files]\r\nGameFile0=Morrowind.esm\r\nGameFile1=Plug.esp\r\nGameFile2=Test.esm\r\n[Archives]\r\nArchive 0=a.bsa\r\n"
    );
    assert!(root.join("Morrowind.ini.old").is_file());
    assert_eq!(
        run(&["active", "--off", "plug.esp", "Nothing.esp"]),
        ("DEACTIVATED: plug.esp\n".into(), Some(0))
    );
    assert_eq!(
        ini(),
        "[General]\r\nX=1\r\n[Game Files]\r\nGameFile0=Morrowind.esm\r\nGameFile1=Test.esm\r\n[Archives]\r\nArchive 0=a.bsa\r\n"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn overdial_topics() {
    let m = master_esm();
    let size = m.len() as u64;
    check(Case {
        name: "overdial",
        files: vec![
            ("Test.esm", m),
            (
                "A.esp",
                plugin(
                    &[("Test.esm", size)],
                    None,
                    &[dial("Balmora", 0), dial("Greeting 1", 1)],
                ),
            ),
            (
                "B.esp",
                plugin(
                    &[("Test.esm", size)],
                    None,
                    &[dial("travel to Balmora", 0), dial("Balmorans", 0)],
                ),
            ),
        ],
        args: vec!["overdial", "A.esp", "B.esp"],
    });
}

#[test]
fn recover_bad_data() {
    let good = stat("rock_09", "r\\rock9.nif", 0);
    let mut body = good.clone();
    body.extend_from_slice(b"\x01\x02junkjunk\xff\xff\xff\xff\x00");
    body.extend(stat("rock_10", "r\\rock10.nif", 0));
    body.extend_from_slice(b"\x05\x05\x05more junk");
    body.extend(gmst("sMaxSale", (b"STRV", b"Max".to_vec())));
    let mut p = plugin(&[], Some(3), &[]);
    p.extend(body);
    for (name, args) in [
        ("recover", vec!["recover", "Bad.esp"]),
        ("recover-clean", vec!["recover", "Good.esp"]),
    ] {
        check(Case {
            name,
            files: vec![
                ("Bad.esp", p.clone()),
                ("Good.esp", plugin(&[], None, std::slice::from_ref(&good))),
            ],
            args,
        });
    }
}

#[test]
fn resetdates_bethesda_files() {
    let mut c = with_master("resetdates", vec![], vec!["resetdates"]);
    c.files.push(("Morrowind.esm", plugin(&[], None, &[])));
    c.files.push(("Tribunal.bsa", b"BSA".to_vec()));
    check(c);
}

/// The fixtures `tools/gen_all_records.pl` writes: one record of every type tes3cmd knows,
/// each subrecord packed by tes3cmd's own encoders (`odd_records`: every lookup out of its
/// table, for the warnings and errors tes3cmd gives then).
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

#[test]
fn dump_every_record_type() {
    for (name, file) in [
        ("dump-all", "all_records.esp"),
        ("dump-odd", "odd_records.esp"),
    ] {
        check(Case {
            name,
            files: vec![(file, fixture(file))],
            args: vec!["dump", file],
        });
    }
}

#[test]
fn dump_selectors() {
    let cases: &[(&str, &[&str])] = &[
        (
            "dump-list",
            &["dump", "--list", "all_records.esp", "odd_records.esp"],
        ),
        (
            "dump-type",
            &[
                "dump",
                "--type",
                "cell",
                "-B",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
        (
            "dump-npc",
            &[
                "dump",
                "--type",
                "npc",
                "--type",
                "CREA.NPDT",
                "all_records.esp",
            ],
        ),
        ("dump-id", &["dump", "--id", "^id", "all_records.esp"]),
        (
            "dump-exact-id",
            &[
                "dump",
                "--exact-id",
                "balmora",
                "--exact-id",
                "Test.Room",
                "all_records.esp",
            ],
        ),
        ("dump-flag", &["dump", "--flag", "p", "all_records.esp"]),
        (
            "dump-flag-number",
            &["dump", "--flag", "0x400", "--list", "all_records.esp"],
        ),
        (
            "dump-match",
            &["dump", "--match", "Rank:", "all_records.esp"],
        ),
        (
            "dump-no-match",
            &["dump", "-M", "id", "--list", "all_records.esp"],
        ),
        ("dump-interior", &["dump", "--interior", "all_records.esp"]),
        (
            "dump-exterior",
            &["dump", "--exterior", "--type", "BOOK", "all_records.esp"],
        ),
        (
            "dump-instances",
            &["dump", "--instance-match", "door", "all_records.esp"],
        ),
        (
            "dump-no-instances",
            &["dump", "--instance-no-match", "rock", "all_records.esp"],
        ),
        (
            "dump-verbose",
            &["dump", "-v", "--type", "CELL", "all_records.esp"],
        ),
        (
            "dump-bad-type",
            &["dump", "--type", "ABC", "all_records.esp"],
        ),
    ];
    for (name, args) in cases {
        check(Case {
            name,
            files: vec![
                ("all_records.esp", fixture("all_records.esp")),
                ("odd_records.esp", fixture("odd_records.esp")),
            ],
            args: args.to_vec(),
        });
    }
}

/// tes3cmd's `unknown_data` dies on 2 or 3 bytes ("Not a SCALAR reference"): the plugin ends
/// there, the records after it unprinted.
#[test]
fn dump_dies_where_tes3cmd_does() {
    let p = plugin(
        &[],
        None,
        &[
            stat("rock_01", "r.nif", 0),
            rec(
                b"LAND",
                0,
                &[sub(b"INTV", &l32(&[1, 2])), sub(b"DATA", &[1, 2])],
            ),
            stat("rock_02", "r2.nif", 0),
        ],
    );
    check(Case {
        name: "dump-dies",
        files: vec![("Die.esp", p)],
        args: vec!["dump", "Die.esp"],
    });
}

fn both_fixtures(name: &'static str, args: &[&'static str]) -> Case {
    Case {
        name,
        files: vec![
            ("all_records.esp", fixture("all_records.esp")),
            ("odd_records.esp", fixture("odd_records.esp")),
        ],
        args: args.to_vec(),
    }
}

#[test]
fn delete_records_subrecords_and_instances() {
    let cases: &[(&str, &[&str])] = &[
        (
            "delete-type",
            &["delete", "--type", "BOOK", "all_records.esp"],
        ),
        (
            "delete-id",
            &[
                "delete",
                "--id",
                "^id",
                "--type",
                "ACTI",
                "--type",
                "CLOT",
                "all_records.esp",
            ],
        ),
        (
            "delete-verbose",
            &["delete", "-v", "--type", "MISC", "all_records.esp"],
        ),
        (
            "delete-subrecords",
            &[
                "delete",
                "--sub-match",
                "Model",
                "--type",
                "ACTI",
                "all_records.esp",
            ],
        ),
        (
            "delete-all-subrecords",
            &[
                "delete",
                "--sub-match",
                ".",
                "--type",
                "GLOB",
                "all_records.esp",
            ],
        ),
        (
            "delete-instances",
            &["delete", "--instance-match", "door", "all_records.esp"],
        ),
        (
            "delete-other-instances",
            &[
                "delete",
                "--instance-no-match",
                "door",
                "--hide-backups",
                "all_records.esp",
            ],
        ),
        (
            "delete-nothing",
            &[
                "delete",
                "--type",
                "NONE",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
    ];
    for (name, args) in cases {
        check(both_fixtures(name, args));
    }
}

#[test]
fn modify_replace() {
    let cases: &[(&str, &[&str])] = &[
        (
            "modify-global",
            &[
                "modify",
                "--replace",
                "/model/MODEL/g",
                "--type",
                "ACTI",
                "all_records.esp",
            ],
        ),
        (
            "modify-groups",
            &[
                "modify",
                "--replace",
                "/id(.)/X$1Y/",
                "--sub-match",
                "Name",
                "all_records.esp",
            ],
        ),
        (
            "modify-faction",
            &[
                "modify",
                "--replace",
                "/Hlaalu/Telvanni/",
                "all_records.esp",
            ],
        ),
        (
            "modify-nothing",
            &["modify", "--replace", "/zzz/y/", "all_records.esp"],
        ),
        // Without --replace tes3cmd runs `main($R);`, which nothing defines.
        (
            "modify-replacefirst",
            &[
                "modify",
                "--replacefirst",
                "/x/y/",
                "--type",
                "BOOK",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
    ];
    for (name, args) in cases {
        check(both_fixtures(name, args));
    }
}

#[test]
fn diff_two_plugins() {
    let cases: &[(&str, &[&str])] = &[
        ("diff", &["diff", "all_records.esp", "odd_records.esp"]),
        (
            "diff-missing",
            &["diff", "--e1", "--e2", "all_records.esp", "odd_records.esp"],
        ),
        (
            "diff-equal",
            &["diff", "--equal", "all_records.esp", "odd_records.esp"],
        ),
        (
            "diff-sorted",
            &[
                "diff",
                "--ne",
                "--sortsubrecs",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
        (
            "diff-ignore",
            &[
                "diff",
                "--ignore-type",
                "BOOK",
                "--ignore-type",
                "CELL.AMBI",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
        (
            "diff-same",
            &["diff", "-v", "all_records.esp", "all_records.esp"],
        ),
    ];
    for (name, args) in cases {
        check_with(both_fixtures(name, args), true);
    }
}

fn multipatch_case(name: &'static str, args: &[&'static str]) -> Case {
    let ini = "[General]\r\nX=1\r\n[Game Files]\r\nGameFile0=mp_Master.esm\r\nGameFile1=mp_A.esp\r\nGameFile2=mp_B.esp\r\nGameFile3=mp_C.esp\r\n[Archives]\r\n";
    Case {
        name,
        files: vec![
            ("Morrowind.ini", ini.as_bytes().to_vec()),
            ("mp_Master.esm", fixture("mp_Master.esm")),
            ("mp_A.esp", fixture("mp_A.esp")),
            ("mp_B.esp", fixture("mp_B.esp")),
            ("mp_C.esp", fixture("mp_C.esp")),
        ],
        args: args.to_vec(),
    }
}

/// `multipatch` without activating the patch (tes3cmd dies there, see `active_on_and_off`) and
/// without its cache (Perl's Storable files; the port keeps none).
#[test]
fn multipatch_patches() {
    let cases: &[(&str, &[&str])] = &[
        ("multipatch", &["multipatch", "--no-cache", "--no-activate"]),
        (
            "multipatch-verbose",
            &["multipatch", "-v", "--no-cache", "--no-activate"],
        ),
        (
            "multipatch-lists",
            &[
                "multipatch",
                "--merge-lists",
                "--no-cache",
                "--no-activate",
                "--dc",
                "wolf",
                "--di",
                "axe",
                "-v",
            ],
        ),
        (
            "multipatch-fog",
            &["multipatch", "--fogbug", "--no-activate"],
        ),
        (
            "multipatch-names",
            &[
                "multipatch",
                "--cellnames",
                "--summons-persist",
                "--no-cache",
                "--no-activate",
            ],
        ),
        (
            "multipatch-objects",
            &["multipatch", "--merge-objects", "--no-activate"],
        ),
    ];
    for (name, args) in cases {
        check(multipatch_case(name, args));
    }
}

/// The patch activated: the port's ini, as `active --on` writes it.
#[test]
fn multipatch_activates_the_patch() {
    let root = std::env::temp_dir().join(format!("t3rs-mp-active-{}", std::process::id()));
    let data = lay_out(&root, &multipatch_case("mp-active", &[]));
    let o = Command::new(env!("CARGO_BIN_EXE_tes3cmd"))
        .args(["multipatch", "--no-cache"])
        .current_dir(&data)
        .env("PWD", &data)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).ends_with("\nACTIVATED: multipatch.esp\n"));
    let ini = std::fs::read_to_string(root.join("Morrowind.ini")).unwrap();
    assert!(ini.contains("GameFile4=multipatch.esp\r\n"), "{ini}");
    assert!(data.join("multipatch.esp").is_file());
    let _ = std::fs::remove_dir_all(&root);
}

/// `fixit`: tes3cmd 0.40-PRE-RELEASE-2 dies activating the multipatch ("Undefined subroutine
/// &TES3::Util::move"), before it synchronizes the headers; the port goes on. Up to there both
/// say and do the same: tes3cmd's output is where the port's starts, and Data Files (the cleaned
/// plugins, their backups, the multipatch) is the same.
#[test]
fn fixit_cleans_patches_and_synchronizes() {
    let Some(mut theirs) = tes3cmd() else {
        eprintln!("skipped fixit: no tes3cmd to compare with");
        return;
    };
    let base = std::env::temp_dir().join(format!("t3rs-fixit-{}", std::process::id()));
    let case = multipatch_case("fixit", &["fixit"]);
    let (pd, rd) = (base.join("perl"), base.join("rust"));
    let (pdata, rdata) = (lay_out(&pd, &case), lay_out(&rd, &case));
    let perl = theirs
        .arg("fixit")
        .current_dir(&pdata)
        .env("PWD", &pdata)
        .output()
        .unwrap();
    let rust = Command::new(env!("CARGO_BIN_EXE_tes3cmd"))
        .arg("fixit")
        .current_dir(&rdata)
        .env("PWD", &rdata)
        .output()
        .unwrap();
    let text = |s: &[u8], dir: &Path| {
        String::from_utf8_lossy(s).replace(&dir.to_string_lossy().into_owned(), "<ROOT>")
    };
    let (po, ro) = (text(&perl.stdout, &pd), text(&rust.stdout, &rd));
    let pe = unplace(&text(&perl.stderr, &pd))
        .replace("Undefined subroutine &TES3::Util::move called\n", "");
    let re = unplace(&text(&rust.stderr, &rd));
    assert!(ro.starts_with(&po), "--- perl\n{po}\n--- port\n{ro}");
    assert!(
        ro[po.len()..].starts_with("\n\nSynchronizing Plugin Headers...\n"),
        "{}",
        &ro[po.len()..]
    );
    assert_eq!(pe, re);
    assert_eq!(
        (perl.status.code(), rust.status.code()),
        (Some(255), Some(0))
    );
    assert_eq!(tree(&pdata), tree(&rdata), "Data Files differ");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn dump_raw_wrap_and_format() {
    let cases: &[(&str, &[&str])] = &[
        // --binary and --header set $opt_binary/$opt_header, which dump never reads.
        (
            "dump-binary-ignored",
            &[
                "dump",
                "--binary",
                "--header",
                "--output",
                "out.esp",
                "--type",
                "BOOK",
                "all_records.esp",
            ],
        ),
        (
            "dump-raw",
            &[
                "dump",
                "--raw",
                "out.esp",
                "--type",
                "GMST",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
        (
            "dump-raw-header",
            &[
                "dump",
                "--raw-with-header",
                "out.esp",
                "--instance-match",
                "door",
                "all_records.esp",
            ],
        ),
        (
            "dump-raw-list",
            &[
                "dump",
                "--raw-with-header",
                "out.esp",
                "--list",
                "--type",
                "ACTI",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
        ("dump-raw-dash", &["dump", "--raw", "-x", "all_records.esp"]),
        (
            "dump-wrap",
            &[
                "dump",
                "--wrap",
                "--type",
                "BOOK",
                "--type",
                "CLAS",
                "--type",
                "INFO",
                "all_records.esp",
            ],
        ),
        (
            "dump-format",
            &[
                "dump",
                "--format",
                "%id%_%name%_%weight%",
                "--type",
                "ALCH",
                "--type",
                "BOOK",
                "--type",
                "CELL",
                "all_records.esp",
            ],
        ),
        (
            "dump-format-lines",
            &[
                "dump",
                "--format",
                "X:%NPDT:level%|%model%",
                "--format",
                "second:%value%",
                "--type",
                "MISC",
                "--type",
                "NPC_",
                "all_records.esp",
            ],
        ),
        (
            "dump-format-unquoted",
            &[
                "dump",
                "--no-quote",
                "--format",
                "%bogus%=%fog_density%=%x%",
                "--type",
                "CELL",
                "--type",
                "LEVC",
                "all_records.esp",
            ],
        ),
        (
            "dump-format-fields",
            &[
                "dump",
                "--format",
                "%name%:%level%:%clear%:%offset%:%objidx%:%type%:%length%",
                "all_records.esp",
                "odd_records.esp",
            ],
        ),
    ];
    for (name, args) in cases {
        check(both_fixtures(name, args));
    }
    // The appended raw output: an out.esp already there.
    let mut c = both_fixtures(
        "dump-raw-append",
        &[
            "dump",
            "--raw",
            "out.esp",
            "--type",
            "BOOK",
            "all_records.esp",
        ],
    );
    c.files.push(("out.esp", b"EXISTING".to_vec()));
    check(c);
}

/// A savegame's record types (`tools/gen_all_records.pl ess`): arrays, equipped indexes and
/// the subrecords tes3cmd prints as unknown data.
#[test]
fn dump_savegame_records() {
    for (name, args) in [
        ("dump-ess", vec!["dump", "all_records.ess"]),
        (
            "dump-ess-list",
            vec![
                "dump",
                "--list",
                "--type",
                "REFR",
                "--type",
                "NPCC",
                "all_records.ess",
            ],
        ),
    ] {
        check(Case {
            name,
            files: vec![("all_records.ess", fixture("all_records.ess"))],
            args,
        });
    }
}
