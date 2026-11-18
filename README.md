# tes3cmd in Rust

[tes3cmd](https://github.com/john-moonsugar/tes3cmd), John Moonsugar's command line tool for
TES3 (Morrowind) plugins, ported from its Perl (0.40-PRE-RELEASE-2) to Rust: the same commands,
flags, messages and output files, on Windows, Linux and macOS, without Perl.

Batch files and tools that run `tes3cmd` (the MOMW Tools Pack, mod managers) are meant to work
with it unchanged, command by command as each is ported.

## Status

| Command | |
|---|---|
| `clean`, `header`, `esp`, `esm`, `common`, `active`, `resetdates`, `overdial`, `recover` | ported, held to the Perl's output (below) |
| `dump` | ported: every record type a plugin has, the selectors (`--type`, `--id`, `--exact-id`, `--flag`, `--match`, `--no-match`, `--interior`, `--exterior`, `--instance-match`, `--instance-no-match`), `--list`, `-B`, `--format`/`--no-quote`, `--raw`/`--raw-with-header`, `--wrap` (Text::Wrap 2021.0814's own algorithm) |
| `delete`, `diff` | ported |
| `modify` | ported: `--replace` (a Perl `s///`: any delimiters, `$1`/`$&`, flags `gimsx`), with the record selectors and `--sub-match`/`--sub-no-match`. Not portable: `--run`, `--program-file` and `/e` run Perl code |
| `multipatch` | ported: the cell renamings, the fog bug, the summoned creatures' persistence, the leveled lists merged (`--delete-creature`/`--delete-item`), `--no-activate`. `--merge-objects` does nothing, as in tes3cmd |
| `fixit` | ported: `resetdates`, `clean` on every active plugin, `multipatch`, then every header synchronized |
| `lint`, `undelete`, `run`, `shell`, `codec`, `testcodec`, `stats`, `wikiout` | not planned: this tes3cmd hides them (`-lint` in its command table), and `run`/`shell` run Perl |

A command or option not ported says so and exits with 2; run the Perl tes3cmd for it.

## Use

As tes3cmd: run it from somewhere under the Morrowind folder (it finds `Data Files` and
`Morrowind.ini` above the working folder), usually from `Data Files` itself.

    tes3cmd clean "My Mod.esp"            # writes Clean_My Mod.esp beside it
    tes3cmd clean --overwrite *.esp       # cleans in place, the originals backed up as name~1.esp
    tes3cmd clean --gmsts --dups Mod.esp  # only those parts

`tes3cmd help` lists what is ported. `tes3cmd clean --help` (or any wrong option) prints
tes3cmd's own usage for clean.

As a library, `tes3cmd::cli::run(argv, out, err)` is the command line with its output in two
writers, and `tes3cmd::cli::run_in(dir, argv, out, err)` the same as if started in `dir`: for a
program that runs it inside itself, whose threads share one working folder (Wraithguard's Tools
page). `tes3cmd::cli::run_over(dir, files, argv, out, err)` runs in `dir` (a `Data Files` in a
Morrowind folder of the caller's) with the plugins it may need elsewhere named in `files`, by
name: they are read where they are (another program's data folders, OpenMW's VFS), so no master
is copied, and never changed there. A write, a rename, a removal or a new date on one goes to a
private copy under the run's `tes3cmd/overlay`, which the rest of the run reads in its place:
`fixit` synchronizing the headers of every active plugin, or `resetdates`, leave the caller's
Morrowind.esm as it was. All three answer the exit code; none exits the process.

## How it is held to tes3cmd

`tests/vs_perl.rs` builds plugins (masters, cells with references, evil GMSTs, junk cells, path
grids, broken and unknown records...), lays out a Morrowind folder for each case, runs the Perl
tes3cmd in one copy and this port in another, and compares standard output, standard error and
every file left behind, byte for byte (Perl's "at FILE line N." suffixes aside).

    TES3CMD_PL=tests/tes3cmd/tes3cmd.pl cargo test    # with Perl and tes3cmd's script (in the repository)
    TES3CMD_EXE=path/to/tes3cmd.exe cargo test        # or with tes3cmd's own program

Your own plugins, the same way: `T3_REAL_MW` is the Morrowind folder, `T3_REAL_PLUGINS` the
plugins to clean (comma-separated). They and the masters they name are copied into a folder of
their own and cleaned by both.

    T3_REAL_MW="C:\Games\Morrowind" T3_REAL_PLUGINS="Some Mod.esp" TES3CMD_EXE=... cargo test real_plugins -- --nocapture

The record definitions, the evil-GMST tables, every command's options and usage text, and the
record formatters tes3cmd generates (with every lookup and flag table) are generated from the
Perl itself (`tools/gen_schema.pl`, `tools/gen_evil.pl`, `tools/gen_commands.pl`,
`tools/gen_tostr.pl`, after `tools/prepare_lib.sh`), not copied by hand; `tools/check_tostr.pl`
runs each generated formatter beside the Perl's own (347 of them, all the same). The formatters
tes3cmd writes by hand are ported in `src/tostr.rs`.

`tests/data/all_records.esp` and `odd_records.esp` hold one record of every type a plugin has,
and `all_records.ess` every savegame type too, each subrecord packed by tes3cmd's own encoders
(`tools/gen_all_records.pl`; in `odd`, every lookup out of its table), so `dump` is compared on
all of them.

CI (`.github/workflows/ci.yml`) checks formatting, Clippy and the tests on Windows, Linux and
macOS, keeps each system's program, and on Linux runs every case against the released Perl
(`tests/tes3cmd`) in two time zones.

## Where it differs, on purpose or for now

- **`active --on` and `--off` work.** The released tes3cmd dies writing `Morrowind.ini`
  ("Undefined subroutine &TES3::Util::move"), leaving `Morrowind.ini.tmp` and the ini as it was;
  the port writes the new `[Game Files]` (the old ini kept as `Morrowind.ini.old`), as GitHub's
  later tes3cmd does. `tests/vs_perl.rs` checks these two on their own.
- **`diff` warns in a fixed order.** Its report and diff files are tes3cmd's; the warnings it
  prints while comparing come in record order, where tes3cmd's follow Perl's hash order (which
  changes from run to run).
- **`modify --replace` changes the fields of generic subrecords and a reference's owner faction**;
  tes3cmd also runs it over the fields of a few hand-decoded ones (a reference's position, a
  cell's coordinates), which the port leaves as they are.
- **tes3cmd's own `dump` quirks are kept**: `--binary` and `--header` do nothing (they set
  variables `dump` never reads; `--raw`/`--raw-with-header` are what write records),
  `--format`'s `TYPE:field` never finds a field (its pattern keeps the type's last letter), a
  field's subtype is remembered across records, and only the first of its "unknown field"
  messages is ever said (`msgonce` keys on `{@_}`, which is always 1).
- **No caches.** tes3cmd keeps Perl `Storable` files in `tes3cmd/cache` (masters, leveled lists);
  the port reads the plugins each run, and says what tes3cmd says the first time
  ("Creating Leveled Lists Cache"). With `--no-cache` both read every run.
- **`multipatch` and `fixit` activate the patch**, where the released tes3cmd dies (the same
  `move` as `active --on`). So `fixit` goes on to synchronize the headers, which tes3cmd never
  reaches. There tes3cmd's later code would also give every plugin the multipatch's author
  ("tes3cmd multipatch", left set by the patch's own header); the port leaves authors alone.
  `multipatch` lists the cell renamings it keeps in a fixed order (tes3cmd: hash order).
- **`overdial` lists in a fixed order.** tes3cmd walks Perl hashes, whose order changes from run
  to run; the port walks the plugins and topics sorted.

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
  never says "Loaded cached Master". It keeps none of its own: the cache holds every record of
  the master, so reading it back costs what reading the master does, which only pays where
  parsing is slow, as it is in Perl.
- **Perl's own wording.** Where tes3cmd dies inside Perl ("Can't locate object method",
  uninitialized values), the port prints the same text but not Perl's "at FILE line N." place.
- **The order of masters.** tes3cmd walks a plugin's masters in Perl's hash order, which changes
  from run to run; the port walks them by name. The result is the same, except where a junk-cell
  check stops at a master that lacks one of the cell's subrecords.

## License

MIT, as tes3cmd is: see `LICENSE`, which carries tes3cmd's notice.
