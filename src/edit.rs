//! The commands that change plugins record by record or compare them: `delete`, `modify`
//! (its `--replace`; `--run` and `--program-file` run Perl, which a port cannot), and `diff`.

use std::collections::BTreeMap;
use std::path::Path;

use crate::commands::{Rec, Wanted, rec_match};
use crate::ctx::{Ctx, Res};
use crate::datadir::Install;
use crate::decode::{Decoded, Fields, Sub};
use crate::output::{Output, fix_output};
use crate::pack::Val;
use crate::record::{Reader, Record, read_one};
use crate::tostr::{State, record_id, record_tostr, sub_ids, sub_tostr};

/// What a command does with one record (`process_plugin_for_update`'s callback).
pub enum Outcome {
    /// `''`: written as read.
    Pass,
    /// A record to write in its place.
    Changed(Decoded, u32),
    /// `undef`: left out.
    Delete,
}

/// `process_plugin_for_update`: each record through `f`, into `<plugin>.tmp`, then
/// `fix_output`. A failure is reported as tes3cmd's `eval` reports it, and the plugin is left.
pub fn for_update(
    ctx: &mut Ctx,
    o: &Output,
    inst: &mut Install,
    plugin: &str,
    mut f: impl FnMut(&mut Ctx, &mut Rec) -> Result<Outcome, String>,
) -> Res<()> {
    if !crate::clean::has_plugin_ext(plugin) {
        ctx.msg(&format!(
            "FATAL ERROR ({plugin}): \"{plugin}\" does not end in .esm/.esp/.ess"
        ));
        return Ok(());
    }
    if !Path::new(plugin).is_file() {
        ctx.msg(&format!(
            "FATAL ERROR ({plugin}): Invalid input file (No such file or directory)"
        ));
        return Ok(());
    }
    let data = std::fs::read(plugin)
        .map_err(|e| ctx.abort(&format!("opening \"{plugin}\" for input ({e})")))?;
    let tmp = format!("{plugin}.tmp");
    let mut out = Vec::new();
    let mut modified = 0;
    let mut pos = 0;
    let mut expected = Some("TES3");
    loop {
        let rec = match read_one(ctx, plugin, &data, &mut pos, expected, Reader::Input) {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(e) => {
                let _ = std::fs::write(&tmp, &out);
                ctx.msg(&e.0);
                return Ok(());
            }
        };
        expected = None;
        let mut r = Rec { rec, d: None };
        let outcome = match f(ctx, &mut r) {
            Ok(x) => x,
            Err(m) => {
                let _ = std::fs::write(&tmp, &out);
                ctx.msg(&m);
                return Ok(());
            }
        };
        let id = || r.d.as_ref().map(record_id).unwrap_or_default();
        match outcome {
            Outcome::Pass => r.rec.write(&mut out),
            Outcome::Changed(d, flags) => {
                let body = d.encode(ctx);
                Record {
                    rectype: d.rectype,
                    hdrflags: flags,
                    buf: body,
                }
                .write(&mut out);
                if ctx.verbose {
                    if modified == 0 {
                        ctx.print(&format!("UPDATED PLUGIN: {plugin}\n"));
                    }
                    let line = format!("RECORD MODIFIED: {} {}\n", r.rec.rectype_str(), id());
                    ctx.print(&line);
                }
                modified += 1;
            }
            Outcome::Delete => {
                if ctx.verbose {
                    if modified == 0 {
                        ctx.print(&format!("UPDATED PLUGIN: {plugin}\n"));
                    }
                    let line = format!("RECORD DELETED: {} {}\n", r.rec.rectype_str(), id());
                    ctx.print(&line);
                }
                modified += 1;
            }
        }
    }
    std::fs::write(&tmp, &out)
        .map_err(|e| ctx.abort(&format!("opening \"{tmp}\" for output ({e})")))?;
    fix_output(
        ctx,
        o,
        inst,
        plugin,
        modified > 0,
        None,
        crate::header::update_record_count,
    )
}

