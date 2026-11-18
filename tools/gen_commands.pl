no warnings;
@ARGV=();
do "./lib.pl"; die "load: $@" if $@;
sub rs { my $s = shift; $s =~ s/\\/\\\\/g; $s =~ s/"/\\"/g; $s =~ s/\r/\\r/g; $s =~ s/\t/\\t/g; $s =~ s/([^\x20-\x7e\n])/sprintf("\\u{%x}",ord($1))/ge; $s }
print "// Generated from tes3cmd 0.40-PRE-RELEASE-2's %COMMAND by tools/gen_commands.pl - do not edit.\n";
print "// (name as dispatched, each Getopt::Long spec, the usage text die() prints)\n";
print "pub const COMMANDS: &[(&str, &[&str], &str)] = &[\n";
for my $c (sort keys %COMMAND) {
  my @specs = grep { !ref } @{$COMMAND{$c}{options} || []};
  my $u = $COMMAND{$c}{usage}; $u = '' unless defined $u;
  print "    (\"" . rs($c) . "\", &[" . join(", ", map { "\"" . rs($_) . "\"" } @specs) . "], \"" . rs($u) . "\"),\n";
}
print "];\n";
