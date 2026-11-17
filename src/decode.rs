//! A record decoded as tes3cmd decodes it (`TES3::Record::decode`): its subrecords in order, each
//! with the bytes it was read with and the fields its codec made of them, and the record's id.
//! Writing a decoded record back re-encodes every subrecord from its fields, as `encode` does;
//! a record nothing changed is written from its bytes instead (see `record`).

use crate::ctx::Ctx;
use crate::pack::{Val, pack_counting, perl_num_str, unpack};
use crate::record::{put_sub, subrecords};
use crate::schema_table::{ID_FIELD, SCHEMA};

/// What a subrecord's codec made of its bytes.
#[derive(Clone, Debug, PartialEq)]
pub enum Fields {
    /// A codec this port does not need to write with yet (a custom one outside CELL and TES3):
    /// written back as read.
    Raw,
    /// A template codec: each field's value, by slot (see `schema_table`).
    Generic {
        slots: Vec<Val>,
        map: &'static [u8],
        enc: &'static str,
    },
    /// CELL.FRMR and CELL.MVRF (`@RD_reference`): the object index and the master's.
    Reference {
        idx: Option<u32>,
    },
    /// CELL.CNAM in an object's group: its owner faction.
    OwnerFaction(Vec<u8>),
    /// CELL.CNAM anywhere else: kept under a name the encoder does not read, so it is written
    /// back empty (tes3cmd decodes it as `unknown_#` and encodes `unknown_1`).
    CnamLost,
    /// CELL.DATA, 24 bytes: a reference's position.
    Position(Vec<Val>),
    /// CELL.DATA, 12 bytes, of an interior.
    Interior {
        flags: u32,
        unknown: u32,
        fog: f32,
    },
    /// CELL.DATA, 12 bytes, of an exterior.
    Exterior {
        flags: u32,
        x: i32,
        y: i32,
    },
    /// CELL.DATA of another length: nothing decoded, written back as six zero floats.
    DataBarfed,
    /// CELL.INTV in an object's group: health left (`l`), or time left (`f`) where the long is
    /// out of 0..=65536.
    HealthLeft(Option<i32>),
    TimeLeft(f32),
    /// CELL.INTV in the cell's header: the old water height.
    WaterHeight(Option<i32>),
    /// The `.ess` arrays (`SLFD`, `SLLD`, `SLSD`, `CHRD`): every whole value.
    Array {
        letter: u8,
        vals: Vec<Val>,
    },
    /// TES3.DATA: the master's size, two longs.
    MasterSize(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sub {
    pub tag: [u8; 4],
    /// The bytes as read (`_subbuf_`), compared where tes3cmd compares `subbuf`.
    pub raw: Vec<u8>,
    pub fields: Fields,
}

impl Sub {
    pub fn tag_str(&self) -> String {
        String::from_utf8_lossy(&self.tag).into_owned()
    }
    /// `encode`: the bytes the fields pack to, and how many undefined values went in.
    pub fn encode(&self) -> (Vec<u8>, usize) {
        let pack = pack_counting;
        match &self.fields {
            Fields::Raw => (self.raw.clone(), 0),
            Fields::Generic { slots, map, enc } => {
                let vals: Vec<Val> = map
                    .iter()
                    .map(|&s| slots.get(s as usize).cloned().unwrap_or(Val::Undef))
                    .collect();
                pack(enc, &vals)
            }
            Fields::Reference { idx } => (idx.unwrap_or(0).to_le_bytes().to_vec(), 0),
            Fields::OwnerFaction(s) => pack("Z*", &[Val::Bytes(s.clone())]),
            Fields::CnamLost => (Vec::new(), 1),
            Fields::Position(v) => pack("f6", v),
            Fields::DataBarfed => pack("f6", &[]),
            Fields::Interior {
                flags,
                unknown,
                fog,
            } => pack(
                "LLf",
                &[
                    Val::Int(*flags as i64),
                    Val::Int(*unknown as i64),
                    Val::Num(*fog as f64),
                ],
            ),
            Fields::Exterior { flags, x, y } => pack(
                "Lll",
                &[
                    Val::Int(*flags as i64),
                    Val::Int(*x as i64),
                    Val::Int(*y as i64),
                ],
            ),
            Fields::HealthLeft(v) | Fields::WaterHeight(v) => {
                pack("l", &[v.map_or(Val::Undef, |v| Val::Int(v as i64))])
            }
            Fields::TimeLeft(v) => pack("f", &[Val::Num(*v as f64)]),
            Fields::Array { letter, vals } => pack(&format!("{}*", *letter as char), vals),
            Fields::MasterSize(len) => {
                let l1 = (*len as i128) & 0xffff_ffff;
                let ld = *len - 4_294_967_295.0;
                let l2 = if ld > 0.0 { ld } else { 0.0 };
                pack("L2", &[Val::Int(l1 as i64), Val::Num(l2)])
            }
        }
    }
    /// The value of a field by name, where the port needs one (`get(SUBTYPE, field)`).
    fn slot(&self, n: usize) -> Val {
        match &self.fields {
            Fields::Generic { slots, .. } => slots.get(n).cloned().unwrap_or(Val::Undef),
            _ => Val::Undef,
        }
    }
}

/// The subtypes that start a group, by record type (`%TYPE_INFO` ... `group start`).
const GROUP_STARTS: &[(&str, &str)] = &[
    ("ALCH", "ENAM"),
    ("ARMO", "INDX"),
    ("BSGN", "NPCS"),
    ("CLOT", "INDX"),
    ("CONT", "NPCO"),
    ("CELL", "FRMR"),
    ("CREA", "AI_T"),
    ("CREA", "AI_W"),
    ("CREA", "DODT"),
    ("CREA", "NPCO"),
    ("CREA", "NPCS"),
    ("CREC", "NPCO"),
    ("ENCH", "ENAM"),
    ("FACT", "ANAM"),
    ("FACT", "RNAM"),
    ("INFO", "SCVR"),
    ("KLST", "KNAM"),
    ("LEVC", "CNAM"),
    ("LEVI", "INAM"),
    ("NPC_", "AI_T"),
    ("NPC_", "AI_W"),
    ("NPC_", "DODT"),
    ("NPC_", "NPCO"),
    ("NPC_", "NPCS"),
    ("NPCC", "NPCO"),
    ("RACE", "NPCS"),
    ("REGN", "SNAM"),
    ("SPEL", "ENAM"),
    ("SPLM", "NAME"),
    ("TES3", "MAST"),
];

pub fn starts_group(rectype: &[u8; 4], tag: &[u8; 4]) -> bool {
    GROUP_STARTS
        .iter()
        .any(|(r, s)| r.as_bytes() == rectype && s.as_bytes() == tag)
}

fn schema(
    rectype: &[u8; 4],
    tag: &[u8],
) -> Option<&'static (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static [u8],
)> {
    SCHEMA
        .iter()
        .find(|(r, s, ..)| r.as_bytes() == rectype && s.as_bytes() == tag)
}

/// Whether tes3cmd defines this subtype for this record type (its malformed-record message).
pub fn known_sub(rectype: &[u8; 4], tag: &[u8]) -> bool {
    schema(rectype, tag).is_some()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    pub rectype: [u8; 4],
    pub hdrflags: u32,
    pub subs: Vec<Sub>,
    /// `_is_interior_`, where a 12-byte CELL.DATA set it.
    interior: Option<bool>,
    /// `_id_` as `decode` leaves it (the last `id` field, lower case).
    field_id: Option<String>,
}

/// Perl's `lc` on bytes: ASCII letters only.
pub fn lc(b: &[u8]) -> String {
    String::from_utf8_lossy(&b.to_ascii_lowercase()).into_owned()
}

/// A value as Perl interpolates it into a string.
fn interp(v: &Val) -> String {
    match v {
        Val::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        Val::Int(i) => i.to_string(),
        Val::Num(n) => perl_num_str(*n),
        Val::Undef => String::new(),
    }
}

/// Perl's truth of a string value: "" and "0" are false.
fn truthy(v: &Val) -> bool {
    match v {
        Val::Undef => false,
        Val::Bytes(b) => !(b.is_empty() || b == b"0"),
        Val::Int(i) => *i != 0,
        Val::Num(n) => *n != 0.0,
    }
}

impl Decoded {
    /// `new_from_recbuf(...)->decode()`: reports, as tes3cmd does, what it cannot read.
    pub fn new(ctx: &mut Ctx, rectype: [u8; 4], hdrflags: u32, buf: &[u8]) -> Decoded {
        let parts = subrecords(ctx, &rectype, buf, "tr->decode()", |t| {
            known_sub(&rectype, t)
        });
        let mut d = Decoded {
            rectype,
            hdrflags,
            subs: Vec::new(),
            interior: None,
            field_id: None,
        };
        let mut group: Option<[u8; 4]> = None;
        for (tag, raw) in parts {
            if starts_group(&rectype, &tag) {
                group = Some(tag);
            }
            let sub = d.decode_sub(ctx, tag, raw, group);
            d.subs.push(sub);
        }
        d
    }