/// A subrecord's text, a formatter's death (`unknown_data`) as the error that ends the plugin.
fn sub_text(
    ctx: &mut Ctx,
    st: &mut State,
    d: &Decoded,
    s: &Sub,
    id: Option<&str>,
) -> Result<Vec<u8>, String> {
    let t = sub_tostr(ctx, st, d, s, id);
    if crate::tostr::died() {
        return Err(crate::tostr::NOT_A_SCALAR.into());
    }
    Ok(t)
}

fn rec_text(ctx: &mut Ctx, st: &mut State, d: &Decoded) -> Result<Vec<u8>, String> {
    record_tostr(ctx, st, d).map_err(String::from)
}

fn first_field(s: &Sub) -> Option<Vec<u8>> {
    match &s.fields {
        Fields::Generic { slots, .. } => slots.first().map(Val::bytes),
        _ => None,
    }
}

// ---------------------------------------------------------------- delete

pub struct DeleteOptions {
    pub wanted: Wanted,
    pub sub_match: Option<regex::bytes::Regex>,
    pub sub_no_match: Option<regex::bytes::Regex>,
    pub instance_match: Option<regex::bytes::Regex>,
    pub instance_no_match: Option<regex::bytes::Regex>,
    pub output: Output,
}

/// `cmd_delete`.
pub fn delete(
    ctx: &mut Ctx,
    o: &DeleteOptions,
    st: &mut State,
    inst: &mut Install,
    plugin: &str,
) -> Res<()> {
    let subs = o.sub_match.is_some() || o.sub_no_match.is_some();
    let instances = o.instance_match.is_some() || o.instance_no_match.is_some();
    for_update(ctx, &o.output, inst, plugin, |ctx, r| {
        let (ok, text) = rec_match(ctx, st, &o.wanted, r).map_err(String::from)?;
        if !ok {
            return Ok(Outcome::Pass);
        }
        let Some(d) = r.d.as_ref() else {
            return Ok(Outcome::Pass);
        };
        let rectype = r.rec.rectype_str();
        let id = record_id(d);
        if subs {
            let ids = sub_ids(d);
            let mut kept = Vec::new();
            for (s, sid) in d.subs.iter().zip(&ids) {
                let t = sub_text(ctx, st, d, s, sid.as_deref())?;
                let hit = o.sub_match.as_ref().is_none_or(|re| re.is_match(&t))
                    && o.sub_no_match.as_ref().is_none_or(|re| !re.is_match(&t));
                if hit {
                    ctx.print_bytes(
                        &[
                            format!("[{rectype} {id}] DELETED SUBRECORD: ").as_bytes(),
                            &t,
                            b"\n",
                        ]
                        .concat(),
                    );
                } else {
                    kept.push(s.clone());
                }
            }
            if kept.is_empty() {
                ctx.print(&format!(
                    "DELETED: {rectype} {id} (all subrecords were deleted)\n"
                ));
                return Ok(Outcome::Delete);
            }
            // `$rectype->new([...])`: a new record, header flags 0, whether or not anything went.
            return Ok(Outcome::Changed(Decoded::from_subs(d.rectype, kept), 0));
        }
        if instances {
            if rectype != "CELL" {
                return Ok(Outcome::Pass);
            }
            let groups = d.split_groups();
            let ids = sub_ids(d);
            let mut at = groups.first().map_or(0, Vec::len);
            let mut newcell: Vec<Sub> = groups.first().cloned().unwrap_or_default();
            let mut deletions = 0;
            let mut nam0: Option<usize> = None;
            let mut n = 0i64;
            for g in groups.iter().skip(1) {
                let first = at;
                at += g.len();
                if g.first().is_some_and(|s| &s.tag == b"NAM0") {
                    newcell.push(g[0].clone());
                    nam0 = Some(newcell.len() - 1);
                    continue;
                }
                let mut gs = Vec::new();
                let mut name: Option<Vec<u8>> = None;
                let mut objidx: Option<u32> = None;
                for (k, s) in g.iter().enumerate() {
                    gs.extend(sub_text(
                        ctx,
                        st,
                        d,
                        s,
                        ids.get(first + k).and_then(|x| x.as_deref()),
                    )?);
                    if name.is_none() && &s.tag == b"NAME" {
                        name = first_field(s);
                    }
                    if objidx.is_none()
                        && &s.tag == b"FRMR"
                        && let Fields::Reference { idx } = s.fields
                    {
                        objidx = idx.map(|i| i & 0xff_ffff);
                    }
                }
                let hit = o.instance_match.as_ref().is_none_or(|re| re.is_match(&gs))
                    && o.instance_no_match
                        .as_ref()
                        .is_none_or(|re| !re.is_match(&gs));
                if hit {
                    let line = [
                        format!("[CELL: {id}] DELETED OBJECT INSTANCE: ").as_bytes(),
                        &name.unwrap_or_default(),
                        format!(
                            " ObjIdx:{}\n",
                            objidx.map(|i| i.to_string()).unwrap_or_default()
                        )
                        .as_bytes(),
                    ]
                    .concat();
                    ctx.print_bytes(&line);
                    deletions += 1;
                } else {
                    newcell.extend(g.iter().cloned());
                    if nam0.is_some() {
                        n += 1;
                    }
                }
            }
            if deletions == 0 {
                return Ok(Outcome::Pass);
            }
            if let Some(i) = nam0
                && let Fields::Generic { slots, .. } = &mut newcell[i].fields
                && !slots.is_empty()
            {
                slots[0] = Val::Int(n);
            }
            return Ok(Outcome::Changed(Decoded::from_subs(*b"CELL", newcell), 0));
        }
        if ctx.verbose {
            let t = match text {
                Some(t) => t,
                None => rec_text(ctx, st, d)?,
            };
            ctx.print_bytes(&[b"\nDELETED RECORD:\n", &t[..], b"\n"].concat());
        } else {
            ctx.print(&format!("DELETED: {rectype} {id}\n"));
        }
        Ok(Outcome::Delete)
    })
}

