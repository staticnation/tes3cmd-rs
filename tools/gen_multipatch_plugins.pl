# Writes the multipatch fixtures, tests/data/mp_*.es?: a master and three plugins that redefine
# its leveled lists (three times and more, so they merge), rename an exterior cell and revert the
# renaming, give two interiors no fog density, and drop a summoned creature's persistence.
#   cd tests/data && perl ../../tools/gen_multipatch_plugins.pl
sub sr { pack("a4L/a*", @_) }
sub rec { my ($t, $f, @s) = @_; my $b = join("", @s); pack("a4LLLa*", $t, length $b, 0, $f, $b) }
sub plug {
    my ($masters, @recs) = @_;
    my $h = sr('HEDR', pack("fLa32a256L", 1.3, 0, "a", "d", scalar @recs));
    $h .= join("", map { sr('MAST', "$_->[0]\0") . sr('DATA', pack("LL", $_->[1], 0)) } @$masters);
    rec('TES3', 0, $h) . join("", @recs);
}
sub levi {
    my ($id, $flags, $chance, @items) = @_;
    rec('LEVI', 0, sr('NAME', "$id\0"), sr('DATA', pack("L", $flags)), sr('NNAM', pack("C", $chance)),
        sr('INDX', pack("L", scalar @items)), map { sr('INAM', "$_->[0]\0") . sr('INTV', pack("S", $_->[1])) } @items);
}
sub levc {
    my ($id, @items) = @_;
    rec('LEVC', 0, sr('NAME', "$id\0"), sr('DATA', pack("L", 1)), sr('NNAM', pack("C", 0)),
        sr('INDX', pack("L", scalar @items)), map { sr('CNAM', "$_->[0]\0") . sr('INTV', pack("S", $_->[1])) } @items);
}
sub icell { my ($name, $fogd, $foga, $flags) = @_; rec('CELL', 0, sr('NAME', "$name\0"), sr('DATA', pack("LLf", $flags, 0, $fogd)), sr('AMBI', pack("LLLf", 1, 2, 3, $foga))) }
sub ecell { my ($name, $x, $y) = @_; rec('CELL', 0, sr('NAME', "$name\0"), sr('DATA', pack("Lll", 2, $x, $y))) }
sub crea {
    my ($id, $flags) = @_;
    rec('CREA', $flags, sr('NAME', "$id\0"), sr('MODL', "c.nif\0"), sr('FNAM', "Crit\0"), sr('NPDT', pack("L24", (1) x 24)), sr('FLAG', pack("L", 8)));
}
sub out { my ($name, $data) = @_; open(my $f, '>', $name) or die "$name: $!"; binmode $f; print $f $data; close $f; }

my $m = plug([], levi("list_a", 1, 10, ["sword", 1], ["axe", 2]), levc("crit_a", ["rat", 1]), ecell("Old Name", 1, 2),
             crea("scamp_summon", 0x400), icell("Dark Room", 0.5, 0.5, 1));
out("mp_Master.esm", $m);
my @mast = (["mp_Master.esm", length $m]);
out("mp_A.esp", plug(\@mast, levi("list_a", 1, 20, ["sword", 1], ["bow", 3]), levc("crit_a", ["rat", 1], ["wolf", 5]),
                     ecell("New Name", 1, 2), crea("scamp_summon", 0), icell("Dark Room", 0, 0, 1)));
out("mp_B.esp", plug(\@mast, levi("list_a", 3, 30, ["sword", 1], ["axe", 2], ["spear", 4]), levc("crit_a", ["rat", 1], ["bear", 9]),
                     ecell("Old Name", 1, 2), icell("Fog Room", 0.0, 0.25, 1)));
out("mp_C.esp", plug(\@mast, levi("list_a", 1, 40, ["axe", 2], ["dagger", 1]), levi("list_b", 0, 5, ["x", 1]), ecell("Other", 3, 4)));
