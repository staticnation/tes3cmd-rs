//! tes3cmd, the TES3 plugin tool by John Moonsugar, ported from its Perl (0.40-PRE-RELEASE-2) to
//! Rust: the same commands, flags, messages and output, without Perl. Each command is held to
//! the Perl's own output on the same plugins (`tests/vs_perl.rs`).
//!
//! Ported: `clean`, `header`, `esp`, `esm`, `common`, `active`, `resetdates`, `overdial`,
//! `recover`, `dump`, `delete`, `modify --replace`, `diff`, `multipatch`, `fixit`. The rest are listed in the README, in the order they are planned.

pub mod clean;
pub mod cli;
pub mod commands;
pub mod commands_table;
pub mod ctx;
pub mod datadir;
pub mod decode;
pub mod edit;
pub mod evil;
pub mod header;
pub mod multipatch;
pub mod output;
pub mod pack;
pub mod record;
pub mod schema_table;
pub mod tostr;
#[rustfmt::skip]
pub mod tostr_table;

/// The tes3cmd this is a port of.
pub const TES3CMD_VERSION: &str = "0.40-PRE-RELEASE-2";
