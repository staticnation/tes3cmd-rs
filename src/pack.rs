//! The part of Perl's `pack` and `unpack` tes3cmd's codecs use (`perlfunc` pack): `a`, `Z`, `H`,
//! `L`, `l`, `s`, `C`, `f` and `x`, each with a count, `*` or none. A subrecord is decoded with its
//! unpack template and written back with its pack template, so what tes3cmd changes in a field it
//! re-encodes (a `Z*` string gains its NUL, anything after the first NUL goes) is changed here too.

/// A Perl scalar as the codecs hold it.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Bytes(Vec<u8>),
    Int(i64),
    Num(f64),
    Undef,
}

impl Val {
    /// The number Perl reads from it: a string's leading number, `undef` and "" as 0.
    pub fn num(&self) -> f64 {
        match self {
            Val::Int(i) => *i as f64,
            Val::Num(n) => *n,
            Val::Undef => 0.0,
            Val::Bytes(b) => leading_number(b),
        }
    }
    /// The bytes Perl reads from it: a number in Perl's own way of printing it.
    pub fn bytes(&self) -> Vec<u8> {
        match self {
            Val::Bytes(b) => b.clone(),
            Val::Int(i) => i.to_string().into_bytes(),
            Val::Num(n) => perl_num_str(*n).into_bytes(),
            Val::Undef => Vec::new(),
        }
    }
    pub fn is_defined(&self) -> bool {
        !matches!(self, Val::Undef)
    }
}

/// How Perl prints a number (`%.15g`, and integers whole).
pub fn perl_num_str(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Inf".into() } else { "-Inf".into() };
    }
    if n == n.trunc() && n.abs() < 1e15 {
        return format!("{}", n as i64);
    }
    // %.15g
    let s = format!("{:.*e}", 14, n);
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if (-5..15).contains(&exp) {
        let digits = (14 - exp).max(0) as usize;
        let mut f = format!("{:.*}", digits, n);
        if f.contains('.') {
            while f.ends_with('0') {
                f.pop();
            }
            if f.ends_with('.') {
                f.pop();
            }
        }
        f
    } else {
        let mut m = mant.to_string();
        if m.contains('.') {
            while m.ends_with('0') {
                m.pop();
            }
            if m.ends_with('.') {
                m.pop();
            }
        }
        format!("{m}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
    }
}

fn leading_number(b: &[u8]) -> f64 {
    let s = String::from_utf8_lossy(b);
    let t = s.trim_start();
    let mut end = 0;
    let bytes = t.as_bytes();
    let mut seen_digit = false;
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        seen_digit = true;
        end = i;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            seen_digit = true;
            end = i;
        }
    }
    if seen_digit && i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > start {
            end = j;
        }
    }
    if !seen_digit {
        return 0.0;
    }
    t[..end].parse().unwrap_or(0.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Count {
    One,
    N(usize),
    Star,
}

#[derive(Clone, Copy, Debug)]
struct Item {
    letter: u8,
    count: Count,
    /// `x[L]`: the size of the bracketed type.
    bracket: Option<usize>,
}

fn parse(template: &str) -> Vec<Item> {
    let b = template.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let letter = b[i];
        i += 1;
        if letter.is_ascii_whitespace() {
            continue;
        }
        let mut count = Count::One;
        let mut bracket = None;
        if i < b.len() && b[i] == b'*' {
            count = Count::Star;
            i += 1;
        } else if i < b.len() && b[i] == b'[' {
            let close = template[i..].find(']').map(|k| i + k).unwrap_or(b.len());
            let inner = &template[i + 1..close];
            bracket = Some(match inner {
                "L" | "l" | "f" => 4,
                "s" | "S" => 2,
                "C" | "c" => 1,
                n => n.parse().unwrap_or(1),
            });
            i = close + 1;
        } else {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i > start {
                count = Count::N(template[start..i].parse().unwrap());
            }
        }
        out.push(Item {
            letter,
            count,
            bracket,
        });
    }
    out
}

