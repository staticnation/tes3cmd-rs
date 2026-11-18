no warnings;
@ARGV=();
do "./lib.pl"; die "load: $@" if $@;
use Scalar::Util qw(refaddr);
sub rs { my $s = shift; $s =~ s/\\/\\\\/g; $s =~ s/"/\\"/g; $s =~ s/\n/\\n/g; $s =~ s/\t/\\t/g; $s =~ s/([^\x20-\x7e])/sprintf("\\u{%x}",ord($1))/ge; $s }
my (%tab_id, @tabs);
sub tab { my ($h, $kind) = @_; my $a = refaddr($h); unless (exists $tab_id{$a}) { $tab_id{$a} = scalar @tabs; push @tabs, [$kind, $h]; } $tab_id{$a} }
my @rows;
for my $def (@TES3::Record::RECDEFS) {
  my ($rectype, $subs, $ropt) = @$def;
  for my $s (@$subs) {
    my ($subtype, $fields, $sopt) = @$s;
    my $package = "${rectype}::${subtype}";
    my $custom = (ref($sopt) eq 'HASH' && $sopt->{tostr}) ? 1 : 0;
    next unless ref($fields) eq 'ARRAY' && @$fields;
    my (@fieldnames, %suffix, @formats, %lookup, %symflags);
    for my $f (@$fields) {
      my ($fieldname, $fmt, $opt) = @$f;
      $fieldname = lc($fieldname);
      if (my ($b) = ($fieldname =~ /^(.+)_\#$/)) { $fieldname = $b . "_" . ++$suffix{$b}; }
      push @fieldnames, $fieldname;
      my ($pfmt, $custom_format);
      if (ref($fmt) eq 'ARRAY') { my ($d) = @$fmt; ($d, $custom_format) = split(/:/, $d, 2); $pfmt = $d; }
      else { ($fmt, $custom_format) = split(/:/, $fmt, 2); $pfmt = $fmt; }
      $lookup{$fieldname} = $opt->{lookup} if $opt->{lookup};
      $symflags{$fieldname} = $opt->{symflags} if $opt->{symflags};
      my $label = $fieldname;
      $label =~ s/([[:alnum:]]+)/ucfirst($1)/ge;
      $label =~ s/^(..)$/uc($1)/e;
      $label =~ s/_id$/_ID/i;
      $label =~ s/idx$/Idx/i;
      if (defined($custom_format)) { push(@formats, ($custom_format) ? "$label:$custom_format" : $label); }
      elsif ($pfmt =~ /^[aZH]/) { push @formats, "$label:%s"; }
      elsif ($pfmt =~ /^f/) { push @formats, "$label:\%0.2f"; }
      elsif ($pfmt =~ /^[LCS]/i) { push @formats, (defined $lookup{$fieldname} or defined $symflags{$fieldname}) ? "$label:%s" : "$label:%d"; }
      else { die "($package): Don't know how to stringify: \"$pfmt\""; }
    }
    my $print_format;
    if (my $ncol = $TES3::Record::COLUMNS{$package}) {
      my $width = int(78 / $ncol); my $n = 0; my (@cl, @line); my @fnames = @fieldnames; my @fm = @formats;
      while (my $fmt = shift @fm) {
        my $fname = shift @fnames; my $fwidth = $width - length($fname);
        $fmt =~ s/%\d*/\%${fwidth}/;
        if ($TES3::Record::FORMAT_INFO{$rectype}->{$subtype}->{$fname}->{BOL}) { if (@line) { push @cl, join("  ", @line); @line = (); $n = 0; } }
        push @line, $fmt;
        if (++$n == $ncol) { push @cl, join("  ", @line); @line = (); $n = 0; }
      }
      push @cl, join("  ", @line) if @line;
      $print_format = join("\n\t", @cl);
    } else {
      my $last = ' ';
      for my $fmt (@formats) { if ($last =~ /\s$/) { $print_format .= $fmt } else { $print_format .= '  ' . $fmt } $last = $fmt; }
    }
    my @kinds = map { defined $lookup{$_} ? "L" . tab($lookup{$_}, 'L') : defined $symflags{$_} ? "F" . tab($symflags{$_}, 'F') : "" } @fieldnames;
    push @rows, [$rectype, $subtype, $custom, $print_format, \@fieldnames, \@kinds];
  }
}
print "// Generated from tes3cmd 0.40-PRE-RELEASE-2 by tools/gen_tostr.pl - do not edit.\n";
print "#![allow(clippy::type_complexity)]\n";
print "// (rectype, subtype, own tostr, the generated tostr's sprintf format, field names, each\n";
print "// field's table: \"L<n>\" a lookup, \"F<n>\" symbolic flags, \"\" its value as it is)\n";
print "pub const TOSTR: &[(&str, &str, bool, &str, &[&str], &[&str])] = &[\n";
for my $r (@rows) {
  my ($rt, $st, $c, $pf, $fn, $k) = @$r;
  print "    (\"$rt\", \"$st\", " . ($c ? "true" : "false") . ", \"" . rs($pf) . "\", &[" . join(", ", map {"\"$_\""} @$fn) . "], &[" . join(", ", map {"\"$_\""} @$k) . "]),\n";
}
print "];\n\n// The tables, by number: a lookup maps a value to its name; flags map a name to its bits.\n";
print "pub const TABLES: &[(char, &[(&str, &str)])] = &[\n";
for my $t (@tabs) {
  my ($kind, $h) = @$t;
  my @pairs = map { "(\"" . rs($_) . "\", \"" . rs($h->{$_}) . "\")" } sort keys %$h;
  print "    ('$kind', &[" . join(", ", @pairs) . "]),\n";
}
print "];\n";
print "\n// tes3cmd's named tables (package TES3), for the formatters written by hand: key, value; a\n// table of tables (ARMOR_CLASS) as \"outer.inner\".\n";
print "pub const NAMED: &[(&str, &[(&str, &str)])] = &[\n";
for my $name (qw(AIDT_FLAGS APPARATUS_TYPE ARMOR_INDEX ARMOR_TYPE ARMOR_CLASS ATTRIBUTE AUTOCALC_FLAGS BIPED_OBJECT BYDT_PART BYDT_FLAGS BYDT_PTYP BYDT_SKIN_TYPE CELL_FLAGS CONTAINER_FLAGS CREATURE_MOVEMENT_FLAGS CREATURE_BLOOD_FLAGS CREATURE_FLAGS CREA_TYPE CTDT_TYPE DIAL_TYPE ENCHANT_TYPE ENDT_AUTO FACT_FLAGS GLOBAL_TYPE INFO_FUN INFO_SCVR_CMP INFO_SCVR_TYPE LEVC_FLAGS LEVI_FLAGS LHDT_FLAGS MAGIC_SCHOOL MGEF_FLAGS NPC_FLAGS PLAYABLE PGCOLOR RADT_FLAGS RANGE_TYPE HDR_FLAGS SEX SKILL SKILL_ACTIONS SNDG_DATA SPECIALIZATION SPEL_FLAGS SPEL_TYPE SPELL_EFFECT SPLM_TYPE TRUTH WEAPON_FLAGS WEAPON_TYPE YESNO FACTION_RANK)) {
  no strict 'refs';
  my $h = \%{"TES3::$name"};
  $h = \%{"TES3::Record::$name"} unless %$h;
  my @pairs;
  for my $k (sort keys %$h) {
    my $v = $h->{$k};
    if (ref $v eq 'HASH') { push @pairs, map { "(\"" . rs("$k.$_") . "\", \"" . rs($v->{$_}) . "\")" } sort keys %$v; }
    elsif (ref $v eq 'ARRAY') { push @pairs, "(\"" . rs($k) . "\", \"" . rs(join("\x1f", @$v)) . "\")"; }
    else { push @pairs, "(\"" . rs($k) . "\", \"" . rs($v) . "\")"; }
  }
  warn "empty: $name\n" unless @pairs;
  print "    (\"$name\", &[" . join(", ", @pairs) . "]),\n";
}
print "];\n";
