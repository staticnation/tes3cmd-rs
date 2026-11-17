//! The command line as tes3cmd's `tes3cmd_main` reads it: the command first, then Getopt::Long's
//! rules (options and plugins in any order, one dash or two, `--opt=value` or `--opt value`, a
//! name shortened to any unique beginning, in any letter case), plugin names with `*` or `?`
//! expanded as Perl's `glob` does, `--ignore-plugin` and `--active` applied.

use std::io::Write;

use crate::clean::{CleanOptions, Masters, clean_plugin};
use crate::ctx::Ctx;
use crate::datadir::Install;

pub const CLEAN_USAGE: &str = include_str!("clean_usage.txt");

/// The commands tes3cmd has; only those in `PORTED` are here yet.
const COMMANDS: &[&str] = &[
    "active",
    "clean",
    "codec",
    "common",
    "delete",
    "diff",
    "dump",
    "esm",
    "esp",
    "fixit",
    "header",
    "help",
    "lint",
    "modify",
    "multipatch",
    "overdial",
    "recover",
    "resetdates",
    "run",
    "shell",
    "testcodec",
    "undelete",
];
const PORTED: &[&str] = &["clean"];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Flag,
    Str,
    StrList,
}

/// One option as Getopt::Long is given it.
struct Spec {
    name: &'static str,
    kind: Kind,
}

const CLEAN_SPECS: &[Spec] = &[
    Spec {
        name: "assert",
        kind: Kind::Flag,
    },
    Spec {
        name: "debug",
        kind: Kind::Flag,
    },
    Spec {
        name: "output",
        kind: Kind::Str,
    },
    Spec {
        name: "verbose",
        kind: Kind::Flag,
    },
    Spec {
        name: "backup-dir",
        kind: Kind::Str,
    },
    Spec {
        name: "hide-backups",
        kind: Kind::Flag,
    },
    Spec {
        name: "active",
        kind: Kind::Flag,
    },
    Spec {
        name: "all",
        kind: Kind::Flag,
    },
    Spec {
        name: "ignore-plugin",
        kind: Kind::StrList,
    },
    Spec {
        name: "instances",
        kind: Kind::Flag,
    },
    Spec {
        name: "cell-params",
        kind: Kind::Flag,
    },
    Spec {
        name: "dups",
        kind: Kind::Flag,
    },
    Spec {
        name: "gmsts",
        kind: Kind::Flag,
    },
    Spec {
        name: "junk-cells",
        kind: Kind::Flag,
    },
    Spec {
        name: "no-cache",
        kind: Kind::Flag,
    },
    // Declared without "=s" in tes3cmd's clean: a flag, whose value is 1.
    Spec {
        name: "output-dir",
        kind: Kind::Flag,
    },
    Spec {
        name: "replace",
        kind: Kind::Flag,
    },
    Spec {
        name: "overwrite",
        kind: Kind::Flag,
    },
];

