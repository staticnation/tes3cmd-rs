no warnings;
@ARGV=();
do "./lib.pl"; die "load: $@" if $@;
my $src = do { local $/; open my $f, '<', '../src/tostr_table.rs'; <$f> };
my @tabs;
while ($src =~ /^    \('([LF])', &\[(.*?)\]\),$/mg) { my ($k,$body)=($1,$2); my %h; while ($body =~ /\("((?:[^"\\]|\\.)*)", "((?:[^"\\]|\\.)*)"\)/g) { $h{$1}=$2 } push @tabs, [$k,\%h]; }
my ($ok, $bad, $n) = (0,0,0);
while ($src =~ /^    \("(\w+)", "(\w+)", (true|false), "((?:[^"\\]|\\.)*)", &\[(.*?)\], &\[(.*?)\]\),$/mg) {
  my ($rt,$st,$c,$pf,$fn,$k)=($1,$2,$3,$4,$5,$6);
  next if $c eq 'true';
  $pf =~ s/\\n/\n/g; $pf =~ s/\\t/\t/g; $pf =~ s/\\"/"/g; $pf =~ s/\\\\/\\/g;
  my @f = $fn =~ /"([^"]*)"/g; my @kk = $k =~ /"([^"]*)"/g;
  my %v; my @vals; my $i = 0;
  for my $j (0..$#f) {
    my $name=$f[$j]; my $kind=$kk[$j];
    if ($kind =~ /^L(\d+)/) { my ($key) = sort keys %{$tabs[$1][1]}; $v{$name}=$key; push @vals, "($tabs[$1][1]{$key})"; }
    elsif ($kind =~ /^F(\d+)/) { $v{$name}=0; push @vals, "[" . TES3::Record::flags_tostr(0, {map { $_ => $tabs[$1][1]{$_} } keys %{$tabs[$1][1]}}) . "]"; }
    else { $v{$name} = ++$i; push @vals, $i; }
  }
  my $pkg = "${rt}::${st}";
  my $o = bless({%v}, $pkg);
  my $got = eval { $o->tostr }; 
  my $want = sprintf($pf, @vals);
  $n++;
  if (defined $got && $got eq $want) { $ok++ } else { $bad++; print "MISMATCH $pkg\n  perl: $got\n  gen:  $want\n" if $bad < 6; }
}
print "checked $n: $ok same, $bad differ\n";
