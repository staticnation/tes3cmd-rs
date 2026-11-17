#!/bin/sh
# Makes lib.pl, the copy of tes3cmd the generators load: the script without its last line (the
# call to tes3cmd_main), with the tables they read declared `our` rather than `my`.
#   tools/prepare_lib.sh path/to/tes3cmd > tools/lib.pl
#   cd tools && perl gen_schema.pl > ../src/schema_table.rs && perl gen_evil.pl > ../src/evil.rs
sed '$d' "$1" | sed 's/^my @RECDEFS =/our @RECDEFS =/; s/^my %EVIL_TB =/our %EVIL_TB =/; s/^my %EVIL_BM =/our %EVIL_BM =/'
