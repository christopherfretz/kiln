//@ expect: pp
//@ requires: clang>=22
// Identifiers with universal-character-names and UTF-8, through ## and #
// ([lex.name], [cpp.concat], [cpp.stringize]): pasting forms one identifier.
// clang 22's -E prints every identifier containing a UCN as UTF-8, pasted or
// not (clang 19 kept the UCN spelling: lex.name-ucn-paste-19.cpp); kiln does
// the same under -fclang-compat=22 (spec/pp.md).
#define CAT(a, b) a ## b
#define STR(x) #x
CAT(caf, é) CAT(é, t) CAT(x, \U00000394) CAT(Δ, Δ)
CAT(na, ïve) CAT(ï, é)
CAT(e, \N{LATIN SMALL LETTER E WITH ACUTE})
STR(é) STR(café) STR(CAT(a, é))
#define été summer
été été
