//@ expect: pp
//@ requires: clang<22
// Identifiers with universal-character-names and UTF-8, through ## and #
// ([lex.name], [cpp.concat], [cpp.stringize]): pasting forms one identifier.
// clang 19's -E keeps a pasted identifier's UCN spelling as written; clang 22
// prints it as UTF-8 (lex.name-ucn-paste.cpp). kiln follows -fclang-compat.
#define CAT(a, b) a ## b
#define STR(x) #x
CAT(caf, é) CAT(é, t) CAT(x, \U00000394) CAT(Δ, Δ)
CAT(na, ïve) CAT(ï, é)
CAT(e, \N{LATIN SMALL LETTER E WITH ACUTE})
STR(é) STR(café) STR(CAT(a, é))
#define été summer
été été
