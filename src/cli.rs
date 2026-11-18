//! The command line as tes3cmd's `tes3cmd_main` reads it: the command first, then Getopt::Long's
//! rules (options and plugins in any order, one dash or two, `--opt=value` or `--opt value`, a
//! name shortened to any unique beginning, in any letter case), plugin names with `*` or `?`
//! expanded as Perl's `glob` does, `--ignore-plugin` and `--active` applied.

use std::io::Write;

use crate::clean::{CleanOptions, Masters, clean_plugin};
use crate::ctx::Ctx;
use crate::datadir::Install;

/// The commands tes3cmd dispatches (its `%COMMAND`, without the ones it hides behind a `-`), and
/// which of them are here yet.
const PORTED: &[&str] = &[
    "active",
    "clean",
    "common",
    "delete",
    "diff",
    "dump",
    "esm",
    "esp",
    "fixit",
    "header",
    "modify",
    "multipatch",
    "overdial",
    "recover",
    "resetdates",
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Flag,
    Str,
    StrList,
}

/// One option as Getopt::Long is given it (`name|alias=s@`).
struct Spec {
    name: &'static str,
    aliases: Vec<&'static str>,
    kind: Kind,
}

/// A command's row of the generated table.
fn command(name: &str) -> Option<&'static (&'static str, &'static [&'static str], &'static str)> {
    crate::commands_table::COMMANDS.iter().find(|c| c.0 == name)
}

