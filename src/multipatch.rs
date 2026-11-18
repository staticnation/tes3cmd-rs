//! `multipatch` (`cmd_multipatch`): `multipatch.esp` for the active plugins, with the cell
//! renamings later plugins revert, the interior cells whose fog density is 0 (the fog bug),
//! the summoned creatures a plugin made non-persistent, and the leveled lists merged.
//! `--merge-objects` does nothing, as in tes3cmd (its merge returns before it starts).

use std::collections::{BTreeMap, BTreeSet};

use crate::commands::{HeaderOptions, update_header};
use crate::ctx::{Ctx, Res};
use crate::datadir::Install;
use crate::decode::{Decoded, Fields};
use crate::pack::{Val, pack_counting, perl_num_str};
use crate::record::{Reader, Record, read_one};

const NOPATCH_PLUGIN: &[(&str, &str)] = &[
    ("bloodmoon.esm", "Bethesda Master"),
    ("cellnamepatch.esp", "does not need patching (obsolete)"),
    ("fogpatch.esp", "does not need patching (obsolete)"),
    ("mashed lists.esp", "does not need patching"),
    (
        "merged_dialogs.esp",
        "does not need patching, but you should delete it anyway",
    ),
    (
        "merged_leveled_lists.esp",
        "does not need patching, but you should use \"tes3cmd multipatch\" instead",
    ),
    ("merged_objects.esp", "does not need patching"),
    ("morrowind.esm", "Bethesda Master"),
    ("multipatch.esp", "does not need patching"),
    ("tribunal.esm", "Bethesda Master"),
];

const SUMMONED_CREATURES: &[&str] = &[
    "centurion_fire_dead",
    "wraith_sul_senipul",
    "ancestor_ghost_summon",
    "atronach_flame_summon",
    "atronach_frost_summon",
    "atronach_storm_summon",
    "bonelord_summon",
    "bonewalker_summon",
    "bonewalker_greater_summ",
    "centurion_sphere_summon",
    "clannfear_summon",
    "daedroth_summon",
    "dremora_summon",
    "golden saint_summon",
    "hunger_summon",
    "scamp_summon",
    "skeleton_summon",
    "ancestor_ghost_variner",
    "fabricant_summon",
    "bm_bear_black_summon",
    "bm_wolf_grey_summon",
    "bm_wolf_bone_summon",
];

const IGNORED: u32 = 0x1000;

