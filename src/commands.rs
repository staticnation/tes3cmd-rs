//! The commands besides `clean`, as tes3cmd's `cmd_*` subs do them: `header`, `esp`/`esm`,
//! `common`, `active`, `resetdates`, `overdial` and `recover`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::ctx::{Abort, Ctx, Res};
use crate::datadir::Install;
use crate::decode::{Decoded, Fields};
use crate::output::{Output, unique_backup_name};
use crate::pack::Val;
use crate::record::{Reader, Record, known_type, read_one};

/// `%ORIGINAL_DATE`: Bethesda's files, at the dates they shipped with.
const ORIGINAL_DATE: &[(&str, i64)] = &[
    ("bloodmoon.bsa", 1051807050),
    ("bloodmoon.esm", 1051807050),
    ("morrowind.bsa", 1024695106),
    ("morrowind.esm", 1024695106),
    ("tribunal.bsa", 1035940926),
    ("tribunal.esm", 1035940926),
];

/// `$HDR_FLAGS{ignored}`.
const HDR_IGNORED: u32 = 0x1000;

fn times(path: &str) -> Option<(std::time::SystemTime, std::time::SystemTime)> {
    let m = crate::fsx::metadata(path).ok()?;
    Some((m.accessed().ok()?, m.modified().ok()?))
}

fn set_times(path: &str, t: Option<(std::time::SystemTime, std::time::SystemTime)>) {
    if let Some((a, m)) = t
        && let Ok(w) = crate::fsx::at_write(path, true)
        && let Ok(f) = std::fs::File::options().write(true).open(w)
    {
        let _ = f.set_times(std::fs::FileTimes::new().set_accessed(a).set_modified(m));
    }
}

