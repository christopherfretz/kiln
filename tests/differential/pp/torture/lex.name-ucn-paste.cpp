//@ expect: pp
// Identifiers with universal-character-names and UTF-8, through ## and #:
// pasting forms one identifier, and spellings are kept as written.
#define CAT(a, b) a ## b
#define STR(x) #x
CAT(caf, é) CAT(é, t) CAT(x, \U00000394) CAT(Δ, Δ)
CAT(na, ïve) CAT(ï, é)
CAT(e, \N{LATIN SMALL LETTER E WITH ACUTE})
STR(é) STR(café) STR(CAT(a, é))
#define été summer
été été
