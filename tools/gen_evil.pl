no warnings;
@ARGV=();
do "./lib.pl"; die "load: $@" if $@;
print "//! The evil GMSTs `clean` removes (`%EVIL_BM`, `%EVIL_TB`): the id, and the value subrecord's\n";
print "//! type, a space and its bytes, hex-encoded. Generated from tes3cmd 0.40-PRE-RELEASE-2 by\n";
print "//! tools/gen_evil.pl - do not edit.\n\n";
for my $t (['EVIL_BM', \%main::EVIL_BM, 'Bloodmoon'], ['EVIL_TB', \%main::EVIL_TB, 'Tribunal']) {
  my ($name, $h, $what) = @$t;
  print "/// $what's.\npub const $name: &[(&str, &str)] = &[\n";
  print "    (\"$_\", \"$h->{$_}\"),\n" for sort keys %$h;
  print "];\n\n";
}
