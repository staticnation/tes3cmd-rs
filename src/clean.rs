//! `tes3cmd clean` (`cmd_clean`), as 0.40-PRE-RELEASE-2 does it: evil GMSTs, object instances
//! and whole records duplicated from the plugin's masters, redundant AMBI and WHGT in interiors,
//! and junk cells. The messages, the output's name, the backups and the header's record count
//! are tes3cmd's; so are its quirks, where the port is held to its output (see the notes).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::ctx::{Abort, Ctx, Res};
use crate::datadir::Install;
use crate::decode::{Decoded, Fields, Sub, quick_id};
use crate::header::{is_backup_name, update_record_count};
use crate::output::{Output, fix_output};
use crate::record::{IGNORED, PERSISTENT, Reader, Record, read_one};

/// The record types cleaned as duplicates (`@CLEAN_DUP_TYPES`).
pub const CLEAN_DUP_TYPES: &[&str] = &[
    "ACTI", "ALCH", "APPA", "ARMO", "BODY", "BOOK", "BSGN", "CELL", "CLAS", "CLOT", "CONT", "CREA",
    "DOOR", "ENCH", "FACT", "GLOB", "GMST", "INFO", "INGR", "LAND", "LEVC", "LEVI", "LIGH", "LOCK",
    "MGEF", "MISC", "NPC_", "PGRD", "PROB", "RACE", "REGN", "REPA", "SCPT", "SKIL", "SNDG", "SOUN",
    "SPEL", "SSCR", "STAT", "WEAP",
];

/// Plugins never cleaned, and why (`%CLEAN_PLUGIN`).
pub const CLEAN_PLUGIN: &[(&str, &str)] = &[
    ("bloodmoon.esm", "Bethesda Master"),
    ("fogpatch.esp", "does not need cleaning"),
    ("gmst fix.esp", "intentionally contain Evil GMSTs"),
    ("gmst vaccine.esp", "intentionally contain Evil GMSTs"),
    ("mashed lists.esp", "does not need cleaning"),
    (
        "merged_dialogs.esp",
        "does not need cleaning, but you should delete it anyway",
    ),
    (
        "merged_leveled_lists.esp",
        "does not need cleaning, but you should use \"tes3cmd multipatch\" instead",
    ),
    ("merged_objects.esp", "does not need cleaning"),
    ("morrowind.esm", "Bethesda Master"),
    ("multipatch.esp", "does not need cleaning"),
    ("tribunal.esm", "Bethesda Master"),
];

/// Cells with only these subtypes may be junk (`%JUNKCELL_SUBTYPE`).
const JUNKCELL_SUBTYPE: &[&[u8; 4]] = &[b"NAME", b"DATA", b"RGNN"];

#[derive(Clone, Debug, Default)]
pub struct CleanOptions {
    pub instances: bool,
    pub cell_params: bool,
    pub dups: bool,
    pub gmsts: bool,
    pub junk_cells: bool,
    /// `--overwrite` / `--replace`: the plugin itself, backed up; else `Clean_<plugin>`.
    pub overwrite: bool,
    pub output: Output,
}

impl CleanOptions {
    /// The clean command's preprocessing: none chosen, or `--all`, is every one.
    pub fn settle(&mut self, all: bool) {
        if all
            || !(self.instances || self.cell_params || self.dups || self.gmsts || self.junk_cells)
        {
            self.instances = true;
            self.cell_params = true;
            self.dups = true;
            self.gmsts = true;
            self.junk_cells = true;
        }
    }
}

/// One master's records: by id, then type, the record's bytes and header flags (`[recbuf, flags]`).
pub type MasterRecords = HashMap<String, HashMap<[u8; 4], (Vec<u8>, u32)>>;

/// The masters' records by lower-case name (`$MASTER_ID`), kept for the whole run.
#[derive(Default)]
pub struct Masters {
    pub by_master: HashMap<String, MasterRecords>,
}

