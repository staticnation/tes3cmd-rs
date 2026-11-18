//! Records as text, as tes3cmd prints them (`tr->tostr`, each subrecord's `tostr`): the
//! formatters tes3cmd generates from its record definitions (`tostr_table`, made from the Perl by
//! `tools/gen_tostr.pl` and checked against it), and the ones it writes by hand, ported here.
//! Output is bytes: plugin strings are printed as they are stored, as Perl prints them.

use std::collections::HashMap;

use crate::ctx::Ctx;
use crate::decode::{Decoded, Fields, Sub, starts_group};
use crate::pack::{Val, perl_num_str, unpack};
use crate::tostr_table::{NAMED, TABLES, TOSTR};

/// What tes3cmd keeps between records while printing them.
#[derive(Default)]
pub struct State {
    /// `$CURRENT_DIAL`: `Type:id` of the last DIAL decoded, for its INFOs.
    pub current_dial: Option<String>,
    /// `%FACTION_RANK`: each faction's rank names, as its RNAMs are printed.
    pub faction_rank: HashMap<String, Vec<Vec<u8>>>,
    /// `VERBOSE`: colors printed with their hex too.
    pub verbose: bool,
    /// `%FORMAT_SUBTYPE`: the subtype `getfield` found a field name in, by record type.
    pub format_subtype: HashMap<(String, String), [u8; 4]>,
    /// `msgonce` has spoken: its key is `{@_}` in scalar context (always 1), so it says only
    /// its first message, ever.
    pub msgonce_said: bool,
}

// ---------------------------------------------------------------- Perl's sprintf

/// Perl's `sprintf` for what tes3cmd's formats use: `%s`, `%d`, `%f`, `%x`, `%%`, with a width,
/// a precision and `-`.
pub fn sprintf(fmt: &[u8], args: &[Val]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut a = args.iter();
    let mut i = 0;
    while i < fmt.len() {
        let c = fmt[i];
        if c != b'%' {
            out.push(c);
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        if fmt.get(i) == Some(&b'%') {
            out.push(b'%');
            i += 1;
            continue;
        }
        let mut left = false;
        let mut zero = false;
        while let Some(&f) = fmt.get(i) {
            match f {
                b'-' => left = true,
                b'0' => zero = true,
                b'+' | b' ' | b'#' => {}
                _ => break,
            }
            i += 1;
        }
        let mut width = 0usize;
        while let Some(d) = fmt.get(i).filter(|d| d.is_ascii_digit()) {
            width = width * 10 + (d - b'0') as usize;
            i += 1;
        }
        let mut prec: Option<usize> = None;
        if fmt.get(i) == Some(&b'.') {
            i += 1;
            let mut p = 0;
            while let Some(d) = fmt.get(i).filter(|d| d.is_ascii_digit()) {
                p = p * 10 + (d - b'0') as usize;
                i += 1;
            }
            prec = Some(p);
        }
        let Some(&conv) = fmt.get(i) else {
            out.extend_from_slice(&fmt[start..]);
            break;
        };
        i += 1;
        let v = a.next().cloned().unwrap_or(Val::Undef);
        let body: Vec<u8> = match conv {
            b's' => {
                let mut b = v.bytes();
                if let Some(p) = prec {
                    b.truncate(p);
                }
                b
            }
            b'd' | b'i' => int_of(&v).to_string().into_bytes(),
            b'u' => (int_of(&v) as u64).to_string().into_bytes(),
            b'x' => format!("{:x}", int_of(&v)).into_bytes(),
            b'X' => format!("{:X}", int_of(&v)).into_bytes(),
            b'f' => format!("{:.*}", prec.unwrap_or(6), v.num()).into_bytes(),
            b'c' => vec![int_of(&v) as u8],
            _ => fmt[start..i].to_vec(),
        };
        if body.len() >= width {
            out.extend(body);
        } else if left {
            out.extend(&body);
            out.extend(std::iter::repeat_n(b' ', width - body.len()));
        } else if zero && conv != b's' {
            let (sign, digits) = if body.first() == Some(&b'-') {
                (&body[..1], &body[1..])
            } else {
                (&body[..0], &body[..])
            };
            out.extend(sign);
            out.extend(std::iter::repeat_n(b'0', width - body.len()));
            out.extend(digits);
        } else {
            out.extend(std::iter::repeat_n(b' ', width - body.len()));
            out.extend(body);
        }
    }
    out
}

/// A value as `%d` takes it: truncated toward zero.
fn int_of(v: &Val) -> i64 {
    match v {
        Val::Int(i) => *i,
        other => {
            let n = other.num();
            if n.is_finite() { n.trunc() as i64 } else { -1 }
        }
    }
}

/// A value as Perl interpolates it.
fn s(v: &Val) -> Vec<u8> {
    v.bytes()
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

// ---------------------------------------------------------------- the tables

fn named(name: &str) -> &'static [(&'static str, &'static str)] {
    NAMED
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, t)| *t)
        .unwrap_or(&[])
}

/// `$TABLE{$key}`, the key as Perl stringifies the value.
fn look(table: &'static [(&'static str, &'static str)], key: &Val) -> Option<&'static str> {
    let k = String::from_utf8_lossy(&key.bytes()).into_owned();
    table.iter().find(|(a, _)| *a == k).map(|(_, b)| *b)
}

/// `$TES3::NAME{$key}`: undefined when the table has no such key.
fn lk(name: &str, key: &Val) -> Val {
    look(named(name), key)
        .map(|s| Val::Bytes(s.as_bytes().to_vec()))
        .unwrap_or(Val::Undef)
}

/// `sprintf`, with Perl's warning for each undefined value it is given.
fn spf(ctx: &mut Ctx, fmt: &[u8], args: &[Val]) -> Vec<u8> {
    for a in args {
        if !a.is_defined() {
            ctx.msg("Use of uninitialized value in sprintf");
        }
    }
    sprintf(fmt, args)
}

/// A value interpolated into a string, with Perl's warning when it is undefined.
fn cc(ctx: &mut Ctx, v: &Val) -> Vec<u8> {
    if !v.is_defined() {
        ctx.msg("Use of uninitialized value in concatenation (.) or string");
    }
    v.bytes()
}

/// `flags_tostr`: `0x0400 (Persistent)`, the names that are set, sorted.
pub fn flags_tostr(flags: i64, table: &[(&str, &str)]) -> String {
    let mut names: Vec<String> = table
        .iter()
        .filter(|(n, _)| n.chars().count() != 1)
        .filter(|(_, v)| {
            let val = parse_num(v);
            (flags & val) != 0 || (flags == 0 && val == 0)
        })
        .map(|(n, _)| ucfirst_words(n))
        .collect();
    names.sort();
    format!("0x{:04x} ({})", flags, names.join(", "))
}

fn parse_num(s: &str) -> i64 {
    s.parse::<i64>()
        .or_else(|_| s.parse::<f64>().map(|f| f as i64))
        .unwrap_or(0)
}

/// `s/([[:alnum:]]+)/ucfirst($1)/ge`.
fn ucfirst_words(s: &str) -> String {
    let mut out = String::new();
    let mut start = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            start = false;
        } else {
            out.push(c);
            start = true;
        }
    }
    out
}