fn size_of(letter: u8) -> usize {
    match letter {
        b'L' | b'l' | b'f' => 4,
        b's' | b'S' => 2,
        b'C' | b'c' => 1,
        _ => 0,
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Perl's `unpack`: the values the template reads from `data`, as many as there is data for.
pub fn unpack(template: &str, data: &[u8]) -> Vec<Val> {
    let mut out = Vec::new();
    let mut p = 0usize;
    for it in parse(template) {
        let rest = &data[p.min(data.len())..];
        match it.letter {
            b'a' | b'Z' | b'A' => {
                let take = match it.count {
                    Count::Star => rest.len(),
                    Count::One => 1.min(rest.len()),
                    Count::N(n) => n.min(rest.len()),
                };
                let mut s = rest[..take].to_vec();
                if it.letter == b'Z' {
                    if let Some(z) = s.iter().position(|&c| c == 0) {
                        s.truncate(z);
                        if it.count == Count::Star {
                            p += z + 1;
                            out.push(Val::Bytes(s));
                            continue;
                        }
                    }
                }
                p += take;
                out.push(Val::Bytes(s));
            }
            b'H' => {
                let digits = match it.count {
                    Count::Star => rest.len() * 2,
                    Count::One => 1.min(rest.len() * 2),
                    Count::N(n) => n.min(rest.len() * 2),
                };
                let mut s = Vec::with_capacity(digits);
                for k in 0..digits {
                    let byte = rest[k / 2];
                    s.push(
                        HEX[if k % 2 == 0 {
                            (byte >> 4) as usize
                        } else {
                            (byte & 15) as usize
                        }],
                    );
                }
                p += digits.div_ceil(2);
                out.push(Val::Bytes(s));
            }
            b'x' => {
                let n = it.bracket.unwrap_or(match it.count {
                    Count::N(n) => n,
                    _ => 1,
                });
                p += n;
            }
            l @ (b'L' | b'l' | b's' | b'S' | b'C' | b'c' | b'f') => {
                let size = size_of(l);
                let mut times = match it.count {
                    Count::Star => usize::MAX,
                    Count::One => 1,
                    Count::N(n) => n,
                };
                while times > 0 {
                    let rest = &data[p.min(data.len())..];
                    if rest.len() < size {
                        break;
                    }
                    let v = &rest[..size];
                    out.push(match l {
                        b'L' => Val::Int(u32::from_le_bytes(v.try_into().unwrap()) as i64),
                        b'l' => Val::Int(i32::from_le_bytes(v.try_into().unwrap()) as i64),
                        b's' => Val::Int(i16::from_le_bytes(v.try_into().unwrap()) as i64),
                        b'S' => Val::Int(u16::from_le_bytes(v.try_into().unwrap()) as i64),
                        b'C' => Val::Int(v[0] as i64),
                        b'c' => Val::Int(v[0] as i8 as i64),
                        _ => Val::Num(f32::from_le_bytes(v.try_into().unwrap()) as f64),
                    });
                    p += size;
                    times -= 1;
                }
            }
            _ => {}
        }
    }
    out
}

/// Perl's `pack`: the template filled from `vals` in order, a missing value as `undef`.
pub fn pack(template: &str, vals: &[Val]) -> Vec<u8> {
    pack_counting(template, vals).0
}

/// [`pack`], and how many `undef` values it packed (Perl warns once for each:
/// "Use of uninitialized value in pack").
pub fn pack_counting(template: &str, vals: &[Val]) -> (Vec<u8>, usize) {
    let undefs = std::cell::Cell::new(0usize);
    let mut out = Vec::new();
    let mut vi = 0usize;
    let next = |vi: &mut usize| -> Val {
        let v = vals.get(*vi).cloned().unwrap_or(Val::Undef);
        *vi += 1;
        if v == Val::Undef {
            undefs.set(undefs.get() + 1);
        }
        v
    };
    for it in parse(template) {
        match it.letter {
            b'a' | b'Z' | b'A' => {
                let s = next(&mut vi).bytes();
                match it.count {
                    Count::Star => {
                        out.extend_from_slice(&s);
                        if it.letter == b'Z' {
                            out.push(0);
                        }
                    }
                    c => {
                        let n = if let Count::N(n) = c { n } else { 1 };
                        let keep = if it.letter == b'Z' {
                            n.saturating_sub(1).min(s.len())
                        } else {
                            n.min(s.len())
                        };
                        out.extend_from_slice(&s[..keep]);
                        out.extend(std::iter::repeat_n(
                            if it.letter == b'A' { b' ' } else { 0 },
                            n - keep,
                        ));
                    }
                }
            }
            b'H' => {
                let s = next(&mut vi).bytes();
                let digits = match it.count {
                    Count::Star => s.len(),
                    Count::One => 1,
                    Count::N(n) => n,
                };
                let nib = |c: u8| -> u8 {
                    // Perl: a letter's low bits plus 9, a digit's low bits.
                    if c.is_ascii_alphabetic() {
                        ((c & 0x0f) + 9) & 0x0f
                    } else {
                        c & 0x0f
                    }
                };
                let mut k = 0;
                while k < digits {
                    let hi = s.get(k).map(|&c| nib(c)).unwrap_or(0);
                    let lo = if k + 1 < digits {
                        s.get(k + 1).map(|&c| nib(c)).unwrap_or(0)
                    } else {
                        0
                    };
                    out.push((hi << 4) | lo);
                    k += 2;
                }
            }
            b'x' => {
                let n = it.bracket.unwrap_or(match it.count {
                    Count::N(n) => n,
                    _ => 1,
                });
                out.extend(std::iter::repeat_n(0, n));
            }
            l @ (b'L' | b'l' | b's' | b'S' | b'C' | b'c' | b'f') => {
                let times = match it.count {
                    Count::Star => vals.len().saturating_sub(vi),
                    Count::One => 1,
                    Count::N(n) => n,
                };
                for _ in 0..times {
                    let v = next(&mut vi);
                    match l {
                        b'f' => out.extend_from_slice(&(v.num() as f32).to_le_bytes()),
                        _ => {
                            let n = int_of(&v);
                            match l {
                                b'L' | b'l' => out.extend_from_slice(&(n as u32).to_le_bytes()),
                                b's' | b'S' => out.extend_from_slice(&(n as u16).to_le_bytes()),
                                _ => out.push(n as u8),
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    (out, undefs.get())
}

/// The integer Perl packs from a value: its number truncated toward zero, wrapped to the width.
fn int_of(v: &Val) -> i64 {
    match v {
        Val::Int(i) => *i,
        other => {
            let n = other.num();
            if n.is_nan() { 0 } else { n.trunc() as i64 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_as_perl_reads_and_writes_them() {
        assert_eq!(unpack("Z*", b"abc\0def"), vec![Val::Bytes(b"abc".to_vec())]);
        assert_eq!(pack("Z*", &[Val::Bytes(b"abc".to_vec())]), b"abc\0");
        assert_eq!(unpack("Z32", b"ab\0junk"), vec![Val::Bytes(b"ab".to_vec())]);
        assert_eq!(pack("a4", &[Val::Bytes(b"ab".to_vec())]), b"ab\0\0");
        assert_eq!(pack("Z4", &[Val::Bytes(b"abcdef".to_vec())]), b"abc\0");
        assert_eq!(
            unpack("H*", &[0xab, 0x01]),
            vec![Val::Bytes(b"ab01".to_vec())]
        );
        assert_eq!(
            pack("H*", &[Val::Bytes(b"ab01".to_vec())]),
            vec![0xab, 0x01]
        );
        assert_eq!(pack("H*", &[Val::Undef]), Vec::<u8>::new());
    }

    #[test]
    fn numbers_and_short_data() {
        assert_eq!(
            unpack("Lll", &[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]),
            vec![Val::Int(1), Val::Int(-1)]
        );
        assert_eq!(pack("l", &[Val::Undef]), vec![0, 0, 0, 0]);
        assert_eq!(
            unpack("x[L]ll", &[9, 9, 9, 9, 2, 0, 0, 0, 3, 0, 0, 0]),
            vec![Val::Int(2), Val::Int(3)]
        );
        assert_eq!(perl_num_str(0.5), "0.5");
        assert_eq!(perl_num_str(-3.0), "-3");
        assert_eq!(perl_num_str(1.0 / 3.0), "0.333333333333333");
    }
}