impl Masters {
    /// `load_master`: every record of the master that is not ignored, by its quick id. Reported
    /// and stopped where a record cannot be read, as tes3cmd does.
    pub fn load(&mut self, ctx: &mut Ctx, inst: &mut Install, esm: &str) {
        let esm = esm.to_lowercase();
        if self.by_master.contains_key(&esm) {
            return;
        }
        ctx.print(&format!("Loading Master: {esm}\n"));
        let Some(path) = inst.datapath(&esm) else {
            ctx.error(&format!("Master: {esm} not found in <DATADIR>"));
            return;
        };
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                // open_for_input's abort, caught by the caller's eval.
                ctx.error(&format!(
                    "FATAL ERROR ({}): opening \"{}\" for input ({e})",
                    ctx.plugin,
                    path.display()
                ));
                return;
            }
        };
        let mut entry: MasterRecords = HashMap::new();
        let mut pos = 0;
        loop {
            match read_one(ctx, &esm, &data, &mut pos, None, Reader::Quick) {
                Ok(Some(rec)) => {
                    if rec.hdrflags & IGNORED != 0 {
                        continue;
                    }
                    let id = quick_id(ctx, &rec.rectype, &rec.buf);
                    if id == "\u{0}no id" {
                        ctx.error(
                            &ctx.abort(&format!(
                                "Oops! Don't know how to make ID for rectype: {}",
                                rec.rectype_str()
                            ))
                            .0,
                        );
                        break;
                    }
                    entry
                        .entry(id)
                        .or_default()
                        .insert(rec.rectype, (rec.buf, rec.hdrflags));
                }
                Ok(None) => break,
                Err(e) => {
                    ctx.error(&e.0);
                    break;
                }
            }
        }
        self.by_master.insert(esm, entry);
    }

    fn get(&self, master: &str, id: &str, rectype: &[u8; 4]) -> Option<&(Vec<u8>, u32)> {
        self.by_master.get(master)?.get(id)?.get(rectype)
    }
}

/// The record in hand as `cmd_clean`'s callback sees it: as read, decoded once asked, and replaced
/// by a new one (no bytes until encoded) where instances were cleaned.
struct Plug {
    rec: Record,
    dec: Option<Decoded>,
    /// The bytes `recbuf` answers: the original's, until the record is rebuilt.
    recbuf: Option<Vec<u8>>,
    cached_id: Option<String>,
}

impl Plug {
    fn decoded(&mut self, ctx: &mut Ctx) -> &mut Decoded {
        if self.dec.is_none() {
            self.dec = Some(Decoded::new(
                ctx,
                self.rec.rectype,
                self.rec.hdrflags,
                &self.rec.buf,
            ));
        }
        self.dec.as_mut().unwrap()
    }
    fn id(&mut self, ctx: &mut Ctx) -> String {
        if let Some(i) = &self.cached_id {
            return i.clone();
        }
        let id = self.decoded(ctx).id();
        self.cached_id = Some(id.clone());
        id
    }
}

/// `buffalize`: a group's object index, the bytes of everything after its FRMR, and its name.
fn buffalize(ctx: &Ctx, group: &[Sub]) -> Res<(u32, Vec<u8>, Option<Vec<u8>>)> {
    let first = &group[0];
    let objidx = match (&first.tag, &first.fields) {
        (b"FRMR" | b"MVRF", Fields::Reference { idx }) => idx.unwrap_or(0) & 0x00FF_FFFF,
        _ => {
            return Err(ctx.abort(&format!(
                "Oops! Expected first subrec of group to be FRMR, instead got: {}",
                first.tag_str()
            )));
        }
    };
    let mut buf = Vec::new();
    let mut name = None;
    for s in &group[1..] {
        if name.is_none() && &s.tag == b"NAME" {
            name = Some(name_of(s));
        }
        buf.extend_from_slice(&s.raw);
    }
    Ok((objidx, buf, name))
}