// ---------------------------------------------------------------- modify --replace

/// A Perl substitution (`s` and what follows it on tes3cmd's command line: `/old/new/gi`).
pub struct Subst {
    re: regex::bytes::Regex,
    replacement: Vec<Piece>,
    global: bool,
}

enum Piece {
    Text(Vec<u8>),
    Group(usize),
}

/// Reads `/pattern/replacement/flags` (any delimiter; brackets pair: `{a}{b}`). `Err` as tes3cmd's
/// "Invalid replacer expression"; `/e` (the replacement as Perl code) is not portable.
pub fn parse_subst(expr: &str) -> Result<Subst, String> {
    let b = expr.as_bytes();
    let bad = || format!("Invalid replacer expression: {expr}");
    let d = *b.first().ok_or_else(bad)?;
    if d.is_ascii_alphanumeric() || d.is_ascii_whitespace() || d == b'\\' {
        return Err(bad());
    }
    let close = |o: u8| match o {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b'<' => b'>',
        x => x,
    };
    // One delimited part from `i`: its text and the index after its closing delimiter.
    let part = |mut i: usize, open: u8| -> Option<(Vec<u8>, usize)> {
        let end = close(open);
        let mut depth = 0;
        let mut out = Vec::new();
        while i < b.len() {
            let c = b[i];
            if c == b'\\' && i + 1 < b.len() {
                out.push(c);
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if open != end && c == open {
                depth += 1;
            } else if c == end {
                if depth == 0 {
                    return Some((out, i + 1));
                }
                depth -= 1;
            }
            out.push(c);
            i += 1;
        }
        None
    };
    let (pat, mut i) = part(1, d).ok_or_else(bad)?;
    let rep_open = if close(d) != d {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let o = *b.get(i).ok_or_else(bad)?;
        i += 1;
        o
    } else {
        d
    };
    let (rep, j) = part(i, rep_open).ok_or_else(bad)?;
    let flags = &expr[j..];
    if !flags.bytes().all(|f| b"msixpogcdualner".contains(&f)) {
        return Err(bad());
    }
    if flags.contains('e') {
        return Err(format!(
            "{expr}: a replacement run as Perl code (/e) is not in this port; use the Perl tes3cmd for it"
        ));
    }
    let mut rf = String::from("(?-u");
    for f in ['i', 's', 'm', 'x'] {
        if flags.contains(f) {
            rf.push(f);
        }
    }
    rf.push(')');
    let re = regex::bytes::Regex::new(&format!("{rf}{}", String::from_utf8_lossy(&pat)))
        .map_err(|_| bad())?;
    Ok(Subst {
        re,
        replacement: pieces(&rep),
        global: flags.contains('g'),
    })
}

/// A replacement as Perl interpolates it: `$1`, `${1}`, `$&`, and the escapes.
fn pieces(rep: &[u8]) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut text = Vec::new();
    let mut i = 0;
    while i < rep.len() {
        let c = rep[i];
        if c == b'\\' && i + 1 < rep.len() {
            let e = rep[i + 1];
            text.push(match e {
                b'n' => b'\n',
                b't' => b'\t',
                b'r' => b'\r',
                b'0' => 0,
                x => x,
            });
            i += 2;
            continue;
        }
        if c == b'$' && i + 1 < rep.len() {
            let (n, len) = if rep[i + 1] == b'&' {
                (Some(0), 2)
            } else if rep[i + 1] == b'{' {
                let end = rep[i + 2..]
                    .iter()
                    .position(|&x| x == b'}')
                    .map(|p| i + 2 + p);
                match end.and_then(|e| {
                    std::str::from_utf8(&rep[i + 2..e])
                        .ok()?
                        .parse::<usize>()
                        .ok()
                        .map(|n| (n, e))
                }) {
                    Some((n, e)) => (Some(n), e + 1 - i),
                    None => (None, 1),
                }
            } else {
                let digits = rep[i + 1..]
                    .iter()
                    .take_while(|d| d.is_ascii_digit())
                    .count();
                if digits > 0 {
                    (
                        std::str::from_utf8(&rep[i + 1..i + 1 + digits])
                            .ok()
                            .and_then(|s| s.parse().ok()),
                        1 + digits,
                    )
                } else {
                    (None, 1)
                }
            };
            if let Some(n) = n {
                if !text.is_empty() {
                    out.push(Piece::Text(std::mem::take(&mut text)));
                }
                out.push(Piece::Group(n));
                i += len;
                continue;
            }
        }
        text.push(c);
        i += 1;
    }
    if !text.is_empty() {
        out.push(Piece::Text(text));
    }
    out
}

