//! Records and subrecords as tes3cmd frames them: a 16-byte header (`a4LLL`: type, length, an
//! unused word, flags) and the record's bytes; inside, subrecords `a4L/a*`. A record is kept as
//! its bytes, and written back as they are unless a command changed it.

use crate::ctx::{Ctx, Res};

/// Every record type tes3cmd knows (`%TES3::Record::RECTYPES`, from its definitions).
pub const RECTYPES: &[&str] = &[
    "ACTI", "ALCH", "APPA", "ARMO", "BODY", "BOOK", "BSGN", "CELL", "CLAS", "CLOT", "CNTC", "CONT",
    "CREA", "CREC", "DIAL", "DOOR", "ENCH", "FACT", "FMAP", "GAME", "GLOB", "GMST", "INFO", "INGR",
    "JOUR", "KLST", "LAND", "LEVC", "LEVI", "LIGH", "LOCK", "LTEX", "MGEF", "MISC", "NPCC", "NPC_",
    "PCDT", "PGRD", "PROB", "PROJ", "QUES", "RACE", "REFR", "REGN", "REPA", "SCPT", "SKIL", "SNDG",
    "SOUN", "SPEL", "SPLM", "SSCR", "STAT", "STLN", "TES3", "VFXM", "WEAP",
];

pub fn known_type(t: &[u8; 4]) -> bool {
    RECTYPES.iter().any(|k| k.as_bytes() == t)
}

/// Header flags (`%HDR_FLAGS`).
pub const DELETED: u32 = 0x0020;
pub const PERSISTENT: u32 = 0x0400;
pub const IGNORED: u32 = 0x1000;
pub const BLOCKED: u32 = 0x2000;

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub rectype: [u8; 4],
    /// The header's third word (tes3cmd writes it back as 0).
    pub hdrflags: u32,
    pub buf: Vec<u8>,
}

impl Record {
    pub fn rectype_str(&self) -> String {
        String::from_utf8_lossy(&self.rectype).into_owned()
    }
    /// `write_rec`: the header with length, a zero word and the flags, then the bytes.
    pub fn write(&self, to: &mut Vec<u8>) {
        to.extend_from_slice(&self.rectype);
        to.extend_from_slice(&(self.buf.len() as u32).to_le_bytes());
        to.extend_from_slice(&0u32.to_le_bytes());
        to.extend_from_slice(&self.hdrflags.to_le_bytes());
        to.extend_from_slice(&self.buf);
    }
}

/// How a reader answers a type it does not know and a type it did not expect.
#[derive(Clone, Copy, PartialEq)]
pub enum Reader {
    /// `new_from_input`: an unknown type is reported and read on; an unexpected one noted.
    Input,
    /// `read_quick_parse` (masters): either stops the read.
    Quick,
    /// `read_rec` (the record count): either stops the read.
    Rec,
}

