//! `update_header` as `fix_output` calls it (quiet, no backup, the record count only): the TES3
//! record decoded, HEDR's count set to the records that follow it, and the record re-encoded and
//! written over itself, where it is the same length as before.

use crate::ctx::{Ctx, Res};
use crate::decode::{Decoded, Fields};
use crate::pack::Val;
use crate::record::{Reader, read_one};

/// `get_record_count`: the records after the header, counted until one cannot be read (reported).
pub fn record_count(ctx: &mut Ctx, name: &str, data: &[u8]) -> usize {
    let mut pos = 0;
    let mut count = 0;
    let mut first = true;
    loop {
        match read_one(
            ctx,
            name,
            data,
            &mut pos,
            first.then_some("TES3"),
            Reader::Rec,
        ) {
            Ok(Some(_)) => {
                if !first {
                    count += 1;
                }
                first = false;
            }
            Ok(None) => break,
            Err(e) => {
                ctx.error(&e.0);
                break;
            }
        }
    }
    count
}

/// The plugin's header with its record count brought up to date; the file's bytes as they are
/// to be written. An `Err` is tes3cmd's abort (a header that does not encode to its own size).
pub fn update_record_count(ctx: &mut Ctx, plugin: &str, data: &mut [u8]) -> Res<()> {
    if is_backup_name(plugin, false) {
        ctx.print(&format!("tes3cmd header skipping Backup: {plugin}\n"));
        return Ok(());
    }
    let mut pos = 0;
    let Some(rec) = read_one(ctx, plugin, data, &mut pos, Some("TES3"), Reader::Input)? else {
        ctx.error(&format!("Error reading TES3 header from {plugin}"));
        return Ok(());
    };
    let tes3len = rec.buf.len();
    let mut d = Decoded::new(ctx, rec.rectype, rec.hdrflags, &rec.buf);
    let count = record_count(ctx, plugin, data);
    let mut modified = false;
    for s in &mut d.subs {
        if &s.tag == b"HEDR"
            && let Fields::Generic { slots, .. } = &mut s.fields
        {
            let n = slots.get(4).map(Val::num).unwrap_or(0.0);
            if n != count as f64 {
                modified = true;
                if let Some(v) = slots.get_mut(4) {
                    *v = Val::Int(count as i64);
                }
            }
        }
    }
    if modified {
        let buf = d.encode(ctx);
        if buf.len() != tes3len {
            return Err(ctx.abort(&format!(
                "update_header(): output header size incorrect (expected:{tes3len}, got: {})",
                buf.len()
            )));
        }
        data[16..16 + tes3len].copy_from_slice(&buf);
    }
    Ok(())
}

/// A backup's name (`~1.esp`): `.ess` too where `ess` is set.
pub fn is_backup_name(name: &str, ess: bool) -> bool {
    let l = name.to_ascii_lowercase();
    let exts: &[&str] = if ess {
        &[".esm", ".esp", ".ess"]
    } else {
        &[".esm", ".esp"]
    };
    for e in exts {
        if let Some(stem) = l.strip_suffix(e)
            && let Some(t) = stem.rfind('~')
            && t + 1 < stem.len()
            && stem[t + 1..].bytes().all(|b| b.is_ascii_digit())
        {
            return true;
        }
    }
    false
}