/// Every record of a plugin as `new_from_input` reads it, the TES3 header first: an error
/// stops the read and is reported as tes3cmd's `eval` reports it (`err($@)`).
fn records(ctx: &mut Ctx, plugin: &str) -> Res<Vec<Record>> {
    let data = crate::fsx::read(plugin)
        .map_err(|e| ctx.abort(&format!("opening \"{plugin}\" for input ({e})")))?;
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        match read_one(ctx, plugin, &data, &mut pos, None, Reader::Input) {
            Ok(Some(r)) => out.push(r),
            Ok(None) => break,
            Err(e) => {
                ctx.error(&e.0);
                break;
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- header

pub struct HeaderOptions {
    pub author: Option<String>,
    pub description: Option<String>,
    pub multiline: bool,
    pub update_masters: bool,
    pub update_record_count: bool,
    pub output: Output,
}

/// `s!\000.*$!!`: the string cut at the first NUL that only non-newlines (and at most one
/// final newline) follow, as Perl's `.` and `$` match.
fn cut_at_nul(s: &[u8]) -> Vec<u8> {
    for (i, &b) in s.iter().enumerate() {
        if b != 0 {
            continue;
        }
        let rest = &s[i + 1..];
        let nl = rest.iter().filter(|&&c| c == b'\n').count();
        if nl == 0 {
            return s[..i].to_vec();
        }
        if nl == 1 && rest.last() == Some(&b'\n') {
            let mut v = s[..i].to_vec();
            v.push(b'\n');
            return v;
        }
    }
    s.to_vec()
}

fn trim_nuls(s: &[u8]) -> Vec<u8> {
    let end = s.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    s[..end].to_vec()
}

fn text(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// `printf("%0.2f")` then `s/0+$//`, as HEDR's tostr shows the version.
fn version_str(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').to_string()
}

fn hedr_vals(s: &crate::decode::Sub) -> Option<&Vec<Val>> {
    match &s.fields {
        Fields::Generic { slots, .. } => Some(slots),
        _ => None,
    }
}

/// HEDR's own `tostr`.
fn hedr_tostr(slots: &[Val]) -> String {
    let mut desc = trim_nuls(&slots[3].bytes());
    if cfg!(target_os = "linux") {
        desc.retain(|&c| c != b'\r');
    }
    let author = trim_nuls(&slots[2].bytes());
    format!(
        "  Version:{}  Is_Master:{}  Author:\"{}\"\n  Description:\"{}\"\n  N_Records:{}",
        version_str(slots[0].num()),
        if slots[1].num() != 0.0 {
            "True"
        } else {
            "False"
        },
        text(&author),
        text(&desc),
        slots[4].num() as i64
    )
}

/// HEDR's `tostr` as bytes, for `tostr`.
pub fn hedr_tostr_bytes(f: &Fields) -> Vec<u8> {
    match f {
        Fields::Generic { slots, .. } if slots.len() >= 5 => {
            hedr_tostr(slots).chars().map(|c| c as u32 as u8).collect()
        }
        _ => Vec::new(),
    }
}

/// The TES3 record's `tostr`: its banner, then each subrecord.
fn tes3_tostr(hdrflags: u32, d: &Decoded) -> String {
    let mut s = format!("Record: TES3 \"()\" Flags:{}\n", hdr_flags_str(hdrflags));
    let lines: Vec<String> = d
        .subs
        .iter()
        .map(|sub| {
            let body = match (&sub.tag, &sub.fields) {
                (b"HEDR", Fields::Generic { slots, .. }) => hedr_tostr(slots),
                (b"MAST", Fields::Generic { slots, .. }) => {
                    format!("Master:{}", text(&slots[0].bytes()))
                }
                (_, Fields::MasterSize(n)) => format!("Length:{}", crate::pack::perl_num_str(*n)),
                _ => String::new(),
            };
            let margin = if crate::decode::starts_group(&d.rectype, &sub.tag) {
                " *"
            } else {
                "  "
            };
            format!("{margin}{}: {body}", sub.tag_str())
        })
        .collect();
    s.push_str(&lines.join("\n"));
    s
}

/// `flags_tostr($flags, \%HDR_FLAGS)`.
pub fn hdr_flags_str(flags: u32) -> String {
    let mut names: Vec<&str> = [
        ("Blocked", 0x2000),
        ("Deleted", 0x20),
        ("Ignored", 0x1000),
        ("Persistent", 0x400),
    ]
    .iter()
    .filter(|(_, v)| flags & v != 0)
    .map(|(n, _)| *n)
    .collect();
    names.sort();
    format!("0x{flags:04x} ({})", names.join(", "))
}

/// `update_header($plugin, $options)`: `quiet` and `nobackup` are `QUIET` and `NOBACKUP`.
pub fn update_header(
    ctx: &mut Ctx,
    o: &HeaderOptions,
    inst: &mut Install,
    plugin: &str,
    quiet: bool,
    nobackup: bool,
) -> Res<()> {
    let lower = plugin.to_ascii_lowercase();
    if (o.update_record_count || o.update_masters) && crate::header::is_backup_name(&lower, false) {
        ctx.print(&format!("tes3cmd header skipping Backup: {plugin}\n"));
        return Ok(());
    }
    let t = times(plugin);
    let mut data = crate::fsx::read(plugin)
        .map_err(|e| ctx.abort(&format!("opening \"{plugin}\" for read/write ({e})")))?;
    let mut pos = 0;
    let rec = match read_one(ctx, plugin, &data, &mut pos, Some("TES3"), Reader::Input)? {
        Some(r) => r,
        None => {
            ctx.error(&format!("Error reading TES3 header from {plugin}"));
            return Ok(());
        }
    };
    let tes3len = rec.buf.len();
    let mut d = Decoded::new(ctx, rec.rectype, rec.hdrflags, &rec.buf);
    let mut update_msg = String::new();
    let mut modified = 0;
    let mut master: Option<Vec<u8>> = None;
    let mut hedr_at = None;
    for i in 0..d.subs.len() {
        let tag = d.subs[i].tag;
        if &tag == b"HEDR" {
            hedr_at = Some(i);
            if let Fields::Generic { slots, .. } = &mut d.subs[i].fields {
                if let Some(a) = &o.author {
                    modified += 1;
                    let mut cur = trim_nuls(&slots[2].bytes());
                    match a.strip_prefix('+') {
                        Some(more) => cur.extend_from_slice(more.as_bytes()),
                        None => cur = a.as_bytes().to_vec(),
                    }
                    slots[2] = Val::Bytes(cur);
                }
                if let Some(desc) = &o.description {
                    modified += 1;
                    let desc = desc.replace("\\n", "\n").replace("\\r", "\r");
                    let mut cur = trim_nuls(&slots[3].bytes());
                    match desc.strip_prefix('+') {
                        Some(more) => cur.extend_from_slice(more.as_bytes()),
                        None => cur = desc.as_bytes().to_vec(),
                    }
                    slots[3] = Val::Bytes(cur);
                }
                if o.update_record_count {
                    let count = crate::header::record_count(ctx, plugin, &data);
                    let have = slots[4].num() as i64;
                    if have != count as i64 {
                        modified += 1;
                        update_msg.push_str(&format!(
                            "  {plugin}: Updated Record Count: {have} --> {count}\n"
                        ));
                        slots[4] = Val::Int(count as i64);
                    }
                }
            }
        }
        if o.update_masters {
            if &tag == b"MAST" {
                if let Fields::Generic { slots, .. } = &d.subs[i].fields {
                    master = Some(slots[0].bytes());
                }
            } else if &tag == b"DATA" {
                if let Fields::MasterSize(length) = d.subs[i].fields {
                    match &master {
                        Some(m) => {
                            let newlen = inst.datapath(&text(m)).and_then(|p| crate::fsx::metadata(p).ok()).map(|m| m.len());
                            if newlen.map(|n| n as f64) != Some(length) {
                                modified += 1;
                                update_msg.push_str(&format!(
                                    "  {plugin}: Synchronized master \"{}\" length: {} --> {}\n",
                                    text(m),
                                    crate::pack::perl_num_str(length),
                                    newlen.map(|n| n.to_string()).unwrap_or_default()
                                ));
                                d.subs[i].fields = Fields::MasterSize(newlen.unwrap_or(0) as f64);
                            }
                        }
                        None => ctx.error("Found DATA (size of master) but no corresponding MAST (name of master)"),
                    }
                }
            }
        }
    }
    if !quiet {
        if ctx.verbose {
            ctx.print(&format!("{plugin}:\n{}\n", tes3_tostr(rec.hdrflags, &d)));
        } else if !(o.author.is_some()
            || o.description.is_some()
            || o.update_record_count
            || o.update_masters)
            || o.author.is_some()
            || o.description.is_some()
        {
            let slots = hedr_at
                .and_then(|i| hedr_vals(&d.subs[i]))
                .cloned()
                .unwrap_or_default();
            if slots.len() >= 5 {
                if o.multiline {
                    ctx.print(&format!("{plugin}:\n{}\n", hedr_tostr(&slots)));
                } else {
                    let auth = text(&cut_at_nul(&slots[2].bytes()));
                    let desc = text(&cut_at_nul(&slots[3].bytes()));
                    let line = format!("{plugin}: AUTH=\"{auth}\"  DESC=\"{desc}\"");
                    let line = line.replace("\r\n", "\\r\\n").replace('\n', "\\r\\n");
                    ctx.print(&format!("{line}\n"));
                }
            }
        }
        if !update_msg.is_empty() {
            ctx.print(&update_msg);
        } else if ctx.verbose {
            ctx.print(&format!("{plugin} not modified\n"));
        }
    }
    if modified > 0 {
        if !nobackup {
            let backup = unique_backup_name(&o.output, inst, plugin)?;
            crate::fsx::copy(plugin, &backup).map_err(|e| {
                ctx.abort(&format!("Error, copy failed {plugin} -> {backup} ({e})"))
            })?;
        }
        let body = d.encode(ctx);
        if body.len() == tes3len {
            data[16..16 + tes3len].copy_from_slice(&body);
            crate::fsx::write(plugin, &data)
                .map_err(|e| ctx.abort(&format!("opening \"{plugin}\" for read/write ({e})")))?;
        } else {
            return Err(ctx.abort(&format!(
                "update_header(): output header size incorrect (expected:{tes3len}, got: {})",
                body.len()
            )));
        }
    }
    set_times(plugin, t);
    Ok(())
}

// ---------------------------------------------------------------- esp / esm

/// `esp_esm_convert`: a copy with the other extension and the master byte flipped.
pub fn esp_esm(ctx: &mut Ctx, input: &str, overwrite: bool) -> Res<()> {
    let lower = input.to_ascii_lowercase();
    let (expected, new, output) = if lower.ends_with(".esp") {
        (0u8, 1u8, format!("{}.esm", &input[..input.len() - 4]))
    } else {
        (1u8, 0u8, format!("{}.esp", &input[..input.len() - 4]))
    };
    if crate::fsx::at(&output).is_file() && !overwrite {
        return Err(ctx.abort(&format!(
            "Error, {output} already exists! (Use --overwrite to overwrite)"
        )));
    }
    crate::fsx::copy(input, &output)
        .map_err(|e| ctx.abort(&format!("Error, copy failed {input} -> {output} ({e})")))?;
    let fail = |ctx: &mut Ctx, m: String| -> Res<()> {
        // err($@): the croak's message keeps its newline inside the brackets.
        ctx.error(&format!("FATAL ERROR ({}): {m}\n", ctx.plugin));
        let _ = crate::fsx::remove_file(&output);
        Ok(())
    };
    let mut data = match crate::fsx::read(&output) {
        Ok(d) => d,
        Err(e) => {
            return fail(
                ctx,
                format!("Error opening \"{output}\" for read/write ({e})"),
            );
        }
    };
    if data.len() < 4 {
        return fail(ctx, format!("{output}: Error reading magic (Success)"));
    }
    match &data[0..4] {
        b"TES3" => {
            if data.len() < 29 {
                return fail(
                    ctx,
                    format!("{output}: Error reading master byte (Success)"),
                );
            }
            if data[28] != expected {
                return fail(
                    ctx,
                    format!(
                        "Error, expected master byte value of {expected}, got {} instead",
                        data[28]
                    ),
                );
            }
            data[28] = new;
            if let Err(e) = crate::fsx::write(&output, &data) {
                return fail(ctx, format!("{output}: Error writing byte ({e})"));
            }
        }
        b"TES4" => {
            return fail(
                ctx,
                "This function is not yet implemented for TES4 files".into(),
            );
        }
        _ => {
            return fail(
                ctx,
                format!("{output}: Error, this does not appear to be Morrowind plugin"),
            );
        }
    }
    set_times(&output, times(input));
    ctx.print(&format!("\"{input}\" copied to \"{output}\"\n"));
    Ok(())
}

// ---------------------------------------------------------------- common

/// `read_records` without a callback: rectype -> ids.
fn record_ids(ctx: &mut Ctx, plugin: &str) -> Res<HashMap<String, BTreeSet<String>>> {
    let mut out: HashMap<String, BTreeSet<String>> = HashMap::new();
    for r in records(ctx, plugin)? {
        if r.hdrflags & HDR_IGNORED != 0 {
            continue;
        }
        let id = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf).id();
        out.entry(r.rectype_str()).or_default().insert(id);
    }
    Ok(out)
}

/// `cmd_common`: the ids of the smaller plugin found again in the larger, in its order.
pub fn common(ctx: &mut Ctx, p1: &str, p2: &str) -> Res<()> {
    let size = |p: &str| crate::fsx::metadata(p).map(|m| m.len()).unwrap_or(0);
    let (p1, p2) = if size(p1) > size(p2) {
        (p2, p1)
    } else {
        (p1, p2)
    };
    let first = record_ids(ctx, p1)?;
    for r in records(ctx, p2)? {
        if r.hdrflags & HDR_IGNORED != 0 {
            continue;
        }
        let rt = r.rectype_str();
        let id = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf).id();
        if rt != "TES3" && first.get(&rt).is_some_and(|ids| ids.contains(&id)) {
            ctx.print(&format!("  {rt}: {id}\n"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- active / resetdates

fn ini_path(inst: &Install) -> String {
    crate::datadir::list_files(&inst.mwdir)
        .and_then(|m| m.get("morrowind.ini").cloned())
        .map(|n| format!("{}/{n}", inst.mwdir))
        .unwrap_or_else(|| format!("{}/Morrowind.ini", inst.mwdir))
}

/// `read_gamefiles`.
fn read_gamefiles(ctx: &mut Ctx, inst: &Install) -> Res<Vec<String>> {
    let ini = ini_path(inst);
    let raw = crate::fsx::read(&ini)
        .map_err(|e| ctx.abort(&format!("opening \"{ini}\" for input ({e})")))?;
    let text: String = text(&raw).replace("\r\n", "\n");
    let mut out = Vec::new();
    let mut lines = text.split_inclusive('\n');
    for l in lines.by_ref() {
        if l.len() >= 12 && l[..12].eq_ignore_ascii_case("[Game Files]") {
            break;
        }
    }
    for l in lines {
        let l = l.strip_suffix('\n').unwrap_or(l);
        if let Some(rest) = l.strip_prefix("GameFile") {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 0 && rest[digits..].starts_with('=') && rest.len() > digits + 1 {
                out.push(rest[digits + 1..].to_string());
                continue;
            }
        }
        if l.starts_with('[') && l.len() > 2 && l[1..].contains(']') {
            break;
        }
    }
    Ok(out)
}

/// `write_gamefiles`: the ini rewritten with a new [Game Files] section, the old one kept as
/// `Morrowind.ini.old`.
fn write_gamefiles(ctx: &mut Ctx, inst: &Install, files: &[String]) -> Res<()> {
    let ini = ini_path(inst);
    let raw = crate::fsx::read(&ini)
        .map_err(|e| ctx.abort(&format!("opening \"{ini}\" for input ({e})")))?;
    let text: String = text(&raw).replace("\r\n", "\n");
    let mut out = String::new();
    let mut lines = text.split_inclusive('\n');
    for l in lines.by_ref() {
        out.push_str(l);
        if l.len() >= 12 && l[..12].eq_ignore_ascii_case("[Game Files]") {
            break;
        }
    }
    for (i, f) in files.iter().enumerate() {
        out.push_str(&format!("GameFile{i}={f}\n"));
    }
    for l in lines.by_ref() {
        if l.starts_with('[') && l[1..].contains(']') {
            out.push_str(l);
            break;
        }
    }
    for l in lines {
        out.push_str(l);
    }
    let crlf = out.replace('\n', "\r\n");
    let bytes: Vec<u8> = crlf.chars().map(|c| c as u32 as u8).collect();
    let tmp = format!("{ini}.tmp");
    crate::fsx::write(&tmp, bytes)
        .map_err(|e| ctx.abort(&format!("opening \"{tmp}\" for output ({e})")))?;
    let old = format!("{ini}.old");
    let _ = crate::fsx::remove_file(&old);
    crate::fsx::rename(&ini, &old)
        .map_err(|e| ctx.abort(&format!("Error renaming \"{ini}\" to \"{old}\" ({e})")))?;
    crate::fsx::rename(&tmp, &ini)
        .map_err(|e| ctx.abort(&format!("Error renaming \"{tmp}\" to \"{ini}\" ({e})")))?;
    Ok(())
}

/// `cmd_active`.
pub fn active(
    ctx: &mut Ctx,
    inst: &mut Install,
    on: bool,
    off: bool,
    plugins: &[String],
) -> Res<()> {
    if on || off {
        let mut g: BTreeMap<String, String> = read_gamefiles(ctx, inst)?
            .into_iter()
            .map(|f| (f.to_lowercase(), f))
            .collect();
        for p in plugins {
            let k = p.to_lowercase();
            if on && !g.contains_key(&k) {
                ctx.print(&format!("ACTIVATED: {p}\n"));
                g.insert(k, p.clone());
            } else if !on && g.contains_key(&k) {
                ctx.print(&format!("DEACTIVATED: {p}\n"));
                g.remove(&k);
            }
        }
        let mut files: Vec<String> = g.into_values().collect();
        files.sort();
        write_gamefiles(ctx, inst, &files)
    } else {
        ctx.print("[LOAD ORDER]\n");
        let lo = inst.load_order();
        for p in &lo {
            ctx.print(&format!("{p}\n"));
        }
        ctx.print(&format!("[{} Active Plugins]\n", lo.len()));
        Ok(())
    }
}

/// C's `struct tm`: the nine ints every platform has first (glibc and the BSDs add a zone
/// after them, which is room enough here).
#[repr(C)]
#[derive(Default)]
struct Tm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    _rest: [i64; 4],
}

#[cfg(unix)]
unsafe extern "C" {
    fn localtime_r(t: *const i64, tm: *mut Tm) -> *mut Tm;
}
#[cfg(windows)]
unsafe extern "C" {
    fn _localtime64_s(tm: *mut Tm, t: *const i64) -> i32;
}

/// Perl's `scalar(localtime($t))`: `Fri Jun 21 17:31:46 2002`, in the local zone.
fn ctime(t: i64) -> String {
    let mut tm = Tm::default();
    // SAFETY: both fill the `tm` they are given from a time read through the pointer, and
    // hold neither pointer past the call.
    #[cfg(unix)]
    unsafe {
        localtime_r(&t, &mut tm);
    }
    #[cfg(windows)]
    unsafe {
        _localtime64_s(&mut tm, &t);
    }
    let wd = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][tm.wday.rem_euclid(7) as usize];
    let mon = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][tm.mon.rem_euclid(12) as usize];
    format!(
        "{wd} {mon} {:>2} {:02}:{:02}:{:02} {}",
        tm.mday,
        tm.hour,
        tm.min,
        tm.sec,
        tm.year + 1900
    )
}

/// `cmd_resetdates`.
pub fn resetdates(ctx: &mut Ctx, inst: &mut Install) -> Res<()> {
    for (name, orig) in ORIGINAL_DATE {
        // A file that is not there has no mtime, which is never the original date.
        let path = inst
            .datapath(name)
            .map(|p| p.to_string_lossy().into_owned());
        let mtime = path
            .as_ref()
            .and_then(|p| crate::fsx::metadata(p).and_then(|m| m.modified()).ok())
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        if path.is_none() {
            ctx.msg("Use of uninitialized value $file_path in stat");
            ctx.msg("Use of uninitialized value $mtime in numeric ne (!=)");
        }
        if mtime != Some(*orig) {
            ctx.print(&format!("Reset Date of: {name} to: {}\n", ctime(*orig)));
            if path.is_none() {
                ctx.msg("Use of uninitialized value $file_path in utime");
            }
            if let Some(p) = &path {
                let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(*orig as u64);
                set_times(p, Some((t, t)));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- overdial

/// `read_dialogs`: each topic (type 0) DIAL id of a plugin, counted.
fn dialogs(ctx: &mut Ctx, plugin: &str) -> Res<Vec<String>> {
    let mut out = Vec::new();
    for r in records(ctx, plugin)? {
        if r.hdrflags & HDR_IGNORED != 0 || &r.rectype != b"DIAL" {
            continue;
        }
        let d = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf);
        let id = d.id();
        // DATA's own decoder: `unpack("C")`, undefined (so `== 0`) when it is missing or empty.
        let ty = d.get(b"DATA").and_then(|s| s.raw.first().copied());
        if ty.unwrap_or(0) == 0 && !id.is_empty() && id != "0" && !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// `$id1 =~ /\b$id2\b/i`, with `$id2` taken as a pattern as tes3cmd takes it: its regex
/// metacharacters are matched literally here (a topic id that is not a valid regex dies in
/// tes3cmd).
fn word_match(id1: &str, id2: &str) -> bool {
    let (a, b) = (id1.to_lowercase(), id2.to_lowercase());
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let ab: Vec<char> = a.chars().collect();
    let bb: Vec<char> = b.chars().collect();
    if bb.is_empty() || bb.len() > ab.len() {
        return false;
    }
    for i in 0..=ab.len() - bb.len() {
        if ab[i..i + bb.len()] != bb[..] {
            continue;
        }
        let before = if i == 0 { None } else { Some(ab[i - 1]) };
        let after = ab.get(i + bb.len()).copied();
        let start_ok = word(before) != word(bb.first().copied());
        let end_ok = word(after) != word(bb.last().copied());
        if start_ok && end_ok {
            return true;
        }
    }
    false
}

/// `cmd_overdial`. tes3cmd walks hash keys, whose order Perl randomizes; this walks them sorted.
pub fn overdial(ctx: &mut Ctx, plugins: &[String], single: bool) -> Res<()> {
    let mut dialog: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in plugins {
        ctx.plugin = p.clone();
        let d = dialogs(ctx, p)?;
        if !d.is_empty() {
            dialog.insert(p.clone(), d);
        }
    }
    let test: Vec<String> = if single {
        plugins.first().cloned().into_iter().collect()
    } else {
        dialog.keys().cloned().collect()
    };
    for (p1, ids1) in &dialog {
        for p2 in &test {
            if p1 == p2 {
                continue;
            }
            let Some(ids2) = dialog.get(p2) else { continue };
            for id1 in ids1 {
                for id2 in ids2 {
                    if id1.chars().count() > id2.chars().count() && word_match(id1, id2) {
                        ctx.print(&format!(
                            "{:<40} \"{id1}\"\n{:<40} \"{id2}\"\n\n",
                            format!("{p1}:"),
                            format!("{p2}:")
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- recover

/// `cmd_recover`: the records that read and decode, the bad stretches between them dropped.
pub fn recover(ctx: &mut Ctx, o: &Output, inst: &mut Install, plugin: &str) -> Res<()> {
    if !crate::clean::has_plugin_ext(plugin) {
        return Err(ctx.abort(&format!("\"{plugin}\" does not end in .esm/.esp/.ess")));
    }
    if !crate::fsx::at(plugin).is_file() {
        return Err(ctx.abort("Invalid input file (No such file or directory)"));
    }
    let data = crate::fsx::read(plugin)
        .map_err(|e| ctx.abort(&format!("opening \"{plugin}\" for input ({e})")))?;
    let tmp = format!("{plugin}.tmp");
    let mut out = Vec::new();
    let len = data.len();
    ctx.print(&format!("{plugin}: length = {len}\n"));
    let mut offset = 0usize;
    let mut expected = Some("TES3");
    let mut removed: Vec<(usize, usize)> = Vec::new();
    let mut recovered = 0;
    'read: while offset < len {
        let mut pos = offset;
        let attempt: Result<Option<Record>, Abort> = (|| {
            let Some(r) = read_one(ctx, plugin, &data, &mut pos, expected, Reader::Input)? else {
                return Ok(None);
            };
            if !known_type(&r.rectype) {
                return Err(ctx.abort(&format!("Error, Invalid record type: {}", r.rectype_str())));
            }
            let d = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf);
            for s in &d.subs {
                if !crate::decode::known_sub(&r.rectype, &s.tag) {
                    return Err(Abort(format!(
                        "Error, Invalid record subtype: \"{}.{}\"",
                        r.rectype_str(),
                        s.tag_str()
                    )));
                }
            }
            if ctx.debug {
                let id = d.id();
                ctx.print(&format!(
                    "Offset: {offset:>8}  Found: {} {id}\n",
                    r.rectype_str()
                ));
            }
            Ok(Some(r))
        })();
        match attempt {
            Ok(None) => break,
            Ok(Some(r)) => {
                expected = None;
                recovered += 1;
                r.write(&mut out);
                offset = pos;
            }
            Err(e) => {
                expected = None;
                ctx.dbg(&e.0);
                let start = offset;
                ctx.print(&format!("READ ERROR on record starting at: {start}\n"));
                offset = (start + 1).min(len);
                ctx.print(&format!("Scanning for records from: {offset}\n"));
                // The first known record type from here on.
                let found = (offset..len.saturating_sub(3))
                    .find(|&i| known_type(data[i..i + 4].try_into().unwrap()));
                match found {
                    Some(at) => {
                        ctx.print(&format!(
                            "Scan found {} at: {at}\n",
                            text(&data[at..at + 4])
                        ));
                        removed.push((start, at));
                        offset = at;
                    }
                    None => break 'read,
                }
            }
        }
    }
    if recovered > 0 && !removed.is_empty() {
        let n = removed.len();
        ctx.print(&format!(
            "Removed {n} section{} of bad data:\n",
            if n > 1 { "s" } else { "" }
        ));
        let mut total = 0;
        for (a, b) in &removed {
            let lost = b - a;
            ctx.print(&format!("  From: {a:>8} To: {b:>8}  (lost {lost} bytes)\n"));
            total += lost;
        }
        if n > 1 {
            ctx.print(&format!("Total bytes lost: {total}\n"));
        }
        crate::fsx::write(&tmp, &out)
            .map_err(|e| ctx.abort(&format!("opening \"{tmp}\" for output ({e})")))?;
        crate::output::fix_output(
            ctx,
            o,
            inst,
            plugin,
            true,
            None,
            crate::header::update_record_count,
        )?;
    } else {
        ctx.print(&format!("Unable to recover data from {plugin}\n"));
        let _ = crate::fsx::remove_file(&tmp);
    }
    Ok(())
}

// ---------------------------------------------------------------- dump

/// The record selectors (`get_wanted`, `rec_match`).
#[derive(Default)]
pub struct Wanted {
    pub types: Option<BTreeSet<String>>,
    pub ids: Option<regex::bytes::Regex>,
    pub flags: u32,
    pub interior: bool,
    pub exterior: bool,
    pub matches: Vec<regex::bytes::Regex>,
    pub no_matches: Vec<regex::bytes::Regex>,
}

/// A Perl pattern as Rust's regex takes it: `/i` (and `/s`), on bytes.
pub fn perl_re(ctx: &Ctx, pat: &str, dotall: bool) -> Res<regex::bytes::Regex> {
    let flags = if dotall { "(?is-u)" } else { "(?i-u)" };
    regex::bytes::Regex::new(&format!("{flags}{pat}"))
        .map_err(|e| ctx.abort(&format!("Invalid regular expression: {pat} ({e})")))
}

pub struct WantedArgs<'a> {
    pub id: &'a [String],
    pub exact_id: &'a [String],
    pub flag: &'a [String],
    pub types: &'a [String],
    pub interior: bool,
    pub exterior: bool,
    pub instance: bool,
    pub matches: &'a [String],
    pub no_matches: &'a [String],
}

/// `get_wanted`.
pub fn get_wanted(ctx: &mut Ctx, a: &WantedArgs) -> Res<Wanted> {
    let mut w = Wanted {
        interior: a.interior,
        exterior: a.exterior,
        ..Default::default()
    };
    let mut ids: Vec<String> = a.id.to_vec();
    ids.extend(a.exact_id.iter().map(|e| format!("^{}$", regex::escape(e))));
    if !ids.is_empty() {
        w.ids = Some(perl_re(ctx, &format!("({})", ids.join("|")), false)?);
    }
    for f in a.flag {
        let low = f.to_lowercase();
        let num = if let Some(h) = low.strip_prefix("0x") {
            u32::from_str_radix(h, 16).ok()
        } else if let Some(b) = low.strip_prefix("0b") {
            u32::from_str_radix(b, 2).ok()
        } else if let Some(o) = low.strip_prefix("0o") {
            u32::from_str_radix(o, 8).ok()
        } else if low.len() > 1 && low.starts_with('0') && low.bytes().all(|c| c.is_ascii_digit()) {
            u32::from_str_radix(&low[1..], 8).ok()
        } else {
            low.parse::<u32>().ok()
        };
        w.flags |= match num {
            Some(n) => n,
            None => match low.as_str() {
                "deleted" | "d" => 0x20,
                "persistent" | "p" => 0x400,
                "ignored" | "i" => 0x1000,
                "blocked" | "b" => 0x2000,
                _ => 0,
            },
        };
    }
    let mut types: Vec<String> = a.types.to_vec();
    if a.instance || a.interior || a.exterior {
        if !types.is_empty() && types.join(" ").to_uppercase() != "CELL" {
            if a.exterior || a.interior {
                ctx.msg("Specifying --interior/--exterior assumes --type CELL");
            }
            if a.instance {
                ctx.msg("Specifying --instance-match/--instance-no-match assumes --type CELL");
            }
        }
        types = vec!["cell".into()];
    }
    if !types.is_empty() {
        let mut set = BTreeSet::new();
        for t in &types {
            let up = t.to_uppercase();
            let (mut rt, st) = match up.split_once('.') {
                Some((r, s)) => (r.to_string(), Some(s.to_string())),
                None => (up.clone(), None),
            };
            if rt == "NPC" {
                ctx.msg("[Using \"NPC\" as shorthand for \"NPC_\" record type.]");
                rt = "NPC_".into();
            }
            if rt.len() != 4 {
                return Err(ctx.abort(&format!(
                    "get_wanted_types: Invalid record type: \"{rt}\" (must be 4 characters long)"
                )));
            }
            set.insert(rt.clone());
            if let Some(st) = st.filter(|s| !s.is_empty()) {
                if st.len() != 4 {
                    return Err(ctx.abort(&format!("get_wanted_types: Invalid record subtype: \"{st}\" (must be 4 characters long)")));
                }
                set.insert(format!("{rt}.{st}"));
            }
        }
        w.types = Some(set);
    }
    for m in a.matches {
        w.matches.push(perl_re(ctx, m, true)?);
    }
    for m in a.no_matches {
        w.no_matches.push(perl_re(ctx, m, true)?);
    }
    Ok(w)
}

/// A record read for printing: decoded, unless tes3cmd does not know its type.
pub struct Rec {
    pub rec: Record,
    pub d: Option<Decoded>,
}

/// `rec_match`: whether the record is wanted, and its text when the selectors needed it.
pub fn rec_match(
    ctx: &mut Ctx,
    st: &mut crate::tostr::State,
    w: &Wanted,
    r: &mut Rec,
) -> Result<(bool, Option<Vec<u8>>), &'static str> {
    let known = known_type(&r.rec.rectype);
    let decode = |ctx: &mut Ctx, r: &mut Rec, st: &mut crate::tostr::State| {
        if r.d.is_none() && known {
            let d = Decoded::new(ctx, r.rec.rectype, r.rec.hdrflags, &r.rec.buf);
            crate::tostr::note_dial(st, &d);
            r.d = Some(d);
        }
    };
    if &r.rec.rectype == b"DIAL" {
        decode(ctx, r, st);
    }
    if w.flags != 0 && r.rec.hdrflags & w.flags == 0 {
        return Ok((false, None));
    }
    if let Some(t) = &w.types
        && !t.contains(&r.rec.rectype_str())
    {
        return Ok((false, None));
    }
    decode(ctx, r, st);
    let id =
        r.d.as_ref()
            .map(crate::tostr::record_id)
            .unwrap_or_else(|| "(unknown)".into());
    if let Some(re) = &w.ids
        && !re.is_match(id.as_bytes())
    {
        return Ok((false, None));
    }
    if &r.rec.rectype == b"CELL"
        && let Some(d) = &r.d
        && (d.is_interior() && w.exterior || !d.is_interior() && w.interior)
    {
        return Ok((false, None));
    }
    if w.matches.is_empty() && w.no_matches.is_empty() {
        return Ok((true, None));
    }
    let text = match &r.d {
        Some(d) => crate::tostr::record_tostr(ctx, st, d)?,
        None => Vec::new(),
    };
    if w.no_matches.iter().any(|re| re.is_match(&text))
        || w.matches.iter().any(|re| !re.is_match(&text))
    {
        return Ok((false, Some(text)));
    }
    Ok((true, Some(text)))
}

pub struct DumpOptions {
    /// `--format`: each line with its `%field%`s made `%s`, and the fields.
    pub formats: Vec<(Vec<u8>, Vec<String>)>,
    /// `--no-quote`.
    pub no_quote: bool,
    /// `--binary`: the records written to this file (appended to what is there) instead of printed.
    pub raw_to: Option<String>,
    pub wanted: Wanted,
    pub list: bool,
    pub no_banner: bool,
    pub instance_match: Option<regex::bytes::Regex>,
    pub instance_no_match: Option<regex::bytes::Regex>,
}

/// `cmd_dump` (as text: `--binary`, `--format` and `--wrap` are not ported yet).
pub fn dump(ctx: &mut Ctx, o: &DumpOptions, st: &mut crate::tostr::State, plugin: &str) -> Res<()> {
    let mut banner = (!o.no_banner).then(|| format!("\nPlugin: {plugin}\n"));
    let data = match crate::fsx::read(plugin) {
        Ok(d) => d,
        Err(e) => {
            ctx.msg(&format!(
                "FATAL ERROR ({plugin}): opening \"{plugin}\" for input ({e})"
            ));
            return Ok(());
        }
    };
    let mut pos = 0;
    let mut expected = Some("TES3");
    loop {
        let rec = match read_one(ctx, plugin, &data, &mut pos, expected, Reader::Input) {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(e) => {
                ctx.msg(&e.0);
                return Ok(());
            }
        };
        expected = None;
        let mut r = Rec { rec, d: None };
        // A formatter that dies ends the plugin, as tes3cmd's eval reports it.
        let died = |ctx: &mut Ctx, m: &str| ctx.msg(m);
        let (ok, text) = match rec_match(ctx, st, &o.wanted, &mut r) {
            Ok(x) => x,
            Err(m) => {
                died(ctx, m);
                return Ok(());
            }
        };
        if !ok {
            continue;
        }
        if let Some(b) = banner.take() {
            ctx.print(&b);
        }
        let rectype = r.rec.rectype_str();
        let Some(d) = r.d.as_ref() else { continue };
        let id = crate::tostr::record_id(d);
        let list_record = |ctx: &mut Ctx, prefix: &str| list_line(ctx, prefix, &rectype, &id, d);
        let raw = |rec: &Record| {
            if let Some(path) = &o.raw_to {
                let mut b = Vec::new();
                rec.write(&mut b);
                use std::io::Write as _;
                if let Ok(w) = crate::fsx::at_write(path, true)
                    && let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(w)
                {
                    let _ = f.write_all(&b);
                }
            }
        };
        if !o.formats.is_empty() {
            for (line, fields) in &o.formats {
                let mut vals = Vec::new();
                for f in fields {
                    let v = crate::tostr::getfield(ctx, st, d, f);
                    let v = if o.no_quote {
                        v
                    } else {
                        match v {
                            None => {
                                ctx.msg("Use of uninitialized value $val in pattern match (m//)");
                                None
                            }
                            Some(b)
                                if b.iter().any(|&c| {
                                    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
                                }) =>
                            {
                                let mut q = vec![b'"'];
                                for c in b {
                                    if c == b'"' {
                                        q.push(b'\\');
                                    }
                                    q.push(c);
                                }
                                q.push(b'"');
                                Some(q)
                            }
                            other => other,
                        }
                    };
                    vals.push(v);
                }
                for (i, v) in vals.iter().enumerate() {
                    if v.is_none() {
                        ctx.msg(&if o.no_quote {
                            "Use of uninitialized value in printf".to_string()
                        } else {
                            format!("Use of uninitialized value $vals[{i}] in printf")
                        });
                    }
                }
                let args: Vec<Val> = vals
                    .into_iter()
                    .map(|v| v.map(Val::Bytes).unwrap_or(Val::Undef))
                    .collect();
                let mut fmt = line.clone();
                fmt.push(b'\n');
                ctx.print_bytes(&crate::tostr::sprintf(&fmt, &args));
            }
        } else if o.list {
            list_record(ctx, "");
        } else if o.instance_match.is_some() || o.instance_no_match.is_some() {
            let groups = d.split_groups();
            let ids = crate::tostr::sub_ids(d);
            let mut at = groups.first().map_or(0, Vec::len);
            let mut newcell: Vec<crate::decode::Sub> = groups.first().cloned().unwrap_or_default();
            let mut selected = 0;
            let mut nam0: Option<usize> = None;
            let mut n = 0;
            for g in groups.iter().skip(1) {
                let first = at;
                at += g.len();
                if g.first().is_some_and(|s| &s.tag == b"NAM0") {
                    newcell.push(g[0].clone());
                    nam0 = Some(newcell.len() - 1);
                    continue;
                }
                let parts: Vec<Vec<u8>> = g
                    .iter()
                    .enumerate()
                    .map(|(k, s)| {
                        crate::tostr::sub_tostr(
                            ctx,
                            st,
                            d,
                            s,
                            ids.get(first + k).and_then(|x| x.as_deref()),
                        )
                    })
                    .collect();
                if crate::tostr::died() {
                    died(ctx, crate::tostr::NOT_A_SCALAR);
                    return Ok(());
                }
                let gs = parts.join(&b"\n"[..]);
                let keep = o.instance_match.as_ref().is_none_or(|re| re.is_match(&gs))
                    && o.instance_no_match
                        .as_ref()
                        .is_none_or(|re| !re.is_match(&gs));
                if keep {
                    newcell.extend(g.iter().cloned());
                    selected += 1;
                    if nam0.is_some() {
                        n += 1;
                    }
                }
            }
            if selected > 0 {
                if let Some(i) = nam0
                    && let Fields::Generic { slots, .. } = &mut newcell[i].fields
                    && !slots.is_empty()
                {
                    slots[0] = Val::Int(n);
                }
                let nd = Decoded::from_subs(*b"CELL", newcell);
                if o.raw_to.is_some() {
                    list_record(ctx, "Raw Output:");
                    let body = nd.encode(ctx);
                    raw(&Record {
                        rectype: *b"CELL",
                        hdrflags: 0,
                        buf: body,
                    });
                    continue;
                }
                match crate::tostr::record_tostr(ctx, st, &nd) {
                    Ok(text) => ctx.print_bytes(&cat_bytes(&[&text, b"\n"])),
                    Err(m) => {
                        died(ctx, m);
                        return Ok(());
                    }
                }
            }
        } else if o.raw_to.is_some() {
            list_record(ctx, "Raw Output:");
            raw(&r.rec);
        } else {
            let text = match text {
                Some(t) => t,
                None => match crate::tostr::record_tostr(ctx, st, d) {
                    Ok(t) => t,
                    Err(m) => {
                        died(ctx, m);
                        return Ok(());
                    }
                },
            };
            if !text.is_empty() {
                ctx.print_bytes(&cat_bytes(&[b"\n", &text, b"\n"]));
            }
        }
    }
    Ok(())
}

/// `list_record`: `<prefix> CELL: id<tab>N objects`, or `<prefix> TYPE: id (name)`.
fn list_line(ctx: &mut Ctx, prefix: &str, rectype: &str, id: &str, d: &Decoded) {
    if rectype == "CELL" {
        let n = d.subs.iter().filter(|s| &s.tag == b"FRMR").count();
        ctx.print(&format!("{prefix} {rectype}: {id:<45}\t{n:>5} objects\n"));
        return;
    }
    let fnam = d.get(b"FNAM").and_then(|s| {
        let row = crate::tostr_table::TOSTR
            .iter()
            .find(|x| x.0.as_bytes() == d.rectype && x.1 == "FNAM")?;
        if row.4.first() != Some(&"name") {
            return None;
        }
        match &s.fields {
            Fields::Generic { slots, .. } => slots.first().map(Val::bytes),
            _ => None,
        }
    });
    let tail = fnam
        .map(|n| cat_bytes(&[b" (", &n, b")"]))
        .unwrap_or_default();
    ctx.print_bytes(&cat_bytes(&[
        format!("{prefix} {rectype}: {id}").as_bytes(),
        &tail,
        b"\n",
    ]));
}

fn cat_bytes(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}
