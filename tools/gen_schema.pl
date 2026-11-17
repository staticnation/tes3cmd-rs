no warnings;
my @args=@ARGV; @ARGV=();
do "./lib.pl"; die "load: $@" if $@;
print "// Generated from tes3cmd 0.40-PRE-RELEASE-2's @RECDEFS by tools/gen_schema.pl - do not edit.\n";
print "// (rectype, subtype, decode template, encode template, each field's slot - a name used twice
// shares one, as in the script's hash; \"\" templates: the script's own codec)\n";
print "pub const SCHEMA: &[(&str, &str, &str, &str, &[u8])] = &[\n";
for my $def (@TES3::Record::RECDEFS) {
  my ($rt, $subs, $opt) = @$def;
  my @subs = @$subs;
  push @subs, ['DELE', [["DELETED", "L"]]];
  for my $s (@subs) {
    my ($st, $fields, $o) = @$s;
    my ($dec, $enc) = ('', '');
    my (@slots, %slot_of, %suffix);
    my $custom = (ref($o) eq 'HASH' && ($o->{decode} || $o->{encode}));
    if (ref($fields) eq 'ARRAY' && @$fields && !$custom) {
      for my $f (@$fields) {
        next unless ref($f) eq 'ARRAY';
        my $fmt = $f->[1];
        my ($d, $e) = ref($fmt) eq 'ARRAY' ? @$fmt : ($fmt, $fmt);
        ($d) = split(/:/, $d, 2);
        unless (ref($fmt) eq 'ARRAY') { ($e) = split(/:/, $e, 2); }
        $dec .= $d; $enc .= $e;
        my $name = lc($f->[0]);
        if (my ($base) = ($name =~ /^(.+)_\#$/)) { $name = $base . "_" . ++$suffix{$base}; }
        $slot_of{$name} = scalar(keys %slot_of) unless exists $slot_of{$name};
        push @slots, $slot_of{$name};
      }
    }
    print "    (\"$rt\", \"$st\", \"$dec\", \"$enc\", &[" . join(", ", @slots) . "]),\n";
  }
}
print "];\n";

print "\n// The subrecord whose field is named `id`, and its slot: the record's id (`decode`).\n";
print "pub const ID_FIELD: &[(&str, &str, u8)] = &[\n";
for my $def (@TES3::Record::RECDEFS) {
  my ($rt, $subs, $opt) = @$def;
  for my $s (@$subs) {
    my ($st, $fields) = @$s;
    next unless ref($fields) eq 'ARRAY';
    my (%slot_of, %suffix);
    for my $f (@$fields) {
      next unless ref($f) eq 'ARRAY';
      my $name = lc($f->[0]);
      if (my ($base) = ($name =~ /^(.+)_\#$/)) { $name = $base . "_" . ++$suffix{$base}; }
      $slot_of{$name} = scalar(keys %slot_of) unless exists $slot_of{$name};
      print "    (\"$rt\", \"$st\", $slot_of{$name}),\n" if $name eq 'id';
    }
  }
}
print "];\n";