/// `color_tostr`.
fn color_tostr(color: &Val, verbose: bool) -> String {
    let c = int_of(color);
    let (r, g, b) = (c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff);
    if verbose {
        format!("RGB=0x{c:x} ({r},{g},{b})")
    } else {
        format!("RGB=({r},{g},{b})")
    }
}

thread_local! {
    /// `WRAP`: tes3cmd decides it before reading its options, from any argument that is exactly
    /// `--wrap` or `-wrap`, whatever the command.
    static WRAP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn set_wrap(on: bool) {
    WRAP.with(|w| w.set(on));
}

fn wrapping() -> bool {
    WRAP.with(|w| w.get())
}

thread_local! {
    /// Set when a formatter died as tes3cmd's does: `unknown_data` on 2 or 3 bytes dereferences
    /// an array as a scalar ("Not a SCALAR reference"), which ends the plugin.
    static DIED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The message tes3cmd dies with in `unknown_data`.
pub const NOT_A_SCALAR: &str = "Not a SCALAR reference";

/// `unknown_data`.
pub fn unknown_data(buf: &[u8]) -> Vec<u8> {
    let len = buf.len();
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    if len > 4 {
        let sep = if len < 25 { "  " } else { "\n\t" };
        let printable: Vec<u8> = buf
            .iter()
            .copied()
            .filter(|&c| (0o40..0o177).contains(&c))
            .take(80)
            .collect();
        cat(&[
            format!("[UNKNOWN_DATA: len:{len}{sep}hex:{}{sep}str:", hex(buf)).as_bytes(),
            &printable,
            b"]",
        ])
    } else if len == 4 {
        let f = f32::from_le_bytes(buf.try_into().unwrap());
        let l = i32::from_le_bytes(buf.try_into().unwrap());
        let s0 = i16::from_le_bytes([buf[0], buf[1]]);
        let s1 = i16::from_le_bytes([buf[2], buf[3]]);
        let c: Vec<i8> = buf.iter().map(|&b| b as i8).collect();
        format!(
            "[UNKNOWN_WORD: 0x{} f:{:.2}  l:{l}  (s:{s0} {s1})  (c:{} {} {} {})]",
            hex(buf),
            f as f64,
            c[0],
            c[1],
            c[2],
            c[3]
        )
        .into_bytes()
    } else if len > 1 {
        DIED.with(|d| d.set(true));
        Vec::new()
    } else if len == 1 {
        format!("[UNKNOWN_BYTE: 0x{} ({})]", hex(buf), buf[0] as i8).into_bytes()
    } else {
        b"UNKNOWN_NULL: (received zero length buffer)".to_vec()
    }
}

/// `tr/\r//d if ($^O eq 'linux')`.
fn no_cr(mut b: Vec<u8>) -> Vec<u8> {
    if cfg!(target_os = "linux") {
        b.retain(|&c| c != b'\r');
    }
    b
}

// ---------------------------------------------------------------- one subrecord

/// Each subrecord's `_id_`: the record's id as decoding had it before that subrecord (the
/// last `id` field so far, lower case), undefined before the first.
pub fn sub_ids(d: &Decoded) -> Vec<Option<String>> {
    let mut cur: Option<String> = None;
    let mut out = Vec::with_capacity(d.subs.len());
    for sub in &d.subs {
        out.push(cur.clone());
        let slot = crate::schema_table::ID_FIELD
            .iter()
            .find(|(r, s, _)| r.as_bytes() == d.rectype && s.as_bytes() == sub.tag)
            .map(|x| x.2);
        if let (Some(slot), Fields::Generic { slots, .. }) = (slot, &sub.fields)
            && let Some(v) = slots.get(slot as usize)
            && v.is_defined()
        {
            cur = Some(crate::decode::lc(&v.bytes()));
        }
    }
    out
}

type Row = (
    &'static str,
    &'static str,
    bool,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
);

fn row(rectype: &[u8; 4], tag: &[u8; 4]) -> Option<&'static Row> {
    TOSTR
        .iter()
        .find(|r| r.0.as_bytes() == rectype && r.1.as_bytes() == tag)
}

/// A subrecord's fields by name, as the generic decoder made them.
struct F<'a> {
    row: Option<&'static Row>,
    sub: &'a Sub,
}

impl F<'_> {
    fn get(&self, name: &str) -> Val {
        let (Some(row), Fields::Generic { slots, map, .. }) = (self.row, &self.sub.fields) else {
            return Val::Undef;
        };
        row.4
            .iter()
            .position(|n| *n == name)
            .and_then(|i| map.get(i))
            .and_then(|&s| slots.get(s as usize).cloned())
            .unwrap_or(Val::Undef)
    }
}

/// The generated `tostr`: the row's format over the fields, a lookup shown `(name)` and flags
/// `[0x.. (...)]`.
fn generic(
    ctx: &mut Ctx,
    rectype: &[u8; 4],
    id: Option<&str>,
    row: &'static Row,
    f: &F,
) -> Vec<u8> {
    let mut vals = Vec::new();
    for (name, kind) in row.4.iter().zip(row.5.iter()) {
        let v = f.get(name);
        if let Some(n) = kind.strip_prefix('L') {
            let t = TABLES[n.parse::<usize>().unwrap()].1;
            match look(t, &v).filter(|s| !s.is_empty() && *s != "0") {
                Some(sym) => vals.push(Val::Bytes(format!("({sym})").into_bytes())),
                None => {
                    if id.is_none() {
                        ctx.msg("Use of uninitialized value in concatenation (.) or string");
                    }
                    let id = id.unwrap_or("");
                    ctx.error(&format!(
                        "{}::{}::tostr(ID={id}): do not know how to lookup symbolic value for: \"{name}\" ({})",
                        String::from_utf8_lossy(rectype),
                        row.1,
                        String::from_utf8_lossy(&v.bytes())
                    ));
                    vals.push(v);
                }
            }
        } else if let Some(n) = kind.strip_prefix('F') {
            let t = TABLES[n.parse::<usize>().unwrap()].1;
            vals.push(Val::Bytes(
                format!("[{}]", flags_tostr(int_of(&v), t)).into_bytes(),
            ));
        } else {
            vals.push(v);
        }
    }
    sprintf(row.3.as_bytes(), &vals)
}