    /// A record built from subrecords already decoded (`CELL->new([...])`): header flags 0.
    pub fn from_subs(rectype: [u8; 4], subs: Vec<Sub>) -> Decoded {
        let mut d = Decoded {
            rectype,
            hdrflags: 0,
            subs,
            interior: None,
            field_id: None,
        };
        d.field_id = d.subs.iter().rev().find_map(|s| id_of_sub(&rectype, s));
        d
    }

    fn decode_sub(
        &mut self,
        ctx: &mut Ctx,
        tag: [u8; 4],
        raw: Vec<u8>,
        group: Option<[u8; 4]>,
    ) -> Sub {
        let rt = &self.rectype;
        let fields = match (rt, &tag) {
            (b"CELL", b"FRMR") | (b"CELL", b"MVRF") | (b"REFR", b"FRMR") => {
                let idx =
                    (raw.len() >= 4).then(|| u32::from_le_bytes(raw[..4].try_into().unwrap()));
                if idx.is_none() {
                    ctx.msg("Use of uninitialized value $idx in bitwise and (&)");
                    ctx.msg("Use of uninitialized value $idx in right bitshift (>>)");
                }
                Fields::Reference { idx }
            }
            (b"CELL", b"CNAM") => {
                if group == Some(*b"FRMR") {
                    Fields::OwnerFaction(
                        unpack("Z*", &raw)
                            .into_iter()
                            .next()
                            .map(|v| v.bytes())
                            .unwrap_or_default(),
                    )
                } else {
                    // Perl compares the unset group twice (`eq 'FRMR'`, `eq 'MVRF'`), and says so.
                    if group.is_none() {
                        for _ in 0..2 {
                            ctx.msg("Use of uninitialized value $TES3::Record::CURRENT_GROUP in string eq");
                        }
                    }
                    Fields::CnamLost
                }
            }
            (b"CELL", b"DATA") => match raw.len() {
                24 => Fields::Position(unpack("f6", &raw)),
                12 => {
                    let flags = u32::from_le_bytes(raw[0..4].try_into().unwrap());
                    if flags & 1 != 0 {
                        self.interior = Some(true);
                        Fields::Interior {
                            flags,
                            unknown: u32::from_le_bytes(raw[4..8].try_into().unwrap()),
                            fog: f32::from_le_bytes(raw[8..12].try_into().unwrap()),
                        }
                    } else {
                        self.interior = Some(false);
                        Fields::Exterior {
                            flags,
                            x: i32::from_le_bytes(raw[4..8].try_into().unwrap()),
                            y: i32::from_le_bytes(raw[8..12].try_into().unwrap()),
                        }
                    }
                }
                n => {
                    ctx.error(&format!(
                        "DECODER barfed on CELL.DATA, Invalid length: {n}, expected 24 or 12!"
                    ));
                    Fields::DataBarfed
                }
            },
            (b"CELL", b"INTV") => {
                let l = (raw.len() >= 4).then(|| i32::from_le_bytes(raw[..4].try_into().unwrap()));
                if group.is_some() {
                    match l {
                        Some(v) if !(0..=65536).contains(&v) => {
                            Fields::TimeLeft(f32::from_le_bytes(raw[..4].try_into().unwrap()))
                        }
                        _ => Fields::HealthLeft(l),
                    }
                } else {
                    Fields::WaterHeight(l)
                }
            }
            (b"TES3", b"DATA") => {
                let v = unpack("L2", &raw);
                let l1 = v.first().map(Val::num).unwrap_or(0.0);
                let l2 = v.get(1).map(Val::num).unwrap_or(0.0);
                Fields::MasterSize(l1 + l2 * 4_294_967_296.0)
            }
            _ => {
                let arr = |l: u8| Fields::Array {
                    letter: l,
                    vals: unpack(&format!("{}*", l as char), &raw),
                };
                match (rt, &tag) {
                    (_, b"SLFD") => arr(b'f'),
                    (_, b"SLLD") | (_, b"CHRD") => arr(b'l'),
                    (_, b"SLSD") => arr(b's'),
                    _ => match schema(rt, &tag) {
                        Some((_, _, dec, enc, map)) if !dec.is_empty() => {
                            let vals = unpack(dec, &raw);
                            let n = map.iter().map(|&s| s as usize + 1).max().unwrap_or(0);
                            let mut slots = vec![Val::Undef; n];
                            for (i, &s) in map.iter().enumerate() {
                                slots[s as usize] = vals.get(i).cloned().unwrap_or(Val::Undef);
                            }
                            Fields::Generic { slots, map, enc }
                        }
                        // The script's own codec, not needed to write yet.
                        Some(_) => Fields::Raw,
                        // A subtype tes3cmd does not know: it makes it `H*`, which writes back as read.
                        None => {
                            ctx.error(&format!(
                                "Encountered unknown subtype: {}::{}",
                                String::from_utf8_lossy(rt),
                                String::from_utf8_lossy(&tag)
                            ));
                            Fields::Raw
                        }
                    },
                }
            }
        };
        let sub = Sub { tag, raw, fields };
        if let Some(id) = id_of_sub(rt, &sub) {
            self.field_id = Some(id);
        }
        sub
    }