/// A plugin's leveled lists: by type, then id.
type Lists = BTreeMap<&'static str, BTreeMap<String, Decoded>>;
/// Each list's definitions in load order: by type, then id, `(plugin, record)`.
type Defs<'a> = BTreeMap<&'static str, BTreeMap<String, Vec<(String, &'a Decoded)>>>;
const PERSISTENT: u32 = 0x400;

#[derive(Clone, Default)]
pub struct MultipatchOptions {
    pub cellnames: bool,
    pub fogbug: bool,
    pub merge_lists: bool,
    pub merge_objects: bool,
    pub summons_persist: bool,
    pub no_activate: bool,
    pub no_cache: bool,
    pub delete_creature: Vec<String>,
    pub delete_item: Vec<String>,
}

/// A field of a generic subrecord, by its slot.
fn slot(d: &Decoded, tag: &[u8; 4], i: usize) -> Val {
    match d.get(tag).map(|s| &s.fields) {
        Some(Fields::Generic { slots, .. }) => slots.get(i).cloned().unwrap_or(Val::Undef),
        _ => Val::Undef,
    }
}

fn bytes_str(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// A subrecord packed by its encoder (`tsr->encode`), Perl's warning for each undefined value.
fn packed(ctx: &mut Ctx, rectype: &str, tag: &str, vals: &[Val]) -> Vec<u8> {
    let enc = crate::schema_table::SCHEMA
        .iter()
        .find(|r| r.0 == rectype && r.1 == tag)
        .map(|r| r.3)
        .unwrap_or("a*");
    let (b, undefs) = pack_counting(enc, vals);
    for _ in 0..undefs {
        ctx.msg("Use of uninitialized value in pack");
    }
    let mut out = tag.as_bytes().to_vec();
    out.extend((b.len() as u32).to_le_bytes());
    out.extend(b);
    out
}

/// Every record of a plugin, as `new_from_input` reads it; an error stops the read (`Err`).
fn read_all(ctx: &mut Ctx, path: &str, name: &str) -> (Vec<Record>, Option<String>) {
    let data = match crate::fsx::read(path) {
        Ok(d) => d,
        Err(e) => {
            return (
                Vec::new(),
                Some(format!(
                    "FATAL ERROR ({name}): opening \"{path}\" for input ({e})\n"
                )),
            );
        }
    };
    let mut out = Vec::new();
    let mut pos = 0;
    let mut expected = Some("TES3");
    loop {
        match read_one(ctx, name, &data, &mut pos, expected, Reader::Input) {
            Ok(Some(r)) => out.push(r),
            Ok(None) => return (out, None),
            Err(e) => return (out, Some(format!("{}\n", e.0))),
        }
        expected = None;
    }
}

/// `load_leveled_lists`, without its cache (the port keeps none, so it reads as tes3cmd does
/// on its first run): each active plugin's LEVC and LEVI, by type then id.
fn load_leveled_lists(
    ctx: &mut Ctx,
    o: &MultipatchOptions,
    inst: &mut Install,
    lo: &[String],
) -> BTreeMap<String, BTreeMap<&'static str, BTreeMap<String, Decoded>>> {
    let cache = format!("{}/leveled_lists_data.cache", inst.cache_dir);
    if o.no_cache {
        let _ = crate::fsx::remove_file(&cache);
    } else {
        ctx.msg("Creating Leveled Lists Cache");
    }
    let mut out: BTreeMap<String, BTreeMap<&'static str, BTreeMap<String, Decoded>>> =
        BTreeMap::new();
    for plugin in lo {
        let low = plugin.to_lowercase();
        if low == "multipatch.esp" {
            continue;
        }
        if low == "mashed lists.esp" || low == "merged_leveled_lists.esp" {
            if o.merge_lists {
                let shown = if low == "mashed lists.esp" {
                    "Mashed Lists.esp"
                } else {
                    "Merged_Leveled_Lists.esp"
                };
                ctx.msg(&format!(
                    "WARNING! \"{shown}\" is not needed when using multipatch"
                ));
            }
            continue;
        }
        if ctx.verbose {
            ctx.print(&format!("Leveled List Cache ADDING: {plugin}\n"));
        }
        let path = inst
            .datapath(plugin)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (recs, err) = read_all(ctx, &path, plugin);
        let mut lists: Lists = BTreeMap::new();
        for r in recs {
            let rt: &'static str = match &r.rectype {
                b"LEVC" => "LEVC",
                b"LEVI" => "LEVI",
                _ => continue,
            };
            if r.hdrflags & IGNORED != 0 {
                continue;
            }
            let d = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf);
            lists.entry(rt).or_default().insert(d.id(), d);
        }
        match err {
            Some(e) => ctx.error(&e),
            None => {
                out.insert(low, lists);
            }
        }
    }
    out
}