/// An enchantment's effect (`@RD_Enchantment`: ALCH, ENCH, SPEL ENAM).
fn enchantment(ctx: &mut Ctx, f: &F) -> Vec<u8> {
    let mut spell = lk("SPELL_EFFECT", &f.get("spell_effect"));
    if !spell.is_defined() {
        ctx.msg("Use of uninitialized value $spell in pattern match (m//)");
    }
    let low = String::from_utf8_lossy(&spell.bytes()).to_ascii_lowercase();
    if low.ends_with("_attribute") {
        let a = cc(ctx, &lk("ATTRIBUTE", &f.get("attribute")));
        spell = Val::Bytes(cat(&[&spell.bytes(), b"/", &a]));
    } else if low.ends_with("_skill") {
        let s = cc(ctx, &lk("SKILL", &f.get("skill")));
        spell = Val::Bytes(cat(&[&spell.bytes(), b"/", &s]));
    }
    let range = lk("RANGE_TYPE", &f.get("range"));
    spf(
        ctx,
        b"Spell_Effect:(%s)  Range:(%s)  Area:%d  Duration:%d  Mag_Min:%d  Mag_Max:%d",
        &[
            spell,
            range,
            f.get("area"),
            f.get("duration"),
            f.get("mag_min"),
            f.get("mag_max"),
        ],
    )
}

fn grid(title: &str, vals: &[Vec<u8>], n: usize) -> Vec<u8> {
    let mut out = title.as_bytes().to_vec();
    for i in 0..n {
        out.extend(format!("\n   {i:>2}:").into_bytes());
        let row: Vec<&[u8]> = (0..n)
            .map(|j| vals.get(i * n + j).map(Vec::as_slice).unwrap_or(b""))
            .collect();
        out.extend(row.join(&b","[..]));
    }
    out
}

fn hex6(raw: &[u8]) -> Vec<Vec<u8>> {
    raw.chunks(3)
        .map(|c| {
            c.iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>()
                .into_bytes()
        })
        .collect()
}

