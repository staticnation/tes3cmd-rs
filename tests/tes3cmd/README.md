# The tes3cmd the port is held to

`tes3cmd.pl` is tes3cmd 0.40-PRE-RELEASE-2 as it is released (MIT, John Moonsugar; the notice is
in the script's head and in this repository's LICENSE), the program `tests/vs_perl.rs` runs beside
the port when `TES3CMD_PL` points here (CI does).

It is not GitHub's current `master` (f72e9ed): that one renames with `rename` where this calls
`move` (so here `active --on`/`--off` die, see the README), and prints a backup's path with
`<DATADIR>` in it. The released one is what batch files and tools run, so it is the measure.

sha256: 5b59ac399a9bd9aba97665de40e7f14669e839120055baeda2faa14a3595ca57