impl Subst {
    /// `$value =~ s...`: the new value, when anything was replaced.
    pub fn apply(&self, v: &[u8]) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        let mut last = 0;
        let mut any = false;
        for c in self.re.captures_iter(v) {
            let m = c.get(0).unwrap();
            any = true;
            out.extend_from_slice(&v[last..m.start()]);
            for p in &self.replacement {
                match p {
                    Piece::Text(t) => out.extend_from_slice(t),
                    Piece::Group(n) => {
                        out.extend_from_slice(c.get(*n).map_or(&b""[..], |g| g.as_bytes()))
                    }
                }
            }
            last = m.end();
            if !self.global {
                break;
            }
        }
        any.then(|| {
            out.extend_from_slice(&v[last..]);
            out
        })
    }
}

pub struct ModifyOptions {
    /// Perl's eval counter: tes3cmd has run 24 when the first plugin starts; each plugin's check
    /// of the replacer is one more, and so is each `--run` on a record.
    pub evals: std::cell::Cell<u32>,
    pub wanted: Wanted,
    pub replace: Option<Subst>,
    pub sub_match: Option<regex::bytes::Regex>,
    pub sub_no_match: Option<regex::bytes::Regex>,
    pub output: Output,
}

/// `cmd_modify` with `--replace`: the substitution on every field of each wanted subrecord; a
/// record whose bytes changed is printed and written.
pub fn modify(
    ctx: &mut Ctx,
    o: &ModifyOptions,
    st: &mut State,
    inst: &mut Install,
    plugin: &str,
) -> Res<()> {
    let subs = o.sub_match.is_some() || o.sub_no_match.is_some();
    o.evals.set(o.evals.get() + 1);
    for_update(ctx, &o.output, inst, plugin, |ctx, r| {
        let (ok, _) = rec_match(ctx, st, &o.wanted, r).map_err(String::from)?;
        if !ok {
            return Ok(Outcome::Pass);
        }
        let Some(rep) = &o.replace else {
            // Without --replace tes3cmd runs its default `main($R);`, which nothing defines.
            let n = o.evals.get() + 1;
            o.evals.set(n);
            ctx.msg(&format!(
                "Error running \"main($R);\" on: {} (Undefined subroutine &main::main called at (eval {n}) line 1.\n)",
                r.rec.rectype_str()
            ));
            return Ok(Outcome::Pass);
        };
        let Some(d) = r.d.as_mut() else {
            return Ok(Outcome::Pass);
        };
        let mut modified = false;
        let ids = sub_ids(d);
        #[allow(clippy::needless_range_loop)] // d.subs is borrowed both ways in the body
        for k in 0..d.subs.len() {
            if subs {
                let t = {
                    let dd: &Decoded = d;
                    sub_text(ctx, st, dd, &dd.subs[k], ids[k].as_deref())?
                };
                let hit = o.sub_match.as_ref().is_none_or(|re| re.is_match(&t))
                    && o.sub_no_match.as_ref().is_none_or(|re| !re.is_match(&t));
                if !hit {
                    continue;
                }
            }
            match &mut d.subs[k].fields {
                Fields::Generic { slots, .. } => {
                    for v in slots.iter_mut() {
                        if !v.is_defined() {
                            continue;
                        }
                        if let Some(new) = rep.apply(&v.bytes()) {
                            *v = Val::Bytes(new);
                            modified = true;
                        }
                    }
                }
                Fields::OwnerFaction(name) => {
                    if let Some(new) = rep.apply(name) {
                        *name = new;
                        modified = true;
                    }
                }
                _ => {}
            }
        }
        if !modified {
            return Ok(Outcome::Pass);
        }
        let body = d.encode(ctx);
        if body == r.rec.buf {
            return Ok(Outcome::Pass);
        }
        let dd: &Decoded = d;
        let t = rec_text(ctx, st, dd)?;
        ctx.print_bytes(&[b"MODIFIED RECORD:\n", &t[..], b"\n"].concat());
        let flags = r.rec.hdrflags;
        Ok(Outcome::Changed(d.clone(), flags))
    })
}