/// One subrecord as tes3cmd prints it after its type (`tsr->tostr`).
pub fn sub_tostr(
    ctx: &mut Ctx,
    st: &mut State,
    d: &Decoded,
    sub: &Sub,
    sub_id: Option<&str>,
) -> Vec<u8> {
    let rectype = &d.rectype;
    let id = sub_id.unwrap_or("").to_string();
    let r = row(rectype, &sub.tag);
    let f = F { row: r, sub };
    let raw = &sub.raw;
    match (&rectype[..], &sub.tag[..]) {
        (b"ALCH" | b"ENCH" | b"SPEL", b"ENAM") => return enchantment(ctx, &f),
        (b"ARMO", b"AODT") => {
            let ty = f.get("type");
            let w = f.get("weight").num();
            let mut class = |k: &str| {
                let key = format!("{}.{k}", String::from_utf8_lossy(&ty.bytes()));
                match named("ARMOR_CLASS").iter().find(|(a, _)| *a == key) {
                    Some((_, b)) => b.parse::<f64>().unwrap_or(0.0),
                    None => {
                        ctx.msg("Use of uninitialized value in numeric gt (>)");
                        0.0
                    }
                }
            };
            let wc = if w > class("medium") {
                "Heavy"
            } else if w > class("light") {
                "Medium"
            } else {
                "Light"
            };
            let fmt = format!(
                "Type:(%s)  Weight:%0.2f ({wc})  Value:%d  Health:%d  Enchantment:%d  AR:%d"
            );
            return spf(
                ctx,
                fmt.as_bytes(),
                &[
                    lk("ARMOR_TYPE", &ty),
                    f.get("weight"),
                    f.get("value"),
                    f.get("health"),
                    f.get("enchantment"),
                    f.get("ar"),
                ],
            );
        }
        (b"BOOK", b"TEXT") => {
            let text = cat(&[b"Text:", &no_cr(s(&f.get("text")))]);
            return if wrapping() {
                wrap(b"", b"\t", &text)
            } else {
                text
            };
        }
        (b"BSGN" | b"CLAS" | b"MGEF" | b"RACE" | b"SKIL", b"DESC") => {
            let text = cat(&[b"Description:", &no_cr(s(&f.get("description")))]);
            return if wrapping() {
                wrap(b"", b"\t", &text)
            } else {
                text
            };
        }
        (b"GMST", b"STRV") => return cat(&[b"String:", &no_cr(s(&f.get("string")))]),
        (b"CELL", b"AMBI") => {
            let parts: Vec<String> = ["ambient", "sunlight", "fog"]
                .iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        ucfirst_words(k),
                        color_tostr(&f.get(k), st.verbose)
                    )
                })
                .collect();
            return cat(&[
                parts.join("  ").as_bytes(),
                &sprintf(b" Fog_Density:%0.2f", &[f.get("fog_density")]),
            ]);
        }
        (_, b"SLLD" | b"CHRD") => {
            let v: Vec<Vec<u8>> = unpack("l*", raw).iter().map(s).collect();
            return cat(&[b"Long_Array: ", &v.join(&b", "[..])]);
        }
        (_, b"SLSD") => {
            let v: Vec<Vec<u8>> = unpack("s*", raw).iter().map(s).collect();
            return cat(&[b"Short_Array: ", &v.join(&b", "[..])]);
        }
        (_, b"SLFD") => {
            // sprintf("$_=%0.2f", $_): the value as Perl prints it, then rounded.
            let v: Vec<Vec<u8>> = unpack("f*", raw)
                .iter()
                .map(|x| cat(&[&s(x), b"=", &sprintf(b"%0.2f", std::slice::from_ref(x))]))
                .collect();
            return cat(&[b"Float_Array: ", &v.join(&b", "[..])]);
        }
        (b"CREC" | b"NPCC", b"WIDX") => {
            let ammo = if f.get("ammo_flag").num() == 0.0 {
                ""
            } else {
                " (Ammo)"
            };
            return cat(&[
                &sprintf(b"Equipped_Index:%d", &[f.get("equipped_index")]),
                ammo.as_bytes(),
            ]);
        }
        (b"CELL" | b"REFR", b"FRMR") | (b"CELL", b"MVRF") => {
            let idx = u32::from_le_bytes(
                raw.get(..4)
                    .and_then(|b| b.try_into().ok())
                    .unwrap_or([0; 4]),
            );
            return format!("ObjIdx:{}  MastIdx:{}", idx & 0xff_ffff, idx >> 24).into_bytes();
        }
        (b"CELL", b"CNAM") => {
            return match &sub.fields {
                Fields::OwnerFaction(n) => cat(&[b"Owner_Faction:", n]),
                _ => unknown_data(raw),
            };
        }
        (b"CELL", b"CNDT") => {
            return cat(&[
                b"X:",
                &s(&f.get("x")),
                b"  Y:",
                &s(&f.get("y")),
                b" (External Cell Destination)",
            ]);
        }
        (b"CELL", b"INDX" | b"UNAM") | (b"LAND", b"DATA") => return unknown_data(raw),
        (b"CELL", b"DATA") => {
            return match &sub.fields {
                Fields::Interior { flags, fog, .. } => {
                    let fl = flags_tostr(*flags as i64, named("CELL_FLAGS"));
                    sprintf(
                        b"(Interior) Fog_Density:%0.2f  Flags:%s",
                        &[Val::Num(*fog as f64), Val::Bytes(fl.into_bytes())],
                    )
                }
                Fields::Exterior { flags, x, y } => {
                    let fl = flags_tostr(*flags as i64, named("CELL_FLAGS"));
                    sprintf(
                        b"(Exterior) Coordinates: (%d, %d)  Flags:%s",
                        &[
                            Val::Int(*x as i64),
                            Val::Int(*y as i64),
                            Val::Bytes(fl.into_bytes()),
                        ],
                    )
                }
                Fields::Position(v) => sprintf(
                    b"X:%0.3f  Y:%0.3f  Z:%0.3f  X_Angle:%0.4f  Y_Angle:%0.4f  Z_Angle:%0.4f",
                    v,
                ),
                _ => sprintf(
                    b"X:%0.3f  Y:%0.3f  Z:%0.3f  X_Angle:%0.4f  Y_Angle:%0.4f  Z_Angle:%0.4f",
                    &[],
                ),
            };
        }
        (b"CELL", b"INTV") => {
            return match &sub.fields {
                Fields::HealthLeft(v) => sprintf(
                    b"Health_Left:%d",
                    &[v.map(|v| Val::Int(v as i64)).unwrap_or(Val::Undef)],
                ),
                Fields::TimeLeft(t) => sprintf(b"Time_Left:%0.2f", &[Val::Num(*t as f64)]),
                Fields::WaterHeight(v) => sprintf(
                    b"Water_Height:%d",
                    &[v.map(|v| Val::Int(v as i64)).unwrap_or(Val::Undef)],
                ),
                _ => sprintf(b"Water_Height:%d", &[]),
            };
        }
        (b"CREA", b"FLAG") => {
            let fl = int_of(&f.get("flags"));
            let mut movement = flags_tostr(fl, named("CREATURE_MOVEMENT_FLAGS"));
            for pat in [", Movement:None", "Movement:None, "] {
                if let Some(k) = movement.find(pat) {
                    movement.replace_range(k..k + pat.len(), "");
                    break;
                }
            }
            let strip = |x: String| match x.find(char::is_whitespace) {
                Some(k) => x[k..].trim_start().to_string(),
                None => x,
            };
            let mut blood = strip(flags_tostr(fl, named("CREATURE_BLOOD_FLAGS")));
            if !blood.to_lowercase().contains("blood") {
                blood = "(Red_Blood)".into();
            }
            let other = strip(flags_tostr(fl, named("CREATURE_FLAGS")));
            let all = format!("{movement}{blood}{other}")
                .replace("()", "")
                .replace(")(", ", ");
            return all.into_bytes();
        }
        (b"DIAL", b"DATA") => {
            let ty = raw
                .first()
                .map(|&b| Val::Int(b as i64))
                .unwrap_or(Val::Undef);
            return match look(named("DIAL_TYPE"), &ty).filter(|s| !s.is_empty()) {
                Some(sym) => format!("Type:({sym})").into_bytes(),
                None => cat(&[b"Type:", &s(&ty)]),
            };
        }
        (b"FACT", b"NAME") => {
            st.faction_rank.clear();
            return s(&f.get("id"));
        }
        (b"FACT", b"RNAM") => {
            let rank = s(&f.get("rank"));
            st.faction_rank
                .entry(id.clone())
                .or_default()
                .push(rank.clone());
            return cat(&[b"Rank:", &rank]);
        }
        (b"FACT", b"FADT") => {
            let ranks = st.faction_rank.get(&id).cloned().unwrap_or_default();
            let a = |k: &str| lk("ATTRIBUTE", &f.get(k));
            let fl = flags_tostr(int_of(&f.get("flags")), named("FACT_FLAGS"));
            let mut lines = vec![spf(
                ctx,
                b"Attrib_1:(%s)  Attrib_2:(%s)  Flags:%s  Unknown:%d",
                &[
                    a("attrib_1"),
                    a("attrib_2"),
                    Val::Bytes(fl.into_bytes()),
                    f.get("unknown"),
                ],
            )];
            let mut n = 1;
            while f.get(&format!("at1_{n}")).is_defined() && ranks.get(n - 1).is_some() {
                let shim = if n < 10 { " " } else { "" };
                let fmt = format!(
                    "\t%-20s {shim}At1_{n}:%2d  {shim}At2_{n}:%2d  {shim}Sk1_{n}:%2d  {shim}Sk2_{n}:%2d  {shim}Rep_{n}:%3d"
                );
                let label = cat(&[b"(Rank:", &ranks[n - 1], b")"]);
                lines.push(sprintf(
                    fmt.as_bytes(),
                    &[
                        Val::Bytes(label),
                        f.get(&format!("at1_{n}")),
                        f.get(&format!("at2_{n}")),
                        f.get(&format!("sk1_{n}")),
                        f.get(&format!("sk2_{n}")),
                        f.get(&format!("rep_{n}")),
                    ],
                ));
                n += 1;
            }
            let mut sk = |k: usize| {
                let name = cc(ctx, &lk("SKILL", &f.get(&format!("sk_{k}"))));
                Val::Bytes(cat(&[format!("sk_{k}:(").as_bytes(), &name, b")"]))
            };
            let row1 = [sk(1), sk(2), sk(3)];
            let row2 = [sk(4), sk(5), sk(6)];
            lines.push(sprintf(b"\t%-20s  %-20s  %-20s", &row1));
            lines.push(sprintf(b"\t%-20s  %-20s  %-20s", &row2));
            return lines.join(&b"\n"[..]);
        }
        (b"INFO", b"BNAM") => {
            let mut out = b"Result:\t".to_vec();
            for c in no_cr(s(&f.get("result"))) {
                if c == b'\n' {
                    out.extend(b"\n\t\t");
                } else {
                    out.push(c);
                }
            }
            return out;
        }
        (b"INFO", b"DATA") => {
            if st
                .current_dial
                .as_deref()
                .is_some_and(|d| d.to_ascii_lowercase().starts_with("journal"))
            {
                return cat(&[b"JournalIndex:", &s(&f.get("disposition"))]);
            }
            let none = |v: Val| {
                if v.num() == -1.0 {
                    Val::Bytes(b"None".to_vec())
                } else {
                    v
                }
            };
            return spf(
                ctx,
                b"Disposition:%s  Rank:%s  Sex:%s  PC_Rank:%s",
                &[
                    f.get("disposition"),
                    none(f.get("rank")),
                    lk("SEX", &f.get("sex")),
                    none(f.get("pc_rank")),
                ],
            );
        }
        (b"INFO", b"NAME") => {
            let text = no_cr(s(&f.get("response")));
            if text.len() < 60 {
                return cat(&[b"Response: ", &text]);
            }
            let text = if wrapping() {
                wrap(b"	", b"	", &text)
            } else {
                text
            };
            return cat(&[
                b"Response:
",
                &text,
            ]);
        }
        (b"INGR", b"IRDT") => {
            let mut effects: Vec<Vec<u8>> = Vec::new();
            for i in 1..=4 {
                let e = f.get(&format!("effect_{i}"));
                if e.num() == -1.0 {
                    continue;
                }
                let name = look(named("SPELL_EFFECT"), &e)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.as_bytes().to_vec())
                    .unwrap_or_else(|| cat(&[&s(&e), b"???"]));
                let effect = cat(&[format!("Effect_{i}:(").as_bytes(), &name]);
                let low = String::from_utf8_lossy(&effect).to_ascii_lowercase();
                if low.ends_with("_attribute") {
                    let a = cc(ctx, &lk("ATTRIBUTE", &f.get(&format!("attribute_{i}"))));
                    effects.push(cat(&[&effect, b"/", &a, b")"]));
                } else if low.ends_with("_skill") {
                    let s = cc(ctx, &lk("SKILL", &f.get(&format!("skill_{i}"))));
                    effects.push(cat(&[&effect, b"/", &s, b")"]));
                } else {
                    effects.push(cat(&[&effect, b")"]));
                }
            }
            let tail = if effects.is_empty() {
                Vec::new()
            } else {
                cat(&[b"\n\t", &effects.join(&b"\n\t"[..])])
            };
            let mut fmt = b"Weight:%0.2f  Value:%d".to_vec();
            fmt.extend(tail);
            return sprintf(&fmt, &[f.get("weight"), f.get("value")]);
        }
        (b"LAND", b"VCLR") => return grid("(Vertex RGB Color Array, 65 x 65):", &hex6(raw), 65),
        (b"LAND", b"VNML") => return grid("(Vertex Normal Array, 65 x 65):", &hex6(raw), 65),
        (b"LAND", b"VTEX") => {
            let v: Vec<Vec<u8>> = unpack("s*", raw).iter().map(s).collect();
            return grid("(Vertex Texture Index Array, 16 x 16):", &v, 16);
        }
        (b"LAND", b"VHGT") => {
            let mut v = unpack("fc4225sc", raw);
            let offset = if v.is_empty() {
                Val::Undef
            } else {
                v.remove(0)
            };
            let unknown_2 = v.pop().unwrap_or(Val::Undef);
            let unknown_1 = if v.is_empty() {
                Val::Undef
            } else {
                v.remove(0)
            };
            let cells: Vec<Vec<u8>> = v.iter().map(s).collect();
            let title = format!(
                "Offset:{}  Unknown_1:{}  Unknown_2:{}\n\t(Vertex Height Array, 65 x 65):",
                String::from_utf8_lossy(&s(&offset)),
                String::from_utf8_lossy(&s(&unknown_1)),
                String::from_utf8_lossy(&s(&unknown_2))
            );
            return grid(&title, &cells, 65);
        }
        (b"LAND", b"WNAM") => {
            let mut out = b"(low-LOD Heightmap, 9 x 9):".to_vec();
            for i in 1..=9 {
                out.extend(format!("\n   {:>2}:", i - 1).into_bytes());
                let row: Vec<Vec<u8>> = (1..=9)
                    .map(|j| s(&f.get(&format!("i_{}", (i - 1) * 9 + j))))
                    .collect();
                out.extend(row.join(&b","[..]));
            }
            return out;
        }
        (b"LIGH", b"LHDT") => {
            let fl = flags_tostr(int_of(&f.get("flags")), named("LHDT_FLAGS"));
            return sprintf(
                b"Weight:%0.2f  Value:%d  Time:%d  Radius:%d  Color:%s\n\tFlags:%s",
                &[
                    f.get("weight"),
                    f.get("value"),
                    f.get("time"),
                    f.get("radius"),
                    Val::Bytes(color_tostr(&f.get("color"), st.verbose).into_bytes()),
                    Val::Bytes(fl.into_bytes()),
                ],
            );
        }
        (b"NPC_", b"NPDT") => return npdt(ctx, raw),
        (b"PGRD", b"PGRP") => {
            let mut lines = vec![b"Points:".to_vec()];
            for (idx, p) in raw.chunks(16).filter(|c| c.len() == 16).enumerate() {
                let l = |k: usize| i32::from_le_bytes(p[k..k + 4].try_into().unwrap());
                let color = match p[12] {
                    0 => "red".to_string(),
                    1 => "blue".to_string(),
                    c => c.to_string(),
                };
                lines.push(
                    format!(
                        "[{idx:>3}]  x:{:>5}  y:{:>5}  z:{:>5}  color:{color}  nconn:{}  unknown:{:02x}{:02x}",
                        l(0),
                        l(4),
                        l(8),
                        p[13],
                        p[14],
                        p[15]
                    )
                    .into_bytes(),
                );
            }
            return lines.join(&b"\n\t"[..]);
        }
        (b"PGRD", b"PGRC") => {
            let points: Vec<u8> = d
                .get(b"PGRP")
                .map(|p| {
                    p.raw
                        .chunks(16)
                        .filter(|c| c.len() == 16)
                        .map(|c| c[13])
                        .collect()
                })
                .unwrap_or_default();
            let count = d
                .get(b"DATA")
                .map(|s| {
                    int_of(
                        &F {
                            row: row(rectype, b"DATA"),
                            sub: s,
                        }
                        .get("pointcount"),
                    )
                })
                .unwrap_or(0);
            if points.len() as i64 != count {
                ctx.error(&format!(
                    "PGRD.PGRC ({id}) (N points={} != pointcount({count})",
                    points.len()
                ));
            }
            let conn: Vec<u32> = raw
                .chunks(4)
                .filter(|c| c.len() == 4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            let mut lines = vec![b"Connections:".to_vec()];
            let mut ci = 0;
            for p1 in 0..count.max(0) as usize {
                let n = points.get(p1).copied().unwrap_or(0) as usize;
                let ps: Vec<String> = (0..n)
                    .map(|_| {
                        let v = conn.get(ci).map(|v| v.to_string()).unwrap_or_default();
                        ci += 1;
                        v
                    })
                    .collect();
                if !ps.is_empty() {
                    lines.push(format!("{p1:>3} -> {}", ps.join(", ")).into_bytes());
                }
            }
            return lines.join(&b"\n\t"[..]);
        }
        (b"RACE", b"RADT") => {
            let fl = flags_tostr(int_of(&f.get("flags")), named("RADT_FLAGS"));
            let mut out = sprintf(
                b"Flags:%s  Male_Height:%0.2f  Male_Weight:%0.2f  Female_Height:%0.2f  Female_Weight:%0.2f\n\t\tSkill Bonus:",
                &[Val::Bytes(fl.into_bytes()), f.get("male_height"), f.get("male_weight"), f.get("female_height"), f.get("female_weight")],
            );
            let mut total = 0i64;
            for i in 1..=7 {
                let (skill, bonus) = (f.get(&format!("skill_{i}")), f.get(&format!("bonus_{i}")));
                total += int_of(&bonus);
                if skill.num() > -1.0 {
                    let name = lk("SKILL", &skill);
                    if !name.is_defined() {
                        ctx.msg(&format!(
                            "Use of uninitialized value $TES3::SKILL{{\"{}\"}} in sprintf",
                            String::from_utf8_lossy(&skill.bytes())
                        ));
                    }
                    out.extend(sprintf(b"\n\t\t%-12s %3d", &[name, bonus]));
                }
            }
            out.extend(
                format!("\n\t\t(Bonus Total: {total})\n\t\tBase Attributes: Male/Female")
                    .into_bytes(),
            );
            for (i, attr) in [
                "Strength",
                "Intelligence",
                "Willpower",
                "Agility",
                "Speed",
                "Endurance",
                "Personality",
                "Luck",
            ]
            .iter()
            .enumerate()
            {
                let n = i + 1;
                out.extend(sprintf(
                    b"\n\t\t%-12s    %3d  %3d",
                    &[
                        Val::Bytes(attr.as_bytes().to_vec()),
                        f.get(&format!("attr_male_{n}")),
                        f.get(&format!("attr_female_{n}")),
                    ],
                ));
            }
            return out;
        }
        (b"REGN", b"WEAT") => {
            let v: Vec<Val> = raw.iter().map(|&b| Val::Int(b as i64)).collect();
            return match raw.len() {
                10 => sprintf(b"Clear:%d  Cloudy:%d  Foggy:%d  Overcast:%d\n\tRain:%d  Thunder:%d  Ash:%d  Blight:%d\n\tSnow:%d  Blizzard:%d", &v),
                _ => sprintf(b"Clear:%d  Cloudy:%d  Foggy:%d  Overcast:%d\n\tRain:%d  Thunder:%d  Ash:%d  Blight:%d", &v),
            };
        }
        (b"SCPT", b"SCTX") => return cat(&[b"Script_Text:\n", &no_cr(s(&f.get("script_text")))]),
        (b"SCPT", b"SCVR") => {
            let v = s(&f.get("variables"));
            let mut parts: Vec<&[u8]> = v.split(|&c| c == 0).collect();
            while parts.last().is_some_and(|p| p.is_empty()) {
                parts.pop();
            }
            return cat(&[b"Script_Variables: ", &parts.join(&b","[..])]);
        }
        (b"SKIL", b"SKDT") => {
            let mut out = spf(
                ctx,
                b"Attribute:(%s)  Specialization:(%s)",
                &[
                    lk("ATTRIBUTE", &f.get("attribute")),
                    lk("SPECIALIZATION", &f.get("specialization")),
                ],
            );
            let actions = named("SKILL_ACTIONS")
                .iter()
                .find(|(k, _)| *k == id)
                .map(|(_, v)| *v)
                .unwrap_or("");
            for (n, action) in actions
                .split('\u{1f}')
                .filter(|a| !a.is_empty())
                .enumerate()
            {
                let fmt = format!("\n\tUse_Value_{}(%s):%0.2f", n + 1);
                out.extend(sprintf(
                    fmt.as_bytes(),
                    &[
                        Val::Bytes(action.as_bytes().to_vec()),
                        f.get(&format!("use_value_{}", n + 1)),
                    ],
                ));
            }
            return out;
        }
        (b"SOUN", b"DATA") => {
            return sprintf(
                b"Volume:%0.2f  Min_Range:%d  Max_Range:%d",
                &[
                    Val::Num(f.get("volume").num() / 255.0),
                    f.get("min_range"),
                    f.get("max_range"),
                ],
            );
        }
        (b"TES3", b"HEDR") => {
            return crate::commands::hedr_tostr_bytes(&f.sub.fields);
        }
        (b"TES3", b"DATA") => {
            if let Fields::MasterSize(n) = sub.fields {
                return format!("Length:{}", perl_num_str(n)).into_bytes();
            }
        }
        _ => {}
    }
    match r {
        Some(row) if !row.2 => generic(ctx, rectype, sub_id, row, &f),
        _ => unknown_data(raw),
    }
}