fn specs(cmd: &'static (&'static str, &'static [&'static str], &'static str)) -> Vec<Spec> {
    cmd.1
        .iter()
        .map(|s| {
            let (names, kind) = match s.split_once('=') {
                Some((n, k)) if k.ends_with('@') => (n, Kind::StrList),
                Some((n, _)) => (n, Kind::Str),
                None => (*s, Kind::Flag),
            };
            let mut all = names.split('|');
            let name = all.next().unwrap_or(names);
            Spec {
                name,
                aliases: all.collect(),
                kind,
            }
        })
        .collect()
}

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
fn getopt(specs: &[Spec], argv: &[String], err: &mut dyn Write) -> Result<Parsed, ()> {
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
        let names = |s: &Spec| {
            std::iter::once(s.name)
                .chain(s.aliases.iter().copied())
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        };
        let exact: Vec<&Spec> = specs.iter().filter(|s| names(s).contains(&lname)).collect();
        let found: Vec<&Spec> = if exact.is_empty() {
            specs
                .iter()
                .filter(|s| names(s).iter().any(|n| n.starts_with(&lname)))
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
    crate::tostr::set_wrap(argv.iter().any(|a| a == "--wrap" || a == "-wrap"));
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
    let Some(row) = command(&cmd) else {
        minihelp(ctx.out);
        return 0;
    };
    if !PORTED.contains(&cmd.as_str()) {
        let _ = writeln!(
            ctx.err,
            "tes3cmd: \"{cmd}\" is not in this port yet (ported: {}); use the Perl tes3cmd for it.",
            PORTED.join(", ")
        );
        return 2;
    }
    let usage = row.2;
    let Ok(p) = getopt(&specs(row), &rest, ctx.err) else {
        let _ = writeln!(ctx.err, "{usage}");
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
    let output = crate::output::Output {
        output_dir: p.has("output-dir").then(|| "1".to_string()),
        hide_backups: p.has("hide-backups"),
        backup_dir: p.last("backup-dir"),
    };
    if let Some(d) = &output.output_dir
        && !std::path::Path::new(d).is_dir()
        && std::fs::create_dir_all(d).is_err()
    {
        let _ = writeln!(ctx.err, "FATAL ERROR (): Unable to create directory: {d}");
        return 255;
    }
    let result = dispatch(&mut ctx, &mut inst, &cmd, usage, &p, plugins, output);
    match result {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(ctx.err, "{}", e.0);
            // Perl's die exits with errno when it is set: esp and esm abort with ENOENT still in
            // it from the file tests before them.
            if matches!(cmd.as_str(), "esp" | "esm") {
                2
            } else {
                255
            }
        }
    }
}

/// The plugins processed one by one (`process`), as tes3cmd_main does: "No plugins to process!"
/// when there are none.
fn each(
    ctx: &mut Ctx,
    usage: &str,
    plugins: &[String],
    mut f: impl FnMut(&mut Ctx, &str) -> crate::ctx::Res<()>,
) -> crate::ctx::Res<i32> {
    if plugins.is_empty() {
        let _ = write!(ctx.err, "No plugins to process!\n\n{usage}\n");
        return Ok(255);
    }
    for plugin in plugins {
        if ctx.verbose {
            ctx.print(&format!("Processing: {plugin}\n"));
        }
        ctx.plugin = plugin.clone();
        f(ctx, plugin)?;
    }
    Ok(0)
}

/// The record selectors as the command line gives them (`get_wanted`); `Err` its exit code.
fn wanted(ctx: &mut Ctx, p: &Parsed, instance: bool) -> Result<crate::commands::Wanted, i32> {
    crate::commands::get_wanted(
        ctx,
        &crate::commands::WantedArgs {
            id: &p.all("id"),
            exact_id: &p.all("exact-id"),
            flag: &p.all("flag"),
            types: &p.all("type"),
            interior: p.has("interior"),
            exterior: p.has("exterior"),
            instance,
            matches: &p.all("match"),
            no_matches: &p.all("no-match"),
        },
    )
    .map_err(|e| {
        let _ = writeln!(ctx.err, "{}", e.0);
        // It dies before any plugin is read, with ENOENT still in errno.
        2
    })
}

fn re(ctx: &Ctx, p: &Parsed, name: &str) -> crate::ctx::Res<Option<regex::bytes::Regex>> {
    p.last(name)
        .map(|m| crate::commands::perl_re(ctx, &m, false))
        .transpose()
}

fn multipatch_options(p: &Parsed) -> crate::multipatch::MultipatchOptions {
    crate::multipatch::MultipatchOptions {
        cellnames: p.has("cellnames"),
        fogbug: p.has("fogbug"),
        merge_lists: p.has("merge-lists"),
        merge_objects: p.has("merge-objects"),
        summons_persist: p.has("summons-persist"),
        no_activate: p.has("no-activate"),
        no_cache: p.has("no-cache"),
        delete_creature: p.all("delete-creature"),
        delete_item: p.all("delete-item"),
    }
}

fn dispatch(
    ctx: &mut Ctx,
    inst: &mut Install,
    cmd: &str,
    usage: &str,
    p: &Parsed,
    plugins: Vec<String>,
    output: crate::output::Output,
) -> crate::ctx::Res<i32> {
    use crate::commands as c;
    match cmd {
        "clean" => {
            let mut opts = CleanOptions {
                instances: p.has("instances"),
                cell_params: p.has("cell-params"),
                dups: p.has("dups"),
                gmsts: p.has("gmsts"),
                junk_cells: p.has("junk-cells"),
                overwrite: p.has("overwrite") || p.has("replace"),
                output,
            };
            opts.settle(p.has("all"));
            let mut masters = Masters::default();
            each(ctx, usage, &plugins, |ctx, plugin| {
                clean_plugin(ctx, &opts, inst, &mut masters, plugin)
            })
        }
        "header" => {
            let sync = p.has("synchronize");
            let o = c::HeaderOptions {
                author: p.last("author"),
                description: p.last("description"),
                multiline: p.has("multiline"),
                update_masters: sync || p.has("update-masters"),
                update_record_count: sync || p.has("update-record-count"),
                output,
            };
            each(ctx, usage, &plugins, |ctx, plugin| {
                c::update_header(ctx, &o, inst, plugin, false, false)
            })
        }
        "esp" | "esm" => {
            let want = if cmd == "esp" { ".esm" } else { ".esp" };
            let overwrite = p.has("overwrite");
            each(ctx, usage, &plugins, |ctx, plugin| {
                if !plugin.to_ascii_lowercase().ends_with(want) {
                    let what = if cmd == "esp" {
                        "a master (.esm)"
                    } else {
                        "a plugin (.esp)"
                    };
                    return Err(ctx.abort(&format!("Error, input must be {what}")));
                }
                c::esp_esm(ctx, plugin, overwrite)
            })
        }
        "recover" => each(ctx, usage, &plugins, |ctx, plugin| {
            c::recover(ctx, &output, inst, plugin)
        }),
        "common" => {
            let (a, b) = (
                plugins.first().cloned().unwrap_or_default(),
                plugins.get(1).cloned().unwrap_or_default(),
            );
            c::common(ctx, &a, &b).map(|_| 0)
        }
        "dump" => {
            // --format: `%field%` becomes `%s`, the fields in order.
            let formats: Vec<(Vec<u8>, Vec<String>)> = p
                .all("format")
                .iter()
                .map(|f| {
                    let re = regex::bytes::Regex::new("%([^%]+)%").unwrap();
                    let fields = re
                        .captures_iter(f.as_bytes())
                        .map(|c| String::from_utf8_lossy(&c[1]).into_owned())
                        .collect();
                    (
                        re.replace_all(f.as_bytes(), &b"%s"[..]).into_owned(),
                        fields,
                    )
                })
                .collect();
            let (im, inm) = (p.last("instance-match"), p.last("instance-no-match"));
            // get_wanted dies before any plugin is read, with ENOENT still in errno: exit 2.
            let wanted = match c::get_wanted(
                ctx,
                &c::WantedArgs {
                    id: &p.all("id"),
                    exact_id: &p.all("exact-id"),
                    flag: &p.all("flag"),
                    types: &p.all("type"),
                    interior: p.has("interior"),
                    exterior: p.has("exterior"),
                    instance: im.is_some() || inm.is_some(),
                    matches: &p.all("match"),
                    no_matches: &p.all("no-match"),
                },
            ) {
                Ok(w) => w,
                Err(e) => {
                    let _ = writeln!(ctx.err, "{}", e.0);
                    return Ok(2);
                }
            };
            // The obsolete switches, as tes3cmd translates them.
            // tes3cmd's --binary and --header set $opt_binary and $opt_header, which dump never
            // reads: only the "obsolete" --raw and --raw-with-header write records.
            let mut binary = false;
            let mut header = false;
            let mut output = p.last("output");
            if let Some(r) = p.last("raw-with-header") {
                ctx.msg("Note: --raw-with-header is obsolete, you can now use:\n\t--binary --header --output <filename>");
                header = true;
                binary = true;
                output = Some(r);
            } else if let Some(r) = p.last("raw") {
                ctx.msg(
                    "Note: --raw is obsolete, you can now use:\n\t--binary --output <filename>",
                );
                if r.starts_with('-') {
                    let _ = writeln!(
                        ctx.err,
                        "FATAL ERROR (): did you forget to give a filename for raw output?"
                    );
                    return Ok(2);
                }
                binary = true;
                output = Some(r);
            }
            let raw_to = if binary {
                let path = output.clone().unwrap_or_default();
                let mut start = Vec::new();
                if header {
                    start = crate::multipatch::make_header();
                } else if std::path::Path::new(&path).is_file() {
                    ctx.msg(&format!("Appending to: \"{path}\""));
                    start = std::fs::read(&path).unwrap_or_default();
                } else {
                    ctx.msg(&format!("Creating: \"{path}\""));
                }
                if let Err(e) = std::fs::write(&path, &start) {
                    let _ = writeln!(
                        ctx.err,
                        "FATAL ERROR (): opening \"{path}\" for output ({})",
                        crate::output::os_message(&e)
                    );
                    return Ok(2);
                }
                Some(path)
            } else {
                None
            };
            let o = c::DumpOptions {
                formats,
                no_quote: p.has("no-quote"),
                raw_to: raw_to.clone(),
                wanted,
                list: p.has("list"),
                no_banner: p.has("no-banner") || plugins.len() == 1,
                instance_match: im.map(|m| c::perl_re(ctx, &m, false)).transpose()?,
                instance_no_match: inm.map(|m| c::perl_re(ctx, &m, false)).transpose()?,
            };
            let mut st = crate::tostr::State {
                verbose: ctx.verbose,
                ..Default::default()
            };
            let code = each(ctx, usage, &plugins, |ctx, plugin| {
                c::dump(ctx, &o, &mut st, plugin)
            })?;
            if let Some(path) = raw_to {
                if header {
                    let h = c::HeaderOptions {
                        author: None,
                        description: None,
                        multiline: false,
                        update_masters: false,
                        update_record_count: true,
                        output: Default::default(),
                    };
                    c::update_header(ctx, &h, inst, &path, true, true)?;
                }
                ctx.print(&format!("\nRaw records saved in {path}\n"));
            }
            Ok(code)
        }
        "delete" => {
            let instance =
                p.last("instance-match").is_some() || p.last("instance-no-match").is_some();
            let w = match wanted(ctx, p, instance) {
                Ok(w) => w,
                Err(code) => return Ok(code),
            };
            let o = crate::edit::DeleteOptions {
                wanted: w,
                sub_match: re(ctx, p, "sub-match")?,
                sub_no_match: re(ctx, p, "sub-no-match")?,
                instance_match: re(ctx, p, "instance-match")?,
                instance_no_match: re(ctx, p, "instance-no-match")?,
                output,
            };
            let mut st = crate::tostr::State {
                verbose: ctx.verbose,
                ..Default::default()
            };
            each(ctx, usage, &plugins, |ctx, plugin| {
                crate::edit::delete(ctx, &o, &mut st, inst, plugin)
            })
        }
        "modify" => {
            if p.last("run").is_some()
                || p.last("program-file").is_some()
                || p.last("replace").is_none() && p.last("replacefirst").is_none()
            {
                let _ = writeln!(
                    ctx.err,
                    "tes3cmd: modify --run and --program-file run Perl code, which this port cannot; use the Perl tes3cmd for them (--replace is here)."
                );
                return Ok(2);
            }
            // Both expressions are checked; only --replace is applied (--replacefirst alone
            // changes nothing, as in tes3cmd).
            let mut replace = None;
            for (opt, keep) in [("replacefirst", false), ("replace", true)] {
                if let Some(x) = p.last(opt) {
                    match crate::edit::parse_subst(&x) {
                        Ok(s) if keep => replace = Some(s),
                        Ok(_) => {}
                        Err(m) => {
                            let _ = writeln!(ctx.err, "FATAL ERROR (): {m}");
                            return Ok(255);
                        }
                    }
                }
            }
            let w = match wanted(ctx, p, false) {
                Ok(w) => w,
                Err(code) => return Ok(code),
            };
            let o = crate::edit::ModifyOptions {
                evals: std::cell::Cell::new(24),
                wanted: w,
                replace,
                sub_match: re(ctx, p, "sub-match")?,
                sub_no_match: re(ctx, p, "sub-no-match")?,
                output,
            };
            let mut st = crate::tostr::State {
                verbose: ctx.verbose,
                ..Default::default()
            };
            each(ctx, usage, &plugins, |ctx, plugin| {
                crate::edit::modify(ctx, &o, &mut st, inst, plugin)
            })
        }
        "diff" => {
            let any = ["1-not-2", "2-not-1", "equal", "not-equal", "types"]
                .iter()
                .any(|f| p.has(f));
            let o = crate::edit::DiffOptions {
                ignore_types: p.all("ignore-type"),
                one_not_two: !any || p.has("1-not-2"),
                two_not_one: !any || p.has("2-not-1"),
                equal: !any || p.has("equal"),
                not_equal: !any || p.has("not-equal"),
                sortsubrecs: p.has("sortsubrecs"),
            };
            let (a, b) = (
                plugins.first().cloned().unwrap_or_default(),
                plugins.get(1).cloned().unwrap_or_default(),
            );
            let mut st = crate::tostr::State {
                verbose: ctx.verbose,
                ..Default::default()
            };
            crate::edit::diff(ctx, &o, &mut st, &a, &b).map(|_| 0)
        }
        "fixit" => {
            // cmd_fixit: Bethesda's dates, every active plugin cleaned in place (all of clean's
            // options, backups hidden), a multipatch, then every header synchronized.
            c::resetdates(ctx, inst)?;
            let mut opts = CleanOptions {
                instances: true,
                cell_params: true,
                dups: true,
                gmsts: true,
                junk_cells: true,
                overwrite: true,
                output: crate::output::Output {
                    hide_backups: true,
                    ..output.clone()
                },
            };
            opts.settle(false);
            let mut masters = Masters::default();
            for plugin in inst.load_order() {
                ctx.plugin = plugin.clone();
                clean_plugin(ctx, &opts, inst, &mut masters, &plugin)?;
            }
            let mut mo = multipatch_options(p);
            mo.cellnames = true;
            mo.fogbug = true;
            mo.merge_lists = true;
            mo.merge_objects = true;
            mo.summons_persist = true;
            crate::multipatch::multipatch(ctx, &mo, inst)?;
            ctx.print("\n\nSynchronizing Plugin Headers...\n");
            let h = c::HeaderOptions {
                author: None,
                description: None,
                multiline: false,
                update_masters: true,
                update_record_count: true,
                output: crate::output::Output {
                    hide_backups: true,
                    ..output
                },
            };
            for plugin in inst.load_order() {
                ctx.plugin = plugin.clone();
                c::update_header(ctx, &h, inst, &plugin, false, false)?;
            }
            Ok(0)
        }
        "multipatch" => {
            let mut o = multipatch_options(p);
            if !(o.cellnames || o.fogbug || o.merge_lists || o.summons_persist || o.merge_objects) {
                o.cellnames = true;
                o.fogbug = true;
                o.merge_lists = true;
                o.merge_objects = true;
                o.summons_persist = true;
            }
            crate::multipatch::multipatch(ctx, &o, inst).map(|_| 0)
        }
        "active" => c::active(ctx, inst, p.has("on"), p.has("off"), &plugins).map(|_| 0),
        "resetdates" => c::resetdates(ctx, inst).map(|_| 0),
        "overdial" => c::overdial(ctx, &plugins, p.has("single")).map(|_| 0),
        _ => Ok(0),
    }
}