#[derive(Default, Debug)]
pub struct Parsed {
    pub flags: Vec<&'static str>,
    pub strs: Vec<(&'static str, String)>,
    pub args: Vec<String>,
}

impl Parsed {
    fn has(&self, n: &str) -> bool {
        self.flags.contains(&n)
    }
    fn last(&self, n: &str) -> Option<String> {
        self.strs
            .iter()
            .rev()
            .find(|(k, _)| *k == n)
            .map(|(_, v)| v.clone())
    }
    fn all(&self, n: &str) -> Vec<String> {
        self.strs
            .iter()
            .filter(|(k, _)| *k == n)
            .map(|(_, v)| v.clone())
            .collect()
    }
}

/// `GetOptions` with `auto_abbrev` (and its default `ignore_case` and `permute`): the options
/// found, the other words in order, and the warnings Getopt::Long gives; `Err` when one was given.
fn getopt(specs: &'static [Spec], argv: &[String], err: &mut dyn Write) -> Result<Parsed, ()> {
    let mut p = Parsed::default();
    let mut ok = true;
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        i += 1;
        if a == "--" {
            p.args.extend(argv[i..].iter().cloned());
            break;
        }
        let body = if let Some(b) = a.strip_prefix("--") {
            b
        } else if let Some(b) = a.strip_prefix('-').filter(|b| !b.is_empty()) {
            b
        } else {
            p.args.push(a.clone());
            continue;
        };
        let (name, inline) = match body.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (body, None),
        };
        let lname = name.to_lowercase();
        let exact: Vec<&Spec> = specs.iter().filter(|s| s.name == lname).collect();
        let found: Vec<&Spec> = if exact.is_empty() {
            specs
                .iter()
                .filter(|s| s.name.starts_with(&lname))
                .collect()
        } else {
            exact
        };
        match found.as_slice() {
            [] => {
                let _ = writeln!(err, "Unknown option: {lname}");
                ok = false;
            }
            [s] => match s.kind {
                Kind::Flag => {
                    if inline.is_some() {
                        let _ = writeln!(err, "Option {} does not take an argument", s.name);
                        ok = false;
                    } else {
                        p.flags.push(s.name);
                    }
                }
                Kind::Str | Kind::StrList => {
                    let v = match inline {
                        Some(v) => Some(v),
                        None if i < argv.len() => {
                            i += 1;
                            Some(argv[i - 1].clone())
                        }
                        None => None,
                    };
                    match v {
                        Some(v) => p.strs.push((s.name, v)),
                        None => {
                            let _ = writeln!(err, "Option {} requires an argument", s.name);
                            ok = false;
                        }
                    }
                }
            },
            many => {
                let names: Vec<&str> = many.iter().map(|s| s.name).collect();
                let _ = writeln!(err, "Option {lname} is ambiguous ({})", names.join(", "));
                ok = false;
            }
        }
    }
    if ok { Ok(p) } else { Err(()) }
}

/// Perl's `glob` on a word with `*` or `?`: split at blanks (quotes kept together), each pattern
/// matched against its folder's names, sorted; a pattern that matches nothing gives nothing.
pub fn glob(word: &str) -> Vec<String> {
    let mut pats = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in word.chars() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    pats.push(std::mem::take(&mut cur));
                }
            }
            (_, c) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        pats.push(cur);
    }
    let mut out = Vec::new();
    for pat in pats {
        if !pat.contains(['*', '?']) {
            out.push(pat);
            continue;
        }
        let (dir, name) = match pat.rfind(['/', '\\']) {
            Some(k) => (pat[..=k].to_string(), pat[k + 1..].to_string()),
            None => (String::new(), pat.clone()),
        };
        let Ok(rd) = std::fs::read_dir(if dir.is_empty() { "." } else { &dir }) else {
            continue;
        };
        let mut hits: Vec<String> = rd
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.') || name.starts_with('.'))
            .filter(|n| wild(&name, n, cfg!(windows)))
            .map(|n| format!("{dir}{n}"))
            .collect();
        hits.sort();
        out.extend(hits);
    }
    out
}

fn wild(pat: &str, s: &str, nocase: bool) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = if nocase {
        (
            pat.to_lowercase().chars().collect(),
            s.to_lowercase().chars().collect(),
        )
    } else {
        (pat.chars().collect(), s.chars().collect())
    };
    fn m(p: &[char], s: &[char]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some('*'), _) => m(&p[1..], s) || (!s.is_empty() && m(p, &s[1..])),
            (Some('?'), Some(_)) => m(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a == b => m(&p[1..], &s[1..]),
            _ => false,
        }
    }
    m(&p, &s)
}

fn minihelp(out: &mut dyn Write) {
    let _ = write!(
        out,
        "tes3cmd (Rust port of tes3cmd {})\n\nUsage: tes3cmd COMMAND OPTIONS plugin...\n\nCOMMANDS ported so far: {}\nThe other commands of tes3cmd are not in this port yet: use the Perl tes3cmd for them.\n",
        crate::TES3CMD_VERSION,
        PORTED.join(", ")
    );
}