/// NPC_.NPDT, 12 or 52 bytes: its own decoder and printer.
fn npdt(ctx: &mut Ctx, raw: &[u8]) -> Vec<u8> {
    match raw.len() {
        12 => {
            let v = unpack("SC6L", raw);
            sprintf(
                b"Level:%d  Disposition:%d  FactionIndex:%s  Rank:%s  Gold:%d",
                &[
                    v[0].clone(),
                    v[1].clone(),
                    v[2].clone(),
                    v[3].clone(),
                    v[7].clone(),
                ],
            )
        }
        52 => {
            let skills_raw = &raw[10..37];
            let skill_names = named("SKILL");
            let mut by_name: HashMap<String, i64> = HashMap::new();
            for (i, &lv) in skills_raw.iter().enumerate() {
                if let Some((_, n)) = skill_names.iter().find(|(k, _)| *k == i.to_string()) {
                    by_name.insert(n.to_ascii_lowercase(), lv as i64);
                }
            }
            let v = unpack("SC8x27CSSSCCCxL", raw);
            let mut names: Vec<&str> = skill_names
                .iter()
                .map(|(_, n)| *n)
                .filter(|n| *n != "None")
                .collect();
            names.sort();
            let mut skills = String::new();
            for (n, sk) in names.iter().enumerate() {
                skills.push_str(if n % 6 == 0 { "\n\t" } else { "  " });
                skills.push_str(&format!(
                    "{sk}:{}",
                    by_name
                        .get(&sk.to_ascii_lowercase())
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                ));
            }
            let fmt = format!(
                "Level:%d\n\tStr:%d  Int:%d  Wil:%d  Agi:%d  Spd:%d  End:%d  Per:%d  Lck:%d\n\tReputation:%d  Health:%d  Magicka:%d  Fatigue:%d  Disposition:%d\n\tFactionIndex:%s  Rank:%s  Gold:%d\n\t(Skills):{skills}"
            );
            let order = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
            let vals: Vec<Val> = order
                .iter()
                .map(|&i| v.get(i).cloned().unwrap_or(Val::Undef))
                .collect();
            sprintf(fmt.as_bytes(), &vals)
        }
        n => {
            ctx.error(&format!(
                "DECODER barfed on NPC_.NPDT, Invalid length: {n}, expected 52 or 12!"
            ));
            sprintf(
                b"Level:%d  Disposition:%d  FactionIndex:%s  Rank:%s  Gold:%d",
                &[],
            )
        }
    }
}