/// A CELL.NAME's `name` field (`Z*`).
fn name_of(s: &Sub) -> Vec<u8> {
    match &s.fields {
        Fields::Generic { slots, .. } => slots.first().map(|v| v.bytes()).unwrap_or_default(),
        _ => s.raw.split(|&b| b == 0).next().unwrap_or(&[]).to_vec(),
    }
}

/// `instance_name`: the first NAME of a group.
fn instance_name(group: &[Sub]) -> Option<Vec<u8>> {
    group.iter().find(|s| &s.tag == b"NAME").map(name_of)
}

fn bytes_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// One record through `cmd_clean`'s callback: `None` to delete it, `Some(None)` to pass it
/// through, `Some(Some(record))` to write it changed.
#[allow(clippy::too_many_arguments)]
fn clean_record(
    ctx: &mut Ctx,
    opts: &CleanOptions,
    inst: &mut Install,
    masters: &mut Masters,
    my_master: &mut BTreeSet<String>,
    persistent: &mut HashMap<String, usize>,
    stats: &mut BTreeMap<String, usize>,
    rec: Record,
) -> Res<Option<Option<Record>>> {
    let rectype = rec.rectype;
    let mut plug = Plug {
        recbuf: Some(rec.buf.clone()),
        rec,
        dec: None,
        cached_id: None,
    };
    let mut modified = false;
    let stat = |stats: &mut BTreeMap<String, usize>, what: &str| {
        *stats.entry(what.to_string()).or_default() += 1
    };

    if opts.instances && plug.rec.hdrflags & PERSISTENT != 0 {
        let id = plug.id(ctx);
        *persistent.entry(id).or_default() += 1;
    }

    if &rectype == b"GMST" {
        if opts.gmsts {
            let id = plug.id(ctx);
            let (ty, tag): (&str, &[u8; 4]) = match id.as_bytes().first() {
                Some(b'i') => ("INTV", b"INTV"),
                Some(b'f') => ("FLTV", b"FLTV"),
                Some(b's') => ("STRV", b"STRV"),
                // tes3cmd dereferences nothing here and dies; its eval reports it.
                _ => {
                    return Err(Abort(
                        "Can't use an undefined value as an ARRAY reference".into(),
                    ));
                }
            };
            if let Some(v) = plug.decoded(ctx).get(tag) {
                let mut hex = String::new();
                for b in ty
                    .bytes()
                    .chain(std::iter::once(b' '))
                    .chain(v.raw.iter().copied())
                {
                    hex.push_str(&format!("{b:02x}"));
                }
                let evil =
                    |table: &[(&str, &str)]| table.iter().any(|(k, h)| *k == id && *h == hex);
                let what = if evil(crate::evil::EVIL_BM) {
                    Some("Evil-GMST Bloodmoon")
                } else if evil(crate::evil::EVIL_TB) {
                    Some("Evil-GMST Tribunal")
                } else {
                    None
                };
                if let Some(what) = what {
                    stat(stats, what);
                    ctx.print(&format!(" Cleaned {what}: {id}\n"));
                    return Ok(None);
                }
            }
        }
    } else if &rectype == b"CELL" {
        let mut cleaned_ambi = false;
        let mut cleaned_whgt = false;
        if opts.instances {
            let id = plug.id(ctx);
            let groups = plug.decoded(ctx).split_groups();
            let mut groups = groups.into_iter();
            let mut newcell: Vec<Sub> = groups.next().unwrap_or_default();
            let mut cell_modified = false;
            for group in groups {
                // (tes3cmd also looks for groups that start with NAM0 or MVRF: only FRMR starts
                // a group in a cell, so neither is ever met, and NAM0's count is never updated.)
                let name = instance_name(&group);
                if persistent.contains_key(&bytes_str(&name.unwrap_or_default())) {
                    newcell.extend(group);
                    continue;
                }
                let (plug_objidx, plug_buf, plug_objname) = buffalize(ctx, &group)?;
                let mut cleaned = false;
                'masters: for master in my_master.iter() {
                    let Some((buf, flags)) = masters.get(master, &id, b"CELL").cloned() else {
                        continue;
                    };
                    let mast = Decoded::new(ctx, *b"CELL", flags, &buf);
                    for mg in mast.split_groups().into_iter().skip(1) {
                        if &mg[0].tag == b"NAM0" {
                            continue;
                        }
                        let (mast_objidx, mast_buf, _) = buffalize(ctx, &mg)?;
                        if plug_objidx == mast_objidx {
                            if plug_buf == mast_buf {
                                cleaned = true;
                                cell_modified = true;
                                break 'masters;
                            }
                            break;
                        }
                    }
                }
                if cleaned {
                    let what = "duplicate object instance";
                    stat(stats, what);
                    ctx.print(&format!(
                        " Cleaned {what} ({} FRMR: {plug_objidx}) from CELL: {id}\n",
                        bytes_str(&plug_objname.unwrap_or_default())
                    ));
                } else {
                    newcell.extend(group);
                }
            }
            if cell_modified {
                plug.dec = Some(Decoded::from_subs(*b"CELL", newcell));
                plug.recbuf = None;
                plug.rec.hdrflags = 0;
                modified = true;
            }
        }
        if opts.cell_params || opts.junk_cells {
            let id = plug.id(ctx);
            for master in my_master.clone().iter() {
                let Some((buf, flags)) = masters.get(master, &id, b"CELL").cloned() else {
                    continue;
                };
                let mast = Decoded::new(ctx, *b"CELL", flags, &buf);
                let interior = plug.decoded(ctx).is_interior();
                if interior && opts.cell_params {
                    if !cleaned_ambi {
                        let d = plug.decoded(ctx);
                        if let (Some(p), Some(m)) = (d.get(b"AMBI"), mast.get(b"AMBI"))
                            && p.raw == m.raw
                        {
                            d.delete_subtype(b"AMBI");
                            cleaned_ambi = true;
                        }
                    }
                    if !cleaned_whgt {
                        let d = plug.decoded(ctx);
                        let whgt = |s: &Sub| match &s.fields {
                            Fields::Generic { slots, .. } => {
                                slots.first().filter(|v| v.is_defined()).map(|v| v.num())
                            }
                            _ => None,
                        };
                        let plugin_whgt = d.get(b"WHGT").and_then(whgt);
                        let master_whgt = mast.get(b"WHGT").and_then(whgt).or_else(|| {
                            match mast.get(b"INTV").map(|s| &s.fields) {
                                Some(Fields::WaterHeight(Some(v))) => Some(*v as f64),
                                _ => None,
                            }
                        });
                        if let (Some(p), Some(m)) = (plugin_whgt, master_whgt)
                            && p == m
                        {
                            d.delete_subtype(b"WHGT");
                            cleaned_whgt = true;
                        }
                    }
                }
                if opts.junk_cells {
                    let d = plug.decoded(ctx);
                    if d.subs.len() < 4 {
                        let mut new_data = false;
                        for s in &d.subs {
                            let p = d.get(&s.tag).unwrap();
                            let Some(m) = mast.get(&s.tag) else {
                                return Err(ctx.abort(&format!(
                                    "no mast subtype: {} (id={id})",
                                    s.tag_str()
                                )));
                            };
                            if !JUNKCELL_SUBTYPE.contains(&&s.tag) || p.raw != m.raw {
                                new_data = true;
                                break;
                            }
                        }
                        if !new_data {
                            let what = "junk-CELL";
                            stat(stats, what);
                            ctx.print(&format!(" Cleaned {what}: {id}\n"));
                            return Ok(None);
                        }
                    }
                }
            }
            let mut what = Vec::new();
            if cleaned_ambi {
                what.push("AMBI");
                stat(stats, "redundant CELL.AMBI");
            }
            if cleaned_whgt {
                what.push("WHGT");
                stat(stats, "redundant CELL.WHGT");
            }
            if !what.is_empty() {
                ctx.print(&format!(
                    " Cleaned redundant {} from CELL: {id}\n",
                    what.join(",")
                ));
                modified = true;
            }
        }
    } else if &rectype == b"TES3" {
        let names: Vec<String> = plug
            .decoded(ctx)
            .subs
            .iter()
            .filter(|s| &s.tag == b"MAST")
            .map(|s| match &s.fields {
                Fields::Generic { slots, .. } => {
                    bytes_str(&slots.first().map(|v| v.bytes()).unwrap_or_default())
                }
                _ => String::new(),
            })
            .collect();
        for m in names {
            my_master.insert(m.to_lowercase());
            masters.load(ctx, inst, &m);
        }
    }

    if opts.dups {
        let id = plug.id(ctx);
        let dup_type = CLEAN_DUP_TYPES.iter().any(|t| t.as_bytes() == rectype);
        for master in my_master.iter() {
            let Some((mbuf, mflags)) = masters.get(master, &id, &rectype) else {
                continue;
            };
            if dup_type
                && plug.recbuf.as_deref() == Some(mbuf.as_slice())
                && plug.rec.hdrflags == *mflags
            {
                let what = "duplicate record";
                stat(stats, what);
                ctx.print(&format!(
                    " Cleaned {what} ({}): {id}\n",
                    plug.rec.rectype_str()
                ));
                return Ok(None);
            }
        }
    }

    if modified {
        let d = plug.decoded(ctx).clone();
        let buf = d.encode(ctx);
        let hdrflags = plug.rec.hdrflags;
        Ok(Some(Some(Record {
            rectype,
            hdrflags,
            buf,
        })))
    } else {
        Ok(Some(None))
    }
}