// ---------------------------------------------------------------- diff

pub struct DiffOptions {
    pub ignore_types: Vec<String>,
    pub one_not_two: bool,
    pub two_not_one: bool,
    pub equal: bool,
    pub not_equal: bool,
    pub sortsubrecs: bool,
}

/// `read_objects`: each record by id, then type (the last of a type and id kept); ignored
/// records and those without an id left out.
fn objects(
    ctx: &mut Ctx,
    st: &mut State,
    plugin: &str,
) -> BTreeMap<String, BTreeMap<String, Decoded>> {
    let mut out: BTreeMap<String, BTreeMap<String, Decoded>> = BTreeMap::new();
    let data = match std::fs::read(plugin) {
        Ok(d) => d,
        Err(e) => {
            ctx.error(&format!(
                "FATAL ERROR ({plugin}): opening \"{plugin}\" for input ({e})\n"
            ));
            return out;
        }
    };
    let mut pos = 0;
    loop {
        match read_one(ctx, plugin, &data, &mut pos, None, Reader::Input) {
            Ok(Some(r)) => {
                if r.hdrflags & 0x1000 != 0 || !crate::record::known_type(&r.rectype) {
                    continue;
                }
                let d = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf);
                crate::tostr::note_dial(st, &d);
                if let Some(id) = d.id_opt() {
                    out.entry(id).or_default().insert(r.rectype_str(), d);
                }
            }
            Ok(None) => break,
            Err(e) => {
                ctx.error(&format!("{}\n", e.0));
                break;
            }
        }
    }
    out
}