// ---------------------------------------------------------------- one record

/// Whether a formatter died since the last call (see `DIED`); clears it.
pub fn died() -> bool {
    DIED.with(|d| d.replace(false))
}

/// `tr->tostr`: the banner, then each subrecord, a group's first marked ` *`. `Err` when a
/// formatter died (`NOT_A_SCALAR`).
pub fn record_tostr(ctx: &mut Ctx, st: &mut State, d: &Decoded) -> Result<Vec<u8>, &'static str> {
    let rt = String::from_utf8_lossy(&d.rectype).into_owned();
    let id = record_id(d);
    let dial = match (&d.rectype, &st.current_dial) {
        (b"INFO", Some(c)) => format!(" ({c})"),
        _ => String::new(),
    };
    let mut out = format!(
        "Record: {rt} \"{id}\"{dial} Flags:{}\n",
        flags_tostr(d.hdrflags as i64, named("HDR_FLAGS"))
    )
    .into_bytes();
    let mut lines = Vec::new();
    let ids = sub_ids(d);
    for (sub, sid) in d.subs.iter().zip(&ids) {
        let margin: &[u8] = if starts_group(&d.rectype, &sub.tag) {
            b" *"
        } else {
            b"  "
        };
        let body = sub_tostr(ctx, st, d, sub, sid.as_deref());
        if died() {
            return Err(NOT_A_SCALAR);
        }
        lines.push(cat(&[margin, &sub.tag, b": ", &body]));
    }
    out.extend(lines.join(&b"\n"[..]));
    Ok(out)
}

