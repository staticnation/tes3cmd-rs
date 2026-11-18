// Generated from tes3cmd 0.40-PRE-RELEASE-2's %COMMAND by tools/gen_commands.pl - do not edit.
// (name as dispatched, each Getopt::Long spec, the usage text die() prints)
pub const COMMANDS: &[(&str, &[&str], &str)] = &[
    ("-codec", &["assert", "debug", "output=s", "verbose"], "Usage: tes3cmd -codec
"),
    ("-lint", &["assert", "debug", "output=s", "verbose", "active", "all", "autocalc-spells", "bloodmoon-dependency", "cell-00", "clean", "deprecated-lists", "dialogue-teleports", "duplicate-info", "duplicate-records", "expansion-dependency", "evil-gmsts", "fogbug", "fogsync", "getsoundplaying", "ignore-plugin=s@", "junk-cells", "master-sync", "menumode", "missing-author", "missing-description", "missing-version", "modified-info", "modified-info_id", "record-count", "overrides", "no-bloodmoon-functions", "no-tribunal-functions", "scripted-doors"], "Usage: tes3cmd lint OPTIONS plugin...

OPTIONS:
 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --all/recommended
\tEnable all or recommended output options. When --recommended is
\tselected only the following options are enabled:
\t--bloodmoon-dependency, --clean, --deprecated-lists,
\t--duplicate-records, --evil-gmsts, --fog-sync, --fogbug,
\t--junk-cells, --modified-info-id, --expansion-dependency

 --autocalc-spells
\tThe plugin defines autocalc'ed spells.

 --bloodmoon-dependency
\tThe plugin uses Bloodmoon specific functions, so it needs version
\t1.6.1820, but the plugin does not list Bloodmoon.esm in its list of
\tMasters. This is similar to the --expansion-dependency option, but it
\tis for Bloodmoon only.

 --cell-00
\tThis means that an apparently dirty copy of cell (0, 0) is included in
\tthe plugin. This cell is often accidentally modified since it is the
\tdefault cell displayed in the cell view window.

 --clean
\tIf this option is used, then a plugin that is not flagged with any of
\tthe other options will be flagged \"CLEAN\". It really just means:
\t\"found nothing of interest\".

 --deprecated-lists
\tThis flag indicates that the plugin makes deprecated use of scripting
\tfunctions to modify standard Bethesda Leveled Lists. This is bad
\tbecause these lists will then be stored in the user's savegame just
\tlike any other changed object. Since savegames load after all plugins
\t(including \"Mashed Lists.esp\" or \"multipatch.esp\" which contains
\tmerged leveled lists), this means that merged leveled lists will be
\tignored. Note that this warning only applies to changes to Bethesda
\tlists. Changes to leveled lists specific to the mod will not be
\tflagged. For more about the problem of using scripting functions to
\tmodify Bethesda lists see:
\thttp://www.uesp.net/wiki/Tes3Mod:Leveled_Lists

 --dialogue-teleports
\tCrashes can happen when the functions Position/PositionCell are called
\tin dialogue results. It is recommended that you do these kinds of
\tteleports via a script and start the script in the dialogue results
\tinstead.

 --duplicate-info
\tindicates the exact INFO dialog with the same ID, text and filter
\texists in one of the masters of this plugin. This may be a problem, or
\tmaybe not. Sometimes duplicate dialog is intentional in order for
\tdialog sorting to be correct. It's only a mistake if it really was
\tunintentional.

 --duplicate-records
\tThese are records in the plugin that are exact duplicates of records
\tthat occur in one of the masters. These are sometimes referred to as
\t\"dirty\" or \"unclean\" entries.

 --evil-gmsts
\tYou know this one! This means that some of the \"72 Evil GMSTs\" are
\tpresent in this plugin and that they have the same exact value as the
\toriginal GMST in the master .esm. Other GMSTs not in the list of known
\tEvil GMSTs or GMSTs that have been modified from their original values
\tare not flagged. Note that it is also possible to have other GMSTs
\tshow up in the DUP-REC list, but those are not from the list of 72
\tGMSTs that would get introduced by the Construction Set.

 --expansion-dependency
\tThe plugin requires either Tribunal or Bloodmoon because it uses
\tengine features not present in the original Morrowind such as startup
\tscripts or Tribunal functions, but the plugin has not listed any
\texpansions in its list of Masters. This is not necessarily a problem,
\tbut it may contradict the Readme for the           mod sometimes.

 --fog-sync
\tThis option indicates that for some reason that the fog density setting
\tin the CELL.DATA subrecord is not equal to the fog density setting in
\tthe CELL.AMBI subrecord. This is unusual, it's probably caused by
\tediting the plugin in Enchanted Editor, and it's probably harmless,
\tbut you can resync the values by editing that cell in the Construction
\tSet.

 --fogbug
\tThe plugin has an interior CELL with a fog density of 0.0, and the
\tcell is not marked to \"behave as exterior\". This circumstance can
\ttrigger the \"fog bug\" in some graphics cards, resulting in the cell
\tbeing rendered as a black or featureless blank screen.

 --getsoundplaying
\tThe plugin appears to use the scripting function GetSoundPlaying to
\tdetect events (as opposed to managing the playing of sound files).
\tGetSoundPlaying fails consistently on a small number of users'
\tsystems, so these users will encounter problems with these scripts.
\tNote that tes3lint uses regular expressions to detect the purpose for
\tusing GetSoundPlaying, and it will sometimes get false positives. For
\tmore about the GetSoundPlaying problem see:
\thttp://sites.google.com/site/johnmoonsugar/Home/morrowind-scripting-tips

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --junk-cells
\tThese are bogus external CELL records that crop up in many plugins,
\tdue to a Construction Set bug. They contain only NAME, DATA and
\tsometimes RGNN subrecords, and the flags in the DATA subrecord are
\tunchanged from the flags in the Master.

 --master_sync
\tChecks the plugin is in sync with its masters by checking their size
\tagainst the size recorded in the plugin header. This is usually never
\ta problem, but it means that Morrowind will give the warning: \"One of
\tthe files that \"(plugin)\" is dependent on has changed since the last
\tsave...\"

 --menumode
\tThe plugin contains scripts that do not check menumode. Each script
\twill be printed in the details section. This may or may not be
\tintentional.

 --missing-author
\tThe plugin header is missing the Author field. It is strongly
\trecommended that authors put their name or handle in the Author field.

 --missing-description
\tThe plugin header is missing a description field. It is strongly
\trecommended that a short description of the plugin is entered in this
\tfield.

 --missing-version
\tThe plugin header description field is missing a \"Version: X.Y\"
\tstring. It is strongly recommended that the version number be added to
\tthe description. This version number is very helpful in documenting
\twhich version of the plugin a user is using. This information is used
\tby tools such as Wrye Mash and mlox. The regular expression pattern
\tused by Wrye Mash to match the version is: \"^(Version:?)
\t*([-0-9\\.]*\\+?) *\\r?$\". (mlox will match a much wider variety of
\tversion number formats). It is safest to use a string of the format:
\t\"Version: X.Y\", on a line by itself.

 --modified-info
\tindicates that an INFO dialog with the same ID exists in the master,
\tbut this plugin has modified the text or the filter. This is quite
\tcommon when a plugin intentionally modifies a master's dialog, but it
\toften a problem when due to unintentional changes. There may be an
\tassociated \"modified-info-id\".

 --modified-info-id
\tindicates that the exact same INFO dialog text exists in this plugin
\tas in a master, but under a different ID. When associated with a
\t\"modified-info\", this situation may be due to creating new dialog by
\tcopying the original, and then accidentally editing the original
\tinstead of the copy which may cause bad dialog problems and should
\tprobably be investigated further.

 --no-bloodmoon-functions
\tBloodmoon.esm is listed as a master, but lint did not detect any usage
\tof Bloodmoon functions. (Note that there may be other reasons for
\tlisting Bloodmoon as a master).

 --no-tribunal-functions
\tTribunal.esm is listed as a master, but lint did not detect any usage
\tof Tribunal functions. (Note that there may be other reasons for
\tlisting Tribunal as a master).

 --overrides
\tThe plugin contains record(s) that override the record with the same
\tID in one of its masters.

 --record-count
\tThis means that the number of records recorded in the TES3 plugin
\theader field does not match the actual number of records in the
\tplugin. This situation is not known to cause any errors or warnings in
\tthe CS or in the Morrowind game itself, so it's just presented as a
\tmatter of interest.

 --scripted-doors
\tThe plugin adds scripts to Bethesda doors, which has the potential
\tto break any script in the same cell that uses the CellChanged
\tfunction. CellChanged in that cell will always return 0 from now on.
\tSee: http://sites.google.com/site/johnmoonsugar/Home/morrowind-scripting-tips

DESCRIPTION

The lint command normally only prints output for interesting things it finds.
If it finds nothing for a plugin, no output is generated unless the --clean
option is given. When something interesting is found, a flag is printed after
the plugin name. On following lines, an indented report is printed that gives
more detail about the flagged items.

"),
    ("-run", &["assert", "debug", "output=s", "verbose", "backup-dir=s", "hide-backups", "active", "exact-id=s@", "exterior", "flag=s@", "id=s@", "ignore-plugin=s@", "interior", "match=s@", "no-match|M=s@", "program-file=s", "sub-match=s", "sub-no-match=s", "type=s@"], "Usage: tes3cmd run programfile OPTIONS plugin...

Loads a tes3cmd extension \"programfile\" to perform custome processing of plugins.

OPTIONS:
 --debug
\tturn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --backup-dir
\twhere backup files get stored. The default is:
\t\"morrowind/tes3cmd/backups\"

 --exact-id* <id-string>
\tonly modify records whose ids exactly match given <id-string>

 --exterior
\tonly match if record is an Exterior CELL

 --flag* <flag>
\tonly modify records with given flag. Flags may be given symbolically
\tas: (deleted, persistent, ignored, blocked), or via their numeric
\tvalues (i.e. persistent is 0x400).

 --hide-backups
\tany created backup files will get stashed in the backup directory
\t(--backup-dir)

 --id* <id-regex>
\tonly modify records whose ids match regular expression pattern
\t<id-regex>

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --interior
\tonly match if record is an Interior CELL

 --match* <regex>
\tonly modify records that match given regular expression <regex>
\tIf more than one of these switches is supplied, ALL must match
\tfor a record to be processed.

 --no-match*|--M* <regex>
\tonly modify records that do not match given regular expression
\t<regex>
\tIf more than one of these switches is supplied, if ANY match,
\tthat record will be skipped.

 --sub-match <regex>
\tonly modify the subrecords that match <regex>. When this option (or
\t--sub-no-match) are used, modification only applies to matching
\tsubrecords. Without either of these options modifications are
\tperformed on whole records.

 --sub-no-match <regex> only modify the subrecords that do not match <regex>.
\t(see also --sub-match above).

 --type* <record-type>
\tonly modify records with the given <record-type>

 Note: starred (*) options can be repeated, and matching of strings is not
 case-sensitive.

This command allows you to do complex batch modifications of plugin records.
You can really damage things with this command, so be careful!

Note: documentation for regular expressions:
  http://perldoc.perl.org/perlre.html

Example(s):

# Run your homegrown extension
tes3cmd run myprog.pl ...

"),
    ("-shell", &["assert", "debug", "output=s", "verbose"], "Usage: tes3cmd -shell

Example:

# Run a shell, and create a new blank ESP file:
% tes3cmd -shell
> open($F,\">blank.esp\") && print $F make_header({ author => 'me', description => '' });

==> 1
[type Ctrl-D to quit shell]
"),
    ("-stats", &["assert", "debug", "output=s", "verbose", "active"], "Usage: tes3cmd stats OPTIONS plugin...
"),
    ("-testcodec", &["assert", "debug", "output=s", "verbose", "continue", "exclude-type|x=s@", "ignore-cruft"], "Usage: tes3cmd -testcodec [--exclude-type|-x exclude_type]
"),
    ("-undelete", &["assert", "debug", "output=s", "verbose"], "Usage: tes3cmd undelete OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --verbose
\tprint items that are undeleted from each cell.

DESCRIPTION:

Sometimes when a plugin deletes an object from a cell, and another plugin in
your load order comes along and tries to modify that object (perhaps by moving
it), then you get a message about the object \"is missing in Master file\". This
function allows you to safely undelete those objects so that you do not get
those annoying messages.

When this function undeletes an object instance, it removes the DELE
subrecord and adds ZNAM subrecord to mark the object as disabled. It also
adds a DATA subrecord that positions the disabled instance at the origin (since
object instances that have been deleted do not have a DATA subrecord, and
Morrowind expects all object instances to have a position).
"),
    ("-wikiout", &["assert", "debug", "output=s", "verbose"], "Usage: tes3cmd -wikiout
"),
    ("active", &["assert", "debug", "output=s", "verbose", "off", "on"], "Usage: tes3cmd active OPTIONS plugin1.esp [plugin2.esp...]

OPTIONS:
 --debug
\tturn on debug messages

 --off
\tdeactivate given plugins

 --on
\tactivate given plugins

DESCRIPTION:

Activates/Deactivates the specified plugins. When no options are specified,
just prints current active load order.

"),
    ("clean", &["assert", "debug", "output=s", "verbose", "backup-dir=s", "hide-backups", "active", "all", "ignore-plugin=s@", "instances", "cell-params", "dups", "gmsts", "junk-cells", "no-cache", "output-dir", "replace", "overwrite"], "Usage: tes3cmd clean OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --backup-dir
\twhere backup files get stored. The default is:
\t\"morrowind/tes3cmd/backups\"

 --cell-params
\tclean cell subrecords AMBI,WHGT duped from masters

 --dups
\tclean other complete records duped from masters

 --gmsts
\tclean Evil GMSTs

 --hide-backups
\tany created backup files will get stashed in the backup directory
\t(--backup-dir)

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --instances
\tclean object instances from cells when duped from masters

 --junk-cells
\tclean junk cells (no new info from definition in masters)

 --no-cache
\tdo not create cache files (used for speedier cleaning)

 --output-dir <dir>
\tset output directory to <dir> (default is where input plugin is)

 --overwrite
\toverwrite original plugin with clean version (original is backed up)

DESCRIPTION:

Cleans plugins of various junk. If no cleaning options are selected, the
default is to assume the options:

  --instances --cell-params --dups --gmsts --junk-cells

The goal of the \"clean\" command is that it should always be safe to use it
with no options to get the default cleaning behavior. The different cleaning
operations are explained below:

Much of what is considered \"dirt\" is with respect to a plugin's masters.
Plugins that do not have masters cannot be checked for duplicate object
definitions and instances (references), cell parameters and junk-cells.

Object Instances (--instances)

  The clean command will clean objects in the plugin that match objects in any
  of its masters. Object instances in cells (sometimes called \"references\",
  but \"object instance\" is a more accurate descriptive term) are defined as
  byte sequences starting in the subrecord following a FRMR subrecord, and
  match only if this is the same byte sequence as in the master, along with
  the same Object-Index from the FRMR. NAM0 subrecords are not part of object
  instances, and if instances are deleted from a cell, the NAM0 subrecord for
  the cell is updated to reflect any changing instance count.

Cell Params (--cell-params)

  The subrecords for AMBI (ambient lighting) and WHGT (water height) for
  interior cells are often duplicated from the master of a plugin when the
  plugin is saved in the Construction Set.

Duplicate Records (--dups)

  Object definitions for various record types defined in a master are
  sometimes unnecessarily duplicated in dependent plugins, and this option
  will safely clean them. Only objects that have identical flags and byte
  sequences will be cleaned.

Evil GMSTs (--gmsts)

  An Evil GMST is defined as a GMST from the list of 11 Tribunal GMSTs or 61
  Bloodmoon GMSTs that are inadvertently introduced to a plugin by the
  Construction Set with specific values. Other GMSTs or GMSTs from those lists
  that do not have the specific Evil Values are NOT cleaned by this function.

  To clean GMSTs that are not Evil, you can use the command:
    \"tes3cmd delete --type gmst\"

Junk Cells (--junk-cells)

  Junk cells are bogus external CELL records that crop up in many plugins due
  to a Construction Set bug. They contain only NAME, DATA and sometimes RGNN
  subrecords with data identical to the master. (In addition, interior cells
  will also be removed if they do not introduce any new information).

Cache Files Feature

  tes3cmd will normally create cached data files for your masters in the
  subdirectory: \"morrowind/tes3cmd/cache\". If you do not wish tes3cmd to create
  cache files, you can use the --no-cache option. (But it is recommended you
  do use them for speedier cleaning).

EXAMPLES:

# clean my plugin of only Evil GMSTs:
tes3cmd clean --gmsts \"my plugin.esp\"

# clean 2 plugins and put the cleaned versions in a subdirectory \"Clean\":
tes3cmd clean --output-dir Clean \"my plugin1.esp\" \"my plugin2.esp\"

# clean all plugins in the current directory, replacing the originals with
# the cleaned versions and save the diagnostic output to a file (clean.txt):
tes3cmd clean --overwrite *.esm *.esp > clean.txt
"),
    ("common", &["assert", "debug", "output=s", "verbose"], "Usage: tes3cmd common OPTIONS plugin1 plugin2

OPTIONS:
 --debug
\tturn on debug messages

DESCRIPTION:

Prints the IDs of records that the 2 given plugins have in common.

EXAMPLES:

# Show the records in common between my plugin and Morrowind.esm:
tes3cmd common \"my plugin.esp\" \"Morrowind.esm\"
"),
    ("delete", &["assert", "debug", "output=s", "verbose", "backup-dir=s", "hide-backups", "active", "ignore-plugin=s@", "instance-match=s", "instance-no-match=s", "exact-id=s@", "exterior", "flag=s@", "id=s@", "interior", "match=s@", "no-match|M=s@", "sub-match=s", "sub-no-match=s", "type=s@"], "Usage: tes3cmd delete OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --backup-dir
\twhere backup files get stored. The default is:
\t\"morrowind/tes3cmd/backups\"

 --exact-id* <id-string>
\tonly delete records whose ids exactly match given <id-string>

 --exterior
\tonly delete if record is an Exterior CELL

 --flag* <flag>
\tonly delete records with given flag. Flags may be given symbolically
\tas: (deleted, persistent, ignored, blocked), or via their numeric
\tvalues (i.e. persistent is 0x400).

 --hide-backups
\tany created backup files will get stashed in the backup directory
\t(--backup-dir)

 --id* <id-regex>
\tonly delete records whose ids match regular expression pattern
\t<id-regex>

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --instance-match <regex>
\tOnly delete the object instances that match <regex> in the matching cell

 --instance-no-match <regex>
\tOnly delete object instances in the cell that do not match <regex>

 --interior
\tonly delete if record is an Interior CELL

 --match* <regex>
\tonly delete records that match given regular expression <regex>
\tIf more than one of these switches is supplied, ALL must match
\tfor a record to be processed.

 --no-match*|--M* <regex>
\tonly delete records that do not match given regular expression
\t<regex>
\tIf more than one of these switches is supplied, if ANY match,
\tthat record will be skipped.

 --sub-match <regex>
\tonly delete the subrecords that match <regex>

 --sub-no-match <regex>
\tonly delete the subrecords that do not match <regex>

 --type* <record-type>
\tonly delete records with the given <record-type>

 Note: starred (*) options can be repeated, and matching of strings is not
 case-sensitive.

DESCRIPTION:

Deletes entire records, or subrecords from records, or object instances from
cell records. You can really damage things with this command, so be careful!

One thing you should be extra careful with is deleting objects from cells.
When objects from a master are \"moved\" by a plugin, a MVRF group appears
before the object instance that starts with a FRMR subrecord. If you really
want to delete that object, you probably want to delete both the FRMR object
instance and the preceeding MVRF group. A future version of tes3cmd may handle
this situation more elegantly.

Note: documentation for regular expressions:
  http://perldoc.perl.org/perlre.html

EXAMPLES:

# Delete all records with IDs matching the pattern: \"foo\":
# (Note that this doesn't also delete records that may depend on \"foo\").
tes3cmd delete --id foo \"my plugin.esp\"

# Delete all GMST records from a plugin:
tes3cmd delete --type gmst plugin.esp

# Delete all records flagged as \"ignored\":
tes3cmd delete --flag ignored plugin.esp

# Delete the poisonbloom spell subrecords from all current \"_mca_\" NPCs in quicksave.ess:
tes3cmd delete --type npc_ --match _mca_ --sub-match spell:poisonbloom quicksave.ess
"),
    ("diff", &["assert", "debug", "output=s", "verbose", "ignore-type=s@", "1-not-2|e1", "2-not-1|e2", "equal", "sortsubrecs", "types", "not-equal|ne"], "Usage: tes3cmd diff OPTIONS plugin1 plugin2

OPTIONS:
 --debug
\tturn on debug messages

 --ignore-type* <record-type>
\tignore given type(s)

 --1-not-2|--e1
\treport records in plugin1 that do not exist in plugin2

 --2-not-1|--e2
\treport records in plugin2 that do not exist in plugin1

 --equal
\treport records in plugin1 that are equal in plugin2

 --not-equal|--ne
\treport records in plugin1 that are different in plugin2

 --sortsubrecs
\ttwo records being compared may be functionally equivalent but
\tdiffer because their subrecords are in different order. This
\tswitch will first sort subrecords before comparison so
\tsubrecord order will not matter for comparison.

 Note: starred (*) options can be repeated, and matching of strings is not
 case-sensitive.

DESCRIPTION:

Prints a report on the differences between the two TES3 files.
A summary report with up to four sections is printed to standard output
that gives an overview of differences, as lists of record IDs.
(Report sections that would have no items are not printed).

When records in plugin1 are different in plugin2, each of these records is
printed in detail to a file \"plugin1-diff.txt\" and \"plugin2-diff.txt\", which
can then be compared textually using a tool such as WinMerge or the ediff
function of Emacs. Note that the output records will be sorted alphabetically
by record type to make the comparison using these tools easier.

To reduce a great deal of \"uninteresting\" differences when diffing savegames,
the the ModIndex field of CELL.FRMR records are automatically ignored. (Note
that in this case, the ObjIndex appears to only be incremented by one).

EXAMPLES:

# Print report on differences between 2 savegames (output to diff.out):
tes3cmd diff \"save1000.ess\" \"save2000.ess\" > diff.out

# You can also use the --ignore-type switch to ignore further subfields in
# order to help reduce the amount of differences as in the following example.
# Report on differences, but ignore the subfields CREA.AIDT and CELL.ND3D:
tes3cmd diff --ignore-type crea.aidt --ignore-type cell.nd3d testa0000.ess testb0000.ess > diff.out

# Just print the records that differ
tes3cmd diff --not-equal \"my plugin1.esp\" plugin2.esp
"),
    ("dump", &["assert", "debug", "output=s", "verbose", "active", "binary", "exact-id=s@", "exterior", "flag=s@", "format=s@", "no-quote", "header", "id=s@", "ignore-plugin=s@", "instance-match=s", "instance-no-match=s", "interior", "list", "match=s@", "no-banner|B", "no-match|M=s@", "wrap", "raw-with-header=s", "raw=s", "separator=s", "type=s@"], "Usage: tes3cmd dump OPTIONS plugin...

OPTIONS:
 --debug
\t turn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --binary
\toutput raw binary records. (Default is to output human readable text)

 --header
\toutput an initial TES3 header record.

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --instance-match <regex>
\twhen printing cells, only print the matching object instances in the
\tcell

 --instance-no-match <regex>
\twhen printing cells, only print the non-matching object instances in
\tthe cell

 --exact-id* <id-string>
\tonly dump records whose ids exactly match given <id-string>

 --exterior
\tonly match if record is an Exterior CELL

 --flag* <flag>
\tonly dump records with given flag. Flags may be given symbolically
\tas: (deleted, persistent, ignored, blocked), or via their numeric
\tvalues (i.e. persistent is 0x400).

 --format* <formatstring>
\tOnly print out given fields, with fields specified by names enclosed
\twith \"%\". For example, the NPC_ screen name field would be: %name%
\tIf multiple formats are given each defines a separate line of output.
\tOutput fields are automatically quoted if they contain whitespace.
\t(see also --no-quote).

 --id* <id-regex>
\tonly process records whose ids match regular expression pattern
\t<id-regex>

 --interior
\tonly match if record is an Interior CELL

 --list
\tonly list the ids of the records to be dumped, instead of the entire record

 --match* <regex>
\tonly process records that match given regular expression <regex>
\tIf more than one of these switches is supplied, ALL must match
\tfor a record to be dumped.

 --no-banner|--B
\tdo not print banner identifying the current plugin

 --no-match*|--M* <regex>
\tonly process records that do not match given regular expression
\t<regex>
\tIf more than one of these switches is supplied, if ANY match,
\tthat record will be skipped.

 --no-quote
\tTurn off automatic quoting of output fields in the --format switches.

 --output <filename>
\tSpecify the name of the output file where records are dumped.

 --wrap
\tWrap some long fields for more nicely formatted output.

 --raw <file>
\t[Obsolete: use --binary --output <file>]
\tdump raw records to <file>, instead of as text. If <file> exists, records
\tare appended to the end.

 --raw-with-header <file>
\t[Obsolete: use --binary --header --output <file>]
\tdump raw records with an initial TES3 header record to <file>

 --separator <string>
\tseparate subrecords with given <string>. Normally subrecords are
\tseparated by line-breaks. You can use this option to change that so
\tthey are all printed on one line.

 --type* <record-type>
\tonly dump records with the given <record-type>

 Note: starred (*) options can be repeated, and matching of strings is not
 case-sensitive.

DESCRIPTION:

Dumps the plugin to stdout in text form for easy perusal or in raw form for
extracting a subset of records to create a new plugin. For large plugins, the
text output can be voluminous. In the text output, a starred subrecord
indicates it is related to following subrecords.

Note: documentation for regular expressions:
  http://perldoc.perl.org/perlre.html

EXAMPLES:

# Print out NPC's by Id and Name and Faction:
tes3cmd dump --type npc_ --format \"%id% %name% %faction%\" Morrowind.esm

# List all the records in the plugin, just one per line, by type and ID:
tes3cmd dump --list \"Drug Realism.esp\"

# Dump all records from a plugin. we redirect the output to a file as there
# can be a lot of output:
tes3cmd dump \"DwemerClock.esp\" > \"DwemerClock-dump.txt\"

# Dump all records with IDs exactly matching \"2247227652827822061\":
tes3cmd dump --exact-id 2247227652827822061 LGNPC_AldVelothi_v1_20.esp

# Dump all the DIAL and INFO records from a plugin:
tes3cmd dump --type dial --type info \"abotWhereAreAllBirdsGoing.esp\"

# Dump all records flagged as persistent (by name) OR blocked (by value)
tes3cmd dump --flag persistent --flag 0x2000 \"Suran_Underworld_2.5.esp\"

# Dump all object instances in cells that match \"galbedir\":
# (this will just show you the cell header, and subrecords just for Galbedir
# and her desk and chest in the cell: \"Balmora, Guild of Mages\"):
tes3cmd dump --instance-match \"name:.*galbedir\" Morrowind.esm
"),
    ("esm", &["assert", "debug", "output=s", "verbose", "ignore-plugin=s@", "overwrite"], "Usage: tes3cmd esm OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --overwrite
\toverwrite output if it exists

DESCRIPTION:

Copies input plugin (.esp) to a master (.esm). If the output file already
exists, you must add the --overwrite option to overwrite it.

EXAMPLES:

# output is: \"my plugin.esm\"
tes3cmd esm \"my plugin.esp\"
"),
    ("esp", &["assert", "debug", "output=s", "verbose", "ignore-plugin=s@", "overwrite"], "Usage: tes3cmd esp OPTIONS master...

OPTIONS:
 --debug
\tturn on debug messages

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --overwrite
\toverwrite output if it exists

DESCRIPTION:

Copies input master (.esm) to a plugin (.esp). If the output file already
exists, you must add the --overwrite option to overwrite it.

EXAMPLES:

# output is: \"my plugin.esp\"
tes3cmd esp \"my plugin.esm\"
"),
    ("fixit", &["assert", "debug", "output=s", "verbose", "backup-dir=s", "hide-backups", "ignore-plugin=s@"], "Usage: tes3cmd fixit OPTIONS

OPTIONS:
 --debug
\tturn on debug messages

 --backup-dir
\twhere backup files get stored. The default is:
\t\"morrowind/tes3cmd/backups\"

 --hide-backups
\tany created backup files will get stashed in the backup directory
\t(--backup-dir)

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

DESCRIPTION:

The fixit command does the following operations:
- Cleans all your plugins (\"tes3cmd clean\")
- Synchronizes plugin headers to your masters (\"tes3cmd header --synchronize\")
- Generates a patch for merged leveled lists and more (\"tes3cmd multipatch\")
- Resets Dates on Bethesda data files (\"tes3cmd resetdates\")
"),
    ("header", &["assert", "debug", "output=s", "verbose", "backup-dir=s", "hide-backups", "active", "author=s", "description=s", "ignore-plugin=s@", "multiline", "synchronize", "update-masters", "update-record-count"], "Usage: tes3cmd header OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --author <author>
\tset the Author field to <author>

 --backup-dir
\twhere backup files get stored. The default is:
\t\"morrowind/tes3cmd/backups\"

 --description <desc>
\tset the Description field to <desc>

 --hide-backups
\tany created backup files will get stashed in the backup directory
\t(--backup-dir)

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --multiline
\tmulti-line output for listing field contents

 --synchronize
\tsame as: --update-masters --update-record-count

 --update-masters
\tupdates master list to reflect new versions

 --update-record-count
\tupdate record count in header

DESCRIPTION:

When no options are given, the author and description are printed.

Author and Description field values are normally replaced by the given string.
But if the string begins with a \"+\", the existing value is appended with the
new given value.

If a given value contains the string \"\\n\", it will be replaced by a CRLF.

Note:
 - the Author value should fit in 32 bytes.
 - the Description value should fit in 256 bytes.

If the value supplied will not fit into the plugin header field, you will be
warned.

The --update-masters (or --synchronize) option will clear any warnings
Morrowind gives when it starts up that say: \"One or more plugins could not
find the correct versions of the master files they depend on...\"

EXAMPLES:

# Show the Author/Description fields for a plugin:
tes3cmd header \"my plugin.esp\"

# Set the Author field to \"john.moonsugar\":
tes3cmd header --author john.moonsugar \"plugin.esp\"

# Append \" and friends\" to the Author field:
tes3cmd header --author \"+ and friends\" \"plugin.esp\"

# Append a Version number to a plugin Description field:
tes3cmd header --description \"+\\nVersion: 1.0\" \"plugin.esp\"

# update header field for the number of records in the plugin (if incorrect)
# and sync the list of masters to the masters installed in \"Data Files\"
tes3cmd header --synchronize \"my plugin.esp\"
"),
    ("help", &[], ""),
    ("modify", &["assert", "debug", "output=s", "verbose", "backup-dir=s", "hide-backups", "active", "exact-id=s@", "exterior", "flag=s@", "id=s@", "ignore-plugin=s@", "interior", "match=s@", "no-match|M=s@", "program-file=s", "replace=s", "replacefirst=s", "run=s", "sub-match=s", "sub-no-match=s", "type=s@"], "Usage: tes3cmd modify OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --backup-dir
\twhere backup files get stored. The default is:
\t\"morrowind/tes3cmd/backups\"

 --exact-id* <id-string>
\tonly modify records whose ids exactly match given <id-string>

 --exterior
\tonly match if record is an Exterior CELL

 --flag* <flag>
\tonly modify records with given flag. Flags may be given symbolically
\tas: (deleted, persistent, ignored, blocked), or via their numeric
\tvalues (i.e. persistent is 0x400).

 --hide-backups
\tany created backup files will get stashed in the backup directory
\t(--backup-dir)

 --id* <id-regex>
\tonly modify records whose ids match regular expression pattern
\t<id-regex>

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --interior
\tonly match if record is an Interior CELL

 --match* <regex>
\tonly modify records that match given regular expression <regex>
\tIf more than one of these switches is supplied, ALL must match
\tfor a record to be processed.

 --no-match*|--M* <regex>
\tonly modify records that do not match given regular expression
\t<regex>
\tIf more than one of these switches is supplied, if ANY match,
\tthat record will be skipped.

 --program-file <file>
\tload Perl code to run on each matched record from file named: <file>

 --replace \"/a/b/\"
\treplace regular expression pattern \"a\" with value \"b\" for every field
\tvalue in every matching record. you can use any character instead of
\tthe slash as long as it does not occur in \"a\" or \"b\". This is a very
\tpowerful alternative to the --run option, as it only requires
\tunderstanding of regular expressions, no perl coding is necessary.

 --replacefirst \"/a/b/\"
\tjust like --replace, but only replaces the first match.

 --run \"<code>\"
\tspecify a string of Perl <code> to run on each matched record

 --sub-match <regex>
\tonly modify the subrecords that match <regex>. When this option (or
\t--sub-no-match) are used, modification only applies to matching
\tsubrecords. Without either of these options modifications are
\tperformed on whole records.

 --sub-no-match <regex> only modify the subrecords that do not match <regex>.
\t(see also --sub-match above).

 --type* <record-type>
\tonly modify records with the given <record-type>

 Note: starred (*) options can be repeated, and matching of strings is not
 case-sensitive.

This command allows you to do complex batch modifications of plugin records.
You can really damage things with this command, so be careful!

Note: documentation for regular expressions:
  http://perldoc.perl.org/perlre.html

Example(s):

# Just print the the cell \"Ashmelech\" from Morrowind.esm. (no modification).
tes3cmd modify --type cell --id ashmelech --run \"$R->dump\" Morrowind.esm

# Add the prefix \"PC_\" to the ID of all the statics in a plugin:
tes3cmd modify --type stat --sub-match \"id:\" --replace \"/^/PC_/\" pc_data.esp

# Problem: Aleanne's clothing mods do not have restocking inventory
# Solution: create a small patch to change the counts for inventory containers
#   to negative numbers so they will be restocking.
# Step 0: confirm the problem, showing the non-negative counts:
tes3cmd dump --type cont ale_clothing_v?.esp
# Step 1: Create the patch file ale_patch.esp containing just the container records:
tes3cmd dump --type cont --raw-with-header ale_patch.esp ale_clothing_v?.esp
# Step 2: Change all the count fields for the containers in Aleanne's Clothing to -3 (for restocking wares)
tes3cmd modify --type cont --run \"$R->set({f=>'count'}, -3)\" ale_patch.esp
# Note: on Linux, the quoting would be a little different:
tes3cmd modify --type cont --run '$R->set({f=>\"count\"}, -3)' ale_patch.esp

# You can also specify a list of indices to restrict which subrecords are modified.
# First, show the subrecord indices for container \"_aleanne_chest\" from: ale_clothing_v0.esp:
tes3cmd modify --type cont --run \"$R->dump\" ale_clothing_v0.esp
# Then only modify the last 3 items:
tes3cmd modify --type cont --run \"$R->set({i=>[-3..-1],f=>'count'}, 4)\" ale_clothing_v0.esp
"),
    ("multipatch", &["assert", "debug", "output=s", "verbose", "cellnames", "delete-creature|dc=s@", "delete-item|di=s@", "fogbug", "merge-lists", "merge-objects", "no-activate", "no-cache", "summons-persist"], "Usage: tes3cmd multipatch

OPTIONS:
 --debug
\tturn on debug messages

 --cellnames
\tresolve conflicts with renamed external cells

 --fogbug
\tfix interior cells with the fog bug

 --merge-lists
\tmerges leveled lists used in your active plugins. Note that when you
\tuse this feature, you do NOT also need other merged lists plugins like
\t\"Mashed Lists.esp\" from Wrye Mash.

\tThis option has the following sub-options which can be used to remove
\tcreatures and items (by exact id match) from all leveled lists in
\twhich they occur:

\t--delete-creature*|--dc* <creature-id>
\t--delete-item*|--di* <item-id>

 --merge-objects
\tmerges leveled lists used in your active plugins. Note that when you
\tuse this feature, you do NOT also need other merged object plugins like
\t\"Merged_Objects.esp\" from TesTool. (NOT IMPLEMENTED YET).

 --no-activate
\tdo not automatically activate multipatch.esp

 --no-cache
\tdo not create cache files (used for speedier operation)

 --summons-persist
\tfixes summoned creatures crash by making them persistent

DESCRIPTION:

The multipatch produces a patch file based on your current load order to solve
various problems. You should regenerate your multipatch whenever you change
your load order. The goal of the \"multipatch\" command is that it should always
be safe to use it with no options to get the default patching behavior (if you
do find any problems, please report them and they will be fixed ASAP). When no
options are specified, the following default options are assumed:

  --cellnames --fogbug --merge-lists --merge-objects --summons-persist

The different patching operations are explained below:

Cell Name Patch (--cellnames)

  Creates a patch to ensure renamed cells are not accidentally reverted to
  their original name.

  This solves the following plugin conflict that causes bugs:
  * Master A names external CELL (1, 1) as: \"\".
  * Plugin B renames CELL (1, 1) to: \"My City\".
  * Plugin C modifies CELL (1, 1), using the original name \"\", reverting
    renaming done by plugin B.
  * References in plugin B (such as in scripts) that refer to \"My City\" break.

  This option works by scanning your currently active plugin load order for
  cell name reversions like those in the above example, and ensures whenever
  possible that cell renaming is properly maintained.

Fog Bug Patch (--fogbug)

  Some video cards are affected by how Morrowind handles a fog density setting
  of zero in interior cells with the result that the interior is pitch black,
  except for some light sources, and no amount of light, night-eye, or gamma
  setting will make the interior visible. This is known as the \"fog bug\".

  This option creates a patch that fixes all fogbugged cells in your active
  plugins by setting the fog density of those cells to a non-zero value.

Merge Leveled Lists (--merge-lists)

  This feature is similar to what you get with Wrye Mash's \"Mashed Lists.esp\".
  However, you should not use more than one plugin to merge leveled lists.

Merge Objects (--merge-objects)

  This feature is similar to what you get with TesTools's \"Merged_Objects.esp\".
  However, you should not use more than one plugin to merge objects.
  (NOT IMPLEMENTED YET)

Summoned creatures persists (--summons-persist)

  There is a bug in Morrowind that can cause the game to crash if you leave a
  cell where an NPC has summoned a creature. The simple workaround is to flag
  summoned creatures as persistent. The Morrowind Patch Project implements
  this fix, however other mods coming later in the load order often revert it.
  This option to the multipatch ensures that known summoned creatures are
  flagged as persistent. The Morrowind Code Patch also fixes this bug, making
  this feature redundant.

EXAMPLES:

# Create the patch plugin \"multipatch.esp\" with all default patch options on:
tes3cmd multipatch
"),
    ("overdial", &["assert", "debug", "output=s", "verbose", "active", "ignore-plugin=s@", "single"], "Usage: tes3cmd overdial OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --active
\tonly process specified plugins that are active. If none specified,
\tprocess all active plugins. Plugins will be processed in the same
\torder as your normal game load order.

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

 --single
\tonly test to see if dialog in the first plugin is overlapped. (by
\tdefault all plugins are checked against all other plugins, which is an
\tn-squared operation, meaning \"possibly very slow\").

DESCRIPTION:

Prints the IDs of dialog records that overlap from the set of given plugins.

An overlap is defined as a dialog (DIAL) Topic from one plugin that entirely
contains a dialog Topic from another plugin as a substring. For example, the
mod \"White Wolf of Lokken\" has a dialog topic \"to rescue me\" which overlaps
with the dialog topic \"rescue me\" from \"Suran Underworld\", which causes the
\"Special Guest\" quest from SU to get stuck because Ylarra will not offer the
topic \"rescue me\" when you find her in her cell.

Note that overlap is only a potential problem if the plugins are loaded in the
order they are listed in the output.

Example(s):

# Show dialog overlaps between Lokken and SU:
tes3cmd overdial \"BT_Whitewolf_2_0.esm\" \"Suran_Underworld_2.5.esp\"
"),
    ("recover", &["assert", "debug", "output=s", "verbose", "ignore-plugin=s@", "backup-dir=s", "hide-backups"], "Usage: tes3cmd recover OPTIONS plugin...

OPTIONS:
 --debug
\tturn on debug messages

 --backup-dir
\twhere backup files get stored. The default is:
\t\"morrowind/tes3cmd/backups\"

 --hide-backups
\tany created backup files will get stashed in the backup directory
\t(--backup-dir)

 --ignore-plugin* <plugin-name>
        skips specified plugin if it appears on command line. plugin name is
        matched by exact, but caseless, string comparison.

DESCRIPTION:

Attempts to recover readable records from a damaged plugin. You should only
use this when Morrowind gives the following type of error on your plugin:

  \"Trying to load a bad form in TES3File::NextForm\"

The main reason you would get this error is if the file has been physically
corrupted, where records have been overwritten with random binary junk, or
if the file has been truncated, or otherwise damaged.

This is not for fixing what is commonly referred to as \"savegame corruption\",
which is almost always not actual corruption but bad data.

In any case, you will get detailed output on what tes3cmd finds damaged.

EXAMPLE(S):

# fix my damaged plugin:
tes3cmd recover \"my plugin.esp\"
"),
    ("resetdates", &["assert", "debug", "output=s", "verbose"], "Usage: tes3cmd resetdates

OPTIONS:
 --debug
\tturn on debug messages

DESCRIPTION:

Resets the dates of the Bethesda masters (.esm) and archives (.bsa) to their
original settings. This may help those people who have problems with textures
and meshes being overridden by the vanilla resources (e.g. as can happen with
the Steam version of Morrowind).

Example(s):

# fix the date settings of Bethesda data files:
tes3cmd resetdates
"),
];
