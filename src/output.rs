//! Where a modified plugin goes (`fix_output`, `unique_backup_name`): `Clean_<plugin>` beside it,
//! or over it with the original moved to a backup (`<name>~1.esp`, the next free number), in the
//! backup folder with `--hide-backups`. The output keeps the original's dates.

use std::path::PathBuf;

use crate::ctx::{Ctx, Res};
use crate::datadir::Install;

#[derive(Clone, Debug, Default)]
pub struct Output {
    /// `--output-dir`. tes3cmd's clean declares it without a value, so it is the flag `1`: the
    /// output goes into a folder named `1` (kept as tes3cmd has it).
    pub output_dir: Option<String>,
    pub hide_backups: bool,
    /// `--backup-dir`, else the install's.
    pub backup_dir: Option<String>,
}

/// `unique_backup_name`: the first of `name~1.esp`, `name~2.esp`... that is not taken.
pub fn unique_backup_name(opts: &Output, inst: &Install, plugbak: &str) -> Res<String> {
    let mut plugbak = plugbak.to_string();
    if opts.hide_backups {
        let dir = opts
            .backup_dir
            .clone()
            .unwrap_or_else(|| inst.backup_dir.clone());
        if !crate::fsx::at(&dir).is_dir() && crate::fsx::create_dir_all(&dir).is_err() {
            return Err(crate::ctx::Abort(format!(
                "FATAL ERROR (): Unable to make directory: \"{dir}\""
            )));
        }
        let (_, file) = crate::clean::split_dir(&plugbak);
        plugbak = format!("{dir}/{file}");
    }
    let lower = plugbak.to_ascii_lowercase();
    let ext_len = if lower.ends_with(".esm") || lower.ends_with(".esp") || lower.ends_with(".ess") {
        3
    } else {
        0
    };
    if ext_len == 0 {
        return Ok(plugbak);
    }
    while crate::fsx::at(&plugbak).is_file() {
        let ext = plugbak[plugbak.len() - 3..].to_string();
        let stem = &plugbak[..plugbak.len() - 4];
        let (base, n) = match stem.rfind('~') {
            Some(t) if t + 1 < stem.len() && stem[t + 1..].bytes().all(|b| b.is_ascii_digit()) => {
                (&stem[..t], stem[t + 1..].parse::<u64>().unwrap_or(0))
            }
            _ => (stem, 0),
        };
        plugbak = format!("{base}~{}.{ext}", n + 1);
    }
    Ok(plugbak)
}

fn abs(p: &str) -> String {
    let pb = PathBuf::from(p);
    let full = if pb.is_absolute() {
        pb
    } else {
        crate::fsx::base()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            .join(pb)
    };
    // File::Spec's canonpath: `.` parts and doubled separators go, `..` stays.
    let mut out = PathBuf::new();
    for c in full.components() {
        if c != std::path::Component::CurDir {
            out.push(c);
        }
    }
    out.to_string_lossy().into_owned()
}

fn times(p: &str) -> Option<std::fs::FileTimes> {
    let m = crate::fsx::metadata(p).ok()?;
    Some(
        std::fs::FileTimes::new()
            .set_accessed(m.accessed().ok()?)
            .set_modified(m.modified().ok()?),
    )
}

fn set_times(p: &str, t: &Option<std::fs::FileTimes>) {
    if let (Some(t), Ok(f)) = (
        t,
        std::fs::OpenOptions::new()
            .write(true)
            .open(crate::fsx::at(p)),
    ) {
        let _ = f.set_times(*t);
    }
}

/// tes3cmd's `move`: a rename, or a copy and delete where a rename cannot be done.
fn mv(from: &str, to: &str) -> std::io::Result<()> {
    if crate::fsx::rename(from, to).is_ok() {
        return Ok(());
    }
    crate::fsx::copy(from, to)?;
    crate::fsx::remove_file(from)
}

/// `fix_output` with `update_header`'s record count (`header`). `newname`: the output's name, or
/// `None` to write over the plugin.
pub fn fix_output(
    ctx: &mut Ctx,
    opts: &Output,
    inst: &Install,
    plugin: &str,
    modified: bool,
    newname: Option<String>,
    header: fn(&mut Ctx, &str, &mut [u8]) -> Res<()>,
) -> Res<()> {
    let tmp = format!("{plugin}.tmp");
    if !modified {
        ctx.print(&format!("{plugin} was not modified\n"));
        let _ = crate::fsx::remove_file(&tmp);
        return Ok(());
    }
    let mut newname = newname;
    if let Some(dir) = &opts.output_dir {
        let base = newname.clone().unwrap_or_else(|| plugin.to_string());
        newname = Some(format!("{dir}/{}", crate::clean::split_dir(&base).1));
    }
    if !crate::clean::has_plugin_ext(plugin) {
        return Err(ctx.abort(&format!("\"{plugin}\" name does not end in .esm/.esp/.ess")));
    }
    let t = times(plugin);
    if let Some(n) = &newname
        && abs(n) == abs(plugin)
    {
        ctx.msg(&format!(
            "fix_output(): newname ({n}) same as plugin ({plugin})"
        ));
        newname = None;
    }
    let write_header = |ctx: &mut Ctx, path: &str| -> Res<()> {
        let mut data = crate::fsx::read(path)
            .map_err(|e| ctx.abort(&format!("opening \"{path}\" for read/write ({e})")))?;
        header(ctx, path, &mut data)?;
        crate::fsx::write(path, &data)
            .map_err(|e| ctx.abort(&format!("opening \"{path}\" for read/write ({e})")))
    };
    match newname {
        Some(n) => {
            if !crate::clean::has_plugin_ext(&n) {
                return Err(ctx.abort(&format!("\"{n}\" name does not end in .esm/.esp/.ess")));
            }
            mv(&tmp, &n).map_err(|e| ctx.abort(&format!("Renaming \"{tmp}\" to \"{n}\" ({e})")))?;
            write_header(ctx, &n)?;
            set_times(&n, &t);
            ctx.print(&format!(
                "Output saved in: \"{n}\"\nOriginal unaltered: \"{plugin}\"\n"
            ));
        }
        None => {
            let bak = unique_backup_name(opts, inst, plugin)?;
            mv(plugin, &bak)
                .map_err(|e| ctx.abort(&format!("Renaming \"{plugin}\" to \"{bak}\" ({e})")))?;
            mv(&tmp, plugin)
                .map_err(|e| ctx.abort(&format!("Renaming \"{tmp}\" to \"{plugin}\" ({e})")))?;
            write_header(ctx, plugin)?;
            set_times(plugin, &t);
            ctx.print(&format!(
                "Output saved in: \"{plugin}\"\nOriginal backed up to: \"{bak}\"\n"
            ));
        }
    }
    Ok(())
}

/// An I/O error as Perl's `$!` reads: the system's message without Rust's " (os error N)".
pub fn os_message(e: &std::io::Error) -> String {
    let s = e.to_string();
    match s.rfind(" (os error ") {
        Some(k) => s[..k].to_string(),
        None => s,
    }
}