/// `tr->id`, as the banner shows it.
pub fn record_id(d: &Decoded) -> String {
    match &d.rectype {
        b"TES3" => "()".into(),
        _ => d.id(),
    }
}

/// What a DIAL leaves for the INFOs after it (`$CURRENT_DIAL`, set by its DATA's decoder).
pub fn note_dial(st: &mut State, d: &Decoded) {
    if &d.rectype != b"DIAL" {
        return;
    }
    if let Some(data) = d.get(b"DATA") {
        let ty = data
            .raw
            .first()
            .map(|&b| Val::Int(b as i64))
            .unwrap_or(Val::Undef);
        let name = look(named("DIAL_TYPE"), &ty).unwrap_or("");
        st.current_dial = Some(format!("{name}:{}", d.field_id_str()));
    }
}

// ---------------------------------------------------------------- Text::Wrap

/// Perl's `\s` on a byte string.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// Text::Tabs `expand` (2021.0814): each tab padded by the length of the piece before it (not
/// by the column: tes3cmd's Perl does that), at tab stops of 8.
pub fn expand(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // split(/^/m): lines, each with its newline.
    let mut lines: Vec<&[u8]> = Vec::new();
    let mut start = 0;
    for (i, &c) in s.iter().enumerate() {
        if c == b'\n' {
            lines.push(&s[start..=i]);
            start = i + 1;
        }
    }
    if start < s.len() || s.is_empty() {
        lines.push(&s[start..]);
    }
    for line in lines {
        let mut offs: Option<usize> = None;
        for seg in line.split(|&c| c == b'\t') {
            if let Some(o) = offs {
                out.extend(std::iter::repeat_n(b' ', 8 - o % 8));
            }
            out.extend_from_slice(seg);
            offs = Some(seg.len());
        }
    }
    out
}

/// Text::Tabs `unexpand`: each line expanded, then within each 8-character piece but the last,
/// two or more trailing spaces become a tab (a last piece of 8 spaces too).
pub fn unexpand(s: &[u8]) -> Vec<u8> {
    let lines: Vec<Vec<u8>> = s
        .split(|&c| c == b'\n')
        .map(|line| {
            let line = expand(line);
            let mut pieces: Vec<Vec<u8>> = line.chunks(8).map(<[u8]>::to_vec).collect();
            // The last piece is what follows the last whole 8: empty when the line is a multiple of 8.
            let last = if line.len() % 8 == 0 {
                Vec::new()
            } else {
                pieces.pop().unwrap_or_default()
            };
            let last = if last == b"        " {
                b"\t".to_vec()
            } else {
                last
            };
            let mut out = Vec::new();
            for mut p in pieces {
                let n = p.iter().rev().take_while(|&&c| c == b' ').count();
                if n >= 2 {
                    p.truncate(p.len() - n);
                    p.push(b'\t');
                }
                out.extend(p);
            }
            out.extend(last);
            out
        })
        .collect();
    lines.join(&b"\n"[..])
}

/// Text::Wrap `wrap($ip, $xp, $text)` (2021.0814, 76 columns, `$huge = 'wrap'`, unexpanded).
pub fn wrap(ip: &[u8], xp: &[u8], text: &[u8]) -> Vec<u8> {
    let columns: usize = 76;
    let t = expand(text);
    let mut nll = columns as isize - expand(xp).len() as isize - 1;
    if nll <= 0 && !xp.is_empty() {
        nll = 1;
    }
    let mut ll = (columns as isize - expand(ip).len() as isize - 1).max(0) as usize;
    let mut lead: &[u8] = ip;
    let mut nl: &[u8] = b"";
    let mut r = Vec::new();
    let mut remainder: Vec<u8> = Vec::new();
    let mut pos = 0;
    while !t[pos..].iter().all(|&c| is_space(c)) {
        let run = t[pos..].iter().take_while(|&&c| c != b'\n').count();
        let kmax = run.min(ll);
        let found = (0..=kmax)
            .rev()
            .find(|&k| pos + k == t.len() || is_space(t[pos + k]));
        let line: Vec<u8> = match found {
            Some(k) => {
                let body = t[pos..pos + k].to_vec();
                pos += k;
                remainder = if pos == t.len() {
                    Vec::new()
                } else if t[pos..].starts_with(b"\r\n") {
                    pos += 2;
                    b"\r\n".to_vec()
                } else {
                    pos += 1;
                    vec![t[pos - 1]]
                };
                body
            }
            None => {
                let body = t[pos..pos + ll].to_vec();
                pos += ll;
                remainder = b"\n".to_vec();
                body
            }
        };
        r.extend(unexpand(&[nl, lead, &line].concat()));
        lead = xp;
        ll = nll as usize;
        nl = b"\n";
    }
    r.extend(remainder);
    r
}

// ---------------------------------------------------------------- getfield