/// The program: answers its exit code (Perl's 255 for a `die`).
pub fn run(argv: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let debug = argv
        .iter()
        .any(|a| matches!(a.as_str(), "-d" | "--debug" | "-debug"));
    let verbose = debug
        || argv
            .iter()
            .any(|a| matches!(a.as_str(), "-v" | "--verbose" | "-verbose"));
    let mut ctx = Ctx::new(out, err);
    ctx.debug = debug;
    ctx.verbose = verbose;
    let mut inst = Install::find(&mut ctx);
    let mut rest = argv.to_vec();
    let cmd = if rest.first().is_none_or(|a| a.starts_with("--")) {
        String::new()
    } else {
        rest.remove(0)
    };
    if cmd.is_empty() || cmd.to_lowercase().contains("help") {
        minihelp(ctx.out);
        return 0;
    }
    if COMMANDS.contains(&cmd.as_str()) && !PORTED.contains(&cmd.as_str()) {
        let _ = writeln!(
            ctx.err,
            "tes3cmd: \"{cmd}\" is not in this port yet (ported: {}); use the Perl tes3cmd for it.",
            PORTED.join(", ")
        );
        return 2;
    }
    if !PORTED.contains(&cmd.as_str()) {
        minihelp(ctx.out);
        return 0;
    }
    // clean
    let Ok(p) = getopt(CLEAN_SPECS, &rest, ctx.err) else {
        let _ = writeln!(ctx.err, "{CLEAN_USAGE}");
        return 255;
    };
    let ignored: Vec<String> = p
        .all("ignore-plugin")
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    let mut plugins = Vec::new();
    for word in &p.args {
        let names = if word.contains(['*', '?']) {
            glob(word)
        } else {
            vec![word.clone()]
        };
        for n in names {
            if ignored.contains(&n.to_lowercase()) {
                ctx.print(&format!("Ignoring Plugin: {n}\n"));
            } else {
                plugins.push(n);
            }
        }
    }
    if p.has("active") {
        let lo = inst.load_order();
        if plugins.is_empty() {
            plugins = lo;
        } else {
            let lower: Vec<String> = lo.iter().map(|s| s.to_lowercase()).collect();
            let mut active: Vec<(String, std::time::SystemTime)> = plugins
                .into_iter()
                .filter(|n| lower.contains(&n.to_lowercase()))
                .map(|n| {
                    let t = std::fs::metadata(&n)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    (n, t)
                })
                .collect();
            active.sort_by_key(|a| a.1);
            plugins = active.into_iter().map(|(n, _)| n).collect();
        }
    }
    let mut opts = CleanOptions {
        instances: p.has("instances"),
        cell_params: p.has("cell-params"),
        dups: p.has("dups"),
        gmsts: p.has("gmsts"),
        junk_cells: p.has("junk-cells"),
        overwrite: p.has("overwrite") || p.has("replace"),
        output: crate::output::Output {
            output_dir: p.has("output-dir").then(|| "1".to_string()),
            hide_backups: p.has("hide-backups"),
            backup_dir: p.last("backup-dir"),
        },
    };
    if let Some(d) = &opts.output.output_dir
        && !std::path::Path::new(d).is_dir()
        && std::fs::create_dir_all(d).is_err()
    {
        let _ = writeln!(ctx.err, "FATAL ERROR (): Unable to create directory: {d}");
        return 255;
    }
    opts.settle(p.has("all"));
    if plugins.is_empty() {
        let _ = write!(ctx.err, "No plugins to process!\n\n{CLEAN_USAGE}\n");
        return 255;
    }
    let mut masters = Masters::default();
    for plugin in &plugins {
        if ctx.verbose {
            ctx.print(&format!("Processing: {plugin}\n"));
        }
        ctx.plugin = plugin.clone();
        if let Err(e) = clean_plugin(&mut ctx, &opts, &mut inst, &mut masters, plugin) {
            let _ = writeln!(ctx.err, "{}", e.0);
            return 255;
        }
    }
    0
}