/// Reads one record at `*pos`, as the reader `how` does; `Ok(None)` at the end of the file.
/// `name` is the file as the reader's messages name it (`$plugin`; a master by its lower-case name).
pub fn read_one(
    ctx: &mut Ctx,
    name: &str,
    data: &[u8],
    pos: &mut usize,
    expected: Option<&str>,
    how: Reader,
) -> Res<Option<Record>> {
    if *pos >= data.len() {
        return Ok(None);
    }
    let start = *pos;
    let n = (data.len() - start).min(16);
    if n != 16 {
        let what = match how {
            Reader::Input => format!(
                "new_from_input(): Read Error on header (at byte: {start}): asked for 16 bytes, got {n}"
            ),
            Reader::Quick => format!(
                "read_quick_parse(): Read Error ({name} header at byte: {start}): asked for 16 bytes, got {n}"
            ),
            Reader::Rec => format!(
                "read_rec(): Read Error ({name} header at byte: {start}): asked for 16 bytes, got {n}"
            ),
        };
        *pos = data.len();
        return Err(ctx.abort(&what));
    }
    let h = &data[start..start + 16];
    let rectype: [u8; 4] = h[0..4].try_into().unwrap();
    let reclen = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
    let hdrflags = u32::from_le_bytes(h[12..16].try_into().unwrap());
    *pos = start + 16;
    let tname = String::from_utf8_lossy(&rectype).into_owned();
    if !known_type(&rectype) {
        match how {
            Reader::Input => ctx.error(&format!("new_from_input(): Type Error (at byte: {start}): Unknown Record Type: \"{tname}\"")),
            Reader::Quick => return Err(ctx.abort(&format!("new_from_input(): Error ({name} at byte: {start}): Invalid Record Type: \"{tname}\""))),
            Reader::Rec => return Err(ctx.abort(&format!("read_rec(): Error ({name} at byte: {start}): Invalid Record Type: \"{tname}\""))),
        }
    }
    if let Some(e) = expected
        && e.as_bytes() != rectype
    {
        match how {
            Reader::Input => ctx.msg(&format!("new_from_input(): Type Error (at byte: {start}): Expected: \"{e}\", got: \"{tname}\"")),
            Reader::Quick => return Err(ctx.abort(&format!("read_quick_parse(): Error ({name} at byte: {start}): Expected: \"{e}\", got: \"{tname}\""))),
            Reader::Rec => return Err(ctx.abort(&format!("read_rec(): Error ({name} at byte: {start}): Expected: \"{e}\", got: \"{tname}\""))),
        }
    }
    let avail = data.len() - *pos;
    let got = reclen.min(avail);
    let buf = data[*pos..*pos + got].to_vec();
    *pos += got;
    if got != reclen {
        let at = *pos - got;
        let what = match how {
            Reader::Input => format!(
                "new_from_input(): Read Error (at byte: {at}, rec_type=\"{tname}\"): asked for {reclen} bytes, got {got}"
            ),
            Reader::Quick => format!(
                "read_quick_parse(): Read Error ({name} record at byte: {at}, rec_type=\"{tname}\"): asked for {reclen} bytes, got {got}"
            ),
            Reader::Rec => format!(
                "read_rec(): Read Error ({name} record at byte: {at}, rec_type=\"{tname}\"): asked for {reclen} bytes, got {got}"
            ),
        };
        return Err(ctx.abort(&what));
    }
    Ok(Some(Record {
        rectype,
        hdrflags,
        buf,
    }))
}

/// A record's subrecords (`(a4L/a*)*`), and the complaints of tes3cmd's careful reader where the
/// framing breaks (a length past the end is read to the end; a missing length is not).
/// `who` names the caller in the complaint (`tr->decode()` or `read_quick_parse()`).
pub fn subrecords(
    ctx: &mut Ctx,
    rectype: &[u8; 4],
    buf: &[u8],
    who: &str,
    known_sub: impl Fn(&[u8]) -> bool,
) -> Vec<([u8; 4], Vec<u8>)> {
    // The quick way first: it fails only where a type or a length runs out.
    let mut out = Vec::new();
    let mut p = 0;
    let mut broken = false;
    while p < buf.len() {
        if buf.len() - p < 8 {
            broken = true;
            break;
        }
        let tag: [u8; 4] = buf[p..p + 4].try_into().unwrap();
        let len = u32::from_le_bytes(buf[p + 4..p + 8].try_into().unwrap()) as usize;
        p += 8;
        let take = len.min(buf.len() - p);
        out.push((tag, buf[p..p + take].to_vec()));
        p += take;
    }
    if !broken {
        return out;
    }
    // The careful way: what the quick way read, then the broken tail reported.
    let rt = String::from_utf8_lossy(rectype).into_owned();
    out.clear();
    let mut p = 0usize;
    while p < buf.len() {
        let rest = &buf[p..];
        let tag_bytes = &rest[..rest.len().min(4)];
        let sublen =
            (rest.len() >= 8).then(|| u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize);
        p += 8;
        let st = String::from_utf8_lossy(tag_bytes).into_owned();
        match sublen {
            Some(len) => {
                if p > buf.len() {
                    ctx.error(&format!("{who}: {rt}.{st} has malformed subbuf"));
                } else {
                    let take = len.min(buf.len() - p);
                    let mut tag = [0u8; 4];
                    tag.copy_from_slice(&rest[..4]);
                    out.push((tag, buf[p..p + take].to_vec()));
                }
                p += len;
            }
            None => {
                if known_sub(tag_bytes) {
                    ctx.error(&format!("{who}: {rt}.{st} has malformed recbuf"));
                } else {
                    ctx.error(&format!("{who}: {rt} has malformed subtype: \"{st}\""));
                }
            }
        }
    }
    out
}

/// A subrecord written into a record (`pack("a4L/a*", ...)`).
pub fn put_sub(to: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    to.extend_from_slice(tag);
    to.extend_from_slice(&(data.len() as u32).to_le_bytes());
    to.extend_from_slice(data);
}