/// A subrecord's fields by name, as tes3cmd's decoders leave them in its hash (a field kept as
/// an array, as a path grid's points, is left out).
pub fn sub_fields(rectype: &[u8; 4], sub: &Sub) -> Vec<(String, Val)> {
    let names =
        |n: &[&str], v: Vec<Val>| n.iter().map(|s| s.to_string()).zip(v).collect::<Vec<_>>();
    let raw = &sub.raw;
    match &sub.fields {
        Fields::Generic { slots, map, .. } => match row(rectype, &sub.tag) {
            Some(r) => {
                r.4.iter()
                    .zip(map.iter())
                    .map(|(n, &s)| {
                        (
                            n.to_string(),
                            slots.get(s as usize).cloned().unwrap_or(Val::Undef),
                        )
                    })
                    .collect()
            }
            None => Vec::new(),
        },
        Fields::Interior {
            flags,
            unknown,
            fog,
        } => names(
            &["flags", "unknown", "fog_density"],
            vec![
                Val::Int(*flags as i64),
                Val::Int(*unknown as i64),
                Val::Num(*fog as f64),
            ],
        ),
        Fields::Exterior { flags, x, y } => names(
            &["flags", "x", "y"],
            vec![
                Val::Int(*flags as i64),
                Val::Int(*x as i64),
                Val::Int(*y as i64),
            ],
        ),
        Fields::Position(v) => names(&["x", "y", "z", "x_angle", "y_angle", "z_angle"], v.clone()),
        Fields::Reference { idx } => match idx {
            Some(i) => names(
                &["objidx", "mastidx"],
                vec![Val::Int((i & 0xff_ffff) as i64), Val::Int((i >> 24) as i64)],
            ),
            None => names(&["objidx", "mastidx"], vec![Val::Undef, Val::Undef]),
        },
        Fields::OwnerFaction(n) => names(&["owner_faction"], vec![Val::Bytes(n.clone())]),
        Fields::CnamLost => names(
            &["unknown_#"],
            vec![Val::Bytes(
                raw.iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
                    .into_bytes(),
            )],
        ),
        Fields::HealthLeft(v) => names(
            &["health_left"],
            vec![v.map(|v| Val::Int(v as i64)).unwrap_or(Val::Undef)],
        ),
        Fields::TimeLeft(v) => names(&["time_left"], vec![Val::Num(*v as f64)]),
        Fields::WaterHeight(v) => names(
            &["water_height"],
            vec![v.map(|v| Val::Int(v as i64)).unwrap_or(Val::Undef)],
        ),
        Fields::MasterSize(n) => names(&["length"], vec![Val::Num(*n)]),
        Fields::DataBarfed | Fields::Array { .. } => Vec::new(),
        Fields::Raw => match (&rectype[..], &sub.tag[..]) {
            (b"DIAL", b"DATA") => names(
                &["type"],
                vec![
                    raw.first()
                        .map(|&b| Val::Int(b as i64))
                        .unwrap_or(Val::Undef),
                ],
            ),
            (b"REGN", b"WEAT") => {
                let n = [
                    "clear", "cloudy", "foggy", "overcast", "rain", "thunder", "ash", "blight",
                    "snow", "blizzard",
                ];
                match raw.len() {
                    10 | 8 => names(
                        &n[..raw.len()],
                        raw.iter().map(|&b| Val::Int(b as i64)).collect(),
                    ),
                    _ => Vec::new(),
                }
            }
            (b"LAND", b"VHGT") => {
                let mut v = unpack("fc4225sc", raw);
                if v.is_empty() {
                    return Vec::new();
                }
                let offset = v.remove(0);
                let u2 = v.pop().unwrap_or(Val::Undef);
                let u1 = if v.is_empty() {
                    Val::Undef
                } else {
                    v.remove(0)
                };
                names(&["offset", "unknown_1", "unknown_2"], vec![offset, u1, u2])
            }
            (b"NPC_", b"NPDT") => match raw.len() {
                12 => names(
                    &[
                        "level",
                        "disposition",
                        "factionindex",
                        "rank",
                        "unk1",
                        "unk2",
                        "unk3",
                        "gold",
                    ],
                    unpack("SC6L", raw),
                ),
                52 => {
                    let mut out = Vec::new();
                    for (i, &lv) in raw[10..37].iter().enumerate() {
                        if let Some((_, n)) =
                            named("SKILL").iter().find(|(k, _)| *k == i.to_string())
                        {
                            out.push((n.to_ascii_lowercase(), Val::Int(lv as i64)));
                        }
                    }
                    let n = [
                        "level",
                        "str",
                        "int",
                        "wil",
                        "agi",
                        "spd",
                        "end",
                        "per",
                        "lck",
                        "reputation",
                        "health",
                        "magicka",
                        "fatigue",
                        "disposition",
                        "factionindex",
                        "rank",
                        "gold",
                    ];
                    for (k, v) in names(&n, unpack("SC8x27CSSSCCCxL", raw)) {
                        match out.iter_mut().find(|(a, _)| *a == k) {
                            Some(e) => e.1 = v,
                            None => out.push((k, v)),
                        }
                    }
                    out
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        },
    }
}

/// `tr->getfield($field)`: a value by field name alone (`None`: undefined).
pub fn getfield(ctx: &mut Ctx, st: &mut State, d: &Decoded, field: &str) -> Option<Vec<u8>> {
    let rt = String::from_utf8_lossy(&d.rectype).into_owned();
    let field = field.to_lowercase();
    let first_of = |tag: &[u8]| d.subs.iter().find(|s| &s.tag[..] == tag);
    let value_in = |tag: &[u8]| -> Option<Vec<u8>> {
        let s = first_of(tag)?;
        sub_fields(&d.rectype, s)
            .into_iter()
            .find(|(n, _)| *n == field)
            .filter(|(_, v)| v.is_defined())
            .map(|(_, v)| v.bytes())
    };
    // `/^([^:])+:([^:]+)$/` keeps only the subtype's last letter, so a "TYPE:field" finds nothing.
    if let Some((sub, name)) = field.split_once(':')
        && !sub.is_empty()
        && !sub.contains(':')
        && !name.is_empty()
        && !name.contains(':')
    {
        let last = sub.chars().last().unwrap().to_ascii_uppercase().to_string();
        let s = first_of(last.as_bytes())?;
        return sub_fields(&d.rectype, s)
            .into_iter()
            .find(|(n, _)| n == name)
            .filter(|(_, v)| v.is_defined())
            .map(|(_, v)| v.bytes());
    }
    if let Some(tag) = st.format_subtype.get(&(rt.clone(), field.clone())).copied() {
        return value_in(&tag);
    }
    let mut seen: Vec<[u8; 4]> = Vec::new();
    for s in &d.subs {
        if !seen.contains(&s.tag) {
            seen.push(s.tag);
        }
    }
    let have: Vec<[u8; 4]> = seen.into_iter().filter(|t| value_in(t).is_some()).collect();
    if have.len() == 1 {
        let v = value_in(&have[0]);
        st.format_subtype.insert((rt, field.clone()), have[0]);
        return v;
    }
    let mut once = |ctx: &mut Ctx, m: String| {
        if !st.msgonce_said {
            st.msgonce_said = true;
            ctx.msg(&m);
        }
    };
    if !have.is_empty() {
        let list: Vec<String> = have
            .iter()
            .map(|t| String::from_utf8_lossy(t).into_owned())
            .collect();
        once(
            ctx,
            format!(
                "{rt}: Specified field name: \"{field}\" is not unique. It occurs in: [{}]\nYou could try: \"{}:{field}\"",
                list.join(" "),
                list[0]
            ),
        );
    }
    once(ctx, format!("{rt}: Unknown field name: \"{field}\""));
    Some(Vec::new())
}
