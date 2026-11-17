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
    for (n, b) in &case.files {
        std::fs::write(data.join(n), b).unwrap();
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

fn run_both(case: &Case) -> Option<(String, String)> {
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
    match run_both(&case) {
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