    pub fn get(&self, tag: &[u8; 4]) -> Option<&Sub> {
        self.subs.iter().find(|s| &s.tag == tag)
    }

    /// `is_interior`: as the 12-byte DATA said, else the first DATA's flags.
    pub fn is_interior(&self) -> bool {
        if let Some(i) = self.interior {
            return i;
        }
        match self.get(b"DATA").map(|s| &s.fields) {
            Some(Fields::Interior { flags, .. }) | Some(Fields::Exterior { flags, .. }) => {
                flags & 1 != 0
            }
            _ => false,
        }
    }

    /// `decode()->id`: the record's id as the plugin side computes it.
    pub fn id(&self) -> String {
        match &self.rectype {
            b"CELL" => {
                let name = self.get(b"NAME").map(|s| s.slot(0)).unwrap_or(Val::Undef);
                if self.is_interior() {
                    return lc(interp(&name).as_bytes());
                }
                let mut name = name;
                if !truthy(&name) {
                    name = self.get(b"RGNN").map(|s| s.slot(0)).unwrap_or(Val::Undef);
                }
                let name = if truthy(&name) {
                    interp(&name)
                } else {
                    "wilderness".to_string()
                };
                let (x, y) = match self.get(b"DATA").map(|s| &s.fields) {
                    Some(Fields::Exterior { x, y, .. }) => (x.to_string(), y.to_string()),
                    Some(Fields::Position(v)) => (
                        v.first().map(interp).unwrap_or_default(),
                        v.get(1).map(interp).unwrap_or_default(),
                    ),
                    _ => (String::new(), String::new()),
                };
                lc(format!("{name} ({x}, {y})").as_bytes())
            }
            b"LAND" => match self.get(b"INTV") {
                Some(s) if s.slot(0).is_defined() && s.slot(1).is_defined() => {
                    format!("({}, {})", interp(&s.slot(0)), interp(&s.slot(1)))
                }
                _ => String::new(),
            },
            b"PGRD" => {
                let name = interp(&self.get(b"NAME").map(|s| s.slot(0)).unwrap_or(Val::Undef));
                let (x, y) = self
                    .get(b"DATA")
                    .map(|s| (s.slot(0), s.slot(1)))
                    .unwrap_or((Val::Undef, Val::Undef));
                if x.num() == 0.0 && y.num() == 0.0 {
                    name
                } else {
                    format!("{name} ({}, {})", interp(&x), interp(&y))
                }
            }
            b"TES3" | b"FMAP" | b"GAME" | b"PCDT" | b"PROJ" | b"SPLM" | b"VFXM" => "()".into(),
            _ => self.field_id.clone().unwrap_or_default(),
        }
    }