/// `cmd_diff`. tes3cmd walks Perl hashes (whose order changes from run to run) and prints its
/// lists sorted; this walks them sorted too, so only the order of its messages can differ.
pub fn diff(ctx: &mut Ctx, o: &DiffOptions, st: &mut State, p1: &str, p2: &str) -> Res<()> {
    let ignored: Vec<String> = o
        .ignore_types
        .iter()
        .map(|t| t.split('.').next().unwrap_or("").to_string())
        .collect();
    let obj1 = objects(ctx, st, p1);
    let obj2 = objects(ctx, st, p2);
    if ctx.verbose {
        ctx.print(&format!("Plugin1: {p1}\nPlugin2: {p2}\n"));
    }
    let mastidx = regex::bytes::Regex::new(r"(?mi-u)^(\s*?\*?FRMR:.*?MastIdx:)\d+").unwrap();
    let mut p1_not_p2 = Vec::new();
    let mut p2_not_p1 = Vec::new();
    let mut equal = Vec::new();
    let mut differ = Vec::new();
    let (mut diff1, mut diff2): (Vec<Vec<u8>>, Vec<Vec<u8>>) = (Vec::new(), Vec::new());
    for (id, types) in &obj1 {
        for (rt, tr1) in types {
            if ignored.contains(rt) {
                continue;
            }
            let Some(tr2) = obj2.get(id).and_then(|t| t.get(rt)) else {
                if o.one_not_two {
                    p1_not_p2.push(format!("{rt}: {id}").into_bytes());
                }
                continue;
            };
            let text = |ctx: &mut Ctx, st: &mut State, d: &Decoded| -> Res<Vec<u8>> {
                let t = record_tostr(ctx, st, d).map_err(|m| crate::ctx::Abort(m.into()))?;
                Ok(if rt == "CELL" {
                    mastidx.replace(&t, &b"$1 [IGNORED]"[..]).into_owned()
                } else {
                    t
                })
            };
            let (t1, t2) = (text(ctx, st, tr1)?, text(ctx, st, tr2)?);
            let (c1, c2) = if o.sortsubrecs {
                let sorted = |t: &[u8]| {
                    let mut l: Vec<&[u8]> = t.split(|&c| c == b'\n').collect();
                    l.sort();
                    l.join(&b"\n"[..])
                };
                (sorted(&t1), sorted(&t2))
            } else {
                (t1.to_ascii_lowercase(), t2.to_ascii_lowercase())
            };
            let label = format!("{rt}: {id}");
            if c1 == c2 {
                if o.equal {
                    equal.push(label.into_bytes());
                }
            } else if o.not_equal {
                diff1.push([format!("\n{label}\n").as_bytes(), &t1, b"\n"].concat());
                diff2.push([format!("\n{label}\n").as_bytes(), &t2, b"\n"].concat());
                differ.push(label.into_bytes());
            }
        }
    }
    for (id, types) in &obj2 {
        for rt in types.keys() {
            if o.two_not_one && obj1.get(id).is_none_or(|t| !t.contains_key(rt)) {
                p2_not_p1.push(format!("{rt}: {id}").into_bytes());
            }
        }
    }
    let list = |ctx: &mut Ctx, head: String, mut v: Vec<Vec<u8>>| {
        v.sort();
        ctx.print(&head);
        for l in v {
            ctx.print_bytes(&[&l[..], b"\n"].concat());
        }
    };
    if o.one_not_two && !p1_not_p2.is_empty() {
        list(
            ctx,
            format!(
                "\nRecords in \"{p1}\" not in \"{p2}\" ({}):\n",
                p1_not_p2.len()
            ),
            p1_not_p2,
        );
    }
    if o.two_not_one && !p2_not_p1.is_empty() {
        list(
            ctx,
            format!(
                "\nRecords in \"{p2}\" not in \"{p1}\" ({}):\n",
                p2_not_p1.len()
            ),
            p2_not_p1,
        );
    }
    if o.equal && !equal.is_empty() {
        list(
            ctx,
            format!(
                "\nRecords that are equal in \"{p1}\" and \"{p2}\" ({}):\n",
                equal.len()
            ),
            equal,
        );
    }
    if o.not_equal {
        let (f1, f2) = (format!("{p1}-diff.txt"), format!("{p2}-diff.txt"));
        if !differ.is_empty() {
            let head = format!(
                "\nRecords that are different between \"{p1}\" and \"{p2}\" ({}):\n(Compare {f1} to {f2})\n",
                differ.len()
            );
            list(ctx, head, differ);
        }
        diff1.sort();
        diff2.sort();
        ctx.print("\n");
        for (file, lines) in [(f1, diff1), (f2, diff2)] {
            if lines.is_empty() {
                continue;
            }
            match std::fs::write(&file, lines.join(&b"\n"[..])) {
                Ok(()) => ctx.print(&format!("Created diff file: {file}\n")),
                Err(e) => ctx.error(&format!("Opening {file} for output ({e})")),
            }
        }
    }
    Ok(())
}