/// `cmd_clean` for one plugin. An `Err` stops the whole run (an abort outside tes3cmd's eval).
pub fn clean_plugin(
    ctx: &mut Ctx,
    opts: &CleanOptions,
    inst: &mut Install,
    masters: &mut Masters,
    plugin: &str,
) -> Res<()> {
    let lower = plugin.to_lowercase();
    if lower.ends_with(".ess") {
        ctx.print(&format!(
            "tes3cmd clean skipping {plugin}: don't know how to clean savegames ... yet.\n"
        ));
        return Ok(());
    }
    if is_backup_name(plugin, true) {
        ctx.print(&format!(
            "tes3cmd clean skipping {plugin}: because it's a backup.\n"
        ));
        return Ok(());
    }
    if let Some((_, reason)) = CLEAN_PLUGIN.iter().find(|(n, _)| *n == lower) {
        ctx.print(&format!("tes3cmd clean skipping {plugin}: {reason}.\n"));
        return Ok(());
    }
    ctx.print(&format!("\nCLEANING: \"{plugin}\" ...\n"));
    let mut my_master = BTreeSet::new();
    let mut persistent = HashMap::new();
    let mut stats = BTreeMap::new();

    // process_plugin_for_update: the records into "<plugin>.tmp", then fix_output.
    let tmp = format!("{plugin}.tmp");
    let mut run = |ctx: &mut Ctx,
                   out: &mut Vec<u8>,
                   modified: &mut usize,
                   masters: &mut Masters,
                   inst: &mut Install|
     -> Res<()> {
        if !has_plugin_ext(plugin) {
            return Err(ctx.abort(&format!("\"{plugin}\" does not end in .esm/.esp/.ess")));
        }
        if !Path::new(plugin).is_file() {
            return Err(ctx.abort("Invalid input file (No such file or directory)"));
        }
        let data = std::fs::read(plugin)
            .map_err(|e| ctx.abort(&format!("opening \"{plugin}\" for input ({e})")))?;
        let mut pos = 0;
        let mut expected = Some("TES3");
        while let Some(rec) = read_one(ctx, plugin, &data, &mut pos, expected, Reader::Input)? {
            expected = None;
            // A type tes3cmd does not know gets stand-in methods, but not `rectype`, the first
            // one asked of it: the plugin stops there.
            if !crate::record::known_type(&rec.rectype) {
                return Err(Abort(format!(
                    "FATAL ERROR ({plugin}): Can't locate object method \"rectype\" via package \"{}\"",
                    rec.rectype_str()
                )));
            }
            let (rectype, verbose_id) = (rec.rectype_str(), ctx.verbose);
            let keep = rec.clone();
            match clean_record(
                ctx,
                opts,
                inst,
                masters,
                &mut my_master,
                &mut persistent,
                &mut stats,
                rec,
            )? {
                Some(Some(changed)) => {
                    changed.write(out);
                    if verbose_id {
                        let mut p = Plug {
                            recbuf: None,
                            rec: keep,
                            dec: None,
                            cached_id: None,
                        };
                        let id = p.id(ctx);
                        if *modified == 0 {
                            ctx.print(&format!("UPDATED PLUGIN: {plugin}\n"));
                        }
                        ctx.print(&format!("RECORD MODIFIED: {rectype} {id}\n"));
                    }
                    *modified += 1;
                }
                Some(None) => keep.write(out),
                None => {
                    if verbose_id {
                        let mut p = Plug {
                            recbuf: None,
                            rec: keep,
                            dec: None,
                            cached_id: None,
                        };
                        let id = p.id(ctx);
                        if *modified == 0 {
                            ctx.print(&format!("UPDATED PLUGIN: {plugin}\n"));
                        }
                        ctx.print(&format!("RECORD DELETED: {rectype} {id}\n"));
                    }
                    *modified += 1;
                }
            }
        }
        Ok(())
    };
    let mut out = Vec::new();
    let mut modified = 0usize;
    let started = has_plugin_ext(plugin) && Path::new(plugin).is_file();
    match run(ctx, &mut out, &mut modified, masters, inst) {
        Ok(()) => {}
        Err(e) => {
            // The eval's report; the plugin is left as it was (the partial .tmp too, as tes3cmd leaves it).
            if started {
                let _ = std::fs::write(&tmp, &out);
            }
            let _ = std::io::Write::write_all(ctx.err, format!("{}\n", e.0).as_bytes());
            return Ok(());
        }
    }
    std::fs::write(&tmp, &out)
        .map_err(|e| ctx.abort(&format!("opening \"{tmp}\" for output ({e})")))?;
    let newname = (!opts.overwrite).then(|| {
        let (dir, file) = split_dir(plugin);
        if dir.is_empty() {
            format!("Clean_{file}")
        } else {
            format!("{dir}Clean_{file}")
        }
    });
    fix_output(
        ctx,
        &opts.output,
        inst,
        plugin,
        modified > 0,
        newname,
        update_record_count,
    )?;

    if !stats.is_empty() {
        ctx.print(&format!("\nCleaning Stats for \"{plugin}\":\n"));
        for (k, v) in &stats {
            ctx.print(&format!("  {k:>30}:  {v:>4}\n"));
        }
    }
    Ok(())
}

pub fn has_plugin_ext(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.ends_with(".esm") || l.ends_with(".esp") || l.ends_with(".ess")
}

/// `fileparse`: the folder (with its separator; "" for `./`) and the file's name.
pub fn split_dir(path: &str) -> (String, String) {
    let cut = if cfg!(windows) {
        path.rfind(['/', '\\'])
    } else {
        path.rfind('/')
    };
    match cut {
        Some(i) => {
            let dir = &path[..=i];
            (
                if dir == "./" {
                    String::new()
                } else {
                    dir.to_string()
                },
                path[i + 1..].to_string(),
            )
        }
        None => (String::new(), path.to_string()),
    }
}