    /// `encode` then the record's bytes: every subrecord re-encoded from its fields, Perl's
    /// warning for each undefined value it packs.
    pub fn encode(&self, ctx: &mut Ctx) -> Vec<u8> {
        let mut out = Vec::new();
        for s in &self.subs {
            let (bytes, undefs) = s.encode();
            for _ in 0..undefs {
                ctx.msg("Use of uninitialized value in pack");
            }
            put_sub(&mut out, &s.tag, &bytes);
        }
        out
    }

    /// `delete_subtype`: every subrecord of the type gone; whether there was one.
    pub fn delete_subtype(&mut self, tag: &[u8; 4]) -> bool {
        let before = self.subs.len();
        self.subs.retain(|s| &s.tag != tag);
        self.subs.len() != before
    }

    /// `split_groups`: the subrecords before the first group, then each group.
    pub fn split_groups(&self) -> Vec<Vec<Sub>> {
        let mut groups = Vec::new();
        let mut this = Vec::new();
        for s in &self.subs {
            if starts_group(&self.rectype, &s.tag) {
                groups.push(std::mem::take(&mut this));
            }
            this.push(s.clone());
        }
        groups.push(this);
        groups
    }
}

/// The lowercased value of a subrecord's `id` field, where it has one.
fn id_of_sub(rectype: &[u8; 4], sub: &Sub) -> Option<String> {
    let slot = if &sub.tag == b"NAME" && named_type(rectype) {
        0
    } else {
        ID_FIELD
            .iter()
            .find(|(r, s, _)| r.as_bytes() == rectype && s.as_bytes() == sub.tag)
            .map(|(_, _, k)| *k)?
    };
    let v = sub.slot(slot as usize);
    v.is_defined().then(|| lc(interp(&v).as_bytes()))
}

/// The types whose NAME is their id (`%NAMED_TYPE`).
pub fn named_type(rectype: &[u8; 4]) -> bool {
    const NAMED: &[&str] = &[
        "ACTI", "ALCH", "APPA", "ARMO", "BODY", "BOOK", "BSGN", "CLAS", "CLOT", "CNTC", "CONT",
        "CREA", "CREC", "DIAL", "DOOR", "ENCH", "FACT", "GLOB", "GMST", "INGR", "JOUR", "LEVC",
        "LEVI", "LIGH", "LOCK", "LTEX", "MISC", "NPCC", "NPC_", "PROB", "QUES", "RACE", "REFR",
        "REGN", "REPA", "SNDG", "SOUN", "SPEL", "STAT", "STLN", "WEAP",
    ];
    NAMED.iter().any(|n| n.as_bytes() == rectype)
}

/// `read_quick_parse`'s id, the one the masters are indexed by: every one lower case (a path
/// grid's too, which the plugin side does not lowercase), and an exterior's coordinates read
/// straight from DATA.
pub fn quick_id(ctx: &mut Ctx, rectype: &[u8; 4], buf: &[u8]) -> String {
    let parts = subrecords(ctx, rectype, buf, "read_quick_parse()", |t| {
        known_sub(rectype, t)
    });
    let first = |tag: &[u8; 4]| {
        parts
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, b)| b.as_slice())
    };
    let z = |b: Option<&[u8]>| {
        b.map(|b| unpack("Z*", b).into_iter().next().unwrap_or(Val::Undef))
            .unwrap_or(Val::Undef)
    };
    let id = if named_type(rectype) {
        interp(&z(first(b"NAME")))
    } else if rectype == b"INFO" {
        interp(&z(first(b"INAM")))
    } else if rectype == b"CELL" {
        let name = z(first(b"NAME"));
        let data = first(b"DATA").unwrap_or(&[]);
        let flags = unpack("L", data)
            .into_iter()
            .next()
            .map(|v| v.num() as u32)
            .unwrap_or(0);
        if flags & 1 != 0 {
            interp(&name)
        } else {
            let name = if truthy(&name) {
                interp(&name)
            } else if let Some(r) = first(b"RGNN") {
                interp(&z(Some(r)))
            } else {
                "wilderness".into()
            };
            let xy = unpack("x[L]ll", data);
            format!(
                "{name} ({}, {})",
                xy.first().map(interp).unwrap_or_default(),
                xy.get(1).map(interp).unwrap_or_default()
            )
        }
    } else if rectype == b"PGRD" {
        let name = interp(&z(first(b"NAME")));
        let xy = unpack("ll", first(b"DATA").unwrap_or(&[]));
        let (x, y) = (
            xy.first().cloned().unwrap_or(Val::Undef),
            xy.get(1).cloned().unwrap_or(Val::Undef),
        );
        if x.num() == 0.0 && y.num() == 0.0 {
            name
        } else {
            format!("{name} ({}, {})", interp(&x), interp(&y))
        }
    } else if rectype == b"SCPT" {
        interp(
            &unpack("Z32", first(b"SCHD").unwrap_or(&[]))
                .into_iter()
                .next()
                .unwrap_or(Val::Undef),
        )
    } else if let Some(ix) = first(b"INDX") {
        interp(&unpack("L", ix).into_iter().next().unwrap_or(Val::Undef))
    } else if rectype == b"LAND" {
        let xy = unpack("ll", first(b"INTV").unwrap_or(&[]));
        format!(
            "({}, {})",
            xy.first().map(interp).unwrap_or_default(),
            xy.get(1).map(interp).unwrap_or_default()
        )
    } else if matches!(
        rectype,
        b"TES3" | b"FMAP" | b"GAME" | b"PCDT" | b"SPLM" | b"VFXM"
    ) {
        "()".into()
    } else if rectype == b"SSCR" {
        interp(
            &unpack("Z32", first(b"DATA").unwrap_or(&[]))
                .into_iter()
                .next()
                .unwrap_or(Val::Undef),
        )
    } else {
        // tes3cmd aborts the master's read here; the caller does.
        return String::from("\u{0}no id");
    };
    lc(id.as_bytes())
}
