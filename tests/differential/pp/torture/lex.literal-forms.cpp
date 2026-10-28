//@ expect: pp
// Literal tokens pass through unchanged: every encoding prefix, raw
// strings, user-defined-literal suffixes, pp-numbers that are not valid
// numbers, and digit separators.
'a' u8'a' u'a' U'a' L'a' 'ab' '\'' '\x41' 'é' '\N{DIGIT ONE}'
"" "a" u8"a" u"a" U"a" L"a" "a\"b" "\\" "é" "é"
R"(raw)" u8R"x(raw)x" LR"(raw)" uR"(raw)" UR"(raw)"
"str"_suffix 'c'_suffix 12_km 1.5_m 0x1p-3 1e+10 1e-10 .5 5. 1'234'567
0x1.fp1 1.2.3.4 0xe+1 1..2 08 0b2 12ABC 1_e+2
