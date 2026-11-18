# Writes tests/data/all_records.esp: one record of every type tes3cmd defines (savegame-only
# types left out), each subrecord encoded by tes3cmd's own encoders from made-up values (a
# lookup field gets one of its table's keys), for `dump` to be compared on. With the argument
# `odd`, every lookup field gets a value its table does not have (tests/data/odd_records.esp).
#   cd tools && perl gen_all_records.pl > ../tests/data/all_records.esp
#   cd tools && perl gen_all_records.pl odd > ../tests/data/odd_records.esp
# With `ess`, the savegame-only types too (tests/data/all_records.ess).
#   cd tools && perl gen_all_records.pl ess > ../tests/data/all_records.ess
no warnings;
my $ODD = (@ARGV and $ARGV[0] eq 'odd');
my $ESS = (@ARGV and $ARGV[0] eq 'ess');
@ARGV=();
do "./lib.pl"; die "load: $@" if $@;
binmode STDOUT;
my $n = 0;
sub val {
    my ($name, $fmt, $opt, $i) = @_;
    if ($opt->{lookup} and $ODD) { return 97 + $i % 3; }
    if (my $h = $opt->{lookup}) { my @k = sort { $a <=> $b } grep { /^-?\d+$/ } keys %$h; return $k[$i % @k] if @k; }
    if ($opt->{symflags}) { my @v = sort { $a <=> $b } values %{$opt->{symflags}}; return $v[$i % @v] | $v[($i+1) % @v]; }
    my $f = ref($fmt) eq 'ARRAY' ? $fmt->[0] : $fmt; ($f) = split /:/, $f;
    return sprintf("%02x", $i % 256) x ($f =~ /^H(\d+)/ ? $1/2 : 2) if $f =~ /^H/;
    return "$name $i" if $f =~ /^[aZ]/;
    return $i + 0.25 if $f =~ /^f/;
    return ($i % 5) - 1 if $f =~ /^[cs]/;
    return $i % 200;
}
my @recs;
for my $def (@TES3::Record::RECDEFS) {
    my ($rt, $subs, $ro) = @$def;
    next if !$ESS and ref $ro eq 'HASH' && $ro->{rdflags} && grep { $_ eq 'ess' } @{$ro->{rdflags}};
    next if $rt eq 'TES3' or $rt eq 'CELL';
    my $body = '';
    my $i = 0;
    for my $s (@$subs) {
        my ($st, $fields, $so) = @$s;
        next if !$ESS and ref $so eq 'HASH' && $so->{rdflags} && grep { $_ eq 'ess' } @{$so->{rdflags}};
        next unless ref($fields) eq 'ARRAY' && @$fields;
        next if ref $so eq 'HASH' && $so->{encode};
        my $obj = bless({}, "${rt}::${st}");
        my %suffix;
        for my $f (@$fields) {
            my ($fname, $fmt, $opt) = @$f; $fname = lc $fname;
            if (my ($b) = ($fname =~ /^(.+)_\#$/)) { $fname = $b . "_" . ++$suffix{$b}; }
            $obj->{$fname} = val($fname, $fmt, $opt, $i++);
        }
        $obj->encode;
        # A 2 or 3 byte subrecord tes3cmd prints as unknown data kills the dump (tested apart).
        next if $ESS and ref $so eq 'HASH' and $so->{tostr} and length($obj->{_subbuf_}) =~ /^[23]$/;
        $body .= pack("a4L/a*", $st, $obj->{_subbuf_});
    }
    if ($rt eq 'LAND') {
        $body =~ s/DATA\x02\x00\x00\x00../DATA\x04\x00\x00\x00\x02\x02\x00\x00/s;
        $body .= pack("a4L/a*", 'VHGT', pack("fc4225sc", 12.5, (map { ($_ % 7) - 3 } 0..4224), 5, 1));
        $body .= pack("a4L/a*", 'VNML', pack("C*", map { $_ % 256 } 0..(65*65*3-1)));
        $body .= pack("a4L/a*", 'VCLR', pack("C*", map { ($_*7) % 256 } 0..(65*65*3-1)));
        $body .= pack("a4L/a*", 'VTEX', pack("s*", map { $_ % 9 } 0..255));
    }
    if ($rt eq 'PGRD') {
        $body = pack("a4L/a*", 'DATA', pack("llSS", 2, 3, 128, 3)) . pack("a4L/a*", 'NAME', "wilds\0");
        $body .= pack("a4L/a*", 'PGRP', pack("l3C2H4", 1, 2, 3, 0, 2, "abcd") . pack("l3C2H4", 4, 5, 6, 1, 1, "0000") . pack("l3C2H4", 7, 8, 9, 3, 1, "0102"));
        $body .= pack("a4L/a*", 'PGRC', pack("L*", 1, 2, 0, 0));
    }
    if ($rt eq 'NPC_') { $body .= pack("a4L/a*", 'NPDT', pack("SC8C27CSSSCCCxL", 5, (40..47), (10..36), 3, 50, 60, 70, 30, 1, 2, 99)); }
    if ($rt eq 'REGN') { $body .= pack("a4L/a*", 'WEAT', pack("C10", 10..19)); }
    if ($ESS and grep { $_->[0] eq 'SLFD' } @$subs) {
        $body .= pack("a4L/a*", 'SLFD', pack("f*", 1.5, -2.25, 0.125)) . pack("a4L/a*", 'SLLD', pack("l*", 7, -8))
            . pack("a4L/a*", 'SLSD', pack("s*", 3, -4, 5));
    }
    if ($ESS and grep { $_->[0] eq 'CHRD' } @$subs) { $body .= pack("a4L/a*", 'CHRD', pack("l*", 1, 2, 3)); }
    if ($ESS and $rt eq 'REFR') { $body = pack("a4L/a*", 'FRMR', pack("L", 9 | (2 << 24))) . $body; }
    if ($rt eq 'DIAL') { $body = pack("a4L/a*", 'NAME', "Greeting 1\0") . pack("a4L/a*", 'DATA', pack("C", 1)); }
    next unless length $body;
    push @recs, pack("a4LLLa*", $rt, length($body), 0, ($n++ % 3 == 0 ? 0x400 : 0), $body);
}
# A cell of each kind, with references and a moved one.
my $cell = pack("a4L/a*", 'NAME', "Test Room\0") . pack("a4L/a*", 'DATA', pack("LLf", 1|4, 0, 0.5))
    . pack("a4L/a*", 'AMBI', pack("LLLf", 0x101010, 0x202020, 0x303030, 0.75))
    . pack("a4L/a*", 'FRMR', pack("L", 1)) . pack("a4L/a*", 'NAME', "rock_01\0") . pack("a4L/a*", 'XSCL', pack("f", 1.5))
    . pack("a4L/a*", 'CNAM', "Hlaalu\0") . pack("a4L/a*", 'INTV', pack("l", 30)) . pack("a4L/a*", 'DATA', pack("f6", 1, 2, 3, 0.1, 0.2, 0.3))
    . pack("a4L/a*", 'NAM0', pack("L", 1))
    . pack("a4L/a*", 'FRMR', pack("L", 2 | (1 << 24))) . pack("a4L/a*", 'NAME', "door_02\0") . pack("a4L/a*", 'DODT', pack("f6", 9, 8, 7, 0, 0, 1.5))
    . pack("a4L/a*", 'DNAM', "Balmora\0") . pack("a4L/a*", 'DATA', pack("f6", 4, 5, 6, 0, 0, 0));
push @recs, pack("a4LLLa*", 'CELL', length($cell), 0, 0, $cell);
my $ext = pack("a4L/a*", 'NAME', "\0") . pack("a4L/a*", 'DATA', pack("Lll", 2, -3, 7)) . pack("a4L/a*", 'RGNN', "Ascadian Isles Region\0")
    . pack("a4L/a*", 'MVRF', pack("L", 5)) . pack("a4L/a*", 'CNAM', "Balmora\0") . pack("a4L/a*", 'FRMR', pack("L", 5)) . pack("a4L/a*", 'NAME', "crate_01\0")
    . pack("a4L/a*", 'DATA', pack("f6", 1, 1, 1, 0, 0, 0));
push @recs, pack("a4LLLa*", 'CELL', length($ext), 0, 0x400, $ext);
my $hedr = pack("fLa32a256L", 1.3, 0, "tes3cmd-rs", "every record type", scalar @recs);
my $tes3 = pack("a4L/a*", 'HEDR', $hedr);
print pack("a4LLLa*", 'TES3', length($tes3), 0, 0, $tes3), @recs;
