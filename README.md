# tes3cmd in Rust

[tes3cmd](https://github.com/john-moonsugar/tes3cmd), John Moonsugar's command line tool for
TES3 (Morrowind) plugins, ported from its Perl (0.40-PRE-RELEASE-2) to Rust: the same commands,
flags, messages and output files, on Windows, Linux and macOS, without Perl.

Batch files and tools that run `tes3cmd` (the MOMW Tools Pack, mod managers) are meant to work
with it unchanged, command by command as each is ported.

## Status

| Command | |
|---|---|
| `clean` | ported, held to the Perl's output (below) |
| `header`, `esp`/`esm`, `dump`, `delete`, `modify`, `diff`, `common`, `undelete`, `recover` | next, in this order |
| `multipatch`, `fixit`, `overdial`, `lint`, `active`, `resetdates` | after those |
| `run`, `shell`, `codec`, `testcodec` | not planned (they run Perl, or test its codecs) |

A command not ported yet says so and exits with 2; run the Perl tes3cmd for it.

## Use

As tes3cmd: run it from somewhere under the Morrowind folder (it finds `Data Files` and
`Morrowind.ini` above the working folder), usually from `Data Files` itself.

    tes3cmd clean "My Mod.esp"            # writes Clean_My Mod.esp beside it
    tes3cmd clean --overwrite *.esp       # cleans in place, the originals backed up as name~1.esp
    tes3cmd clean --gmsts --dups Mod.esp  # only those parts

`tes3cmd help` lists what is ported. `tes3cmd clean --help` (or any wrong option) prints
tes3cmd's own usage for clean.

## How it is held to tes3cmd

`tests/vs_perl.rs` builds plugins (masters, cells with references, evil GMSTs, junk cells, path
grids, broken and unknown records...), lays out a Morrowind folder for each case, runs the Perl
tes3cmd in one copy and this port in another, and compares standard output, standard error and
every file left behind, byte for byte (Perl's "at FILE line N." suffixes aside).

    TES3CMD_PL=path/to/tes3cmd cargo test             # with Perl and tes3cmd's script
    TES3CMD_EXE=path/to/tes3cmd.exe cargo test        # or with tes3cmd's own program

Your own plugins, the same way: `T3_REAL_MW` is the Morrowind folder, `T3_REAL_PLUGINS` the
plugins to clean (comma-separated). They and the masters they name are copied into a folder of
their own and cleaned by both.

    T3_REAL_MW="C:\Games\Morrowind" T3_REAL_PLUGINS="Some Mod.esp" TES3CMD_EXE=... cargo test real_plugins -- --nocapture

The record definitions and the evil-GMST tables are generated from the Perl itself
(`tools/gen_schema.pl`, `tools/gen_evil.pl`, after `tools/prepare_lib.sh`), not copied by hand.

## Where it differs, on purpose or for now

- **Records as bytes.** tes3cmd compares and writes subrecords as bytes, and so does this port; it
  does not read plugins through a typed library, which could write some bytes differently.
- **tes3cmd's own quirks are kept**, since the output is the measure. Some of them:
  - `clean` declares `--output-dir` without a value, so it is a flag: the output goes into a
    folder named `1`, and the word after it is taken as a plugin.
  - A cell's `NAM0` (its count of temporary references) is never updated when references are
    cleaned: tes3cmd looks for groups that start with NAM0, and only FRMR starts one.
  - A path grid whose name has capitals is never cleaned as a duplicate: the masters are indexed
    by a lower-case id, the plugin's path grid by its name as written.
  - A cell rebuilt after cleaning its references loses its header flags.
- **The master cache.** tes3cmd keeps parsed masters in `tes3cmd/cache` as Perl `Storable`
  files, which this port cannot read; it reads the masters each run (as `--no-cache` does), so it
  never says "Loaded cached Master".
- **Perl's own wording.** Where tes3cmd dies inside Perl ("Can't locate object method",
  uninitialized values), the port prints the same text but not Perl's "at FILE line N." place.
- **The order of masters.** tes3cmd walks a plugin's masters in Perl's hash order, which changes
  from run to run; the port walks them by name. The result is the same, except where a junk-cell
  check stops at a master that lacks one of the cell's subrecords.

## License

MIT, as tes3cmd is: see `LICENSE`, which carries tes3cmd's notice.
