//! Where Morrowind is (`TES3::Util::find_morrowind`): the first folder at or above the working
//! folder that holds both "Data Files" and "Morrowind.ini", in any letter case. Files in Data
//! Files are found in any letter case too (`datapath`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ctx::Ctx;

pub struct Install {
    /// `$MWDIR`, `.` when none was found.
    pub mwdir: String,
    /// `$DATADIR`: `$MWDIR/<Data Files as spelt>`, `.` when none was found.
    pub datadir: String,
    pub tes3cmd_dir: String,
    pub cache_dir: String,
    /// No Morrowind found: no cache.
    pub no_cache: bool,
    pub backup_dir: String,
    listing: Option<HashMap<String, String>>,
}

/// `list_files`: a folder's names, by their lower case.
pub fn list_files(dir: &str) -> Option<HashMap<String, String>> {
    let rd = std::fs::read_dir(if dir.is_empty() { "." } else { dir }).ok()?;
    let mut m = HashMap::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        m.insert(name.to_lowercase(), name);
    }
    Some(m)
}

impl Install {
    /// `find_morrowind`, its folders made as tes3cmd makes them.
    pub fn find(ctx: &mut Ctx) -> Install {
        let start = std::env::var("PWD")
            .ok()
            .filter(|p| Path::new(p).is_dir())
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        let mut dir = start.replace('\\', "/");
        let mut found: Option<(String, String)> = None;
        while !dir.is_empty() && Path::new(&dir).is_dir() {
            if let Some(files) = list_files(&dir)
                && let (Some(data), Some(_)) = (files.get("data files"), files.get("morrowind.ini"))
            {
                found = Some((dir.clone(), format!("{dir}/{data}")));
                break;
            }
            let mut parts: Vec<&str> = dir.split(['/', '\\']).collect();
            parts.pop();
            dir = parts.join("/");
        }
        let mut i = match found {
            Some((mw, data)) => {
                if ctx.verbose {
                    ctx.msg(&format!("DATADIR = \"{data}\""));
                }
                let t = format!("{mw}/tes3cmd");
                let cache = format!("{mw}/tes3cmd/cache");
                let _ = std::fs::create_dir_all(&cache);
                Install {
                    mwdir: mw,
                    datadir: data,
                    tes3cmd_dir: t,
                    cache_dir: cache,
                    no_cache: false,
                    backup_dir: String::new(),
                    listing: None,
                }
            }
            None => {
                ctx.msg("WARNING: Can't find \"Data Files\" directory, functionality reduced. You should first cd (change directory) to somewhere under where Morrowind is installed.");
                Install {
                    mwdir: ".".into(),
                    datadir: ".".into(),
                    tes3cmd_dir: ".".into(),
                    cache_dir: ".".into(),
                    no_cache: true,
                    backup_dir: String::new(),
                    listing: None,
                }
            }
        };
        if ctx.verbose {
            ctx.print(&format!("<DATADIR> is \"{}\"\n", i.datadir));
        }
        let _ = std::fs::create_dir_all(&i.tes3cmd_dir);
        if Path::new(&format!("{}/tes3cmd", i.datadir)).is_dir() {
            ctx.msg(&format!(
                "[Found old version of tes3cmd directory: {}/tes3cmd]",
                i.datadir
            ));
            ctx.msg("[This old version can be safely removed. New version is now one level up.]");
        }
        i.backup_dir = format!("{}/backups", i.tes3cmd_dir);
        i
    }

    /// `datafiles_map(RELOAD)`: Data Files listed again (a file was just made there).
    pub fn reload(&mut self) {
        self.listing = None;
    }

    /// `datapath`: a file in Data Files, found in any letter case.
    pub fn datapath(&mut self, name: &str) -> Option<PathBuf> {
        if self.listing.is_none() {
            self.listing = list_files(&self.datadir);
        }
        let l = self.listing.as_ref()?;
        l.get(&name.to_lowercase())
            .map(|n| PathBuf::from(format!("{}/{n}", self.datadir)))
    }

    /// `load_order`: the ini's game files that are there, by date, masters first.
    pub fn load_order(&mut self) -> Vec<String> {
        let ini = list_files(&self.mwdir)
            .and_then(|m| m.get("morrowind.ini").cloned())
            .map(|n| format!("{}/{n}", self.mwdir));
        let Some(ini) = ini.and_then(|p| std::fs::read(p).ok()) else {
            return Vec::new();
        };
        let text = String::from_utf8_lossy(&ini).replace("\r\n", "\n");
        let mut files = Vec::new();
        let mut inside = false;
        for line in text.lines() {
            if !inside {
                inside = line.to_ascii_lowercase().starts_with("[game files]");
                continue;
            }
            if let Some(rest) = line.strip_prefix("GameFile") {
                let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                if digits > 0 && rest[digits..].starts_with('=') && rest.len() > digits + 1 {
                    files.push(rest[digits + 1..].to_string());
                }
            } else if line.starts_with('[') && line.len() > 2 && line.ends_with(']') {
                break;
            }
        }
        let mut active: Vec<(String, std::time::SystemTime)> = Vec::new();
        for f in files {
            if let Some(p) = self.datapath(&f) {
                let t = std::fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                active.push((f, t));
            }
        }
        active.sort_by_key(|a| a.1);
        let by = |ext: &str| {
            active
                .iter()
                .filter(|(n, _)| n.to_ascii_lowercase().ends_with(ext))
                .map(|(n, _)| n.clone())
                .collect::<Vec<_>>()
        };
        let mut out = by(".esm");
        out.extend(by(".esp"));
        out
    }
}