/// `multipatch_merge_leveled_lists_old`: each list defined three or more times, merged; its
/// verbose report, and the records.
fn merge_lists(
    ctx: &mut Ctx,
    o: &MultipatchOptions,
    inst: &mut Install,
    lo: &[String],
) -> Vec<Vec<u8>> {
    let cache = load_leveled_lists(ctx, o, inst, lo);
    let mut ll: Defs = BTreeMap::new();
    for plugin in lo {
        if let Some(lists) = cache.get(&plugin.to_lowercase()) {
            for rt in ["LEVC", "LEVI"] {
                for (id, d) in lists.get(rt).into_iter().flatten() {
                    ll.entry(rt)
                        .or_default()
                        .entry(id.clone())
                        .or_default()
                        .push((plugin.clone(), d));
                }
            }
        }
    }
    let mut merged = Vec::new();
    for rt in ["LEVC", "LEVI"] {
        let (element_tag, deletions): (&[u8; 4], &Vec<String>) = if rt == "LEVC" {
            (b"CNAM", &o.delete_creature)
        } else {
            (b"INAM", &o.delete_item)
        };
        let user_deletions: BTreeSet<&str> = deletions.iter().map(String::as_str).collect();
        for (id, defs) in ll.get(rt).into_iter().flatten() {
            if defs.len() == 1 {
                if ctx.verbose {
                    ctx.print(&format!(
                        " [LEVLIST] skipping {rt} {id} - defined only once in: [{}]\n",
                        defs[0].0
                    ));
                }
                continue;
            }
            if defs.len() == 2 {
                if ctx.verbose {
                    ctx.print(&format!(
                        " [LEVLIST] skipping {rt} {id} - defined only twice, using: [{}]\n",
                        defs[1].0
                    ));
                }
                continue;
            }
            let last = defs[defs.len() - 1].1;
            let last_flags = slot(last, b"DATA", 0);
            let last_chance = slot(last, b"NNAM", 0);
            // element id -> level -> count
            type Def = BTreeMap<Vec<u8>, BTreeMap<i64, i64>>;
            let mut levlist: Def = BTreeMap::new();
            let mut first: Option<Def> = None;
            for (_, d) in defs {
                let mut cur: Def = BTreeMap::new();
                let mut element: Vec<u8> = Vec::new();
                for s in &d.subs {
                    let v = match &s.fields {
                        Fields::Generic { slots, .. } => {
                            slots.first().cloned().unwrap_or(Val::Undef)
                        }
                        _ => Val::Undef,
                    };
                    if &s.tag == element_tag {
                        element = v.bytes();
                    } else if &s.tag == b"INTV" {
                        *cur.entry(element.clone())
                            .or_default()
                            .entry(v.num() as i64)
                            .or_default() += 1;
                    }
                }
                match &first {
                    Some(f) => {
                        for (e, levels) in &cur {
                            for (lv, n) in levels {
                                if f.get(e).and_then(|l| l.get(lv)) != Some(n) {
                                    levlist.entry(e.clone()).or_default().insert(*lv, *n);
                                }
                            }
                        }
                        for (e, levels) in f {
                            match cur.get(e) {
                                None => {
                                    levlist.remove(e);
                                }
                                Some(c) => {
                                    for lv in levels.keys() {
                                        if !c.contains_key(lv)
                                            && let Some(l) = levlist.get_mut(e)
                                        {
                                            l.remove(lv);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    None => {
                        for (e, levels) in &cur {
                            for (lv, n) in levels {
                                levlist.entry(e.clone()).or_default().insert(*lv, *n);
                            }
                        }
                        first = Some(cur);
                    }
                }
            }
            if ctx.verbose {
                ctx.print(&format!(" [LEVLIST] Defining {rt} {id} ->\n"));
            }
            let mut list: Vec<(i64, Vec<u8>)> = Vec::new();
            let mut deleted: BTreeSet<Vec<u8>> = BTreeSet::new();
            for (e, levels) in &levlist {
                for (lv, n) in levels {
                    for _ in 0..*n {
                        if user_deletions.contains(bytes_str(e).as_str()) {
                            deleted.insert(e.clone());
                        } else {
                            list.push((*lv, e.clone()));
                        }
                    }
                }
            }
            list.sort();
            if ctx.verbose {
                for (lv, e) in &list {
                    ctx.print_bytes(&[format!("\t[lev={lv} cre=").as_bytes(), e, b"]\n"].concat());
                }
                if !deleted.is_empty() {
                    ctx.print("\tDeleted IDs:\n");
                    for e in &deleted {
                        ctx.print_bytes(&[b"\t  ", &e[..], b"\n"].concat());
                    }
                }
            }
            let mut body = Vec::new();
            body.extend(packed(
                ctx,
                rt,
                "NAME",
                &[Val::Bytes(id.as_bytes().to_vec())],
            ));
            body.extend(packed(ctx, rt, "DATA", &[last_flags]));
            body.extend(packed(ctx, rt, "NNAM", &[last_chance]));
            body.extend(packed(ctx, rt, "INDX", &[Val::Int(list.len() as i64)]));
            let etag = if rt == "LEVC" { "CNAM" } else { "INAM" };
            for (lv, e) in &list {
                body.extend(packed(ctx, rt, etag, &[Val::Bytes(e.clone())]));
                body.extend(packed(ctx, rt, "INTV", &[Val::Int(*lv)]));
            }
            let mut rec = Vec::new();
            Record {
                rectype: rt.as_bytes().try_into().unwrap(),
                hdrflags: 0,
                buf: body,
            }
            .write(&mut rec);
            merged.push((rt, rec));
        }
    }
    let n = |t: &str| merged.iter().filter(|(r, _)| *r == t).count();
    let summary = format!(
        "\nMerged Leveled Lists ({} LEVC, {} LEVI)\n",
        n("LEVC"),
        n("LEVI")
    );
    let mut out: Vec<Vec<u8>> = merged.into_iter().map(|(_, r)| r).collect();
    out.push(summary.into_bytes());
    out
}

/// What `multipatch_check_cell_rename_reversions` keeps for one exterior cell.
#[derive(Default)]
struct Rename {
    orig: Vec<u8>,
    name: Vec<u8>,
    plug: String,
    output: usize,
    rec: Option<Vec<u8>>,
}

/// `cmd_multipatch`.
pub fn multipatch(ctx: &mut Ctx, o: &MultipatchOptions, inst: &mut Install) -> Res<()> {
    ctx.print("\nMultipatch: Scanning Active Plugins...\n");
    let patch_file = "multipatch.esp";
    let patch_path = format!("{}/{patch_file}", inst.datadir);
    let lo = inst.load_order();
    let lists = if o.merge_lists {
        Some(merge_lists(ctx, o, inst, &lo))
    } else {
        None
    };
    let mut fogcell: BTreeMap<String, (Decoded, String)> = BTreeMap::new();
    let mut rencell: BTreeMap<String, Rename> = BTreeMap::new();
    let mut sumcrea: BTreeMap<String, Record> = BTreeMap::new();
    for plugin in &lo {
        ctx.plugin = plugin.clone();
        if let Some((_, reason)) = NOPATCH_PLUGIN
            .iter()
            .find(|(n, _)| *n == plugin.to_lowercase())
        {
            ctx.print(&format!(
                "tes3cmd multipatch skipping {plugin}: {reason}.\n"
            ));
            continue;
        }
        ctx.print(&format!("Scanning plugin: {plugin}\n"));
        let path = inst
            .datapath(plugin)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (recs, err) = read_all(ctx, &path, plugin);
        for r in recs {
            if r.hdrflags & IGNORED != 0 {
                continue;
            }
            if (o.fogbug || o.cellnames) && &r.rectype == b"CELL" {
                let d = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf);
                if d.is_interior() {
                    let flags = match d.get(b"DATA").map(|s| &s.fields) {
                        Some(Fields::Interior { flags, .. }) => *flags,
                        _ => 0,
                    };
                    if o.fogbug && flags & 128 == 0 {
                        check_fogbug(ctx, plugin, d, &mut fogcell);
                    }
                } else if o.cellnames {
                    check_rename(ctx, plugin, &d, &mut rencell);
                }
            }
            if o.summons_persist && &r.rectype == b"CREA" {
                let id = Decoded::new(ctx, r.rectype, r.hdrflags, &r.buf).id();
                if SUMMONED_CREATURES.contains(&id.as_str()) {
                    if r.hdrflags & PERSISTENT != 0 {
                        if ctx.verbose {
                            ctx.print(&format!(
                                "  [SUMMCREA] {plugin} PRESERVES persistence for: {id}\n"
                            ));
                        }
                        sumcrea.remove(&id);
                    } else {
                        if ctx.verbose {
                            ctx.print(&format!(
                                "  [SUMMCREA] {plugin} REVERTS persistence for: {id}\n"
                            ));
                        }
                        let mut r = r.clone();
                        r.hdrflags |= PERSISTENT;
                        sumcrea.insert(id, r);
                    }
                }
            }
        }
        if let Some(e) = err {
            ctx.print(&e);
        }
    }
    let nrenrevs = rencell.values().filter(|c| c.output > 0).count();
    let merged = lists.as_ref().is_some_and(|l| !l.is_empty());
    if fogcell.is_empty() && nrenrevs == 0 && sumcrea.is_empty() && !merged {
        ctx.print("\nNo patching necessary. \"multipatch.esp\" not generated.\n");
        return Ok(());
    }
    ctx.print(&format!("\n{}\n", "=".repeat(75)));
    ctx.print("A multipatch has been conjured for you to address the following issues:\n");
    let mut out = make_header();
    if nrenrevs > 0 {
        ctx.print(&format!(
            "\nPreserving the following {nrenrevs} CELL renamings:\n"
        ));
        for (coord, c) in &rencell {
            if c.output > 0
                && let Some(rec) = &c.rec
            {
                ctx.print_bytes(
                    &[
                        format!("  CELL: ({coord}) -> NAME: \"").as_bytes(),
                        &c.name,
                        b"\"\n",
                    ]
                    .concat(),
                );
                out.extend(rec);
            }
        }
    }
    if !fogcell.is_empty() {
        ctx.print(&format!("\nPatching {} fogbugged cells:\n", fogcell.len()));
        for (name, (d, plugin)) in &fogcell {
            ctx.print(&format!("  CELL: {name}\t\t[{plugin}]\n"));
            let mut subs = Vec::new();
            for tag in [b"NAME", b"DATA", b"AMBI"] {
                if let Some(s) = d.get(tag) {
                    let mut s = s.clone();
                    match &mut s.fields {
                        Fields::Interior { fog, .. } => *fog = 0.01,
                        Fields::Generic { slots, .. } if tag == b"AMBI" && slots.len() > 3 => {
                            slots[3] = Val::Num(0.01)
                        }
                        _ => {}
                    }
                    subs.push(s);
                }
            }
            let nd = Decoded::from_subs(*b"CELL", subs);
            let body = nd.encode(ctx);
            Record {
                rectype: *b"CELL",
                hdrflags: 0,
                buf: body,
            }
            .write(&mut out);
        }
    }
    if !sumcrea.is_empty() {
        ctx.print(&format!(
            "\nResetting the following {} Summoned Creatures persistence:\n",
            sumcrea.len()
        ));
        for (id, r) in &sumcrea {
            ctx.print(&format!("  CREA: {id}\n"));
            r.write(&mut out);
        }
    }
    if let Some(l) = lists {
        let (recs, summary) = l.split_at(l.len() - 1);
        for r in recs {
            out.extend(r);
        }
        ctx.print_bytes(&summary[0]);
    }
    crate::fsx::write(&patch_path, &out)
        .map_err(|e| ctx.abort(&format!("opening \"{patch_path}\" for output ({e})")))?;
    let mut options = Vec::new();
    for (on, name) in [
        (o.cellnames, "cellnames"),
        (o.fogbug, "fogbug"),
        (o.merge_lists, "merge_lists"),
        (o.summons_persist, "summons_persist"),
    ] {
        if on {
            options.push(name);
        }
    }
    let h = HeaderOptions {
        author: Some("tes3cmd multipatch".into()),
        description: Some(format!("options: {}", options.join(","))),
        multiline: false,
        update_masters: false,
        update_record_count: true,
        output: Default::default(),
    };
    update_header(ctx, &h, inst, &patch_path, true, true)?;
    if !o.no_activate {
        ctx.print("\n");
        inst.reload();
        crate::commands::active(ctx, inst, true, false, &[patch_file.to_string()])?;
    }
    Ok(())
}

/// `make_header`: a TES3 header for a new plugin (version 1.3, `tes3cmd`, `(generated)`).
pub fn make_header() -> Vec<u8> {
    let mut hedr = Vec::new();
    hedr.extend(1.3f32.to_le_bytes());
    hedr.extend(0u32.to_le_bytes());
    let mut z = |s: &str, n: usize| {
        let mut b = s.as_bytes().to_vec();
        b.truncate(n - 1);
        b.resize(n, 0);
        hedr.extend(b);
    };
    z("tes3cmd", 32);
    z("(generated)", 256);
    hedr.extend(0u32.to_le_bytes());
    let mut sub = b"HEDR".to_vec();
    sub.extend((hedr.len() as u32).to_le_bytes());
    sub.extend(hedr);
    let mut out = Vec::new();
    Record {
        rectype: *b"TES3",
        hdrflags: 0,
        buf: sub,
    }
    .write(&mut out);
    out
}

fn check_fogbug(
    ctx: &mut Ctx,
    plugin: &str,
    d: Decoded,
    fogcell: &mut BTreeMap<String, (Decoded, String)>,
) {
    let name = d.id();
    let data = match d.get(b"DATA").map(|s| &s.fields) {
        Some(Fields::Interior { fog, .. }) => Some(*fog as f64),
        _ => None,
    };
    let ambi = match d.get(b"AMBI").map(|s| &s.fields) {
        Some(Fields::Generic { slots, .. }) => {
            slots.get(3).filter(|v| v.is_defined()).map(Val::num)
        }
        _ => None,
    };
    let (Some(fd), Some(fa)) = (data, ambi) else {
        return;
    };
    if fa != fd {
        ctx.print(&format!(
            "{plugin}: [CELL {name}] Warning, Fog Density in DATA ({}) != AMBI ({})\n",
            perl_num_str(fd),
            perl_num_str(fa)
        ));
    }
    if fa == 0.0 || fd == 0.0 {
        if ctx.verbose {
            ctx.print(&format!("  [FOGBUG] {plugin}\t\tCELL: {name}\n"));
        }
        fogcell.insert(name, (d, plugin.to_string()));
    } else if fogcell.contains_key(&name) {
        if ctx.verbose {
            ctx.print(&format!("  [FOGBUG] CORRECTED {plugin} CELL: {name}\n"));
        }
        fogcell.remove(&name);
    }
}

fn check_rename(ctx: &mut Ctx, plugin: &str, d: &Decoded, rencell: &mut BTreeMap<String, Rename>) {
    let name = slot(d, b"NAME", 0).bytes();
    let (x, y) = match d.get(b"DATA").map(|s| &s.fields) {
        Some(Fields::Exterior { x, y, .. }) => (x.to_string(), y.to_string()),
        _ => (String::new(), String::new()),
    };
    let coord = format!("{x}, {y}");
    let shown = |b: &[u8]| bytes_str(b);
    match rencell.get_mut(&coord) {
        Some(c) => {
            if c.name == name {
                return;
            }
            if name == c.orig {
                if ctx.verbose {
                    ctx.print(&format!(
                        "  [REVNAME] CELL: ({coord}) RENAME REVERTED by: [{plugin}] from: \"{}\" to: \"{}\". Reversion will be undone.\n",
                        shown(&c.name),
                        shown(&name)
                    ));
                }
                c.output += 1;
            } else if !name.is_empty() && name != b"0" {
                if ctx.verbose {
                    ctx.print(&format!("  [REVNAME] CELL: ({coord}) Renamed by: [{plugin}] from: \"{}\" to: \"{}\"\n", shown(&c.name), shown(&name)));
                    ctx.print(&format!(
                        "  [REVNAME] CELL: ({coord}) Replacing: [{}]:\"{}\" with: [{plugin}]:\"{}\"\n",
                        c.plug,
                        shown(&c.name),
                        shown(&name)
                    ));
                }
                let subs: Vec<_> = [b"NAME", b"DATA"]
                    .iter()
                    .filter_map(|t| d.get(t).cloned())
                    .collect();
                let nd = Decoded::from_subs(*b"CELL", subs);
                let body = nd.encode(ctx);
                let mut rec = Vec::new();
                Record {
                    rectype: *b"CELL",
                    hdrflags: 0,
                    buf: body,
                }
                .write(&mut rec);
                c.name = name;
                c.plug = plugin.to_string();
                c.rec = Some(rec);
            } else if ctx.verbose {
                ctx.print(&format!(
                    "  [REVNAME] CELL: ({coord}) Skipping null name: [{plugin}]:\"{}\"\n",
                    shown(&name)
                ));
            }
        }
        None => {
            rencell.insert(
                coord,
                Rename {
                    orig: name.clone(),
                    name,
                    plug: plugin.to_string(),
                    output: 0,
                    rec: None,
                },
            );
        }
    }
}
